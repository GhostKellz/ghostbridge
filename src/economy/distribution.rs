/*!
Fee distribution system for 4-token economy

Distributes fees according to tokenomics:
- 40% to L2 validators
- 30% to L1 validators  
- 20% to security fund
- 10% to protocol development
*/

use crate::error::{BridgeError, Result};
use crate::types::{TokenType, TokenAmount, U256, MultiTokenFee};
use crate::services::ServiceManager;
use crate::economy::FeeDistributionBreakdown;
use std::sync::Arc;
use tracing::{debug, instrument};

/// Fee distribution manager
pub struct FeeDistributor {
    services: Arc<ServiceManager>,
    distribution_config: DistributionConfig,
}

/// Distribution configuration
#[derive(Debug, Clone)]
struct DistributionConfig {
    l2_validators_percent: u8,
    l1_validators_percent: u8, 
    security_fund_percent: u8,
    protocol_development_percent: u8,
}

impl Default for DistributionConfig {
    fn default() -> Self {
        Self {
            l2_validators_percent: 40,
            l1_validators_percent: 30,
            security_fund_percent: 20,
            protocol_development_percent: 10,
        }
    }
}

impl FeeDistributor {
    pub async fn new(services: Arc<ServiceManager>) -> Result<Self> {
        Ok(Self {
            services,
            distribution_config: DistributionConfig::default(),
        })
    }

    #[instrument(skip(self))]
    pub async fn calculate_distribution(
        &self,
        total_fee: &MultiTokenFee,
    ) -> Result<FeeDistributionBreakdown> {
        debug!("Calculating fee distribution for total fee: {}", total_fee.total_value());

        let l2_validators = self.calculate_portion(total_fee, self.distribution_config.l2_validators_percent)?;
        let l1_validators = self.calculate_portion(total_fee, self.distribution_config.l1_validators_percent)?;
        let security_fund = self.calculate_portion(total_fee, self.distribution_config.security_fund_percent)?;
        let protocol_development = self.calculate_portion(total_fee, self.distribution_config.protocol_development_percent)?;
        
        // Calculate burn amounts (1% of GCC, 0.5% of MANA)
        let burn_amount = MultiTokenFee {
            gcc_fee: TokenAmount::new(
                TokenType::Gcc,
                U256::from(total_fee.gcc_fee.amount.to_u64() / 100), // 1%
            ),
            spirit_fee: TokenAmount::new(TokenType::Spirit, U256::ZERO),
            mana_fee: TokenAmount::new(
                TokenType::Mana,
                U256::from(total_fee.mana_fee.amount.to_u64() / 200), // 0.5%
            ),
            ghost_fee: TokenAmount::new(TokenType::Ghost, U256::ZERO),
        };

        Ok(FeeDistributionBreakdown {
            l2_validators,
            l1_validators,
            security_fund,
            protocol_development,
            burn_amount,
        })
    }

    #[instrument(skip(self))]
    pub async fn distribute_fees(
        &self,
        distribution: &FeeDistributionBreakdown,
    ) -> Result<()> {
        debug!("Distributing fees to validators and funds");

        // TODO: Implement actual distribution to validator addresses
        // This would involve:
        // 1. Get validator addresses from GHOSTD
        // 2. Calculate individual validator rewards
        // 3. Transfer tokens via GLEDGER
        // 4. Update security fund balances
        // 5. Update protocol development fund

        debug!("Fee distribution completed");
        Ok(())
    }

    pub async fn is_healthy(&self) -> bool {
        // Verify distribution percentages sum to 100%
        let total = self.distribution_config.l2_validators_percent +
                   self.distribution_config.l1_validators_percent +
                   self.distribution_config.security_fund_percent +
                   self.distribution_config.protocol_development_percent;
        total == 100
    }

    fn calculate_portion(&self, total_fee: &MultiTokenFee, percentage: u8) -> Result<MultiTokenFee> {
        let multiplier = percentage as f64 / 100.0;

        Ok(MultiTokenFee {
            gcc_fee: TokenAmount::new(
                TokenType::Gcc,
                U256::from((total_fee.gcc_fee.amount.to_u64() as f64 * multiplier) as u64),
            ),
            spirit_fee: TokenAmount::new(
                TokenType::Spirit,
                U256::from((total_fee.spirit_fee.amount.to_u64() as f64 * multiplier) as u64),
            ),
            mana_fee: TokenAmount::new(
                TokenType::Mana,
                U256::from((total_fee.mana_fee.amount.to_u64() as f64 * multiplier) as u64),
            ),
            ghost_fee: TokenAmount::new(
                TokenType::Ghost,
                U256::from((total_fee.ghost_fee.amount.to_u64() as f64 * multiplier) as u64),
            ),
        })
    }
}