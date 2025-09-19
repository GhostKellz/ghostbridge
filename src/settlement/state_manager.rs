/*!
State management for L2 settlement

Handles state transitions, snapshots, and rollback capabilities for the
high-performance settlement engine.
*/

use crate::error::{BridgeError, Result};
use crate::types::{Address, U256, Transaction};
use crate::settlement::SettlementConfig;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;
use tracing::{debug, error, info, instrument, warn};
use serde::{Deserialize, Serialize};

/// State manager for L2 settlement
pub struct StateManager {
    config: SettlementConfig,
    current_state: Arc<RwLock<L2State>>,
    state_history: Arc<RwLock<StateHistory>>,
    snapshot_manager: SnapshotManager,
    merkle_tree: MerklePatriciaTree,
    state_cache: Arc<RwLock<StateCache>>,
    sync_manager: StateSyncManager,
}

/// L2 state representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct L2State {
    pub state_root: Vec<u8>,
    pub block_number: u64,
    pub timestamp: SystemTime,
    pub accounts: HashMap<Address, AccountState>,
    pub storage: HashMap<(Address, U256), U256>,
    pub balances: HashMap<(Address, String), U256>, // (address, token_type) -> balance
    pub nonces: HashMap<Address, u64>,
    pub total_supply: HashMap<String, U256>, // token_type -> total_supply
    pub transaction_count: u64,
    pub gas_used: u64,
}

/// Account state in L2
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountState {
    pub nonce: u64,
    pub balances: HashMap<String, U256>, // token_type -> balance
    pub storage_root: Vec<u8>,
    pub code_hash: Vec<u8>,
    pub code: Option<Vec<u8>>,
    pub created_at: SystemTime,
    pub last_updated: SystemTime,
}

/// State update representing a change
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateUpdate {
    pub update_id: String,
    pub block_number: u64,
    pub transaction_id: String,
    pub updates: Vec<StateChange>,
    pub gas_used: u64,
    pub timestamp: SystemTime,
    pub state_root_before: Vec<u8>,
    pub state_root_after: Vec<u8>,
}

/// Individual state change
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateChange {
    pub change_type: StateChangeType,
    pub address: Address,
    pub key: Option<U256>,
    pub old_value: StateValue,
    pub new_value: StateValue,
    pub proof: Option<Vec<u8>>,
}

/// Types of state changes
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum StateChangeType {
    AccountCreation,
    AccountDeletion,
    BalanceUpdate,
    NonceUpdate,
    StorageUpdate,
    CodeUpdate,
    SupplyUpdate,
}

/// State value types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StateValue {
    None,
    U256(U256),
    Bytes(Vec<u8>),
    Address(Address),
    String(String),
    Account(AccountState),
}

/// State history tracking
#[derive(Debug, Clone)]
struct StateHistory {
    updates: VecDeque<StateUpdate>,
    snapshots: HashMap<u64, StateSnapshot>, // block_number -> snapshot
    rollback_points: Vec<RollbackPoint>,
    max_history_size: usize,
    compression_enabled: bool,
}

/// State snapshot for fast recovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateSnapshot {
    pub snapshot_id: String,
    pub block_number: u64,
    pub state_root: Vec<u8>,
    pub compressed_state: Vec<u8>,
    pub created_at: SystemTime,
    pub size_bytes: usize,
    pub compression_ratio: f64,
}

/// Rollback point for state recovery
#[derive(Debug, Clone)]
struct RollbackPoint {
    point_id: String,
    block_number: u64,
    state_snapshot: StateSnapshot,
    reason: RollbackReason,
    created_at: SystemTime,
}

/// Reasons for rollback
#[derive(Debug, Clone)]
enum RollbackReason {
    FraudProof,
    ChainReorg,
    ManualRollback,
    SystemError,
}

/// Snapshot management
struct SnapshotManager {
    snapshot_interval: Duration,
    max_snapshots: usize,
    compression_algorithm: CompressionAlgorithm,
    async_snapshots: bool,
}

/// Compression algorithms
#[derive(Debug, Clone)]
enum CompressionAlgorithm {
    None,
    Gzip,
    Lz4,
    Zstd,
}

/// Merkle Patricia Tree for state proofs
struct MerklePatriciaTree {
    root_hash: Vec<u8>,
    nodes: HashMap<Vec<u8>, TreeNode>,
    cache: Arc<RwLock<HashMap<Vec<u8>, TreeNode>>>,
}

