# 🌉 INT_BRIDGE.md - gquic Integration Guide for GhostBridge

> **Complete integration guide for using gquic library with GhostBridge Zig-Rust bridge for high-performance blockchain networking**

---

## 🔧 Quick Start

### Prerequisites

- **Rust** 1.70+ with Cargo
- **Zig** 0.11+ 
- **GCC** crypto library (gcrypt)
- **OpenSSL** for TLS certificates

### Build gquic Library

```bash
# Clone and build gquic
git clone https://github.com/ghostkellz/gquic.git
cd gquic

# Build with all features
cargo build --release --features gcc-crypto,ffi

# Build FFI library for Zig integration
cargo build --release --features ffi
```

---

## 🏗️ Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                        GhostBridge Architecture                 │
├─────────────────────────────────────────────────────────────────┤
│  Zig Applications (zwallet, zns, realid)                       │
│  ├─ FFI Bindings (gquic.zig)                                   │
│  └─ C Headers (gquic_ffi.h)                                    │
├─────────────────────────────────────────────────────────────────┤
│  gquic Library (libgquic.so/dylib)                             │
│  ├─ QUIC Protocol Implementation                                │
│  ├─ GCC Crypto Backend                                         │
│  ├─ Connection Pool & Multiplexing                             │
│  └─ gRPC-over-QUIC Support                                     │
├─────────────────────────────────────────────────────────────────┤
│  Rust Services (ghostd, walletd, ghostbridge)                  │
│  ├─ Native gquic Integration                                   │
│  ├─ Blockchain Protocol Handlers                               │
│  └─ High-Performance Networking                                │
└─────────────────────────────────────────────────────────────────┘
```

---

## 📋 API Reference

### Core Types

#### Connection Management
```rust
pub struct Endpoint {
    socket: Arc<UdpSocket>,
    connections: HashMap<ConnectionId, Connection>,
}

pub struct Connection {
    id: ConnectionId,
    remote_addr: SocketAddr,
    socket: Arc<UdpSocket>,
    crypto_backend: Arc<dyn CryptoBackend>,
    handshake: Option<QuicHandshake>,
    shared_secret: Option<SharedSecret>,
}
```

#### Crypto Integration
```rust
pub trait CryptoBackend {
    fn generate_keypair(&self, key_type: KeyType) -> Result<KeyPair, CryptoError>;
    fn sign(&self, private_key: &PrivateKey, data: &[u8]) -> Result<Signature, CryptoError>;
    fn verify(&self, public_key: &PublicKey, data: &[u8], signature: &Signature) -> Result<bool, CryptoError>;
    fn encrypt(&self, key: &[u8], data: &[u8]) -> Result<Vec<u8>, CryptoError>;
    fn decrypt(&self, key: &[u8], encrypted_data: &[u8]) -> Result<Vec<u8>, CryptoError>;
}
```

### Key API Methods

#### **Endpoint Creation**
```rust
// Basic endpoint
let endpoint = Endpoint::bind("127.0.0.1:4433".parse()?).await?;

// Crypto-enhanced endpoint
let crypto_endpoint = Endpoint::bind_crypto(
    "127.0.0.1:4434".parse()?,
    b"my_secret_crypto_key_32_bytes___".to_vec()
).await?;
```

#### **Connection Operations**
```rust
// Accept connections
let connection = endpoint.accept().await?;

// Send data
connection.send(b"Hello, QUIC!").await?;

// Send encrypted data
connection.send_encrypted(b"Encrypted payload").await?;

// Receive decrypted data
let data = connection.receive_decrypted().await?;
```

#### **Blockchain-Specific Frames**
```rust
// Create blockchain transaction frame
let frame = Frame::BlockchainData {
    chain_id: 1,
    block_hash: Bytes::from(block_hash),
    data: Bytes::from(transaction_data),
};

