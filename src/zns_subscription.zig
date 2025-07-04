const std = @import("std");
const zns_types = @import("zns_types.zig");

pub const ZNSSubscriptionManager = struct {
    allocator: std.mem.Allocator,
    subscriptions: std.HashMap([]const u8, *Subscription, std.hash_map.StringContext, std.hash_map.default_max_load_percentage),
    domain_watchers: std.HashMap([]const u8, std.ArrayList(*Subscription), std.hash_map.StringContext, std.hash_map.default_max_load_percentage),
    event_queue: std.ArrayList(zns_types.ZNSDomainChangeEvent),
    event_processors: std.ArrayList(*EventProcessor),
    next_subscription_id: u64,
    
    pub fn init(allocator: std.mem.Allocator) @This() {
        return @This(){
            .allocator = allocator,
            .subscriptions = std.HashMap([]const u8, *Subscription, std.hash_map.StringContext, std.hash_map.default_max_load_percentage).init(allocator),
            .domain_watchers = std.HashMap([]const u8, std.ArrayList(*Subscription), std.hash_map.StringContext, std.hash_map.default_max_load_percentage).init(allocator),
            .event_queue = std.ArrayList(zns_types.ZNSDomainChangeEvent).init(allocator),
            .event_processors = std.ArrayList(*EventProcessor).init(allocator),
            .next_subscription_id = 1,
        };
    }
    
    pub fn deinit(self: *@This()) void {
        // Clean up all subscriptions
        var sub_iterator = self.subscriptions.iterator();
        while (sub_iterator.next()) |entry| {
            self.allocator.free(entry.key_ptr.*);
            entry.value_ptr.*.deinit();
            self.allocator.destroy(entry.value_ptr.*);
        }
        self.subscriptions.deinit();
        
        // Clean up domain watchers
        var watcher_iterator = self.domain_watchers.iterator();
        while (watcher_iterator.next()) |entry| {
            self.allocator.free(entry.key_ptr.*);
            entry.value_ptr.deinit();
        }
        self.domain_watchers.deinit();
        
        // Clean up event queue
        self.event_queue.deinit();
        
        // Clean up event processors
        for (self.event_processors.items) |processor| {
            processor.deinit();
            self.allocator.destroy(processor);
        }
        self.event_processors.deinit();
    }
    
    pub fn create_subscription(self: *@This(), request: zns_types.ZNSDomainSubscription, client_id: []const u8) ![]const u8 {
        const subscription_id = try std.fmt.allocPrint(self.allocator, "sub_{s}_{}", .{ client_id, self.next_subscription_id });
        self.next_subscription_id += 1;
        
        const subscription = try self.allocator.create(Subscription);
        subscription.* = try Subscription.init(self.allocator, subscription_id, request, client_id);
        
        // Register subscription
        try self.subscriptions.put(try self.allocator.dupe(u8, subscription_id), subscription);
        
        // Register domain watchers
        if (request.domains.len == 0) {
            // Watch all domains - add to special wildcard list
            const wildcard_key = try self.allocator.dupe(u8, "*");
            var watchers = self.domain_watchers.get(wildcard_key) orelse blk: {
                const new_list = std.ArrayList(*Subscription).init(self.allocator);
                try self.domain_watchers.put(wildcard_key, new_list);
                break :blk self.domain_watchers.getPtr(wildcard_key).?;
            };
            try watchers.append(subscription);
        } else {
            // Watch specific domains
            for (request.domains) |domain| {
                const domain_key = try self.allocator.dupe(u8, domain);
                var watchers = self.domain_watchers.get(domain_key) orelse blk: {
                    const new_list = std.ArrayList(*Subscription).init(self.allocator);
                    try self.domain_watchers.put(domain_key, new_list);
                    break :blk self.domain_watchers.getPtr(domain_key).?;
                };
                try watchers.append(subscription);
            }
        }
        
        return subscription_id;
    }
    
    pub fn cancel_subscription(self: *@This(), subscription_id: []const u8) bool {
        if (self.subscriptions.fetchRemove(subscription_id)) |removed| {
            const subscription = removed.value;
            
            // Remove from domain watchers
            self.remove_subscription_from_watchers(subscription);
            
            // Clean up
            subscription.deinit();
            self.allocator.destroy(subscription);
            self.allocator.free(removed.key);
            
            return true;
        }
        return false;
    }
    
    pub fn publish_domain_change(self: *@This(), event: zns_types.ZNSDomainChangeEvent) !void {
        // Add to event queue
        try self.event_queue.append(event);
        
        // Find all subscribers for this domain
        var subscribers = std.ArrayList(*Subscription).init(self.allocator);
        defer subscribers.deinit();
        
        // Check specific domain watchers
        if (self.domain_watchers.get(event.domain)) |watchers| {
            try subscribers.appendSlice(watchers.items);
        }
        
        // Check wildcard watchers
        if (self.domain_watchers.get("*")) |wildcard_watchers| {
            try subscribers.appendSlice(wildcard_watchers.items);
        }
        
        // Send event to all matching subscribers
        for (subscribers.items) |subscription| {
            if (subscription.should_receive_event(&event)) {
                try subscription.queue_event(event);
            }
        }
        
        // Process events asynchronously
        try self.process_queued_events();
    }
    
    pub fn get_subscription_events(self: *@This(), subscription_id: []const u8, max_events: u32) ![]zns_types.ZNSDomainChangeEvent {
        if (self.subscriptions.get(subscription_id)) |subscription| {
            return try subscription.get_queued_events(max_events);
        }
        return &[_]zns_types.ZNSDomainChangeEvent{};
    }
    
    pub fn add_event_processor(self: *@This(), processor: *EventProcessor) !void {
        try self.event_processors.append(processor);
    }
    
    pub fn get_subscription_count(self: *const @This()) usize {
        return self.subscriptions.count();
    }
    
    pub fn get_active_domain_count(self: *const @This()) usize {
        return self.domain_watchers.count();
    }
    
    // Private helper methods
    fn remove_subscription_from_watchers(self: *@This(), subscription: *Subscription) void {
        var watcher_iterator = self.domain_watchers.iterator();
        while (watcher_iterator.next()) |entry| {
            var watchers = entry.value_ptr;
            for (watchers.items, 0..) |watcher, i| {
                if (watcher == subscription) {
                    _ = watchers.swapRemove(i);
                    break;
                }
            }
        }
    }
    
    fn process_queued_events(self: *@This()) !void {
        for (self.event_processors.items) |processor| {
            try processor.process_events(&self.event_queue);
        }
    }
};