/// Tree node in Merkle Patricia Tree
#[derive(Debug, Clone)]
struct TreeNode {
    node_type: NodeType,
    key: Vec<u8>,
    value: Vec<u8>,
    children: HashMap<u8, Vec<u8>>, // nibble -> child_hash
    hash: Vec<u8>,
}

/// Types of tree nodes
#[derive(Debug, Clone)]
enum NodeType {
    Leaf,
    Extension,
    Branch,
}

/// State cache for performance
#[derive(Debug, Clone)]
struct StateCache {
    cached_accounts: HashMap<Address, CachedAccount>,
    cached_storage: HashMap<(Address, U256), CachedStorage>,
    cache_statistics: CacheStatistics,
    eviction_policy: EvictionPolicy,
}

/// Cached account data
#[derive(Debug, Clone)]
struct CachedAccount {
    account: AccountState,
    cached_at: SystemTime,
    access_count: u64,
    dirty: bool,
}

/// Cached storage data
#[derive(Debug, Clone)]
struct CachedStorage {
    value: U256,
    cached_at: SystemTime,
    access_count: u64,
    dirty: bool,
}

/// Cache statistics
#[derive(Debug, Clone)]
struct CacheStatistics {
    total_size: usize,
    hit_rate: f64,
    miss_rate: f64,
    eviction_count: u64,
    dirty_entries: usize,
}

/// Cache eviction policies
#[derive(Debug, Clone)]
enum EvictionPolicy {
    LRU,
    LFU,
    FIFO,
    Random,
}

/// State synchronization manager
struct StateSyncManager {
    sync_mode: SyncMode,
    peer_states: HashMap<String, PeerState>,
    sync_progress: SyncProgress,
    conflict_resolution: ConflictResolution,
}

/// Synchronization modes
#[derive(Debug, Clone)]
enum SyncMode {
    Full,      // Full state sync
    Fast,      // State snapshots only
    Light,     // Headers and proofs only
    Archive,   // Full history
}

/// Peer state information
#[derive(Debug, Clone)]
struct PeerState {
    peer_id: String,
    state_root: Vec<u8>,
    block_number: u64,
    last_update: SystemTime,
    trust_score: f64,
}

/// Sync progress tracking
#[derive(Debug, Clone)]
struct SyncProgress {
    current_block: u64,
    target_block: u64,
    sync_percentage: f64,
    estimated_completion: SystemTime,
    sync_speed: f64, // blocks per second
}

/// Conflict resolution strategies
#[derive(Debug, Clone)]
enum ConflictResolution {
    LastWriteWins,
    Consensus,
    Manual,
    Rollback,
}

impl StateManager {
    /// Initialize state manager
    #[instrument(skip(config))]
    pub async fn new(config: SettlementConfig) -> Result<Self> {
        info!("Initializing L2 state manager");

        let current_state = Arc::new(RwLock::new(L2State {
            state_root: vec![0; 32], // Genesis state
            block_number: 0,
            timestamp: SystemTime::now(),
            accounts: HashMap::new(),
            storage: HashMap::new(),
            balances: HashMap::new(),
            nonces: HashMap::new(),
            total_supply: HashMap::new(),
            transaction_count: 0,
            gas_used: 0,
        }));

        let state_history = Arc::new(RwLock::new(StateHistory {
            updates: VecDeque::new(),
            snapshots: HashMap::new(),
            rollback_points: Vec::new(),
            max_history_size: 10000,
            compression_enabled: true,
        }));

        let snapshot_manager = SnapshotManager {
            snapshot_interval: config.checkpoint_interval,
            max_snapshots: 100,
            compression_algorithm: CompressionAlgorithm::Zstd,
            async_snapshots: true,
        };

        let merkle_tree = MerklePatriciaTree {
            root_hash: vec![0; 32],
            nodes: HashMap::new(),
            cache: Arc::new(RwLock::new(HashMap::new())),
        };

        let state_cache = Arc::new(RwLock::new(StateCache {
            cached_accounts: HashMap::new(),
            cached_storage: HashMap::new(),
            cache_statistics: CacheStatistics {
                total_size: 0,
                hit_rate: 0.0,
                miss_rate: 0.0,
                eviction_count: 0,
                dirty_entries: 0,
            },
            eviction_policy: EvictionPolicy::LRU,
        }));

        let sync_manager = StateSyncManager {
            sync_mode: SyncMode::Fast,
            peer_states: HashMap::new(),
            sync_progress: SyncProgress {
                current_block: 0,
                target_block: 0,
                sync_percentage: 100.0,
                estimated_completion: SystemTime::now(),
                sync_speed: 0.0,
            },
            conflict_resolution: ConflictResolution::Consensus,
        };

        Ok(Self {
            config,
            current_state,
            state_history,
            snapshot_manager,
            merkle_tree,
            state_cache,
            sync_manager,
        })
    }

