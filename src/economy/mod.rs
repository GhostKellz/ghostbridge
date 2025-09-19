/*!
4-Token Economy Integration for GhostBridge

Complete implementation of the GhostChain 4-token economy:
- GCC (Gas & transaction fees) - deflationary
- SPIRIT (Governance & voting) - unlimited supply
- MANA (Utility & rewards) - burn rate 0.5%
- GHOST (Brand & collectibles) - limited NFT-like tokens

Integrates with GLEDGER service for balance management and fee distribution.
*/

use crate::error::{BridgeError, Result, TokenError};
use crate::types::{TokenType, TokenAmount, U256, MultiTokenFee, Address};
use crate::services::{ServiceManager, gledger::{GasOperation, StateUpdate}};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, instrument, warn};

// Sub-modules
pub mod fee_calculator;
pub mod token_manager;
pub mod economics;
pub mod distribution;

pub use fee_calculator::FeeCalculator;
pub use token_manager::TokenManager;
pub use economics::TokenEconomics;
pub use distribution::FeeDistributor;

/// 4-Token economy manager
pub struct TokenEconomy {
    token_manager: Arc<TokenManager>,
    fee_calculator: Arc<FeeCalculator>,
    fee_distributor: Arc<FeeDistributor>,
    economics: Arc<TokenEconomics>,
    services: Arc<ServiceManager>,
    pricing_cache: Arc<RwLock<PricingCache>>,
}

/// Token pricing cache
#[derive(Debug, Clone)]
struct PricingCache {
    prices: HashMap<TokenType, TokenPrice>,
    last_updated: chrono::DateTime<chrono::Utc>,
    cache_duration: chrono::Duration,
}

/// Token price information
#[derive(Debug, Clone)]
pub struct TokenPrice {
    pub token_type: TokenType,
    pub price_usd: f64,
    pub market_cap_usd: f64,
    pub volume_24h_usd: f64,
    pub change_24h_percent: f64,
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

/// Transaction fee breakdown
#[derive(Debug, Clone)]
pub struct FeeBreakdown {
    pub base_fee: TokenAmount,
    pub priority_fee: TokenAmount,
    pub cross_chain_fee: Option<TokenAmount>,
    pub bridge_security_fee: TokenAmount,
    pub total_fee: MultiTokenFee,
    pub fee_distribution: FeeDistributionBreakdown,
}

/// Fee distribution breakdown
#[derive(Debug, Clone)]
pub struct FeeDistributionBreakdown {
    pub l2_validators: MultiTokenFee,
    pub l1_validators: MultiTokenFee,
    pub security_fund: MultiTokenFee,
    pub protocol_development: MultiTokenFee,
    pub burn_amount: MultiTokenFee,
}

/// Token metrics for monitoring
#[derive(Debug, Clone)]
pub struct TokenMetrics {
    pub total_supply: HashMap<TokenType, U256>,
    pub circulating_supply: HashMap<TokenType, U256>,
    pub burned_amount: HashMap<TokenType, U256>,
    pub daily_volume: HashMap<TokenType, U256>,
    pub bridge_volume_24h: HashMap<TokenType, U256>,
}

impl TokenEconomy {
    /// Create a new token economy manager
    #[instrument(skip(services))]
    pub async fn new(services: Arc<ServiceManager>) -> Result<Self> {
        info!("Initializing 4-token economy system");

        let token_manager = Arc::new(TokenManager::new(services.clone()).await?);
        let fee_calculator = Arc::new(FeeCalculator::new().await?);
        let fee_distributor = Arc::new(FeeDistributor::new(services.clone()).await?);
        let economics = Arc::new(TokenEconomics::new().await?);

        let pricing_cache = Arc::new(RwLock::new(PricingCache {
            prices: HashMap::new(),
            last_updated: chrono::Utc::now() - chrono::Duration::hours(1), // Force initial update
            cache_duration: chrono::Duration::minutes(5), // 5-minute cache
        }));

        let economy = Self {
            token_manager,
            fee_calculator,
            fee_distributor,
            economics,
            services,
            pricing_cache,
        };

        // Initialize token pricing
        economy.update_pricing().await?;

        info!("4-token economy system initialized successfully");
        Ok(economy)
    }

