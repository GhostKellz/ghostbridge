const std = @import("std");
const grpc = @import("grpc.zig");
const protobuf = @import("protobuf.zig");
const tokioz = @import("tokioz_stub.zig");
const net = std.net;

/// Wallet Service - gRPC client for walletd
pub const WalletService = struct {
    allocator: std.mem.Allocator,
    walletd_address: net.Address,
    connection_pool: ConnectionPool,
    
    const Self = @This();
    
    const ConnectionPool = struct {
        connections: std.ArrayList(*grpc.ClientConnection),
        max_connections: usize = 10,
        allocator: std.mem.Allocator,
        
        pub fn init(allocator: std.mem.Allocator) ConnectionPool {
            return .{
                .connections = std.ArrayList(*grpc.ClientConnection).init(allocator),
                .allocator = allocator,
            };
        }
        
        pub fn deinit(self: *ConnectionPool) void {
            for (self.connections.items) |conn| {
                conn.close();
                self.allocator.destroy(conn);
            }
            self.connections.deinit();
        }
        
        pub fn getConnection(self: *ConnectionPool) !*grpc.ClientConnection {
            // Find available connection
            for (self.connections.items) |conn| {
                if (conn.isAvailable()) {
                    return conn;
                }
            }
            
            // Create new connection if under limit
            if (self.connections.items.len < self.max_connections) {
                const conn = try self.allocator.create(grpc.ClientConnection);
                try self.connections.append(conn);
                return conn;
            }
            
            // Wait for available connection
            return self.connections.items[0]; // Simple round-robin
        }
    };
    
    pub fn init(allocator: std.mem.Allocator, walletd_host: []const u8, walletd_port: u16) !Self {
        const address = try net.Address.parseIp4(walletd_host, walletd_port);
        
        return Self{
            .allocator = allocator,
            .walletd_address = address,
            .connection_pool = ConnectionPool.init(allocator),
        };
    }
    
    pub fn deinit(self: *Self) void {
        self.connection_pool.deinit();
    }
    
    /// Create a new wallet
    pub fn createWallet(self: *Self, request: CreateWalletRequest) !CreateWalletResponse {
        const conn = try self.connection_pool.getConnection();
        defer conn.release();
        
        try conn.connect(self.walletd_address);
        
        // Create gRPC request
        const grpc_request = grpc.Request{
            .method = "/walletd.WalletService/CreateWallet",
            .headers = &[_]grpc.Header{
                .{ .name = "content-type", .value = "application/grpc" },
            },
            .body = try protobuf.encode(self.allocator, request),
        };
        
        const response = try conn.sendRequest(grpc_request);
        defer response.deinit();
        
        return try protobuf.decode(CreateWalletResponse, response.body);
    }
    
    /// Send transaction
    pub fn sendTransaction(self: *Self, request: SendTransactionRequest) !SendTransactionResponse {
        const conn = try self.connection_pool.getConnection();
        defer conn.release();
        
        try conn.connect(self.walletd_address);
        
        const grpc_request = grpc.Request{
            .method = "/walletd.WalletService/SendTransaction",
            .headers = &[_]grpc.Header{
                .{ .name = "content-type", .value = "application/grpc" },
            },
            .body = try protobuf.encode(self.allocator, request),
        };
        
        const response = try conn.sendRequest(grpc_request);
        defer response.deinit();
        
        return try protobuf.decode(SendTransactionResponse, response.body);
    }
    
    /// Get wallet balance
    pub fn getBalance(self: *Self, request: GetBalanceRequest) !GetBalanceResponse {
        const conn = try self.connection_pool.getConnection();
        defer conn.release();
        
        try conn.connect(self.walletd_address);
        
        const grpc_request = grpc.Request{
            .method = "/walletd.WalletService/GetWalletBalance",
            .headers = &[_]grpc.Header{
                .{ .name = "content-type", .value = "application/grpc" },
            },
            .body = try protobuf.encode(self.allocator, request),
        };
        
        const response = try conn.sendRequest(grpc_request);
        defer response.deinit();
        
        return try protobuf.decode(GetBalanceResponse, response.body);
    }
    
    /// Sign transaction
    pub fn signTransaction(self: *Self, request: SignTransactionRequest) !SignTransactionResponse {
        const conn = try self.connection_pool.getConnection();
        defer conn.release();
        
        try conn.connect(self.walletd_address);
        
        const grpc_request = grpc.Request{
            .method = "/walletd.WalletService/SignTransaction",
            .headers = &[_]grpc.Header{
                .{ .name = "content-type", .value = "application/grpc" },
            },
            .body = try protobuf.encode(self.allocator, request),
        };
        
        const response = try conn.sendRequest(grpc_request);
        defer response.deinit();
        
        return try protobuf.decode(SignTransactionResponse, response.body);
    }
    
    /// Handle async requests using TokioZ
    pub fn handleAsync(self: *Self, request: anytype) !void {
        const AsyncWalletTask = struct {
            service: *WalletService,
            req: @TypeOf(request),
            
            pub fn run(task: @This()) !void {
                // Process wallet requests asynchronously
                const result = switch (@TypeOf(task.req)) {
                    CreateWalletRequest => try task.service.createWallet(task.req),
                    SendTransactionRequest => try task.service.sendTransaction(task.req),
                    GetBalanceRequest => try task.service.getBalance(task.req),
                    SignTransactionRequest => try task.service.signTransaction(task.req),
                    else => return error.UnknownRequestType,
                };
                
                // Handle result
                std.log.info("Wallet operation completed: {any}", .{result});
            }
        };
        
        const task = AsyncWalletTask{ .service = self, .req = request };
        try tokioz.runtime.run(task.run);
    }
};