const Subscription = struct {
    allocator: std.mem.Allocator,
    id: []const u8,
    client_id: []const u8,
    domains: [][]const u8,
    record_types: []zns_types.DnsRecordType,
    include_metadata: bool,
    event_queue: std.ArrayList(zns_types.ZNSDomainChangeEvent),
    created_at: u64,
    last_activity: u64,
    
    fn init(allocator: std.mem.Allocator, id: []const u8, request: zns_types.ZNSDomainSubscription, client_id: []const u8) !@This() {
        // Deep copy domains
        var domains = try allocator.alloc([]const u8, request.domains.len);
        for (request.domains, 0..) |domain, i| {
            domains[i] = try allocator.dupe(u8, domain);
        }
        
        // Deep copy record types
        const record_types = try allocator.alloc(zns_types.DnsRecordType, request.record_types.len);
        @memcpy(record_types, request.record_types);
        
        const now = @as(u64, @intCast(std.time.timestamp()));
        
        return @This(){
            .allocator = allocator,
            .id = try allocator.dupe(u8, id),
            .client_id = try allocator.dupe(u8, client_id),
            .domains = domains,
            .record_types = record_types,
            .include_metadata = request.include_metadata,
            .event_queue = std.ArrayList(zns_types.ZNSDomainChangeEvent).init(allocator),
            .created_at = now,
            .last_activity = now,
        };
    }
    
    fn deinit(self: *@This()) void {
        self.allocator.free(self.id);
        self.allocator.free(self.client_id);
        
        for (self.domains) |domain| {
            self.allocator.free(domain);
        }
        self.allocator.free(self.domains);
        self.allocator.free(self.record_types);
        
        self.event_queue.deinit();
    }
    
    fn should_receive_event(self: *const @This(), event: *const zns_types.ZNSDomainChangeEvent) bool {
        // Check if we're watching this domain (empty domains means watch all)
        if (self.domains.len > 0) {
            var found = false;
            for (self.domains) |domain| {
                if (std.mem.eql(u8, domain, event.domain)) {
                    found = true;
                    break;
                }
            }
            if (!found) return false;
        }
        
        // Check if we're interested in the record types affected
        if (self.record_types.len > 0) {
            var found = false;
            for (event.new_records) |record| {
                for (self.record_types) |watched_type| {
                    if (record.record_type == watched_type) {
                        found = true;
                        break;
                    }
                }
                if (found) break;
            }
            if (!found) return false;
        }
        
        return true;
    }
    
    fn queue_event(self: *@This(), event: zns_types.ZNSDomainChangeEvent) !void {
        try self.event_queue.append(event);
        self.last_activity = @as(u64, @intCast(std.time.timestamp()));
        
        // Limit queue size to prevent memory issues
        const max_queue_size = 1000;
        if (self.event_queue.items.len > max_queue_size) {
            _ = self.event_queue.orderedRemove(0); // Remove oldest event
        }
    }
    
    fn get_queued_events(self: *@This(), max_events: u32) ![]zns_types.ZNSDomainChangeEvent {
        const event_count = std.math.min(self.event_queue.items.len, max_events);
        if (event_count == 0) return &[_]zns_types.ZNSDomainChangeEvent{};
        
        const events = try self.allocator.alloc(zns_types.ZNSDomainChangeEvent, event_count);
        @memcpy(events, self.event_queue.items[0..event_count]);
        
        // Remove retrieved events from queue
        self.event_queue.replaceRange(0, event_count, &[_]zns_types.ZNSDomainChangeEvent{}) catch {};
        
        self.last_activity = @as(u64, @intCast(std.time.timestamp()));
        
        return events;
    }
};

