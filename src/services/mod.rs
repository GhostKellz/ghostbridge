/*!
GhostChain service integration via etherlink

Integration with all 6 core GhostChain services using etherlink's gRPC/QUIC clients:
- GHOSTD (Blockchain daemon)
- WALLETD (Wallet management)
- GID (Identity system)
- CNS (Crypto Name Server)
- GLEDGER (Ledger and token management)
- GSIG (Signature and verification)
*/

use crate::error::{BridgeError, Result, ServiceError};
use crate::types::{Address, TokenAmount, TokenType, Network, ChainId};
use etherlink::{CNSClient, GhostPlaneClient, EtherlinkError};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, error, info, instrument, warn};

// Re-export service modules
pub mod ghostd;
pub mod walletd;
pub mod gid;
pub mod cns;
pub mod gledger;
pub mod gsig;

pub use self::{
    ghostd::GhostdService,
    walletd::WalletdService,
    gid::GidService,
    cns::CnsService,
    gledger::GledgerService,
    gsig::GsigService,
};

/// Service configuration for all GhostChain services
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ServiceConfig {
    pub ghostd: ServiceEndpoint,
    pub walletd: ServiceEndpoint,
    pub gid: ServiceEndpoint,
    pub cns: ServiceEndpoint,
    pub gledger: ServiceEndpoint,
    pub gsig: ServiceEndpoint,
    pub ghostplane: ServiceEndpoint,
    pub default_timeout: Duration,
    pub max_retries: u32,
    pub enable_guardian_auth: bool,
}

/// Individual service endpoint configuration
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ServiceEndpoint {
    pub host: String,
    pub port: u16,
    pub use_tls: bool,
    pub timeout_ms: u64,
}

impl ServiceEndpoint {
    /// Get the full endpoint URL
    pub fn url(&self) -> String {
        let scheme = if self.use_tls { "https" } else { "http" };
        format!("{}://{}:{}", scheme, self.host, self.port)
    }

    /// Create gRPC endpoint
    pub fn grpc_endpoint(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

impl Default for ServiceConfig {
    fn default() -> Self {
        Self {
            ghostd: ServiceEndpoint {
                host: "localhost".to_string(),
                port: 8545,
                use_tls: false,
                timeout_ms: 5000,
            },
            walletd: ServiceEndpoint {
                host: "localhost".to_string(),
                port: 8546,
                use_tls: false,
                timeout_ms: 5000,
            },
            gid: ServiceEndpoint {
                host: "localhost".to_string(),
                port: 8547,
                use_tls: false,
                timeout_ms: 5000,
            },
            cns: ServiceEndpoint {
                host: "localhost".to_string(),
                port: 8548,
                use_tls: false,
                timeout_ms: 5000,
            },
            gledger: ServiceEndpoint {
                host: "localhost".to_string(),
                port: 8549,
                use_tls: false,
                timeout_ms: 5000,
            },
            gsig: ServiceEndpoint {
                host: "localhost".to_string(),
                port: 8550,
                use_tls: false,
                timeout_ms: 5000,
            },
            ghostplane: ServiceEndpoint {
                host: "localhost".to_string(),
                port: 9090,
                use_tls: false,
                timeout_ms: 10000, // L2 operations may take longer
            },
            default_timeout: Duration::from_secs(5),
            max_retries: 3,
            enable_guardian_auth: true,
        }
    }
}

/// Unified service manager for all GhostChain services
pub struct ServiceManager {
    config: ServiceConfig,
    ghostd: Arc<RwLock<Option<GhostdService>>>,
    walletd: Arc<RwLock<Option<WalletdService>>>,
    gid: Arc<RwLock<Option<GidService>>>,
    cns: Arc<RwLock<Option<CnsService>>>,
    gledger: Arc<RwLock<Option<GledgerService>>>,
    gsig: Arc<RwLock<Option<GsigService>>>,
    ghostplane_client: Arc<RwLock<Option<GhostPlaneClient>>>,
}

impl ServiceManager {
    /// Create a new service manager
    pub fn new(config: ServiceConfig) -> Self {
        Self {
            config,
            ghostd: Arc::new(RwLock::new(None)),
            walletd: Arc::new(RwLock::new(None)),
            gid: Arc::new(RwLock::new(None)),
            cns: Arc::new(RwLock::new(None)),
            gledger: Arc::new(RwLock::new(None)),
            gsig: Arc::new(RwLock::new(None)),
            ghostplane_client: Arc::new(RwLock::new(None)),
        }
    }

    /// Initialize all services
    #[instrument(skip(self))]
    pub async fn initialize(&self) -> Result<()> {
        info!("Initializing GhostChain service connections");

        // Initialize etherlink first
        etherlink::init();

        // Initialize all services in parallel
        let init_tasks = vec![
            self.init_ghostd(),
            self.init_walletd(),
            self.init_gid(),
            self.init_cns(),
            self.init_gledger(),
            self.init_gsig(),
            self.init_ghostplane(),
        ];

        let results = futures::future::join_all(init_tasks).await;

        // Check for any failures
        for (i, result) in results.into_iter().enumerate() {
            if let Err(e) = result {
                let service_name = match i {
                    0 => "GHOSTD",
                    1 => "WALLETD",
                    2 => "GID",
                    3 => "CNS",
                    4 => "GLEDGER",
                    5 => "GSIG",
                    6 => "GHOSTPLANE",
                    _ => "UNKNOWN",
                };
                error!("Failed to initialize {}: {}", service_name, e);
                return Err(e);
            }
        }

        info!("All GhostChain services initialized successfully");
        Ok(())
    }

    /// Get GHOSTD service
    pub async fn ghostd(&self) -> Result<tokio::sync::RwLockReadGuard<'_, Option<GhostdService>>> {
        let guard = self.ghostd.read().await;
        if guard.is_none() {
            return Err(BridgeError::Service(ServiceError::ServiceUnavailable {
                service: "GHOSTD".to_string(),
            }));
        }
        Ok(guard)
    }

    /// Get WALLETD service
    pub async fn walletd(&self) -> Result<tokio::sync::RwLockReadGuard<'_, Option<WalletdService>>> {
        let guard = self.walletd.read().await;
        if guard.is_none() {
            return Err(BridgeError::Service(ServiceError::ServiceUnavailable {
                service: "WALLETD".to_string(),
            }));
        }
        Ok(guard)
    }

