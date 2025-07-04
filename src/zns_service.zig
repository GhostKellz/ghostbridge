const std = @import("std");
const zns_types = @import("zns_types.zig");
const zns_resolver = @import("zns_resolver.zig");
const zns_cache = @import("zns_cache.zig");
const zns_subscription = @import("zns_subscription.zig");
const zns_metrics = @import("zns_metrics.zig");
const zns_validator = @import("zns_validator.zig");

pub const ZNSService = struct {
    allocator: std.mem.Allocator,
    resolver: *zns_resolver.ZNSResolver,
    subscription_manager: *zns_subscription.ZNSSubscriptionManager,
    cache_subscription_manager: *zns_subscription.CacheSubscriptionManager,
    metrics_collector: *zns_metrics.ZNSMetricsCollector,
    alert_manager: *zns_metrics.AlertManager,
    config: ZNSServiceConfig,
    
    pub fn init(allocator: std.mem.Allocator, config: ZNSServiceConfig) !@This() {
        const resolver = try allocator.create(zns_resolver.ZNSResolver);
        resolver.* = try zns_resolver.ZNSResolver.init(allocator, config.resolver_config);
        
        const subscription_manager = try allocator.create(zns_subscription.ZNSSubscriptionManager);
        subscription_manager.* = zns_subscription.ZNSSubscriptionManager.init(allocator);
        
        const cache_subscription_manager = try allocator.create(zns_subscription.CacheSubscriptionManager);
        cache_subscription_manager.* = zns_subscription.CacheSubscriptionManager.init(allocator);
        
        const metrics_collector = try allocator.create(zns_metrics.ZNSMetricsCollector);
        metrics_collector.* = zns_metrics.ZNSMetricsCollector.init(allocator);
        
        const alert_manager = try allocator.create(zns_metrics.AlertManager);
        alert_manager.* = zns_metrics.AlertManager.init(allocator);
        
        // Setup default alert rules
        try alert_manager.add_alert_rule(.{
            .name = "High Error Rate",
            .condition = .{ .error_rate_above = 0.1 }, // >10% error rate
            .severity = .critical,
            .message = "ZNS error rate is above 10%",
        });
        
        try alert_manager.add_alert_rule(.{
            .name = "Slow Response Time",
            .condition = .{ .response_time_above = 5000.0 }, // >5s response time
            .severity = .warning,
            .message = "ZNS response time is above 5 seconds",
        });
        
        try alert_manager.add_alert_rule(.{
            .name = "Low Cache Hit Rate",
            .condition = .{ .cache_hit_rate_below = 0.5 }, // <50% cache hit rate
            .severity = .warning,
            .message = "ZNS cache hit rate is below 50%",
        });
        
        return @This(){
            .allocator = allocator,
            .resolver = resolver,
            .subscription_manager = subscription_manager,
            .cache_subscription_manager = cache_subscription_manager,
            .metrics_collector = metrics_collector,
            .alert_manager = alert_manager,
            .config = config,
        };
    }
    
    pub fn deinit(self: *@This()) void {
        self.resolver.deinit();
        self.allocator.destroy(self.resolver);
        
        self.subscription_manager.deinit();
        self.allocator.destroy(self.subscription_manager);
        
        self.cache_subscription_manager.deinit();
        self.allocator.destroy(self.cache_subscription_manager);
        
        self.metrics_collector.deinit();
        self.allocator.destroy(self.metrics_collector);
        
        self.alert_manager.deinit();
        self.allocator.destroy(self.alert_manager);
    }
    
    // Core ZNS operations
    pub fn resolve_domain(self: *@This(), request: zns_types.ZNSResolveRequest, client_id: []const u8) !zns_types.ZNSResolveResponse {
        const start_time = std.time.milliTimestamp();
        
        const response = try self.resolver.resolve_domain(request, client_id);
        
        const end_time = std.time.milliTimestamp();
        const resolution_time = @as(u64, @intCast(end_time - start_time));
        
        // Record metrics
        self.metrics_collector.record_query(
            request.domain,
            response.resolution_info.source,
            resolution_time,
            response.resolution_info.was_cached,
            response.zns_error == null,
        );
        
        if (response.zns_error) |error_info| {
            self.metrics_collector.record_error(error_info.code);
        }
        
        return response;
    }
    
    pub fn register_domain(self: *@This(), request: zns_types.ZNSRegisterRequest, client_id: []const u8) !zns_types.ZNSRegisterResponse {
        const response = try self.resolver.register_domain(request, client_id);
        
        // If registration was successful, publish domain change event
        if (response.success) {
            const change_event = zns_types.ZNSDomainChangeEvent{
                .domain = request.domain,
                .event_type = .DOMAIN_REGISTERED,
                .old_records = &[_]zns_types.DnsRecord{},
                .new_records = request.initial_records,
                .timestamp = @as(u64, @intCast(std.time.timestamp())),
                .transaction_hash = response.transaction_hash,
            };
            
            try self.subscription_manager.publish_domain_change(change_event);
        }
        
        return response;
    }
    
    pub fn update_domain(self: *@This(), request: zns_types.ZNSUpdateRequest, client_id: []const u8) !zns_types.ZNSUpdateResponse {
        const response = try self.resolver.update_domain(request, client_id);
        
        // If update was successful, publish domain change event
        if (response.success) {
            const change_event = zns_types.ZNSDomainChangeEvent{
                .domain = request.domain,
                .event_type = .DOMAIN_UPDATED,
                .old_records = &[_]zns_types.DnsRecord{}, // Would need to fetch old records
                .new_records = response.updated_records,
                .timestamp = @as(u64, @intCast(std.time.timestamp())),
                .transaction_hash = response.transaction_hash,
            };
            
            try self.subscription_manager.publish_domain_change(change_event);
        }
        
        return response;
    }
    
    // Subscription management
    pub fn create_domain_subscription(self: *@This(), request: zns_types.ZNSDomainSubscription, client_id: []const u8) ![]const u8 {
        return try self.subscription_manager.create_subscription(request, client_id);
    }
    
    pub fn cancel_domain_subscription(self: *@This(), subscription_id: []const u8) bool {
        return self.subscription_manager.cancel_subscription(subscription_id);
    }
    
    pub fn get_subscription_events(self: *@This(), subscription_id: []const u8, max_events: u32) ![]zns_types.ZNSDomainChangeEvent {
        return try self.subscription_manager.get_subscription_events(subscription_id, max_events);
    }
    
    pub fn create_cache_subscription(self: *@This(), include_hits: bool, include_misses: bool, include_evictions: bool, client_id: []const u8) ![]const u8 {
        return try self.cache_subscription_manager.create_cache_subscription(include_hits, include_misses, include_evictions, client_id);
    }
    
    pub fn get_cache_events(self: *@This(), subscription_id: []const u8, max_events: u32) ![]zns_subscription.ZNSCacheEvent {
        return try self.cache_subscription_manager.get_cache_events(subscription_id, max_events);
    }
    
    // Health and status
    pub fn get_health_status(self: *@This()) zns_metrics.HealthStatus {
        return self.metrics_collector.perform_health_check();
    }
    
    pub fn get_status_report(self: *const @This()) ZNSStatusReport {
        const metrics_summary = self.metrics_collector.get_summary_report();
        
        return ZNSStatusReport{
            .healthy = metrics_summary.health_status == .healthy,
            .version = "ZNS-Service-1.0",
            .uptime_seconds = metrics_summary.uptime_seconds,
            .cache_statistics = self.resolver.get_statistics(),
            .resolution_metrics = metrics_summary,
            .subscription_count = self.subscription_manager.get_subscription_count(),
            .active_domain_count = self.subscription_manager.get_active_domain_count(),
        };
    }
    
    pub fn get_metrics_report(self: *const @This()) zns_metrics.MetricsSummary {
        return self.metrics_collector.get_summary_report();
    }
    
    pub fn export_prometheus_metrics(self: *const @This()) ![]u8 {
        return try self.metrics_collector.export_prometheus_metrics(self.allocator);
    }
    
    // Administrative operations
    pub fn flush_cache(self: *@This()) void {
        self.resolver.cache.clear();
        
        // Publish cache flush event
        const cache_event = zns_subscription.ZNSCacheEvent{
            .event_type = .FLUSH,
            .domain = "all",
            .timestamp = @as(u64, @intCast(std.time.timestamp())),
            .hit_count = 0,
            .ttl_remaining = 0,
            .original_source = .ZNS_NATIVE,
        };
        
        self.cache_subscription_manager.publish_cache_event(cache_event) catch {};
    }
    
    pub fn update_configuration(self: *@This(), new_config: ZNSServiceConfig) void {
        self.config = new_config;
        // TODO: Apply configuration changes to components
    }
    
    // Background tasks
    pub fn run_periodic_tasks(self: *@This()) !void {
        // Clean up expired cache entries
        _ = try self.resolver.cache.cleanup_expired_entries();
        
        // Evaluate alert conditions
        try self.alert_manager.evaluate_alerts(self.metrics_collector);
        
        // Update resource usage metrics (simplified)
        const memory_usage = self.estimate_memory_usage();
        self.metrics_collector.update_resource_usage(memory_usage, 0.0, 0, self.subscription_manager.get_subscription_count());
    }
    
    // Cache event publishing
    pub fn publish_cache_hit(self: *@This(), domain: []const u8, hit_count: u64, ttl_remaining: u64, source: zns_types.ResolverSource) !void {
        const cache_event = zns_subscription.ZNSCacheEvent{
            .event_type = .HIT,
            .domain = domain,
            .timestamp = @as(u64, @intCast(std.time.timestamp())),
            .hit_count = hit_count,
            .ttl_remaining = ttl_remaining,
            .original_source = source,
        };
        
        try self.cache_subscription_manager.publish_cache_event(cache_event);
    }
    
    pub fn publish_cache_miss(self: *@This(), domain: []const u8) !void {
        const cache_event = zns_subscription.ZNSCacheEvent{
            .event_type = .MISS,
            .domain = domain,
            .timestamp = @as(u64, @intCast(std.time.timestamp())),
            .hit_count = 0,
            .ttl_remaining = 0,
            .original_source = .ZNS_NATIVE,
        };
        
        try self.cache_subscription_manager.publish_cache_event(cache_event);
    }
    
    pub fn publish_cache_eviction(self: *@This(), domain: []const u8, hit_count: u64, source: zns_types.ResolverSource) !void {
        const cache_event = zns_subscription.ZNSCacheEvent{
            .event_type = .EVICTION,
            .domain = domain,
            .timestamp = @as(u64, @intCast(std.time.timestamp())),
            .hit_count = hit_count,
            .ttl_remaining = 0,
            .original_source = source,
        };
        
        try self.cache_subscription_manager.publish_cache_event(cache_event);
    }
    
    // Private helper methods
    fn estimate_memory_usage(self: *const @This()) u64 {
        // Simplified memory usage estimation
        const cache_stats = self.resolver.get_statistics();
        return cache_stats.memory_usage_bytes + 
               (self.subscription_manager.get_subscription_count() * 1024) + // Estimate 1KB per subscription
               (1024 * 1024); // Base service overhead
    }
};

