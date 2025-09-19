/*!
Bridge configuration for multi-chain support

Configuration for GhostBridge supporting Ethereum, Bitcoin, Polygon, Arbitrum,
and custom chains with the 4-token economy integration.
*/

use crate::error::{BridgeError, Result};
use crate::services::ServiceEndpoint;
use crate::types::{Network, ChainId, TokenType};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

/// Main bridge configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeConfig {
    /// Service endpoint configurations
    pub service_endpoints: ServiceEndpoints,

    /// Multi-chain network configurations
    pub networks: HashMap<ChainId, NetworkConfig>,

    /// L2 configuration
    pub l2_config: L2Config,

    /// Token economy configuration
    pub token_config: TokenConfig,

    /// Security and validation settings
    pub validation_rules: ValidationRules,

    /// Guardian Framework settings
    pub guardian_config: GuardianConfig,

    /// Performance settings
    pub default_timeout: Duration,
    pub max_retries: u32,
    pub enable_guardian_auth: bool,
    pub enable_metrics: bool,
}

/// Service endpoint configurations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceEndpoints {
    pub ghostd: ServiceEndpoint,
    pub walletd: ServiceEndpoint,
    pub gid: ServiceEndpoint,
    pub cns: ServiceEndpoint,
    pub gledger: ServiceEndpoint,
    pub gsig: ServiceEndpoint,
    pub ghostplane: ServiceEndpoint,
}

/// Network-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub network: Network,
    pub rpc_url: String,
    pub chain_id: ChainId,
    pub confirmation_blocks: u32,
    pub gas_price_multiplier: f64,
    pub max_gas_price: u64,
    pub supported_tokens: Vec<TokenType>,
    pub bridge_contract: Option<String>,
    pub is_testnet: bool,
    pub block_time_ms: u64,
}

/// L2 configuration for GhostPlane
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct L2Config {
    pub max_batch_size: u32,
    pub settlement_timeout: Duration,
    pub enable_optimistic_execution: bool,
    pub enable_zk_proofs: bool,
    pub target_tps: u32,
    pub max_pending_batches: u32,
    pub fraud_proof_window: Duration,
}

/// Token economy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenConfig {
    /// GCC (Gas & transaction fees) settings
    pub gcc: TokenSettings,
    /// SPIRIT (Governance & voting) settings
    pub spirit: TokenSettings,
    /// MANA (Utility & rewards) settings
    pub mana: TokenSettings,
    /// GHOST (Brand & collectibles) settings
    pub ghost: TokenSettings,
    /// Fee distribution percentages
    pub fee_distribution: FeeDistribution,
}

/// Individual token settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenSettings {
    pub decimals: u8,
    pub is_deflationary: bool,
    pub burn_rate_bps: u16, // Basis points (100 = 1%)
    pub min_fee_amount: u64,
    pub max_supply: Option<u64>,
}

/// Fee distribution across the ecosystem
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeDistribution {
    /// Percentage to L2 validators
    pub l2_validators: u8,
    /// Percentage to L1 settlement validators
    pub l1_validators: u8,
    /// Percentage to bridge security fund
    pub security_fund: u8,
    /// Percentage to protocol development
    pub protocol_development: u8,
}

/// Validation rules for transactions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationRules {
    pub min_transaction_amount: u64,
    pub max_transaction_amount: u64,
    pub require_signature: bool,
    pub require_guardian_approval: bool,
    pub blacklisted_addresses: Vec<String>,
    pub whitelist_only: bool,
    pub whitelisted_addresses: Vec<String>,
}

/// Guardian Framework configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuardianConfig {
    pub enable_zero_trust: bool,
    pub require_identity_verification: bool,
    pub privacy_policy_enforcement: bool,
    pub trust_level_threshold: u8,
    pub audit_all_operations: bool,
    pub guardian_endpoints: Vec<String>,
}

impl Default for BridgeConfig {
    fn default() -> Self {
        Self {
            service_endpoints: ServiceEndpoints::default(),
            networks: Self::default_networks(),
            l2_config: L2Config::default(),
            token_config: TokenConfig::default(),
            validation_rules: ValidationRules::default(),
            guardian_config: GuardianConfig::default(),
            default_timeout: Duration::from_secs(30),
            max_retries: 3,
            enable_guardian_auth: true,
            enable_metrics: true,
        }
    }
}

impl Default for ServiceEndpoints {
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
                timeout_ms: 10000,
            },
        }
    }
}

impl Default for L2Config {
    fn default() -> Self {
        Self {
            max_batch_size: 1000,
            settlement_timeout: Duration::from_secs(30),
            enable_optimistic_execution: true,
            enable_zk_proofs: true,
            target_tps: 50000,
            max_pending_batches: 10,
            fraud_proof_window: Duration::from_days(7),
        }
    }
}

