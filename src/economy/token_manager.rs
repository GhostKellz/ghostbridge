/*!
Token management for 4-token economy

Manages token supplies, burns, minting, and cross-chain token tracking.
*/

use crate::error::{BridgeError, Result};
use crate::types::{TokenType, TokenAmount, U256, Address};
use crate::services::ServiceManager;
use std::sync::Arc;
use tracing::{debug, instrument};

/// Token manager for supply and burn operations
pub struct TokenManager {
    services: Arc<ServiceManager>,
    supply_tracking: SupplyTracker,
}

/// Supply tracking for all tokens
#[derive(Debug, Clone)]
struct SupplyTracker {
    total_supplies: std::collections::HashMap<TokenType, U256>,
    burned_amounts: std::collections::HashMap<TokenType, U256>,
}

impl TokenManager {
    pub async fn new(services: Arc<ServiceManager>) -> Result<Self> {
        let mut total_supplies = std::collections::HashMap::new();
        let mut burned_amounts = std::collections::HashMap::new();

        // Initialize with current supplies
        total_supplies.insert(TokenType::Gcc, U256::from(21_000_000 * 10u64.pow(18))); // 21M GCC
        total_supplies.insert(TokenType::Spirit, U256::from(100_000_000 * 10u64.pow(18))); // 100M SPIRIT
        total_supplies.insert(TokenType::Mana, U256::from(100_000_000 * 10u64.pow(18))); // 100M MANA
        total_supplies.insert(TokenType::Ghost, U256::from(10_000)); // 10K GHOST

        // Initialize burn tracking
        for token_type in [TokenType::Gcc, TokenType::Spirit, TokenType::Mana, TokenType::Ghost] {
            burned_amounts.insert(token_type, U256::ZERO);
        }

        Ok(Self {
            services,
            supply_tracking: SupplyTracker {
                total_supplies,
                burned_amounts,
            },
        })
    }

    #[instrument(skip(self))]
    pub async fn burn_tokens(&mut self, token_type: TokenType, amount: U256) -> Result<()> {
        debug!("Burning {} tokens of type {}", amount.to_u64(), token_type);

        // Update burned amount tracking
        let current_burned = self.supply_tracking.burned_amounts.get(&token_type)
            .unwrap_or(&U256::ZERO);
        let new_burned = current_burned + &amount;
        self.supply_tracking.burned_amounts.insert(token_type, new_burned);

        // For deflationary tokens, reduce total supply
        match token_type {
            TokenType::Gcc | TokenType::Mana => {
                let current_supply = self.supply_tracking.total_supplies.get(&token_type)
                    .unwrap_or(&U256::ZERO);
                let new_supply = current_supply - &amount;
                self.supply_tracking.total_supplies.insert(token_type, new_supply);
            }
            _ => {
                // Non-deflationary tokens don't reduce total supply
            }
        }

        debug!("Burned {} {} tokens successfully", amount.to_u64(), token_type);
        Ok(())
    }

    pub fn get_total_supply(&self, token_type: TokenType) -> U256 {
        self.supply_tracking.total_supplies.get(&token_type)
            .unwrap_or(&U256::ZERO)
            .clone()
    }

    pub fn get_burned_amount(&self, token_type: TokenType) -> U256 {
        self.supply_tracking.burned_amounts.get(&token_type)
            .unwrap_or(&U256::ZERO)
            .clone()
    }

    pub fn get_circulating_supply(&self, token_type: TokenType) -> U256 {
        let total = self.get_total_supply(token_type);
        let burned = self.get_burned_amount(token_type);
        &total - &burned
    }

    pub async fn is_healthy(&self) -> bool {
        // Check that supplies are reasonable
        for token_type in [TokenType::Gcc, TokenType::Spirit, TokenType::Mana, TokenType::Ghost] {
            let supply = self.get_total_supply(token_type);
            if supply == U256::ZERO {
                return false;
            }
        }
        true
    }
}