pub const ZNSServiceConfig = struct {
    resolver_config: zns_resolver.ZNSResolverConfig = .{},
    enable_subscriptions: bool = true,
    enable_cache_events: bool = true,
    enable_metrics: bool = true,
    enable_alerts: bool = true,
    periodic_task_interval_ms: u64 = 60000, // 1 minute
    max_concurrent_requests: u32 = 1000,
    request_timeout_ms: u64 = 10000, // 10 seconds
};

const ZNSStatusReport = struct {
    healthy: bool,
    version: []const u8,
    uptime_seconds: u64,
    cache_statistics: zns_types.CacheStatistics,
    resolution_metrics: zns_metrics.MetricsSummary,
    subscription_count: usize,
    active_domain_count: usize,
};

// QUIC handler functions for integration with existing multiplexer
pub fn handle_zns_resolve(allocator: std.mem.Allocator, service: *ZNSService, request_data: []const u8, client_id: []const u8) ![]u8 {
    // Parse request from JSON or binary format
    const request = try parse_resolve_request(allocator, request_data);
    defer free_resolve_request(allocator, request);
    
    const response = try service.resolve_domain(request, client_id);
    
    // Serialize response to JSON or binary format
    return try serialize_resolve_response(allocator, response);
}

pub fn handle_zns_register(allocator: std.mem.Allocator, service: *ZNSService, request_data: []const u8, client_id: []const u8) ![]u8 {
    const request = try parse_register_request(allocator, request_data);
    defer free_register_request(allocator, request);
    
    const response = try service.register_domain(request, client_id);
    
    return try serialize_register_response(allocator, response);
}

