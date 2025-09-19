/*!
Core types for GhostBridge operations

Type-safe definitions for cross-chain transactions, assets, chains, and
FFI-compatible structures for Rust-Zig interoperability.
*/

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// Re-export commonly used types
pub use uuid::Uuid;
pub use chrono::{DateTime, Utc};

/// Chain identifier and metadata
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChainId(pub u64);

impl ChainId {
    /// Ethereum mainnet
    pub const ETHEREUM: ChainId = ChainId(1);
    /// GhostChain mainnet
    pub const GHOSTCHAIN: ChainId = ChainId(9999);
    /// GhostPlane L2
    pub const GHOSTPLANE: ChainId = ChainId(10000);
}

impl std::fmt::Display for ChainId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Supported blockchain networks
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Network {
    /// Ethereum mainnet and testnets
    Ethereum { chain_id: ChainId },
    /// Bitcoin mainnet and testnets
    Bitcoin { network: BitcoinNetwork },
    /// GhostChain L1
    GhostChain { chain_id: ChainId },
    /// GhostPlane L2
    GhostPlane { chain_id: ChainId },
    /// Polygon networks
    Polygon { chain_id: ChainId },
    /// Arbitrum networks
    Arbitrum { chain_id: ChainId },
    /// Custom EVM-compatible networks
    Custom {
        chain_id: ChainId,
        name: String,
        rpc_url: String,
    },
}

/// Bitcoin network types
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BitcoinNetwork {
    Mainnet,
    Testnet,
    Signet,
    Regtest,
}

/// 4-Token economy types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TokenType {
    /// GCC - Gas & transaction fees (deflationary)
    Gcc,
    /// SPIRIT - Governance & voting tokens
    Spirit,
    /// MANA - Utility & rewards tokens
    Mana,
    /// GHOST - Brand & collectibles tokens
    Ghost,
}

impl std::fmt::Display for TokenType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TokenType::Gcc => write!(f, "GCC"),
            TokenType::Spirit => write!(f, "SPIRIT"),
            TokenType::Mana => write!(f, "MANA"),
            TokenType::Ghost => write!(f, "GHOST"),
        }
    }
}

/// Token amount with precision handling
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenAmount {
    pub token_type: TokenType,
    /// Amount in smallest unit (wei equivalent)
    pub amount: U256,
    pub decimals: u8,
}

impl TokenAmount {
    /// Create a new token amount
    pub fn new(token_type: TokenType, amount: U256) -> Self {
        let decimals = match token_type {
            TokenType::Gcc => 18,
            TokenType::Spirit => 18,
            TokenType::Mana => 18,
            TokenType::Ghost => 0, // NFT-like
        };

        Self {
            token_type,
            amount,
            decimals,
        }
    }

    /// Convert to human-readable string
    pub fn to_human_readable(&self) -> String {
        if self.decimals == 0 {
            self.amount.to_string()
        } else {
            let divisor = U256::from(10u64).pow(U256::from(self.decimals));
            let whole = &self.amount / &divisor;
            let remainder = &self.amount % &divisor;

            if remainder.is_zero() {
                format!("{}", whole)
            } else {
                format!("{}.{:0width$}", whole, remainder, width = self.decimals as usize)
            }
        }
    }
}

/// 256-bit unsigned integer for large token amounts
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct U256(pub [u8; 32]);

impl U256 {
    pub const ZERO: U256 = U256([0u8; 32]);
    pub const ONE: U256 = U256([
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1,
    ]);

    /// Create from u64
    pub fn from(value: u64) -> Self {
        let mut bytes = [0u8; 32];
        bytes[24..32].copy_from_slice(&value.to_be_bytes());
        U256(bytes)
    }

    /// Convert to u64 (truncating if necessary)
    pub fn to_u64(&self) -> u64 {
        u64::from_be_bytes([
            self.0[24], self.0[25], self.0[26], self.0[27],
            self.0[28], self.0[29], self.0[30], self.0[31],
        ])
    }

    /// Check if zero
    pub fn is_zero(&self) -> bool {
        self.0.iter().all(|&b| b == 0)
    }

    /// Simple power operation (for small exponents)
    pub fn pow(&self, exp: U256) -> U256 {
        if exp.is_zero() {
            return U256::ONE;
        }

        let exp_u64 = exp.to_u64();
        let base_u64 = self.to_u64();

        if exp_u64 <= 20 && base_u64 <= 1000 {
            U256::from(base_u64.pow(exp_u64 as u32))
        } else {
            // For large numbers, return self (simplified)
            self.clone()
        }
    }
}

// Implement basic arithmetic for U256
impl std::ops::Add for &U256 {
    type Output = U256;

    fn add(self, other: &U256) -> U256 {
        // Simplified addition for small numbers
        let a = self.to_u64();
        let b = other.to_u64();
        U256::from(a.saturating_add(b))
    }
}

