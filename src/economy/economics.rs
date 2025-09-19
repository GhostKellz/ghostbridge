/*!
Token economics tracking and analysis

Tracks token metrics, supply changes, transaction volumes, and economic health.
*/

use crate::error::{BridgeError, Result};
use crate::types::{TokenType, U256};
use crate::economy::TokenMetrics;
use crate::services::gledger::StateUpdate;
use std::collections::HashMap;
use tracing::{debug, instrument};

/// Token economics analyzer
pub struct TokenEconomics {
    metrics: TokenMetrics,
    daily_volumes: HashMap<TokenType, U256>,
}

impl TokenEconomics {
    pub async fn new() -> Result<Self> {
        let mut total_supply = HashMap::new();
        let mut circulating_supply = HashMap::new();
        let mut burned_amount = HashMap::new();
        let mut daily_volume = HashMap::new();
        let mut bridge_volume_24h = HashMap::new();

        // Initialize with current state
        for token_type in [TokenType::Gcc, TokenType::Spirit, TokenType::Mana, TokenType::Ghost] {
            total_supply.insert(token_type, U256::from(match token_type {
                TokenType::Gcc => 21_000_000 * 10u64.pow(18),
                TokenType::Spirit => 100_000_000 * 10u64.pow(18),
                TokenType::Mana => 100_000_000 * 10u64.pow(18), 
                TokenType::Ghost => 10_000,
            }));
            
            circulating_supply.insert(token_type, total_supply[&token_type].clone());
            burned_amount.insert(token_type, U256::ZERO);
            daily_volume.insert(token_type, U256::ZERO);
            bridge_volume_24h.insert(token_type, U256::ZERO);
        }

        Ok(Self {
            metrics: TokenMetrics {
                total_supply,
                circulating_supply,
                burned_amount,
                daily_volume,
                bridge_volume_24h,
            },
            daily_volumes: HashMap::new(),
        })
    }

    #[instrument(skip(self))]
    pub async fn get_current_metrics(&self) -> Result<TokenMetrics> {
        debug!("Retrieving current token metrics");
        Ok(self.metrics.clone())
    }

    #[instrument(skip(self))]
    pub async fn record_l2_operations(&self, state_updates: &[StateUpdate]) -> Result<()> {
        debug!("Recording {} L2 operations", state_updates.len());
        
        // TODO: Update metrics based on L2 state changes
        // This would track:
        // - Token transfers
        // - Burns from fee payments
        // - Bridge volume
        // - Daily transaction volumes

        Ok(())
    }

    pub fn calculate_deflation_rate(&self, token_type: TokenType) -> f64 {
        let total = self.metrics.total_supply.get(&token_type)
            .unwrap_or(&U256::ZERO).to_u64() as f64;
        let burned = self.metrics.burned_amount.get(&token_type)
            .unwrap_or(&U256::ZERO).to_u64() as f64;
        
        if total > 0.0 {
            (burned / total) * 100.0
        } else {
            0.0
        }
    }

    pub fn get_token_velocity(&self, token_type: TokenType) -> f64 {
        let circulating = self.metrics.circulating_supply.get(&token_type)
            .unwrap_or(&U256::ZERO).to_u64() as f64;
        let daily_vol = self.metrics.daily_volume.get(&token_type)
            .unwrap_or(&U256::ZERO).to_u64() as f64;
        
        if circulating > 0.0 {
            daily_vol / circulating
        } else {
            0.0
        }
    }

    pub async fn is_healthy(&self) -> bool {
        // Check economic health indicators
        for token_type in [TokenType::Gcc, TokenType::Spirit, TokenType::Mana, TokenType::Ghost] {
            let velocity = self.get_token_velocity(token_type);
            if velocity > 10.0 {
                // Unusually high velocity might indicate problems
                return false;
            }
            
            let deflation = self.calculate_deflation_rate(token_type);
            if deflation > 50.0 {
                // Too much deflation
                return false;
            }
        }
        true
    }
}