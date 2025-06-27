# 🔗 HOWTO-INTEGRATION.md – Integrating with WalletD v0.2.0

This guide provides step-by-step instructions for integrating external crypto projects, libraries, and services with the GhostChain Wallet Daemon (`walletd`). Whether you're building in Rust, Zig, or other languages, this guide will help you connect to walletd's secure wallet infrastructure.

---

## 🎯 What You Can Integrate With

WalletD provides multiple integration points:

- **🔐 ZID Authentication** - Web5 identity management
- **✍️ Transaction Signing** - GhostChain + EVM transactions  
- **📡 gRPC API** - High-performance service interface
- **🌐 REST API** - HTTP-based wallet operations
- **📦 Zig FFI** - Direct library integration
- **🗄️ Ledger Access** - Transaction history and audit trails

---

## 🚀 Quick Start Integration

### 1. **REST API Integration (Easiest)**

Perfect for web apps, mobile apps, or any HTTP-capable service.

```bash
# Start walletd with REST API
cargo start
# OR
./target/release/walletd --config walletd.toml start
```

**Create a wallet:**
```bash
curl -X POST http://localhost:8080/wallets \
  -H "Content-Type: application/json" \
  -d '{
    "name": "MyProject Wallet",
    "account_type": "ed25519",
    "passphrase": "my-secure-passphrase"
  }'
```

**Send a transaction:**
```bash
curl -X POST http://localhost:8080/transactions \
  -H "Content-Type: application/json" \
  -d '{
    "from_wallet_id": "wallet-uuid",
    "to_address": "ghostchain:recipient-address",
    "amount": "1.5",
    "passphrase": "my-secure-passphrase"
  }'
```

### 2. **gRPC Integration (Recommended for Services)**

Best for microservices, daemons, and high-performance applications.

**Install gRPC tools:**
```bash
# For Rust projects
cargo add tonic tonic-build prost
```

**Use the proto file:**
```bash
# Copy walletd's proto definition
cp /path/to/walletd/proto/walletd.proto ./proto/
```

**Example Rust gRPC client:**
```rust
use tonic::Request;
use walletd::walletd::{wallet_service_client::WalletServiceClient, CreateWalletRequest};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = WalletServiceClient::connect("http://127.0.0.1:50051").await?;
    
    let request = Request::new(CreateWalletRequest {
        name: "My Service Wallet".to_string(),
        account_type: Some("ed25519".to_string()),
        passphrase: Some("secure-passphrase".to_string()),
        network: None,
    });
    
    let response = client.create_wallet(request).await?;
    println!("Wallet created: {:?}", response.into_inner());
    
    Ok(())
}
```

---

## 🔧 Language-Specific Integration Guides

### 🦀 **Rust Projects**

**Option A: Direct Dependency (if open-source)**
```toml
[dependencies]
walletd = { git = "https://github.com/ghostkellz/walletd", branch = "main" }
tokio = { version = "1.0", features = ["full"] }
tonic = "0.12"
```

**Option B: gRPC Client**
```rust
// In build.rs
fn main() {
    tonic_build::compile_protos("proto/walletd.proto")
        .unwrap_or_else(|e| panic!("Failed to compile protos {:?}", e));
}

// In your service
use crate::walletd::wallet_service_client::WalletServiceClient;

pub struct MyWalletService {
    walletd_client: WalletServiceClient<tonic::transport::Channel>,
}

impl MyWalletService {
    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let client = WalletServiceClient::connect("http://127.0.0.1:50051").await?;
        Ok(Self { walletd_client: client })
    }
    
    pub async fn create_user_wallet(&mut self, user_id: &str, passphrase: &str) -> Result<String, Box<dyn std::error::Error>> {
        let request = CreateWalletRequest {
            name: format!("User-{}", user_id),
            account_type: Some("ed25519".to_string()),
            passphrase: Some(passphrase.to_string()),
            network: None,
        };
        
        let response = self.walletd_client.create_wallet(request).await?;
        Ok(response.into_inner().wallet.unwrap().id)
    }
}
```

### ⚡ **Zig Projects**

**FFI Integration (Direct Library Linking)**

