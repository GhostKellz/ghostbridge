const std = @import("std");
const zns_types = @import("zns_types.zig");

pub const ZNSMetricsCollector = struct {
    allocator: std.mem.Allocator,
    
    // Core resolution metrics
    total_queries: u64,
    successful_queries: u64,
    failed_queries: u64,
    cache_hits: u64,
    cache_misses: u64,
    
    // Performance metrics
    total_resolution_time_ms: u64,
    min_resolution_time_ms: u64,
    max_resolution_time_ms: u64,
    resolution_count: u64,
    
    // Resolver-specific metrics
    native_queries: u64,
    ens_queries: u64,
    unstoppable_queries: u64,
    dns_fallback_queries: u64,
    
    // Error tracking
    timeout_errors: u64,
    rate_limit_errors: u64,
    invalid_domain_errors: u64,
    signature_errors: u64,
    internal_errors: u64,
    
    // TLD statistics
    queries_by_tld: std.HashMap([]const u8, TLDMetrics, std.hash_map.StringContext, std.hash_map.default_max_load_percentage),
    
    // Time-based metrics (moving averages)
    queries_per_second: MovingAverage,
    avg_resolution_time: MovingAverage,
    cache_hit_rate: MovingAverage,
    error_rate: MovingAverage,
    
    // Health monitoring
    uptime_start: u64,
    last_health_check: u64,
    health_status: HealthStatus,
    
    // Resource usage
    memory_usage_bytes: u64,
    cpu_usage_percent: f64,
    open_connections: u64,
    active_subscriptions: u64,
    
    pub fn init(allocator: std.mem.Allocator) @This() {
        return @This(){
            .allocator = allocator,
            .total_queries = 0,
            .successful_queries = 0,
            .failed_queries = 0,
            .cache_hits = 0,
            .cache_misses = 0,
            .total_resolution_time_ms = 0,
            .min_resolution_time_ms = std.math.maxInt(u64),
            .max_resolution_time_ms = 0,
            .resolution_count = 0,
            .native_queries = 0,
            .ens_queries = 0,
            .unstoppable_queries = 0,
            .dns_fallback_queries = 0,
            .timeout_errors = 0,
            .rate_limit_errors = 0,
            .invalid_domain_errors = 0,
            .signature_errors = 0,
            .internal_errors = 0,
            .queries_by_tld = std.HashMap([]const u8, TLDMetrics, std.hash_map.StringContext, std.hash_map.default_max_load_percentage).init(allocator),
            .queries_per_second = MovingAverage.init(60), // 60-second window
            .avg_resolution_time = MovingAverage.init(100), // 100-query window
            .cache_hit_rate = MovingAverage.init(100),
            .error_rate = MovingAverage.init(100),
            .uptime_start = @as(u64, @intCast(std.time.timestamp())),
            .last_health_check = @as(u64, @intCast(std.time.timestamp())),
            .health_status = .healthy,
            .memory_usage_bytes = 0,
            .cpu_usage_percent = 0.0,
            .open_connections = 0,
            .active_subscriptions = 0,
        };
    }
    
    pub fn deinit(self: *@This()) void {
        var iterator = self.queries_by_tld.iterator();
        while (iterator.next()) |entry| {
            self.allocator.free(entry.key_ptr.*);
        }
        self.queries_by_tld.deinit();
    }
    
    pub fn record_query(self: *@This(), domain: []const u8, source: zns_types.ResolverSource, resolution_time_ms: u64, was_cache_hit: bool, success: bool) void {
        const now = @as(u64, @intCast(std.time.timestamp()));
        
        // Update core metrics
        self.total_queries += 1;
        if (success) {
            self.successful_queries += 1;
        } else {
            self.failed_queries += 1;
        }
        
        if (was_cache_hit) {
            self.cache_hits += 1;
        } else {
            self.cache_misses += 1;
        }
        
        // Update performance metrics
        if (success) {
            self.total_resolution_time_ms += resolution_time_ms;
            self.resolution_count += 1;
            
            if (resolution_time_ms < self.min_resolution_time_ms) {
                self.min_resolution_time_ms = resolution_time_ms;
            }
            if (resolution_time_ms > self.max_resolution_time_ms) {
                self.max_resolution_time_ms = resolution_time_ms;
            }
        }
        
        // Update resolver-specific metrics
        switch (source) {
            .ZNS_NATIVE => self.native_queries += 1,
            .ENS_BRIDGE => self.ens_queries += 1,
            .UNSTOPPABLE_BRIDGE => self.unstoppable_queries += 1,
            .TRADITIONAL_DNS => self.dns_fallback_queries += 1,
            .CACHE => {}, // Already counted in cache_hits
        }
        
        // Update TLD metrics
        if (self.get_tld(domain)) |tld| {
            var tld_metrics = self.queries_by_tld.getPtr(tld) orelse blk: {
                const tld_key = self.allocator.dupe(u8, tld) catch return;
                const new_metrics = TLDMetrics{
                    .total_queries = 0,
                    .successful_queries = 0,
                    .cache_hits = 0,
                    .avg_resolution_time_ms = 0,
                    .last_query_time = now,
                };
                self.queries_by_tld.put(tld_key, new_metrics) catch return;
                break :blk self.queries_by_tld.getPtr(tld_key).?;
            };
            
            tld_metrics.total_queries += 1;
            if (success) tld_metrics.successful_queries += 1;
            if (was_cache_hit) tld_metrics.cache_hits += 1;
            if (success) {
                // Update moving average for resolution time
                const alpha = 0.1;
                tld_metrics.avg_resolution_time_ms = alpha * @as(f64, @floatFromInt(resolution_time_ms)) + 
                                                    (1.0 - alpha) * tld_metrics.avg_resolution_time_ms;
            }
            tld_metrics.last_query_time = now;
        }
        
        // Update moving averages
        self.queries_per_second.add(1.0);
        if (success) {
            self.avg_resolution_time.add(@as(f64, @floatFromInt(resolution_time_ms)));
        }
        self.cache_hit_rate.add(if (was_cache_hit) 1.0 else 0.0);
        self.error_rate.add(if (success) 0.0 else 1.0);
    }
    
    pub fn record_error(self: *@This(), error_code: zns_types.ZNSErrorCode) void {
        switch (error_code) {
            .TIMEOUT => self.timeout_errors += 1,
            .RATE_LIMITED => self.rate_limit_errors += 1,
            .INVALID_DOMAIN => self.invalid_domain_errors += 1,
            .SIGNATURE_INVALID => self.signature_errors += 1,
            .INTERNAL_ERROR => self.internal_errors += 1,
            else => {},
        }
    }
    
    pub fn update_resource_usage(self: *@This(), memory_bytes: u64, cpu_percent: f64, connections: u64, subscriptions: u64) void {
        self.memory_usage_bytes = memory_bytes;
        self.cpu_usage_percent = cpu_percent;
        self.open_connections = connections;
        self.active_subscriptions = subscriptions;
    }
    
    pub fn perform_health_check(self: *@This()) HealthStatus {
        const now = @as(u64, @intCast(std.time.timestamp()));
        
        // Check error rate (>10% is unhealthy)
        const current_error_rate = self.error_rate.get_average();
        if (current_error_rate > 0.1) {
            self.health_status = .degraded;
        }
        
        // Check memory usage (>90% is unhealthy)
        const memory_usage_percent = @as(f64, @floatFromInt(self.memory_usage_bytes)) / (1024.0 * 1024.0 * 1024.0); // Assume 1GB limit
        if (memory_usage_percent > 0.9) {
            self.health_status = .unhealthy;
        }
        
        // Check CPU usage (>80% is unhealthy)
        if (self.cpu_usage_percent > 80.0) {
            self.health_status = .degraded;
        }
        
        // Check response time (>5000ms average is unhealthy)
        const avg_response_time = self.avg_resolution_time.get_average();
        if (avg_response_time > 5000.0) {
            self.health_status = .degraded;
        }
        
        // If all checks pass, we're healthy
        if (current_error_rate <= 0.1 and memory_usage_percent <= 0.9 and 
            self.cpu_usage_percent <= 80.0 and avg_response_time <= 5000.0) {
            self.health_status = .healthy;
        }
        
        self.last_health_check = now;
        return self.health_status;
    }
    
    pub fn get_summary_report(self: *const @This()) MetricsSummary {
        const uptime = @as(u64, @intCast(std.time.timestamp())) - self.uptime_start;
        const avg_resolution_time = if (self.resolution_count > 0)
            @as(f64, @floatFromInt(self.total_resolution_time_ms)) / @as(f64, @floatFromInt(self.resolution_count))
        else 0.0;
        
        const cache_hit_rate = if (self.total_queries > 0)
            @as(f64, @floatFromInt(self.cache_hits)) / @as(f64, @floatFromInt(self.total_queries))
        else 0.0;
        
        const success_rate = if (self.total_queries > 0)
            @as(f64, @floatFromInt(self.successful_queries)) / @as(f64, @floatFromInt(self.total_queries))
        else 0.0;
        
        const queries_per_second = if (uptime > 0)
            @as(f64, @floatFromInt(self.total_queries)) / @as(f64, @floatFromInt(uptime))
        else 0.0;
        
        return MetricsSummary{
            .uptime_seconds = uptime,
            .total_queries = self.total_queries,
            .successful_queries = self.successful_queries,
            .failed_queries = self.failed_queries,
            .cache_hit_rate = cache_hit_rate,
            .success_rate = success_rate,
            .avg_resolution_time_ms = avg_resolution_time,
            .min_resolution_time_ms = self.min_resolution_time_ms,
            .max_resolution_time_ms = self.max_resolution_time_ms,
            .queries_per_second = queries_per_second,
            .health_status = self.health_status,
            .memory_usage_bytes = self.memory_usage_bytes,
            .cpu_usage_percent = self.cpu_usage_percent,
            .open_connections = self.open_connections,
            .active_subscriptions = self.active_subscriptions,
            .resolver_breakdown = ResolverBreakdown{
                .native_queries = self.native_queries,
                .ens_queries = self.ens_queries,
                .unstoppable_queries = self.unstoppable_queries,
                .dns_fallback_queries = self.dns_fallback_queries,
            },
            .error_breakdown = ErrorBreakdown{
                .timeout_errors = self.timeout_errors,
                .rate_limit_errors = self.rate_limit_errors,
                .invalid_domain_errors = self.invalid_domain_errors,
                .signature_errors = self.signature_errors,
                .internal_errors = self.internal_errors,
            },
        };
    }
    
    pub fn get_tld_metrics(self: *const @This()) []TLDMetricsReport {
        var reports = std.ArrayList(TLDMetricsReport).init(self.allocator);
        
        var iterator = self.queries_by_tld.iterator();
        while (iterator.next()) |entry| {
            const tld = entry.key_ptr.*;
            const metrics = entry.value_ptr.*;
            
            const success_rate = if (metrics.total_queries > 0)
                @as(f64, @floatFromInt(metrics.successful_queries)) / @as(f64, @floatFromInt(metrics.total_queries))
            else 0.0;
            
            const cache_hit_rate = if (metrics.total_queries > 0)
                @as(f64, @floatFromInt(metrics.cache_hits)) / @as(f64, @floatFromInt(metrics.total_queries))
            else 0.0;
            
            const report = TLDMetricsReport{
                .tld = tld,
                .total_queries = metrics.total_queries,
                .successful_queries = metrics.successful_queries,
                .cache_hits = metrics.cache_hits,
                .success_rate = success_rate,
                .cache_hit_rate = cache_hit_rate,
                .avg_resolution_time_ms = metrics.avg_resolution_time_ms,
                .last_query_time = metrics.last_query_time,
            };
            
            reports.append(report) catch continue;
        }
        
        return reports.toOwnedSlice() catch &[_]TLDMetricsReport{};
    }
    
    pub fn export_prometheus_metrics(self: *const @This(), allocator: std.mem.Allocator) ![]u8 {
        var buffer = std.ArrayList(u8).init(allocator);
        defer buffer.deinit();
        
        const writer = buffer.writer();
        
        // Counter metrics
        try writer.print("# HELP zns_queries_total Total number of DNS queries\n");
        try writer.print("# TYPE zns_queries_total counter\n");
        try writer.print("zns_queries_total {}\n", .{self.total_queries});
        
        try writer.print("# HELP zns_queries_successful_total Successful DNS queries\n");
        try writer.print("# TYPE zns_queries_successful_total counter\n");
        try writer.print("zns_queries_successful_total {}\n", .{self.successful_queries});
        
        try writer.print("# HELP zns_cache_hits_total Cache hits\n");
        try writer.print("# TYPE zns_cache_hits_total counter\n");
        try writer.print("zns_cache_hits_total {}\n", .{self.cache_hits});
        
        // Gauge metrics
        try writer.print("# HELP zns_resolution_time_ms Resolution time in milliseconds\n");
        try writer.print("# TYPE zns_resolution_time_ms gauge\n");
        try writer.print("zns_resolution_time_ms {d}\n", .{self.avg_resolution_time.get_average()});
        
        try writer.print("# HELP zns_cache_hit_rate Cache hit rate\n");
        try writer.print("# TYPE zns_cache_hit_rate gauge\n");
        try writer.print("zns_cache_hit_rate {d}\n", .{self.cache_hit_rate.get_average()});
        
        try writer.print("# HELP zns_memory_usage_bytes Memory usage in bytes\n");
        try writer.print("# TYPE zns_memory_usage_bytes gauge\n");
        try writer.print("zns_memory_usage_bytes {}\n", .{self.memory_usage_bytes});
        
        try writer.print("# HELP zns_cpu_usage_percent CPU usage percentage\n");
        try writer.print("# TYPE zns_cpu_usage_percent gauge\n");
        try writer.print("zns_cpu_usage_percent {d}\n", .{self.cpu_usage_percent});
        
        // Health status
        try writer.print("# HELP zns_health_status Health status (0=unhealthy, 1=degraded, 2=healthy)\n");
        try writer.print("# TYPE zns_health_status gauge\n");
        try writer.print("zns_health_status {}\n", .{@intFromEnum(self.health_status)});
        
        return buffer.toOwnedSlice();
    }
    
    fn get_tld(_: *@This(), domain: []const u8) ?[]const u8 {
        if (std.mem.lastIndexOf(u8, domain, ".")) |index| {
            return domain[index..];
        }
        return null;
    }
};