pub fn handle_zns_update(allocator: std.mem.Allocator, service: *ZNSService, request_data: []const u8, client_id: []const u8) ![]u8 {
    const request = try parse_update_request(allocator, request_data);
    defer free_update_request(allocator, request);
    
    const response = try service.update_domain(request, client_id);
    
    return try serialize_update_response(allocator, response);
}

pub fn handle_zns_subscribe(allocator: std.mem.Allocator, service: *ZNSService, request_data: []const u8, client_id: []const u8) ![]u8 {
    const request = try parse_subscription_request(allocator, request_data);
    defer free_subscription_request(allocator, request);
    
    const subscription_id = try service.create_domain_subscription(request, client_id);
    
    return try serialize_subscription_response(allocator, subscription_id);
}

pub fn handle_zns_status(allocator: std.mem.Allocator, service: *ZNSService) ![]u8 {
    const status = service.get_status_report();
    return try serialize_status_report(allocator, status);
}

pub fn handle_zns_metrics(allocator: std.mem.Allocator, service: *ZNSService) ![]u8 {
    const metrics = service.get_metrics_report();
    return try serialize_metrics_report(allocator, metrics);
}

// Simplified JSON parsing/serialization functions (in production would use proper JSON library)
fn parse_resolve_request(allocator: std.mem.Allocator, data: []const u8) !zns_types.ZNSResolveRequest {
    _ = allocator;
    _ = data;
    // TODO: Implement proper JSON parsing
    return zns_types.ZNSResolveRequest{
        .domain = "example.ghost",
        .record_types = @as([][]const u8, @constCast(&[_][]const u8{"A"})),
        .include_metadata = true,
        .use_cache = true,
        .max_ttl = 3600,
    };
}

