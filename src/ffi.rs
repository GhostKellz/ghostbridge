/*!
Enhanced FFI safety layer for Rust-Zig communication

Memory-safe abstraction layer for calling into GhostPlane (Zig) from Rust,
with comprehensive error handling and type conversion.
*/

use crate::error::{BridgeError, FfiError, Result};
use crate::types::{FfiResult, FfiTransaction, Transaction, TransactionReceipt, U256};
use std::ffi::{c_char, c_void, CStr, CString};
use std::ptr;
use std::sync::Arc;
use parking_lot::RwLock;
use tracing::{debug, error, instrument, warn};

/// Safe FFI handle for GhostPlane Zig runtime
pub struct GhostPlaneHandle {
    inner: Arc<RwLock<*mut c_void>>,
    initialized: bool,
}

unsafe impl Send for GhostPlaneHandle {}
unsafe impl Sync for GhostPlaneHandle {}

impl GhostPlaneHandle {
    /// Create a new uninitialized handle
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(ptr::null_mut())),
            initialized: false,
        }
    }

    /// Initialize the GhostPlane runtime
    #[instrument(skip(self, config))]
    pub async fn initialize(&mut self, config: &GhostPlaneConfig) -> Result<()> {
        debug!("Initializing GhostPlane FFI runtime");

        let config_json = serde_json::to_string(config)
            .map_err(|e| BridgeError::Ffi(FfiError::InvalidUtf8))?;

        let config_cstring = CString::new(config_json)
            .map_err(|_| BridgeError::Ffi(FfiError::InvalidUtf8))?;

        let handle = unsafe { ghostplane_init(config_cstring.as_ptr()) };

        if handle.is_null() {
            error!("Failed to initialize GhostPlane runtime");
            return Err(BridgeError::Ffi(FfiError::GhostPlane(
                "Runtime initialization failed".to_string(),
            )));
        }

        let mut inner = self.inner.write();
        *inner = handle;
        self.initialized = true;

        debug!("GhostPlane FFI runtime initialized successfully");
        Ok(())
    }

    /// Check if the handle is initialized and valid
    pub fn is_initialized(&self) -> bool {
        self.initialized && !self.inner.read().is_null()
    }

    /// Get the raw handle (unsafe)
    pub(crate) unsafe fn raw_handle(&self) -> *mut c_void {
        *self.inner.read()
    }
}

impl Drop for GhostPlaneHandle {
    fn drop(&mut self) {
        if self.is_initialized() {
            let handle = *self.inner.read();
            if !handle.is_null() {
                unsafe {
                    ghostplane_cleanup(handle);
                }
                debug!("GhostPlane FFI runtime cleaned up");
            }
        }
    }
}

/// Configuration for GhostPlane Zig runtime
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GhostPlaneConfig {
    pub network_id: u64,
    pub rpc_endpoint: String,
    pub max_batch_size: u32,
    pub settlement_timeout_ms: u64,
    pub enable_optimistic_execution: bool,
    pub zk_proof_generation: bool,
    pub memory_limit_mb: u64,
}

impl Default for GhostPlaneConfig {
    fn default() -> Self {
        Self {
            network_id: 10000, // GhostPlane default
            rpc_endpoint: "http://localhost:9090".to_string(),
            max_batch_size: 1000,
            settlement_timeout_ms: 30000, // 30 seconds
            enable_optimistic_execution: true,
            zk_proof_generation: true,
            memory_limit_mb: 1024, // 1GB
        }
    }
}

/// Safe FFI wrapper for GhostPlane operations
pub struct GhostPlaneFfi {
    handle: GhostPlaneHandle,
    config: GhostPlaneConfig,
}

impl GhostPlaneFfi {
    /// Create a new FFI wrapper
    pub fn new(config: GhostPlaneConfig) -> Self {
        Self {
            handle: GhostPlaneHandle::new(),
            config,
        }
    }

    /// Initialize the FFI connection
    pub async fn initialize(&mut self) -> Result<()> {
        self.handle.initialize(&self.config).await
    }