const TLDMetrics = struct {
    total_queries: u64,
    successful_queries: u64,
    cache_hits: u64,
    avg_resolution_time_ms: f64,
    last_query_time: u64,
};

const TLDMetricsReport = struct {
    tld: []const u8,
    total_queries: u64,
    successful_queries: u64,
    cache_hits: u64,
    success_rate: f64,
    cache_hit_rate: f64,
    avg_resolution_time_ms: f64,
    last_query_time: u64,
};

const MovingAverage = struct {
    values: []f64,
    current_index: usize,
    count: usize,
    window_size: usize,
    
    fn init(window_size: usize) @This() {
        const values = std.heap.page_allocator.alloc(f64, window_size) catch unreachable;
        @memset(values, 0.0);
        
        return @This(){
            .values = values,
            .current_index = 0,
            .count = 0,
            .window_size = window_size,
        };
    }
    
    fn add(self: *@This(), value: f64) void {
        self.values[self.current_index] = value;
        self.current_index = (self.current_index + 1) % self.window_size;
        if (self.count < self.window_size) {
            self.count += 1;
        }
    }
    
    fn get_average(self: *const @This()) f64 {
        if (self.count == 0) return 0.0;
        
        var sum: f64 = 0.0;
        for (self.values[0..self.count]) |value| {
            sum += value;
        }
        
        return sum / @as(f64, @floatFromInt(self.count));
    }
};

