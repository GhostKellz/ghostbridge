  FFI Layer Issues to Fix

  1. zcrypto_ffi.zig - 4 errors:

  Error 1: ED25519 key size mismatch
  ghostwire/zquic/ffi/zcrypto_ffi.zig:48:5: error: non-matching copy lengths
  @memcpy(private_key[0..ED25519_PRIVATE_KEY_SIZE], &keypair.private_key);
  - Issue: ED25519_PRIVATE_KEY_SIZE is 32 but keypair.private_key is 64 bytes
  - Fix: Use correct size constant or adjust the copy length

  Error 2: ED25519 signature parameter mismatch
  ghostwire/zquic/ffi/zcrypto_ffi.zig:67:49: error: expected type '[64]u8', found '[32]u8'
  private_key[0..ED25519_PRIVATE_KEY_SIZE].*,
  - Issue: Function expects 64-byte array but getting 32-byte array
  - Fix: Use correct array size for the signature function

  Error 3: Blake3 finalResult method missing
  ghostwire/zquic/ffi/zcrypto_ffi.zig:169:26: error: no field or member function named 'finalResult'
  const digest = hasher.finalResult();
  - Issue: Blake3 hasher uses different method name
  - Fix: Use hasher.final() instead of hasher.finalResult()

  Error 4: zcrypto Sha256 finalResult method missing
  ghostwire/zquic/ffi/zcrypto_ffi.zig:187:26: error: no field or member function named 'finalResult'
  const digest = hasher.finalResult();
  - Issue: zcrypto Sha256 hasher uses different method name
  - Fix: Use hasher.final() instead of hasher.finalResult()

  2. ghostbridge_ffi.zig - 10 errors:

  Error 1: Format string without specifier
  error: cannot format slice without a specifier (i.e. {s} or {any})
  - Issue: Using {} instead of {s} or {any} for slice formatting
  - Fix: Add proper format specifiers to all log statements

  Error 2: Deprecated std.mem.split
  error: deprecated; use splitSequence, splitAny, or splitScalar
  - Issue: Using old std.mem.split API
  - Fix: Replace with std.mem.splitSequence, splitAny, or splitScalar

  Error 3: Invalid pointer cast
  ghostbridge_ffi.zig:100:21: error: expected pointer type, found 'grpc.server.GrpcServer'
  return @ptrCast(bridge);
  - Issue: Trying to cast non-pointer type
  - Fix: Use &bridge or proper pointer handling

  Error 4: Missing ServiceType member
  ghostbridge_ffi.zig:143:48: error: struct 'grpc.server.EchoService' has no member named 
  'ServiceType'
  - Issue: ServiceType doesn't exist in EchoService
  - Fix: Use correct member name or different approach

  Error 5: Missing connection_id field
  ghostbridge_ffi.zig:202:65: error: no field named 'connection_id' in struct
  'grpc.client.GrpcClient'
  - Issue: GrpcClient doesn't have connection_id field
  - Fix: Use correct field name or different identifier

  Error 6 & 7: GrpcMethod.init doesn't exist
  ghostbridge_ffi.zig:217:40: error: enum 'grpc.server.GrpcStatus' has no member named 'init'
  const grpc_method = GrpcMethod.init(allocator, service_str, method_str)
  - Issue: GrpcMethod doesn't have init method
  - Fix: Use correct constructor or initialization method

  Error 8: Missing sendMessage method
  ghostbridge_ffi.zig:292:20: error: no field or member function named 'sendMessage'
  real_stream.sendMessage(.stream_data, message_data)
  - Issue: GrpcMessage doesn't have sendMessage method
  - Fix: Use correct method name or dereference pointer with real_stream.*.sendMessage

  Error 9: Missing receiveMessage method
  ghostbridge_ffi.zig:308:24: error: no field or member function named 'receiveMessage'
  if (real_stream.receiveMessage()) |message| {
  - Issue: GrpcMessage doesn't have receiveMessage method
  - Fix: Use correct method name or dereference pointer

  Error 10: Missing close method
  ghostbridge_ffi.zig:333:20: error: no field or member function named 'close'
  real_stream.close()
  - Issue: GrpcMessage doesn't have close method
  - Fix: Use correct method name or dereference pointer

  Summary

  - 4 zcrypto FFI errors: Size mismatches and method name issues
  - 10 ghostbridge FFI errors: API mismatches, pointer issues, and missing methods
  - Total: 14 FFI-specific compilation errors to fix

  For Rust FFI Integration:
  - Should use gcrypt (Rust crypto library)
  - Not Blake3 from Zig std library

  The Blake3 Error Should Be:

  Instead of:
  var hasher = std.crypto.hash.Blake3.init(.{});  // ❌ Wrong - using Zig std

  It should probably be:
  // Call into gcrypt (Rust) for Blake3
  // OR use your zcrypto implementation
  var hasher = hash.blake3(input[0..input_len]);  // ✅ Use zcrypto

  Questions to Fix This:

  1. Is the FFI supposed to bridge to gcrypt (Rust) or use zcrypto (Zig)?
  2. Should zcrypto_ffi.zig call gcrypt functions or wrap zcrypto functions?
  3. Are you exposing zcrypto to Rust, or calling Rust gcrypt from Zig?
  4. https://github.com/ghostkellz/gcrypt
  The error suggests the FFI layer is confused about which crypto library to use. Can you clarify
  the intended crypto architecture for the FFI layer?

:
