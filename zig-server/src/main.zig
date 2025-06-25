const std = @import("std");
const net = std.net;
const io = std.io;
const http2 = @import("http2.zig");
const grpc = @import("grpc.zig");
const protobuf = @import("protobuf.zig");

pub fn main() !void {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    defer _ = gpa.deinit();
    const allocator = gpa.allocator();

    const args = try std.process.argsAlloc(allocator);
    defer std.process.argsFree(allocator, args);

    var bind_addr: []const u8 = "127.0.0.1:9090";
    var enable_quic = true;
    var enable_http2 = true;

    for (args[1..]) |arg| {
        if (std.mem.eql(u8, arg, "--no-quic")) {
            enable_quic = false;
        } else if (std.mem.eql(u8, arg, "--no-http2")) {
            enable_http2 = false;
        } else if (std.mem.startsWith(u8, arg, "--bind=")) {
            bind_addr = arg[7..];
        }
    }

    var server = try GhostBridgeServer.init(allocator, .{
        .bind_address = bind_addr,
        .enable_quic = enable_quic,
        .enable_http2 = enable_http2,
    });
    defer server.deinit();

    std.log.info("GhostBridge server starting on {s}", .{bind_addr});
    std.log.info("Protocols: HTTP/2={} HTTP/3(QUIC)={}", .{ enable_http2, enable_quic });

    try server.run();
}

const ServerOptions = struct {
    bind_address: []const u8,
    enable_quic: bool = true,
    enable_http2: bool = true,
    max_connections: u32 = 10000,
    connection_timeout_ms: u32 = 30000,
};

