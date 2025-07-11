# 🧠 ZVM TODO – GhostChain Smart Contract Runtime

**Status**: Active  
**Author**: ghostkellz  
**Target Version**: `zcrypto v0.5.0`, `zquic v0.3.0`, `ghost-wasm v0.1.0`
----
REPOS 
Ghostchain - https://github.com/ghostkellz/ghostchain  - Rust based 
Shroud - https://github.com/ghostkellz/shroud   
zvm  - https://github.com/ghostkellz/zvm  - zig based 
---

## 🎯 Objective

Build out `zvm` (Zig Virtual Machine) as the secure, QUIC-native smart contract execution environment for GhostChain. Integrate with the Zig crypto and networking stack, and embed `ghost-wasm` as a lightweight WASM interpreter optimized for on-chain execution.

---

## 📦 Module Overview

| Module         | Purpose                                | Depends On                     |
|----------------|----------------------------------------|--------------------------------|
| `runtime/`     | Core WASM VM logic (ghost-wasm)        | `zcrypto`, `ghost-wasm`        |
| `quic/`        | Load contracts over QUIC               | `zquic`                        |
| `exec/`        | Contract executor + gas meter          | `zcrypto`, `ghostd`            |
| `abi/`         | ABI compatibility and serialization    | `realid`, `zsig`, `zcrypto`    |
| `state/`       | Storage backend + consensus hooks      | `ghostd`, `walletd`            |
| `build.zig`    | Build system + `zquic`, `zcrypto` link | Zig                            |

---

## ✅ Immediate Tasks

### 🔧 1. **QUIC Module Loader**
- [ ] Accept incoming contract bytecode via `zquic`
- [ ] Stream WASM module chunks into `ghost-wasm`
- [ ] Validate with SHA-256 or Blake3 checksum (`zcrypto`)
- [ ] Handle QUIC disconnects and retries

### 🧠 2. **Ghost-WASM Integration**
- [ ] Fork or integrate `ghost-wasm` into `runtime/`
- [ ] Validate host functions (e.g., `env.log`, `env.hash`, etc.)
- [ ] Support opcode metering (gas)
- [ ] Isolate per-contract VM contexts

### 🔐 3. **Crypto & ABI Support**
- [ ] Link `zcrypto` v0.5.0 for:
  - [ ] SHA-256, Blake3
  - [ ] Ed25519 signature checks
- [ ] Use `realid` or `zsig` for auth in contract context
- [ ] Implement ABI compatibility layer (`abi/`)

### 📡 4. **Consensus & State Sync**
- [ ] Hook into `ghostd` for block height + timestamp
- [ ] Write pre/post-execution state to GhostChain DB
- [ ] Integrate gas usage and execution logs

---

## 🛠 Build System

Update `build.zig`:

```zig
exe.linkLibC();
exe.linkLibrary(zcrypto_dep.artifact("zcrypto"));
exe.linkLibrary(zquic_dep.artifact("zquic"));
exe.addIncludePath("src/ghost-wasm/include");

Add dependencies to build.zig.zon:

.{
  .name = "zvm",
  .version = "0.1.0",
  .dependencies = .{
    .zcrypto = .{ .url = "https://github.com/ghostkellz/zcrypto", .version = "0.5.0" },
    .zquic = .{ .url = "https://github.com/ghostkellz/zquic", .version = "0.3.0" },
  },
}

📜 API Design Goals

    Contracts are streamed over QUIC

    init() and invoke() are core contract functions

    Contracts can call cryptographic primitives directly

    Return values include:

        status_code

        gas_used

        return_data

📋 Validation Checklist

Load WASM via QUIC stream

Execute init + invoke cycles

Track gas usage accurately

Fail securely on invalid signatures

Serialize/deserialize return data

Integrate ghostd state engine

Benchmark against evm baseline

    Validate ghost-wasm op coverage (MVP set)

🚀 Next Steps

    Implement the QUIC loader and module cache

    Wire up ghost-wasm for opcode support and gas metering

    Add crypto-backed execution and host imports

    Validate state integration with ghostd


## ZNS needs: 
### **1. GhostBridge Enhancement** ⚡ CRITICAL
**Current Status:** ✅ FFI layer complete, needs production deployment  
**ZNS Dependency:** Real .ghost domain resolution via gRPC-over-QUIC

**Required Features for ZNS:**
```protobuf
// Enhanced ZNS service definitions needed
service ZNSService {
    rpc ResolveDomain(ZNSResolveRequest) returns (ZNSResolveResponse);
    rpc RegisterDomain(ZNSRegisterRequest) returns (ZNSRegisterResponse);
    rpc SubscribeDomainChanges(ZNSSubscribeRequest) returns (stream ZNSChangeEvent);
    rpc ValidateDomainOwnership(ZNSValidateRequest) returns (ZNSValidateResponse);
    rpc GetDomainMetadata(ZNSMetadataRequest) returns (ZNSMetadataResponse);
}

message ZNSResolveRequest {
    string domain = 1;
    repeated string record_types = 2; // A, AAAA, TXT, etc.
    bool include_metadata = 3;
}

message ZNSResolveResponse {
    string domain = 1;
    repeated DNSRecord records = 2;
    DomainMetadata metadata = 3;
    bytes signature = 4; // Ed25519 signature
    bytes public_key = 5; // Domain owner's public key
    int64 expires_at = 6;
}
```

**Tasks:**
- [ ] **Deploy production GhostBridge endpoint** with ZNS service
- [ ] **Implement complete ZNS service API** (not just stubs)
- [ ] **Add signature verification** for domain ownership
- [ ] **Create domain registration service** for .ghost/.zns domains
- [ ] **Add subscription support** for real-time domain updates
- [ ] **Performance optimization** for 10,000+ requests/second

**ZNS Impact:** Without this, ZNS cannot resolve real .ghost domains and remains limited to mock data.




