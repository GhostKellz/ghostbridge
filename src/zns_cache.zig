const std = @import("std");
const zns_types = @import("zns_types.zig");

pub const CacheConfig = struct {
    // Size limits
    max_entries: usize = 10000,           // Maximum number of cached domains
    max_memory_bytes: usize = 100 * 1024 * 1024, // 100MB memory limit
    
    // TTL configuration
    default_ttl: u32 = 3600,              // 1 hour default TTL
    min_ttl: u32 = 60,                    // 1 minute minimum TTL
    max_ttl: u32 = 86400,                 // 24 hours maximum TTL
    
    // Cleanup configuration
    cleanup_interval_ms: u64 = 300000,    // 5 minutes cleanup interval
    eviction_batch_size: u32 = 100,       // Number of entries to evict at once
    
    // Performance tuning
    initial_capacity: usize = 1000,       // Initial hash map capacity
    load_factor: f64 = 0.75,              // Hash map load factor
    
    pub fn development() CacheConfig {
        return CacheConfig{
            .max_entries = 1000,
            .max_memory_bytes = 10 * 1024 * 1024, // 10MB
            .default_ttl = 300,                   // 5 minutes
            .cleanup_interval_ms = 60000,         // 1 minute
        };
    }
    
    pub fn production() CacheConfig {
        return CacheConfig{
            .max_entries = 100000,
            .max_memory_bytes = 1024 * 1024 * 1024, // 1GB
            .default_ttl = 3600,                    // 1 hour
            .cleanup_interval_ms = 300000,          // 5 minutes
        };
    }
    
    pub fn high_performance() CacheConfig {
        return CacheConfig{
            .max_entries = 1000000,
            .max_memory_bytes = 4 * 1024 * 1024 * 1024, // 4GB
            .default_ttl = 7200,                        // 2 hours
            .cleanup_interval_ms = 600000,              // 10 minutes
        };
    }
};