    /// Apply state update
    #[instrument(skip(self, update))]
    pub async fn apply_state_update(&self, update: StateUpdate) -> Result<()> {
        debug!("Applying state update: {} with {} changes",
               update.update_id, update.updates.len());

        // Get current state
        let mut state = self.current_state.write().await;

        // Verify state root matches
        if state.state_root != update.state_root_before {
            return Err(BridgeError::Settlement(format!(
                "State root mismatch: expected {:?}, got {:?}",
                hex::encode(&state.state_root),
                hex::encode(&update.state_root_before)
            )));
        }

        // Apply each change
        for change in &update.updates {
            self.apply_single_change(&mut state, change).await?;
        }

        // Update state metadata
        state.block_number = update.block_number;
        state.timestamp = update.timestamp;
        state.transaction_count += 1;
        state.gas_used += update.gas_used;
        state.state_root = update.state_root_after.clone();

        drop(state);

        // Add to history
        {
            let mut history = self.state_history.write().await;
            history.updates.push_back(update.clone());

            // Maintain history size
            if history.updates.len() > history.max_history_size {
                history.updates.pop_front();
            }
        }

        // Update cache
        self.invalidate_cache_for_update(&update).await;

        debug!("State update applied successfully: {}", update.update_id);
        Ok(())
    }

    /// Get current state root
    pub async fn get_state_root(&self) -> Vec<u8> {
        self.current_state.read().await.state_root.clone()
    }

    /// Get account state
    #[instrument(skip(self))]
    pub async fn get_account(&self, address: &Address) -> Result<Option<AccountState>> {
        // Check cache first
        {
            let cache = self.state_cache.read().await;
            if let Some(cached) = cache.cached_accounts.get(address) {
                return Ok(Some(cached.account.clone()));
            }
        }

        // Get from state
        let state = self.current_state.read().await;
        if let Some(account) = state.accounts.get(address) {
            // Cache the result
            self.cache_account(address, account).await;
            Ok(Some(account.clone()))
        } else {
            Ok(None)
        }
    }

    /// Get account balance for specific token
    #[instrument(skip(self))]
    pub async fn get_balance(&self, address: &Address, token_type: &str) -> Result<U256> {
        let state = self.current_state.read().await;
        let balance_key = (address.clone(), token_type.to_string());
        Ok(state.balances.get(&balance_key).unwrap_or(&U256::ZERO).clone())
    }

    /// Get account nonce
    #[instrument(skip(self))]
    pub async fn get_nonce(&self, address: &Address) -> Result<u64> {
        let state = self.current_state.read().await;
        Ok(state.nonces.get(address).unwrap_or(&0).clone())
    }

    /// Create state snapshot
    #[instrument(skip(self))]
    pub async fn create_snapshot(&self) -> Result<StateSnapshot> {
        debug!("Creating state snapshot");

        let state = self.current_state.read().await;
        let snapshot_id = format!("snapshot-{}-{}",
                                 state.block_number,
                                 SystemTime::now().duration_since(std::time::UNIX_EPOCH)
                                     .unwrap_or_default().as_millis());

        // Serialize state
        let serialized_state = serde_json::to_vec(&*state)?;

        // Compress if enabled
        let compressed_state = if self.state_history.read().await.compression_enabled {
            self.compress_data(&serialized_state).await?
        } else {
            serialized_state.clone()
        };

        let compression_ratio = compressed_state.len() as f64 / serialized_state.len() as f64;

        let snapshot = StateSnapshot {
            snapshot_id: snapshot_id.clone(),
            block_number: state.block_number,
            state_root: state.state_root.clone(),
            compressed_state,
            created_at: SystemTime::now(),
            size_bytes: compressed_state.len(),
            compression_ratio,
        };

        // Store snapshot
        {
            let mut history = self.state_history.write().await;
            history.snapshots.insert(state.block_number, snapshot.clone());

            // Cleanup old snapshots
            if history.snapshots.len() > self.snapshot_manager.max_snapshots {
                let oldest_block = history.snapshots.keys().min().copied();
                if let Some(block) = oldest_block {
                    history.snapshots.remove(&block);
                }
            }
        }

        info!("Created state snapshot: {} at block {} (compressed: {:.2}%)",
              snapshot_id, state.block_number, compression_ratio * 100.0);

        Ok(snapshot)
    }

