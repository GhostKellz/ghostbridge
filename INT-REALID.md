# 🔌 RealID Integration Guide

This guide covers integrating RealID into Zig and Rust projects - the primary supported use cases.

## 📋 Table of Contents

- [Zig Integration](#zig-integration) - **Primary Use Case**
- [Rust Integration](#rust-integration) - **Primary Use Case**  
- [C/C++ FFI](#cc-ffi) - **For Advanced Interop**
- [Advanced Examples](#advanced-examples)
- [Production Deployment](#production-deployment)

---

## 🦎 Zig Integration (Primary Use Case)

RealID is built in Zig and provides the most natural integration for Zig projects.

### Adding RealID as a Dependency

Add RealID to your `build.zig.zon`:

```zig
.dependencies = .{
    .realid = .{
        .path = "../realid", // Local path
        // OR for remote:
        // .url = "https://github.com/your-org/realid/archive/main.tar.gz",
        // .hash = "...",
    },
},
```

Update your `build.zig`:

```zig
const realid_dep = b.dependency("realid", .{
    .target = target,
    .optimize = optimize,
});

exe.root_module.addImport("realid", realid_dep.module("realid"));
```

### Basic Usage

```zig
const std = @import("std");
const realid = @import("realid");

pub fn main() !void {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    defer _ = gpa.deinit();
    const allocator = gpa.allocator();

    // Generate identity from passphrase
    const passphrase = "user_passphrase_123";
    const keypair = try realid.realid_generate_from_passphrase(passphrase);
    
    // Sign data
    const data = "Hello, GhostNet Web5!";
    const signature = try realid.realid_sign(data, keypair.private_key);
    
    // Verify signature
    const is_valid = realid.realid_verify(signature, data, keypair.public_key);
    std.debug.print("Signature valid: {}\n", .{is_valid});
    
    // Generate QID (IPv6 identity)
    const qid = realid.realid_qid_from_pubkey(keypair.public_key);
    std.debug.print("QID: ", .{});
    for (qid.bytes) |byte| {
        std.debug.print("{x:0>2}", .{byte});
    }
    std.debug.print("\n", .{});
    
    // Device fingerprint for additional security
    const device_fp = try realid.generate_device_fingerprint(allocator);
    const device_keypair = try realid.realid_generate_from_passphrase_with_device(passphrase, device_fp);
}
```

### Advanced Usage for walletd/ghostd

```zig
// Integration example for walletd
const std = @import("std");
const realid = @import("realid");

const WalletIdentity = struct {
    keypair: realid.RealIDKeyPair,
    qid: realid.QID,
    device_bound: bool,
    
    const Self = @This();
    
    pub fn fromPassphrase(allocator: std.mem.Allocator, passphrase: []const u8, device_binding: bool) !Self {
        const keypair = if (device_binding) blk: {
            const device_fp = try realid.generate_device_fingerprint(allocator);
            break :blk try realid.realid_generate_from_passphrase_with_device(passphrase, device_fp);
        } else try realid.realid_generate_from_passphrase(passphrase);
        
        const qid = realid.realid_qid_from_pubkey(keypair.public_key);
        
        return Self{
            .keypair = keypair,
            .qid = qid,
            .device_bound = device_binding,
        };
    }
    
    pub fn signTransaction(self: Self, transaction_data: []const u8) !realid.RealIDSignature {
        return realid.realid_sign(transaction_data, self.keypair.private_key);
    }
    
    pub fn verifyTransaction(self: Self, transaction_data: []const u8, signature: realid.RealIDSignature) bool {
        return realid.realid_verify(signature, transaction_data, self.keypair.public_key);
    }
    
    pub fn getQIDString(self: Self, buffer: []u8) ![]u8 {
        return realid.qid.qid_to_string(self.qid, buffer);
    }
};

// Example usage in walletd
pub fn main() !void {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    defer _ = gpa.deinit();
    const allocator = gpa.allocator();
    
    const wallet = try WalletIdentity.fromPassphrase(allocator, "secure_wallet_passphrase", true);
    
    const transaction = "transfer:100:ghostnet:0x123...";
    const signature = try wallet.signTransaction(transaction);
    
    std.debug.print("Transaction signed with RealID\n");
    std.debug.print("QID: {}\n", .{std.fmt.fmtSliceHexLower(&wallet.qid.bytes)});
}
```

---

## 🔧 C/C++ FFI

For advanced interoperability, RealID provides a C FFI interface.

### Static Library Linking

```bash
# Build the static library
zig build -Doptimize=ReleaseFast

# Link in your C/C++ project
gcc -o myapp myapp.c -L./zig-out/lib -lrealid
```

### C Usage Example

```c
#include "realid.h"
#include <stdio.h>
#include <string.h>

int main() {
    uint8_t public_key[32];
    uint8_t private_key[64];
    uint8_t device_fingerprint[32];
    
    // Generate identity
    int result = realid_generate("password123", "my_device", 
                                public_key, private_key, device_fingerprint);
    if (result != 0) {
        printf("Failed to generate identity\n");
        return 1;
    }
    
    // Sign a message
    const char* message = "Hello from C!";
    uint8_t signature[64];
    
    result = realid_sign(private_key, (const uint8_t*)message, 
                        strlen(message), signature);
    if (result != 0) {
        printf("Failed to sign message\n");
        return 1;
    }
    
    // Verify signature
    int is_valid = realid_verify(public_key, (const uint8_t*)message, 
                                strlen(message), signature);
    
    printf("Message: %s\n", message);
    printf("Signature valid: %s\n", is_valid ? "true" : "false");
    
    return 0;
}
```

---

## 🚀 Advanced Examples

### Device-Bound Identity Management

```zig
const DeviceManager = struct {
    const Self = @This();
    
    pub fn createSecureIdentity(allocator: std.mem.Allocator, user_password: []const u8) !realid.RealIDIdentity {
        // Gather comprehensive device fingerprint
        var device_info = std.ArrayList(u8).init(allocator);
        defer device_info.deinit();
        
        try device_info.appendSlice("CPU:");
        try device_info.appendSlice(getCPUId());
        try device_info.appendSlice("|GPU:");
        try device_info.appendSlice(getGPUId());
        try device_info.appendSlice("|MAC:");
        try device_info.appendSlice(getMACAddress());
        try device_info.appendSlice("|BIOS:");
        try device_info.appendSlice(getBIOSInfo());
        
        return try realid.RealIDCore.generate(allocator, user_password, device_info.items);
    }
    
    fn getCPUId() []const u8 { return "intel_i7_12700k"; }
    fn getGPUId() []const u8 { return "nvidia_rtx_4080"; }
    fn getMACAddress() []const u8 { return "00:1B:44:11:3A:B7"; }
    fn getBIOSInfo() []const u8 { return "american_megatrends_v2.1"; }
};
```

### Multi-Signature Validation (Rust)

```rust
pub struct MultiSigValidator {
    required_signatures: usize,
    validator_keys: Vec<[u8; 32]>,
}

impl MultiSigValidator {
    pub fn verify_multisig(&self, message: &[u8], signatures: &[[u8; 64]]) -> bool {
        if signatures.len() < self.required_signatures {
            return false;
        }
        
        let mut valid_signatures = 0;
        for (i, signature) in signatures.iter().enumerate() {
            if i < self.validator_keys.len() {
                if verify(&self.validator_keys[i], message, signature) {
                    valid_signatures += 1;
                }
            }
        }
        
        valid_signatures >= self.required_signatures
    }
}
```

---

## 🏭 Production Deployment

### Security Considerations

1. **Key Storage**: Never store private keys in plain text
2. **Device Binding**: Implement comprehensive device fingerprinting
3. **Password Policy**: Enforce strong password requirements
4. **Audit Logging**: Log all identity operations
5. **Regular Rotation**: Implement key rotation strategies

### Performance Optimization

```zig
// Pre-compute common operations
const IdentityCache = struct {
    const Self = @This();
    
    cached_fingerprints: std.HashMap([32]u8, realid.RealIDIdentity, std.hash_map.HashMap([32]u8, realid.RealIDIdentity, std.hash_map.StringContext, std.hash_map.default_max_load_percentage)),
    
    pub fn getOrCreateIdentity(self: *Self, allocator: std.mem.Allocator, password: []const u8, device_info: []const u8) !*realid.RealIDIdentity {
        const device_hash = realid.Fingerprint.generate(device_info);
        
        if (self.cached_fingerprints.get(device_hash)) |cached| {
            return &cached;
        }
        
        const identity = try realid.RealIDCore.generate(allocator, password, device_info);
        try self.cached_fingerprints.put(device_hash, identity);
        
        return self.cached_fingerprints.getPtr(device_hash).?;
    }
};
```

### Monitoring & Metrics (Rust)

```rust
use prometheus::{Counter, Histogram, register_counter, register_histogram};

lazy_static! {
    static ref IDENTITY_GENERATIONS: Counter = register_counter!(
        "realid_identity_generations_total",
        "Total number of identity generations"
    ).unwrap();
    
    static ref SIGNATURE_VERIFICATIONS: Counter = register_counter!(
        "realid_signature_verifications_total", 
        "Total number of signature verifications"
    ).unwrap();
    
    static ref OPERATION_DURATION: Histogram = register_histogram!(
        "realid_operation_duration_seconds",
        "Duration of RealID operations"
    ).unwrap();
}

pub fn monitored_generate(password: &str, device_info: &str) -> Result<RealIDIdentity, &'static str> {
    let timer = OPERATION_DURATION.start_timer();
    let result = RealIDIdentity::generate(password, device_info);
    timer.observe_duration();
    
    if result.is_ok() {
        IDENTITY_GENERATIONS.inc();
    }
    
    result
}
```

---

## 📖 Additional Resources

- **Zig Documentation**: [ziglang.org/documentation](https://ziglang.org/documentation/)
- **Rust FFI Guide**: [doc.rust-lang.org/nomicon/ffi.html](https://doc.rust-lang.org/nomicon/ffi.html)
- **zcrypto Library**: Used for all cryptographic operations
- **Ed25519 Specification**: [RFC 8032](https://tools.ietf.org/html/rfc8032)

For questions or contributions, please refer to the main [README.md](./README    .name = .realid,
    .version = "0.2.1",
    .dependencies = .{.md) and [DOCS.md](./DOCS.md).
