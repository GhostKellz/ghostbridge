use std::sync::Arc;
use tokio::sync::RwLock;
use tonic::{transport::Channel, Request, Response, Status};
use tracing::{debug, error, info};

use crate::ghost::chain::v1::{
    ghost_chain_service_client::GhostChainServiceClient,
    DomainQuery, DomainResponse,
    AccountQuery, AccountResponse,
    BalanceQuery, BalanceResponse,
    BlockQuery, BlockResponse,
    DomainSubscription, DomainEvent,
};
use crate::ghost::dns::v1::{
    ghost_dns_service_client::GhostDnsServiceClient,
    DnsStats, CacheStats,
};
use crate::ghost::common::v1::Empty;
use crate::connection_pool::ConnectionPool;
use crate::quic_transport::QuicTransport;

#[derive(Debug, thiserror::Error)]
pub enum GhostBridgeError {
    #[error("Connection error: {0}")]
    Connection(#[from] tonic::transport::Error),
    
    #[error("Request failed: {0}")]
    Request(#[from] Status),
    
    #[error("Invalid configuration: {0}")]
    Config(String),
    
    #[error("Invalid URI: {0}")]
    InvalidUri(#[from] http::uri::InvalidUri),
    
    #[error("QUIC connection error: {0}")]
    QuicConnection(String),
    
    #[error("QUIC write error: {0}")]
    QuicWrite(String),
    
    #[error("QUIC read error: {0}")]
    QuicRead(String),
    
    #[error("QUIC stream closed: {0}")]
    QuicClosed(String),
}

pub type Result<T> = std::result::Result<T, GhostBridgeError>;

#[derive(Clone)]
pub struct GhostBridgeClient {
    chain_pool: Arc<ConnectionPool<GhostChainServiceClient<Channel>>>,
    dns_pool: Arc<ConnectionPool<GhostDnsServiceClient<Channel>>>,
    quic_transport: Option<Arc<QuicTransport>>,
    config: ClientConfig,
}

#[derive(Clone)]
pub struct ClientConfig {
    pub endpoint: String,
    pub enable_quic: bool,
    pub pool_size: usize,
    pub request_timeout: std::time::Duration,
    pub enable_compression: bool,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            endpoint: "http://127.0.0.1:9090".to_string(),
            enable_quic: true,
            pool_size: 4,
            request_timeout: std::time::Duration::from_secs(10),
            enable_compression: true,
        }
    }
}

pub struct GhostBridgeClientBuilder {
    config: ClientConfig,
}

impl GhostBridgeClient {
    pub fn builder() -> GhostBridgeClientBuilder {
        GhostBridgeClientBuilder {
            config: ClientConfig::default(),
        }
    }

    pub async fn connect(endpoint: impl Into<String>) -> Result<Self> {
        Self::builder()
            .endpoint(endpoint)
            .build()
            .await
    }

    // Chain service methods
    pub async fn resolve_domain(
        &self,
        domain: impl Into<String>,
        record_types: Vec<String>,
    ) -> Result<DomainResponse> {
        let request = Request::new(DomainQuery {
            domain: domain.into(),
            record_types,
        });

        let mut client = self.chain_pool.get().await?;
        let response = client.resolve_domain(request).await?;
        
        Ok(response.into_inner())
    }

    pub async fn resolve_domains_batch(
        &self,
        queries: Vec<DomainQuery>,
    ) -> Result<Vec<DomainResponse>> {
        let futures: Vec<_> = queries
            .into_iter()
            .map(|query| {
                let client = self.clone();
                async move {
                    client.resolve_domain(query.domain, query.record_types).await
                }
            })
            .collect();

        let results = futures::future::try_join_all(futures).await?;
        Ok(results)
    }

    pub async fn get_account(&self, account_id: impl Into<String>) -> Result<AccountResponse> {
        let request = Request::new(AccountQuery {
            account_id: account_id.into(),
        });

        let mut client = self.chain_pool.get().await?;
        let response = client.get_account(request).await?;
        
        Ok(response.into_inner())
    }

