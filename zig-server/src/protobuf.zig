const std = @import("std");

// Protobuf wire types
const WireType = enum(u3) {
    varint = 0,
    fixed64 = 1,
    length_delimited = 2,
    start_group = 3,
    end_group = 4,
    fixed32 = 5,
};

// Domain query message
pub const DomainQuery = struct {
    domain: []const u8,
    record_types: [][]const u8,
    allocator: std.mem.Allocator,

    pub fn deinit(self: *const DomainQuery) void {
        self.allocator.free(self.domain);
        for (self.record_types) |rt| {
            self.allocator.free(rt);
        }
        self.allocator.free(self.record_types);
    }
};

// Domain response message
pub const DomainResponse = struct {
    domain: []const u8,
    records: []const DNSRecord,
    owner_id: []const u8,
    signature: []const u8,
    timestamp: u64,
    ttl: u32,
};

pub const DNSRecord = struct {
    type: []const u8,
    value: []const u8,
    priority: u32,
    ttl: u32,
};

// Block response
pub const BlockResponse = struct {
    height: u64,
    hash: []const u8,
    parent_hash: []const u8,
    timestamp: u64,
    transactions: []const Transaction,
};

pub const Transaction = struct {
    id: []const u8,
    from: []const u8,
    to: []const u8,
    amount: u64,
    data: []const u8,
};

// Balance query/response
pub const BalanceQuery = struct {
    account_id: []const u8,
    allocator: std.mem.Allocator,

    pub fn deinit(self: *const BalanceQuery) void {
        self.allocator.free(self.account_id);
    }
};

pub const BalanceResponse = struct {
    balance: u64,
    locked_balance: u64,
};

// DNS stats
pub const DNSStats = struct {
    queries_total: u64,
    cache_hits: u64,
    blockchain_queries: u64,
    avg_response_time_ms: f64,
    active_connections: u64,
};

pub const CacheStats = struct {
    entries_count: u64,
    memory_bytes: u64,
    hits_total: u64,
    misses_total: u64,
    hit_rate: f64,
    evictions_total: u64,
};

// Decoder
pub fn decode(allocator: std.mem.Allocator, comptime T: type, data: []const u8) !T {
    var decoder = Decoder{
        .data = data,
        .offset = 0,
        .allocator = allocator,
    };
    
    return switch (T) {
        DomainQuery => try decoder.decodeDomainQuery(),
        BalanceQuery => try decoder.decodeBalanceQuery(),
        else => error.UnsupportedType,
    };
}

// Encoder
pub fn encode(allocator: std.mem.Allocator, value: anytype) ![]u8 {
    var encoder = Encoder{
        .buffer = std.ArrayList(u8).init(allocator),
        .allocator = allocator,
    };
    
    switch (@TypeOf(value)) {
        DomainResponse => try encoder.encodeDomainResponse(value),
        BlockResponse => try encoder.encodeBlockResponse(value),
        BalanceResponse => try encoder.encodeBalanceResponse(value),
        DNSStats => try encoder.encodeDNSStats(value),
        CacheStats => try encoder.encodeCacheStats(value),
        else => return error.UnsupportedType,
    }
    
    return encoder.buffer.toOwnedSlice();
}

const Decoder = struct {
    data: []const u8,
    offset: usize,
    allocator: std.mem.Allocator,

    fn decodeDomainQuery(self: *Decoder) !DomainQuery {
        var domain: []const u8 = "";
        var record_types = std.ArrayList([]const u8).init(self.allocator);

        while (self.offset < self.data.len) {
            const field_header = try self.readVarint();
            const field_number = @as(u32, @intCast(field_header >> 3));
            const wire_type = @as(WireType, @enumFromInt(@as(u3, @intCast(field_header & 0x7))));

            switch (field_number) {
                1 => { // domain
                    if (wire_type != .length_delimited) return error.InvalidWireType;
                    domain = try self.readString();
                },
                2 => { // record_types
                    if (wire_type != .length_delimited) return error.InvalidWireType;
                    const record_type = try self.readString();
                    try record_types.append(record_type);
                },
                else => {
                    // Skip unknown fields
                    try self.skipField(wire_type);
                },
            }
        }

        return DomainQuery{
            .domain = domain,
            .record_types = try record_types.toOwnedSlice(),
            .allocator = self.allocator,
        };
    }

    fn decodeBalanceQuery(self: *Decoder) !BalanceQuery {
        var account_id: []const u8 = "";

        while (self.offset < self.data.len) {
            const field_header = try self.readVarint();
            const field_number = @as(u32, @intCast(field_header >> 3));
            const wire_type = @as(WireType, @enumFromInt(@as(u3, @intCast(field_header & 0x7))));

            switch (field_number) {
                1 => { // account_id
                    if (wire_type != .length_delimited) return error.InvalidWireType;
                    account_id = try self.readString();
                },
                else => {
                    try self.skipField(wire_type);
                },
            }
        }

        return BalanceQuery{
            .account_id = account_id,
            .allocator = self.allocator,
        };
    }

    fn readVarint(self: *Decoder) !u64 {
        var result: u64 = 0;
        var shift: u6 = 0;

        while (true) {
            if (self.offset >= self.data.len) return error.UnexpectedEOF;
            
            const byte = self.data[self.offset];
            self.offset += 1;

            result |= @as(u64, byte & 0x7F) << shift;

            if ((byte & 0x80) == 0) break;
            
            shift += 7;
            if (shift >= 64) return error.VarintOverflow;
        }

        return result;
    }

    fn readString(self: *Decoder) ![]const u8 {
        const len = try self.readVarint();
        if (self.offset + len > self.data.len) return error.UnexpectedEOF;
        
        const str = try self.allocator.dupe(u8, self.data[self.offset..self.offset + len]);
        self.offset += len;
        
        return str;
    }

    fn skipField(self: *Decoder, wire_type: WireType) !void {
        switch (wire_type) {
            .varint => _ = try self.readVarint(),
            .fixed64 => self.offset += 8,
            .length_delimited => {
                const len = try self.readVarint();
                self.offset += len;
            },
            .fixed32 => self.offset += 4,
            else => return error.UnsupportedWireType,
        }
    }
};

