# 🔗 GQUIC Integration Guide for GhostChain Ecosystem

This comprehensive guide demonstrates how to integrate **gquic** with modern blockchain projects in the GhostChain ecosystem, including Etherlink, RVM, GhostPlane, and the rebuilt GhostBridge.

## Table of Contents

- [Overview](#overview)
- [Etherlink Integration](#etherlink-integration)
- [RVM Plugin Integration](#rvm-plugin-integration)
- [GhostPlane Zig L2 Integration](#ghostplane-zig-l2-integration)
- [GhostBridge Cross-Chain Integration](#ghostbridge-cross-chain-integration)
- [Advanced Rust Integration](#advanced-rust-integration)
- [Zig FFI Patterns](#zig-ffi-patterns)
- [DNS over QUIC](#dns-over-quic)
- [Performance Optimization](#performance-optimization)
- [Production Deployment](#production-deployment)

## Overview

**gquic** provides high-performance networking infrastructure for the next-generation GhostChain ecosystem:

| Project | Language | Integration Method | Use Case |
|---------|----------|-------------------|----------|
| **Etherlink** | Rust | Native gRPC/QUIC client | Secure Rust-Zig bridge communication |
| **RVM** | Rust | Plugin system integration | VM networking and runtime hooks |
| **GhostPlane** | Zig + Rust | FFI bridge + native Rust | L2 execution engine with mesh networking |
| **GhostBridge** | Rust | Cross-chain QUIC transport | Ultra-fast cross-chain communication |
| **Legacy Projects** | Zig/C | FFI (C-compatible) | Backward compatibility |

## Etherlink Integration

**Etherlink** is a Rust-native bridge providing secure communication between Rust and Zig technologies with gRPC/QUIC transport.

### Setting Up Etherlink with GQUIC

Add to your `Cargo.toml`:

```toml
[dependencies]
gquic = { git = "https://github.com/ghostkellz/gquic", features = ["gcc-crypto", "rustls-tls"] }
etherlink = { git = "https://github.com/ghostkellz/etherlink" }
tokio = { version = "1.0", features = ["full"] }
tonic = "0.10"
prost = "0.12"
anyhow = "1.0"
tracing = "0.1"
```

### Etherlink gRPC/QUIC Client

```rust
// etherlink/src/bridge_client.rs
use gquic::prelude::*;
use tonic::{transport::Channel, Request, Response};
use anyhow::Result;

pub struct EtherlinkBridgeClient {
    quic_client: QuicClient,
    grpc_channel: Channel,
    connection_pool: ConnectionPool,
}

impl EtherlinkBridgeClient {
    pub async fn new(bridge_endpoint: &str) -> Result<Self> {
        // Configure QUIC with Etherlink-specific settings
        let quic_config = QuicClientConfig::builder()
            .server_name("etherlink.ghostchain.local".to_string())
            .with_alpn("grpc")
            .with_alpn("etherlink-bridge")
            .max_idle_timeout(60_000)
            .keep_alive_interval(20_000)
            .max_concurrent_streams(1000)
            .build();

        let client = QuicClient::new(quic_config)?;

        // Create gRPC channel over QUIC
        let channel = tonic::transport::Endpoint::from_shared(bridge_endpoint)?
            .connect_with_connector(QuicGrpcConnector::new(client.clone()))
            .await?;

        let pool = ConnectionPool::new(PoolConfig::builder()
            .max_connections_per_endpoint(50)
            .enable_multiplexing(true)
            .build());

        Ok(Self {
            quic_client: client,
            grpc_channel: channel,
            connection_pool: pool,
        })
    }

    pub async fn bridge_zig_call(
        &self,
        zig_function: &str,
        payload: &[u8],
    ) -> Result<Vec<u8>> {
        // Create secure bridge request
        let request = BridgeRequest {
            target: "ghostplane".to_string(),
            function: zig_function.to_string(),
            payload: payload.to_vec(),
            safety_checks: true,
            timeout_ms: 5000,
        };

        // Send over QUIC with memory safety guarantees
        let mut client = BridgeServiceClient::new(self.grpc_channel.clone());
        let response = client.execute_bridge_call(Request::new(request)).await?;

        Ok(response.into_inner().result)
    }
}
```

## RVM Plugin Integration

**RVM** (Rust Virtual Machine) is a stack-based bytecode execution engine with a modular plugin system that can leverage GQUIC for networking operations.

### Setting Up RVM with GQUIC Networking Plugin

Add to your `Cargo.toml`:

```toml
[dependencies]
gquic = { git = "https://github.com/ghostkellz/gquic", features = ["gcc-crypto", "hardware-accel"] }
rvm = { git = "https://github.com/ghostkellz/rvm" }
serde = { version = "1.0", features = ["derive"] }
bincode = "1.3"
tokio = { version = "1.0", features = ["full"] }
```

### RVM Network Plugin Implementation

```rust
// rvm-plugins/src/quic_network.rs
use gquic::prelude::*;
use rvm_core::{Plugin, VmContext, OpCode, Stack, Value};
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct QuicNetworkPlugin {
    client: QuicClient,
    connections: Arc<RwLock<HashMap<String, Connection>>>,
    gas_counter: Arc<AtomicU64>,
}

impl QuicNetworkPlugin {
    pub fn new() -> Result<Self> {
        let config = QuicClientConfig::builder()
            .server_name("rvm.ghostchain.local".to_string())
            .with_alpn("rvm-net")
            .max_idle_timeout(300_000) // 5 minutes for long-running VM operations
            .build();

        let client = QuicClient::new(config)?;

        Ok(Self {
            client,
            connections: Arc::new(RwLock::new(HashMap::new())),
            gas_counter: Arc::new(AtomicU64::new(0)),
        })
    }
}

#[async_trait::async_trait]
impl Plugin for QuicNetworkPlugin {
    fn name(&self) -> &str {
        "quic_network"
    }

    fn opcodes(&self) -> Vec<OpCode> {
        vec![
            OpCode::Custom(0x80), // NET_CONNECT
            OpCode::Custom(0x81), // NET_SEND
            OpCode::Custom(0x82), // NET_RECV
            OpCode::Custom(0x83), // NET_CLOSE
        ]
    }

    async fn execute_opcode(
        &self,
        opcode: OpCode,
        stack: &mut Stack,
        ctx: &mut VmContext,
    ) -> rvm_core::Result<()> {
        // Charge gas for network operations
        self.charge_gas(ctx, 1000)?;

        match opcode {
            OpCode::Custom(0x80) => self.net_connect(stack, ctx).await,
            OpCode::Custom(0x81) => self.net_send(stack, ctx).await,
            OpCode::Custom(0x82) => self.net_recv(stack, ctx).await,
            OpCode::Custom(0x83) => self.net_close(stack, ctx).await,
            _ => Err(rvm_core::Error::InvalidOpcode),
        }
    }
}

impl QuicNetworkPlugin {
    async fn net_connect(&self, stack: &mut Stack, ctx: &mut VmContext) -> rvm_core::Result<()> {
        let addr_bytes = stack.pop()?.as_bytes()?;
        let addr_str = String::from_utf8(addr_bytes)
            .map_err(|_| rvm_core::Error::InvalidAddress)?;

        let socket_addr: SocketAddr = addr_str.parse()
            .map_err(|_| rvm_core::Error::InvalidAddress)?;

        // Establish QUIC connection with gas limits
        let connection = self.client.connect(socket_addr).await
            .map_err(|_| rvm_core::Error::NetworkError)?;

        let conn_id = uuid::Uuid::new_v4().to_string();
        self.connections.write().await.insert(conn_id.clone(), connection);

        // Push connection ID onto stack
        stack.push(Value::Bytes(conn_id.into_bytes()))?;
        Ok(())
    }

    async fn net_send(&self, stack: &mut Stack, ctx: &mut VmContext) -> rvm_core::Result<()> {
        let data = stack.pop()?.as_bytes()?;
        let conn_id_bytes = stack.pop()?.as_bytes()?;
        let conn_id = String::from_utf8(conn_id_bytes)
            .map_err(|_| rvm_core::Error::InvalidConnectionId)?;

        let connections = self.connections.read().await;
        let connection = connections.get(&conn_id)
            .ok_or(rvm_core::Error::ConnectionNotFound)?;

        // Send data over QUIC with VM gas accounting
        let mut stream = connection.open_uni().await
            .map_err(|_| rvm_core::Error::NetworkError)?;

        stream.write_all(&data).await
            .map_err(|_| rvm_core::Error::NetworkError)?;
        stream.finish().await
            .map_err(|_| rvm_core::Error::NetworkError)?;

        // Charge gas proportional to data size
        self.charge_gas(ctx, data.len() as u64)?;

        stack.push(Value::Bool(true))?; // Success
        Ok(())
    }

    fn charge_gas(&self, ctx: &mut VmContext, amount: u64) -> rvm_core::Result<()> {
        let current_gas = self.gas_counter.load(Ordering::Relaxed);
        let new_gas = current_gas + amount;

        if new_gas > ctx.gas_limit() {
            return Err(rvm_core::Error::OutOfGas);
        }

        self.gas_counter.store(new_gas, Ordering::Relaxed);
        ctx.set_gas_used(new_gas);
        Ok(())
    }
}
```

## GhostPlane Zig L2 Integration

**GhostPlane** demonstrates high-performance Zig + Rust integration patterns for Layer 2 blockchain execution environments with mesh networking over QUIC.

### Generic FFI Bridge Pattern

This pattern works for any Zig project that needs QUIC networking capabilities:

### Building GQUIC with FFI Support

```bash
# Build with all features for maximum compatibility
cargo build --release --features "ffi,gcc-crypto,hardware-accel"

# For Zig projects specifically
cargo build --release --features "ffi,ring-crypto" --target-dir target/zig-ffi
```

This creates platform-specific libraries:
- Linux: `libgquic.so`
- macOS: `libgquic.dylib`
- Windows: `gquic.dll`

### Modern Zig FFI Bindings

Create `src/gquic.zig` for any Zig project:

```zig
const std = @import("std");
const builtin = @import("builtin");

// Dynamic library loading for cross-platform support
const lib_name = switch (builtin.target.os.tag) {
    .linux => "libgquic.so",
    .macos => "libgquic.dylib",
    .windows => "gquic.dll",
    else => @compileError("Unsupported platform"),
};

// Modern Zig FFI with comptime safety
pub const GQUIC_OK: c_int = 0;
pub const GQUIC_ERROR: c_int = -1;
pub const GQUIC_INVALID_PARAM: c_int = -2;
pub const GQUIC_CONNECTION_FAILED: c_int = -3;

// Opaque handles - safer than raw pointers
pub const GQuicClient = opaque {};
pub const GQuicServer = opaque {};
pub const GQuicConnection = opaque {};

// Modern error handling
pub const GQuicError = error{
    InitFailed,
    ConnectionFailed,
    StreamError,
    InvalidParam,
    OutOfMemory,
    Timeout,
};

// External function declarations
extern "c" fn gquic_client_new(server_name: [*:0]const u8, client_out: *?*GQuicClient) c_int;
extern "c" fn gquic_client_connect(client: *GQuicClient, addr: [*:0]const u8, conn_out: *?*GQuicConnection) c_int;
extern "c" fn gquic_send_data(conn: *GQuicConnection, data: [*]const u8, len: usize) c_int;
extern "c" fn gquic_recv_data(conn: *GQuicConnection, buffer: [*]u8, buffer_len: usize, received_len: *usize) c_int;
extern "c" fn gquic_client_destroy(client: *GQuicClient) void;

// High-level Zig wrapper
pub const QuicClient = struct {
    handle: *GQuicClient,
    allocator: std.mem.Allocator,

    const Self = @This();

    pub fn init(allocator: std.mem.Allocator, server_name: []const u8) !Self {
        // Ensure null termination for C FFI
        const c_server_name = try allocator.dupeZ(u8, server_name);
        defer allocator.free(c_server_name);

        var client: ?*GQuicClient = null;
        const result = gquic_client_new(c_server_name.ptr, &client);

        if (result != GQUIC_OK or client == null) {
            return GQuicError.InitFailed;
        }

        return Self{
            .handle = client.?,
            .allocator = allocator,
        };
    }

    pub fn connect(self: *Self, address: []const u8) !Connection {
        const c_address = try self.allocator.dupeZ(u8, address);
        defer self.allocator.free(c_address);

        var conn: ?*GQuicConnection = null;
        const result = gquic_client_connect(self.handle, c_address.ptr, &conn);

        return switch (result) {
            GQUIC_OK => Connection{ .handle = conn.?, .allocator = self.allocator },
            GQUIC_CONNECTION_FAILED => GQuicError.ConnectionFailed,
            GQUIC_INVALID_PARAM => GQuicError.InvalidParam,
            else => GQuicError.InitFailed,
        };
    }

    pub fn deinit(self: *Self) void {
        gquic_client_destroy(self.handle);
    }
};

pub const Connection = struct {
    handle: *GQuicConnection,
    allocator: std.mem.Allocator,

    const Self = @This();

    pub fn send(self: *Self, data: []const u8) !void {
        const result = gquic_send_data(self.handle, data.ptr, data.len);
        if (result != GQUIC_OK) {
            return GQuicError.StreamError;
        }
    }

    pub fn receive(self: *Self, buffer: []u8) ![]u8 {
        var received_len: usize = 0;
        const result = gquic_recv_data(
            self.handle,
            buffer.ptr,
            buffer.len,
            &received_len
        );

        return switch (result) {
            GQUIC_OK => buffer[0..received_len],
            else => GQuicError.StreamError,
        };
    }
};

```

### Example Usage in Zig L2 Projects

```zig
// ghostplane/src/network.zig - Example L2 networking
const std = @import("std");
const gquic = @import("gquic.zig");

pub const L2Network = struct {
    client: gquic.QuicClient,
    allocator: std.mem.Allocator,

    const Self = @This();

    pub fn init(allocator: std.mem.Allocator) !Self {
        const client = try gquic.QuicClient.init(allocator, "ghostplane.local");
        return Self{
            .client = client,
            .allocator = allocator,
        };
    }

    pub fn broadcast_transaction(self: *Self, tx_data: []const u8, peers: []const []const u8) !void {
        for (peers) |peer_addr| {
            var connection = self.client.connect(peer_addr) catch |err| {
                std.log.warn("Failed to connect to peer {s}: {}", .{ peer_addr, err });
                continue;
            };

            connection.send(tx_data) catch |err| {
                std.log.warn("Failed to send to peer {s}: {}", .{ peer_addr, err });
                continue;
            };
        }
    }

    pub fn deinit(self: *Self) void {
        self.client.deinit();
    }
};
```

## GhostBridge Cross-Chain Integration

**GhostBridge** provides ultra-fast cross-chain communication infrastructure with type-safe Rust implementation and FFI abstractions.
### Cross-Chain Communication Pattern

```rust
// ghostbridge/src/cross_chain.rs
use gquic::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossChainMessage {
    pub source_chain: String,
    pub target_chain: String,
    pub message_type: MessageType,
    pub payload: Vec<u8>,
    pub nonce: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageType {
    Transfer,
    ContractCall,
    StateSync,
    Bridge,
}

pub struct CrossChainBridge {
    quic_client: QuicClient,
    connection_pool: ConnectionPool,
    chain_endpoints: HashMap<String, SocketAddr>,
}

impl CrossChainBridge {
    pub async fn new() -> Result<Self> {
        let config = QuicClientConfig::builder()
            .server_name("bridge.ghostchain.network".to_string())
            .with_alpn("cross-chain")
            .with_alpn("bridge-v1")
            .max_idle_timeout(120_000) // 2 minutes for cross-chain ops
            .build();

        let client = QuicClient::new(config)?;
        let pool = ConnectionPool::new(PoolConfig::default());

        Ok(Self {
            quic_client: client,
            connection_pool: pool,
            chain_endpoints: HashMap::new(),
        })
    }

    pub async fn register_chain(&mut self, chain_id: &str, endpoint: SocketAddr) -> Result<()> {
        // Test connection to ensure chain is reachable
        let connection = self.quic_client.connect(endpoint).await?;
        self.connection_pool.return_connection(endpoint, connection).await;

        self.chain_endpoints.insert(chain_id.to_string(), endpoint);
        tracing::info!("Registered chain {} at {}", chain_id, endpoint);
        Ok(())
    }

    pub async fn send_cross_chain_message(
        &self,
        message: CrossChainMessage,
    ) -> Result<Vec<u8>> {
        let target_endpoint = self.chain_endpoints
            .get(&message.target_chain)
            .ok_or_else(|| anyhow::anyhow!("Unknown target chain: {}", message.target_chain))?;

        // Get or create connection to target chain
        let connection = match self.connection_pool.get_connection(*target_endpoint).await {
            Some(conn) => conn,
            None => {
                let conn = self.quic_client.connect(*target_endpoint).await?;
                self.connection_pool.return_connection(*target_endpoint, conn.clone()).await;
                conn
            }
        };

        // Serialize and send message
        let serialized = bincode::serialize(&message)?;
        let mut stream = connection.open_bi().await?;

        stream.write_all(&serialized).await?;
        stream.finish().await?;

        // Read response
        let response = stream.read_to_end(1024 * 1024).await?; // 1MB max
        Ok(response)
    }
}

```

## DNS over QUIC

Modern DNS resolution with enhanced privacy and performance using QUIC transport.

### Setting Up DNS over QUIC

```rust
// src/dns_quic.rs
use gquic::prelude::*;

pub struct DnsOverQuic {
    client: QuicClient,
    resolver_addr: SocketAddr,
}

impl DnsOverQuic {
    pub async fn new(resolver: &str) -> Result<Self> {
        let config = QuicClientConfig::builder()
            .server_name("dns.ghostchain.network".to_string())
            .with_alpn("doq") // DNS over QUIC
            .max_idle_timeout(30_000)
            .build();

        let client = QuicClient::new(config)?;
        let resolver_addr = resolver.parse()?;

        Ok(Self { client, resolver_addr })
    }

    pub async fn resolve(&self, domain: &str, record_type: DnsRecordType) -> Result<Vec<String>> {
        let connection = self.client.connect(self.resolver_addr).await?;
        let mut stream = connection.open_bi().await?;

        // Send DNS query
        let query = DnsQuery::new(domain, record_type);
        let query_bytes = query.serialize()?;

        stream.write_all(&query_bytes).await?;
        stream.finish().await?;

        // Read DNS response
        let response_bytes = stream.read_to_end(4096).await?;
        let response = DnsResponse::parse(&response_bytes)?;

        Ok(response.answers)
    }
}
```

## Advanced Rust Integration

### Generic gRPC/QUIC Service Pattern

```rust
// Generic pattern for any Rust service with QUIC transport
use gquic::prelude::*;
use tonic::{Request, Response, Status};

pub struct GenericQuicService<T> {
    inner_service: T,
    quic_server: QuicServer,
}

impl<T> GenericQuicService<T>
where
    T: Clone + Send + Sync + 'static,
{
    pub async fn new(service: T, bind_addr: SocketAddr) -> Result<Self> {
        let handler = GrpcHandler::new(service.clone());

        let server = QuicServer::builder()
            .bind(bind_addr)
            .with_self_signed_cert()? // Use proper certs in production
            .with_alpn("grpc")
            .with_alpn("h3")
            .max_concurrent_bidi_streams(1000)
            .build()?;

        Ok(Self {
            inner_service: service,
            quic_server: server,
        })
    }

    pub async fn serve(self) -> Result<()> {
        tracing::info!("Starting QUIC service on {}", self.quic_server.local_addr());
        self.quic_server.run().await
    }
}

```

## Complete GhostChain Ecosystem Integration

The following projects can all leverage GQUIC for high-performance networking:

### GhostChain Core Integration

```rust
// Integration with github.com/ghostkellz/ghostchain
use gquic::prelude::*;

pub struct GhostChainNode {
    quic_transport: QuicP2PTransport,
    validator: GhostChainValidator,
}

impl GhostChainNode {
    pub async fn new(bind_addr: SocketAddr) -> Result<Self> {
        let transport = QuicP2PTransport::new(bind_addr).await?;
        let validator = GhostChainValidator::new();

        Ok(Self {
            quic_transport: transport,
            validator,
        })
    }

    pub async fn sync_with_network(&self) -> Result<()> {
        // Use QUIC for fast blockchain sync
        let peers = self.discover_peers().await?;

        for peer in peers {
            let latest_block = self.quic_transport.request_latest_block(peer).await?;
            self.validator.validate_and_apply(latest_block).await?;
        }

        Ok(())
    }
}
```

### GCrypt Integration Pattern

```rust
// Integration with github.com/ghostkellz/gcrypt
use gquic::crypto::CryptoBackend;
use gcrypt::{Cipher, Hash, KeyDerivation};

pub struct GCryptBackend {
    inner: gcrypt::Backend,
}

impl CryptoBackend for GCryptBackend {
    fn generate_keypair(&self, key_type: KeyType) -> Result<KeyPair> {
        match key_type {
            KeyType::Ed25519 => self.inner.generate_ed25519(),
            KeyType::Secp256k1 => self.inner.generate_secp256k1(),
            _ => Err(CryptoError::UnsupportedKeyType),
        }
    }

    fn derive_shared_secret(&self, private_key: &PrivateKey, public_key: &PublicKey) -> Result<SharedSecret> {
        self.inner.ecdh(private_key, public_key)
    }
}

// Configure GQUIC to use GCrypt
pub fn configure_gquic_with_gcrypt() -> Result<QuicClientConfig> {
    let crypto_backend = GCryptBackend::new()?;

    QuicClientConfig::builder()
        .with_crypto_backend(Box::new(crypto_backend))
        .with_alpn("ghostchain")
        .build()
}

```

## Production Deployment

### Build Configuration

```toml
# Cargo.toml for production builds
[dependencies]
gquic = { git = "https://github.com/ghostkellz/gquic", features = ["gcc-crypto", "hardware-accel", "metrics"] }

[profile.release]
lto = true
codegen-units = 1
panic = "abort"
strip = true

[profile.release-with-debug]
inherits = "release"
debug = true
strip = false
```

### Cross-Platform Build Scripts

```bash
#!/bin/bash
# build.sh - Universal build script

set -e

# Detect platform
OS=$(uname -s)
ARCH=$(uname -m)

echo "Building GQUIC for $OS/$ARCH"

# Configure features based on platform
case "$OS" in
    Linux)
        FEATURES="gcc-crypto,hardware-accel,rustls-tls"
        TARGET_SUFFIX="linux"
        ;;
    Darwin)
        FEATURES="ring-crypto,rustls-tls"
        TARGET_SUFFIX="macos"
        ;;
    MINGW*|CYGWIN*|MSYS*)
        FEATURES="ring-crypto,rustls-tls"
        TARGET_SUFFIX="windows"
        ;;
    *)
        echo "Unsupported OS: $OS"
        exit 1
        ;;
esac

# Build with FFI for Zig integration
cargo build --release --features "ffi,$FEATURES"

# Create distribution package
mkdir -p dist/$TARGET_SUFFIX
cp target/release/libgquic.* dist/$TARGET_SUFFIX/ 2>/dev/null || true
cp include/gquic_ffi.h dist/$TARGET_SUFFIX/
cp README.md dist/$TARGET_SUFFIX/

echo "Build complete: dist/$TARGET_SUFFIX/"
```

### Docker Integration

```dockerfile
# Dockerfile.gquic
FROM rust:1.75-slim as builder

WORKDIR /build
COPY . .

# Install system dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Build with production features
RUN cargo build --release --features "gcc-crypto,hardware-accel,metrics"

# Runtime image
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/libgquic.so /usr/local/lib/
COPY --from=builder /build/include/gquic_ffi.h /usr/local/include/

# Update library path
RUN ldconfig

EXPOSE 9090/udp

# Default command for testing
CMD ["echo", "GQUIC library ready"]
```

### Environment Configuration

```bash
# .env.production
GQUIC_LOG_LEVEL=info
GQUIC_BIND_ADDR=0.0.0.0:9090
GQUIC_TLS_CERT_PATH=/etc/ssl/certs/gquic.crt
GQUIC_TLS_KEY_PATH=/etc/ssl/private/gquic.key
GQUIC_MAX_CONNECTIONS=10000
GQUIC_CRYPTO_BACKEND=gcrypt
GQUIC_HARDWARE_ACCEL=true
```

### Performance Tuning

```rust
// production_config.rs
use gquic::prelude::*;

pub fn production_server_config() -> Result<QuicServerConfig> {
    QuicServerConfig::builder()
        .max_concurrent_bidi_streams(5000)
        .max_concurrent_uni_streams(5000)
        .max_idle_timeout(Duration::from_secs(300))
        .keep_alive_interval(Duration::from_secs(30))
        .initial_rtt(Duration::from_millis(100))
        .max_ack_delay(Duration::from_millis(25))
        .ack_delay_exponent(3)
        .max_udp_payload_size(1472) // Avoid fragmentation
        .active_connection_id_limit(4)
        .enable_0rtt(true)
        .enable_migration(true)
        .build()
}

pub fn production_client_config() -> Result<QuicClientConfig> {
    QuicClientConfig::builder()
        .max_idle_timeout(Duration::from_secs(60))
        .keep_alive_interval(Duration::from_secs(20))
        .max_concurrent_streams(1000)
        .enable_0rtt(true)
        .enable_migration(true)
        .congestion_control(CongestionControl::Bbr)
        .build()
}
```

## Integration Summary

### Quick Start Guide

1. **Add GQUIC to your project:**
   ```toml
   [dependencies]
   gquic = { git = "https://github.com/ghostkellz/gquic", features = ["gcc-crypto"] }
   ```

2. **Choose your integration pattern:**
   - **Rust projects**: Use native GQUIC APIs
   - **Zig projects**: Use FFI bindings
   - **Plugin systems**: Implement GQUIC network plugins
   - **Cross-chain**: Use GQUIC for bridge communication

3. **Configure for your use case:**
   - **Etherlink**: Secure Rust-Zig bridge communication
   - **RVM**: VM networking with gas metering
   - **GhostPlane**: L2 execution with mesh networking
   - **GhostBridge**: Ultra-fast cross-chain transport
   - **GhostChain**: P2P blockchain networking
   - **GCrypt**: Advanced cryptographic backend

### Feature Selection Guide

Choose the right features for your project:

```toml
# For Rust projects with GCrypt
gquic = { git = "https://github.com/ghostkellz/gquic", features = ["gcc-crypto", "metrics"] }

# For Zig FFI integration
gquic = { git = "https://github.com/ghostkellz/gquic", features = ["ffi", "ring-crypto"] }

# For production deployment
gquic = { git = "https://github.com/ghostkellz/gquic", features = ["gcc-crypto", "hardware-accel", "metrics"] }

# For maximum compatibility
gquic = { git = "https://github.com/ghostkellz/gquic", features = ["ring-crypto", "rustls-tls"] }
```

### Integration Checklist

**For Rust Projects:**
- ✅ Add GQUIC with appropriate features
- ✅ Configure ALPN protocols for your service
- ✅ Implement connection pooling for performance
- ✅ Set up proper TLS certificates
- ✅ Add metrics monitoring
- ✅ Handle errors gracefully

**For Zig Projects:**
- ✅ Build GQUIC with FFI support
- ✅ Create Zig bindings using provided patterns
- ✅ Link against libgquic in build.zig
- ✅ Handle C memory management safely
- ✅ Test FFI integration thoroughly

**For All Projects:**
- ✅ Configure firewall for UDP traffic
- ✅ Set up monitoring and logging
- ✅ Plan for graceful shutdowns
- ✅ Test with realistic network conditions
- ✅ Document integration patterns for your team

## Next Steps

### Community and Support

- **Documentation**: See [DOCS.md](DOCS.md) for detailed API documentation
- **Examples**: Check the `examples/` directory for complete working examples
- **Issues**: Report bugs and feature requests on [GitHub Issues](https://github.com/ghostkellz/gquic/issues)
- **Discussions**: Join the community discussions for questions and best practices

### Related Projects

The complete GhostChain ecosystem:

| Project | Language | Purpose |
|---------|----------|---------|
| [GQUIC](https://github.com/ghostkellz/gquic) | Rust | High-performance QUIC networking library |
| [Etherlink](https://github.com/ghostkellz/etherlink) | Rust | Secure Rust-Zig bridge communication |
| [RVM](https://github.com/ghostkellz/rvm) | Rust | Modular virtual machine with networking plugins |
| [GhostPlane](https://github.com/ghostkellz/ghostplane) | Zig + Rust | Layer 2 execution engine with mesh networking |
| [GhostBridge](https://github.com/ghostkellz/ghostbridge) | Rust | Ultra-fast cross-chain communication infrastructure |
| [GhostChain](https://github.com/ghostkellz/ghostchain) | Rust | Core blockchain implementation |
| [GCrypt](https://github.com/ghostkellz/gcrypt) | Rust | Advanced cryptographic backend |

---

**Ready to integrate GQUIC into your project?** Start with the [Quick Start Guide](#integration-summary) above and choose the pattern that fits your architecture. For production deployments, follow the [build configuration](#production-deployment) and [performance tuning](#performance-tuning) guidelines.

This integration guide provides the foundation for building high-performance, secure networking in the modern blockchain ecosystem. Scale up based on your specific requirements and don't hesitate to contribute improvements back to the community.