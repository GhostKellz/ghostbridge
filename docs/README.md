# GhostBridge Documentation

## Overview

GhostBridge is a high-performance cross-chain bridge infrastructure for the GhostChain ecosystem, designed to achieve 50,000+ TPS with zero-trust security and seamless Rust-Zig integration.

## Architecture

```
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
- **Multi-Chain Support**: Ethereum, Bitcoin, Polygon, Arbitrum, GhostChain, GhostPlane
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

## Documentation Index

- [Architecture](./architecture.md) - System architecture and design
- [Installation](./installation.md) - Setup and deployment guide
- [API Reference](./api.md) - Complete API documentation
- [Security](./security.md) - Guardian Framework and zero-trust
- [Economy](./economy.md) - 4-token economy and tokenomics
- [L2 Settlement](./l2-settlement.md) - High-performance settlement engine
- [Transport](./transport.md) - GQUIC networking layer
- [Services](./services.md) - GhostChain service integration
- [FFI](./ffi.md) - Rust-Zig communication layer
- [Examples](./examples/) - Usage examples and tutorials

## Development

- [Contributing](./contributing.md) - How to contribute
- [Testing](./testing.md) - Test strategy and guidelines
- [Benchmarks](./benchmarks.md) - Performance benchmarking
- [Debugging](./debugging.md) - Debugging and troubleshooting