```zig
// walletd_ffi.zig - FFI bindings for walletd
const std = @import("std");

// External C functions from libwalletd_ffi
extern fn walletd_create_wallet(name: [*:0]const u8, account_type: [*:0]const u8, passphrase: [*:0]const u8) [*:0]u8;
extern fn walletd_sign_transaction(wallet_id: [*:0]const u8, to_addr: [*:0]const u8, amount: [*:0]const u8, passphrase: [*:0]const u8) [*:0]u8;
extern fn walletd_free_string(ptr: [*:0]u8) void;

pub const WalletD = struct {
    pub fn createWallet(name: []const u8, account_type: []const u8, passphrase: []const u8) ![]const u8 {
        const c_name = try std.cstr.addNullByte(std.heap.c_allocator, name);
        defer std.heap.c_allocator.free(c_name);
        
        const c_account_type = try std.cstr.addNullByte(std.heap.c_allocator, account_type);
        defer std.heap.c_allocator.free(c_account_type);
        
        const c_passphrase = try std.cstr.addNullByte(std.heap.c_allocator, passphrase);
        defer std.heap.c_allocator.free(c_passphrase);
        
        const result = walletd_create_wallet(c_name.ptr, c_account_type.ptr, c_passphrase.ptr);
        defer walletd_free_string(result);
        
        return std.mem.span(result);
    }
    
    pub fn signTransaction(wallet_id: []const u8, to_addr: []const u8, amount: []const u8, passphrase: []const u8) ![]const u8 {
        // Similar implementation...
        const c_wallet_id = try std.cstr.addNullByte(std.heap.c_allocator, wallet_id);
        defer std.heap.c_allocator.free(c_wallet_id);
        
        // ... convert other parameters ...
        
        const result = walletd_sign_transaction(c_wallet_id.ptr, c_to_addr.ptr, c_amount.ptr, c_passphrase.ptr);
        defer walletd_free_string(result);
        
        return std.mem.span(result);
    }
};
```

**Build script (build.zig):**
```zig
const std = @import("std");

pub fn build(b: *std.Build) void {
    const target = b.standardTargetOptions(.{});
    const optimize = b.standardOptimizeOption(.{});

    const exe = b.addExecutable(.{
        .name = "my-zig-wallet",
        .root_source_file = .{ .path = "src/main.zig" },
        .target = target,
        .optimize = optimize,
    });

    // Link against walletd FFI library
    exe.linkSystemLibrary("walletd_ffi");
    exe.addLibraryPath(.{ .path = "/path/to/walletd/target/release" });
    
    b.installArtifact(exe);
}
```

### 🐍 **Python Projects**

**HTTP REST Client:**
```python
import requests
import json
from typing import Dict, Optional

class WalletDClient:
    def __init__(self, base_url: str = "http://localhost:8080"):
        self.base_url = base_url
        self.session = requests.Session()
    
    def create_wallet(self, name: str, account_type: str = "ed25519", 
                     passphrase: Optional[str] = None) -> Dict:
        """Create a new wallet"""
        payload = {
            "name": name,
            "account_type": account_type
        }
        if passphrase:
            payload["passphrase"] = passphrase
            
        response = self.session.post(f"{self.base_url}/wallets", json=payload)
        response.raise_for_status()
        return response.json()
    
    def send_transaction(self, from_wallet_id: str, to_address: str, 
                        amount: str, passphrase: str) -> Dict:
        """Send a transaction"""
        payload = {
            "from_wallet_id": from_wallet_id,
            "to_address": to_address,
            "amount": amount,
            "passphrase": passphrase
        }
        
        response = self.session.post(f"{self.base_url}/transactions", json=payload)
        response.raise_for_status()
        return response.json()
    
    def get_wallet_balance(self, wallet_id: str) -> Dict:
        """Get wallet balance"""
        response = self.session.get(f"{self.base_url}/wallets/{wallet_id}/balances")
        response.raise_for_status()
        return response.json()

# Usage example
if __name__ == "__main__":
    client = WalletDClient()
    
    # Create wallet
    wallet = client.create_wallet("Python Test Wallet", passphrase="test-passphrase")
    wallet_id = wallet["data"]["id"]
    print(f"Created wallet: {wallet_id}")
    
    # Send transaction
    tx = client.send_transaction(
        from_wallet_id=wallet_id,
        to_address="ghostchain:recipient-address",
        amount="1.0",
        passphrase="test-passphrase"
    )
    print(f"Transaction sent: {tx['data']['transaction_hash']}")
```