// Send crypto authentication frame
let auth_frame = Frame::CryptoAuth {
    signature: Bytes::from(signature),
    public_key: Bytes::from(public_key),
};
```

---

## 🦀 Rust Integration

### Basic Client Setup

```rust
// Cargo.toml
[dependencies]
gquic = { path = "../gquic", features = ["gcc-crypto", "metrics"] }
tokio = { version = "1.0", features = ["full"] }
anyhow = "1.0"

// src/ghostbridge_client.rs
use gquic::prelude::*;
use anyhow::Result;

pub struct GhostBridgeClient {
    client: QuicClient,
    pool: ConnectionPool,
}

impl GhostBridgeClient {
    pub fn new() -> Result<Self> {
        let config = QuicClientConfig::builder()
            .server_name("ghostbridge.local".to_string())
            .with_alpn("ghostbridge-v1")
            .with_alpn("grpc")
            .max_idle_timeout(30_000)
            .build();

        let client = QuicClient::new(config)?;
        let pool = ConnectionPool::new(PoolConfig::default());

        Ok(Self { client, pool })
    }

    pub async fn send_wallet_request(&self, addr: SocketAddr, request: &[u8]) -> Result<Vec<u8>> {
        let conn = match self.pool.get_connection(addr).await {
            Some(conn) => conn,
            None => {
                let conn = self.client.connect(addr).await?;
                self.pool.return_connection(addr, conn.clone()).await;
                conn
            }
        };

        let mut stream = self.client.open_bi_stream(&conn).await?;
        stream.write_all(request).await?;
        stream.finish().await?;
        
        let response = stream.read_to_end(64 * 1024).await?;
        Ok(response)
    }
}
```

### Server Implementation

```rust
// src/ghostbridge_server.rs
use gquic::prelude::*;
use gquic::server::handler::{ConnectionHandler, DefaultHandler};
use async_trait::async_trait;

pub struct GhostBridgeHandler {
    // Blockchain state, database connections, etc.
}

#[async_trait]
impl ConnectionHandler for GhostBridgeHandler {
    async fn handle_connection(
        &self,
        connection: NewConnection,
        _config: Arc<QuicServerConfig>,
    ) -> Result<()> {
        let remote_addr = connection.connection.remote_address();
        tracing::info!("New GhostBridge connection from {}", remote_addr);

        // Handle bidirectional streams (request/response)
        while let Ok((mut send, mut recv)) = connection.bi_streams.accept().await {
            let handler = self.clone();
            tokio::spawn(async move {
                // Read request
                let request_data = recv.read_to_end(1024 * 1024).await?; // 1MB max
                
                // Process request based on protocol
                let response = handler.process_bridge_request(&request_data).await?;
                
                // Send response
                send.write_all(&response).await?;
                send.finish().await?;
                
                Ok::<(), anyhow::Error>(())
            });
        }

        Ok(())
    }
}

impl GhostBridgeHandler {
    async fn process_bridge_request(&self, data: &[u8]) -> Result<Vec<u8>> {
        // Parse request (could be gRPC, JSON, or custom binary format)
        match parse_request_type(data) {
            RequestType::WalletOperation => self.handle_wallet_request(data).await,
            RequestType::BlockchainQuery => self.handle_blockchain_request(data).await,
            RequestType::DomainResolution => self.handle_dns_request(data).await,
            _ => Err(anyhow::anyhow!("Unknown request type")),
        }
    }

    async fn handle_wallet_request(&self, data: &[u8]) -> Result<Vec<u8>> {
        // Forward to walletd service
        // Return serialized response
        Ok(b"wallet_response".to_vec())
    }

    async fn handle_blockchain_request(&self, data: &[u8]) -> Result<Vec<u8>> {
        // Forward to ghostd service
        // Return serialized response
        Ok(b"blockchain_response".to_vec())
    }

    async fn handle_dns_request(&self, data: &[u8]) -> Result<Vec<u8>> {
        // Handle zns domain resolution
        // Return serialized response
        Ok(b"dns_response".to_vec())
    }
}