pub const EventProcessor = struct {
    allocator: std.mem.Allocator,
    processor_type: ProcessorType,
    config: ProcessorConfig,
    
    pub fn init(allocator: std.mem.Allocator, processor_type: ProcessorType, config: ProcessorConfig) @This() {
        return @This(){
            .allocator = allocator,
            .processor_type = processor_type,
            .config = config,
        };
    }
    
    pub fn deinit(self: *@This()) void {
        // Clean up processor-specific resources
        switch (self.processor_type) {
            .webhook => {
                if (self.config.webhook_url) |url| {
                    self.allocator.free(url);
                }
            },
            .quic_stream => {},
            .log_file => {
                if (self.config.log_file_path) |path| {
                    self.allocator.free(path);
                }
            },
        }
    }
    
    pub fn process_events(self: *@This(), events: *std.ArrayList(zns_types.ZNSDomainChangeEvent)) !void {
        switch (self.processor_type) {
            .webhook => try self.process_webhook_events(events),
            .quic_stream => try self.process_quic_stream_events(events),
            .log_file => try self.process_log_file_events(events),
        }
    }
    
    fn process_webhook_events(self: *@This(), events: *std.ArrayList(zns_types.ZNSDomainChangeEvent)) !void {
        if (self.config.webhook_url == null) return;
        
        for (events.items) |event| {
            // Serialize event to JSON
            const json_data = try self.serialize_event_to_json(event);
            defer self.allocator.free(json_data);
            
            // Send HTTP POST to webhook URL
            try self.send_webhook_request(self.config.webhook_url.?, json_data);
        }
    }
    
    fn process_quic_stream_events(_: *@This(), events: *std.ArrayList(zns_types.ZNSDomainChangeEvent)) !void {
        // TODO: Implement QUIC stream event broadcasting
        _ = events;
    }
    
    fn process_log_file_events(self: *@This(), events: *std.ArrayList(zns_types.ZNSDomainChangeEvent)) !void {
        if (self.config.log_file_path == null) return;
        
        const file = std.fs.cwd().openFile(self.config.log_file_path.?, .{ .mode = .write_only }) catch |err| switch (err) {
            error.FileNotFound => try std.fs.cwd().createFile(self.config.log_file_path.?, .{}),
            else => return err,
        };
        defer file.close();
        
        try file.seekFromEnd(0); // Append to end
        
        for (events.items) |event| {
            const log_line = try std.fmt.allocPrint(self.allocator, 
                "[{}] Domain: {} Event: {} Transaction: {s}\n",
                .{ event.timestamp, event.domain, @tagName(event.event_type), event.transaction_hash }
            );
            defer self.allocator.free(log_line);
            
            try file.writeAll(log_line);
        }
    }
    
    fn serialize_event_to_json(self: *@This(), event: zns_types.ZNSDomainChangeEvent) ![]u8 {
        // Simplified JSON serialization
        return try std.fmt.allocPrint(self.allocator,
            \\{{"domain": "{s}", "event_type": "{s}", "timestamp": {}, "transaction_hash": "{s}"}}
        , .{ event.domain, @tagName(event.event_type), event.timestamp, event.transaction_hash });
    }
    
    fn send_webhook_request(self: *@This(), url: []const u8, json_data: []const u8) !void {
        // Simplified HTTP POST - in production would use proper HTTP client
        _ = self;
        _ = url;
        _ = json_data;
        // TODO: Implement actual HTTP POST request
    }
};

const ProcessorType = enum {
    webhook,      // Send events to HTTP webhook
    quic_stream,  // Broadcast events via QUIC streams
    log_file,     // Write events to log file
};

const ProcessorConfig = struct {
    webhook_url: ?[]const u8 = null,
    log_file_path: ?[]const u8 = null,
    batch_size: u32 = 100,
    batch_timeout_ms: u64 = 1000,
};

// Cache event subscription types
pub const CacheEventType = enum {
    HIT,          // Cache hit
    MISS,         // Cache miss
    EVICTION,     // Cache eviction
    FLUSH,        // Cache flush
};