**gRPC Client (using grpcio):**
```python
import grpc
from walletd_pb2 import CreateWalletRequest, SendTransactionRequest
from walletd_pb2_grpc import WalletServiceStub

class WalletDGrpcClient:
    def __init__(self, server_address: str = "localhost:50051"):
        self.channel = grpc.insecure_channel(server_address)
        self.stub = WalletServiceStub(self.channel)
    
    def create_wallet(self, name: str, passphrase: str) -> str:
        request = CreateWalletRequest(
            name=name,
            account_type="ed25519",
            passphrase=passphrase
        )
        response = self.stub.CreateWallet(request)
        return response.wallet.id
    
    def send_transaction(self, from_wallet_id: str, to_address: str, 
                        amount: str, passphrase: str) -> str:
        request = SendTransactionRequest(
            from_wallet_id=from_wallet_id,
            to_address=to_address,
            amount=amount,
            passphrase=passphrase
        )
        response = self.stub.SendTransaction(request)
        return response.transaction_hash
```

### 🌐 **JavaScript/TypeScript Projects**

**REST API Client:**
```typescript
interface WalletResponse {
  success: boolean;
  data: {
    id: string;
    name: string;
    account_type: string;
    address: string;
    public_key: string;
  };
}

interface TransactionResponse {
  success: boolean;
  data: {
    transaction_hash: string;
    status: string;
  };
}

class WalletDClient {
  private baseUrl: string;

  constructor(baseUrl = 'http://localhost:8080') {
    this.baseUrl = baseUrl;
  }

  async createWallet(name: string, accountType = 'ed25519', passphrase?: string): Promise<WalletResponse> {
    const response = await fetch(`${this.baseUrl}/wallets`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        name,
        account_type: accountType,
        ...(passphrase && { passphrase })
      })
    });

    if (!response.ok) {
      throw new Error(`HTTP error! status: ${response.status}`);
    }

    return response.json();
  }

  async sendTransaction(
    fromWalletId: string,
    toAddress: string,
    amount: string,
    passphrase: string
  ): Promise<TransactionResponse> {
    const response = await fetch(`${this.baseUrl}/transactions`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        from_wallet_id: fromWalletId,
        to_address: toAddress,
        amount,
        passphrase
      })
    });

    if (!response.ok) {
      throw new Error(`HTTP error! status: ${response.status}`);
    }

    return response.json();
  }

  async getWalletBalance(walletId: string) {
    const response = await fetch(`${this.baseUrl}/wallets/${walletId}/balances`);
    
    if (!response.ok) {
      throw new Error(`HTTP error! status: ${response.status}`);
    }

    return response.json();
  }
}

// Usage example
const client = new WalletDClient();

async function example() {
  try {
    // Create wallet
    const wallet = await client.createWallet('JS Test Wallet', 'ed25519', 'my-passphrase');
    console.log('Created wallet:', wallet.data.id);

    // Send transaction
    const tx = await client.sendTransaction(
      wallet.data.id,
      'ghostchain:recipient-address',
      '1.0',
      'my-passphrase'
    );
    console.log('Transaction sent:', tx.data.transaction_hash);

  } catch (error) {
    console.error('Error:', error);
  }
}
```

### 🐹 **Go Projects**

**REST Client:**
```go
package main

import (
    "bytes"
    "encoding/json"
    "fmt"
    "net/http"
)

type WalletDClient struct {
    BaseURL string
    Client  *http.Client
}

type CreateWalletRequest struct {
    Name        string  `json:"name"`
    AccountType string  `json:"account_type"`
    Passphrase  *string `json:"passphrase,omitempty"`
}

type WalletResponse struct {
    Success bool `json:"success"`
    Data    struct {
        ID         string `json:"id"`
        Name       string `json:"name"`
        Address    string `json:"address"`
        PublicKey  string `json:"public_key"`
    } `json:"data"`
}

func NewWalletDClient(baseURL string) *WalletDClient {
    return &WalletDClient{
        BaseURL: baseURL,
        Client:  &http.Client{},
    }
}

func (c *WalletDClient) CreateWallet(name, accountType string, passphrase *string) (*WalletResponse, error) {
    req := CreateWalletRequest{
        Name:        name,
        AccountType: accountType,
        Passphrase:  passphrase,
    }

    jsonData, err := json.Marshal(req)
    if err != nil {
        return nil, err
    }

    resp, err := c.Client.Post(c.BaseURL+"/wallets", "application/json", bytes.NewBuffer(jsonData))
    if err != nil {
        return nil, err
    }
    defer resp.Body.Close()

    var walletResp WalletResponse
    err = json.NewDecoder(resp.Body).Decode(&walletResp)
    return &walletResp, err
}

func main() {
    client := NewWalletDClient("http://localhost:8080")
    
    passphrase := "my-go-passphrase"
    wallet, err := client.CreateWallet("Go Test Wallet", "ed25519", &passphrase)
    if err != nil {
        panic(err)
    }
    
    fmt.Printf("Created wallet: %s\n", wallet.Data.ID)
}
```