pub async fn start_ghostbridge_server() -> Result<()> {
    let handler = GhostBridgeHandler::new();
    
    let server = QuicServer::builder()
        .bind("0.0.0.0:9090".parse()?)
        .with_tls_files("certs/ghostbridge.crt", "certs/ghostbridge.key")?
        .with_alpn("ghostbridge-v1")
        .with_alpn("grpc")
        .with_handler(Arc::new(handler))
        .max_concurrent_bidi_streams(2000)
        .build()?;

    server.run().await
}
```

---

## 🔄 Zig Integration (FFI)

### Build Configuration

```zig
// build.zig
const std = @import("std");

pub fn build(b: *std.Build) void {
    const target = b.standardTargetOptions(.{});
    const optimize = b.standardOptimizeOption(.{});

    const exe = b.addExecutable(.{
        .name = "ghostbridge-client",
        .root_source_file = .{ .path = "src/main.zig" },
        .target = target,
        .optimize = optimize,
    });

    // Link gquic library
    exe.linkLibC();
    exe.linkSystemLibrary("gquic");
    exe.addLibraryPath(.{ .path = "../gquic/target/release" });
    exe.addIncludePath(.{ .path = "../gquic/include" });

    b.installArtifact(exe);

    const run_cmd = b.addRunArtifact(exe);
    run_cmd.step.dependOn(b.getInstallStep());

    if (b.args) |args| {
        run_cmd.addArgs(args);
    }

    const run_step = b.step("run", "Run the app");
    run_step.dependOn(&run_cmd.step);
}
```

### Zig FFI Bindings

```zig
// src/gquic.zig
const std = @import("std");
const c = @cImport({
    @cInclude("gquic_ffi.h");
});

pub const GQuicError = error{
    InvalidParam,
    ConnectionFailed,
    StreamError,
    InitFailed,
};

pub const GQuicClient = struct {
    handle: ?*c.GQuicClient,
    allocator: std.mem.Allocator,

    const Self = @This();

    pub fn init(allocator: std.mem.Allocator, server_name: []const u8) !Self {
        var client: ?*c.GQuicClient = null;
        const server_name_cstr = try allocator.dupeZ(u8, server_name);
        defer allocator.free(server_name_cstr);

        const result = c.gquic_client_new(server_name_cstr.ptr, &client);
        if (result != c.GQUIC_OK) {
            return GQuicError.InitFailed;
        }

        return Self{
            .handle = client,
            .allocator = allocator,
        };
    }

    pub fn connect(self: *Self, addr: []const u8) !?*anyopaque {
        const addr_cstr = try self.allocator.dupeZ(u8, addr);
        defer self.allocator.free(addr_cstr);

        var connection: ?*anyopaque = null;
        const result = c.gquic_client_connect(self.handle, addr_cstr.ptr, &connection);
        
        return switch (result) {
            c.GQUIC_OK => connection,
            c.GQUIC_CONNECTION_FAILED => GQuicError.ConnectionFailed,
            else => GQuicError.InvalidParam,
        };
    }

    pub fn sendData(self: *Self, connection: *anyopaque, data: []const u8) !void {
        const result = c.gquic_client_send_data(
            self.handle,
            connection,
            data.ptr,
            data.len,
        );

        if (result != c.GQUIC_OK) {
            return GQuicError.StreamError;
        }
    }

    pub fn deinit(self: *Self) void {
        if (self.handle) |handle| {
            c.gquic_client_destroy(handle);
            self.handle = null;
        }
    }
};