pub const GhostBridgeServer = struct {
    allocator: std.mem.Allocator,
    http2_server: ?*http2.Server,
    quic_server: ?*QuicServer,
    grpc_handler: grpc.Handler,
    stats: ServerStats,
    cache: ResponseCache,

    const Self = @This();

    pub fn init(allocator: std.mem.Allocator, options: ServerOptions) !Self {
        var self = Self{
            .allocator = allocator,
            .http2_server = null,
            .quic_server = null,
            .grpc_handler = try grpc.Handler.init(allocator),
            .stats = ServerStats{},
            .cache = try ResponseCache.init(allocator, 1024 * 1024 * 100), // 100MB cache
        };

        // Register gRPC services
        try self.registerServices();

        // Initialize HTTP/2 server
        if (options.enable_http2) {
            self.http2_server = try http2.Server.init(allocator, .{
                .address = try net.Address.parseIp(options.bind_address, 9090),
                .max_concurrent_streams = 1000,
            });
        }

        // Initialize QUIC/HTTP3 server
        if (options.enable_quic) {
            self.quic_server = try QuicServer.init(allocator, .{
                .address = try net.Address.parseIp(options.bind_address, 9090),
                .alpn_protocols = &[_][]const u8{"h3"},
                .max_concurrent_streams = 1000,
            });
        }

        return self;
    }

    pub fn deinit(self: *Self) void {
        if (self.http2_server) |server| {
            server.deinit();
        }
        if (self.quic_server) |server| {
            server.deinit();
        }
        self.grpc_handler.deinit();
        self.cache.deinit();
    }

    fn registerServices(self: *Self) !void {
        // Register GhostChain service
        try self.grpc_handler.registerService("ghost.chain.v1.GhostChainService", .{
            .ResolveDomain = resolveDomain,
            .GetLatestBlock = getLatestBlock,
            .GetBalance = getBalance,
        });

        // Register GhostDNS service
        try self.grpc_handler.registerService("ghost.dns.v1.GhostDNSService", .{
            .GetStats = getStats,
            .GetCacheStatus = getCacheStatus,
        });
    }

    pub fn run(self: *Self) !void {
        // For now, run serially until we integrate full async runtime
        std.log.info("GhostBridge server starting with async runtime...", .{});

        // Start HTTP/2 server if enabled
        if (self.http2_server) |server| {
            std.log.info("Starting HTTP/2 server...", .{});
            try self.runHttp2Server(server);
        }

        // Start QUIC server if enabled  
        if (self.quic_server) |server| {
            std.log.info("Starting QUIC server...", .{});
            try self.runQuicServer(server);
        }

        // Start stats reporting
        try self.reportStats();
    }

    fn runHttp2Server(self: *Self, server: *http2.Server) !void {
        while (true) {
            var stream = try server.accept();
            // Use async task spawning instead of threads
            _ = try self.spawnStreamHandler(handleHttp2Stream, .{ self, &stream });
        }
    }

    fn runQuicServer(self: *Self, server: *QuicServer) !void {
        while (true) {
            const stream = try server.accept();
            // Use async task spawning instead of threads  
            _ = try self.spawnStreamHandler(handleQuicStream, .{ self, stream });
        }
    }

    fn spawnStreamHandler(self: *Self, comptime handler: anytype, args: anytype) !void {
        // For now, just call directly until we integrate TokioZ
        _ = self;
        try @call(.auto, handler, args);
    }

    fn handleHttp2Stream(self: *Self, stream: *http2.Stream) !void {
        defer stream.close();
        
        const start_time = std.time.milliTimestamp();
        defer {
            const duration = std.time.milliTimestamp() - start_time;
            self.stats.addRequest(duration);
        }

        // Read gRPC frame
        var buffer: [8192]u8 = undefined;
        const frame = try stream.readFrame(&buffer);

        // Check cache
        const cache_key = try self.computeCacheKey(frame.data);
        if (self.cache.get(cache_key)) |cached_response| {
            try stream.writeFrame(cached_response);
            self.stats.incrementCacheHits();
            return;
        }

        // Process request
        const response = try self.grpc_handler.processRequest(frame);
        
        // Cache response
        try self.cache.put(cache_key, response);
        
        // Send response
        try stream.writeFrame(response);
    }

    fn handleQuicStream(self: *Self, stream: *QuicStream) !void {
        // Similar to HTTP/2 but with QUIC-specific handling
        defer stream.close();
        
        const start_time = std.time.milliTimestamp();
        defer {
            const duration = std.time.milliTimestamp() - start_time;
            self.stats.addRequest(duration);
        }

        var buffer: [8192]u8 = undefined;
        const frame = try stream.readFrame(&buffer);

        const cache_key = try self.computeCacheKey(frame.data);
        if (self.cache.get(cache_key)) |cached_response| {
            try stream.writeFrame(cached_response);
            self.stats.incrementCacheHits();
            return;
        }

        const response = try self.grpc_handler.processRequest(frame);
        try self.cache.put(cache_key, response);
        try stream.writeFrame(response);
    }

    fn computeCacheKey(self: *Self, data: []const u8) !u64 {
        _ = self;
        var hasher = std.hash.Wyhash.init(0);
        hasher.update(data);
        return hasher.final();
    }

    fn reportStats(self: *Self) !void {
        const stats = self.stats.snapshot();
        std.log.info("Stats: requests={d} cache_hits={d} avg_latency={d}ms", .{
            stats.total_requests,
            stats.cache_hits,
            stats.avg_latency_ms,
        });
    }

    // gRPC method implementations
    fn resolveDomain(context: *grpc.Context) ![]const u8 {
        const request = try protobuf.decode(context.allocator, protobuf.DomainQuery, context.request_data);
        defer request.deinit();

        // Mock response for prototype
        const response = protobuf.DomainResponse{
            .domain = request.domain,
            .records = &[_]protobuf.DNSRecord{
                .{
                    .type = "A",
                    .value = "192.168.1.100",
                    .priority = 0,
                    .ttl = 300,
                },
                .{
                    .type = "AAAA",
                    .value = "2001:db8::1",
                    .priority = 0,
                    .ttl = 300,
                },
            },
            .owner_id = "ghost1234567890",
            .signature = &[_]u8{0} ** 64,
            .timestamp = @intCast(std.time.timestamp()),
            .ttl = 3600,
        };

        return try protobuf.encode(context.allocator, response);
    }

    fn getLatestBlock(context: *grpc.Context) ![]const u8 {
        _ = context.request_data;
        
        const response = protobuf.BlockResponse{
            .height = 12345,
            .hash = "0x1234567890abcdef",
            .parent_hash = "0x0987654321fedcba",
            .timestamp = @intCast(std.time.timestamp()),
            .transactions = &[_]protobuf.Transaction{},
        };

        return try protobuf.encode(context.allocator, response);
    }

    fn getBalance(context: *grpc.Context) ![]const u8 {
        const request = try protobuf.decode(context.allocator, protobuf.BalanceQuery, context.request_data);
        defer request.deinit();

        const response = protobuf.BalanceResponse{
            .balance = 1000000000, // 1 billion units
            .locked_balance = 100000000, // 100 million locked
        };

        return try protobuf.encode(context.allocator, response);
    }

    fn getStats(context: *grpc.Context) ![]const u8 {
        _ = context.request_data;
        // Get stats from the context's server instance
        // For now, return mock stats until we fix the architecture
        const response = protobuf.DNSStats{
            .queries_total = 12345,
            .cache_hits = 1000,
            .blockchain_queries = 11345,
            .avg_response_time_ms = 5.2,
            .active_connections = 100,
        };

        return try protobuf.encode(context.allocator, response);
    }

    fn getCacheStatus(context: *grpc.Context) ![]const u8 {
        _ = context.request_data;
        // Return mock cache stats for now
        const response = protobuf.CacheStats{
            .entries_count = 500,
            .memory_bytes = 1024 * 1024 * 50, // 50MB
            .hits_total = 1000,
            .misses_total = 500,
            .hit_rate = 0.67,
            .evictions_total = 10,
        };

        return try protobuf.encode(context.allocator, response);
    }
};