---

## 🔌 Advanced Integration Patterns

### 1. **Microservice Integration**

**Service Discovery Pattern:**
```yaml
# docker-compose.yml
version: '3.8'
services:
  walletd:
    build: ./walletd
    ports:
      - "8080:8080"
      - "50051:50051"
    environment:
      - DATABASE_PATH=/data/walletd.db
    volumes:
      - wallet_data:/data

  my-service:
    build: ./my-service
    environment:
      - WALLETD_GRPC_URL=walletd:50051
      - WALLETD_REST_URL=http://walletd:8080
    depends_on:
      - walletd

volumes:
  wallet_data:
```

### 2. **Event-Driven Integration**

**Listen for transaction events:**
```rust
use tokio_stream::StreamExt;

async fn listen_for_transaction_events() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = WalletServiceClient::connect("http://127.0.0.1:50051").await?;
    
    let request = tonic::Request::new(TransactionEventsRequest {
        wallet_id: "your-wallet-id".to_string(),
    });
    
    let mut stream = client.get_transaction_events(request).await?.into_inner();
    
    while let Some(event) = stream.next().await {
        match event {
            Ok(tx_event) => {
                println!("New transaction: {:?}", tx_event);
                // Handle transaction event in your application
            },
            Err(e) => {
                eprintln!("Stream error: {}", e);
                break;
            }
        }
    }
    
    Ok(())
}
```

### 3. **Authentication Integration**

**ZID-based authentication:**
```rust
use walletd::auth::{AuthManager, Identity};

pub struct MyAuthenticatedService {
    auth_manager: AuthManager,
}

impl MyAuthenticatedService {
    pub async fn authenticate_user(&self, passphrase: &str) -> Result<Identity, AuthError> {
        let identity = self.auth_manager.generate_identity_from_passphrase(passphrase).await?;
        
        // Verify identity with challenge-response
        let challenge = self.auth_manager.create_challenge().await?;
        let signature = self.auth_manager.sign_data(&identity, &challenge.data).await?;
        
        if self.auth_manager.verify_challenge_response(&challenge, &signature).await? {
            Ok(identity)
        } else {
            Err(AuthError::InvalidChallenge)
        }
    }
}
```

### 4. **Custom Transaction Types**

**Extend walletd for custom transaction types:**
```rust
use walletd::signer::{TransactionSigner, UnsignedTransaction, TransactionType};

#[derive(Debug, Clone)]
pub enum CustomTransactionType {
    Standard,
    MultiSig,
    SmartContract,
    NFTMint,
}

impl From<CustomTransactionType> for TransactionType {
    fn from(custom_type: CustomTransactionType) -> Self {
        match custom_type {
            CustomTransactionType::Standard => TransactionType::Transfer,
            CustomTransactionType::MultiSig => TransactionType::MultiSig,
            CustomTransactionType::SmartContract => TransactionType::ContractCall,
            CustomTransactionType::NFTMint => TransactionType::ContractDeploy,
        }
    }
}

pub async fn sign_custom_transaction(
    signer: &TransactionSigner,
    from: &str,
    to: &str,
    amount: &str,
    custom_type: CustomTransactionType,
    passphrase: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let tx = UnsignedTransaction {
        from: from.to_string(),
        to: to.to_string(),
        amount: amount.parse()?,
        transaction_type: custom_type.into(),
        data: vec![], // Custom transaction data
        gas_limit: 21000,
        gas_price: 20_000_000_000,
        nonce: 0, // Get from ledger
    };
    
    let signed_tx = signer.sign_transaction(&tx, passphrase).await?;
    Ok(signed_tx)
}
```

---

## 🚀 Production Deployment

### 1. **Configuration Management**

