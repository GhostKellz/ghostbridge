/*!
Error types for GhostBridge operations

Comprehensive error handling for cross-chain bridge operations, FFI safety,
and service integration failures.
*/

use std::fmt;
use thiserror::Error;

/// Result type alias for GhostBridge operations
pub type Result<T> = std::result::Result<T, BridgeError>;

/// Comprehensive error types for GhostBridge operations
#[derive(Error, Debug)]
pub enum BridgeError {
    /// Configuration errors
    #[error("Configuration error: {0}")]
    Config(String),

    /// Network and transport errors
    #[error("Network error: {0}")]
    Network(#[from] NetworkError),

    /// FFI and memory safety errors
    #[error("FFI error: {0}")]
    Ffi(#[from] FfiError),

    /// Service integration errors
    #[error("Service error: {0}")]
    Service(#[from] ServiceError),

    /// Cross-chain operation errors
    #[error("Cross-chain error: {0}")]
    CrossChain(#[from] CrossChainError),

    /// L2 settlement errors
    #[error("L2 settlement error: {0}")]
    Settlement(#[from] SettlementError),

    /// Security and authentication errors
    #[error("Security error: {0}")]
    Security(#[from] SecurityError),

    /// Token economy errors
    #[error("Token error: {0}")]
    Token(#[from] TokenError),

    /// Serialization/deserialization errors
    #[error("Serialization error: {0}")]
    Serialization(#[from] SerializationError),

    /// Generic I/O errors
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Internal errors that shouldn't normally occur
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Network and transport specific errors
#[derive(Error, Debug)]
pub enum NetworkError {
    #[error("Connection failed to {endpoint}: {source}")]
    ConnectionFailed {
        endpoint: String,
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    #[error("GQUIC transport error: {0}")]
    QuicTransport(String),

    #[error("gRPC error: {0}")]
    Grpc(#[from] tonic::Status),

    #[error("Timeout after {duration_ms}ms")]
    Timeout { duration_ms: u64 },

    #[error("Invalid endpoint: {0}")]
    InvalidEndpoint(String),

    #[error("TLS handshake failed: {0}")]
    TlsHandshake(String),

    #[error("Connection pool exhausted")]
    PoolExhausted,
}

/// FFI boundary and memory safety errors
#[derive(Error, Debug)]
pub enum FfiError {
    #[error("Null pointer passed to FFI function")]
    NullPointer,

    #[error("Invalid FFI data length: expected {expected}, got {actual}")]
    InvalidDataLength { expected: usize, actual: usize },

    #[error("Memory allocation failed")]
    MemoryAllocation,

    #[error("Invalid UTF-8 string from FFI")]
    InvalidUtf8,

    #[error("GhostPlane FFI error: {0}")]
    GhostPlane(String),

    #[error("FFI result code: {code}")]
    ResultCode { code: i32 },

    #[error("Memory safety violation: {0}")]
    MemorySafety(String),
}

/// Service integration errors
#[derive(Error, Debug)]
pub enum ServiceError {
    #[error("GHOSTD service error: {0}")]
    Ghostd(String),

    #[error("WALLETD service error: {0}")]
    Walletd(String),

    #[error("GID service error: {0}")]
    Gid(String),

    #[error("CNS service error: {0}")]
    Cns(String),

    #[error("GLEDGER service error: {0}")]
    Gledger(String),

    #[error("GSIG service error: {0}")]
    Gsig(String),

    #[error("RVM service error: {0}")]
    Rvm(String),

    #[error("Service unavailable: {service}")]
    ServiceUnavailable { service: String },

    #[error("Service authentication failed: {service}")]
    AuthenticationFailed { service: String },

    #[error("Etherlink client error: {0}")]
    EtherlinkClient(String),
}

/// Cross-chain operation errors
#[derive(Error, Debug)]
pub enum CrossChainError {
    #[error("Unsupported chain: {chain_id}")]
    UnsupportedChain { chain_id: u64 },

    #[error("Chain not available: {chain_id}")]
    ChainUnavailable { chain_id: u64 },

    #[error("Invalid transaction for chain {chain_id}: {reason}")]
    InvalidTransaction { chain_id: u64, reason: String },

    #[error("Bridge operation failed: {operation}")]
    BridgeOperationFailed { operation: String },

    #[error("Asset not supported on chain {chain_id}: {asset}")]
    UnsupportedAsset { chain_id: u64, asset: String },

    #[error("Insufficient liquidity for {asset} on chain {chain_id}")]
    InsufficientLiquidity { chain_id: u64, asset: String },

    #[error("Lock proof verification failed")]
    LockProofVerificationFailed,

    #[error("Cross-chain message timeout")]
    MessageTimeout,
}

/// L2 settlement specific errors
#[derive(Error, Debug)]
pub enum SettlementError {
    #[error("Batch processing failed: {0}")]
    BatchProcessingFailed(String),

    #[error("State root mismatch: expected {expected}, got {actual}")]
    StateRootMismatch { expected: String, actual: String },

    #[error("ZK proof generation failed: {0}")]
    ZkProofFailed(String),

    #[error("ZK proof verification failed")]
    ZkProofVerificationFailed,

    #[error("Fraud proof generation failed: {0}")]
    FraudProofFailed(String),

    #[error("Settlement timeout after {duration_ms}ms")]
    SettlementTimeout { duration_ms: u64 },

    #[error("L1 settlement failed: {0}")]
    L1SettlementFailed(String),

    #[error("Gas estimation failed: {0}")]
    GasEstimationFailed(String),
}

/// Security and Guardian Framework errors
#[derive(Error, Debug)]
pub enum SecurityError {
    #[error("Guardian authentication failed: {0}")]
    GuardianAuthFailed(String),

    #[error("Privacy policy violation: {policy}")]
    PrivacyPolicyViolation { policy: String },

    #[error("Identity verification failed: {identity}")]
    IdentityVerificationFailed { identity: String },

    #[error("Signature verification failed")]
    SignatureVerificationFailed,

    #[error("Invalid cryptographic operation: {0}")]
    CryptographicOperation(String),

    #[error("Key derivation failed: {0}")]
    KeyDerivationFailed(String),

    #[error("Encryption failed: {0}")]
    EncryptionFailed(String),

    #[error("Decryption failed: {0}")]
    DecryptionFailed(String),

    #[error("Trust level insufficient: required {required}, got {actual}")]
    InsufficientTrustLevel { required: u8, actual: u8 },
}

/// Token economy specific errors
#[derive(Error, Debug)]
pub enum TokenError {
    #[error("Insufficient {token} balance: required {required}, available {available}")]
    InsufficientBalance {
        token: String,
        required: String,
        available: String,
    },

    #[error("Invalid token type: {token}")]
    InvalidTokenType { token: String },

    #[error("Token transfer failed: {0}")]
    TransferFailed(String),

    #[error("Gas calculation failed for token {token}: {reason}")]
    GasCalculationFailed { token: String, reason: String },

    #[error("Fee distribution failed: {0}")]
    FeeDistributionFailed(String),

    #[error("Token pricing unavailable for {token}")]
    PricingUnavailable { token: String },

    #[error("Invalid token amount: {amount}")]
    InvalidAmount { amount: String },
}

/// Serialization and data conversion errors
#[derive(Error, Debug)]
pub enum SerializationError {
    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Bincode serialization error: {0}")]
    Bincode(#[from] bincode::Error),

    #[error("Borsh serialization error: {0}")]
    Borsh(String),

    #[error("Protobuf encoding error: {0}")]
    Protobuf(#[from] prost::EncodeError),

    #[error("Protobuf decoding error: {0}")]
    ProtobufDecode(#[from] prost::DecodeError),

    #[error("Invalid data format: {0}")]
    InvalidFormat(String),
}

impl BridgeError {
    /// Create a configuration error
    pub fn config(msg: impl Into<String>) -> Self {
        Self::Config(msg.into())
    }

    /// Create an internal error
    pub fn internal(msg: impl Into<String>) -> Self {
        Self::Internal(msg.into())
    }

    /// Check if this error is retryable
    pub fn is_retryable(&self) -> bool {
        match self {
            BridgeError::Network(NetworkError::Timeout { .. }) => true,
            BridgeError::Network(NetworkError::ConnectionFailed { .. }) => true,
            BridgeError::Network(NetworkError::PoolExhausted) => true,
            BridgeError::Service(ServiceError::ServiceUnavailable { .. }) => true,
            BridgeError::CrossChain(CrossChainError::ChainUnavailable { .. }) => true,
            BridgeError::Settlement(SettlementError::SettlementTimeout { .. }) => true,
            _ => false,
        }
    }

    /// Get the error category for metrics
    pub fn category(&self) -> &'static str {
        match self {
            BridgeError::Config(_) => "config",
            BridgeError::Network(_) => "network",
            BridgeError::Ffi(_) => "ffi",
            BridgeError::Service(_) => "service",
            BridgeError::CrossChain(_) => "cross_chain",
            BridgeError::Settlement(_) => "settlement",
            BridgeError::Security(_) => "security",
            BridgeError::Token(_) => "token",
            BridgeError::Serialization(_) => "serialization",
            BridgeError::Io(_) => "io",
            BridgeError::Internal(_) => "internal",
        }
    }
}

// Conversion from etherlink errors
impl From<etherlink::EtherlinkError> for BridgeError {
    fn from(err: etherlink::EtherlinkError) -> Self {
        BridgeError::Service(ServiceError::EtherlinkClient(err.to_string()))
    }
}

// Conversion from GQUIC errors (when available)
#[cfg(feature = "gquic")]
impl From<gquic::Error> for BridgeError {
    fn from(err: gquic::Error) -> Self {
        BridgeError::Network(NetworkError::QuicTransport(err.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_categories() {
        assert_eq!(BridgeError::config("test").category(), "config");
        assert_eq!(
            BridgeError::Network(NetworkError::PoolExhausted).category(),
            "network"
        );
    }

    #[test]
    fn test_retryable_errors() {
        assert!(BridgeError::Network(NetworkError::Timeout { duration_ms: 1000 }).is_retryable());
        assert!(BridgeError::Network(NetworkError::PoolExhausted).is_retryable());
        assert!(!BridgeError::config("test").is_retryable());
    }

    #[test]
    fn test_error_display() {
        let err = BridgeError::config("invalid endpoint");
        assert!(err.to_string().contains("Configuration error"));
    }
}