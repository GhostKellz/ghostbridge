# 🌉 GhostBridge Integration Guide

> **GhostBridge** is a high-performance gRPC-over-QUIC bridge that enables seamless interoperability between Zig and Rust components in the GhostChain ecosystem. This guide covers the latest features and integration patterns for production deployment.

---

## 🎯 Overview

GhostBridge serves as the universal communication layer for the GhostChain ecosystem, providing:

- **🚀 QUIC Transport**: Ultra-fast UDP-based transport with 0-RTT support
- **🔐 Crypto Integration**: Native support for Ed25519, AES-GCM, and X25519 via gcrypt
- **🌐 Multi-Protocol**: HTTP/3, gRPC-over-QUIC, and custom protocols
- **⚡ Zero-Copy**: Optimized packet processing for maximum throughput  
- **🔄 Cross-Language**: Seamless Zig ↔ Rust interoperability
- **📊 Observability**: Built-in metrics, tracing, and monitoring

---

## 🏗️ Architecture

```
┌─────────────────────────────────────────────────────┐
│                  GhostBridge                        │
├─────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  │
│  │   HTTP/3    │  │    gRPC     │  │   Custom    │  │
│  │  Handler    │  │   Handler   │  │   Protocol  │  │
│  └─────────────┘  └─────────────┘  └─────────────┘  │
├─────────────────────────────────────────────────────┤
│              QUIC Connection Manager                │
├─────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  │
│  │     TLS     │  │    Pool     │  │   Crypto    │  │
│  │   Handler   │  │   Manager   │  │   Backend   │  │
│  └─────────────┘  └─────────────┘  └─────────────┘  │
└─────────────────────────────────────────────────────┘
```

---

## 🚀 Quick Start

### 1. Server Setup

```rust
use gquic::prelude::*;
use gquic::bridge::{GhostBridge, BridgeConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize bridge with auto-discovery
    let bridge = GhostBridge::builder()
        .bind("0.0.0.0:9090")
        .with_tls_files("certs/bridge.crt", "certs/bridge.key")?
        .with_service_discovery()?
        .enable_walletd_integration()?
        .enable_ghostd_integration()?
        .enable_zns_integration()?
        .enable_metrics()?
        .build().await?;

    tracing::info!("🌉 GhostBridge starting on :9090");
    bridge.serve().await
}
```

### 2. Client Connection

```rust
use gquic::bridge::BridgeClient;

let client = BridgeClient::builder()
    .server_name("ghostbridge.local")
    .with_alpn("grpc")
    .with_connection_pool(50)
    .build().await?;

// Use WalletD service
let wallet_client = client.wallet_service().await?;
let account = wallet_client.create_account(CreateAccountRequest {
    name: "main".to_string(),
    passphrase: "secure_passphrase".to_string(),
    key_type: KeyType::KeyTypeEd25519 as i32,
}).await?;

println!("Created account: {}", account.into_inner().account_id);
```

---

## 🔧 Core Features

### 1. Service Integration

#### WalletD Integration
```rust
// Automatic service discovery and routing
let bridge = GhostBridge::builder()
    .with_walletd_endpoint("127.0.0.1:8080")
    .with_walletd_tls("certs/walletd.crt")
    .enable_account_caching(Duration::from_secs(300))
    .build().await?;
```

#### GhostD Integration
```rust
// P2P blockchain node integration
let bridge = GhostBridge::builder()
    .with_ghostd_endpoint("127.0.0.1:8081")
    .enable_block_streaming()
    .enable_mempool_sync()
    .with_consensus_integration()
    .build().await?;
```

#### ZNS Integration
```rust
// DNS and identity resolution
let bridge = GhostBridge::builder()
    .with_zns_endpoint("127.0.0.1:8082")
    .enable_dns_caching(Duration::from_secs(600))
    .with_domain_validation()
    .build().await?;
```

### 2. Protocol Handlers