// Request/Response types matching walletd proto
pub const CreateWalletRequest = struct {
    name: []const u8,
    account_type: ?[]const u8 = "ed25519",
    passphrase: ?[]const u8 = null,
    network: ?[]const u8 = null,
};

pub const CreateWalletResponse = struct {
    success: bool,
    wallet: ?Wallet,
    err: ?[]const u8,
};

pub const Wallet = struct {
    id: []const u8,
    name: []const u8,
    account_type: []const u8,
    address: []const u8,
    public_key: []const u8,
    created_at: i64,
};

pub const SendTransactionRequest = struct {
    from_wallet_id: []const u8,
    to_address: []const u8,
    amount: []const u8,
    passphrase: []const u8,
    gas_limit: ?u64 = null,
    gas_price: ?u64 = null,
};

pub const SendTransactionResponse = struct {
    success: bool,
    transaction_hash: []const u8,
    status: []const u8,
    err: ?[]const u8,
};

pub const GetBalanceRequest = struct {
    wallet_id: []const u8,
};

pub const GetBalanceResponse = struct {
    success: bool,
    balances: []Balance,
    err: ?[]const u8,
};

pub const Balance = struct {
    token: []const u8,
    amount: []const u8,
    decimals: u32,
};

pub const SignTransactionRequest = struct {
    wallet_id: []const u8,
    transaction_data: []const u8,
    passphrase: []const u8,
};

pub const SignTransactionResponse = struct {
    success: bool,
    signature: []const u8,
    public_key: []const u8,
    err: ?[]const u8,
};

/// Register wallet service proxy handlers
pub fn registerWalletService(grpc_handler: *grpc.Handler, wallet_service: *WalletService) !void {
    try grpc_handler.registerMethod("wallet.WalletService/CreateWallet",
        struct {
            fn handle(service: *WalletService, req: CreateWalletRequest) !CreateWalletResponse {
                return service.createWallet(req);
            }
        }.handle, wallet_service);
        
    try grpc_handler.registerMethod("wallet.WalletService/SendTransaction",
        struct {
            fn handle(service: *WalletService, req: SendTransactionRequest) !SendTransactionResponse {
                return service.sendTransaction(req);
            }
        }.handle, wallet_service);
        
    try grpc_handler.registerMethod("wallet.WalletService/GetBalance",
        struct {
            fn handle(service: *WalletService, req: GetBalanceRequest) !GetBalanceResponse {
                return service.getBalance(req);
            }
        }.handle, wallet_service);
        
    try grpc_handler.registerMethod("wallet.WalletService/SignTransaction",
        struct {
            fn handle(service: *WalletService, req: SignTransactionRequest) !SignTransactionResponse {
                return service.signTransaction(req);
            }
        }.handle, wallet_service);
}