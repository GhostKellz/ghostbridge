const std = @import("std");
const zns_types = @import("zns_types.zig");
const zns_cache = @import("zns_cache.zig");
const zns_validator = @import("zns_validator.zig");
const ens_resolver = @import("resolvers/ens.zig");
const ud_resolver = @import("resolvers/ud.zig");

pub const ZNSResolver = struct {
    allocator: std.mem.Allocator,
    cache: *zns_cache.DomainCache,
    metrics: *zns_types.ResolutionMetrics,
    rate_limiter: *zns_validator.RateLimiter,
    
    // Resolver backends
    native_resolver: NativeZNSResolver,
    ens_resolver: ens_resolver.ENSResolver,
    ud_resolver: ud_resolver.UnstoppableDomainsResolver,
    dns_resolver: TraditionalDNSResolver,
    
    // Configuration
    enable_cache: bool,
    enable_ens_bridge: bool,
    enable_ud_bridge: bool,
    enable_dns_fallback: bool,
    max_resolution_time_ms: u64,
    
    pub fn init(allocator: std.mem.Allocator, config: ZNSResolverConfig) !@This() {
        const cache = try allocator.create(zns_cache.DomainCache);
        cache.* = try zns_cache.DomainCache.init(allocator, config.cache_config);
        
        const metrics = try allocator.create(zns_types.ResolutionMetrics);
        metrics.* = zns_types.ResolutionMetrics{
            .total_queries = 0,
            .cache_hits = 0,
            .cache_misses = 0,
            .successful_resolutions = 0,
            .failed_resolutions = 0,
            .average_resolution_time_ms = 0.0,
            .queries_by_tld = std.HashMap([]const u8, u64, std.hash_map.StringContext, std.hash_map.default_max_load_percentage).init(allocator),
        };
        
        const rate_limiter = try allocator.create(zns_validator.RateLimiter);
        rate_limiter.* = zns_validator.RateLimiter.init(allocator, config.rate_limit_per_minute);
        
        return @This(){
            .allocator = allocator,
            .cache = cache,
            .metrics = metrics,
            .rate_limiter = rate_limiter,
            .native_resolver = NativeZNSResolver.init(allocator, config.ghost_node_endpoint),
            .ens_resolver = ens_resolver.ENSResolver.init(allocator, config.ethereum_rpc_endpoint),
            .ud_resolver = ud_resolver.UnstoppableDomainsResolver.init(allocator, config.unstoppable_api_key),
            .dns_resolver = TraditionalDNSResolver.init(allocator),
            .enable_cache = config.enable_cache,
            .enable_ens_bridge = config.enable_ens_bridge,
            .enable_ud_bridge = config.enable_ud_bridge,
            .enable_dns_fallback = config.enable_dns_fallback,
            .max_resolution_time_ms = config.max_resolution_time_ms,
        };
    }
    
    pub fn deinit(self: *@This()) void {
        self.cache.deinit();
        self.allocator.destroy(self.cache);
        
        self.metrics.queries_by_tld.deinit();
        self.allocator.destroy(self.metrics);
        
        self.rate_limiter.deinit();
        self.allocator.destroy(self.rate_limiter);
    }
    
    pub fn resolve_domain(self: *@This(), request: zns_types.ZNSResolveRequest, client_id: []const u8) !zns_types.ZNSResolveResponse {
        const start_time = std.time.milliTimestamp();
        
        // Rate limiting
        if (!self.rate_limiter.is_allowed(client_id)) {
            return self.create_rate_limited_response(request.domain);
        }
        
        // Domain validation
        if (!zns_validator.DomainValidator.is_valid_domain(request.domain)) {
            return self.create_invalid_domain_response(request.domain);
        }
        
        // Check cache first if enabled and requested
        if (self.enable_cache and request.use_cache) {
            if (self.cache.get_domain(request.domain)) |cached_data| {
                const end_time = std.time.milliTimestamp();
                const resolution_time = @as(u64, @intCast(end_time - start_time));
                
                self.metrics.record_query(request.domain, true, resolution_time, true);
                
                return self.create_cached_response(cached_data, resolution_time);
            }
        }
        
        // Determine resolver priority based on domain category
        const domain_category = zns_validator.DomainValidator.get_domain_category(request.domain);
        const resolvers = try self.get_resolver_priority(domain_category);
        
        // Try resolvers in priority order
        var last_error: ?zns_types.ZNSError = null;
        for (resolvers) |resolver_type| {
            const result = try self.try_resolver(resolver_type, request);
            
            if (result) |response| {
                if (response.zns_error == null) {
                    // Successful resolution - cache if enabled
                    if (self.enable_cache and response.records.len > 0) {
                        const domain_data = self.response_to_domain_data(response) catch continue;
                        const ttl = self.get_min_ttl_from_records(response.records);
                        const cache_source = self.resolver_source_to_cache_source(response.resolution_info.source);
                        self.cache.cache_domain(domain_data, ttl, cache_source) catch {};
                    }
                    
                    const end_time = std.time.milliTimestamp();
                    const resolution_time = @as(u64, @intCast(end_time - start_time));
                    self.metrics.record_query(request.domain, false, resolution_time, true);
                    
                    return response;
                } else {
                    last_error = response.zns_error;
                }
            }
        }
        
        // All resolvers failed
        const end_time = std.time.milliTimestamp();
        const resolution_time = @as(u64, @intCast(end_time - start_time));
        self.metrics.record_query(request.domain, false, resolution_time, false);
        
        return self.create_failed_response(request.domain, last_error);
    }
    
    pub fn register_domain(self: *@This(), request: zns_types.ZNSRegisterRequest, client_id: []const u8) !zns_types.ZNSRegisterResponse {
        // Rate limiting
        if (!self.rate_limiter.is_allowed(client_id)) {
            return zns_types.ZNSRegisterResponse{
                .success = false,
                .transaction_hash = "",
                .domain = request.domain,
                .contract_address = "",
                .block_number = 0,
                .zns_error = zns_types.ZNSError{
                    .code = .RATE_LIMITED,
                    .message = "Rate limit exceeded",
                    .details = "Too many requests",
                    .resolution_chain = &[_][]const u8{},
                },
            };
        }
        
        // Domain validation
        if (!zns_validator.DomainValidator.is_valid_domain(request.domain)) {
            return zns_types.ZNSRegisterResponse{
                .success = false,
                .transaction_hash = "",
                .domain = request.domain,
                .contract_address = "",
                .block_number = 0,
                .zns_error = zns_types.ZNSError{
                    .code = .INVALID_DOMAIN,
                    .message = "Invalid domain name",
                    .details = request.domain,
                    .resolution_chain = &[_][]const u8{},
                },
            };
        }
        
        // Only native ZNS domains can be registered through this interface
        const domain_category = zns_validator.DomainValidator.get_domain_category(request.domain);
        if (domain_category != .identity and domain_category != .infrastructure) {
            return zns_types.ZNSRegisterResponse{
                .success = false,
                .transaction_hash = "",
                .domain = request.domain,
                .contract_address = "",
                .block_number = 0,
                .zns_error = zns_types.ZNSError{
                    .code = .PERMISSION_DENIED,
                    .message = "Can only register native ZNS domains",
                    .details = request.domain,
                    .resolution_chain = &[_][]const u8{},
                },
            };
        }
        
        // Delegate to native resolver
        return try self.native_resolver.register_domain(request);
    }
    
    pub fn update_domain(self: *@This(), request: zns_types.ZNSUpdateRequest, client_id: []const u8) !zns_types.ZNSUpdateResponse {
        // Rate limiting
        if (!self.rate_limiter.is_allowed(client_id)) {
            return zns_types.ZNSUpdateResponse{
                .success = false,
                .transaction_hash = "",
                .updated_records = &[_]zns_types.DnsRecord{},
                .zns_error = zns_types.ZNSError{
                    .code = .RATE_LIMITED,
                    .message = "Rate limit exceeded",
                    .details = "Too many requests",
                    .resolution_chain = &[_][]const u8{},
                },
            };
        }
        
        // Only native ZNS domains can be updated
        const domain_category = zns_validator.DomainValidator.get_domain_category(request.domain);
        if (domain_category != .identity and domain_category != .infrastructure) {
            return zns_types.ZNSUpdateResponse{
                .success = false,
                .transaction_hash = "",
                .updated_records = &[_]zns_types.DnsRecord{},
                .zns_error = zns_types.ZNSError{
                    .code = .PERMISSION_DENIED,
                    .message = "Can only update native ZNS domains",
                    .details = request.domain,
                    .resolution_chain = &[_][]const u8{},
                },
            };
        }
        
        // Validate records
        for (request.records) |record| {
            const validation_result = zns_validator.RecordValidator.validate_record(&record);
            if (validation_result != .valid) {
                return zns_types.ZNSUpdateResponse{
                    .success = false,
                    .transaction_hash = "",
                    .updated_records = &[_]zns_types.DnsRecord{},
                    .zns_error = zns_types.ZNSError{
                        .code = .INVALID_RECORD_TYPE,
                        .message = "Invalid record data",
                        .details = record.value,
                        .resolution_chain = &[_][]const u8{},
                    },
                };
            }
        }
        
        // Delegate to native resolver
        const result = try self.native_resolver.update_domain(request);
        
        // Invalidate cache if update was successful
        if (result.success and self.enable_cache) {
            _ = self.cache.remove_domain(request.domain);
        }
        
        return result;
    }
    
    pub fn get_statistics(self: *const @This()) zns_types.CacheStatistics {
        return self.cache.get_statistics();
    }
    
    pub fn get_metrics(self: *const @This()) *const zns_types.ResolutionMetrics {
        return self.metrics;
    }
    
    // Private helper methods
    fn get_resolver_priority(self: *@This(), category: ?zns_validator.DomainCategory) ![]ResolverType {
        switch (category orelse .experimental) {
            .identity, .infrastructure => {
                return @as([]ResolverType, @constCast(&[_]ResolverType{ .native, .dns_fallback }));
            },
            .ens_bridge => {
                return if (self.enable_ens_bridge)
                    @as([]ResolverType, @constCast(&[_]ResolverType{ .ens, .dns_fallback }))
                else
                    @as([]ResolverType, @constCast(&[_]ResolverType{ .dns_fallback }));
            },
            .unstoppable_bridge => {
                return if (self.enable_ud_bridge)
                    @as([]ResolverType, @constCast(&[_]ResolverType{ .unstoppable, .dns_fallback }))
                else
                    @as([]ResolverType, @constCast(&[_]ResolverType{ .dns_fallback }));
            },
            .experimental => {
                var resolvers = std.ArrayList(ResolverType).init(self.allocator);
                try resolvers.append(.native);
                if (self.enable_ens_bridge) try resolvers.append(.ens);
                if (self.enable_ud_bridge) try resolvers.append(.unstoppable);
                if (self.enable_dns_fallback) try resolvers.append(.dns_fallback);
                return resolvers.toOwnedSlice();
            },
        }
    }
    
    fn try_resolver(self: *@This(), resolver_type: ResolverType, request: zns_types.ZNSResolveRequest) !?zns_types.ZNSResolveResponse {
        switch (resolver_type) {
            .native => return try self.native_resolver.resolve_domain(request.domain, request.record_types),
            .ens => return if (self.enable_ens_bridge) try self.ens_resolver.resolve_domain(request.domain, request.record_types) else null,
            .unstoppable => return if (self.enable_ud_bridge) try self.ud_resolver.resolve_domain(request.domain, request.record_types) else null,
            .dns_fallback => return if (self.enable_dns_fallback) try self.dns_resolver.resolve_domain(request.domain, request.record_types) else null,
        }
    }
    
    fn response_to_domain_data(_: *@This(), response: zns_types.ZNSResolveResponse) !zns_types.DomainData {
        return zns_types.DomainData{
            .domain = response.domain,
            .owner = "unknown", // Would need to extract from response
            .records = response.records,
            .contract_address = null,
            .metadata = response.metadata orelse zns_types.DomainMetadata{
                .version = 1,
                .registrar = "Unknown",
                .tags = null,
                .description = null,
                .avatar = null,
                .website = null,
                .social = null,
            },
            .last_updated = @as(u64, @intCast(std.time.timestamp())),
            .expiry = null,
            .signature = &[_]u8{},
        };
    }
    
    fn get_min_ttl_from_records(_: *@This(), records: []zns_types.DnsRecord) ?u32 {
        var min_ttl: ?u32 = null;
        for (records) |record| {
            if (min_ttl == null or record.ttl < min_ttl.?) {
                min_ttl = record.ttl;
            }
        }
        return min_ttl;
    }
    
    fn create_cached_response(_: *@This(), cached_data: *const zns_types.DomainData, resolution_time: u64) zns_types.ZNSResolveResponse {
        return zns_types.ZNSResolveResponse{
            .domain = cached_data.domain,
            .records = cached_data.records,
            .metadata = cached_data.metadata,
            .resolution_info = zns_types.ResolutionInfo{
                .source = .CACHE,
                .resolution_time_ms = resolution_time,
                .was_cached = true,
                .resolved_at = @as(u64, @intCast(std.time.timestamp())),
                .resolver_version = "ZNS-Resolver-1.0",
                .resolution_path = @as([][]const u8, @constCast(&[_][]const u8{"cache"})),
            },
            .zns_error = null,
        };
    }
    
    fn create_rate_limited_response(self: *@This(), domain: []const u8) zns_types.ZNSResolveResponse {
        return self.create_error_response(domain, .RATE_LIMITED, "Rate limit exceeded");
    }
    
    fn create_invalid_domain_response(self: *@This(), domain: []const u8) zns_types.ZNSResolveResponse {
        return self.create_error_response(domain, .INVALID_DOMAIN, "Invalid domain name");
    }
    
    fn create_failed_response(_: *@This(), domain: []const u8, last_error: ?zns_types.ZNSError) zns_types.ZNSResolveResponse {
        return zns_types.ZNSResolveResponse{
            .domain = domain,
            .records = &[_]zns_types.DnsRecord{},
            .metadata = null,
            .resolution_info = zns_types.ResolutionInfo{
                .source = .ZNS_NATIVE,
                .resolution_time_ms = 0,
                .was_cached = false,
                .resolved_at = @as(u64, @intCast(std.time.timestamp())),
                .resolver_version = "ZNS-Resolver-1.0",
                .resolution_path = @as([][]const u8, @constCast(&[_][]const u8{"zns_resolver"})),
            },
            .zns_error = last_error orelse zns_types.ZNSError{
                .code = .DOMAIN_NOT_FOUND,
                .message = "Domain not found in any resolver",
                .details = domain,
                .resolution_chain = &[_][]const u8{"all_resolvers"},
            },
        };
    }
    
    fn create_error_response(_: *@This(), domain: []const u8, error_code: zns_types.ZNSErrorCode, message: []const u8) zns_types.ZNSResolveResponse {
        return zns_types.ZNSResolveResponse{
            .domain = domain,
            .records = &[_]zns_types.DnsRecord{},
            .metadata = null,
            .resolution_info = zns_types.ResolutionInfo{
                .source = .ZNS_NATIVE,
                .resolution_time_ms = 0,
                .was_cached = false,
                .resolved_at = @as(u64, @intCast(std.time.timestamp())),
                .resolver_version = "ZNS-Resolver-1.0",
                .resolution_path = @as([][]const u8, @constCast(&[_][]const u8{"zns_resolver"})),
            },
            .zns_error = zns_types.ZNSError{
                .code = error_code,
                .message = message,
                .details = domain,
                .resolution_chain = @as([][]const u8, @constCast(&[_][]const u8{"zns_resolver"})),
            },
        };
    }
    
    fn resolver_source_to_cache_source(_: *@This(), resolver_source: zns_types.ResolverSource) zns_types.CacheSource {
        return switch (resolver_source) {
            .ZNS_NATIVE => .zns_native,
            .ENS_BRIDGE => .ens_bridge,
            .UNSTOPPABLE_BRIDGE => .unstoppable_bridge,
            .TRADITIONAL_DNS => .traditional_dns,
            .CACHE => .peer_cache,
        };
    }
};