**Environment-based configuration:**
```toml
# walletd-production.toml
[network]
chain_id = 1337
network_name = "ghostchain-mainnet"
default_gas_limit = 21000
default_gas_price = 20000000000

[security]
require_auth = true
session_timeout_seconds = 1800  # 30 minutes
max_concurrent_sessions = 1000

[features]
enable_evm = true
enable_cli = false  # Disable CLI in production

# Use Unix socket for local services
grpc_bind_address = "unix:///var/run/walletd/walletd.sock"
api_bind_address = "127.0.0.1:8080"

# External service endpoints
ghostd_endpoint = "https://ghostd.yourcompany.com:50052"

# Database with backup
database_path = "/var/lib/walletd/walletd.db"
database_backup_path = "/var/lib/walletd/backups"
```

### 2. **Health Checks**

**Service health monitoring:**
```bash
# Check if walletd is responding
curl -f http://localhost:8080/health || exit 1

# Check gRPC health
grpc_health_probe -addr=localhost:50051 || exit 1
```

### 3. **Monitoring Integration**

**Prometheus metrics:**
```rust
use prometheus::{Counter, Histogram, Registry};

pub struct WalletMetrics {
    pub wallet_creates: Counter,
    pub transaction_signs: Counter,
    pub auth_attempts: Counter,
    pub request_duration: Histogram,
}

impl WalletMetrics {
    pub fn new() -> Self {
        Self {
            wallet_creates: Counter::new("walletd_wallet_creates_total", "Total wallet creations").unwrap(),
            transaction_signs: Counter::new("walletd_transaction_signs_total", "Total transaction signs").unwrap(),
            auth_attempts: Counter::new("walletd_auth_attempts_total", "Total auth attempts").unwrap(),
            request_duration: Histogram::new("walletd_request_duration_seconds", "Request duration").unwrap(),
        }
    }
    
    pub fn register(&self, registry: &Registry) -> Result<(), prometheus::Error> {
        registry.register(Box::new(self.wallet_creates.clone()))?;
        registry.register(Box::new(self.transaction_signs.clone()))?;
        registry.register(Box::new(self.auth_attempts.clone()))?;
        registry.register(Box::new(self.request_duration.clone()))?;
        Ok(())
    }
}
```

---

## 🔒 Security Best Practices

### 1. **API Key Authentication**
```bash
# Add API key to requests
curl -X POST http://localhost:8080/wallets \
  -H "Authorization: Bearer your-api-key" \
  -H "Content-Type: application/json" \
  -d '{"name": "Secure Wallet"}'
```

### 2. **Rate Limiting**
```rust
use tower::ServiceBuilder;
use tower_http::limit::RateLimitLayer;

let app = Router::new()
    .route("/wallets", post(create_wallet))
    .layer(
        ServiceBuilder::new()
            .layer(RateLimitLayer::new(10, Duration::from_secs(60))) // 10 requests per minute
            .into_inner()
    );
```

### 3. **Input Validation**
```rust
use validator::{Validate, ValidationError};

#[derive(Validate, Deserialize)]
pub struct CreateWalletRequest {
    #[validate(length(min = 1, max = 100))]
    pub name: String,
    
    #[validate(custom = "validate_account_type")]
    pub account_type: String,
    
    #[validate(length(min = 12, max = 256))]
    pub passphrase: Option<String>,
}

fn validate_account_type(account_type: &str) -> Result<(), ValidationError> {
    match account_type {
        "ed25519" | "secp256k1" | "secp256r1" => Ok(()),
        _ => Err(ValidationError::new("invalid_account_type")),
    }
}
```

---

## 🧪 Testing Your Integration

### 1. **Integration Test Template**

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;
    use tokio;
    
    #[tokio::test]
    async fn test_wallet_creation_and_transaction() {
        // Start test walletd instance
        let walletd = start_test_walletd().await;
        
        // Create client
        let mut client = WalletServiceClient::connect("http://127.0.0.1:50051").await.unwrap();
        
        // Test wallet creation
        let wallet_req = CreateWalletRequest {
            name: "Test Wallet".to_string(),
            account_type: Some("ed25519".to_string()),
            passphrase: Some("test-passphrase".to_string()),
            network: None,
        };
        
        let wallet_resp = client.create_wallet(wallet_req).await.unwrap();
        let wallet_id = wallet_resp.into_inner().wallet.unwrap().id;
        
        // Test transaction
        let tx_req = SendTransactionRequest {
            from_wallet_id: wallet_id,
            to_address: "ghostchain:test-recipient".to_string(),
            amount: "1.0".to_string(),
            passphrase: "test-passphrase".to_string(),
            gas_limit: Some(21000),
            gas_price: Some(20_000_000_000),
        };
        
        let tx_resp = client.send_transaction(tx_req).await.unwrap();
        assert!(!tx_resp.into_inner().transaction_hash.is_empty());
        
        // Cleanup
        walletd.shutdown().await;
    }
}
```

### 2. **Load Testing**

```bash
# Install k6 for load testing
npm install -g k6

