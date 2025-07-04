const std = @import("std");

// ZNS Record Types
pub const DnsRecordType = enum {
    A,           // IPv4 address
    AAAA,        // IPv6 address
    CNAME,       // Canonical name
    MX,          // Mail exchange
    TXT,         // Text record
    SRV,         // Service record
    NS,          // Name server
    SOA,         // Start of authority
    PTR,         // Pointer record
    GHOST,       // GhostChain-specific metadata
    CONTRACT,    // Smart contract address
    WALLET,      // Wallet address mapping
};

pub const DnsRecord = struct {
    record_type: DnsRecordType,
    name: []const u8,        // Domain name (e.g., "ghostkellz.ghost")
    value: []const u8,       // Record value
    ttl: u32,               // Time to live in seconds
    priority: ?u16,         // For MX, SRV records
    port: ?u16,             // For SRV records
    weight: ?u16,           // For SRV records
    target: ?[]const u8,    // For SRV, CNAME records
    created_at: u64,        // Unix timestamp
    signature: ?[]const u8, // Ed25519 signature for validation
};

pub const SocialLinks = struct {
    twitter: ?[]const u8,
    github: ?[]const u8,
    discord: ?[]const u8,
    telegram: ?[]const u8,
    linkedin: ?[]const u8,
    instagram: ?[]const u8,
};

pub const DomainMetadata = struct {
    version: u8,                  // Schema version
    registrar: []const u8,        // Registration source (ZNS, ENS, etc.)
    tags: ?[][]const u8,          // Optional tags/categories
    description: ?[]const u8,     // Optional description
    avatar: ?[]const u8,          // Optional avatar/logo URL
    website: ?[]const u8,         // Optional website URL
    social: ?SocialLinks,         // Optional social media links
};

pub const DomainData = struct {
    domain: []const u8,           // Full domain name
    owner: []const u8,            // Owner address/GhostID
    records: []DnsRecord,         // Array of DNS records
    contract_address: ?[]const u8, // Associated smart contract
    metadata: DomainMetadata,      // Additional domain metadata
    last_updated: u64,            // Unix timestamp
    expiry: ?u64,                 // Domain expiration (null for permanent)
    signature: []const u8,        // Owner signature for integrity
};

// ZNS Request/Response Types
pub const ZNSResolveRequest = struct {
    domain: []const u8,                    // Domain to resolve
    record_types: [][]const u8,            // Requested record types
    include_metadata: bool,                // Include domain metadata
    use_cache: bool,                       // Allow cached responses
    max_ttl: u32,                         // Maximum acceptable TTL
};

pub const ResolverSource = enum {
    ZNS_NATIVE,              // Native ZNS resolution
    ENS_BRIDGE,              // ENS bridge resolution
    UNSTOPPABLE_BRIDGE,      // Unstoppable Domains bridge
    TRADITIONAL_DNS,         // Traditional DNS fallback
    CACHE,                   // Local cache
};

pub const ResolutionInfo = struct {
    source: ResolverSource,            // Resolution source
    resolution_time_ms: u64,           // Resolution time in milliseconds
    was_cached: bool,                  // Whether result was from cache
    resolved_at: u64,                  // Resolution timestamp
    resolver_version: []const u8,      // Resolver version
    resolution_path: [][]const u8,     // Resolution chain (for debugging)
};

pub const ZNSErrorCode = enum {
    UNSPECIFIED,
    DOMAIN_NOT_FOUND,
    INVALID_DOMAIN,
    INVALID_RECORD_TYPE,
    PERMISSION_DENIED,
    SIGNATURE_INVALID,
    DOMAIN_EXPIRED,
    RESOLVER_UNAVAILABLE,
    TIMEOUT,
    RATE_LIMITED,
    INTERNAL_ERROR,
};

pub const ZNSError = struct {
    code: ZNSErrorCode,                // Error code
    message: []const u8,               // Human-readable error message
    details: []const u8,               // Detailed error information
    resolution_chain: [][]const u8,    // Resolution chain for debugging
};

pub const ZNSResolveResponse = struct {
    domain: []const u8,                    // Resolved domain name
    records: []DnsRecord,                  // DNS records found
    metadata: ?DomainMetadata,             // Domain metadata (if requested)
    resolution_info: ResolutionInfo,       // Resolution details
    zns_error: ?ZNSError,                      // Error information (if failed)
};

pub const ZNSRegisterRequest = struct {
    domain: []const u8,                    // Domain to register
    owner_address: []const u8,             // Owner's blockchain address
    initial_records: []DnsRecord,          // Initial DNS records
    metadata: DomainMetadata,              // Domain metadata
    expiry_timestamp: u64,                 // Expiration time (0 = permanent)
    signature: []const u8,                 // Owner's signature
};

pub const ZNSRegisterResponse = struct {
    success: bool,                     // Registration success
    transaction_hash: []const u8,      // Blockchain transaction hash
    domain: []const u8,                // Registered domain
    contract_address: []const u8,      // Smart contract address
    block_number: u64,                 // Block number of registration
    zns_error: ?ZNSError,                  // Error information (if failed)
};