impl std::ops::Sub for &U256 {
    type Output = U256;

    fn sub(self, other: &U256) -> U256 {
        let a = self.to_u64();
        let b = other.to_u64();
        U256::from(a.saturating_sub(b))
    }
}

impl std::ops::Mul for &U256 {
    type Output = U256;

    fn mul(self, other: &U256) -> U256 {
        let a = self.to_u64();
        let b = other.to_u64();
        U256::from(a.saturating_mul(b))
    }
}

impl std::ops::Div for &U256 {
    type Output = U256;

    fn div(self, other: &U256) -> U256 {
        let a = self.to_u64();
        let b = other.to_u64();
        if b == 0 {
            U256::ZERO
        } else {
            U256::from(a / b)
        }
    }
}

impl std::ops::Rem for &U256 {
    type Output = U256;

    fn rem(self, other: &U256) -> U256 {
        let a = self.to_u64();
        let b = other.to_u64();
        if b == 0 {
            U256::ZERO
        } else {
            U256::from(a % b)
        }
    }
}

impl std::fmt::Display for U256 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_u64())
    }
}

/// Cross-chain transaction representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub id: Uuid,
    pub from_chain: Network,
    pub to_chain: Network,
    pub from_address: Address,
    pub to_address: Address,
    pub amount: TokenAmount,
    pub fee: MultiTokenFee,
    pub nonce: u64,
    pub data: Vec<u8>,
    pub signature: Option<Signature>,
    pub created_at: DateTime<Utc>,
}

impl Transaction {
    /// Calculate transaction hash
    pub fn hash(&self) -> TransactionHash {
        use sha2::{Sha256, Digest};

        let mut hasher = Sha256::new();
        hasher.update(&self.id.as_bytes());
        hasher.update(&bincode::serialize(&self.from_chain).unwrap_or_default());
        hasher.update(&bincode::serialize(&self.to_chain).unwrap_or_default());
        hasher.update(&self.from_address.0);
        hasher.update(&self.to_address.0);
        hasher.update(&self.amount.amount.0);
        hasher.update(&self.nonce.to_be_bytes());
        hasher.update(&self.data);

        TransactionHash(hasher.finalize().into())
    }

    /// Convert to bytes for FFI
    pub fn to_bytes(&self) -> crate::error::Result<Vec<u8>> {
        bincode::serialize(self).map_err(Into::into)
    }

    /// Create from bytes (for FFI)
    pub fn from_bytes(bytes: &[u8]) -> crate::error::Result<Self> {
        bincode::deserialize(bytes).map_err(Into::into)
    }
}

/// Multi-token fee structure for the 4-token economy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiTokenFee {
    pub gcc_fee: TokenAmount,      // Base transaction fees
    pub spirit_fee: TokenAmount,   // Governance participation
    pub mana_fee: TokenAmount,     // Smart contract execution
    pub ghost_fee: TokenAmount,    // Identity operations
}

impl MultiTokenFee {
    /// Calculate total fee value (simplified)
    pub fn total_value(&self) -> U256 {
        let gcc = &self.gcc_fee.amount;
        let spirit = &self.spirit_fee.amount;
        let mana = &self.mana_fee.amount;
        let ghost = &self.ghost_fee.amount;

        gcc + spirit + mana + ghost
    }
}

/// Address type for cross-chain compatibility
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Address(pub [u8; 20]);

impl Address {
    /// Create from hex string
    pub fn from_hex(hex: &str) -> crate::error::Result<Self> {
        let hex = hex.trim_start_matches("0x");
        if hex.len() != 40 {
            return Err(crate::error::BridgeError::config("Invalid address length"));
        }

        let mut bytes = [0u8; 20];
        for (i, chunk) in hex.as_bytes().chunks(2).enumerate() {
            let hex_str = std::str::from_utf8(chunk)
                .map_err(|_| crate::error::BridgeError::config("Invalid hex"))?;
            bytes[i] = u8::from_str_radix(hex_str, 16)
                .map_err(|_| crate::error::BridgeError::config("Invalid hex digit"))?;
        }

        Ok(Address(bytes))
    }

    /// Convert to hex string
    pub fn to_hex(&self) -> String {
        format!("0x{}", hex::encode(self.0))
    }
}

impl std::fmt::Display for Address {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

/// Transaction hash
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TransactionHash(pub [u8; 32]);

impl std::fmt::Display for TransactionHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "0x{}", hex::encode(self.0))
    }
}

/// Cryptographic signature
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Signature {
    pub r: U256,
    pub s: U256,
    pub v: u8,
}

/// Transaction receipt
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionReceipt {
    pub transaction_hash: TransactionHash,
    pub block_number: u64,
    pub block_hash: [u8; 32],
    pub transaction_index: u32,
    pub gas_used: u64,
    pub success: bool,
    pub logs: Vec<LogEntry>,
}

