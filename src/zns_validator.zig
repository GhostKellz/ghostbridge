const std = @import("std");
const zns_types = @import("zns_types.zig");

pub const ValidationResult = enum {
    valid,
    invalid_format,
    invalid_length,
    unsupported_type,
    signature_invalid,
};

pub const DomainValidator = struct {
    pub fn is_valid_domain(domain: []const u8) bool {
        // 1. Length: 1-253 characters total
        if (domain.len == 0 or domain.len > 253) return false;
        
        // 2. Must not start/end with dot or hyphen
        if (domain[0] == '.' or domain[0] == '-') return false;
        if (domain[domain.len - 1] == '.' or domain[domain.len - 1] == '-') return false;
        
        // 3. Valid TLDs: .ghost, .gcc, .sig, etc.
        return is_supported_tld(domain);
    }
    
    pub fn is_supported_tld(domain: []const u8) bool {
        const supported_tlds = [_][]const u8{
            // Core Identity Domains
            ".ghost", ".gcc", ".sig", ".gpk", ".key", ".pin",
            // Infrastructure Domains
            ".bc", ".zns", ".ops",
            // Reserved/Future Domains
            ".sid", ".dvm", ".tmp", ".dbg", ".lib", ".txo",
            // Bridge Support
            ".eth",                       // ENS bridge
            ".crypto", ".nft", ".x",      // Unstoppable Domains
            ".wallet", ".bitcoin",        // Additional Web3 TLDs
        };
        
        for (supported_tlds) |tld| {
            if (std.mem.endsWith(u8, domain, tld)) {
                return true;
            }
        }
        return false;
    }
    
    pub fn get_domain_category(domain: []const u8) ?DomainCategory {
        if (std.mem.endsWith(u8, domain, ".ghost") or
            std.mem.endsWith(u8, domain, ".gcc") or
            std.mem.endsWith(u8, domain, ".sig") or
            std.mem.endsWith(u8, domain, ".gpk") or
            std.mem.endsWith(u8, domain, ".key") or
            std.mem.endsWith(u8, domain, ".pin")) {
            return .identity;
        }
        
        if (std.mem.endsWith(u8, domain, ".bc") or
            std.mem.endsWith(u8, domain, ".zns") or
            std.mem.endsWith(u8, domain, ".ops")) {
            return .infrastructure;
        }
        
        if (std.mem.endsWith(u8, domain, ".eth")) {
            return .ens_bridge;
        }
        
        if (std.mem.endsWith(u8, domain, ".crypto") or
            std.mem.endsWith(u8, domain, ".nft") or
            std.mem.endsWith(u8, domain, ".x")) {
            return .unstoppable_bridge;
        }
        
        return .experimental;
    }
};

pub const DomainCategory = enum {
    identity,
    infrastructure,
    ens_bridge,
    unstoppable_bridge,
    experimental,
};

pub const RecordValidator = struct {
    pub fn validate_record(record: *const zns_types.DnsRecord) ValidationResult {
        switch (record.record_type) {
            .A => return validate_ipv4(record.value),
            .AAAA => return validate_ipv6(record.value),
            .CNAME => return validate_domain_name(record.value),
            .MX => return validate_mx_record(record),
            .TXT => return validate_txt_record(record.value),
            .SRV => return validate_srv_record(record),
            .GHOST => return validate_ghost_metadata(record.value),
            .CONTRACT => return validate_contract_address(record.value),
            .WALLET => return validate_wallet_address(record.value),
            else => return .valid,
        }
    }
    
    fn validate_ipv4(address: []const u8) ValidationResult {
        // IPv4 format: xxx.xxx.xxx.xxx
        var parts = std.mem.split(u8, address, ".");
        var count: u8 = 0;
        
        while (parts.next()) |part| {
            count += 1;
            if (count > 4) return .invalid_format;
            
            const num = std.fmt.parseInt(u8, part, 10) catch return .invalid_format;
            if (num > 255) return .invalid_format;
        }
        
        return if (count == 4) .valid else .invalid_format;
    }
    
    fn validate_ipv6(address: []const u8) ValidationResult {
        // Basic IPv6 validation (simplified)
        if (address.len < 2 or address.len > 39) return .invalid_format;
        
        // Must contain colons for IPv6
        if (std.mem.indexOf(u8, address, ":") == null) return .invalid_format;
        
        return .valid; // Full RFC 4291 validation would be more complex
    }
    
    fn validate_domain_name(domain: []const u8) ValidationResult {
        return if (DomainValidator.is_valid_domain(domain)) .valid else .invalid_format;
    }
    
    fn validate_mx_record(record: *const zns_types.DnsRecord) ValidationResult {
        // MX records must have priority and target
        if (record.priority == null) return .invalid_format;
        if (record.target == null) return .invalid_format;
        
        return validate_domain_name(record.target.?);
    }
    
    fn validate_txt_record(value: []const u8) ValidationResult {
        // TXT records can contain any text, but check length
        if (value.len > 255) return .invalid_length;
        return .valid;
    }
    
    fn validate_srv_record(record: *const zns_types.DnsRecord) ValidationResult {
        // SRV records must have priority, weight, port, and target
        if (record.priority == null or 
            record.weight == null or 
            record.port == null or 
            record.target == null) {
            return .invalid_format;
        }
        
        return validate_domain_name(record.target.?);
    }
    
    fn validate_ghost_metadata(metadata: []const u8) ValidationResult {
        // Ghost metadata should be valid JSON
        // For now, just check it's not empty and reasonable length
        if (metadata.len == 0 or metadata.len > 4096) return .invalid_length;
        
        // Could add JSON validation here
        return .valid;
    }
    
    fn validate_contract_address(address: []const u8) ValidationResult {
        // GhostChain contract address validation
        if (address.len != 42) return .invalid_format; // 0x + 40 hex chars
        if (!std.mem.startsWith(u8, address, "0x")) return .invalid_format;
        
        // Validate hex characters
        for (address[2..]) |c| {
            if (!std.ascii.isHex(c)) return .invalid_format;
        }
        
        return .valid;
    }
    
    fn validate_wallet_address(address: []const u8) ValidationResult {
        // Similar to contract address for now
        return validate_contract_address(address);
    }
};