    /// Restore from snapshot
    #[instrument(skip(self, snapshot))]
    pub async fn restore_from_snapshot(&self, snapshot: &StateSnapshot) -> Result<()> {
        warn!("Restoring state from snapshot: {} at block {}",
              snapshot.snapshot_id, snapshot.block_number);

        // Decompress state
        let serialized_state = if self.state_history.read().await.compression_enabled {
            self.decompress_data(&snapshot.compressed_state).await?
        } else {
            snapshot.compressed_state.clone()
        };

        // Deserialize state
        let restored_state: L2State = serde_json::from_slice(&serialized_state)?;

        // Replace current state
        {
            let mut current_state = self.current_state.write().await;
            *current_state = restored_state;
        }

        // Clear cache
        self.clear_cache().await;

        info!("State restored from snapshot: {}", snapshot.snapshot_id);
        Ok(())
    }

    /// Rollback to specific block
    #[instrument(skip(self))]
    pub async fn rollback_to_block(&self, target_block: u64, reason: RollbackReason) -> Result<()> {
        warn!("Rolling back state to block: {} (reason: {:?})", target_block, reason);

        let current_block = self.current_state.read().await.block_number;

        if target_block >= current_block {
            return Err(BridgeError::Settlement(
                "Cannot rollback to future block".to_string()
            ));
        }

        // Find appropriate snapshot
        let snapshot = {
            let history = self.state_history.read().await;

            // Find the latest snapshot <= target_block
            let mut best_snapshot = None;
            for (block_num, snapshot) in &history.snapshots {
                if *block_num <= target_block {
                    if best_snapshot.is_none() || *block_num > best_snapshot.unwrap().0 {
                        best_snapshot = Some((*block_num, snapshot.clone()));
                    }
                }
            }

            best_snapshot.map(|(_, snapshot)| snapshot)
        };

        if let Some(snapshot) = snapshot {
            // Restore from snapshot
            self.restore_from_snapshot(&snapshot).await?;

            // Create rollback point
            let rollback_point = RollbackPoint {
                point_id: format!("rollback-{}", SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default().as_millis()),
                block_number: target_block,
                state_snapshot: snapshot,
                reason,
                created_at: SystemTime::now(),
            };

            {
                let mut history = self.state_history.write().await;
                history.rollback_points.push(rollback_point);
            }

            info!("Rollback completed to block: {}", target_block);
        } else {
            return Err(BridgeError::Settlement(
                "No snapshot found for rollback target".to_string()
            ));
        }

        Ok(())
    }

    /// Generate merkle proof for state
    #[instrument(skip(self))]
    pub async fn generate_merkle_proof(&self, address: &Address, key: Option<U256>) -> Result<Vec<u8>> {
        debug!("Generating merkle proof for address: {:?}", address);

        // TODO: Implement actual merkle proof generation
        // This would build a proof path from the state root to the specific account/storage

        Ok(vec![0; 256]) // Placeholder proof
    }

    /// Verify merkle proof
    #[instrument(skip(self, proof))]
    pub async fn verify_merkle_proof(
        &self,
        proof: &[u8],
        address: &Address,
        key: Option<U256>,
        value: &StateValue,
    ) -> Result<bool> {
        debug!("Verifying merkle proof for address: {:?}", address);

        // TODO: Implement actual merkle proof verification
        Ok(true) // Placeholder verification
    }

    /// Health check
    pub async fn is_healthy(&self) -> bool {
        let state = self.current_state.read().await;
        let cache = self.state_cache.read().await;

        // Check if state manager is functioning
        state.state_root.len() == 32 &&
        cache.cache_statistics.total_size < 1_000_000_000 && // 1GB cache limit
        state.block_number < u64::MAX
    }

