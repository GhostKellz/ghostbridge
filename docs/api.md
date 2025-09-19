# GhostBridge API Reference

## Bridge Core API

### GhostBridge

Main bridge interface for cross-chain operations.

```rust
impl GhostBridge {
    /// Create new bridge instance
    pub async fn new(config: BridgeConfig) -> Result<Self>;

    /// Bridge a transaction
    pub async fn bridge_transaction(&self, tx: Transaction) -> Result<BridgeReceipt>;

    /// Get transaction status
    pub async fn get_transaction_status(&self, tx_id: &str) -> Result<TransactionStatus>;

    /// Get bridge statistics
    pub async fn get_statistics(&self) -> BridgeStatistics;
}
```

### BridgeConfig

Bridge configuration builder.

```rust
impl BridgeConfig {
    pub fn builder() -> BridgeConfigBuilder;
    pub fn ethereum_rpc(url: &str) -> Self;
    pub fn ghostchain_rpc(url: &str) -> Self;
    pub fn enable_l2_settlement(enabled: bool) -> Self;
    pub fn build(self) -> BridgeConfig;
}
```

## L2 Settlement API

### L2SettlementEngine

High-performance settlement engine.

```rust
impl L2SettlementEngine {
    /// Initialize settlement engine
    pub async fn new(
        config: SettlementConfig,
        services: Arc<ServiceManager>,
        fee_calculator: Arc<FeeCalculator>,
        security: Arc<GuardianSecurity>,
    ) -> Result<Self>;

    /// Start settlement engine
    pub async fn start(&self) -> Result<()>;

    /// Submit transaction for settlement
    pub async fn submit_transaction(&self, transaction: Transaction) -> Result<String>;

    /// Get settlement status
    pub async fn get_settlement_status(&self, tx_id: &str) -> Result<SettlementStatus>;

    /// Get performance metrics
    pub async fn get_performance_metrics(&self) -> PerformanceMetrics;

    /// Get settlement statistics
    pub async fn get_settlement_statistics(&self) -> SettlementStatistics;

    /// Health check
    pub async fn is_healthy(&self) -> bool;
}
```

## Security API

### GuardianSecurity

Zero-trust security framework.

```rust
impl GuardianSecurity {
    /// Initialize Guardian Framework
    pub async fn new(config: GuardianConfig) -> Result<Self>;

    /// Perform security check
    pub async fn security_check(&self, transaction: &Transaction) -> Result<SecurityResult>;

    /// Monitor for suspicious activity
    pub async fn monitor_activity(&self) -> Result<()>;

    /// Get security status
    pub async fn get_security_status(&self) -> SecurityStatus;

    /// Health check
    pub async fn health_check(&self) -> Result<SecurityHealth>;
}
```

### IdentityManager

Identity verification and management.

```rust
impl IdentityManager {
    /// Initialize identity manager
    pub async fn new(config: GuardianConfig) -> Result<Self>;

    /// Verify identity
    pub async fn verify_identity(&self, address: &Address) -> Result<IdentityResult>;

    /// Create new identity
    pub async fn create_identity(&self, address: &Address, did: DID) -> Result<Identity>;

    /// Add verification
    pub async fn add_verification(
        &self,
        address: &Address,
        method: &str,
        verifier: &str,
        proof: Vec<u8>,
    ) -> Result<()>;

    /// Add attestation
    pub async fn add_attestation(&self, address: &Address, attestation: Attestation) -> Result<()>;

    /// Record violation
    pub async fn record_violation(
        &self,
        address: &Address,
        violation_type: &str,
        severity: f64,
        description: &str,
    ) -> Result<()>;
}
```

## Economy API

### TokenManager

Multi-token economy management.

```rust
impl TokenManager {
    /// Initialize token manager
    pub async fn new(services: Arc<ServiceManager>) -> Result<Self>;

    /// Burn tokens
    pub async fn burn_tokens(&mut self, token_type: TokenType, amount: U256) -> Result<()>;

    /// Get total supply
    pub fn get_total_supply(&self, token_type: TokenType) -> U256;

    /// Get burned amount
    pub fn get_burned_amount(&self, token_type: TokenType) -> U256;

    /// Get circulating supply
    pub fn get_circulating_supply(&self, token_type: TokenType) -> U256;
}
```

### FeeCalculator

Dynamic fee calculation.

```rust
impl FeeCalculator {
    /// Initialize fee calculator
    pub async fn new(config: EconomyConfig) -> Result<Self>;

    /// Calculate transaction fee
    pub async fn calculate_fee(&self, transaction: &Transaction) -> Result<MultiTokenFee>;

    /// Calculate bridge fee
    pub async fn calculate_bridge_fee(
        &self,
        amount: &TokenAmount,
        source_chain: u64,
        destination_chain: u64,
    ) -> Result<MultiTokenFee>;

    /// Get current gas price
    pub async fn get_gas_price(&self, chain_id: u64) -> Result<U256>;
}
```

## Transport API

### GQUICTransport

High-performance QUIC transport.