    /// Get GID service
    pub async fn gid(&self) -> Result<tokio::sync::RwLockReadGuard<'_, Option<GidService>>> {
        let guard = self.gid.read().await;
        if guard.is_none() {
            return Err(BridgeError::Service(ServiceError::ServiceUnavailable {
                service: "GID".to_string(),
            }));
        }
        Ok(guard)
    }

    /// Get CNS service
    pub async fn cns(&self) -> Result<tokio::sync::RwLockReadGuard<'_, Option<CnsService>>> {
        let guard = self.cns.read().await;
        if guard.is_none() {
            return Err(BridgeError::Service(ServiceError::ServiceUnavailable {
                service: "CNS".to_string(),
            }));
        }
        Ok(guard)
    }

    /// Get GLEDGER service
    pub async fn gledger(&self) -> Result<tokio::sync::RwLockReadGuard<'_, Option<GledgerService>>> {
        let guard = self.gledger.read().await;
        if guard.is_none() {
            return Err(BridgeError::Service(ServiceError::ServiceUnavailable {
                service: "GLEDGER".to_string(),
            }));
        }
        Ok(guard)
    }

    /// Get GSIG service
    pub async fn gsig(&self) -> Result<tokio::sync::RwLockReadGuard<'_, Option<GsigService>>> {
        let guard = self.gsig.read().await;
        if guard.is_none() {
            return Err(BridgeError::Service(ServiceError::ServiceUnavailable {
                service: "GSIG".to_string(),
            }));
        }
        Ok(guard)
    }

    /// Get GhostPlane client
    pub async fn ghostplane(&self) -> Result<tokio::sync::RwLockReadGuard<'_, Option<GhostPlaneClient>>> {
        let guard = self.ghostplane_client.read().await;
        if guard.is_none() {
            return Err(BridgeError::Service(ServiceError::ServiceUnavailable {
                service: "GHOSTPLANE".to_string(),
            }));
        }
        Ok(guard)
    }

    /// Health check for all services
    pub async fn health_check(&self) -> Result<ServiceHealthStatus> {
        let mut status = ServiceHealthStatus::default();

        // Check each service
        let checks = vec![
            ("GHOSTD", self.check_ghostd_health()),
            ("WALLETD", self.check_walletd_health()),
            ("GID", self.check_gid_health()),
            ("CNS", self.check_cns_health()),
            ("GLEDGER", self.check_gledger_health()),
            ("GSIG", self.check_gsig_health()),
            ("GHOSTPLANE", self.check_ghostplane_health()),
        ];

        for (service_name, health_check) in checks {
            let is_healthy = health_check.await.is_ok();
            status.services.insert(service_name.to_string(), is_healthy);

            if is_healthy {
                status.healthy_services += 1;
            }
        }

        status.all_healthy = status.healthy_services == status.services.len();
        Ok(status)
    }

    // Private initialization methods
    async fn init_ghostd(&self) -> Result<()> {
        debug!("Initializing GHOSTD service");
        let service = GhostdService::new(&self.config.ghostd).await?;
        *self.ghostd.write().await = Some(service);
        Ok(())
    }

    async fn init_walletd(&self) -> Result<()> {
        debug!("Initializing WALLETD service");
        let service = WalletdService::new(&self.config.walletd).await?;
        *self.walletd.write().await = Some(service);
        Ok(())
    }

    async fn init_gid(&self) -> Result<()> {
        debug!("Initializing GID service");
        let service = GidService::new(&self.config.gid).await?;
        *self.gid.write().await = Some(service);
        Ok(())
    }

    async fn init_cns(&self) -> Result<()> {
        debug!("Initializing CNS service");
        let service = CnsService::new(&self.config.cns).await?;
        *self.cns.write().await = Some(service);
        Ok(())
    }

    async fn init_gledger(&self) -> Result<()> {
        debug!("Initializing GLEDGER service");
        let service = GledgerService::new(&self.config.gledger).await?;
        *self.gledger.write().await = Some(service);
        Ok(())
    }

    async fn init_gsig(&self) -> Result<()> {
        debug!("Initializing GSIG service");
        let service = GsigService::new(&self.config.gsig).await?;
        *self.gsig.write().await = Some(service);
        Ok(())
    }

    async fn init_ghostplane(&self) -> Result<()> {
        debug!("Initializing GhostPlane client");
        let client = GhostPlaneClient::connect(self.config.ghostplane.grpc_endpoint()).await
            .map_err(|e| BridgeError::Service(ServiceError::EtherlinkClient(e.to_string())))?;
        *self.ghostplane_client.write().await = Some(client);
        Ok(())
    }

    // Health check methods
    async fn check_ghostd_health(&self) -> Result<()> {
        if let Ok(service_guard) = self.ghostd().await {
            if let Some(service) = service_guard.as_ref() {
                return service.health_check().await;
            }
        }
        Err(BridgeError::Service(ServiceError::ServiceUnavailable {
            service: "GHOSTD".to_string(),
        }))
    }

    async fn check_walletd_health(&self) -> Result<()> {
        if let Ok(service_guard) = self.walletd().await {
            if let Some(service) = service_guard.as_ref() {
                return service.health_check().await;
            }
        }
        Err(BridgeError::Service(ServiceError::ServiceUnavailable {
            service: "WALLETD".to_string(),
        }))
    }

    async fn check_gid_health(&self) -> Result<()> {
        if let Ok(service_guard) = self.gid().await {
            if let Some(service) = service_guard.as_ref() {
                return service.health_check().await;
            }
        }
        Err(BridgeError::Service(ServiceError::ServiceUnavailable {
            service: "GID".to_string(),
        }))
    }

    async fn check_cns_health(&self) -> Result<()> {
        if let Ok(service_guard) = self.cns().await {
            if let Some(service) = service_guard.as_ref() {
                return service.health_check().await;
            }
        }
        Err(BridgeError::Service(ServiceError::ServiceUnavailable {
            service: "CNS".to_string(),
        }))
    }

    async fn check_gledger_health(&self) -> Result<()> {
        if let Ok(service_guard) = self.gledger().await {
            if let Some(service) = service_guard.as_ref() {
                return service.health_check().await;
            }
        }
        Err(BridgeError::Service(ServiceError::ServiceUnavailable {
            service: "GLEDGER".to_string(),
        }))
    }

    async fn check_gsig_health(&self) -> Result<()> {
        if let Ok(service_guard) = self.gsig().await {
            if let Some(service) = service_guard.as_ref() {
                return service.health_check().await;
            }
        }
        Err(BridgeError::Service(ServiceError::ServiceUnavailable {
            service: "GSIG".to_string(),
        }))
    }

    async fn check_ghostplane_health(&self) -> Result<()> {
        if let Ok(client_guard) = self.ghostplane().await {
            if let Some(_client) = client_guard.as_ref() {
                // TODO: Implement health check for GhostPlane
                return Ok(());
            }
        }
        Err(BridgeError::Service(ServiceError::ServiceUnavailable {
            service: "GHOSTPLANE".to_string(),
        }))
    }
}

