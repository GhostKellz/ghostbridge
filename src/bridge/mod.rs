/*!
Core GhostBridge implementation

Main bridge orchestration for cross-chain transactions, L2 settlement, and service coordination.
*/

use crate::error::{BridgeError, Result, CrossChainError};
use crate::types::{
    Transaction, TransactionReceipt, BridgeReceipt, BridgeStatus, Network, ChainId,
    TokenAmount, MultiTokenFee, L2Batch, SettlementProof,
};
use crate::services::{ServiceManager, ServiceConfig};
use crate::ffi::{GhostPlaneFfi, GhostPlaneConfig};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, instrument, warn};
use uuid::Uuid;

// Sub-modules
pub mod config;
pub mod validator;
pub mod settlement;

pub use config::BridgeConfig;
pub use validator::TransactionValidator;
pub use settlement::SettlementEngine;

/// Main GhostBridge instance
pub struct GhostBridge {
    config: BridgeConfig,
    services: Arc<ServiceManager>,
    ghostplane_ffi: Arc<RwLock<GhostPlaneFfi>>,
    validator: TransactionValidator,
    settlement_engine: Arc<SettlementEngine>,
    metrics: Arc<BridgeMetrics>,
}

impl GhostBridge {
    /// Create a new GhostBridge instance
    #[instrument(skip(config))]
    pub async fn new(config: BridgeConfig) -> Result<Self> {
        info!("Initializing GhostBridge with config");

        // Initialize service manager
        let service_config = ServiceConfig {
            ghostd: config.service_endpoints.ghostd.clone(),
            walletd: config.service_endpoints.walletd.clone(),
            gid: config.service_endpoints.gid.clone(),
            cns: config.service_endpoints.cns.clone(),
            gledger: config.service_endpoints.gledger.clone(),
            gsig: config.service_endpoints.gsig.clone(),
            ghostplane: config.service_endpoints.ghostplane.clone(),
            default_timeout: config.default_timeout,
            max_retries: config.max_retries,
            enable_guardian_auth: config.enable_guardian_auth,
        };

        let services = Arc::new(ServiceManager::new(service_config));
        services.initialize().await?;

        // Initialize GhostPlane FFI
        let ghostplane_config = GhostPlaneConfig {
            network_id: 10000, // GhostPlane L2
            rpc_endpoint: config.service_endpoints.ghostplane.url(),
            max_batch_size: config.l2_config.max_batch_size,
            settlement_timeout_ms: config.l2_config.settlement_timeout.as_millis() as u64,
            enable_optimistic_execution: config.l2_config.enable_optimistic_execution,
            zk_proof_generation: config.l2_config.enable_zk_proofs,
            memory_limit_mb: 1024,
        };

        let mut ghostplane_ffi = GhostPlaneFfi::new(ghostplane_config);
        ghostplane_ffi.initialize().await?;

        // Initialize other components
        let validator = TransactionValidator::new(config.validation_rules.clone());
        let settlement_engine = Arc::new(SettlementEngine::new(
            config.l2_config.clone(),
            services.clone(),
        ).await?);
        let metrics = Arc::new(BridgeMetrics::new());

        let bridge = Self {
            config,
            services,
            ghostplane_ffi: Arc::new(RwLock::new(ghostplane_ffi)),
            validator,
            settlement_engine,
            metrics,
        };

        info!("GhostBridge initialized successfully");
        Ok(bridge)
    }

