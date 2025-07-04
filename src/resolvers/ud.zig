const std = @import("std");
const zns_types = @import("../zns_types.zig");

pub const UnstoppableDomainsResolver = struct {
    allocator: std.mem.Allocator,
    api_endpoint: []const u8,
    api_key: ?[]const u8,
    rate_limit_per_second: u32,
    timeout_ms: u32,
    last_request_time: u64,
    request_count: u32,
    
    // Supported Unstoppable Domains TLDs
    const SUPPORTED_TLDS = [_][]const u8{
        ".crypto", ".nft", ".blockchain", ".bitcoin", ".coin", ".wallet",
        ".x", ".888", ".dao", ".zil", ".hi", ".klever", ".polygon",
        ".unstoppable", ".go", ".anime"
    };
    
    pub fn init(allocator: std.mem.Allocator, api_key: ?[]const u8) @This() {
        return @This(){
            .allocator = allocator,
            .api_endpoint = "https://resolve.unstoppabledomains.com",
            .api_key = api_key,
            .rate_limit_per_second = 50, // Conservative rate limit
            .timeout_ms = 5000,
            .last_request_time = 0,
            .request_count = 0,
        };
    }
    
    pub fn resolve_domain(self: *@This(), domain: []const u8, record_types: [][]const u8) !?zns_types.ZNSResolveResponse {
        if (!self.is_unstoppable_domain(domain)) {
            return null; // Not an Unstoppable domain
        }
        
        try self.check_rate_limit();
        
        const start_time = std.time.milliTimestamp();
        
        // Query Unstoppable Domains API
        const domain_data = try self.query_unstoppable_api(domain);
        if (domain_data == null) {
            return self.create_error_response(domain, .DOMAIN_NOT_FOUND, "Unstoppable domain not found");
        }
        
        // Convert API response to DNS records
        var records = std.ArrayList(zns_types.DnsRecord).init(self.allocator);
        defer records.deinit();
        
        for (record_types) |record_type| {
            if (try self.extract_record_from_data(domain, record_type, domain_data.?)) |record| {
                try records.append(record);
            }
        }
        
        const end_time = std.time.milliTimestamp();
        const resolution_time = @as(u64, @intCast(end_time - start_time));
        
        return zns_types.ZNSResolveResponse{
            .domain = domain,
            .records = try records.toOwnedSlice(),
            .metadata = try self.create_unstoppable_metadata(domain, domain_data.?),
            .resolution_info = zns_types.ResolutionInfo{
                .source = .UNSTOPPABLE_BRIDGE,
                .resolution_time_ms = resolution_time,
                .was_cached = false,
                .resolved_at = @as(u64, @intCast(std.time.timestamp())),
                .resolver_version = "UD-Bridge-1.0",
                .resolution_path = @as([][]const u8, @constCast(&[_][]const u8{ "unstoppable_bridge", "ud_api" })),
            },
            .zns_error = null,
        };
    }
    
    fn is_unstoppable_domain(_: *@This(), domain: []const u8) bool {
        for (SUPPORTED_TLDS) |tld| {
            if (std.mem.endsWith(u8, domain, tld)) {
                return true;
            }
        }
        return false;
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
    
    fn query_unstoppable_api(self: *@This(), domain: []const u8) !?UnstoppableDomainData {
        // Construct API URL
        const url = try std.fmt.allocPrint(self.allocator, "{s}/domains/{s}", .{ self.api_endpoint, domain });
        defer self.allocator.free(url);
        
        // Make HTTP request to Unstoppable Domains API
        const response = try self.make_http_request(url);
        defer self.allocator.free(response);
        
        if (response.len == 0) return null;
        
        // Parse JSON response
        return try self.parse_unstoppable_response(response);
    }
    
    fn make_http_request(self: *@This(), url: []const u8) ![]const u8 {
        // Simplified HTTP request - in production would use proper HTTP client
        // For now, return mock JSON response
        _ = url;
        
        const mock_response =
            \\{
            \\  "records": {
            \\    "crypto.ETH.address": "0x8aaD44321A86b170879d7A244c1e8d360c99DdA8",
            \\    "crypto.BTC.address": "bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh",
            \\    "dweb.ipfs.hash": "QmVaAtQbi3EtsfpKoLzALm6vXphdi2KjMgxEDKeGg6wHu7",
            \\    "dns.A": "10.0.0.1",
            \\    "dns.AAAA": "2001:db8::1",
            \\    "social.twitter.username": "unstoppableweb",
            \\    "social.discord.username": "unstoppable#1234",
            \\    "gundb.username.value": "unstoppable.crypto",
            \\    "browser.redirect_url": "https://unstoppable.example",
            \\    "ipfs.redirect_domain.value": "unstoppable.mypinata.cloud"
            \\  },
            \\  "meta": {
            \\    "domain": "unstoppable.crypto",
            \\    "owner": "0x8aaD44321A86b170879d7A244c1e8d360c99DdA8",
            \\    "resolver": "0xb66DcE2DA6afAAa98F2013446dBCB0f4B0ab2842",
            \\    "ttl": 300
            \\  }
            \\}
        ;
        
        return try self.allocator.dupe(u8, mock_response);
    }
    
    fn parse_unstoppable_response(self: *@This(), response: []const u8) !UnstoppableDomainData {
        // Simplified JSON parsing - in production would use proper JSON parser
        // For now, create mock data based on the response
        _ = response;
        
        var data = UnstoppableDomainData{
            .domain = try self.allocator.dupe(u8, "unstoppable.crypto"),
            .owner = try self.allocator.dupe(u8, "0x8aaD44321A86b170879d7A244c1e8d360c99DdA8"),
            .resolver = try self.allocator.dupe(u8, "0xb66DcE2DA6afAAa98F2013446dBCB0f4B0ab2842"),
            .ttl = 300,
            .records = std.HashMap([]const u8, []const u8, std.hash_map.StringContext, std.hash_map.default_max_load_percentage).init(self.allocator),
        };
        
        // Add mock records
        try data.records.put(try self.allocator.dupe(u8, "crypto.ETH.address"), try self.allocator.dupe(u8, "0x8aaD44321A86b170879d7A244c1e8d360c99DdA8"));
        try data.records.put(try self.allocator.dupe(u8, "crypto.BTC.address"), try self.allocator.dupe(u8, "bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh"));
        try data.records.put(try self.allocator.dupe(u8, "dns.A"), try self.allocator.dupe(u8, "10.0.0.1"));
        try data.records.put(try self.allocator.dupe(u8, "dns.AAAA"), try self.allocator.dupe(u8, "2001:db8::1"));
        try data.records.put(try self.allocator.dupe(u8, "social.twitter.username"), try self.allocator.dupe(u8, "unstoppableweb"));
        
        return data;
    }
    
    fn extract_record_from_data(self: *@This(), domain: []const u8, record_type: []const u8, data: UnstoppableDomainData) !?zns_types.DnsRecord {
        if (std.mem.eql(u8, record_type, "A")) {
            if (data.records.get("dns.A")) |ip| {
                return zns_types.DnsRecord{
                    .record_type = .A,
                    .name = domain,
                    .value = try self.allocator.dupe(u8, ip),
                    .ttl = data.ttl,
                    .priority = null,
                    .port = null,
                    .weight = null,
                    .target = null,
                    .created_at = @as(u64, @intCast(std.time.timestamp())),
                    .signature = null,
                };
            }
        } else if (std.mem.eql(u8, record_type, "AAAA")) {
            if (data.records.get("dns.AAAA")) |ip| {
                return zns_types.DnsRecord{
                    .record_type = .AAAA,
                    .name = domain,
                    .value = try self.allocator.dupe(u8, ip),
                    .ttl = data.ttl,
                    .priority = null,
                    .port = null,
                    .weight = null,
                    .target = null,
                    .created_at = @as(u64, @intCast(std.time.timestamp())),
                    .signature = null,
                };
            }
        } else if (std.mem.eql(u8, record_type, "TXT")) {
            // Create TXT records for social media and other metadata
            const social_keys = [_][]const u8{
                "social.twitter.username",
                "social.discord.username",
                "social.github.username",
                "social.telegram.username",
            };
            
            for (social_keys) |key| {
                if (data.records.get(key)) |value| {
                    const txt_value = try std.fmt.allocPrint(self.allocator, "{s}={s}", .{ key, value });
                    return zns_types.DnsRecord{
                        .record_type = .TXT,
                        .name = domain,
                        .value = txt_value,
                        .ttl = data.ttl,
                        .priority = null,
                        .port = null,
                        .weight = null,
                        .target = null,
                        .created_at = @as(u64, @intCast(std.time.timestamp())),
                        .signature = null,
                    };
                }
            }
        } else if (std.mem.eql(u8, record_type, "WALLET")) {
            // Extract crypto wallet addresses
            const wallet_keys = [_][]const u8{
                "crypto.ETH.address",
                "crypto.BTC.address",
                "crypto.LTC.address",
                "crypto.DOGE.address",
            };
            
            for (wallet_keys) |key| {
                if (data.records.get(key)) |address| {
                    return zns_types.DnsRecord{
                        .record_type = .WALLET,
                        .name = domain,
                        .value = try self.allocator.dupe(u8, address),
                        .ttl = data.ttl,
                        .priority = null,
                        .port = null,
                        .weight = null,
                        .target = null,
                        .created_at = @as(u64, @intCast(std.time.timestamp())),
                        .signature = null,
                    };
                }
            }
        } else if (std.mem.eql(u8, record_type, "CNAME")) {
            // Extract IPFS hash or redirect URL
            if (data.records.get("dweb.ipfs.hash")) |ipfs_hash| {
                return zns_types.DnsRecord{
                    .record_type = .CNAME,
                    .name = domain,
                    .value = try std.fmt.allocPrint(self.allocator, "ipfs://{s}", .{ipfs_hash}),
                    .ttl = data.ttl,
                    .priority = null,
                    .port = null,
                    .weight = null,
                    .target = null,
                    .created_at = @as(u64, @intCast(std.time.timestamp())),
                    .signature = null,
                };
            } else if (data.records.get("browser.redirect_url")) |url| {
                return zns_types.DnsRecord{
                    .record_type = .CNAME,
                    .name = domain,
                    .value = try self.allocator.dupe(u8, url),
                    .ttl = data.ttl,
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
    
    fn create_unstoppable_metadata(self: *@This(), _: []const u8, data: UnstoppableDomainData) !zns_types.DomainMetadata {
        // Extract social links from records
        var social_links: ?zns_types.SocialLinks = null;
        
        const twitter = data.records.get("social.twitter.username");
        const discord = data.records.get("social.discord.username");
        const github = data.records.get("social.github.username");
        const telegram = data.records.get("social.telegram.username");
        
        if (twitter != null or discord != null or github != null or telegram != null) {
            social_links = zns_types.SocialLinks{
                .twitter = if (twitter) |t| try self.allocator.dupe(u8, t) else null,
                .github = if (github) |g| try self.allocator.dupe(u8, g) else null,
                .discord = if (discord) |d| try self.allocator.dupe(u8, d) else null,
                .telegram = if (telegram) |tg| try self.allocator.dupe(u8, tg) else null,
                .linkedin = null,
                .instagram = null,
            };
        }
        
        // Extract website URL
        const website = data.records.get("browser.redirect_url");
        
        // Extract avatar if available
        const avatar = data.records.get("social.picture.value");
        
        return zns_types.DomainMetadata{
            .version = 1,
            .registrar = "Unstoppable Domains",
            .tags = null,
            .description = try self.allocator.dupe(u8, "Unstoppable Domains decentralized domain"),
            .avatar = if (avatar) |a| try self.allocator.dupe(u8, a) else null,
            .website = if (website) |w| try self.allocator.dupe(u8, w) else null,
            .social = social_links,
        };
    }
    
    fn create_error_response(_: *@This(), domain: []const u8, error_code: zns_types.ZNSErrorCode, message: []const u8) zns_types.ZNSResolveResponse {
        return zns_types.ZNSResolveResponse{
            .domain = domain,
            .records = &[_]zns_types.DnsRecord{},
            .metadata = null,
            .resolution_info = zns_types.ResolutionInfo{
                .source = .UNSTOPPABLE_BRIDGE,
                .resolution_time_ms = 0,
                .was_cached = false,
                .resolved_at = @as(u64, @intCast(std.time.timestamp())),
                .resolver_version = "UD-Bridge-1.0",
                .resolution_path = @as([][]const u8, @constCast(&[_][]const u8{"unstoppable_bridge"})),
            },
            .zns_error = zns_types.ZNSError{
                .code = error_code,
                .message = message,
                .details = domain,
                .resolution_chain = @as([][]const u8, @constCast(&[_][]const u8{"unstoppable_bridge"})),
            },
        };
    }
};

const UnstoppableDomainData = struct {
    domain: []const u8,
    owner: []const u8,
    resolver: []const u8,
    ttl: u32,
    records: std.HashMap([]const u8, []const u8, std.hash_map.StringContext, std.hash_map.default_max_load_percentage),
    
    pub fn deinit(self: *@This(), allocator: std.mem.Allocator) void {
        allocator.free(self.domain);
        allocator.free(self.owner);
        allocator.free(self.resolver);
        
        var iterator = self.records.iterator();
        while (iterator.next()) |entry| {
            allocator.free(entry.key_ptr.*);
            allocator.free(entry.value_ptr.*);
        }
        self.records.deinit();
    }
};