```rust
impl GQUICTransport {
    /// Initialize GQUIC transport
    pub async fn new(config: GQUICConfig) -> Result<Self>;

    /// Connect to peer
    pub async fn connect(&self, endpoint: &str) -> Result<GQUICConnection>;

    /// Send message
    pub async fn send_message(
        &self,
        connection: &GQUICConnection,
        message: &[u8],
    ) -> Result<()>;

    /// Receive message
    pub async fn receive_message(&self, connection: &GQUICConnection) -> Result<Vec<u8>>;

    /// Get connection pool stats
    pub async fn get_pool_stats(&self) -> ConnectionPoolStats;
}
```

## Types

### Core Types

```rust
/// Transaction representation
pub struct Transaction {
    pub id: String,
    pub from_address: Address,
    pub to_address: Address,
    pub amount: TokenAmount,
    pub chain_id: u64,
    pub nonce: u64,
    pub gas_limit: u64,
    pub gas_price: U256,
    pub data: Vec<u8>,
    pub signature: Option<Vec<u8>>,
}

/// Token amount with type
pub struct TokenAmount {
    pub token_type: TokenType,
    pub amount: U256,
}

/// Supported token types
pub enum TokenType {
    Gcc,    // Gas token (deflationary)
    Spirit, // Governance token
    Mana,   // Utility token (deflationary)
    Ghost,  // NFT/collectible token
}

/// Address wrapper
pub struct Address(String);

/// 256-bit unsigned integer
pub struct U256([u64; 4]);
```

### Settlement Types

```rust
/// Settlement batch
pub struct SettlementBatch {
    pub batch_id: String,
    pub transactions: Vec<Transaction>,
    pub state_root: Vec<u8>,
    pub previous_state_root: Vec<u8>,
    pub merkle_proof: Vec<u8>,
    pub zk_proof: Option<Vec<u8>>,
    pub created_at: SystemTime,
    pub gas_used: u64,
    pub fee_paid: TokenAmount,
}

/// Settlement status
pub enum SettlementStatus {
    Pending,
    Processing,
    BatchedForSettlement,
    SubmittedToL1,
    ChallengePhase,
    Finalized,
    Failed(String),
}
```

### Security Types

```rust
/// Security check result
pub struct SecurityResult {
    pub approved: bool,
    pub trust_score: u8,
    pub risk_score: f64,
    pub violations: Vec<String>,
    pub required_actions: Vec<String>,
    pub audit_trail: Vec<String>,
}

/// Decentralized identifier
pub struct DID {
    pub method: String,
    pub identifier: String,
    pub full_did: String,
}

/// Identity representation
pub struct Identity {
    pub did: DID,
    pub address: Address,
    pub verification_level: u8,
    pub verification_methods: Vec<VerificationRecord>,
    pub attestations: Vec<Attestation>,
    pub created_at: SystemTime,
    pub last_verified: SystemTime,
    pub status: IdentityStatus,
}
```

## Error Handling

```rust
/// Main error type
pub enum BridgeError {
    Configuration(String),
    Network(String),
    Security(SecurityError),
    Economy(String),
    Settlement(String),
    Transport(String),
    Service(String),
}

/// Security-specific errors
pub enum SecurityError {
    IdentityVerificationFailed,
    PolicyViolation,
    InsufficientTrustLevel,
    CryptoOperationFailed,
    AuditLogFailed,
}

/// Result type alias
pub type Result<T> = std::result::Result<T, BridgeError>;
```

## Usage Examples

### Basic Bridge Operation

```rust
use ghostbridge::{GhostBridge, BridgeConfig, Transaction, TokenAmount, TokenType, U256, Address};

// Initialize bridge
let config = BridgeConfig::builder()
    .ethereum_rpc("https://mainnet.infura.io/v3/YOUR_KEY")
    .ghostchain_rpc("https://rpc.ghostchain.io")
    .enable_l2_settlement(true)
    .build();

let bridge = GhostBridge::new(config).await?;

// Create transaction
let transaction = Transaction {
    id: "tx_123".to_string(),
    from_address: Address::from("0x1234..."),
    to_address: Address::from("0x5678..."),
    amount: TokenAmount::new(TokenType::Gcc, U256::from(1000)),
    chain_id: 1,
    nonce: 1,
    gas_limit: 21000,
    gas_price: U256::from(20_000_000_000u64),
    data: vec![],
    signature: None, // Will be signed automatically
};

// Bridge the transaction
let receipt = bridge.bridge_transaction(transaction).await?;
println!("Transaction bridged: {}", receipt.transaction_hash);
```

### L2 Settlement

```rust
use ghostbridge::settlement::{L2SettlementEngine, SettlementConfig};

// Configure and start settlement engine
let config = SettlementConfig::default();
let engine = L2SettlementEngine::new(config, services, fee_calculator, security).await?;
engine.start().await?;

// Submit transaction
let tx_id = engine.submit_transaction(transaction).await?;

// Check status
let status = engine.get_settlement_status(&tx_id).await?;
match status {
    SettlementStatus::Finalized => println!("Transaction finalized!"),
    SettlementStatus::ChallengePhase => println!("In challenge phase"),
    _ => println!("Processing..."),
}
```

### Security Integration

```rust
use ghostbridge::security::{GuardianSecurity, GuardianConfig};

// Initialize security
let config = GuardianConfig::default();
let security = GuardianSecurity::new(config).await?;

// Security check
let result = security.security_check(&transaction).await?;
if result.approved {
    println!("Transaction approved with trust score: {}", result.trust_score);
} else {
    println!("Transaction rejected: {:?}", result.violations);
}
```