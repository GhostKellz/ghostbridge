const std = @import("std");
const realid = @import("realid_stub.zig");
const grpc = @import("grpc.zig");
const protobuf = @import("protobuf.zig");
const tokioz = @import("tokioz_stub.zig");

/// Identity Service using realID for Web5 identity management
pub const IdentityService = struct {
    allocator: std.mem.Allocator,
    identity_cache: std.hash_map.HashMap([]const u8, CachedIdentity, std.hash_map.StringContext, 80),
    
    const Self = @This();
    
    const CachedIdentity = struct {
        keypair: realid.RealIDKeyPair,
        qid: realid.QID,
        device_bound: bool,
        created_at: i64,
        
        pub fn isExpired(self: CachedIdentity) bool {
            const now = std.time.milliTimestamp();
            return (now - self.created_at) > (60 * 60 * 1000); // 1 hour expiry
        }
    };
    
    pub fn init(allocator: std.mem.Allocator) !Self {
        return Self{
            .allocator = allocator,
            .identity_cache = std.hash_map.HashMap([]const u8, CachedIdentity, std.hash_map.StringContext, 80).init(allocator),
        };
    }
    
    pub fn deinit(self: *Self) void {
        self.identity_cache.deinit();
    }
    
    /// Generate a new identity from passphrase
    pub fn generateIdentity(self: *Self, request: GenerateIdentityRequest) !GenerateIdentityResponse {
        // Check cache first
        if (self.identity_cache.get(request.passphrase)) |cached| {
            if (!cached.isExpired()) {
                return GenerateIdentityResponse{
                    .public_key = cached.keypair.public_key,
                    .qid = cached.qid.bytes,
                    .device_bound = cached.device_bound,
                };
            }
        }
        
        // Generate new identity
        const keypair = if (request.device_binding) blk: {
            const device_fp = try realid.generate_device_fingerprint(self.allocator);
            break :blk try realid.realid_generate_from_passphrase_with_device(
                request.passphrase,
                device_fp,
            );
        } else try realid.realid_generate_from_passphrase(request.passphrase);
        
        const qid = realid.realid_qid_from_pubkey(keypair.public_key);
        
        // Cache the identity
        try self.identity_cache.put(request.passphrase, CachedIdentity{
            .keypair = keypair,
            .qid = qid,
            .device_bound = request.device_binding,
            .created_at = std.time.milliTimestamp(),
        });
        
        return GenerateIdentityResponse{
            .public_key = keypair.public_key,
            .qid = qid.bytes,
            .device_bound = request.device_binding,
        };
    }
    
    /// Sign data with identity
    pub fn signData(self: *Self, request: SignDataRequest) !SignDataResponse {
        const cached = self.identity_cache.get(request.passphrase) orelse
            return error.IdentityNotFound;
            
        if (cached.isExpired()) {
            return error.IdentityExpired;
        }
        
        const signature = try realid.realid_sign(
            request.data,
            cached.keypair.private_key,
        );
        
        return SignDataResponse{
            .signature = signature.bytes,
            .public_key = cached.keypair.public_key,
            .qid = cached.qid.bytes,
        };
    }
    
    /// Verify signature
    pub fn verifySignature(self: *Self, request: VerifySignatureRequest) !VerifySignatureResponse {
        _ = self;
        const is_valid = realid.realid_verify(
            realid.RealIDSignature{ .bytes = request.signature },
            request.data,
            request.public_key,
        );
        
        return VerifySignatureResponse{
            .is_valid = is_valid,
            .qid = if (is_valid) blk: {
                const qid = realid.realid_qid_from_pubkey(request.public_key);
                break :blk qid.bytes;
            } else null,
        };
    }
    
    /// Get QID from public key
    pub fn getQID(self: *Self, request: GetQIDRequest) !GetQIDResponse {
        _ = self;
        const qid = realid.realid_qid_from_pubkey(request.public_key);
        
        var qid_string_buf: [64]u8 = undefined;
        const qid_string = try realid.qid.qid_to_string(qid, &qid_string_buf);
        
        return GetQIDResponse{
            .qid = qid.bytes,
            .qid_string = qid_string,
        };
    }
    
    /// Handle async requests using TokioZ
    pub fn handleAsync(self: *Self) !void {
        const AsyncTask = struct {
            service: *IdentityService,
            
            pub fn run(task: @This()) !void {
                while (true) {
                    // Process identity requests asynchronously
                    const sleep_duration = tokioz.time.Duration.fromMillis(100);
                    try tokioz.time.sleep(sleep_duration);
                    
                    // Clean expired cache entries
                    var iter = task.service.identity_cache.iterator();
                    while (iter.next()) |entry| {
                        if (entry.value_ptr.*.isExpired()) {
                            _ = task.service.identity_cache.remove(entry.key_ptr.*);
                        }
                    }
                }
            }
        };
        
        const task = AsyncTask{ .service = self };
        try tokioz.runtime.run(task.run);
    }
};

// Request/Response types for gRPC
pub const GenerateIdentityRequest = struct {
    passphrase: []const u8,
    device_binding: bool = false,
};

pub const GenerateIdentityResponse = struct {
    public_key: [32]u8,
    qid: [16]u8,
    device_bound: bool,
};

pub const SignDataRequest = struct {
    passphrase: []const u8,
    data: []const u8,
};

pub const SignDataResponse = struct {
    signature: [64]u8,
    public_key: [32]u8,
    qid: [16]u8,
};

pub const VerifySignatureRequest = struct {
    signature: [64]u8,
    data: []const u8,
    public_key: [32]u8,
};

pub const VerifySignatureResponse = struct {
    is_valid: bool,
    qid: ?[16]u8,
};

pub const GetQIDRequest = struct {
    public_key: [32]u8,
};

pub const GetQIDResponse = struct {
    qid: [16]u8,
    qid_string: []const u8,
};

/// gRPC service handlers
pub fn registerIdentityService(grpc_handler: *grpc.Handler, identity_service: *IdentityService) !void {
    try grpc_handler.registerMethod("identity.IdentityService/GenerateIdentity", 
        struct {
            fn handle(service: *IdentityService, req: GenerateIdentityRequest) !GenerateIdentityResponse {
                return service.generateIdentity(req);
            }
        }.handle, identity_service);
        
    try grpc_handler.registerMethod("identity.IdentityService/SignData",
        struct {
            fn handle(service: *IdentityService, req: SignDataRequest) !SignDataResponse {
                return service.signData(req);
            }
        }.handle, identity_service);
        
    try grpc_handler.registerMethod("identity.IdentityService/VerifySignature",
        struct {
            fn handle(service: *IdentityService, req: VerifySignatureRequest) !VerifySignatureResponse {
                return service.verifySignature(req);
            }
        }.handle, identity_service);
        
    try grpc_handler.registerMethod("identity.IdentityService/GetQID",
        struct {
            fn handle(service: *IdentityService, req: GetQIDRequest) !GetQIDResponse {
                return service.getQID(req);
            }
        }.handle, identity_service);
}