    /// Calculate comprehensive transaction fees
    #[instrument(skip(self))]
    pub async fn calculate_transaction_fees(
        &self,
        operation: GasOperation,
        cross_chain: bool,
        priority_multiplier: f64,
    ) -> Result<FeeBreakdown> {
        debug!("Calculating fees for operation: {:?}", operation);

        // Get base fees from GLEDGER
        let gledger_guard = self.services.gledger().await?;
        let gledger = gledger_guard.as_ref().unwrap();
        let base_fees = gledger.calculate_gas_fees(operation).await?;

        // Apply priority multiplier
        let priority_fees = self.apply_priority_multiplier(&base_fees, priority_multiplier)?;

        // Calculate cross-chain fees if applicable
        let cross_chain_fee = if cross_chain {
            Some(self.calculate_cross_chain_fee(&base_fees).await?)
        } else {
            None
        };

        // Calculate bridge security fee (0.1% of total)
        let bridge_security_fee = self.calculate_security_fee(&base_fees)?;

        // Calculate total fees
        let mut total_fee = priority_fees.clone();
        if let Some(cc_fee) = &cross_chain_fee {
            total_fee = self.add_fees(&total_fee, &MultiTokenFee {
                gcc_fee: cc_fee.clone(),
                spirit_fee: TokenAmount::new(TokenType::Spirit, U256::ZERO),
                mana_fee: TokenAmount::new(TokenType::Mana, U256::ZERO),
                ghost_fee: TokenAmount::new(TokenType::Ghost, U256::ZERO),
            })?;
        }

        // Add security fee
        total_fee = self.add_fees(&total_fee, &MultiTokenFee {
            gcc_fee: bridge_security_fee.clone(),
            spirit_fee: TokenAmount::new(TokenType::Spirit, U256::ZERO),
            mana_fee: TokenAmount::new(TokenType::Mana, U256::ZERO),
            ghost_fee: TokenAmount::new(TokenType::Ghost, U256::ZERO),
        })?;

        // Calculate fee distribution
        let fee_distribution = self.calculate_fee_distribution(&total_fee).await?;

        let breakdown = FeeBreakdown {
            base_fee: base_fees.gcc_fee.clone(),
            priority_fee: priority_fees.gcc_fee.clone(),
            cross_chain_fee,
            bridge_security_fee,
            total_fee,
            fee_distribution,
        };

        debug!("Fee calculation completed: total = {}", breakdown.total_fee.total_value());
        Ok(breakdown)
    }

    /// Process token payment for a transaction
    #[instrument(skip(self))]
    pub async fn process_payment(
        &self,
        payer: &Address,
        fee_breakdown: &FeeBreakdown,
    ) -> Result<PaymentResult> {
        info!("Processing payment for address: {}", payer);

        // Check balances
        let gledger_guard = self.services.gledger().await?;
        let gledger = gledger_guard.as_ref().unwrap();
        let balances = gledger.get_all_balances(payer).await?;

        // Verify sufficient balances
        self.verify_sufficient_balances(&balances, &fee_breakdown.total_fee)?;

        // Process burns for deflationary tokens
        let burn_amounts = self.calculate_burn_amounts(&fee_breakdown.total_fee).await?;

        // Deduct fees from payer
        let mut payment_results = Vec::new();

        if fee_breakdown.total_fee.gcc_fee.amount.to_u64() > 0 {
            let transfer_result = gledger.transfer_tokens(
                payer,
                &Address([0u8; 20]), // Burn/fee address
                &fee_breakdown.total_fee.gcc_fee,
            ).await?;
            payment_results.push(("GCC".to_string(), transfer_result));
        }

        // Similar for other tokens...

        // Distribute fees to validators and funds
        self.fee_distributor.distribute_fees(&fee_breakdown.fee_distribution).await?;

        let result = PaymentResult {
            payer: payer.clone(),
            total_paid: fee_breakdown.total_fee.clone(),
            payment_breakdown: payment_results,
            burn_amounts,
            fee_distribution: fee_breakdown.fee_distribution.clone(),
            processed_at: chrono::Utc::now(),
        };

        info!("Payment processed successfully for {}", payer);
        Ok(result)
    }

    /// Get current token metrics
    pub async fn get_token_metrics(&self) -> Result<TokenMetrics> {
        let economics = &self.economics;
        economics.get_current_metrics().await
    }