    pub async fn get_balance(&self, account_id: impl Into<String>) -> Result<BalanceResponse> {
        let request = Request::new(BalanceQuery {
            account_id: account_id.into(),
        });

        let mut client = self.chain_pool.get().await?;
        let response = client.get_balance(request).await?;
        
        Ok(response.into_inner())
    }

    pub async fn get_latest_block(&self) -> Result<BlockResponse> {
        let request = Request::new(Empty {});

        let mut client = self.chain_pool.get().await?;
        let response = client.get_latest_block(request).await?;
        
        Ok(response.into_inner())
    }

    pub async fn subscribe_blocks(
        &self,
    ) -> Result<tonic::Streaming<BlockResponse>> {
        let request = Request::new(Empty {});

        let mut client = self.chain_pool.get().await?;
        let response = client.subscribe_blocks(request).await?;
        
        Ok(response.into_inner())
    }

    pub async fn subscribe_domain_changes(
        &self,
        domains: Vec<String>,
        event_types: Vec<String>,
    ) -> Result<tonic::Streaming<DomainEvent>> {
        let request = Request::new(DomainSubscription {
            domains,
            event_types,
        });

        let mut client = self.chain_pool.get().await?;
        let response = client.subscribe_domain_changes(request).await?;
        
        Ok(response.into_inner())
    }

    // DNS service methods
    pub async fn get_dns_stats(&self) -> Result<DnsStats> {
        let request = Request::new(Empty {});

        let mut client = self.dns_pool.get().await?;
        let response = client.get_stats(request).await?;
        
        Ok(response.into_inner())
    }

    pub async fn get_cache_status(&self) -> Result<CacheStats> {
        let request = Request::new(Empty {});

        let mut client = self.dns_pool.get().await?;
        let response = client.get_cache_status(request).await?;
        
        Ok(response.into_inner())
    }

    // QUIC-specific methods
    pub async fn resolve_domain_quic(
        &self,
        domain: impl Into<String>,
        record_types: Vec<String>,
    ) -> Result<DomainResponse> {
        if let Some(quic) = &self.quic_transport {
            quic.resolve_domain(domain.into(), record_types).await
        } else {
            Err(GhostBridgeError::Config("QUIC not enabled".to_string()))
        }
    }
}

impl GhostBridgeClientBuilder {
    pub fn endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.config.endpoint = endpoint.into();
        self
    }

    pub fn enable_quic(mut self, enable: bool) -> Self {
        self.config.enable_quic = enable;
        self
    }

    pub fn pool_size(mut self, size: usize) -> Self {
        self.config.pool_size = size;
        self
    }

    pub fn request_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.config.request_timeout = timeout;
        self
    }

    pub fn enable_compression(mut self, enable: bool) -> Self {
        self.config.enable_compression = enable;
        self
    }

    pub async fn build(self) -> Result<GhostBridgeClient> {
        info!("Building GhostBridge client with endpoint: {}", self.config.endpoint);

        // Create connection pools
        let chain_pool = Arc::new(
            ConnectionPool::new(
                self.config.pool_size,
                self.config.clone(),
                |config| async move {
                    let channel = Channel::from_shared(config.endpoint.clone())
                        .unwrap()
                        .timeout(config.request_timeout)
                        .connect()
                        .await?;
                    Ok(GhostChainServiceClient::new(channel))
                },
            )
            .await?,
        );

        let dns_pool = Arc::new(
            ConnectionPool::new(
                self.config.pool_size,
                self.config.clone(),
                |config| async move {
                    let channel = Channel::from_shared(config.endpoint.clone())
                        .unwrap()
                        .timeout(config.request_timeout)
                        .connect()
                        .await?;
                    Ok(GhostDnsServiceClient::new(channel))
                },
            )
            .await?,
        );

        // Initialize QUIC transport if enabled
        let quic_transport = if self.config.enable_quic {
            match QuicTransport::new(&self.config).await {
                Ok(transport) => Some(Arc::new(transport)),
                Err(e) => {
                    error!("Failed to initialize QUIC transport: {}", e);
                    None
                }
            }
        } else {
            None
        };

        Ok(GhostBridgeClient {
            chain_pool,
            dns_pool,
            quic_transport,
            config: self.config,
        })
    }
}