pub const ZNSCacheEvent = struct {
    event_type: CacheEventType,
    domain: []const u8,
    timestamp: u64,
    hit_count: u64,
    ttl_remaining: u64,
    original_source: zns_types.ResolverSource,
};

pub const CacheSubscriptionManager = struct {
    allocator: std.mem.Allocator,
    cache_subscriptions: std.HashMap([]const u8, *CacheSubscription, std.hash_map.StringContext, std.hash_map.default_max_load_percentage),
    next_subscription_id: u64,
    
    pub fn init(allocator: std.mem.Allocator) @This() {
        return @This(){
            .allocator = allocator,
            .cache_subscriptions = std.HashMap([]const u8, *CacheSubscription, std.hash_map.StringContext, std.hash_map.default_max_load_percentage).init(allocator),
            .next_subscription_id = 1,
        };
    }
    
    pub fn deinit(self: *@This()) void {
        var iterator = self.cache_subscriptions.iterator();
        while (iterator.next()) |entry| {
            self.allocator.free(entry.key_ptr.*);
            entry.value_ptr.*.deinit();
            self.allocator.destroy(entry.value_ptr.*);
        }
        self.cache_subscriptions.deinit();
    }
    
    pub fn create_cache_subscription(self: *@This(), include_hits: bool, include_misses: bool, include_evictions: bool, client_id: []const u8) ![]const u8 {
        const subscription_id = try std.fmt.allocPrint(self.allocator, "cache_sub_{s}_{}", .{ client_id, self.next_subscription_id });
        self.next_subscription_id += 1;
        
        const subscription = try self.allocator.create(CacheSubscription);
        subscription.* = CacheSubscription.init(self.allocator, subscription_id, include_hits, include_misses, include_evictions, client_id);
        
        try self.cache_subscriptions.put(try self.allocator.dupe(u8, subscription_id), subscription);
        
        return subscription_id;
    }
    
    pub fn publish_cache_event(self: *@This(), event: ZNSCacheEvent) !void {
        var iterator = self.cache_subscriptions.iterator();
        while (iterator.next()) |entry| {
            const subscription = entry.value_ptr.*;
            if (subscription.should_receive_cache_event(&event)) {
                try subscription.queue_cache_event(event);
            }
        }
    }
    
    pub fn get_cache_events(self: *@This(), subscription_id: []const u8, max_events: u32) ![]ZNSCacheEvent {
        if (self.cache_subscriptions.get(subscription_id)) |subscription| {
            return try subscription.get_queued_cache_events(max_events);
        }
        return &[_]ZNSCacheEvent{};
    }
};

const CacheSubscription = struct {
    allocator: std.mem.Allocator,
    id: []const u8,
    client_id: []const u8,
    include_hits: bool,
    include_misses: bool,
    include_evictions: bool,
    cache_event_queue: std.ArrayList(ZNSCacheEvent),
    created_at: u64,
    
    fn init(allocator: std.mem.Allocator, id: []const u8, include_hits: bool, include_misses: bool, include_evictions: bool, client_id: []const u8) @This() {
        return @This(){
            .allocator = allocator,
            .id = allocator.dupe(u8, id) catch unreachable,
            .client_id = allocator.dupe(u8, client_id) catch unreachable,
            .include_hits = include_hits,
            .include_misses = include_misses,
            .include_evictions = include_evictions,
            .cache_event_queue = std.ArrayList(ZNSCacheEvent).init(allocator),
            .created_at = @as(u64, @intCast(std.time.timestamp())),
        };
    }
    
    fn deinit(self: *@This()) void {
        self.allocator.free(self.id);
        self.allocator.free(self.client_id);
        self.cache_event_queue.deinit();
    }
    
    fn should_receive_cache_event(self: *const @This(), event: *const ZNSCacheEvent) bool {
        switch (event.event_type) {
            .HIT => return self.include_hits,
            .MISS => return self.include_misses,
            .EVICTION, .FLUSH => return self.include_evictions,
        }
    }
    
    fn queue_cache_event(self: *@This(), event: ZNSCacheEvent) !void {
        try self.cache_event_queue.append(event);
        
        // Limit queue size
        const max_queue_size = 1000;
        if (self.cache_event_queue.items.len > max_queue_size) {
            _ = self.cache_event_queue.orderedRemove(0);
        }
    }
    
    fn get_queued_cache_events(self: *@This(), max_events: u32) ![]ZNSCacheEvent {
        const event_count = std.math.min(self.cache_event_queue.items.len, max_events);
        if (event_count == 0) return &[_]ZNSCacheEvent{};
        
        const events = try self.allocator.alloc(ZNSCacheEvent, event_count);
        @memcpy(events, self.cache_event_queue.items[0..event_count]);
        
        // Remove retrieved events from queue
        self.cache_event_queue.replaceRange(0, event_count, &[_]ZNSCacheEvent{}) catch {};
        
        return events;
    }
};