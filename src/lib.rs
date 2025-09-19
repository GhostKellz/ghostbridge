/*!
# GhostBridge - Cross-Chain Bridge Infrastructure

High-performance Rust-based bridge with safe FFI abstraction layer for GhostPlane (Zig)
integration, enabling seamless cross-chain communication and L2 settlement.

## Architecture

```text
┌─────────────────┐    Etherlink    ┌──────────────┐
│ GhostChain L1   │ <-------------> │ GhostPlane L2│
│ (Rust)          │   gRPC/QUIC     │ (Zig)        │
│ Services:       │                 │              │
│ - GHOSTD        │                 │              │
│ - WALLETD       │                 │              │
│ - GID           │                 │              │
│ - CNS           │                 │              │
│ - GLEDGER       │                 │              │
│ - GSIG          │                 │              │
└─────────────────┘                 └──────────────┘
         │
         ▼
┌─────────────────┐
│ GhostBridge     │
│ Cross-Chain     │
│ Communication   │
└─────────────────┘
```

## Features

- **Enhanced FFI Safety**: Memory-safe Rust ↔ Zig communication
- **Multi-Chain Support**: Ethereum, Bitcoin, Polygon, Arbitrum, and custom chains
- **GQUIC Transport**: High-performance networking with connection pooling
- **Guardian Framework**: Zero-trust security and privacy protection
- **4-Token Economy**: GCC, SPIRIT, MANA, GHOST token integration
- **L2 Settlement**: 50,000+ TPS target with optimistic rollups
- **Service Integration**: All 6 GhostChain services via etherlink

## Quick Start

```rust
use ghostbridge::{GhostBridge, BridgeConfig, init};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    init();

    // Configure bridge
    let config = BridgeConfig::builder()
        .ethereum_rpc("https://mainnet.infura.io/v3/YOUR_KEY")
        .ghostchain_rpc("https://rpc.ghostchain.io")
        .enable_l2_settlement(true)
        .build();

    // Create bridge instance
    let bridge = GhostBridge::new(config).await?;

    // Bridge a transaction
    let receipt = bridge.bridge_transaction(tx).await?;

    Ok(())
}
```
*/

#![deny(unsafe_op_in_unsafe_fn)]
#![warn(missing_docs, rust_2018_idioms)]

// Re-export key types and functions for convenience
pub use bridge::{GhostBridge, BridgeConfig, BridgeReceipt};
pub use error::{BridgeError, Result};
pub use types::*;

// Core modules
pub mod bridge;
pub mod ghostplane;
pub mod services;
pub mod types;
pub mod error;
pub mod config;
pub mod transport;
pub mod economy;

// Internal modules
mod ffi;
mod metrics;
mod settlement;
mod security;

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Initialize GhostBridge with default tracing configuration
pub fn init() {
    init_with_tracing("info")
}

/// Initialize GhostBridge with custom tracing filter
pub fn init_with_tracing(filter: &str) {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| filter.into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("GhostBridge initialized with tracing filter: {}", filter);
}

/// Get the library version
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Get the library name
pub fn name() -> &'static str {
    env!("CARGO_PKG_NAME")
}

/// Check if FFI features are enabled
pub fn has_ffi_support() -> bool {
    cfg!(feature = "ffi")
}

/// Check if metrics are enabled
pub fn has_metrics_support() -> bool {
    cfg!(feature = "metrics")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert!(!version().is_empty());
    }

    #[test]
    fn test_name() {
        assert_eq!(name(), "ghostbridge");
    }

    #[test]
    fn test_init() {
        // Should not panic
        init();
    }
}