#### HTTP/3 Handler
```rust
use gquic::bridge::handlers::Http3Handler;

let handler = Http3Handler::builder()
    .with_compression(true)
    .with_push_promises(false)
    .max_field_section_size(16384)
    .build();

bridge.register_handler("h3", Arc::new(handler));
```

#### gRPC Handler
```rust
use gquic::bridge::handlers::GrpcHandler;

let handler = GrpcHandler::builder()
    .with_reflection(true)
    .with_health_check(true)
    .max_message_size(4 * 1024 * 1024) // 4MB
    .build();

bridge.register_handler("grpc", Arc::new(handler));
```

#### Custom Protocol Handler
```rust
use gquic::bridge::handlers::{ProtocolHandler, HandlerContext};

struct GhostChainHandler;

#[async_trait]
impl ProtocolHandler for GhostChainHandler {
    async fn handle_stream(
        &self,
        ctx: HandlerContext,
        stream: BidiStream,
    ) -> Result<()> {
        // Handle custom GhostChain protocol
        let request = stream.read_message::<GhostChainRequest>().await?;
        
        match request.method.as_str() {
            "submit_transaction" => {
                let response = self.handle_transaction(request.data).await?;
                stream.write_message(&response).await?;
            }
            "query_balance" => {
                let response = self.handle_balance_query(request.data).await?;
                stream.write_message(&response).await?;
            }
            _ => return Err(anyhow::anyhow!("Unknown method")),
        }
        
        Ok(())
    }
}
```

### 3. Connection Management

#### Connection Pool Configuration
```rust
use gquic::pool::{PoolConfig, PoolStrategy};

let pool_config = PoolConfig::builder()
    .max_connections_per_endpoint(100)
    .max_connection_age(Duration::from_secs(3600))
    .max_idle_time(Duration::from_secs(300))
    .cleanup_interval(Duration::from_secs(30))
    .strategy(PoolStrategy::RoundRobin)
    .enable_health_checks(true)
    .health_check_interval(Duration::from_secs(10))
    .build();

let bridge = GhostBridge::builder()
    .with_connection_pool(pool_config)
    .build().await?;
```

#### Load Balancing
```rust
use gquic::bridge::LoadBalancer;

let balancer = LoadBalancer::builder()
    .add_backend("walletd-1", "127.0.0.1:8080")
    .add_backend("walletd-2", "127.0.0.1:8081")
    .add_backend("walletd-3", "127.0.0.1:8082")
    .strategy(LoadBalancingStrategy::LeastConnections)
    .health_check_path("/health")
    .build();

bridge.register_load_balancer("walletd", balancer);
```

---

## 🔐 Security Features

### 1. mTLS Authentication
```rust
let bridge = GhostBridge::builder()
    .with_mtls_config(MtlsConfig {
        client_ca_cert: "certs/client-ca.crt".to_string(),
        server_cert: "certs/server.crt".to_string(),
        server_key: "certs/server.key".to_string(),
        require_client_auth: true,
    })
    .build().await?;
```

### 2. Token-Based Authentication
```rust
use gquic::bridge::auth::{JwtAuthenticator, TokenValidator};

let authenticator = JwtAuthenticator::builder()
    .with_secret("your-jwt-secret")
    .with_issuer("ghostchain.org")
    .with_audience("ghostbridge")
    .token_expiry(Duration::from_secs(3600))
    .build();

bridge.register_authenticator(Arc::new(authenticator));
```

### 3. Rate Limiting
```rust
use gquic::bridge::RateLimiter;

let limiter = RateLimiter::builder()
    .requests_per_second(1000)
    .burst_size(100)
    .per_ip_limit(100)
    .build();

bridge.register_rate_limiter(Arc::new(limiter));
```

---

## 🎯 Service Definitions