    /// Submit a transaction to GhostPlane L2
    #[instrument(skip(self, transaction))]
    pub async fn submit_transaction(&self, transaction: &Transaction) -> Result<TransactionReceipt> {
        if !self.handle.is_initialized() {
            return Err(BridgeError::Ffi(FfiError::GhostPlane(
                "GhostPlane not initialized".to_string(),
            )));
        }

        debug!("Converting transaction to FFI format");
        let ffi_tx = self.convert_transaction_to_ffi(transaction)?;

        debug!("Submitting transaction to GhostPlane");
        let mut result: FfiResult<FfiTransactionReceipt> = FfiResult {
            success: false,
            data: FfiTransactionReceipt::default(),
            error_code: 0,
            error_message: ptr::null(),
        };

        let status = unsafe {
            ghostplane_submit_transaction(
                self.handle.raw_handle(),
                &ffi_tx,
                &mut result,
            )
        };

        self.handle_ffi_result(status, result, "submit_transaction")
            .map(|ffi_receipt| self.convert_receipt_from_ffi(&ffi_receipt))
    }

    /// Submit a batch of transactions
    #[instrument(skip(self, transactions))]
    pub async fn submit_batch(&self, transactions: &[Transaction]) -> Result<BatchResult> {
        if !self.handle.is_initialized() {
            return Err(BridgeError::Ffi(FfiError::GhostPlane(
                "GhostPlane not initialized".to_string(),
            )));
        }

        if transactions.len() > self.config.max_batch_size as usize {
            return Err(BridgeError::Ffi(FfiError::InvalidDataLength {
                expected: self.config.max_batch_size as usize,
                actual: transactions.len(),
            }));
        }

        debug!("Converting {} transactions to FFI batch", transactions.len());
        let ffi_transactions: Result<Vec<FfiTransaction>> = transactions
            .iter()
            .map(|tx| self.convert_transaction_to_ffi(tx))
            .collect();

        let ffi_transactions = ffi_transactions?;

        let mut result: FfiResult<FfiBatchResult> = FfiResult {
            success: false,
            data: FfiBatchResult::default(),
            error_code: 0,
            error_message: ptr::null(),
        };

        let status = unsafe {
            ghostplane_submit_batch(
                self.handle.raw_handle(),
                ffi_transactions.as_ptr(),
                ffi_transactions.len() as u32,
                &mut result,
            )
        };

        self.handle_ffi_result(status, result, "submit_batch")
            .map(|ffi_batch| self.convert_batch_result_from_ffi(&ffi_batch))
    }

    /// Query L2 state
    #[instrument(skip(self, key))]
    pub async fn query_state(&self, key: &[u8]) -> Result<Vec<u8>> {
        if !self.handle.is_initialized() {
            return Err(BridgeError::Ffi(FfiError::GhostPlane(
                "GhostPlane not initialized".to_string(),
            )));
        }

        let mut result_len: u32 = 0;
        let result_ptr = unsafe {
            ghostplane_query_state(
                self.handle.raw_handle(),
                key.as_ptr(),
                key.len() as u32,
                &mut result_len,
            )
        };

        if result_ptr.is_null() {
            return Err(BridgeError::Ffi(FfiError::NullPointer));
        }

        // Safely copy data and free FFI memory
        let result = unsafe {
            let slice = std::slice::from_raw_parts(result_ptr, result_len as usize);
            let vec = slice.to_vec();
            ghostplane_free_bytes(result_ptr);
            vec
        };

        debug!("Retrieved {} bytes from L2 state", result.len());
        Ok(result)
    }

    /// Get current L2 state root
    pub async fn get_state_root(&self) -> Result<[u8; 32]> {
        if !self.handle.is_initialized() {
            return Err(BridgeError::Ffi(FfiError::GhostPlane(
                "GhostPlane not initialized".to_string(),
            )));
        }

        let mut result: FfiResult<[u8; 32]> = FfiResult {
            success: false,
            data: [0u8; 32],
            error_code: 0,
            error_message: ptr::null(),
        };

        let status = unsafe {
            ghostplane_get_state_root(self.handle.raw_handle(), &mut result)
        };

        self.handle_ffi_result(status, result, "get_state_root")
    }