const Encoder = struct {
    buffer: std.ArrayList(u8),
    allocator: std.mem.Allocator,

    fn encodeDomainResponse(self: *Encoder, response: DomainResponse) !void {
        // Field 1: domain
        try self.writeTag(1, .length_delimited);
        try self.writeString(response.domain);

        // Field 2: records
        for (response.records) |record| {
            try self.writeTag(2, .length_delimited);
            try self.encodeDNSRecord(record);
        }

        // Field 3: owner_id
        try self.writeTag(3, .length_delimited);
        try self.writeString(response.owner_id);

        // Field 4: signature
        try self.writeTag(4, .length_delimited);
        try self.writeBytes(response.signature);

        // Field 5: timestamp
        try self.writeTag(5, .varint);
        try self.writeVarint(response.timestamp);

        // Field 6: ttl
        try self.writeTag(6, .varint);
        try self.writeVarint(response.ttl);
    }

    fn encodeDNSRecord(self: *Encoder, record: DNSRecord) !void {
        var temp_buffer = std.ArrayList(u8).init(self.allocator);
        defer temp_buffer.deinit();

        // Encode into temporary buffer first
        var temp_encoder = Encoder{
            .buffer = temp_buffer,
            .allocator = self.allocator,
        };

        try temp_encoder.writeTag(1, .length_delimited);
        try temp_encoder.writeString(record.type);
        
        try temp_encoder.writeTag(2, .length_delimited);
        try temp_encoder.writeString(record.value);
        
        try temp_encoder.writeTag(3, .varint);
        try temp_encoder.writeVarint(record.priority);
        
        try temp_encoder.writeTag(4, .varint);
        try temp_encoder.writeVarint(record.ttl);

        // Write length-delimited
        try self.writeVarint(temp_encoder.buffer.items.len);
        try self.buffer.appendSlice(temp_encoder.buffer.items);
    }

    fn encodeBlockResponse(self: *Encoder, response: BlockResponse) !void {
        try self.writeTag(1, .varint);
        try self.writeVarint(response.height);

        try self.writeTag(2, .length_delimited);
        try self.writeString(response.hash);

        try self.writeTag(3, .length_delimited);
        try self.writeString(response.parent_hash);

        try self.writeTag(4, .varint);
        try self.writeVarint(response.timestamp);
    }

    fn encodeBalanceResponse(self: *Encoder, response: BalanceResponse) !void {
        try self.writeTag(1, .varint);
        try self.writeVarint(response.balance);

        try self.writeTag(2, .varint);
        try self.writeVarint(response.locked_balance);
    }

    fn encodeDNSStats(self: *Encoder, stats: DNSStats) !void {
        try self.writeTag(1, .varint);
        try self.writeVarint(stats.queries_total);

        try self.writeTag(2, .varint);
        try self.writeVarint(stats.cache_hits);

        try self.writeTag(3, .varint);
        try self.writeVarint(stats.blockchain_queries);

        try self.writeTag(4, .fixed64);
        try self.writeFixed64(@bitCast(stats.avg_response_time_ms));

        try self.writeTag(5, .varint);
        try self.writeVarint(stats.active_connections);
    }

    fn encodeCacheStats(self: *Encoder, stats: CacheStats) !void {
        try self.writeTag(1, .varint);
        try self.writeVarint(stats.entries_count);

        try self.writeTag(2, .varint);
        try self.writeVarint(stats.memory_bytes);

        try self.writeTag(3, .varint);
        try self.writeVarint(stats.hits_total);

        try self.writeTag(4, .varint);
        try self.writeVarint(stats.misses_total);

        try self.writeTag(5, .fixed64);
        try self.writeFixed64(@bitCast(stats.hit_rate));

        try self.writeTag(6, .varint);
        try self.writeVarint(stats.evictions_total);
    }

    fn writeTag(self: *Encoder, field_number: u32, wire_type: WireType) !void {
        const tag = (field_number << 3) | @intFromEnum(wire_type);
        try self.writeVarint(tag);
    }

    fn writeVarint(self: *Encoder, value: u64) !void {
        var v = value;
        while (v >= 0x80) {
            try self.buffer.append(@as(u8, @intCast((v & 0x7F) | 0x80)));
            v >>= 7;
        }
        try self.buffer.append(@as(u8, @intCast(v)));
    }

    fn writeFixed64(self: *Encoder, value: u64) !void {
        var bytes: [8]u8 = undefined;
        std.mem.writeInt(u64, &bytes, value, .little);
        try self.buffer.appendSlice(&bytes);
    }

    fn writeString(self: *Encoder, value: []const u8) !void {
        try self.writeVarint(value.len);
        try self.buffer.appendSlice(value);
    }

    fn writeBytes(self: *Encoder, value: []const u8) !void {
        try self.writeVarint(value.len);
        try self.buffer.appendSlice(value);
    }
};