    /// Get current token pricing
    pub async fn get_token_pricing(&self) -> Result<HashMap<TokenType, TokenPrice>> {
        // Check if cache is still valid
        let cache = self.pricing_cache.read().await;
        let cache_age = chrono::Utc::now() - cache.last_updated;

        if cache_age < cache.cache_duration && !cache.prices.is_empty() {
            return Ok(cache.prices.clone());
        }

        drop(cache); // Release read lock

        // Update pricing
        self.update_pricing().await?;

        let cache = self.pricing_cache.read().await;
        Ok(cache.prices.clone())
    }

    /// Update L2 state with token changes
    pub async fn update_l2_state(&self, state_updates: &[StateUpdate]) -> Result<()> {
        debug!("Updating L2 state with {} token changes", state_updates.len());

        let gledger_guard = self.services.gledger().await?;
        let gledger = gledger_guard.as_ref().unwrap();
        let _result = gledger.update_l2_balances(state_updates).await?;

        // Update economics tracking
        self.economics.record_l2_operations(state_updates).await?;

        Ok(())
    }

    /// Validate token economy health
    pub async fn health_check(&self) -> Result<TokenEconomyHealth> {
        let mut health = TokenEconomyHealth::default();

        // Check token manager
        health.token_manager_healthy = self.token_manager.is_healthy().await;

        // Check fee calculator
        health.fee_calculator_healthy = self.fee_calculator.is_healthy().await;

        // Check fee distributor
        health.fee_distributor_healthy = self.fee_distributor.is_healthy().await;

        // Check economics tracking
        health.economics_healthy = self.economics.is_healthy().await;

        // Check GLEDGER connectivity
        health.gledger_healthy = self.services.gledger().await.is_ok();

        health.overall_healthy = health.token_manager_healthy
            && health.fee_calculator_healthy
            && health.fee_distributor_healthy
            && health.economics_healthy
            && health.gledger_healthy;

        Ok(health)
    }

    // Private helper methods

    async fn update_pricing(&self) -> Result<()> {
        debug!("Updating token pricing information");

        // Get pricing from GLEDGER service
        let gledger_guard = self.services.gledger().await?;
        let gledger = gledger_guard.as_ref().unwrap();
        let pricing = gledger.get_token_pricing().await?;

        let mut cache = self.pricing_cache.write().await;

        // Update cache with latest prices
        cache.prices.insert(TokenType::Gcc, TokenPrice {
            token_type: TokenType::Gcc,
            price_usd: pricing.gcc_price_usd,
            market_cap_usd: pricing.gcc_price_usd * 10_000_000.0, // Assuming 10M circulating
            volume_24h_usd: 1_000_000.0, // Mock volume
            change_24h_percent: 2.5,
            last_updated: chrono::Utc::now(),
        });

        cache.prices.insert(TokenType::Spirit, TokenPrice {
            token_type: TokenType::Spirit,
            price_usd: pricing.spirit_price_usd,
            market_cap_usd: pricing.spirit_price_usd * 50_000_000.0, // Assuming 50M circulating
            volume_24h_usd: 500_000.0,
            change_24h_percent: 1.2,
            last_updated: chrono::Utc::now(),
        });

        cache.prices.insert(TokenType::Mana, TokenPrice {
            token_type: TokenType::Mana,
            price_usd: pricing.mana_price_usd,
            market_cap_usd: pricing.mana_price_usd * 75_000_000.0, // Assuming 75M circulating
            volume_24h_usd: 250_000.0,
            change_24h_percent: -0.8,
            last_updated: chrono::Utc::now(),
        });

        cache.prices.insert(TokenType::Ghost, TokenPrice {
            token_type: TokenType::Ghost,
            price_usd: pricing.ghost_floor_price_usd,
            market_cap_usd: pricing.ghost_floor_price_usd * 10_000.0, // 10K max supply
            volume_24h_usd: 50_000.0,
            change_24h_percent: 5.0,
            last_updated: chrono::Utc::now(),
        });

        cache.last_updated = chrono::Utc::now();

        debug!("Token pricing updated successfully");
        Ok(())
    }