pub const UpdateAction = enum {
    ADD,        // Add new records
    UPDATE,     // Update existing records
    DELETE,     // Delete records
    REPLACE,    // Replace all records
};

pub const ZNSUpdateRequest = struct {
    domain: []const u8,                    // Domain to update
    records: []DnsRecord,                  // New/updated records
    action: UpdateAction,                  // Update action
    owner_signature: []const u8,           // Owner's signature
    transaction_id: []const u8,            // Optional transaction reference
};

pub const ZNSUpdateResponse = struct {
    success: bool,                     // Update success
    transaction_hash: []const u8,      // Blockchain transaction hash
    updated_records: []DnsRecord,      // Successfully updated records
    zns_error: ?ZNSError,                  // Error information (if failed)
};

// Subscription Types
pub const ZNSDomainSubscription = struct {
    domains: [][]const u8,          // Specific domains to watch (empty = all)
    record_types: []DnsRecordType,  // Record types to watch (empty = all)
    include_metadata: bool,         // Include metadata in events
};

pub const ChangeEventType = enum {
    DOMAIN_REGISTERED,     // New domain registered
    DOMAIN_UPDATED,        // Domain records updated
    DOMAIN_TRANSFERRED,    // Domain ownership transferred
    DOMAIN_EXPIRED,        // Domain expired
    DOMAIN_RENEWED,        // Domain renewed
};

pub const ZNSDomainChangeEvent = struct {
    domain: []const u8,                    // Changed domain
    event_type: ChangeEventType,           // Type of change
    old_records: []DnsRecord,              // Previous records
    new_records: []DnsRecord,              // New records
    timestamp: u64,                        // Event timestamp
    transaction_hash: []const u8,          // Associated blockchain transaction
};

// Cache Types
pub const CacheSource = enum {
    zns_native,              // Native ZNS resolution
    ens_bridge,              // ENS bridge resolution
    unstoppable_bridge,      // Unstoppable Domains bridge
    traditional_dns,         // Traditional DNS fallback
    peer_cache,              // From peer node cache
    contract_sync,           // From smart contract sync
};

pub const CacheEntry = struct {
    domain_data: DomainData,
    cached_at: u64,          // Unix timestamp when cached
    expires_at: u64,         // When cache entry expires
    last_accessed: u64,      // Last access time for LRU
    hit_count: u32,          // Number of times accessed
    source: CacheSource,     // Where data originated
    size_bytes: u32,         // Memory footprint of this entry
    
    pub fn is_expired(self: *const @This()) bool {
        const now = @as(u64, @intCast(std.time.timestamp()));
        return now > self.expires_at;
    }
    
    pub fn time_until_expiry(self: *const @This()) u64 {
        const now = @as(u64, @intCast(std.time.timestamp()));
        if (self.expires_at <= now) return 0;
        return self.expires_at - now;
    }
    
    pub fn update_access_time(self: *@This()) void {
        self.last_accessed = @as(u64, @intCast(std.time.timestamp()));
        self.hit_count += 1;
    }
};

// Metrics Types
pub const CacheStatistics = struct {
    total_entries: usize,
    memory_usage_bytes: usize,
    max_memory_bytes: usize,
    total_hits: u64,
    total_misses: u64,
    total_evictions: u64,
    total_expirations: u64,
    hit_rate: f64,                        // 0.0 to 1.0
    memory_utilization: f64,              // 0.0 to 1.0
};

pub const ResolutionMetrics = struct {
    total_queries: u64,
    cache_hits: u64,
    cache_misses: u64,
    successful_resolutions: u64,
    failed_resolutions: u64,
    average_resolution_time_ms: f64,
    queries_by_tld: std.HashMap([]const u8, u64, std.hash_map.StringContext, std.hash_map.default_max_load_percentage),
    
    pub fn record_query(self: *@This(), domain: []const u8, was_cache_hit: bool, resolution_time_ms: u64, success: bool) void {
        self.total_queries += 1;
        
        if (was_cache_hit) {
            self.cache_hits += 1;
        } else {
            self.cache_misses += 1;
        }
        
        if (success) {
            self.successful_resolutions += 1;
        } else {
            self.failed_resolutions += 1;
        }
        
        // Update moving average for resolution time
        const alpha = 0.1; // Smoothing factor
        self.average_resolution_time_ms = alpha * @as(f64, @floatFromInt(resolution_time_ms)) + 
                                         (1.0 - alpha) * self.average_resolution_time_ms;
        
        // Track queries by TLD
        if (get_tld(domain)) |tld| {
            const current_count = self.queries_by_tld.get(tld) orelse 0;
            self.queries_by_tld.put(tld, current_count + 1) catch {};
        }
    }
    
    pub fn get_cache_hit_rate(self: *const @This()) f64 {
        if (self.total_queries == 0) return 0.0;
        return @as(f64, @floatFromInt(self.cache_hits)) / @as(f64, @floatFromInt(self.total_queries));
    }
    
    fn get_tld(domain: []const u8) ?[]const u8 {
        if (std.mem.lastIndexOf(u8, domain, ".")) |index| {
            return domain[index..];
        }
        return null;
    }
};