fn free_resolve_request(allocator: std.mem.Allocator, request: zns_types.ZNSResolveRequest) void {
    _ = allocator;
    _ = request;
    // TODO: Free allocated memory
}

fn serialize_resolve_response(allocator: std.mem.Allocator, response: zns_types.ZNSResolveResponse) ![]u8 {
    // TODO: Implement proper JSON serialization
    return try std.fmt.allocPrint(allocator, 
        \\{{"domain": "{s}", "records": [], "error": null}}
    , .{response.domain});
}

fn parse_register_request(allocator: std.mem.Allocator, data: []const u8) !zns_types.ZNSRegisterRequest {
    _ = allocator;
    _ = data;
    return zns_types.ZNSRegisterRequest{
        .domain = "",
        .owner_address = "",
        .initial_records = &[_]zns_types.DnsRecord{},
        .metadata = zns_types.DomainMetadata{
            .version = 1,
            .registrar = "ZNS",
            .tags = null,
            .description = null,
            .avatar = null,
            .website = null,
            .social = null,
        },
        .expiry_timestamp = 0,
        .signature = &[_]u8{},
    };
}

fn free_register_request(allocator: std.mem.Allocator, request: zns_types.ZNSRegisterRequest) void {
    _ = allocator;
    _ = request;
}

fn serialize_register_response(allocator: std.mem.Allocator, response: zns_types.ZNSRegisterResponse) ![]u8 {
    return try std.fmt.allocPrint(allocator, 
        \\{{"success": {}, "domain": "{s}", "transaction_hash": "{s}"}}
    , .{ response.success, response.domain, response.transaction_hash });
}

fn parse_update_request(allocator: std.mem.Allocator, data: []const u8) !zns_types.ZNSUpdateRequest {
    _ = allocator;
    _ = data;
    return zns_types.ZNSUpdateRequest{
        .domain = "",
        .records = &[_]zns_types.DnsRecord{},
        .action = .UPDATE,
        .owner_signature = &[_]u8{},
        .transaction_id = "",
    };
}

fn free_update_request(allocator: std.mem.Allocator, request: zns_types.ZNSUpdateRequest) void {
    _ = allocator;
    _ = request;
}

fn serialize_update_response(allocator: std.mem.Allocator, response: zns_types.ZNSUpdateResponse) ![]u8 {
    return try std.fmt.allocPrint(allocator, 
        \\{{"success": {}, "transaction_hash": "{s}"}}
    , .{ response.success, response.transaction_hash });
}

fn parse_subscription_request(allocator: std.mem.Allocator, data: []const u8) !zns_types.ZNSDomainSubscription {
    _ = allocator;
    _ = data;
    return zns_types.ZNSDomainSubscription{
        .domains = &[_][]const u8{},
        .record_types = &[_]zns_types.DnsRecordType{},
        .include_metadata = true,
    };
}

fn free_subscription_request(allocator: std.mem.Allocator, request: zns_types.ZNSDomainSubscription) void {
    _ = allocator;
    _ = request;
}

fn serialize_subscription_response(allocator: std.mem.Allocator, subscription_id: []const u8) ![]u8 {
    return try std.fmt.allocPrint(allocator, 
        \\{{"subscription_id": "{s}"}}
    , .{subscription_id});
}

fn serialize_status_report(allocator: std.mem.Allocator, status: ZNSStatusReport) ![]u8 {
    return try std.fmt.allocPrint(allocator, 
        \\{{"healthy": {}, "version": "{s}", "uptime_seconds": {}}}
    , .{ status.healthy, status.version, status.uptime_seconds });
}

fn serialize_metrics_report(allocator: std.mem.Allocator, metrics: zns_metrics.MetricsSummary) ![]u8 {
    return try std.fmt.allocPrint(allocator, 
        \\{{"total_queries": {}, "success_rate": {d}, "cache_hit_rate": {d}}}
    , .{ metrics.total_queries, metrics.success_rate, metrics.cache_hit_rate });
}