### 1. Enhanced WalletD Service
```protobuf
service WalletService {
    // Account operations
    rpc CreateAccount(CreateAccountRequest) returns (CreateAccountResponse);
    rpc GetBalance(GetBalanceRequest) returns (GetBalanceResponse);
    rpc ListAccounts(ListAccountsRequest) returns (ListAccountsResponse);
    rpc DeleteAccount(DeleteAccountRequest) returns (DeleteAccountResponse);
    
    // Transaction operations
    rpc SendTransaction(SendTransactionRequest) returns (SendTransactionResponse);
    rpc BatchTransactions(BatchTransactionsRequest) returns (BatchTransactionsResponse);
    rpc GetTransaction(GetTransactionRequest) returns (GetTransactionResponse);
    rpc GetTransactionHistory(GetTransactionHistoryRequest) returns (GetTransactionHistoryResponse);
    rpc EstimateGas(EstimateGasRequest) returns (EstimateGasResponse);
    
    // Signing operations
    rpc SignData(SignDataRequest) returns (SignDataResponse);
    rpc SignTransaction(SignTransactionRequest) returns (SignTransactionResponse);
    rpc VerifySignature(VerifySignatureRequest) returns (VerifySignatureResponse);
    rpc MultiSig(MultiSigRequest) returns (MultiSigResponse);
    
    // Identity operations
    rpc GetIdentity(GetIdentityRequest) returns (GetIdentityResponse);
    rpc RegisterIdentity(RegisterIdentityRequest) returns (RegisterIdentityResponse);
    rpc UpdateIdentity(UpdateIdentityRequest) returns (UpdateIdentityResponse);
    rpc ResolveIdentity(ResolveIdentityRequest) returns (ResolveIdentityResponse);
    
    // Streaming operations
    rpc StreamTransactions(StreamTransactionsRequest) returns (stream TransactionEvent);
    rpc StreamBalanceUpdates(StreamBalanceRequest) returns (stream BalanceUpdate);
}
```

### 2. GhostD Service
```protobuf
service GhostDService {
    // Blockchain operations
    rpc GetBlockchainInfo(GetBlockchainInfoRequest) returns (GetBlockchainInfoResponse);
    rpc GetBlock(GetBlockRequest) returns (GetBlockResponse);
    rpc GetBlockHash(GetBlockHashRequest) returns (GetBlockHashResponse);
    rpc GetBestBlockHash(GetBestBlockHashRequest) returns (GetBestBlockHashResponse);
    
    // Mempool operations
    rpc GetMempoolInfo(GetMempoolInfoRequest) returns (GetMempoolInfoResponse);
    rpc GetRawMempool(GetRawMempoolRequest) returns (GetRawMempoolResponse);
    rpc GetMempoolEntry(GetMempoolEntryRequest) returns (GetMempoolEntryResponse);
    
    // P2P operations
    rpc GetPeerInfo(GetPeerInfoRequest) returns (GetPeerInfoResponse);
    rpc AddPeer(AddPeerRequest) returns (AddPeerResponse);
    rpc BanPeer(BanPeerRequest) returns (BanPeerResponse);
    
    // Consensus operations
    rpc GetConsensusInfo(GetConsensusInfoRequest) returns (GetConsensusInfoResponse);
    rpc ProposeBlock(ProposeBlockRequest) returns (ProposeBlockResponse);
    rpc ValidateBlock(ValidateBlockRequest) returns (ValidateBlockResponse);
    
    // Streaming operations
    rpc StreamBlocks(StreamBlocksRequest) returns (stream BlockEvent);
    rpc StreamTransactions(StreamTransactionsRequest) returns (stream TransactionEvent);
    rpc StreamConsensusEvents(StreamConsensusRequest) returns (stream ConsensusEvent);
}
```

