![GhostBridge Logo](assets/ghostbridge_logo.png)

# 👻️ GhostBridge ⚡ – Cross-Chain Bridge Infrastructure

## 🌉 High-Performance Cross-Chain Bridge

> Production-ready **Rust-based bridge** with safe FFI abstraction layer for **GhostPlane (Zig)** integration, enabling seamless cross-chain communication and domain resolution.

---

![Built with Zig](https://img.shields.io/badge/Built%20with-Zig-f7a41d?logo=zig&logoColor=black)
![Built with Rust](https://img.shields.io/badge/Built%20with-Rust-de3423?logo=rust&logoColor=white)
![gRPC Bridge](https://img.shields.io/badge/gRPC-Bridge-00C7B7?logo=grpc&logoColor=white)
![DNS over QUIC](https://img.shields.io/badge/DNS-over--QUIC-1976D2?logo=cloudflare&logoColor=white)
![ENS Support](https://img.shields.io/badge/ENS-Support-4A90E2?logo=ethereum&logoColor=white)
![Built for Web3](https://img.shields.io/badge/Built%20for-Web3-29B6F6?logo=web3dotjs&logoColor=white)
![Web2 Compatible](https://img.shields.io/badge/Web2-Compatible-9E9E9E?logo=internetexplorer&logoColor=white)

---

## 🔧 Architecture: **Rust-Based Bridge**

### **Benefits of Rust-based GhostBridge:**

1. **Better integration** - Since your main codebase is Rust, having GhostBridge in Rust means seamless integration with ghostchain-shared types and GCRYPT
2. **Safer FFI boundary** - Rust has excellent FFI safety features and can better manage the unsafe boundary when calling into Zig
3. **Single build system** - Cargo can manage the Rust side, and you only need to deal with Zig's build system for GhostPlane
4. **Type consistency** - Your Rust types can be the source of truth, with Zig bindings generated from them

### **Architecture Pattern:**
```
GhostChain (Rust)
    ↓
GhostBridge (Rust) - handles FFI safely
    ↓
FFI Boundary (C ABI)
    ↓
GhostPlane (Zig)
```

This way GhostBridge becomes your Rust-side abstraction layer that:
- Exposes safe Rust APIs to the rest of GhostChain
- Handles all unsafe FFI calls to Zig
- Manages memory safety across the boundary
- Provides async wrappers for Zig functions

---

## 🏗️ Project Structure

```rust
// ghostbridge/src/lib.rs
pub mod ghostplane {
    // Safe Rust API
    pub async fn submit_to_l2(tx: Transaction) -> Result<Receipt> {
        // Handle FFI to Zig internally
    }
}
```

```
📁 ghostbridge/
├── 📁 src/                      # Rust bridge implementation
│   ├── lib.rs                   # Main library interface
│   ├── ghostplane/              # FFI abstraction layer
│   │   ├── mod.rs               # Safe Rust APIs
│   │   ├── ffi.rs               # Unsafe FFI bindings
│   │   └── types.rs             # Shared type definitions
│   ├── bridge/                  # Core bridge logic
│   │   ├── mod.rs               # Bridge coordination
│   │   ├── cross_chain.rs       # Cross-chain communication
│   │   └── validator.rs         # Transaction validation
│   └── main.rs                  # Binary entry point
├── 📁 archive/                  # Legacy Zig implementation (reference)
├── 📁 assets/                   # Project assets
│   └── ghostbridge_logo.png    # GhostBridge logo
├── Cargo.toml                   # Rust dependencies
└── README.md                    # This file
```

---

## 🚀 Protocol Definitions

### **Core gRPC Services**

```protobuf
// proto/ghostchain.proto
syntax = "proto3";
package ghostchain.v1;

// Blockchain state queries for DNS resolution
service GhostChainService {
  // Domain resolution
  rpc ResolveDomain(DomainQuery) returns (DomainResponse);
  rpc RegisterDomain(DomainRegistration) returns (TransactionResponse);
  
  // Account queries
  rpc GetAccount(AccountQuery) returns (AccountResponse);
  rpc GetBalance(BalanceQuery) returns (BalanceResponse);
  
  // Block queries  
  rpc GetBlock(BlockQuery) returns (BlockResponse);
  rpc GetLatestBlock(Empty) returns (BlockResponse);
  
  // Real-time subscriptions
  rpc SubscribeBlocks(Empty) returns (stream BlockResponse);
  rpc SubscribeDomainChanges(DomainSubscription) returns (stream DomainEvent);
}

message DomainQuery {
  string domain = 1;
  repeated string record_types = 2; // A, AAAA, MX, TXT, etc.
}

message DomainResponse {
  string domain = 1;
  repeated DNSRecord records = 2;
  string owner_id = 3;           // GhostID
  bytes signature = 4;           // Ed25519 signature
  uint64 timestamp = 5;
  uint32 ttl = 6;
}

message DNSRecord {
  string type = 1;               // A, AAAA, MX, TXT
  string value = 2;              // IP address, hostname, text
  uint32 priority = 3;           // For MX records
  uint32 ttl = 4;
}
```

```protobuf
// proto/ghostdns.proto  
syntax = "proto3";
package ghostdns.v1;

// DNS server management and statistics
service GhostDNSService {
  rpc GetStats(Empty) returns (DNSStats);
  rpc FlushCache(CacheFlushRequest) returns (Empty);
  rpc UpdateZone(ZoneUpdate) returns (Empty);
  rpc GetCacheStatus(Empty) returns (CacheStats);
}

message DNSStats {
  uint64 queries_total = 1;
  uint64 cache_hits = 2;
  uint64 blockchain_queries = 3;
  double avg_response_time_ms = 4;
  uint64 active_connections = 5;
}
```

---

## ⚡ Rust Bridge Implementation

### **Safe FFI Abstraction Layer**

```rust
// src/ghostplane/mod.rs
use std::ffi::{c_void, CString};
use std::sync::Arc;
use tokio::sync::RwLock;

// Safe Rust API that other GhostChain components use
pub struct GhostPlane {
    handle: Arc<RwLock<*mut c_void>>,
}

impl GhostPlane {
    pub async fn new(config: &Config) -> Result<Self> {
        let handle = unsafe {
            // Call into Zig via FFI
            let config_str = CString::new(serde_json::to_string(config)?)?;
            ghostplane_init(config_str.as_ptr())
        };

        Ok(Self {
            handle: Arc::new(RwLock::new(handle)),
        })
    }

    pub async fn submit_transaction(&self, tx: Transaction) -> Result<Receipt> {
        let handle = self.handle.read().await;

        // Serialize transaction for FFI
        let tx_bytes = tx.to_bytes()?;
        let receipt_ptr = unsafe {
            ghostplane_submit_tx(
                *handle,
                tx_bytes.as_ptr(),
                tx_bytes.len() as u32,
            )
        };

        // Safely convert receipt back to Rust type
        let receipt = unsafe { Receipt::from_ffi(receipt_ptr)? };

        Ok(receipt)
    }

    pub async fn query_state(&self, key: &[u8]) -> Result<Vec<u8>> {
        let handle = self.handle.read().await;

        let mut result_len: u32 = 0;
        let result_ptr = unsafe {
            ghostplane_query_state(
                *handle,
                key.as_ptr(),
                key.len() as u32,
                &mut result_len,
            )
        };

        // Copy data from FFI and free Zig memory
        let result = unsafe {
            let slice = std::slice::from_raw_parts(result_ptr, result_len as usize);
            let vec = slice.to_vec();
            ghostplane_free(result_ptr as *mut c_void);
            vec
        };

        Ok(result)
    }
}

// FFI declarations (linked to Zig implementation)
extern "C" {
    fn ghostplane_init(config: *const i8) -> *mut c_void;
    fn ghostplane_submit_tx(handle: *mut c_void, tx: *const u8, len: u32) -> *const u8;
    fn ghostplane_query_state(handle: *mut c_void, key: *const u8, len: u32, out_len: *mut u32) -> *const u8;
    fn ghostplane_free(ptr: *mut c_void);
}
```

### **Memory-Safe Type Conversion**

```rust
// src/ghostplane/types.rs
use serde::{Deserialize, Serialize};
use std::ffi::CStr;

// Rust types that match Zig structures
#[repr(C)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub from: [u8; 32],
    pub to: [u8; 32],
    pub value: u64,
    pub data: Vec<u8>,
    pub nonce: u64,
    pub signature: [u8; 64],
}

impl Transaction {
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        bincode::serialize(self).map_err(Into::into)
    }

    pub unsafe fn from_ffi(ptr: *const u8) -> Result<Self> {
        // Read length prefix
        let len = *(ptr as *const u32);
        let data = std::slice::from_raw_parts(ptr.add(4), len as usize);

        bincode::deserialize(data).map_err(Into::into)
    }
}

#[repr(C)]
pub struct Receipt {
    pub tx_hash: [u8; 32],
    pub block_number: u64,
    pub success: bool,
    pub gas_used: u64,
}

impl Receipt {
    pub unsafe fn from_ffi(ptr: *const u8) -> Result<Self> {
        // Safe deserialization with bounds checking
        let receipt_ptr = ptr as *const Receipt;
        Ok((*receipt_ptr).clone())
    }
}
```

---

## 🦀 Integration with GhostChain

### **Bridge Module Integration**

```rust
// src/bridge/mod.rs
use crate::ghostplane::GhostPlane;
use ghostchain_shared::types::{Transaction, Block};
use std::sync::Arc;

pub struct GhostBridge {
    ghostplane: Arc<GhostPlane>,
    validator: TransactionValidator,
    metrics: BridgeMetrics,
}

impl GhostBridge {
    pub async fn new(config: BridgeConfig) -> Result<Self> {
        let ghostplane = Arc::new(GhostPlane::new(&config.ghostplane).await?);

        Ok(Self {
            ghostplane,
            validator: TransactionValidator::new(config.validation_rules),
            metrics: BridgeMetrics::new(),
        })
    }

    pub async fn bridge_transaction(&self, tx: Transaction) -> Result<BridgeReceipt> {
        // Validate transaction
        self.validator.validate(&tx)?;

        // Record metrics
        self.metrics.record_bridge_attempt();

        // Submit to L2 via GhostPlane
        let receipt = self.ghostplane.submit_transaction(tx).await?;

        // Record success
        self.metrics.record_bridge_success();

        Ok(BridgeReceipt {
            l1_tx_hash: tx.hash(),
            l2_receipt: receipt,
            bridged_at: std::time::SystemTime::now(),
        })
    }

    pub async fn query_cross_chain_state(&self, chain_id: u32, key: &[u8]) -> Result<Vec<u8>> {
        // Route to appropriate L2 via GhostPlane
        match chain_id {
            1 => self.ghostplane.query_state(key).await,
            _ => Err(Error::UnsupportedChain(chain_id)),
        }
    }
}
```

### **Using GhostBridge in Your Application**

```rust
// In your GhostChain Cargo.toml
[dependencies]
ghostbridge = { path = "../ghostbridge" }
ghostchain-shared = { path = "../ghostchain-shared" }
tokio = { version = "1", features = ["full"] }

// In your application
use ghostbridge::GhostBridge;
use ghostchain_shared::types::Transaction;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize bridge
    let bridge = GhostBridge::new(config).await?;

    // Bridge a transaction to L2
    let tx = Transaction::new(/* ... */);
    let receipt = bridge.bridge_transaction(tx).await?;

    println!("Transaction bridged: {:?}", receipt);

    Ok(())
}
```

---

## 🚀 Performance Optimizations

### **Connection Pooling & Caching**

```zig
pub const ConnectionPool = struct {
    connections: []Connection,
    available: std.atomic.Atomic(u32),
    
    pub fn getConnection(self: *ConnectionPool) !*Connection {
        // Round-robin connection selection
        const idx = self.available.fetchAdd(1, .SeqCst) % self.connections.len;
        return &self.connections[idx];
    }
};

pub const ResponseCache = struct {
    entries: std.HashMap(u64, CachedResponse, std.hash_map.AutoContext, std.heap.page_allocator),
    
    pub fn get(self: *ResponseCache, request_hash: u64) ?CachedResponse {
        return self.entries.get(request_hash);
    }
};
```

---

## 📊 Performance Targets

### **Latency Goals**
- **DNS Query → Blockchain**: <5ms average
- **gRPC Call Overhead**: <100μs  
- **Serialization**: <50μs per message
- **Connection Establishment**: <1ms

### **Throughput Goals**
- **Concurrent Connections**: 10,000+
- **Requests/Second**: 50,000+ per core
- **Memory Usage**: <512MB for 10k connections
- **CPU Usage**: <30% at max throughput

---

## 🔧 Development Timeline

### **Week 1: Foundation**
- [ ] gRPC protocol definitions
- [ ] Basic Zig server skeleton  
- [ ] Rust client library structure
- [ ] Build system setup

### **Week 2: Core Implementation**
- [ ] Protobuf serialization in Zig
- [ ] Domain resolution service
- [ ] Connection pooling
- [ ] Basic error handling

### **Week 3: Integration**
- [ ] GhostChain integration
- [ ] ZNS + CNS integration (web3 + web3 dns) 
- [ ] End-to-end testing
- [ ] Performance benchmarking

### **Week 4: Optimization**
- [ ] Response caching
- [ ] Connection multiplexing
- [ ] Load testing
- [ ] Production hardening

---

🌐 DNS Resolution Pipeline

GhostBridge powers real-time DNS resolution over gRPC, acting as the glue between:

🧠 Zig-based GhostDNS resolver (DoQ/QUIC/HTTP3)

🔗 Rust-based GhostChain node (domain ↔ identity ↔ ledger)

🌎 Web2/Web3 clients (browsers, VPNs, CLI

### 🌐 DNS Resolution Pipeline

GhostBridge enables seamless DNS lookups by routing Web2/Web3 client queries through a high-speed Zig resolver to the Rust-based GhostChain backend. This unlocks real-time domain → identity → ownership resolution over QUIC, DoQ, and HTTP/3.

```text
+-----------+     gRPC     +----------------+     Ledger Lookup    +------------------+
| Web Client| ─────────▶  |  GhostBridge   | ───────────────────▶ |  GhostChain Node |
+-----------+             +----------------+                       +------------------+
                          ▲         ▲
                          |         |
               DoQ / QUIC / HTTP3   |
                          |         |
                     +--------+     |
                     |GhostDNS| ◀───┘
                     +--------+
```

---

## 🚀 Deployment Strategy

### **Development**
```bash
# Terminal 1: Start Zig bridge server
cd ghostbridge/zig-server
zig build run -- --bind 127.0.0.1:9090

# Terminal 2: Start Rust blockchain node  
cd ghostchain
cargo run -- node --bridge-endpoint http://127.0.0.1:9090

# Terminal 3: Start Zig DNS server
cd ghostdns  
zig build run -- --bridge-endpoint http://127.0.0.1:9090
```

### **Production**
```yaml
# docker-compose.yml
version: "3.8"
services:
  ghostbridge:
    build: ./ghostbridge/zig-server
    ports: ["9090:9090"]
    
  ghostchain:
    build: ./ghostchain
    environment:
      - BRIDGE_ENDPOINT=http://ghostbridge:9090
    depends_on: [ghostbridge]
    
  ghostdns:
    build: ./ghostdns
    environment:  
      - BRIDGE_ENDPOINT=http://ghostbridge:9090
    depends_on: [ghostbridge]
    ports: ["53:53/udp"]
```

This architecture gives you the best of both worlds: Zig's performance for the bridge layer and Rust's ecosystem for blockchain logic, with type-safe gRPC communication between them.
