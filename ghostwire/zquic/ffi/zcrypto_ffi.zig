const std = @import("std");
const zcrypto = @import("zcrypto");

// Constants for key and signature sizes
const ED25519_PUBLIC_KEY_SIZE = 32;
const ED25519_PRIVATE_KEY_SIZE = 64; // Ed25519 expanded secret key
const ED25519_SIGNATURE_SIZE = 64;
const SECP256K1_PUBLIC_KEY_SIZE = 33; // Compressed
const SECP256K1_PRIVATE_KEY_SIZE = 32;
const SECP256K1_SIGNATURE_SIZE = 64;
const BLAKE3_HASH_SIZE = 32;
const SHA256_HASH_SIZE = 32;

// Ed25519 operations - C ABI for Rust clients
pub export fn zcrypto_ed25519_keypair(public_key: [*]u8, private_key: [*]u8) callconv(.C) c_int {
    // Generate Ed25519 keypair using Shroud's crypto
    const keypair = zcrypto.sign.ed25519.KeyPair.create(null) catch return -1;
    
    // Copy public key (32 bytes)
    @memcpy(public_key[0..ED25519_PUBLIC_KEY_SIZE], &keypair.public_key);
    
    // Copy expanded secret key (64 bytes - seed + public key)
    @memcpy(private_key[0..ED25519_PRIVATE_KEY_SIZE], &keypair.secret_key);
    
    return 0;
}

pub export fn zcrypto_ed25519_sign(
    private_key: [*]const u8, 
    message: [*]const u8, 
    message_len: usize, 
    signature: [*]u8
) callconv(.C) c_int {
    // Create keypair from expanded secret key
    const secret_key = private_key[0..ED25519_PRIVATE_KEY_SIZE].*;
    const keypair = zcrypto.sign.ed25519.KeyPair{
        .public_key = secret_key[32..64].*,
        .secret_key = secret_key,
    };
    
    // Sign the message
    const sig = keypair.sign(message[0..message_len], null) catch return -1;
    
    // Copy signature (64 bytes)
    @memcpy(signature[0..ED25519_SIGNATURE_SIZE], &sig);
    
    return 0;
}

pub export fn zcrypto_ed25519_verify(
    public_key: [*]const u8, 
    message: [*]const u8, 
    message_len: usize, 
    signature: [*]const u8
) callconv(.C) c_int {
    // Convert arrays to proper types
    const pub_key = public_key[0..ED25519_PUBLIC_KEY_SIZE].*;
    const sig = signature[0..ED25519_SIGNATURE_SIZE].*;
    
    // Verify signature using Shroud
    zcrypto.sign.ed25519.verify(sig, message[0..message_len], pub_key) catch return -1;
    
    return 0;
}

// Secp256k1 operations - C ABI for Rust clients
pub export fn zcrypto_secp256k1_keypair(public_key: [*]u8, private_key: [*]u8) callconv(.C) c_int {
    // Generate Secp256k1 keypair using Shroud
    const secret_key = zcrypto.sign.secp256k1.SecretKey.random();
    const pub_key = zcrypto.sign.secp256k1.PublicKey.fromSecretKey(secret_key);
    
    // Copy private key (32 bytes)
    @memcpy(private_key[0..SECP256K1_PRIVATE_KEY_SIZE], &secret_key.data);
    
    // Copy compressed public key (33 bytes)
    const compressed = pub_key.toCompressedSec1();
    @memcpy(public_key[0..SECP256K1_PUBLIC_KEY_SIZE], &compressed);
    
    return 0;
}

pub export fn zcrypto_secp256k1_sign(
    private_key: [*]const u8, 
    message_hash: [*]const u8, 
    signature: [*]u8
) callconv(.C) c_int {
    // Create secret key from bytes
    const secret_key = zcrypto.sign.secp256k1.SecretKey{
        .data = private_key[0..SECP256K1_PRIVATE_KEY_SIZE].*,
    };
    
    // Sign the message hash (should be 32 bytes)
    const sig = zcrypto.sign.secp256k1.sign(message_hash[0..32].*, secret_key) catch return -1;
    
    // Copy signature (64 bytes - r and s values)
    @memcpy(signature[0..32], &sig.r);
    @memcpy(signature[32..64], &sig.s);
    
    return 0;
}

// Hashing functions - C ABI for Rust clients
pub export fn zcrypto_blake3_hash(input: [*]const u8, input_len: usize, output: [*]u8) callconv(.C) c_int {
    // Create Blake3 hasher using Shroud
    var hasher = zcrypto.hash.blake3.init(.{});
    
    // Update with input data
    hasher.update(input[0..input_len]);
    
    // Finalize and get hash
    const digest = hasher.final();
    
    // Copy hash output (32 bytes)
    @memcpy(output[0..BLAKE3_HASH_SIZE], &digest);
    
    return 0;
}

pub export fn zcrypto_sha256_hash(input: [*]const u8, input_len: usize, output: [*]u8) callconv(.C) c_int {
    // Create SHA256 hasher using Shroud
    var hasher = zcrypto.hash.sha256.init(.{});
    
    // Update with input data
    hasher.update(input[0..input_len]);
    
    // Finalize and get hash
    const digest = hasher.final();
    
    // Copy hash output (32 bytes)
    @memcpy(output[0..SHA256_HASH_SIZE], &digest);
    
    return 0;
}