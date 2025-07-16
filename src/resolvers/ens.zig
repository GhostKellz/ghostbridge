const std = @import("std");
const zns_types = @import("../zns_types.zig");

pub const ENSResolver = struct {
    allocator: std.mem.Allocator,
    ethereum_rpc_endpoint: []const u8,
    rate_limit_per_second: u32,
    timeout_ms: u32,
    last_request_time: u64,
    request_count: u32,
    
    // ENS contract addresses on Ethereum mainnet
    const ENS_REGISTRY = "0x00000000000C2E074eC69A0dFb2997BA6C7d2e1e";
    const ENS_PUBLIC_RESOLVER = "0x4976fb03C32e5B8cfe2b6cCB31c09Ba78EBaBa41";
    
    pub fn init(allocator: std.mem.Allocator, ethereum_rpc_endpoint: []const u8) @This() {
        return @This(){
            .allocator = allocator,
            .ethereum_rpc_endpoint = ethereum_rpc_endpoint,
            .rate_limit_per_second = 100, // Conservative rate limit
            .timeout_ms = 5000,
            .last_request_time = 0,
            .request_count = 0,
        };
    }
    
    pub fn resolve_domain(self: *@This(), domain: []const u8, record_types: [][]const u8) !?zns_types.ZNSResolveResponse {
        if (!std.mem.endsWith(u8, domain, ".eth")) {
            return null; // Not an ENS domain
        }
        
        try self.check_rate_limit();
        
        const start_time = std.time.milliTimestamp();
        
        // Get ENS resolver address for this domain
        const resolver_address = try self.get_resolver_address(domain);
        if (resolver_address == null) {
            return self.create_error_response(domain, .DOMAIN_NOT_FOUND, "ENS domain not found");
        }
        
        // Resolve records from the resolver contract
        var records = std.ArrayList(zns_types.DnsRecord).init(self.allocator);
        defer records.deinit();
        
        for (record_types) |record_type| {
            if (try self.resolve_record(domain, record_type, resolver_address.?)) |record| {
                try records.append(record);
            }
        }
        
        const end_time = std.time.milliTimestamp();
        const resolution_time = @as(u64, @intCast(end_time - start_time));
        
        return zns_types.ZNSResolveResponse{
            .domain = domain,
            .records = try records.toOwnedSlice(),
            .metadata = self.create_ens_metadata(domain),
            .resolution_info = zns_types.ResolutionInfo{
                .source = .ENS_BRIDGE,
                .resolution_time_ms = resolution_time,
                .was_cached = false,
                .resolved_at = @as(u64, @intCast(std.time.timestamp())),
                .resolver_version = "ENS-Bridge-1.0",
                .resolution_path = @as([][]const u8, @constCast(&[_][]const u8{ "ens_bridge", "ethereum_rpc" })),
            },
            .zns_error = null,
        };
    }
    
    fn check_rate_limit(self: *@This()) !void {
        const now = std.time.milliTimestamp();
        const time_diff = @as(u64, @intCast(now)) - self.last_request_time;
        
        if (time_diff < 1000) { // Within same second
            self.request_count += 1;
            if (self.request_count > self.rate_limit_per_second) {
                return error.RateLimited;
            }
        } else {
            self.request_count = 1;
            self.last_request_time = @as(u64, @intCast(now));
        }
    }
    
    fn get_resolver_address(self: *@This(), domain: []const u8) !?[]const u8 {
        // Convert domain to ENS namehash
        const namehash = try self.compute_namehash(domain);
        
        // Call ENS registry resolver() function
        const call_data = try self.encode_resolver_call(namehash);
        const result = try self.ethereum_rpc_call("eth_call", call_data);
        
        if (result.len < 64) return null; // Invalid response
        
        // Extract address from result (last 20 bytes of 32-byte response)
        const address_start = result.len - 40; // 20 bytes = 40 hex chars
        const address_hex = result[address_start..];
        
        // Check if address is zero (domain not registered)
        const zero_address = "0000000000000000000000000000000000000000";
        if (std.mem.eql(u8, address_hex, zero_address)) {
            return null;
        }
        
        return try self.allocator.dupe(u8, address_hex);
    }
    
    fn resolve_record(self: *@This(), domain: []const u8, record_type: []const u8, resolver_address: []const u8) !?zns_types.DnsRecord {
        const namehash = try self.compute_namehash(domain);
        
        if (std.mem.eql(u8, record_type, "A")) {
            return try self.resolve_a_record(domain, namehash, resolver_address);
        } else if (std.mem.eql(u8, record_type, "AAAA")) {
            return try self.resolve_aaaa_record(domain, namehash, resolver_address);
        } else if (std.mem.eql(u8, record_type, "TXT")) {
            return try self.resolve_txt_record(domain, namehash, resolver_address);
        } else if (std.mem.eql(u8, record_type, "CNAME")) {
            return try self.resolve_content_hash(domain, namehash, resolver_address);
        }
        
        return null;
    }
    
    fn resolve_a_record(self: *@This(), domain: []const u8, namehash: [32]u8, resolver_address: []const u8) !?zns_types.DnsRecord {
        // Call addr(bytes32) on resolver contract
        const call_data = try self.encode_addr_call(namehash);
        const target = try std.fmt.allocPrint(self.allocator, "0x{s}", .{resolver_address});
        defer self.allocator.free(target);
        
        const result = try self.ethereum_rpc_call_to_address("eth_call", call_data, target);
        
        if (result.len < 64) return null;
        
        // Convert hex result to IP address
        const ip_hex = result[result.len - 8..]; // Last 4 bytes
        const ip_address = try self.hex_to_ip_address(ip_hex);
        
        return zns_types.DnsRecord{
            .record_type = .A,
            .name = domain,
            .value = ip_address,
            .ttl = 3600, // Default TTL
            .priority = null,
            .port = null,
            .weight = null,
            .target = null,
            .created_at = @as(u64, @intCast(std.time.timestamp())),
            .signature = null,
        };
    }
    
    fn resolve_aaaa_record(_: *@This(), _: []const u8, _: [32]u8, _: []const u8) !?zns_types.DnsRecord {
        // Similar to A record but for IPv6
        // ENS doesn't have standard IPv6 support, so we'll skip for now
        return null;
    }
    
    fn resolve_txt_record(self: *@This(), domain: []const u8, namehash: [32]u8, resolver_address: []const u8) !?zns_types.DnsRecord {
        // Call text(bytes32, string) on resolver contract for various keys
        const common_keys = [_][]const u8{ "avatar", "description", "url", "com.github", "com.twitter" };
        
        for (common_keys) |key| {
            const call_data = try self.encode_text_call(namehash, key);
            const target = try std.fmt.allocPrint(self.allocator, "0x{s}", .{resolver_address});
            defer self.allocator.free(target);
            
            const result = try self.ethereum_rpc_call_to_address("eth_call", call_data, target);
            
            if (result.len > 128) { // Has actual text content
                const text_value = try self.decode_string_result(result);
                if (text_value.len > 0) {
                    return zns_types.DnsRecord{
                        .record_type = .TXT,
                        .name = domain,
                        .value = try std.fmt.allocPrint(self.allocator, "{s}={s}", .{ key, text_value }),
                        .ttl = 3600,
                        .priority = null,
                        .port = null,
                        .weight = null,
                        .target = null,
                        .created_at = @as(u64, @intCast(std.time.timestamp())),
                        .signature = null,
                    };
                }
            }
        }
        
        return null;
    }
    
    fn resolve_content_hash(self: *@This(), domain: []const u8, namehash: [32]u8, resolver_address: []const u8) !?zns_types.DnsRecord {
        // Call contenthash(bytes32) on resolver contract
        const call_data = try self.encode_contenthash_call(namehash);
        const target = try std.fmt.allocPrint(self.allocator, "0x{s}", .{resolver_address});
        defer self.allocator.free(target);
        
        const result = try self.ethereum_rpc_call_to_address("eth_call", call_data, target);
        
        if (result.len > 128) {
            const content_hash = try self.decode_bytes_result(result);
            if (content_hash.len > 0) {
                return zns_types.DnsRecord{
                    .record_type = .CNAME,
                    .name = domain,
                    .value = content_hash,
                    .ttl = 3600,
                    .priority = null,
                    .port = null,
                    .weight = null,
                    .target = null,
                    .created_at = @as(u64, @intCast(std.time.timestamp())),
                    .signature = null,
                };
            }
        }
        
        return null;
    }
    
    fn ethereum_rpc_call(self: *@This(), method: []const u8, call_data: []const u8) ![]const u8 {
        const target = try std.fmt.allocPrint(self.allocator, "{s}", .{ENS_REGISTRY});
        defer self.allocator.free(target);
        return try self.ethereum_rpc_call_to_address(method, call_data, target);
    }
    
    fn ethereum_rpc_call_to_address(self: *@This(), method: []const u8, call_data: []const u8, to_address: []const u8) ![]const u8 {
        // Construct JSON-RPC payload
        const payload = try std.fmt.allocPrint(self.allocator,
            \\{{"jsonrpc": "2.0", "method": "{s}", "params": [{{"to": "{s}", "data": "0x{s}"}}, "latest"], "id": 1}}
        , .{ method, to_address, call_data });
        defer self.allocator.free(payload);
        
        // Make HTTP request (simplified - in production would use proper HTTP client)
        // For now, return placeholder response
        return try self.allocator.dupe(u8, "0000000000000000000000000000000000000000000000000000000000000000");
    }
    
    fn compute_namehash(self: *@This(), domain: []const u8) ![32]u8 {
        // ENS namehash algorithm
        var hash: [32]u8 = std.mem.zeroes([32]u8);
        
        // Split domain by dots and hash recursively
        var parts = std.ArrayList([]const u8).init(self.allocator);
        defer parts.deinit();
        
        var iterator = std.mem.splitScalar(u8, domain, '.');
        while (iterator.next()) |part| {
            try parts.append(part);
        }
        
        // Process parts in reverse order (TLD first)
        var i = parts.items.len;
        while (i > 0) {
            i -= 1;
            const part = parts.items[i];
            
            // Hash the label
            var label_hash: [32]u8 = undefined;
            std.crypto.hash.sha3.Keccak256.hash(part, &label_hash, .{});
            
            // Combine with current hash
            var combined: [64]u8 = undefined;
            @memcpy(combined[0..32], &hash);
            @memcpy(combined[32..64], &label_hash);
            
            std.crypto.hash.sha3.Keccak256.hash(&combined, &hash, .{});
        }
        
        return hash;
    }
    
    fn encode_resolver_call(self: *@This(), namehash: [32]u8) ![]const u8 {
        // Function selector for resolver(bytes32) = 0x0178b8bf
        var call_data: [68]u8 = undefined;
        
        // Function selector
        call_data[0] = 0x01;
        call_data[1] = 0x78;
        call_data[2] = 0xb8;
        call_data[3] = 0xbf;
        
        // Namehash parameter
        @memcpy(call_data[4..36], &namehash);
        
        return try std.fmt.allocPrint(self.allocator, "{x}", .{call_data});
    }
    
    fn encode_addr_call(self: *@This(), namehash: [32]u8) ![]const u8 {
        // Function selector for addr(bytes32) = 0x3b3b57de
        var call_data: [36]u8 = undefined;
        
        call_data[0] = 0x3b;
        call_data[1] = 0x3b;
        call_data[2] = 0x57;
        call_data[3] = 0xde;
        
        @memcpy(call_data[4..36], &namehash);
        
        return try std.fmt.allocPrint(self.allocator, "{x}", .{call_data});
    }
    
    fn encode_text_call(self: *@This(), namehash: [32]u8, key: []const u8) ![]const u8 {
        // Function selector for text(bytes32, string) = 0x59d1d43c
        // This is simplified - proper ABI encoding would be more complex
        _ = namehash;
        _ = key;
        return try self.allocator.dupe(u8, "59d1d43c");
    }
    
    fn encode_contenthash_call(self: *@This(), namehash: [32]u8) ![]const u8 {
        // Function selector for contenthash(bytes32) = 0xbc1c58d1
        _ = namehash;
        return try self.allocator.dupe(u8, "bc1c58d1");
    }
    
    fn hex_to_ip_address(self: *@This(), hex: []const u8) ![]const u8 {
        if (hex.len != 8) return error.InvalidHexLength;
        
        var bytes: [4]u8 = undefined;
        _ = try std.fmt.hexToBytes(&bytes, hex);
        
        return try std.fmt.allocPrint(self.allocator, "{}.{}.{}.{}", .{ bytes[0], bytes[1], bytes[2], bytes[3] });
    }
    
    fn decode_string_result(self: *@This(), result: []const u8) ![]const u8 {
        // Simplified string decoding from ABI-encoded result
        if (result.len < 128) return "";
        
        // In real implementation, would properly decode ABI string
        return try self.allocator.dupe(u8, "decoded_string_placeholder");
    }
    
    fn decode_bytes_result(self: *@This(), result: []const u8) ![]const u8 {
        // Simplified bytes decoding from ABI-encoded result
        if (result.len < 128) return "";
        
        return try self.allocator.dupe(u8, "decoded_bytes_placeholder");
    }
    
    fn create_ens_metadata(self: *@This(), _: []const u8) ?zns_types.DomainMetadata {
        return zns_types.DomainMetadata{
            .version = 1,
            .registrar = "ENS",
            .tags = null,
            .description = self.allocator.dupe(u8, "Ethereum Name Service domain") catch null,
            .avatar = null,
            .website = null,
            .social = null,
        };
    }
    
    fn create_error_response(_: *@This(), domain: []const u8, error_code: zns_types.ZNSErrorCode, message: []const u8) zns_types.ZNSResolveResponse {
        return zns_types.ZNSResolveResponse{
            .domain = domain,
            .records = &[_]zns_types.DnsRecord{},
            .metadata = null,
            .resolution_info = zns_types.ResolutionInfo{
                .source = .ENS_BRIDGE,
                .resolution_time_ms = 0,
                .was_cached = false,
                .resolved_at = @as(u64, @intCast(std.time.timestamp())),
                .resolver_version = "ENS-Bridge-1.0",
                .resolution_path = @as([][]const u8, @constCast(&[_][]const u8{"ens_bridge"})),
            },
            .zns_error = zns_types.ZNSError{
                .code = error_code,
                .message = message,
                .details = domain,
                .resolution_chain = @as([][]const u8, @constCast(&[_][]const u8{"ens_bridge"})),
            },
        };
    }
};