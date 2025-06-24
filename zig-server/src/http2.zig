const std = @import("std");
const net = std.net;

pub const Server = struct {
    allocator: std.mem.Allocator,
    listener: net.Server,
    options: Options,

    pub const Options = struct {
        address: net.Address,
        max_concurrent_streams: u32 = 1000,
        initial_window_size: u32 = 65535,
        max_frame_size: u32 = 16384,
    };

    pub fn init(allocator: std.mem.Allocator, options: Options) !*Server {
        const self = try allocator.create(Server);
        errdefer allocator.destroy(self);

        self.* = .{
            .allocator = allocator,
            .listener = try net.Address.listen(options.address, .{
                .reuse_address = true,
                .kernel_backlog = 128,
            }),
            .options = options,
        };

        return self;
    }

    pub fn deinit(self: *Server) void {
        self.listener.deinit();
        self.allocator.destroy(self);
    }

    pub fn accept(self: *Server) !Stream {
        const connection = try self.listener.accept();
        
        // Perform HTTP/2 handshake
        try self.performHandshake(connection.stream);
        
        return Stream{
            .connection = connection,
            .window_size = self.options.initial_window_size,
        };
    }

    fn performHandshake(self: *Server, stream: net.Stream) !void {
        _ = self;
        
        // Send HTTP/2 connection preface
        const preface = "PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";
        try stream.writeAll(preface);
        
        // Send SETTINGS frame
        const settings_frame = [_]u8{
            0x00, 0x00, 0x00, // Length: 0
            0x04, // Type: SETTINGS
            0x00, // Flags: none
            0x00, 0x00, 0x00, 0x00, // Stream ID: 0
        };
        try stream.writeAll(&settings_frame);
    }
};

pub const Stream = struct {
    connection: net.Server.Connection,
    window_size: u32,
    stream_id: u32 = 1,

    pub fn close(self: *Stream) void {
        self.connection.stream.close();
    }

    pub fn readFrame(self: *Stream, buffer: []u8) !Frame {
        // Read HTTP/2 frame header (9 bytes)
        var header: [9]u8 = undefined;
        _ = try self.connection.stream.read(&header);
        
        const length = (@as(u32, header[0]) << 16) | (@as(u32, header[1]) << 8) | header[2];
        const frame_type = header[3];
        const flags = header[4];
        const stream_id = (@as(u32, header[5] & 0x7F) << 24) |
                         (@as(u32, header[6]) << 16) |
                         (@as(u32, header[7]) << 8) |
                         header[8];
        
        // Read frame payload
        if (length > buffer.len) return error.BufferTooSmall;
        _ = try self.connection.stream.read(buffer[0..length]);
        
        return Frame{
            .type = frame_type,
            .flags = flags,
            .stream_id = stream_id,
            .data = buffer[0..length],
        };
    }

    pub fn writeFrame(self: *Stream, data: []const u8) !void {
        // Write DATA frame
        var header: [9]u8 = undefined;
        
        // Length (24 bits)
        header[0] = @intCast((data.len >> 16) & 0xFF);
        header[1] = @intCast((data.len >> 8) & 0xFF);
        header[2] = @intCast(data.len & 0xFF);
        
        // Type: DATA
        header[3] = 0x00;
        
        // Flags: END_STREAM
        header[4] = 0x01;
        
        // Stream ID
        header[5] = @intCast((self.stream_id >> 24) & 0x7F);
        header[6] = @intCast((self.stream_id >> 16) & 0xFF);
        header[7] = @intCast((self.stream_id >> 8) & 0xFF);
        header[8] = @intCast(self.stream_id & 0xFF);
        
        try self.connection.stream.writeAll(&header);
        try self.connection.stream.writeAll(data);
    }
};

pub const Frame = struct {
    type: u8,
    flags: u8,
    stream_id: u32,
    data: []const u8,
};