impl Default for TokenConfig {
    fn default() -> Self {
        Self {
            gcc: TokenSettings {
                decimals: 18,
                is_deflationary: true,
                burn_rate_bps: 100, // 1% burn rate
                min_fee_amount: 1000000000000000, // 0.001 GCC
                max_supply: Some(21_000_000 * 10u64.pow(18)), // 21M GCC
            },
            spirit: TokenSettings {
                decimals: 18,
                is_deflationary: false,
                burn_rate_bps: 0,
                min_fee_amount: 500000000000000, // 0.0005 SPIRIT
                max_supply: None, // Unlimited for governance
            },
            mana: TokenSettings {
                decimals: 18,
                is_deflationary: false,
                burn_rate_bps: 50, // 0.5% burn rate
                min_fee_amount: 750000000000000, // 0.00075 MANA
                max_supply: Some(100_000_000 * 10u64.pow(18)), // 100M MANA
            },
            ghost: TokenSettings {
                decimals: 0, // NFT-like tokens
                is_deflationary: false,
                burn_rate_bps: 0,
                min_fee_amount: 1, // 1 GHOST
                max_supply: Some(10_000), // Limited collectibles
            },
            fee_distribution: FeeDistribution {
                l2_validators: 40,
                l1_validators: 30,
                security_fund: 20,
                protocol_development: 10,
            },
        }
    }
}

impl Default for ValidationRules {
    fn default() -> Self {
        Self {
            min_transaction_amount: 1000000000000000, // 0.001 tokens
            max_transaction_amount: 1000000000000000000000u64, // 1000 tokens
            require_signature: true,
            require_guardian_approval: false,
            blacklisted_addresses: vec![],
            whitelist_only: false,
            whitelisted_addresses: vec![],
        }
    }
}

impl Default for GuardianConfig {
    fn default() -> Self {
        Self {
            enable_zero_trust: true,
            require_identity_verification: true,
            privacy_policy_enforcement: true,
            trust_level_threshold: 7, // Out of 10
            audit_all_operations: true,
            guardian_endpoints: vec![
                "https://guardian1.ghostchain.io".to_string(),
                "https://guardian2.ghostchain.io".to_string(),
            ],
        }
    }
}

impl BridgeConfig {
    /// Create a new bridge configuration builder
    pub fn builder() -> BridgeConfigBuilder {
        BridgeConfigBuilder::new()
    }

    /// Get default network configurations
    fn default_networks() -> HashMap<ChainId, NetworkConfig> {
        let mut networks = HashMap::new();

        // Ethereum Mainnet
        networks.insert(
            ChainId::ETHEREUM,
            NetworkConfig {
                network: Network::Ethereum { chain_id: ChainId::ETHEREUM },
                rpc_url: "https://mainnet.infura.io/v3/YOUR_KEY".to_string(),
                chain_id: ChainId::ETHEREUM,
                confirmation_blocks: 12,
                gas_price_multiplier: 1.2,
                max_gas_price: 100_000_000_000, // 100 gwei
                supported_tokens: vec![TokenType::Gcc, TokenType::Spirit],
                bridge_contract: Some("0x1234...".to_string()),
                is_testnet: false,
                block_time_ms: 12000,
            },
        );

        // GhostChain Mainnet
        networks.insert(
            ChainId::GHOSTCHAIN,
            NetworkConfig {
                network: Network::GhostChain { chain_id: ChainId::GHOSTCHAIN },
                rpc_url: "https://rpc.ghostchain.io".to_string(),
                chain_id: ChainId::GHOSTCHAIN,
                confirmation_blocks: 6,
                gas_price_multiplier: 1.0,
                max_gas_price: 50_000_000_000, // 50 gwei
                supported_tokens: vec![TokenType::Gcc, TokenType::Spirit, TokenType::Mana, TokenType::Ghost],
                bridge_contract: Some("ghost1abcd...".to_string()),
                is_testnet: false,
                block_time_ms: 3000,
            },
        );

        // GhostPlane L2
        networks.insert(
            ChainId::GHOSTPLANE,
            NetworkConfig {
                network: Network::GhostPlane { chain_id: ChainId::GHOSTPLANE },
                rpc_url: "https://l2.ghostchain.io".to_string(),
                chain_id: ChainId::GHOSTPLANE,
                confirmation_blocks: 1,
                gas_price_multiplier: 0.1,
                max_gas_price: 1_000_000_000, // 1 gwei
                supported_tokens: vec![TokenType::Gcc, TokenType::Spirit, TokenType::Mana, TokenType::Ghost],
                bridge_contract: None, // Native L2
                is_testnet: false,
                block_time_ms: 100, // 100ms for high TPS
            },
        );

        // Polygon Mainnet
        networks.insert(
            ChainId(137), // Polygon mainnet
            NetworkConfig {
                network: Network::Polygon { chain_id: ChainId(137) },
                rpc_url: "https://polygon-rpc.com".to_string(),
                chain_id: ChainId(137),
                confirmation_blocks: 20,
                gas_price_multiplier: 1.1,
                max_gas_price: 200_000_000_000, // 200 gwei
                supported_tokens: vec![TokenType::Gcc],
                bridge_contract: Some("0x5678...".to_string()),
                is_testnet: false,
                block_time_ms: 2000,
            },
        );

        // Arbitrum One
        networks.insert(
            ChainId(42161), // Arbitrum One
            NetworkConfig {
                network: Network::Arbitrum { chain_id: ChainId(42161) },
                rpc_url: "https://arb1.arbitrum.io/rpc".to_string(),
                chain_id: ChainId(42161),
                confirmation_blocks: 1,
                gas_price_multiplier: 1.0,
                max_gas_price: 10_000_000_000, // 10 gwei
                supported_tokens: vec![TokenType::Gcc, TokenType::Spirit],
                bridge_contract: Some("0x9abc...".to_string()),
                is_testnet: false,
                block_time_ms: 250, // ~250ms
            },
        );

        networks
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<()> {
        // Validate fee distribution sums to 100%
        let total_distribution = self.token_config.fee_distribution.l2_validators
            + self.token_config.fee_distribution.l1_validators
            + self.token_config.fee_distribution.security_fund
            + self.token_config.fee_distribution.protocol_development;

        if total_distribution != 100 {
            return Err(BridgeError::config(format!(
                "Fee distribution must sum to 100%, got {}%",
                total_distribution
            )));
        }

        // Validate L2 configuration
        if self.l2_config.max_batch_size == 0 {
            return Err(BridgeError::config("L2 max batch size must be greater than 0"));
        }

        if self.l2_config.target_tps == 0 {
            return Err(BridgeError::config("L2 target TPS must be greater than 0"));
        }

        // Validate network configurations
        for (chain_id, network_config) in &self.networks {
            if network_config.confirmation_blocks == 0 {
                return Err(BridgeError::config(format!(
                    "Chain {} must have at least 1 confirmation block",
                    chain_id
                )));
            }

            if network_config.rpc_url.is_empty() {
                return Err(BridgeError::config(format!(
                    "Chain {} must have a valid RPC URL",
                    chain_id
                )));
            }
        }

        // Validate Guardian configuration
        if self.guardian_config.trust_level_threshold > 10 {
            return Err(BridgeError::config(
                "Guardian trust level threshold must be between 0 and 10"
            ));
        }

        Ok(())
    }
}

/// Builder for BridgeConfig
pub struct BridgeConfigBuilder {
    config: BridgeConfig,
}

impl BridgeConfigBuilder {
    pub fn new() -> Self {
        Self {
            config: BridgeConfig::default(),
        }
    }