pub const GQuicServer = struct {
    handle: ?*c.GQuicServer,
    allocator: std.mem.Allocator,

    const Self = @This();

    pub fn init(allocator: std.mem.Allocator, config: ServerConfig) !Self {
        const bind_addr_cstr = try allocator.dupeZ(u8, config.bind_addr);
        defer allocator.free(bind_addr_cstr);

        const cert_path_cstr = try allocator.dupeZ(u8, config.cert_path);
        defer allocator.free(cert_path_cstr);

        const key_path_cstr = try allocator.dupeZ(u8, config.key_path);
        defer allocator.free(key_path_cstr);

        // Convert ALPN protocols to C strings
        var alpn_cstrs = std.ArrayList([*:0]const u8).init(allocator);
        defer alpn_cstrs.deinit();

        for (config.alpn_protocols) |protocol| {
            const cstr = try allocator.dupeZ(u8, protocol);
            try alpn_cstrs.append(cstr.ptr);
        }

        const c_config = c.GQuicConfig{
            .bind_addr = bind_addr_cstr.ptr,
            .cert_path = cert_path_cstr.ptr,
            .key_path = key_path_cstr.ptr,
            .alpn_protocols = alpn_cstrs.items.ptr,
            .alpn_count = alpn_cstrs.items.len,
            .max_connections = config.max_connections,
            .use_self_signed = if (config.use_self_signed) 1 else 0,
        };

        var server: ?*c.GQuicServer = null;
        const result = c.gquic_server_new(&c_config, &server);
        
        // Clean up ALPN strings
        for (alpn_cstrs.items) |cstr| {
            allocator.free(std.mem.span(cstr));
        }

        if (result != c.GQUIC_OK) {
            return GQuicError.InitFailed;
        }

        return Self{
            .handle = server,
            .allocator = allocator,
        };
    }

    pub fn start(self: *Self, callback: c.GQuicConnectionCallback, user_data: ?*anyopaque) !void {
        const result = c.gquic_server_start(self.handle, callback, user_data);
        if (result != c.GQUIC_OK) {
            return GQuicError.InitFailed;
        }
    }

    pub fn deinit(self: *Self) void {
        if (self.handle) |handle| {
            c.gquic_server_destroy(handle);
            self.handle = null;
        }
    }
};

pub const ServerConfig = struct {
    bind_addr: []const u8,
    cert_path: []const u8,
    key_path: []const u8,
    alpn_protocols: []const []const u8,
    max_connections: u32,
    use_self_signed: bool,
};
```

### Zig Application Example

```zig
// src/main.zig
const std = @import("std");
const gquic = @import("gquic.zig");

pub fn main() !void {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    defer _ = gpa.deinit();
    const allocator = gpa.allocator();

    // Initialize logging
    _ = gquic.c.gquic_init_logging(2); // INFO level

    // Create GhostBridge client
    var client = try gquic.GQuicClient.init(allocator, "ghostbridge.local");
    defer client.deinit();

    // Connect to GhostBridge server
    const connection = try client.connect("127.0.0.1:9090");
    if (connection == null) {
        std.debug.print("Failed to connect to GhostBridge server\n");
        return;
    }

    // Send wallet request
    const wallet_request = createWalletRequest(allocator) catch |err| {
        std.debug.print("Failed to create wallet request: {}\n", .{err});
        return;
    };
    defer allocator.free(wallet_request);

    try client.sendData(connection.?, wallet_request);
    std.debug.print("Wallet request sent successfully\n");

    // Send blockchain query
    const blockchain_query = createBlockchainQuery(allocator) catch |err| {
        std.debug.print("Failed to create blockchain query: {}\n", .{err});
        return;
    };
    defer allocator.free(blockchain_query);

    try client.sendData(connection.?, blockchain_query);
    std.debug.print("Blockchain query sent successfully\n");

    // Send domain resolution request
    const domain_request = createDomainRequest(allocator, "ghostkellz.zkellz") catch |err| {
        std.debug.print("Failed to create domain request: {}\n", .{err});
        return;
    };
    defer allocator.free(domain_request);

    try client.sendData(connection.?, domain_request);
    std.debug.print("Domain resolution request sent successfully\n");
}

fn createWalletRequest(allocator: std.mem.Allocator) ![]u8 {
    // Create a simple wallet balance request
    const request = std.json.stringify(.{
        .type = "wallet_balance",
        .account = "ghostkellz.ghost",
        .token = "MANA",
    }, allocator);
    
    return request;
}

fn createBlockchainQuery(allocator: std.mem.Allocator) ![]u8 {
    // Create a blockchain query request
    const request = std.json.stringify(.{
        .type = "get_block",
        .block_number = 12345,
        .include_transactions = true,
    }, allocator);
    
    return request;
}