# Create load test script (load-test.js)
import http from 'k6/http';
import { check } from 'k6';

export let options = {
  stages: [
    { duration: '30s', target: 10 },
    { duration: '1m', target: 50 },
    { duration: '30s', target: 0 },
  ],
};

export default function() {
  const payload = JSON.stringify({
    name: `Load Test Wallet ${Math.random()}`,
    account_type: 'ed25519',
    passphrase: 'load-test-passphrase'
  });

  const response = http.post('http://localhost:8080/wallets', payload, {
    headers: { 'Content-Type': 'application/json' },
  });

  check(response, {
    'status is 200': (r) => r.status === 200,
    'wallet created': (r) => JSON.parse(r.body).success === true,
  });
}

# Run load test
k6 run load-test.js
```

---

## 📋 Integration Checklist

Use this checklist to ensure your integration is complete and secure:

### 🔧 **Basic Integration**
- [ ] Can connect to walletd (REST or gRPC)
- [ ] Can create wallets
- [ ] Can send transactions
- [ ] Can query balances
- [ ] Error handling implemented
- [ ] Logging configured

### 🔒 **Security**
- [ ] Passphrases handled securely (never logged)
- [ ] API keys/authentication configured
- [ ] Rate limiting implemented
- [ ] Input validation added
- [ ] HTTPS/TLS enabled for production

### 🧪 **Testing**
- [ ] Unit tests written
- [ ] Integration tests with walletd
- [ ] Load testing completed
- [ ] Error scenarios tested
- [ ] Security testing done

### 🚀 **Production**
- [ ] Configuration management
- [ ] Health checks implemented
- [ ] Monitoring/metrics added
- [ ] Backup strategy defined
- [ ] Deployment automation ready

### 📚 **Documentation**
- [ ] API usage documented
- [ ] Configuration options explained
- [ ] Troubleshooting guide created
- [ ] Security guidelines documented

---

## 🆘 Troubleshooting

### Common Issues

**1. Connection Refused**
```bash
# Check if walletd is running
ps aux | grep walletd
# Check ports
netstat -tulpn | grep -E "(8080|50051)"
# Check logs
journalctl -u walletd -f
```

**2. gRPC Errors**
```bash
# Test gRPC connectivity
grpcurl -plaintext localhost:50051 list
grpcurl -plaintext localhost:50051 walletd.WalletService/GetWallets
```

**3. FFI Issues**
```bash
# Check library linking
ldd /path/to/your/binary
# Check library path
export LD_LIBRARY_PATH=/path/to/walletd/target/release:$LD_LIBRARY_PATH
```

**4. Permission Issues**
```bash
# Check file permissions
ls -la /var/lib/walletd/
# Fix permissions
sudo chown -R walletd:walletd /var/lib/walletd/
sudo chmod 600 /var/lib/walletd/walletd.db
```

---

## 🎯 Next Steps

1. **Choose your integration method** (REST, gRPC, or FFI)
2. **Follow the language-specific guide** for your project
3. **Implement authentication and security**
4. **Add comprehensive testing**
5. **Deploy with monitoring**
6. **Join the GhostChain community** for support and updates

---

## 📞 Support & Community

- **Issues**: [GitHub Issues](https://github.com/ghostkellz/walletd/issues)
- **Discussions**: [GitHub Discussions](https://github.com/ghostkellz/walletd/discussions)
- **Documentation**: [docs.ghostchain.io](https://docs.ghostchain.io)
- **Community**: [Discord](https://discord.gg/ghostchain)

---

**🔐 Happy integrating with WalletD!** 👻

*This guide is part of the GhostChain ecosystem. For more integration guides, see the [GhostChain Integration Matrix](./JUNE_INTEGRATION.md).*