    pub fn ethereum_rpc(mut self, url: &str) -> Self {
        if let Some(network) = self.config.networks.get_mut(&ChainId::ETHEREUM) {
            network.rpc_url = url.to_string();
        }
        self
    }

    pub fn ghostchain_rpc(mut self, url: &str) -> Self {
        if let Some(network) = self.config.networks.get_mut(&ChainId::GHOSTCHAIN) {
            network.rpc_url = url.to_string();
        }
        self
    }

    pub fn ghostplane_endpoint(mut self, host: &str, port: u16) -> Self {
        self.config.service_endpoints.ghostplane.host = host.to_string();
        self.config.service_endpoints.ghostplane.port = port;
        self
    }

    pub fn enable_l2_settlement(mut self, enable: bool) -> Self {
        self.config.l2_config.enable_optimistic_execution = enable;
        self
    }

    pub fn target_tps(mut self, tps: u32) -> Self {
        self.config.l2_config.target_tps = tps;
        self
    }

    pub fn enable_guardian_auth(mut self, enable: bool) -> Self {
        self.config.enable_guardian_auth = enable;
        self
    }

    pub fn add_custom_network(mut self, chain_id: u64, config: NetworkConfig) -> Self {
        self.config.networks.insert(ChainId(chain_id), config);
        self
    }

    pub fn build(self) -> Result<BridgeConfig> {
        self.config.validate()?;
        Ok(self.config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_validation() {
        let config = BridgeConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_fee_distribution_validation() {
        let mut config = BridgeConfig::default();
        config.token_config.fee_distribution.l2_validators = 50; // Total = 110%
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_builder() {
        let config = BridgeConfig::builder()
            .ethereum_rpc("https://eth.example.com")
            .ghostchain_rpc("https://ghost.example.com")
            .target_tps(75000)
            .enable_guardian_auth(false)
            .build()
            .unwrap();

        assert_eq!(config.l2_config.target_tps, 75000);
        assert!(!config.enable_guardian_auth);
    }

    #[test]
    fn test_network_configs() {
        let config = BridgeConfig::default();

        assert!(config.networks.contains_key(&ChainId::ETHEREUM));
        assert!(config.networks.contains_key(&ChainId::GHOSTCHAIN));
        assert!(config.networks.contains_key(&ChainId::GHOSTPLANE));
        assert!(config.networks.contains_key(&ChainId(137))); // Polygon
        assert!(config.networks.contains_key(&ChainId(42161))); // Arbitrum
    }

    #[test]
    fn test_token_settings() {
        let config = BridgeConfig::default();

        assert!(config.token_config.gcc.is_deflationary);
        assert!(!config.token_config.spirit.is_deflationary);
        assert_eq!(config.token_config.ghost.decimals, 0); // NFT-like
    }
}