    /// Bridge a transaction from L1 to L2
    #[instrument(skip(self, transaction))]
    pub async fn bridge_transaction(&self, transaction: Transaction) -> Result<BridgeReceipt> {
        info!("Processing bridge transaction: {}", transaction.id);

        // Validate transaction
        self.validator.validate(&transaction).await?;
        self.metrics.record_bridge_attempt();

        // Create bridge receipt
        let bridge_id = Uuid::new_v4();
        let mut receipt = BridgeReceipt {
            bridge_id,
            l1_transaction: None,
            l2_transaction: None,
            status: BridgeStatus::Pending,
            bridged_at: chrono::Utc::now(),
            settled_at: None,
        };

        // Process L1 side (if needed)
        if self.requires_l1_processing(&transaction) {
            match self.process_l1_transaction(&transaction).await {
                Ok(l1_receipt) => {
                    receipt.l1_transaction = Some(l1_receipt);
                    receipt.status = BridgeStatus::L1Confirmed;
                }
                Err(e) => {
                    error!("L1 processing failed: {}", e);
                    receipt.status = BridgeStatus::Failed {
                        reason: format!("L1 processing failed: {}", e),
                    };
                    return Ok(receipt);
                }
            }
        }

        // Submit to L2 (GhostPlane)
        match self.submit_to_l2(&transaction).await {
            Ok(l2_receipt) => {
                receipt.l2_transaction = Some(l2_receipt);
                receipt.status = BridgeStatus::L2Confirmed;
                self.metrics.record_bridge_success();
            }
            Err(e) => {
                error!("L2 submission failed: {}", e);
                receipt.status = BridgeStatus::Failed {
                    reason: format!("L2 submission failed: {}", e),
                };
                self.metrics.record_bridge_failure();
                return Ok(receipt);
            }
        }

        info!("Bridge transaction completed: {}", bridge_id);
        Ok(receipt)
    }

    /// Submit a batch of transactions to L2
    #[instrument(skip(self, transactions))]
    pub async fn submit_batch(&self, transactions: Vec<Transaction>) -> Result<L2Batch> {
        info!("Submitting batch of {} transactions to L2", transactions.len());

        // Validate all transactions
        for tx in &transactions {
            self.validator.validate(tx).await?;
        }

        // Submit batch to GhostPlane via FFI
        let ghostplane_ffi = self.ghostplane_ffi.read().await;
        let batch_result = ghostplane_ffi.submit_batch(&transactions).await?;

        // Create L2 batch record
        let batch = L2Batch {
            batch_id: Uuid::new_v4(),
            transactions,
            state_root: batch_result.state_root,
            previous_state_root: [0u8; 32], // TODO: Get previous state root
            block_number: 12345, // TODO: Get actual block number
            timestamp: chrono::Utc::now(),
        };

        // Trigger settlement process
        self.settlement_engine.process_batch(&batch).await?;

        info!("Batch submitted successfully: {}", batch.batch_id);
        Ok(batch)
    }

    /// Query cross-chain state
    #[instrument(skip(self))]
    pub async fn query_cross_chain_state(
        &self,
        network: &Network,
        key: &[u8],
    ) -> Result<Vec<u8>> {
        debug!("Querying cross-chain state for network: {:?}", network);

        match network {
            Network::GhostPlane { .. } => {
                let ghostplane_ffi = self.ghostplane_ffi.read().await;
                ghostplane_ffi.query_state(key).await
            }
            Network::GhostChain { .. } => {
                // Query through GHOSTD service
                // TODO: Implement GHOSTD state query
                Ok(vec![])
            }
            _ => Err(BridgeError::CrossChain(CrossChainError::UnsupportedChain {
                chain_id: match network {
                    Network::Ethereum { chain_id } => chain_id.0,
                    Network::Polygon { chain_id } => chain_id.0,
                    Network::Arbitrum { chain_id } => chain_id.0,
                    Network::Custom { chain_id, .. } => chain_id.0,
                    _ => 0,
                },
            })),
        }
    }

    /// Get bridge status and metrics
    pub async fn get_status(&self) -> Result<BridgeStatus> {
        // TODO: Implement comprehensive status reporting
        Ok(BridgeStatus::Pending)
    }

    /// Health check for all bridge components
    pub async fn health_check(&self) -> Result<BridgeHealthStatus> {
        info!("Performing bridge health check");

        let mut status = BridgeHealthStatus::default();

        // Check services
        match self.services.health_check().await {
            Ok(service_status) => {
                status.services_healthy = service_status.all_healthy;
                status.healthy_services = service_status.healthy_services;
            }
            Err(e) => {
                warn!("Service health check failed: {}", e);
                status.services_healthy = false;
            }
        }

        // Check GhostPlane FFI
        let ghostplane_ffi = self.ghostplane_ffi.read().await;
        status.ffi_healthy = ghostplane_ffi.handle.is_initialized();

        // Check settlement engine
        status.settlement_healthy = self.settlement_engine.is_healthy().await;

        status.overall_healthy = status.services_healthy && status.ffi_healthy && status.settlement_healthy;

        info!("Bridge health check completed: healthy = {}", status.overall_healthy);
        Ok(status)
    }