    /// Convert Rust transaction to FFI format
    fn convert_transaction_to_ffi(&self, tx: &Transaction) -> Result<FfiTransaction> {
        let from_bytes = match &tx.from_address {
            addr => addr.0,
        };

        let to_bytes = match &tx.to_address {
            addr => addr.0,
        };

        let value = tx.amount.amount.to_u64();
        let gas_limit = 21000; // Default gas limit
        let gas_price = tx.fee.gcc_fee.amount.to_u64();

        // Serialize transaction data
        let data = tx.to_bytes()?;

        // Create FFI transaction
        let ffi_tx = FfiTransaction {
            from: from_bytes,
            to: to_bytes,
            value,
            gas_limit,
            gas_price,
            nonce: tx.nonce,
            data_ptr: data.as_ptr(),
            data_len: data.len() as u32,
            signature: [0u8; 65], // Placeholder for signature
        };

        Ok(ffi_tx)
    }

    /// Convert FFI receipt to Rust format
    fn convert_receipt_from_ffi(&self, ffi_receipt: &FfiTransactionReceipt) -> TransactionReceipt {
        TransactionReceipt {
            transaction_hash: crate::types::TransactionHash(ffi_receipt.tx_hash),
            block_number: ffi_receipt.block_number,
            block_hash: ffi_receipt.block_hash,
            transaction_index: ffi_receipt.tx_index,
            gas_used: ffi_receipt.gas_used,
            success: ffi_receipt.success,
            logs: vec![], // TODO: Convert logs
        }
    }

    /// Convert FFI batch result to Rust format
    fn convert_batch_result_from_ffi(&self, ffi_batch: &FfiBatchResult) -> BatchResult {
        BatchResult {
            batch_hash: ffi_batch.batch_hash,
            state_root: ffi_batch.state_root,
            transaction_count: ffi_batch.tx_count,
            gas_used: ffi_batch.total_gas_used,
            success: ffi_batch.success,
        }
    }

    /// Handle FFI result and extract data safely
    fn handle_ffi_result<T>(
        &self,
        status: i32,
        result: FfiResult<T>,
        operation: &str,
    ) -> Result<T> {
        if status != 0 {
            error!("FFI operation '{}' failed with status: {}", operation, status);
            return Err(BridgeError::Ffi(FfiError::ResultCode { code: status }));
        }

        if !result.success {
            let error_msg = if !result.error_message.is_null() {
                unsafe {
                    CStr::from_ptr(result.error_message)
                        .to_string_lossy()
                        .to_string()
                }
            } else {
                format!("Unknown error in {}", operation)
            };

            error!("FFI operation '{}' failed: {}", operation, error_msg);
            return Err(BridgeError::Ffi(FfiError::GhostPlane(error_msg)));
        }

        debug!("FFI operation '{}' completed successfully", operation);
        Ok(result.data)
    }
}

/// Batch operation result
#[derive(Debug, Clone)]
pub struct BatchResult {
    pub batch_hash: [u8; 32],
    pub state_root: [u8; 32],
    pub transaction_count: u32,
    pub gas_used: u64,
    pub success: bool,
}

/// FFI-compatible transaction receipt
#[repr(C)]
#[derive(Debug, Clone)]
pub struct FfiTransactionReceipt {
    pub tx_hash: [u8; 32],
    pub block_number: u64,
    pub block_hash: [u8; 32],
    pub tx_index: u32,
    pub gas_used: u64,
    pub success: bool,
}

impl Default for FfiTransactionReceipt {
    fn default() -> Self {
        Self {
            tx_hash: [0u8; 32],
            block_number: 0,
            block_hash: [0u8; 32],
            tx_index: 0,
            gas_used: 0,
            success: false,
        }
    }
}