### 3. ZNS Service
```protobuf
service ZNSService {
    // Domain operations
    rpc ResolveDomain(ResolveDomainRequest) returns (ResolveDomainResponse);
    rpc RegisterDomain(RegisterDomainRequest) returns (RegisterDomainResponse);
    rpc UpdateDomain(UpdateDomainRequest) returns (UpdateDomainResponse);
    rpc DeleteDomain(DeleteDomainRequest) returns (DeleteDomainResponse);
    rpc ListDomains(ListDomainsRequest) returns (ListDomainsResponse);
    
    // Record operations
    rpc GetRecord(GetRecordRequest) returns (GetRecordResponse);
    rpc SetRecord(SetRecordRequest) returns (SetRecordResponse);
    rpc DeleteRecord(DeleteRecordRequest) returns (DeleteRecordResponse);
    rpc ListRecords(ListRecordsRequest) returns (ListRecordsResponse);
    
    // Identity operations
    rpc GetDomainIdentity(GetDomainIdentityRequest) returns (GetDomainIdentityResponse);
    rpc VerifyDomainOwnership(VerifyDomainOwnershipRequest) returns (VerifyDomainOwnershipResponse);
    
    // Streaming operations
    rpc StreamDomainUpdates(StreamDomainUpdatesRequest) returns (stream DomainEvent);
    rpc SubscribeDomainChanges(SubscribeDomainChangesRequest) returns (stream DomainChangeEvent);
}
```

---

## 🔄 Zig Integration

### 1. FFI Bindings
```zig
const std = @import("std");
const c = @cImport({
    @cInclude("ghostbridge_ffi.h");
});

pub const GhostBridge = struct {
    handle: ?*c.GhostBridge,
    
    pub fn init(config: BridgeConfig) !GhostBridge {
        const c_config = c.GhostBridgeConfig{
            .bind_addr = config.bind_addr.ptr,
            .tls_cert = config.tls_cert.ptr,
            .tls_key = config.tls_key.ptr,
            .enable_walletd = if (config.enable_walletd) 1 else 0,
            .enable_ghostd = if (config.enable_ghostd) 1 else 0,
            .enable_zns = if (config.enable_zns) 1 else 0,
        };
        
        var bridge: ?*c.GhostBridge = null;
        const result = c.ghostbridge_new(&c_config, &bridge);
        
        if (result != c.GHOSTBRIDGE_OK) {
            return error.InitializationFailed;
        }
        
        return GhostBridge{ .handle = bridge };
    }
    
    pub fn start(self: *GhostBridge) !void {
        const result = c.ghostbridge_start(self.handle);
        if (result != c.GHOSTBRIDGE_OK) {
            return error.StartFailed;
        }
    }
    
    pub fn deinit(self: *GhostBridge) void {
        if (self.handle) |handle| {
            c.ghostbridge_destroy(handle);
        }
    }
};
```

### 2. Client Usage
```zig
const bridge = @import("ghostbridge.zig");

pub fn main() !void {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    defer _ = gpa.deinit();
    
    const allocator = gpa.allocator();
    
    // Initialize bridge client
    var client = try bridge.BridgeClient.init(allocator, "127.0.0.1:9090");
    defer client.deinit();
    
    // Create wallet account
    const account_req = bridge.CreateAccountRequest{
        .name = "test_account",
        .passphrase = "secure_passphrase",
        .key_type = .Ed25519,
    };
    
    const account_resp = try client.createAccount(account_req);
    defer account_resp.deinit();
    
    std.debug.print("Created account: {s}\n", .{account_resp.account_id});
    
    // Get balance
    const balance_req = bridge.GetBalanceRequest{
        .account_id = account_resp.account_id,
        .token_type = "MANA",
    };
    
    const balance_resp = try client.getBalance(balance_req);
    defer balance_resp.deinit();
    
    std.debug.print("Balance: {s} MANA\n", .{balance_resp.balance});
}
```

---

## 📊 Monitoring & Observability

### 1. Metrics Collection
```rust
use gquic::bridge::metrics::{BridgeMetrics, MetricsCollector};

let metrics = BridgeMetrics::builder()
    .with_prometheus_exporter()
    .bind_metrics_server("0.0.0.0:9091")
    .enable_custom_metrics()
    .build();

bridge.register_metrics(metrics);

// Custom metrics
let counter = metrics.create_counter("ghostbridge_requests_total", "Total requests");
let histogram = metrics.create_histogram("ghostbridge_request_duration", "Request duration");
```

