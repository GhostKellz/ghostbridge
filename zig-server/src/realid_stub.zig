/// RealID stub for compilation
/// Will be replaced with actual realID import when dependency is fixed

const std = @import("std");

pub const RealIDKeyPair = struct {
    public_key: [32]u8,
    private_key: [64]u8,
};

pub const RealIDSignature = struct {
    bytes: [64]u8,
};

pub const QID = struct {
    bytes: [16]u8,
};

pub fn realid_generate_from_passphrase(passphrase: []const u8) !RealIDKeyPair {
    _ = passphrase;
    // Stub implementation
    return RealIDKeyPair{
        .public_key = [_]u8{0} ** 32,
        .private_key = [_]u8{0} ** 64,
    };
}

pub fn realid_generate_from_passphrase_with_device(passphrase: []const u8, device_fp: []const u8) !RealIDKeyPair {
    _ = passphrase;
    _ = device_fp;
    return RealIDKeyPair{
        .public_key = [_]u8{0} ** 32,
        .private_key = [_]u8{0} ** 64,
    };
}

pub fn generate_device_fingerprint(allocator: std.mem.Allocator) ![]const u8 {
    const fp = try allocator.alloc(u8, 32);
    @memset(fp, 0);
    return fp;
}

pub fn realid_sign(data: []const u8, private_key: [64]u8) !RealIDSignature {
    _ = data;
    _ = private_key;
    return RealIDSignature{ .bytes = [_]u8{0} ** 64 };
}

pub fn realid_verify(signature: RealIDSignature, data: []const u8, public_key: [32]u8) bool {
    _ = signature;
    _ = data;
    _ = public_key;
    return true;
}

pub fn realid_qid_from_pubkey(public_key: [32]u8) QID {
    _ = public_key;
    return QID{ .bytes = [_]u8{0} ** 16 };
}

pub const qid = struct {
    pub fn qid_to_string(q: QID, buffer: []u8) ![]const u8 {
        _ = q;
        const str = "qid:stub:0000";
        @memcpy(buffer[0..str.len], str);
        return buffer[0..str.len];
    }
};