    // Private helper methods

    /// Check if transaction requires L1 processing
    fn requires_l1_processing(&self, transaction: &Transaction) -> bool {
        matches!(
            (&transaction.from_chain, &transaction.to_chain),
            (Network::Ethereum { .. }, Network::GhostPlane { .. }) |
            (Network::Polygon { .. }, Network::GhostPlane { .. }) |
            (Network::Arbitrum { .. }, Network::GhostPlane { .. })
        )
    }

    /// Process L1 side of transaction
    async fn process_l1_transaction(&self, transaction: &Transaction) -> Result<TransactionReceipt> {
        debug!("Processing L1 transaction");
        // TODO: Implement actual L1 processing via external chain clients
        Ok(TransactionReceipt {
            transaction_hash: transaction.hash(),
            block_number: 12345,
            block_hash: [0u8; 32],
            transaction_index: 0,
            gas_used: 21000,
            success: true,
            logs: vec![],
        })
    }

    /// Submit transaction to L2
    async fn submit_to_l2(&self, transaction: &Transaction) -> Result<TransactionReceipt> {
        debug!("Submitting transaction to L2");
        let ghostplane_ffi = self.ghostplane_ffi.read().await;
        ghostplane_ffi.submit_transaction(transaction).await
    }
}

/// Bridge health status
#[derive(Debug, Clone, Default)]
pub struct BridgeHealthStatus {
    pub overall_healthy: bool,
    pub services_healthy: bool,
    pub ffi_healthy: bool,
    pub settlement_healthy: bool,
    pub healthy_services: usize,
}

/// Bridge metrics collection
pub struct BridgeMetrics {
    bridge_attempts: parking_lot::Mutex<u64>,
    bridge_successes: parking_lot::Mutex<u64>,
    bridge_failures: parking_lot::Mutex<u64>,
}

impl BridgeMetrics {
    pub fn new() -> Self {
        Self {
            bridge_attempts: parking_lot::Mutex::new(0),
            bridge_successes: parking_lot::Mutex::new(0),
            bridge_failures: parking_lot::Mutex::new(0),
        }
    }

    pub fn record_bridge_attempt(&self) {
        *self.bridge_attempts.lock() += 1;
    }

    pub fn record_bridge_success(&self) {
        *self.bridge_successes.lock() += 1;
    }

    pub fn record_bridge_failure(&self) {
        *self.bridge_failures.lock() += 1;
    }

    pub fn get_stats(&self) -> BridgeStats {
        BridgeStats {
            total_attempts: *self.bridge_attempts.lock(),
            successful_bridges: *self.bridge_successes.lock(),
            failed_bridges: *self.bridge_failures.lock(),
        }
    }
}

/// Bridge statistics
#[derive(Debug, Clone)]
pub struct BridgeStats {
    pub total_attempts: u64,
    pub successful_bridges: u64,
    pub failed_bridges: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bridge_metrics() {
        let metrics = BridgeMetrics::new();

        metrics.record_bridge_attempt();
        metrics.record_bridge_success();

        let stats = metrics.get_stats();
        assert_eq!(stats.total_attempts, 1);
        assert_eq!(stats.successful_bridges, 1);
        assert_eq!(stats.failed_bridges, 0);
    }

    #[test]
    fn test_bridge_health_status() {
        let status = BridgeHealthStatus {
            overall_healthy: true,
            services_healthy: true,
            ffi_healthy: true,
            settlement_healthy: true,
            healthy_services: 6,
        };

        assert!(status.overall_healthy);
        assert_eq!(status.healthy_services, 6);
    }
}