/// FFI-compatible batch result
#[repr(C)]
#[derive(Debug, Clone)]
pub struct FfiBatchResult {
    pub batch_hash: [u8; 32],
    pub state_root: [u8; 32],
    pub tx_count: u32,
    pub total_gas_used: u64,
    pub success: bool,
}

impl Default for FfiBatchResult {
    fn default() -> Self {
        Self {
            batch_hash: [0u8; 32],
            state_root: [0u8; 32],
            tx_count: 0,
            total_gas_used: 0,
            success: false,
        }
    }
}

// External FFI function declarations for GhostPlane (Zig)
extern "C" {
    /// Initialize GhostPlane runtime
    fn ghostplane_init(config: *const c_char) -> *mut c_void;

    /// Submit a single transaction
    fn ghostplane_submit_transaction(
        handle: *mut c_void,
        tx: *const FfiTransaction,
        result: *mut FfiResult<FfiTransactionReceipt>,
    ) -> i32;

    /// Submit a batch of transactions
    fn ghostplane_submit_batch(
        handle: *mut c_void,
        transactions: *const FfiTransaction,
        count: u32,
        result: *mut FfiResult<FfiBatchResult>,
    ) -> i32;

    /// Query L2 state
    fn ghostplane_query_state(
        handle: *mut c_void,
        key: *const u8,
        key_len: u32,
        result_len: *mut u32,
    ) -> *const u8;

    /// Get current state root
    fn ghostplane_get_state_root(
        handle: *mut c_void,
        result: *mut FfiResult<[u8; 32]>,
    ) -> i32;

    /// Free bytes allocated by GhostPlane
    fn ghostplane_free_bytes(ptr: *const u8);

    /// Cleanup GhostPlane runtime
    fn ghostplane_cleanup(handle: *mut c_void);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Address, Network, ChainId, TokenType, TokenAmount};

    #[test]
    fn test_ghostplane_config_default() {
        let config = GhostPlaneConfig::default();
        assert_eq!(config.network_id, 10000);
        assert!(config.enable_optimistic_execution);
    }

    #[test]
    fn test_handle_creation() {
        let handle = GhostPlaneHandle::new();
        assert!(!handle.is_initialized());
    }

    #[tokio::test]
    async fn test_ffi_wrapper_creation() {
        let config = GhostPlaneConfig::default();
        let ffi = GhostPlaneFfi::new(config);
        assert!(!ffi.handle.is_initialized());
    }

    #[test]
    fn test_transaction_conversion() {
        let config = GhostPlaneConfig::default();
        let ffi = GhostPlaneFfi::new(config);

        let tx = Transaction {
            id: uuid::Uuid::new_v4(),
            from_chain: Network::Ethereum { chain_id: ChainId::ETHEREUM },
            to_chain: Network::GhostPlane { chain_id: ChainId::GHOSTPLANE },
            from_address: Address([1u8; 20]),
            to_address: Address([2u8; 20]),
            amount: TokenAmount::new(TokenType::Gcc, U256::from(1000)),
            fee: crate::types::MultiTokenFee {
                gcc_fee: TokenAmount::new(TokenType::Gcc, U256::from(21)),
                spirit_fee: TokenAmount::new(TokenType::Spirit, U256::ZERO),
                mana_fee: TokenAmount::new(TokenType::Mana, U256::ZERO),
                ghost_fee: TokenAmount::new(TokenType::Ghost, U256::ZERO),
            },
            nonce: 1,
            data: vec![1, 2, 3, 4],
            signature: None,
            created_at: chrono::Utc::now(),
        };

        let ffi_tx = ffi.convert_transaction_to_ffi(&tx).unwrap();
        assert_eq!(ffi_tx.from, [1u8; 20]);
        assert_eq!(ffi_tx.to, [2u8; 20]);
        assert_eq!(ffi_tx.value, 1000);
        assert_eq!(ffi_tx.nonce, 1);
    }
}