pub const SignatureValidator = struct {
    pub fn verify_domain_signature(domain_data: *const zns_types.DomainData, public_key: [32]u8) !bool {
        if (domain_data.signature.len == 0) return false;
        
        const canonical_data = try create_canonical_representation(domain_data);
        defer std.heap.page_allocator.free(canonical_data);
        
        // TODO: Implement Ed25519 signature verification
        // For now, return true as placeholder
        _ = public_key;
        return true;
    }
    
    fn create_canonical_representation(domain_data: *const zns_types.DomainData) ![]u8 {
        // Create deterministic byte representation for signing
        // Format: domain|owner|records_hash|last_updated
        var buffer = std.ArrayList(u8).init(std.heap.page_allocator);
        defer buffer.deinit();
        
        try buffer.appendSlice(domain_data.domain);
        try buffer.append('|');
        try buffer.appendSlice(domain_data.owner);
        try buffer.append('|');
        
        // Hash all records for consistency
        const records_hash = try hash_records(domain_data.records);
        try buffer.appendSlice(&records_hash);
        try buffer.append('|');
        
        const timestamp_str = try std.fmt.allocPrint(std.heap.page_allocator, "{}", .{domain_data.last_updated});
        defer std.heap.page_allocator.free(timestamp_str);
        try buffer.appendSlice(timestamp_str);
        
        return buffer.toOwnedSlice();
    }
    
    fn hash_records(records: []zns_types.DnsRecord) ![32]u8 {
        // Simple hash of all record data
        var hasher = std.crypto.hash.sha2.Sha256.init(.{});
        
        for (records) |record| {
            hasher.update(record.name);
            hasher.update(record.value);
            hasher.update(std.mem.asBytes(&record.ttl));
        }
        
        var hash: [32]u8 = undefined;
        hasher.final(&hash);
        return hash;
    }
};

pub const RateLimiter = struct {
    requests_per_minute: std.HashMap([]const u8, u32, std.hash_map.StringContext, std.hash_map.default_max_load_percentage),
    allocator: std.mem.Allocator,
    max_requests_per_minute: u32,
    
    pub fn init(allocator: std.mem.Allocator, max_requests: u32) @This() {
        return @This(){
            .requests_per_minute = std.HashMap([]const u8, u32, std.hash_map.StringContext, std.hash_map.default_max_load_percentage).init(allocator),
            .allocator = allocator,
            .max_requests_per_minute = max_requests,
        };
    }
    
    pub fn deinit(self: *@This()) void {
        self.requests_per_minute.deinit();
    }
    
    pub fn is_allowed(self: *@This(), client_id: []const u8) bool {
        const current_count = self.requests_per_minute.get(client_id) orelse 0;
        
        if (current_count >= self.max_requests_per_minute) {
            return false;
        }
        
        self.requests_per_minute.put(client_id, current_count + 1) catch return false;
        return true;
    }
    
    pub fn reset_counters(self: *@This()) void {
        self.requests_per_minute.clearAndFree();
    }
};