fn createDomainRequest(allocator: std.mem.Allocator, domain: []const u8) ![]u8 {
    // Create a domain resolution request
    const request = std.json.stringify(.{
        .type = "resolve_domain",
        .domain = domain,
        .record_types = &[_][]const u8{ "A", "TXT", "CNAME" },
    }, allocator);
    
    return request;
}
```

---

## 🚀 Performance Optimization

### Connection Pool Configuration

```rust
// High-performance connection pool
let pool_config = PoolConfig::builder()
    .max_connections_per_endpoint(100)
    .max_connection_age(Duration::from_secs(3600))  // 1 hour
    .max_idle_time(Duration::from_secs(300))        // 5 minutes
    .enable_multiplexing(true)
    .max_concurrent_streams(500)
    .build();

// Server configuration for high throughput
let server_config = QuicServerConfig::builder()
    .max_concurrent_bidi_streams(5000)
    .max_concurrent_uni_streams(5000)
    .max_idle_timeout(Duration::from_secs(60))
    .keep_alive_interval(Duration::from_secs(20))
    .build()?;
```

### Metrics and Monitoring

```rust
#[cfg(feature = "metrics")]
{
    use gquic::metrics::get_metrics;
    
    // Periodic metrics reporting
    tokio::spawn(async {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        
        loop {
            interval.tick().await;
            let metrics = get_metrics().get_metrics().await;
            
            tracing::info!(
                "gquic metrics - Active: {}, Total: {}, Failed: {}, Latency: {:.2}ms",
                metrics.connection.active_connections,
                metrics.connection.total_connections,
                metrics.connection.failed_connections,
                metrics.connection.average_latency_ms
            );
        }
    });
}
```

---

## 🔧 Build Issues & Solutions

### Common FFI Build Problems

1. **Missing libgquic.so/dylib**
   ```bash
   # Ensure library is built with FFI support
   cd gquic
   cargo build --release --features ffi
   
   # Check library was created
   ls -la target/release/libgquic.*
   ```

2. **Zig Linker Issues**
   ```bash
   # Add library path to LD_LIBRARY_PATH
   export LD_LIBRARY_PATH=$LD_LIBRARY_PATH:$(pwd)/gquic/target/release
   
   # Or copy to system library path
   sudo cp gquic/target/release/libgquic.so /usr/local/lib/
   sudo ldconfig
   ```

3. **Header File Missing**
   ```bash
   # Ensure header is accessible
   cp gquic/include/gquic_ffi.h /usr/local/include/
   # Or use relative path in build.zig
   ```

### Rust Dependencies

```toml
# Cargo.toml - Known working versions
[dependencies]
gquic = { path = "../gquic", features = ["gcc-crypto", "ffi"] }
tokio = { version = "1.45", features = ["full"] }
bytes = "1.10"
thiserror = "1.0"
anyhow = "1.0"
tracing = "0.1"
tracing-subscriber = "0.3"

# For gRPC integration
tonic = "0.10"
prost = "0.12"
```

### System Dependencies

```bash
# Ubuntu/Debian
sudo apt-get update
sudo apt-get install -y build-essential libssl-dev pkg-config

# macOS
brew install openssl pkg-config

# gCrypt dependency (for GCC crypto backend)
# Ubuntu/Debian
sudo apt-get install -y libgcrypt20-dev