/// Service health status
#[derive(Debug, Clone, Default)]
pub struct ServiceHealthStatus {
    pub all_healthy: bool,
    pub healthy_services: usize,
    pub services: std::collections::HashMap<String, bool>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_config_default() {
        let config = ServiceConfig::default();
        assert_eq!(config.ghostd.port, 8545);
        assert_eq!(config.walletd.port, 8546);
        assert_eq!(config.gid.port, 8547);
        assert_eq!(config.cns.port, 8548);
        assert_eq!(config.gledger.port, 8549);
        assert_eq!(config.gsig.port, 8550);
        assert_eq!(config.ghostplane.port, 9090);
    }

    #[test]
    fn test_service_endpoint_url() {
        let endpoint = ServiceEndpoint {
            host: "localhost".to_string(),
            port: 8545,
            use_tls: false,
            timeout_ms: 5000,
        };
        assert_eq!(endpoint.url(), "http://localhost:8545");

        let tls_endpoint = ServiceEndpoint {
            host: "example.com".to_string(),
            port: 443,
            use_tls: true,
            timeout_ms: 5000,
        };
        assert_eq!(tls_endpoint.url(), "https://example.com:443");
    }

    #[test]
    fn test_service_manager_creation() {
        let config = ServiceConfig::default();
        let manager = ServiceManager::new(config);
        // Should not panic
        assert!(true);
    }
}