const ResolverType = enum {
    native,
    ens,
    unstoppable,
    dns_fallback,
};

pub const ZNSResolverConfig = struct {
    cache_config: zns_cache.CacheConfig = zns_cache.CacheConfig.production(),
    rate_limit_per_minute: u32 = 1000,
    enable_cache: bool = true,
    enable_ens_bridge: bool = true,
    enable_ud_bridge: bool = true,
    enable_dns_fallback: bool = true,
    max_resolution_time_ms: u64 = 5000,
    ghost_node_endpoint: []const u8 = "quic://localhost:443",
    ethereum_rpc_endpoint: []const u8 = "https://eth-mainnet.alchemyapi.io/v2/demo",
    unstoppable_api_key: ?[]const u8 = null,
};

// Placeholder implementations for native and DNS resolvers
const NativeZNSResolver = struct {
    allocator: std.mem.Allocator,
    endpoint: []const u8,
    
    fn init(allocator: std.mem.Allocator, endpoint: []const u8) @This() {
        return @This(){
            .allocator = allocator,
            .endpoint = endpoint,
        };
    }
    
    fn resolve_domain(self: *@This(), domain: []const u8, record_types: [][]const u8) !?zns_types.ZNSResolveResponse {
        // TODO: Implement native ZNS resolution via QUIC to ghost node
        _ = self;
        _ = domain;
        _ = record_types;
        return null;
    }
    
    fn register_domain(self: *@This(), request: zns_types.ZNSRegisterRequest) !zns_types.ZNSRegisterResponse {
        // TODO: Implement native ZNS registration
        _ = self;
        _ = request;
        return zns_types.ZNSRegisterResponse{
            .success = false,
            .transaction_hash = "",
            .domain = "",
            .contract_address = "",
            .block_number = 0,
            .zns_error = zns_types.ZNSError{
                .code = .INTERNAL_ERROR,
                .message = "Native registration not implemented",
                .details = "",
                .resolution_chain = &[_][]const u8{},
            },
        };
    }
    
    fn update_domain(self: *@This(), request: zns_types.ZNSUpdateRequest) !zns_types.ZNSUpdateResponse {
        // TODO: Implement native ZNS updates
        _ = self;
        _ = request;
        return zns_types.ZNSUpdateResponse{
            .success = false,
            .transaction_hash = "",
            .updated_records = &[_]zns_types.DnsRecord{},
            .zns_error = zns_types.ZNSError{
                .code = .INTERNAL_ERROR,
                .message = "Native updates not implemented",
                .details = "",
                .resolution_chain = &[_][]const u8{},
            },
        };
    }
};

const TraditionalDNSResolver = struct {
    allocator: std.mem.Allocator,
    
    fn init(allocator: std.mem.Allocator) @This() {
        return @This(){
            .allocator = allocator,
        };
    }
    
    fn resolve_domain(self: *@This(), domain: []const u8, record_types: [][]const u8) !?zns_types.ZNSResolveResponse {
        // TODO: Implement traditional DNS resolution fallback
        _ = self;
        _ = domain;
        _ = record_types;
        return null;
    }
};