const HealthStatus = enum(u8) {
    unhealthy = 0,
    degraded = 1,
    healthy = 2,
};

const MetricsSummary = struct {
    uptime_seconds: u64,
    total_queries: u64,
    successful_queries: u64,
    failed_queries: u64,
    cache_hit_rate: f64,
    success_rate: f64,
    avg_resolution_time_ms: f64,
    min_resolution_time_ms: u64,
    max_resolution_time_ms: u64,
    queries_per_second: f64,
    health_status: HealthStatus,
    memory_usage_bytes: u64,
    cpu_usage_percent: f64,
    open_connections: u64,
    active_subscriptions: u64,
    resolver_breakdown: ResolverBreakdown,
    error_breakdown: ErrorBreakdown,
};

const ResolverBreakdown = struct {
    native_queries: u64,
    ens_queries: u64,
    unstoppable_queries: u64,
    dns_fallback_queries: u64,
};

const ErrorBreakdown = struct {
    timeout_errors: u64,
    rate_limit_errors: u64,
    invalid_domain_errors: u64,
    signature_errors: u64,
    internal_errors: u64,
};

// Alert system for monitoring
pub const AlertManager = struct {
    allocator: std.mem.Allocator,
    alert_rules: std.ArrayList(AlertRule),
    active_alerts: std.ArrayList(Alert),
    notification_channels: std.ArrayList(NotificationChannel),
    
    pub fn init(allocator: std.mem.Allocator) @This() {
        return @This(){
            .allocator = allocator,
            .alert_rules = std.ArrayList(AlertRule).init(allocator),
            .active_alerts = std.ArrayList(Alert).init(allocator),
            .notification_channels = std.ArrayList(NotificationChannel).init(allocator),
        };
    }
    
    pub fn deinit(self: *@This()) void {
        self.alert_rules.deinit();
        self.active_alerts.deinit();
        self.notification_channels.deinit();
    }
    
    pub fn add_alert_rule(self: *@This(), rule: AlertRule) !void {
        try self.alert_rules.append(rule);
    }
    
    pub fn evaluate_alerts(self: *@This(), metrics: *const ZNSMetricsCollector) !void {
        const summary = metrics.get_summary_report();
        
        for (self.alert_rules.items) |rule| {
            const triggered = switch (rule.condition) {
                .error_rate_above => |threshold| summary.success_rate < (1.0 - threshold),
                .response_time_above => |threshold| summary.avg_resolution_time_ms > threshold,
                .cache_hit_rate_below => |threshold| summary.cache_hit_rate < threshold,
                .memory_usage_above => |threshold| summary.memory_usage_bytes > threshold,
                .health_degraded => summary.health_status != .healthy,
            };
            
            if (triggered) {
                try self.fire_alert(rule, summary);
            } else {
                self.resolve_alert(rule.name);
            }
        }
    }
    
    fn fire_alert(self: *@This(), rule: AlertRule, _: MetricsSummary) !void {
        // Check if alert is already active
        for (self.active_alerts.items) |alert| {
            if (std.mem.eql(u8, alert.rule_name, rule.name)) {
                return; // Alert already active
            }
        }
        
        const alert = Alert{
            .rule_name = rule.name,
            .severity = rule.severity,
            .message = rule.message,
            .fired_at = @as(u64, @intCast(std.time.timestamp())),
            .resolved_at = null,
        };
        
        try self.active_alerts.append(alert);
        
        // Send notifications
        for (self.notification_channels.items) |channel| {
            self.send_notification(channel, alert) catch {};
        }
    }
    
    fn resolve_alert(self: *@This(), rule_name: []const u8) void {
        for (self.active_alerts.items, 0..) |*alert, i| {
            if (std.mem.eql(u8, alert.rule_name, rule_name)) {
                alert.resolved_at = @as(u64, @intCast(std.time.timestamp()));
                _ = self.active_alerts.swapRemove(i);
                break;
            }
        }
    }
    
    fn send_notification(self: *@This(), channel: NotificationChannel, alert: Alert) !void {
        switch (channel.type) {
            .webhook => {
                // Send HTTP webhook notification
                _ = self;
                _ = alert;
            },
            .email => {
                // Send email notification
                _ = self;
                _ = alert;
            },
            .slack => {
                // Send Slack notification
                _ = self;
                _ = alert;
            },
        }
    }
};

const AlertRule = struct {
    name: []const u8,
    condition: AlertCondition,
    severity: AlertSeverity,
    message: []const u8,
};

const AlertCondition = union(enum) {
    error_rate_above: f64,
    response_time_above: f64,
    cache_hit_rate_below: f64,
    memory_usage_above: u64,
    health_degraded,
};

const AlertSeverity = enum {
    info,
    warning,
    critical,
};

const Alert = struct {
    rule_name: []const u8,
    severity: AlertSeverity,
    message: []const u8,
    fired_at: u64,
    resolved_at: ?u64,
};

const NotificationChannel = struct {
    type: NotificationType,
    config: NotificationConfig,
};

const NotificationType = enum {
    webhook,
    email,
    slack,
};

const NotificationConfig = union(enum) {
    webhook: struct {
        url: []const u8,
    },
    email: struct {
        smtp_server: []const u8,
        recipients: [][]const u8,
    },
    slack: struct {
        webhook_url: []const u8,
        channel: []const u8,
    },
};