pub const DomainCache = struct {
    const Self = @This();
    
    allocator: std.mem.Allocator,
    entries: std.HashMap([]const u8, zns_types.CacheEntry, std.hash_map.StringContext, std.hash_map.default_max_load_percentage),
    lru_list: std.ArrayList([]const u8), // Simple LRU implementation
    
    // Configuration
    max_entries: usize,
    max_memory_bytes: usize,
    default_ttl: u32,
    min_ttl: u32,
    max_ttl: u32,
    
    // Statistics
    current_memory_bytes: usize,
    total_hits: u64,
    total_misses: u64,
    total_evictions: u64,
    total_expirations: u64,
    
    // Background cleanup
    cleanup_interval_ms: u64,
    last_cleanup_time: u64,
    
    pub fn init(allocator: std.mem.Allocator, config: CacheConfig) !Self {
        return Self{
            .allocator = allocator,
            .entries = std.HashMap([]const u8, zns_types.CacheEntry, std.hash_map.StringContext, std.hash_map.default_max_load_percentage).init(allocator),
            .lru_list = std.ArrayList([]const u8).init(allocator),
            .max_entries = config.max_entries,
            .max_memory_bytes = config.max_memory_bytes,
            .default_ttl = config.default_ttl,
            .min_ttl = config.min_ttl,
            .max_ttl = config.max_ttl,
            .current_memory_bytes = 0,
            .total_hits = 0,
            .total_misses = 0,
            .total_evictions = 0,
            .total_expirations = 0,
            .cleanup_interval_ms = config.cleanup_interval_ms,
            .last_cleanup_time = @as(u64, @intCast(std.time.milliTimestamp())),
        };
    }
    
    pub fn deinit(self: *Self) void {
        self.clear();
        self.entries.deinit();
        self.lru_list.deinit();
    }
    
    /// Get domain from cache, returns null if not found or expired
    pub fn get_domain(self: *Self, domain: []const u8) ?*const zns_types.DomainData {
        if (self.entries.getPtr(domain)) |entry| {
            // Check if entry is expired
            if (entry.is_expired()) {
                _ = self.remove_entry(domain);
                self.total_expirations += 1;
                self.total_misses += 1;
                return null;
            }
            
            // Update access statistics
            entry.update_access_time();
            self.update_lru_position(domain);
            self.total_hits += 1;
            
            return &entry.domain_data;
        }
        
        self.total_misses += 1;
        return null;
    }
    
    /// Cache domain data with specified TTL
    pub fn cache_domain(self: *Self, domain_data: zns_types.DomainData, ttl: ?u32, source: zns_types.CacheSource) !void {
        const effective_ttl = self.calculate_effective_ttl(ttl);
        const now = @as(u64, @intCast(std.time.timestamp()));
        
        // Calculate memory footprint
        const entry_size = self.calculate_entry_size(&domain_data);
        
        // Check if we need to make space
        try self.ensure_space_available(entry_size);
        
        // Create cache entry
        const entry = zns_types.CacheEntry{
            .domain_data = try self.deep_copy_domain_data(domain_data),
            .cached_at = now,
            .expires_at = now + effective_ttl,
            .last_accessed = now,
            .hit_count = 0,
            .source = source,
            .size_bytes = entry_size,
        };
        
        // Store domain name copy for the key
        const domain_key = try self.allocator.dupe(u8, domain_data.domain);
        
        // Remove existing entry if present
        if (self.entries.contains(domain_key)) {
            _ = self.remove_entry(domain_key);
        }
        
        // Add to cache
        try self.entries.put(domain_key, entry);
        try self.add_to_lru(domain_key);
        self.current_memory_bytes += entry_size;
        
        // Periodic cleanup
        try self.maybe_cleanup();
    }
    
    /// Remove domain from cache
    pub fn remove_domain(self: *Self, domain: []const u8) bool {
        return self.remove_entry(domain);
    }
    
    /// Clear all cached entries
    pub fn clear(self: *Self) void {
        var iterator = self.entries.iterator();
        while (iterator.next()) |entry| {
            self.allocator.free(entry.key_ptr.*);
            self.free_domain_data(&entry.value_ptr.domain_data);
        }
        
        self.entries.clearAndFree();
        self.lru_list.clearAndFree();
        self.current_memory_bytes = 0;
    }
    
    /// Get cache statistics
    pub fn get_statistics(self: *const Self) zns_types.CacheStatistics {
        const total_queries = self.total_hits + self.total_misses;
        const hit_rate = if (total_queries > 0) 
            @as(f64, @floatFromInt(self.total_hits)) / @as(f64, @floatFromInt(total_queries))
        else 0.0;
        
        return zns_types.CacheStatistics{
            .total_entries = self.entries.count(),
            .memory_usage_bytes = self.current_memory_bytes,
            .max_memory_bytes = self.max_memory_bytes,
            .total_hits = self.total_hits,
            .total_misses = self.total_misses,
            .total_evictions = self.total_evictions,
            .total_expirations = self.total_expirations,
            .hit_rate = hit_rate,
            .memory_utilization = @as(f64, @floatFromInt(self.current_memory_bytes)) / @as(f64, @floatFromInt(self.max_memory_bytes)),
        };
    }
    
    /// Background cleanup of expired entries
    pub fn cleanup_expired_entries(self: *Self) !u32 {
        var expired_domains = std.ArrayList([]const u8).init(self.allocator);
        defer expired_domains.deinit();
        
        var iterator = self.entries.iterator();
        while (iterator.next()) |entry| {
            if (entry.value_ptr.is_expired()) {
                try expired_domains.append(entry.key_ptr.*);
            }
        }
        
        for (expired_domains.items) |domain| {
            _ = self.remove_entry(domain);
            self.total_expirations += 1;
        }
        
        return @as(u32, @intCast(expired_domains.items.len));
    }
    
    // Private helper methods
    fn calculate_effective_ttl(self: *const Self, requested_ttl: ?u32) u32 {
        const ttl = requested_ttl orelse self.default_ttl;
        return std.math.clamp(ttl, self.min_ttl, self.max_ttl);
    }
    
    fn calculate_entry_size(_: *const Self, domain_data: *const zns_types.DomainData) u32 {
        var size: u32 = 0;
        
        // Domain name
        size += @as(u32, @intCast(domain_data.domain.len));
        
        // Owner
        size += @as(u32, @intCast(domain_data.owner.len));
        
        // Records
        for (domain_data.records) |dns_record| {
            size += @as(u32, @intCast(dns_record.name.len));
            size += @as(u32, @intCast(dns_record.value.len));
            if (dns_record.target) |target| {
                size += @as(u32, @intCast(target.len));
            }
            if (dns_record.signature) |sig| {
                size += @as(u32, @intCast(sig.len));
            }
        }
        
        // Contract address
        if (domain_data.contract_address) |addr| {
            size += @as(u32, @intCast(addr.len));
        }
        
        // Metadata
        size += @as(u32, @intCast(domain_data.metadata.registrar.len));
        if (domain_data.metadata.description) |desc| {
            size += @as(u32, @intCast(desc.len));
        }
        
        // Signature
        size += @as(u32, @intCast(domain_data.signature.len));
        
        // Add overhead for structs and pointers
        size += 256; // Estimated overhead
        
        return size;
    }
    
    fn ensure_space_available(self: *Self, required_bytes: u32) !void {
        // Check memory limit
        if (self.current_memory_bytes + required_bytes > self.max_memory_bytes) {
            try self.evict_lru_entries(required_bytes);
        }
        
        // Check entry count limit
        if (self.entries.count() >= self.max_entries) {
            try self.evict_lru_entries(0); // Evict at least one entry
        }
    }
    
    fn evict_lru_entries(self: *Self, min_bytes_to_free: u32) !void {
        var bytes_freed: u32 = 0;
        var entries_to_evict = std.ArrayList([]const u8).init(self.allocator);
        defer entries_to_evict.deinit();
        
        // Find LRU entries to evict (from end of list)
        var i = self.lru_list.items.len;
        while (i > 0 and (bytes_freed < min_bytes_to_free or self.entries.count() >= self.max_entries)) {
            i -= 1;
            const domain = self.lru_list.items[i];
            
            if (self.entries.get(domain)) |entry| {
                bytes_freed += entry.size_bytes;
                try entries_to_evict.append(domain);
            }
        }
        
        // Evict the selected entries
        for (entries_to_evict.items) |domain| {
            _ = self.remove_entry(domain);
            self.total_evictions += 1;
        }
    }
    
    fn remove_entry(self: *Self, domain: []const u8) bool {
        if (self.entries.fetchRemove(domain)) |removed| {
            self.current_memory_bytes -= removed.value.size_bytes;
            self.remove_from_lru(domain);
            self.free_domain_data(&removed.value.domain_data);
            self.allocator.free(removed.key);
            return true;
        }
        return false;
    }
    
    fn update_lru_position(self: *Self, domain: []const u8) void {
        self.remove_from_lru(domain);
        self.add_to_lru(domain) catch {};
    }
    
    fn add_to_lru(self: *Self, domain: []const u8) !void {
        // Add to front of LRU list
        try self.lru_list.insert(0, domain);
    }
    
    fn remove_from_lru(self: *Self, domain: []const u8) void {
        // Find and remove from LRU list
        for (self.lru_list.items, 0..) |item, i| {
            if (std.mem.eql(u8, item, domain)) {
                _ = self.lru_list.orderedRemove(i);
                break;
            }
        }
    }
    
    fn deep_copy_domain_data(self: *Self, domain_data: zns_types.DomainData) !zns_types.DomainData {
        // Deep copy all strings and arrays in domain_data
        const copied_domain = try self.allocator.dupe(u8, domain_data.domain);
        const copied_owner = try self.allocator.dupe(u8, domain_data.owner);
        
        // Copy records array
        var copied_records = try self.allocator.alloc(zns_types.DnsRecord, domain_data.records.len);
        for (domain_data.records, 0..) |dns_record, i| {
            copied_records[i] = try self.deep_copy_dns_record(dns_record);
        }
        
        // Copy contract address if present
        const copied_contract_address = if (domain_data.contract_address) |addr|
            try self.allocator.dupe(u8, addr)
        else
            null;
        
        // Copy signature
        const copied_signature = try self.allocator.dupe(u8, domain_data.signature);
        
        return zns_types.DomainData{
            .domain = copied_domain,
            .owner = copied_owner,
            .records = copied_records,
            .contract_address = copied_contract_address,
            .metadata = try self.deep_copy_metadata(domain_data.metadata),
            .last_updated = domain_data.last_updated,
            .expiry = domain_data.expiry,
            .signature = copied_signature,
        };
    }
    
    fn deep_copy_dns_record(self: *Self, dns_record: zns_types.DnsRecord) !zns_types.DnsRecord {
        return zns_types.DnsRecord{
            .record_type = dns_record.record_type,
            .name = try self.allocator.dupe(u8, dns_record.name),
            .value = try self.allocator.dupe(u8, dns_record.value),
            .ttl = dns_record.ttl,
            .priority = dns_record.priority,
            .port = dns_record.port,
            .weight = dns_record.weight,
            .target = if (dns_record.target) |target| try self.allocator.dupe(u8, target) else null,
            .created_at = dns_record.created_at,
            .signature = if (dns_record.signature) |sig| try self.allocator.dupe(u8, sig) else null,
        };
    }
    
    fn deep_copy_metadata(self: *Self, metadata: zns_types.DomainMetadata) !zns_types.DomainMetadata {
        return zns_types.DomainMetadata{
            .version = metadata.version,
            .registrar = try self.allocator.dupe(u8, metadata.registrar),
            .tags = if (metadata.tags) |tags| try self.deep_copy_string_array(tags) else null,
            .description = if (metadata.description) |desc| try self.allocator.dupe(u8, desc) else null,
            .avatar = if (metadata.avatar) |avatar| try self.allocator.dupe(u8, avatar) else null,
            .website = if (metadata.website) |website| try self.allocator.dupe(u8, website) else null,
            .social = if (metadata.social) |social| try self.deep_copy_social_links(social) else null,
        };
    }
    
    fn deep_copy_string_array(self: *Self, strings: [][]const u8) ![][]const u8 {
        var copied_strings = try self.allocator.alloc([]const u8, strings.len);
        for (strings, 0..) |string, i| {
            copied_strings[i] = try self.allocator.dupe(u8, string);
        }
        return copied_strings;
    }
    
    fn deep_copy_social_links(self: *Self, social: zns_types.SocialLinks) !zns_types.SocialLinks {
        return zns_types.SocialLinks{
            .twitter = if (social.twitter) |twitter| try self.allocator.dupe(u8, twitter) else null,
            .github = if (social.github) |github| try self.allocator.dupe(u8, github) else null,
            .discord = if (social.discord) |discord| try self.allocator.dupe(u8, discord) else null,
            .telegram = if (social.telegram) |telegram| try self.allocator.dupe(u8, telegram) else null,
            .linkedin = if (social.linkedin) |linkedin| try self.allocator.dupe(u8, linkedin) else null,
            .instagram = if (social.instagram) |instagram| try self.allocator.dupe(u8, instagram) else null,
        };
    }
    
    fn free_domain_data(self: *Self, domain_data: *const zns_types.DomainData) void {
        // Free all allocated memory in domain_data
        self.allocator.free(domain_data.domain);
        self.allocator.free(domain_data.owner);
        
        for (domain_data.records) |dns_record| {
            self.allocator.free(dns_record.name);
            self.allocator.free(dns_record.value);
            if (dns_record.target) |target| {
                self.allocator.free(target);
            }
            if (dns_record.signature) |sig| {
                self.allocator.free(sig);
            }
        }
        self.allocator.free(domain_data.records);
        
        if (domain_data.contract_address) |addr| {
            self.allocator.free(addr);
        }
        
        self.allocator.free(domain_data.signature);
        
        // Free metadata
        self.allocator.free(domain_data.metadata.registrar);
        if (domain_data.metadata.description) |desc| {
            self.allocator.free(desc);
        }
        if (domain_data.metadata.avatar) |avatar| {
            self.allocator.free(avatar);
        }
        if (domain_data.metadata.website) |website| {
            self.allocator.free(website);
        }
        if (domain_data.metadata.tags) |tags| {
            for (tags) |tag| {
                self.allocator.free(tag);
            }
            self.allocator.free(tags);
        }
        if (domain_data.metadata.social) |social| {
            if (social.twitter) |twitter| self.allocator.free(twitter);
            if (social.github) |github| self.allocator.free(github);
            if (social.discord) |discord| self.allocator.free(discord);
            if (social.telegram) |telegram| self.allocator.free(telegram);
            if (social.linkedin) |linkedin| self.allocator.free(linkedin);
            if (social.instagram) |instagram| self.allocator.free(instagram);
        }
    }
    
    fn maybe_cleanup(self: *Self) !void {
        const now = @as(u64, @intCast(std.time.milliTimestamp()));
        if (now - self.last_cleanup_time >= self.cleanup_interval_ms) {
            _ = try self.cleanup_expired_entries();
            self.last_cleanup_time = now;
        }
    }
};