### 2. Distributed Tracing
```rust
use gquic::bridge::tracing::{JaegerTracer, TracingConfig};

let tracer = JaegerTracer::builder()
    .service_name("ghostbridge")
    .jaeger_endpoint("http://localhost:14268/api/traces")
    .sampling_ratio(0.1)
    .build();

bridge.register_tracer(Arc::new(tracer));
```

### 3. Health Checks
```rust
use gquic::bridge::health::{HealthChecker, HealthStatus};

let health_checker = HealthChecker::builder()
    .add_check("walletd", Box::new(WalletdHealthCheck))
    .add_check("ghostd", Box::new(GhostdHealthCheck))
    .add_check("zns", Box::new(ZnsHealthCheck))
    .check_interval(Duration::from_secs(30))
    .build();

bridge.register_health_checker(health_checker);
```

---

## 🚀 Performance Optimization

### 1. Connection Pool Tuning
```rust
let pool_config = PoolConfig::builder()
    .max_connections_per_endpoint(200)     // Scale with load
    .max_connection_age(Duration::from_secs(7200))  // 2 hours
    .max_idle_time(Duration::from_secs(300))        // 5 minutes
    .cleanup_interval(Duration::from_secs(15))      // Frequent cleanup
    .enable_connection_reuse(true)
    .enable_multiplexing(true)
    .max_concurrent_streams(500)
    .build();
```

### 2. Message Batching
```rust
let bridge = GhostBridge::builder()
    .enable_message_batching(true)
    .batch_size(100)
    .batch_timeout(Duration::from_millis(10))
    .build().await?;
```

### 3. Compression
```rust
let bridge = GhostBridge::builder()
    .enable_compression(true)
    .compression_level(6)
    .compression_threshold(1024)  // Only compress messages > 1KB
    .build().await?;
```

---

## 🛡️ Error Handling

### 1. Graceful Degradation
```rust
use gquic::bridge::resilience::{CircuitBreaker, RetryPolicy};

let circuit_breaker = CircuitBreaker::builder()
    .failure_threshold(5)
    .recovery_timeout(Duration::from_secs(30))
    .build();

let retry_policy = RetryPolicy::builder()
    .max_attempts(3)
    .base_delay(Duration::from_millis(100))
    .max_delay(Duration::from_secs(5))
    .exponential_backoff(2.0)
    .build();

bridge.register_circuit_breaker("walletd", circuit_breaker);
bridge.register_retry_policy("walletd", retry_policy);
```

### 2. Error Recovery
```rust
use gquic::bridge::error::{ErrorHandler, ErrorPolicy};

let error_handler = ErrorHandler::builder()
    .on_connection_error(ErrorPolicy::Retry)
    .on_timeout_error(ErrorPolicy::Fallback)
    .on_auth_error(ErrorPolicy::Reject)
    .build();

bridge.register_error_handler(error_handler);
```

---

## 📋 Deployment Guide

### 1. Docker Deployment
```dockerfile
FROM rust:1.75-slim as builder

WORKDIR /app
COPY . .
RUN cargo build --release --features grpc

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates
COPY --from=builder /app/target/release/ghostbridge /usr/local/bin/
COPY --from=builder /app/certs /etc/ghostbridge/certs/

EXPOSE 9090
CMD ["ghostbridge", "--config", "/etc/ghostbridge/config.toml"]
```