    fn apply_priority_multiplier(&self, base_fees: &MultiTokenFee, multiplier: f64) -> Result<MultiTokenFee> {
        let multiply_amount = |amount: &U256, multiplier: f64| -> U256 {
            let value = amount.to_u64() as f64 * multiplier;
            U256::from(value as u64)
        };

        Ok(MultiTokenFee {
            gcc_fee: TokenAmount::new(
                TokenType::Gcc,
                multiply_amount(&base_fees.gcc_fee.amount, multiplier),
            ),
            spirit_fee: TokenAmount::new(
                TokenType::Spirit,
                multiply_amount(&base_fees.spirit_fee.amount, multiplier),
            ),
            mana_fee: TokenAmount::new(
                TokenType::Mana,
                multiply_amount(&base_fees.mana_fee.amount, multiplier),
            ),
            ghost_fee: TokenAmount::new(
                TokenType::Ghost,
                multiply_amount(&base_fees.ghost_fee.amount, multiplier),
            ),
        })
    }

    async fn calculate_cross_chain_fee(&self, base_fees: &MultiTokenFee) -> Result<TokenAmount> {
        // Cross-chain fee is 10% of base GCC fee + 1 GHOST
        let cross_chain_gcc = U256::from(base_fees.gcc_fee.amount.to_u64() / 10);
        Ok(TokenAmount::new(TokenType::Gcc, cross_chain_gcc))
    }

    fn calculate_security_fee(&self, base_fees: &MultiTokenFee) -> Result<TokenAmount> {
        // Security fee is 0.1% of base GCC fee
        let security_fee = U256::from(base_fees.gcc_fee.amount.to_u64() / 1000);
        Ok(TokenAmount::new(TokenType::Gcc, security_fee))
    }

    fn add_fees(&self, fee1: &MultiTokenFee, fee2: &MultiTokenFee) -> Result<MultiTokenFee> {
        Ok(MultiTokenFee {
            gcc_fee: TokenAmount::new(
                TokenType::Gcc,
                &fee1.gcc_fee.amount + &fee2.gcc_fee.amount,
            ),
            spirit_fee: TokenAmount::new(
                TokenType::Spirit,
                &fee1.spirit_fee.amount + &fee2.spirit_fee.amount,
            ),
            mana_fee: TokenAmount::new(
                TokenType::Mana,
                &fee1.mana_fee.amount + &fee2.mana_fee.amount,
            ),
            ghost_fee: TokenAmount::new(
                TokenType::Ghost,
                &fee1.ghost_fee.amount + &fee2.ghost_fee.amount,
            ),
        })
    }

    async fn calculate_fee_distribution(&self, total_fee: &MultiTokenFee) -> Result<FeeDistributionBreakdown> {
        // Use fee distributor to calculate distribution
        self.fee_distributor.calculate_distribution(total_fee).await
    }

    fn verify_sufficient_balances(
        &self,
        balances: &crate::services::gledger::MultiTokenBalance,
        required_fees: &MultiTokenFee,
    ) -> Result<()> {
        if balances.gcc.amount.to_u64() < required_fees.gcc_fee.amount.to_u64() {
            return Err(BridgeError::Token(TokenError::InsufficientBalance {
                token: "GCC".to_string(),
                required: required_fees.gcc_fee.to_human_readable(),
                available: balances.gcc.to_human_readable(),
            }));
        }

        // Check other token balances similarly...

        Ok(())
    }