    async fn apply_single_change(&self, state: &mut L2State, change: &StateChange) -> Result<()> {
        match &change.change_type {
            StateChangeType::AccountCreation => {
                if let StateValue::Account(account) = &change.new_value {
                    state.accounts.insert(change.address.clone(), account.clone());
                }
            }
            StateChangeType::AccountDeletion => {
                state.accounts.remove(&change.address);
            }
            StateChangeType::BalanceUpdate => {
                if let StateValue::U256(new_balance) = &change.new_value {
                    // Extract token type from context (simplified)
                    let token_type = "GCC".to_string(); // TODO: Extract properly
                    let balance_key = (change.address.clone(), token_type);
                    state.balances.insert(balance_key, *new_balance);
                }
            }
            StateChangeType::NonceUpdate => {
                if let StateValue::U256(new_nonce) = &change.new_value {
                    state.nonces.insert(change.address.clone(), new_nonce.as_u64());
                }
            }
            StateChangeType::StorageUpdate => {
                if let (Some(key), StateValue::U256(new_value)) = (&change.key, &change.new_value) {
                    state.storage.insert((change.address.clone(), *key), *new_value);
                }
            }
            StateChangeType::CodeUpdate => {
                if let StateValue::Bytes(new_code) = &change.new_value {
                    if let Some(account) = state.accounts.get_mut(&change.address) {
                        account.code = Some(new_code.clone());
                        account.code_hash = self.compute_code_hash(new_code).await;
                    }
                }
            }
            StateChangeType::SupplyUpdate => {
                if let StateValue::U256(new_supply) = &change.new_value {
                    // Extract token type from context
                    let token_type = "GCC".to_string(); // TODO: Extract properly
                    state.total_supply.insert(token_type, *new_supply);
                }
            }
        }

        Ok(())
    }

    async fn compute_code_hash(&self, code: &[u8]) -> Vec<u8> {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(code);
        hasher.finalize().to_vec()
    }

    async fn cache_account(&self, address: &Address, account: &AccountState) {
        let mut cache = self.state_cache.write().await;
        cache.cached_accounts.insert(address.clone(), CachedAccount {
            account: account.clone(),
            cached_at: SystemTime::now(),
            access_count: 1,
            dirty: false,
        });
    }

    async fn invalidate_cache_for_update(&self, update: &StateUpdate) {
        let mut cache = self.state_cache.write().await;

        for change in &update.updates {
            // Invalidate account cache
            cache.cached_accounts.remove(&change.address);

            // Invalidate storage cache if applicable
            if let Some(key) = &change.key {
                cache.cached_storage.remove(&(change.address.clone(), *key));
            }
        }
    }

    async fn clear_cache(&self) {
        let mut cache = self.state_cache.write().await;
        cache.cached_accounts.clear();
        cache.cached_storage.clear();
        cache.cache_statistics = CacheStatistics {
            total_size: 0,
            hit_rate: 0.0,
            miss_rate: 0.0,
            eviction_count: 0,
            dirty_entries: 0,
        };
    }

    async fn compress_data(&self, data: &[u8]) -> Result<Vec<u8>> {
        // TODO: Implement actual compression based on algorithm
        // For now, return data as-is
        Ok(data.to_vec())
    }

    async fn decompress_data(&self, compressed_data: &[u8]) -> Result<Vec<u8>> {
        // TODO: Implement actual decompression
        // For now, return data as-is
        Ok(compressed_data.to_vec())
    }
}

impl Default for L2State {
    fn default() -> Self {
        Self {
            state_root: vec![0; 32],
            block_number: 0,
            timestamp: SystemTime::now(),
            accounts: HashMap::new(),
            storage: HashMap::new(),
            balances: HashMap::new(),
            nonces: HashMap::new(),
            total_supply: HashMap::new(),
            transaction_count: 0,
            gas_used: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_state_manager_creation() {
        let config = SettlementConfig::default();
        let manager = StateManager::new(config).await.unwrap();
        assert!(manager.is_healthy().await);
    }

    #[tokio::test]
    async fn test_snapshot_creation() {
        let config = SettlementConfig::default();
        let manager = StateManager::new(config).await.unwrap();

        let snapshot = manager.create_snapshot().await.unwrap();
        assert!(!snapshot.snapshot_id.is_empty());
        assert_eq!(snapshot.block_number, 0);
    }

    #[tokio::test]
    async fn test_account_balance() {
        let config = SettlementConfig::default();
        let manager = StateManager::new(config).await.unwrap();

        let address = Address::from("0x1234567890123456789012345678901234567890");
        let balance = manager.get_balance(&address, "GCC").await.unwrap();
        assert_eq!(balance, U256::ZERO);
    }
}