### 2. Kubernetes Deployment
```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: ghostbridge
spec:
  replicas: 3
  selector:
    matchLabels:
      app: ghostbridge
  template:
    metadata:
      labels:
        app: ghostbridge
    spec:
      containers:
      - name: ghostbridge
        image: ghostkellz/ghostbridge:latest
        ports:
        - containerPort: 9090
        env:
        - name: GHOSTBRIDGE_LOG_LEVEL
          value: "info"
        - name: GHOSTBRIDGE_BIND_ADDR
          value: "0.0.0.0:9090"
        resources:
          limits:
            memory: "512Mi"
            cpu: "500m"
          requests:
            memory: "256Mi"
            cpu: "250m"
        volumeMounts:
        - name: tls-certs
          mountPath: /etc/ghostbridge/certs
          readOnly: true
      volumes:
      - name: tls-certs
        secret:
          secretName: ghostbridge-tls
```

### 3. Production Configuration
```toml
[server]
bind_addr = "0.0.0.0:9090"
tls_cert = "/etc/ghostbridge/certs/server.crt"
tls_key = "/etc/ghostbridge/certs/server.key"
enable_metrics = true
metrics_addr = "0.0.0.0:9091"

[services]
walletd_endpoint = "walletd:8080"
ghostd_endpoint = "ghostd:8081"
zns_endpoint = "zns:8082"

[pool]
max_connections_per_endpoint = 100
max_connection_age = "1h"
max_idle_time = "5m"
cleanup_interval = "30s"

[security]
enable_mtls = true
client_ca_cert = "/etc/ghostbridge/certs/client-ca.crt"
require_client_auth = true

[observability]
enable_tracing = true
jaeger_endpoint = "http://jaeger:14268/api/traces"
sampling_ratio = 0.1
```

---

## 🧪 Testing

### 1. Integration Tests
```rust
#[cfg(test)]
mod integration_tests {
    use super::*;
    use tokio_test;
    
    #[tokio::test]
    async fn test_walletd_integration() {
        let bridge = GhostBridge::builder()
            .with_test_config()
            .build().await.unwrap();
        
        let client = bridge.test_client().await;
        
        let account = client.create_account(CreateAccountRequest {
            name: "test".to_string(),
            passphrase: "test".to_string(),
            key_type: KeyType::KeyTypeEd25519 as i32,
        }).await.unwrap();
        
        assert!(!account.into_inner().account_id.is_empty());
    }
}
```

### 2. Load Testing
```rust
#[tokio::test]
async fn test_concurrent_requests() {
    let bridge = GhostBridge::builder()
        .with_test_config()
        .build().await.unwrap();
    
    let client = bridge.test_client().await;
    
    let mut handles = vec![];
    for i in 0..1000 {
        let client = client.clone();
        handles.push(tokio::spawn(async move {
            client.get_balance(GetBalanceRequest {
                account_id: format!("account_{}", i),
                token_type: "MANA".to_string(),
            }).await
        }));
    }
    
    for handle in handles {
        handle.await.unwrap().unwrap();
    }
}
```

---

## 🔗 API Reference

### Core Types
- `GhostBridge`: Main bridge server
- `BridgeClient`: Client for connecting to bridge
- `BridgeConfig`: Configuration structure
- `ServiceRegistry`: Service discovery and routing
- `ConnectionPool`: Connection management
- `ProtocolHandler`: Custom protocol handler trait

### Error Types
- `BridgeError`: Main error type
- `ConnectionError`: Connection-specific errors
- `AuthError`: Authentication errors
- `ServiceError`: Service-specific errors

### Traits
- `ProtocolHandler`: Custom protocol implementation
- `ServiceProvider`: Service implementation
- `Authenticator`: Authentication provider
- `LoadBalancer`: Load balancing strategy

---

## 📚 Resources

- [GhostChain Documentation](https://docs.ghostchain.org)
- [QUIC Protocol Specification](https://tools.ietf.org/html/rfc9000)
- [gRPC Documentation](https://grpc.io/docs/)
- [Zig FFI Guide](https://ziglang.org/documentation/master/#C)

---

## 🤝 Contributing

1. Fork the repository
2. Create a feature branch
3. Add tests for new features
4. Ensure all tests pass
5. Update documentation
6. Submit a pull request

## 📄 License

MIT © 2025 CK Technology LLC

---

*Built with ❤️ for the GhostChain ecosystem*