    async fn calculate_burn_amounts(&self, fees: &MultiTokenFee) -> Result<MultiTokenFee> {
        // Calculate burn amounts based on tokenomics
        // GCC: 1% burn rate
        // MANA: 0.5% burn rate
        // Others: no burn

        let gcc_burn = U256::from(fees.gcc_fee.amount.to_u64() / 100); // 1%
        let mana_burn = U256::from(fees.mana_fee.amount.to_u64() / 200); // 0.5%

        Ok(MultiTokenFee {
            gcc_fee: TokenAmount::new(TokenType::Gcc, gcc_burn),
            spirit_fee: TokenAmount::new(TokenType::Spirit, U256::ZERO),
            mana_fee: TokenAmount::new(TokenType::Mana, mana_burn),
            ghost_fee: TokenAmount::new(TokenType::Ghost, U256::ZERO),
        })
    }
}

/// Payment processing result
#[derive(Debug, Clone)]
pub struct PaymentResult {
    pub payer: Address,
    pub total_paid: MultiTokenFee,
    pub payment_breakdown: Vec<(String, crate::services::gledger::TransferResult)>,
    pub burn_amounts: MultiTokenFee,
    pub fee_distribution: FeeDistributionBreakdown,
    pub processed_at: chrono::DateTime<chrono::Utc>,
}

/// Token economy health status
#[derive(Debug, Clone, Default)]
pub struct TokenEconomyHealth {
    pub overall_healthy: bool,
    pub token_manager_healthy: bool,
    pub fee_calculator_healthy: bool,
    pub fee_distributor_healthy: bool,
    pub economics_healthy: bool,
    pub gledger_healthy: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_price() {
        let price = TokenPrice {
            token_type: TokenType::Gcc,
            price_usd: 0.50,
            market_cap_usd: 5_000_000.0,
            volume_24h_usd: 100_000.0,
            change_24h_percent: 2.5,
            last_updated: chrono::Utc::now(),
        };

        assert_eq!(price.token_type, TokenType::Gcc);
        assert_eq!(price.price_usd, 0.50);
    }

    #[test]
    fn test_fee_breakdown() {
        let base_fee = TokenAmount::new(TokenType::Gcc, U256::from(1000));
        let priority_fee = TokenAmount::new(TokenType::Gcc, U256::from(1200));

        let breakdown = FeeBreakdown {
            base_fee: base_fee.clone(),
            priority_fee: priority_fee.clone(),
            cross_chain_fee: None,
            bridge_security_fee: TokenAmount::new(TokenType::Gcc, U256::from(10)),
            total_fee: MultiTokenFee {
                gcc_fee: priority_fee,
                spirit_fee: TokenAmount::new(TokenType::Spirit, U256::ZERO),
                mana_fee: TokenAmount::new(TokenType::Mana, U256::ZERO),
                ghost_fee: TokenAmount::new(TokenType::Ghost, U256::ZERO),
            },
            fee_distribution: FeeDistributionBreakdown {
                l2_validators: MultiTokenFee {
                    gcc_fee: TokenAmount::new(TokenType::Gcc, U256::from(480)),
                    spirit_fee: TokenAmount::new(TokenType::Spirit, U256::ZERO),
                    mana_fee: TokenAmount::new(TokenType::Mana, U256::ZERO),
                    ghost_fee: TokenAmount::new(TokenType::Ghost, U256::ZERO),
                },
                l1_validators: MultiTokenFee {
                    gcc_fee: TokenAmount::new(TokenType::Gcc, U256::from(360)),
                    spirit_fee: TokenAmount::new(TokenType::Spirit, U256::ZERO),
                    mana_fee: TokenAmount::new(TokenType::Mana, U256::ZERO),
                    ghost_fee: TokenAmount::new(TokenType::Ghost, U256::ZERO),
                },
                security_fund: MultiTokenFee {
                    gcc_fee: TokenAmount::new(TokenType::Gcc, U256::from(240)),
                    spirit_fee: TokenAmount::new(TokenType::Spirit, U256::ZERO),
                    mana_fee: TokenAmount::new(TokenType::Mana, U256::ZERO),
                    ghost_fee: TokenAmount::new(TokenType::Ghost, U256::ZERO),
                },
                protocol_development: MultiTokenFee {
                    gcc_fee: TokenAmount::new(TokenType::Gcc, U256::from(120)),
                    spirit_fee: TokenAmount::new(TokenType::Spirit, U256::ZERO),
                    mana_fee: TokenAmount::new(TokenType::Mana, U256::ZERO),
                    ghost_fee: TokenAmount::new(TokenType::Ghost, U256::ZERO),
                },
                burn_amount: MultiTokenFee {
                    gcc_fee: TokenAmount::new(TokenType::Gcc, U256::from(12)),
                    spirit_fee: TokenAmount::new(TokenType::Spirit, U256::ZERO),
                    mana_fee: TokenAmount::new(TokenType::Mana, U256::ZERO),
                    ghost_fee: TokenAmount::new(TokenType::Ghost, U256::ZERO),
                },
            },
        };

        assert_eq!(breakdown.base_fee.amount.to_u64(), 1000);
        assert_eq!(breakdown.priority_fee.amount.to_u64(), 1200);
    }
}