# macOS
brew install libgcrypt
```

---

## 📋 Integration Checklist

### For Rust Projects
- [ ] Add gquic dependency with `gcc-crypto` feature
- [ ] Configure connection pooling for high throughput
- [ ] Set appropriate ALPN protocols (`ghostbridge-v1`, `grpc`)
- [ ] Configure TLS certificates (production) or self-signed (dev)
- [ ] Add metrics monitoring with periodic reporting
- [ ] Handle connection failures and retries gracefully
- [ ] Test with multiple concurrent connections

### For Zig Projects
- [ ] Build gquic with FFI support (`--features ffi`)
- [ ] Create complete Zig bindings in `src/gquic.zig`
- [ ] Configure build.zig to link against libgquic
- [ ] Handle C memory management properly (allocator usage)
- [ ] Test FFI integration with simple client/server
- [ ] Add error handling for all FFI calls
- [ ] Create higher-level abstractions over raw FFI

### For GhostBridge Deployment
- [ ] Set up proper TLS certificates for production
- [ ] Configure firewalls to allow UDP traffic on QUIC ports
- [ ] Set up monitoring and alerting for connection metrics
- [ ] Configure log rotation and structured logging
- [ ] Plan for graceful shutdowns and restarts
- [ ] Test load balancing across multiple instances
- [ ] Document API endpoints and protocols

---

## 🔍 Troubleshooting Guide

### Connection Issues

**Problem**: Client cannot connect to server
```bash
# Check if server is listening
sudo netstat -ulnp | grep 9090

# Check firewall rules
sudo ufw status
sudo firewall-cmd --list-all

# Test basic connectivity
nc -u 127.0.0.1 9090
```

**Solution**: Ensure UDP port is open and server is bound to correct address.

### FFI Integration Issues

**Problem**: Zig cannot find libgquic
```bash
# Check if library exists
ls -la gquic/target/release/libgquic.*

# Check if library is in path
echo $LD_LIBRARY_PATH

# Add to path
export LD_LIBRARY_PATH=$LD_LIBRARY_PATH:$(pwd)/gquic/target/release
```

**Solution**: Ensure library is built and accessible to Zig linker.

### Performance Issues

**Problem**: High latency or connection failures
```rust
// Add detailed logging
tracing::info!("Connection attempt to {}", addr);

// Check connection pool stats
let pool_stats = pool.get_stats().await;
tracing::info!("Pool stats: active={}, idle={}", 
               pool_stats.active_connections, 
               pool_stats.idle_connections);
```

**Solution**: Monitor metrics, adjust pool settings, and optimize network configuration.

---

## 📞 Support & Resources

### Getting Help

1. **Build Issues**: Check `cargo build --release --features ffi` output
2. **Runtime Issues**: Enable debug logging with `RUST_LOG=debug`
3. **Performance Issues**: Use metrics to identify bottlenecks
4. **Integration Issues**: Verify FFI bindings match library API

### Useful Commands

```bash
# Check gquic library symbols
nm -D gquic/target/release/libgquic.so | grep gquic

# Test Zig FFI integration
zig build-exe test_ffi.zig -lgquic -L./gquic/target/release

# Monitor QUIC traffic
sudo tcpdump -i any -nn port 9090

# Check system UDP limits
cat /proc/sys/net/core/rmem_max
cat /proc/sys/net/core/wmem_max
```

### Performance Tuning

```bash
# Increase UDP buffer sizes
sudo sysctl -w net.core.rmem_max=134217728
sudo sysctl -w net.core.wmem_max=134217728
sudo sysctl -w net.ipv4.udp_mem="102400 873800 16777216"

# Make permanent
echo "net.core.rmem_max = 134217728" | sudo tee -a /etc/sysctl.conf
echo "net.core.wmem_max = 134217728" | sudo tee -a /etc/sysctl.conf
sudo sysctl -p
```

---

## 🎯 Next Steps

1. **Basic Integration**: Start with simple client-server example
2. **Add Encryption**: Implement GCC crypto backend for secure communication
3. **Scale Up**: Add connection pooling and load balancing
4. **Monitor**: Set up metrics collection and alerting
5. **Production**: Configure proper TLS certificates and security

### Example Implementation Timeline

- **Week 1**: Basic FFI integration, simple client/server
- **Week 2**: Add crypto support and proper error handling
- **Week 3**: Implement connection pooling and metrics
- **Week 4**: Production deployment and monitoring

This guide provides everything needed to integrate gquic with your GhostBridge project. The library is production-ready and the FFI bindings are complete, so you can start building high-performance blockchain networking applications immediately.

---

**© 2025 CK Technology LLC - MIT License**