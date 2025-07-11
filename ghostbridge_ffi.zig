const std = @import("std");
const shroud = @import("shroud");
const grpc = @import("zig-server/src/grpc.zig");

// Global allocator for FFI
var gpa = std.heap.GeneralPurposeAllocator(.{}){};
const allocator = gpa.allocator();

// Opaque handle types for C ABI
pub const GhostBridge = opaque {};
pub const GrpcStream = opaque {};

// C-compatible structures
pub const BridgeConfig = extern struct {
    host: [*:0]const u8,
    port: u16,
    cert_path: [*:0]const u8,
    key_path: [*:0]const u8,
};

pub const GrpcRequest = extern struct {
    service: [*:0]const u8,
    method: [*:0]const u8,
    data: [*]const u8,
    data_len: usize,
};

pub const GrpcResponse = extern struct {
    data: [*]u8,
    data_len: usize,
    status_code: i32,
    status_message: [*:0]const u8,
};

// Internal bridge structure
const GhostBridgeImpl = struct {
    server: grpc.server.GrpcServer,
    allocator: std.mem.Allocator,
    
    pub fn init(alloc: std.mem.Allocator, config: BridgeConfig) !*GhostBridgeImpl {
        const self = try alloc.create(GhostBridgeImpl);
        
        const host = std.mem.span(config.host);
        const cert_path = std.mem.span(config.cert_path);
        const key_path = std.mem.span(config.key_path);
        
        self.* = .{
            .server = try grpc.server.GrpcServer.init(alloc, .{
                .host = host,
                .port = config.port,
                .cert_path = cert_path,
                .key_path = key_path,
            }),
            .allocator = alloc,
        };
        
        return self;
    }
    
    pub fn deinit(self: *GhostBridgeImpl) void {
        self.server.deinit();
        self.allocator.destroy(self);
    }
};

// FFI exports for Rust integration
pub export fn ghostbridge_init(config: *const BridgeConfig) callconv(.C) ?*GhostBridge {
    std.log.info("Initializing GhostBridge with host: {s}, port: {}", .{ config.host, config.port });
    
    const bridge = GhostBridgeImpl.init(allocator, config.*) catch |err| {
        std.log.err("Failed to initialize GhostBridge: {}", .{err});
        return null;
    };
    
    return @ptrCast(bridge);
}

pub export fn ghostbridge_destroy(bridge: ?*GhostBridge) callconv(.C) void {
    if (bridge) |b| {
        const impl: *GhostBridgeImpl = @ptrCast(@alignCast(b));
        impl.deinit();
    }
}

pub export fn ghostbridge_register_service(
    bridge: ?*GhostBridge,
    name: [*:0]const u8,
    endpoint: [*:0]const u8
) callconv(.C) c_int {
    if (bridge) |b| {
        const impl: *GhostBridgeImpl = @ptrCast(@alignCast(b));
        const service_name = std.mem.span(name);
        const service_endpoint = std.mem.span(endpoint);
        
        std.log.info("Registering service: {s} -> {s}", .{ service_name, service_endpoint });
        
        // Create echo service for now (will be replaced with actual service registration)
        const echo_service = impl.allocator.create(grpc.server.EchoService) catch return -1;
        echo_service.* = grpc.server.EchoService.init(impl.allocator);
        
        impl.server.registerService(service_name, echo_service) catch |err| {
            std.log.err("Failed to register service: {}", .{err});
            return -1;
        };
        
        return 0;
    }
    return -1;
}

pub export fn ghostbridge_relay_call(
    bridge: ?*GhostBridge,
    request: *const GrpcRequest
) callconv(.C) ?*GrpcResponse {
    if (bridge) |b| {
        const impl: *GhostBridgeImpl = @ptrCast(@alignCast(b));
        
        const service_str = std.mem.span(request.service);
        const method_str = std.mem.span(request.method);
        const request_data = request.data[0..request.data_len];
        
        std.log.info("Relaying gRPC call: {s}/{s}", .{ service_str, method_str });
        
        // Create response
        const response = allocator.create(GrpcResponse) catch return null;
        
        // For now, echo back the request
        const response_data = allocator.alloc(u8, request_data.len) catch {
            allocator.destroy(response);
            return null;
        };
        @memcpy(response_data, request_data);
        
        response.* = .{
            .data = response_data.ptr,
            .data_len = response_data.len,
            .status_code = 0,
            .status_message = "OK",
        };
        
        return response;
    }
    return null;
}

pub export fn ghostbridge_free_response(response: ?*GrpcResponse) callconv(.C) void {
    if (response) |r| {
        if (r.data_len > 0) {
            allocator.free(r.data[0..r.data_len]);
        }
        allocator.destroy(r);
    }
}

// Stream operations for bidirectional gRPC streaming
pub export fn ghostbridge_create_stream(
    bridge: ?*GhostBridge,
    service: [*:0]const u8,
    method: [*:0]const u8
) callconv(.C) ?*GrpcStream {
    if (bridge) |b| {
        const impl: *GhostBridgeImpl = @ptrCast(@alignCast(b));
        
        const service_str = std.mem.span(service);
        const method_str = std.mem.span(method);
        
        std.log.info("Creating gRPC stream: {s}/{s}", .{ service_str, method_str });
        
        // Create stream message
        const stream = impl.allocator.create(grpc.server.GrpcMessage) catch return null;
        stream.* = grpc.server.GrpcMessage{
            .message_type = .stream_data,
            .stream_id = std.crypto.random.int(u32),
            .flags = 0,
            .data = &[_]u8{},
        };
        
        return @ptrCast(stream);
    }
    return null;
}

pub export fn ghostbridge_stream_send(
    stream: ?*GrpcStream,
    data: [*]const u8,
    data_len: usize
) callconv(.C) c_int {
    if (stream) |s| {
        const real_stream: *grpc.server.GrpcMessage = @ptrCast(@alignCast(s));
        const message_data = data[0..data_len];
        
        // Update stream data
        real_stream.data = message_data;
        
        std.log.debug("Sent {} bytes on stream {}", .{ data_len, real_stream.stream_id });
        return 0;
    }
    return -1;
}

pub export fn ghostbridge_stream_receive(
    stream: ?*GrpcStream,
    buffer: [*]u8,
    buffer_len: usize
) callconv(.C) isize {
    if (stream) |s| {
        const real_stream: *grpc.server.GrpcMessage = @ptrCast(@alignCast(s));
        
        // For now, return empty data
        _ = real_stream;
        _ = buffer;
        _ = buffer_len;
        
        return 0;
    }
    return -1;
}

pub export fn ghostbridge_stream_close(stream: ?*GrpcStream) callconv(.C) void {
    if (stream) |s| {
        const real_stream: *grpc.server.GrpcMessage = @ptrCast(@alignCast(s));
        allocator.destroy(real_stream);
    }
}

// Server lifecycle
pub export fn ghostbridge_start(bridge: ?*GhostBridge) callconv(.C) c_int {
    if (bridge) |b| {
        const impl: *GhostBridgeImpl = @ptrCast(@alignCast(b));
        impl.server.start() catch |err| {
            std.log.err("Failed to start server: {}", .{err});
            return -1;
        };
        return 0;
    }
    return -1;
}

pub export fn ghostbridge_stop(bridge: ?*GhostBridge) callconv(.C) c_int {
    if (bridge) |b| {
        const impl: *GhostBridgeImpl = @ptrCast(@alignCast(b));
        impl.server.stop() catch |err| {
            std.log.err("Failed to stop server: {}", .{err});
            return -1;
        };
        return 0;
    }
    return -1;
}