const ServerStats = struct {
    total_requests: std.atomic.Value(u64) = std.atomic.Value(u64).init(0),
    cache_hits: std.atomic.Value(u64) = std.atomic.Value(u64).init(0),
    cache_misses: std.atomic.Value(u64) = std.atomic.Value(u64).init(0),
    total_latency_ns: std.atomic.Value(u64) = std.atomic.Value(u64).init(0),
    active_connections: std.atomic.Value(u32) = std.atomic.Value(u32).init(0),

    pub fn addRequest(self: *ServerStats, latency_ms: i64) void {
        _ = self.total_requests.fetchAdd(1, .monotonic);
        _ = self.total_latency_ns.fetchAdd(@intCast(latency_ms * std.time.ns_per_ms), .monotonic);
    }

    pub fn incrementCacheHits(self: *ServerStats) void {
        _ = self.cache_hits.fetchAdd(1, .monotonic);
    }

    pub fn snapshot(self: *const ServerStats) struct {
        total_requests: u64,
        cache_hits: u64,
        avg_latency_ms: f64,
        active_connections: u32,
    } {
        const total = self.total_requests.load(.monotonic);
        const latency = self.total_latency_ns.load(.monotonic);
        
        return .{
            .total_requests = total,
            .cache_hits = self.cache_hits.load(.monotonic),
            .avg_latency_ms = if (total > 0) 
                @as(f64, @floatFromInt(latency)) / @as(f64, @floatFromInt(total)) / std.time.ns_per_ms
                else 0.0,
            .active_connections = self.active_connections.load(.monotonic),
        };
    }
};

const ResponseCache = struct {
    allocator: std.mem.Allocator,
    entries: std.AutoHashMap(u64, CachedResponse),
    max_memory: usize,
    current_memory: usize,
    evictions: u64,
    mutex: std.Thread.Mutex,

    const CachedResponse = struct {
        data: []u8,
        timestamp: i64,
        hits: u32,
    };

    pub fn init(allocator: std.mem.Allocator, max_memory: usize) !ResponseCache {
        return ResponseCache{
            .allocator = allocator,
            .entries = std.AutoHashMap(u64, CachedResponse).init(allocator),
            .max_memory = max_memory,
            .current_memory = 0,
            .evictions = 0,
            .mutex = .{},
        };
    }

    pub fn deinit(self: *ResponseCache) void {
        var iter = self.entries.iterator();
        while (iter.next()) |entry| {
            self.allocator.free(entry.value_ptr.data);
        }
        self.entries.deinit();
    }

    pub fn get(self: *ResponseCache, key: u64) ?[]const u8 {
        self.mutex.lock();
        defer self.mutex.unlock();

        if (self.entries.getPtr(key)) |entry| {
            entry.hits += 1;
            return entry.data;
        }
        return null;
    }

    pub fn put(self: *ResponseCache, key: u64, data: []const u8) !void {
        self.mutex.lock();
        defer self.mutex.unlock();

        const data_copy = try self.allocator.dupe(u8, data);
        
        // Evict old entries if needed
        while (self.current_memory + data.len > self.max_memory) {
            // Simple LRU eviction
            var oldest_key: ?u64 = null;
            var oldest_time: i64 = std.math.maxInt(i64);
            
            var iter = self.entries.iterator();
            while (iter.next()) |entry| {
                if (entry.value_ptr.timestamp < oldest_time) {
                    oldest_time = entry.value_ptr.timestamp;
                    oldest_key = entry.key_ptr.*;
                }
            }
            
            if (oldest_key) |key_to_evict| {
                if (self.entries.fetchRemove(key_to_evict)) |entry| {
                    self.allocator.free(entry.value.data);
                    self.current_memory -= entry.value.data.len;
                    self.evictions += 1;
                }
            } else {
                break;
            }
        }

        try self.entries.put(key, .{
            .data = data_copy,
            .timestamp = std.time.milliTimestamp(),
            .hits = 0,
        });
        self.current_memory += data.len;
    }

    pub fn count(self: *ResponseCache) u64 {
        self.mutex.lock();
        defer self.mutex.unlock();
        return self.entries.count();
    }

    pub fn memoryUsage(self: *ResponseCache) usize {
        self.mutex.lock();
        defer self.mutex.unlock();
        return self.current_memory;
    }
};

// Placeholder for QUIC server - would use a library like quiche or quinn
const QuicServer = struct {
    allocator: std.mem.Allocator,
    
    pub fn init(allocator: std.mem.Allocator, options: anytype) !*QuicServer {
        _ = options;
        const self = try allocator.create(QuicServer);
        self.* = .{ .allocator = allocator };
        return self;
    }
    
    pub fn deinit(self: *QuicServer) void {
        self.allocator.destroy(self);
    }
    
    pub fn accept(self: *QuicServer) !*QuicStream {
        _ = self;
        return error.NotImplemented;
    }
};

const QuicStream = struct {
    pub fn close(self: *QuicStream) void {
        _ = self;
    }
    
    pub fn readFrame(self: *QuicStream, buffer: []u8) !Frame {
        _ = self;
        _ = buffer;
        return error.NotImplemented;
    }
    
    pub fn writeFrame(self: *QuicStream, data: []const u8) !void {
        _ = self;
        _ = data;
    }
};

const Frame = struct {
    data: []const u8,
};