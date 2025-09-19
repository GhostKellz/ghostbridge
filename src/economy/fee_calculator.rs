/*!
Fee calculation engine for 4-token economy

Advanced fee calculation taking into account gas prices, priority fees,
cross-chain operations, and token economics.
*/

use crate::error::{BridgeError, Result};
use crate::types::{TokenType, TokenAmount, U256, MultiTokenFee};
use tracing::{debug, instrument};

/// Fee calculation engine
pub struct FeeCalculator {
    base_rates: BaseRates,
}

/// Base rate configuration for each token
#[derive(Debug, Clone)]
struct BaseRates {
    gcc_base_rate: u64,      // Wei per gas unit
    spirit_base_rate: u64,   // For governance operations
    mana_base_rate: u64,     // For smart contract execution
    ghost_base_rate: u64,    // For identity operations
}

impl Default for BaseRates {
    fn default() -> Self {
        Self {
            gcc_base_rate: 1_000_000_000,      // 1 gwei
            spirit_base_rate: 500_000_000,      // 0.5 gwei
            mana_base_rate: 2_000_000_000,      // 2 gwei
            ghost_base_rate: 1,                 // 1 wei (fixed)
        }
    }
}

impl FeeCalculator {
    pub async fn new() -> Result<Self> {
        Ok(Self {
            base_rates: BaseRates::default(),
        })
    }

    #[instrument(skip(self))]
    pub async fn calculate_dynamic_fees(
        &self,
        gas_used: u64,
        network_congestion: f64,
        token_demand: f64,
    ) -> Result<MultiTokenFee> {
        debug!("Calculating dynamic fees for {} gas units", gas_used);

        // Apply congestion multiplier (1.0 to 10.0)
        let congestion_multiplier = 1.0 + (network_congestion * 9.0);
        
        // Apply token demand multiplier (0.5 to 2.0)
        let demand_multiplier = 0.5 + (token_demand * 1.5);
        
        let total_multiplier = congestion_multiplier * demand_multiplier;

        let gcc_fee = TokenAmount::new(
            TokenType::Gcc,
            U256::from((self.base_rates.gcc_base_rate as f64 * gas_used as f64 * total_multiplier) as u64),
        );

        let spirit_fee = TokenAmount::new(
            TokenType::Spirit,
            U256::from((self.base_rates.spirit_base_rate as f64 * gas_used as f64 * demand_multiplier) as u64),
        );

        let mana_fee = TokenAmount::new(
            TokenType::Mana,
            U256::from((self.base_rates.mana_base_rate as f64 * gas_used as f64 * total_multiplier) as u64),
        );

        let ghost_fee = TokenAmount::new(
            TokenType::Ghost,
            U256::from(if gas_used > 100_000 { 1 } else { 0 }), // 1 GHOST for complex operations
        );

        Ok(MultiTokenFee {
            gcc_fee,
            spirit_fee,
            mana_fee,
            ghost_fee,
        })
    }

    pub async fn is_healthy(&self) -> bool {
        true // TODO: Implement health checks
    }
}