use std::sync::Arc;
use tokio::sync::RwLock;
use tonic::{transport::Channel, Request, Response, Status};
use tracing::{debug, error, info};

// GhostLink v0.3.0 imports for unified client architecture
use ghostlink::{GhostClient, GhostClientConfig, TransportProtocol};

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
// Legacy QUIC transport disabled - using GhostLink's unified transport
// use crate::quic_transport::QuicTransport;

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
    // GhostLink v0.3.0 unified client for GhostChain communication
    ghostlink_client: Arc<GhostClient>,
    // Legacy connection pools for backward compatibility with existing gRPC API
    chain_pool: Arc<ConnectionPool<GhostChainServiceClient<Channel>>>,
    dns_pool: Arc<ConnectionPool<GhostDnsServiceClient<Channel>>>,
    // Legacy QUIC transport disabled - using GhostLink's unified transport
    // quic_transport: Option<Arc<QuicTransport>>,
    config: ClientConfig,
}

#[derive(Clone)]
pub struct ClientConfig {
    pub endpoint: String,
    pub enable_quic: bool,
    pub pool_size: usize,
    pub request_timeout: std::time::Duration,
    pub enable_compression: bool,
    // GhostLink v0.3.0 transport protocol selection
    pub transport_protocol: TransportProtocol,
    pub enable_tls: bool,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            endpoint: "https://127.0.0.1:9090".to_string(),
            enable_quic: true,
            pool_size: 4,
            request_timeout: std::time::Duration::from_secs(10),
            enable_compression: true,
            // GhostLink v0.3.0 defaults: QUIC with TLS
            transport_protocol: TransportProtocol::Quic,
            enable_tls: true,
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

    /// Access the underlying GhostLink client for direct GhostChain operations
    pub fn ghostlink(&self) -> &GhostClient {
        &self.ghostlink_client
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

    // QUIC-specific methods now handled by GhostLink's unified transport
    pub async fn resolve_domain_quic(
        &self,
        domain: impl Into<String>,
        record_types: Vec<String>,
    ) -> Result<DomainResponse> {
        // Use GhostLink's unified transport (which includes QUIC)
        self.resolve_domain(domain, record_types).await
    }
}

impl GhostBridgeClientBuilder {
    pub fn endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.config.endpoint = endpoint.into();
        self
    }

    pub fn enable_quic(mut self, enable: bool) -> Self {
        self.config.enable_quic = enable;
        // Update transport protocol based on QUIC preference
        if enable {
            self.config.transport_protocol = TransportProtocol::Quic;
        } else {
            self.config.transport_protocol = TransportProtocol::Http2Grpc;
        }
        self
    }

    pub fn transport_protocol(mut self, protocol: TransportProtocol) -> Self {
        self.config.transport_protocol = protocol;
        self
    }

    pub fn with_tls(mut self, enable: bool) -> Self {
        self.config.enable_tls = enable;
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

        // Create GhostLink v0.3.0 client with unified transport
        let config_clone = self.config.clone();
        let mut ghostlink_config = GhostClientConfig::builder()
            .endpoint(config_clone.endpoint.clone())
            .transport_protocol(config_clone.transport_protocol);
        
        if config_clone.enable_tls {
            ghostlink_config = ghostlink_config.with_tls();
        }
        
        let ghostlink_config = ghostlink_config.build();

        let ghostlink_client = Arc::new(
            GhostClient::connect(ghostlink_config)
                .await
                .map_err(|e| GhostBridgeError::Config(format!("GhostLink connection failed: {}", e)))?
        );

        // Create connection pools for legacy gRPC API compatibility
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

        // Legacy QUIC transport disabled - using GhostLink's unified transport
        let _quic_transport: Option<()> = None;

        Ok(GhostBridgeClient {
            ghostlink_client,
            chain_pool,
            dns_pool,
            // Legacy QUIC transport disabled - using GhostLink's unified transport
            // quic_transport,
            config: self.config,
        })
    }
}