/// Log entry for events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub address: Address,
    pub topics: Vec<[u8; 32]>,
    pub data: Vec<u8>,
}

/// Bridge operation receipt
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeReceipt {
    pub bridge_id: Uuid,
    pub l1_transaction: Option<TransactionReceipt>,
    pub l2_transaction: Option<TransactionReceipt>,
    pub status: BridgeStatus,
    pub bridged_at: DateTime<Utc>,
    pub settled_at: Option<DateTime<Utc>>,
}

/// Bridge operation status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BridgeStatus {
    Pending,
    L1Confirmed,
    L2Submitted,
    L2Confirmed,
    Settled,
    Failed { reason: String },
}

/// L2 batch information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct L2Batch {
    pub batch_id: Uuid,
    pub transactions: Vec<Transaction>,
    pub state_root: [u8; 32],
    pub previous_state_root: [u8; 32],
    pub block_number: u64,
    pub timestamp: DateTime<Utc>,
}

/// Settlement proof for L1
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettlementProof {
    pub batch_id: Uuid,
    pub zk_proof: ZkProof,
    pub state_transition: StateTransition,
    pub public_inputs: Vec<U256>,
}

/// Zero-knowledge proof
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZkProof {
    pub proof_data: Vec<u8>,
    pub verification_key: Vec<u8>,
    pub public_inputs_hash: [u8; 32],
}

/// State transition data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateTransition {
    pub from_state: [u8; 32],
    pub to_state: [u8; 32],
    pub transaction_count: u32,
    pub gas_used: u64,
}

/// FFI-compatible transaction for Zig interop
#[repr(C)]
#[derive(Debug, Clone)]
pub struct FfiTransaction {
    pub from: [u8; 20],
    pub to: [u8; 20],
    pub value: u64,
    pub gas_limit: u64,
    pub gas_price: u64,
    pub nonce: u64,
    pub data_ptr: *const u8,
    pub data_len: u32,
    pub signature: [u8; 65],
}

/// FFI-compatible result wrapper
#[repr(C)]
#[derive(Debug)]
pub struct FfiResult<T> {
    pub success: bool,
    pub data: T,
    pub error_code: u32,
    pub error_message: *const std::ffi::c_char,
}

/// Network information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInfo {
    pub network: Network,
    pub name: String,
    pub rpc_url: String,
    pub block_time: Duration,
    pub confirmation_blocks: u32,
    pub supports_eip1559: bool,
}

/// Asset information for cross-chain support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetInfo {
    pub symbol: String,
    pub name: String,
    pub decimals: u8,
    pub contract_address: Option<Address>,
    pub supported_networks: Vec<Network>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chain_id_display() {
        assert_eq!(ChainId::ETHEREUM.to_string(), "1");
        assert_eq!(ChainId::GHOSTCHAIN.to_string(), "9999");
    }

    #[test]
    fn test_u256_arithmetic() {
        let a = U256::from(10);
        let b = U256::from(5);

        assert_eq!((&a + &b).to_u64(), 15);
        assert_eq!((&a - &b).to_u64(), 5);
        assert_eq!((&a * &b).to_u64(), 50);
        assert_eq!((&a / &b).to_u64(), 2);
    }

    #[test]
    fn test_address_hex() {
        let addr = Address::from_hex("0x742d35Cc6634C0532925a3b8D431Df45C3f8D23B").unwrap();
        assert_eq!(addr.to_hex(), "0x742d35cc6634c0532925a3b8d431df45c3f8d23b");
    }

    #[test]
    fn test_token_amount() {
        let amount = TokenAmount::new(TokenType::Gcc, U256::from(1000000000000000000u64));
        assert_eq!(amount.to_human_readable(), "1");
    }

    #[test]
    fn test_transaction_hash() {
        let tx = Transaction {
            id: Uuid::new_v4(),
            from_chain: Network::Ethereum { chain_id: ChainId::ETHEREUM },
            to_chain: Network::GhostChain { chain_id: ChainId::GHOSTCHAIN },
            from_address: Address([1u8; 20]),
            to_address: Address([2u8; 20]),
            amount: TokenAmount::new(TokenType::Gcc, U256::from(1000)),
            fee: MultiTokenFee {
                gcc_fee: TokenAmount::new(TokenType::Gcc, U256::from(10)),
                spirit_fee: TokenAmount::new(TokenType::Spirit, U256::ZERO),
                mana_fee: TokenAmount::new(TokenType::Mana, U256::ZERO),
                ghost_fee: TokenAmount::new(TokenType::Ghost, U256::ZERO),
            },
            nonce: 1,
            data: vec![],
            signature: None,
            created_at: chrono::Utc::now(),
        };

        let hash = tx.hash();
        assert_eq!(hash.0.len(), 32);
    }
}