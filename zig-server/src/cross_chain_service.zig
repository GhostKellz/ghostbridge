const std = @import("std");
const grpc = @import("grpc.zig");
const protobuf = @import("protobuf.zig");
const eth_bridge = @import("eth_bridge.zig");
const stellar_bridge = @import("stellar_bridge.zig");

pub const CrossChainService = struct {
    allocator: std.mem.Allocator,
    eth_service: *eth_bridge.EthBridgeService,
    stellar_service: *stellar_bridge.StellarBridgeService,
    ghostchain_handler: ?*grpc.Handler,
    active_transfers: std.AutoHashMap([]const u8, TransferState),
    
    const Self = @This();

    const TransferState = struct {
        transfer_id: []const u8,
        source_chain: ChainType,
        destination_chain: ChainType,
        status: TransferStatus,
        source_tx_hash: ?[]const u8,
        destination_tx_hash: ?[]const u8,
        created_at: i64,
        updated_at: i64,
        request: protobuf.CrossChainTransferRequest,
    };

    const ChainType = enum(u8) {
        unspecified = 0,
        ghostchain = 1,
        ethereum = 2,
        stellar = 3,
    };

    const TransferStatus = enum(u8) {
        unspecified = 0,
        pending = 1,
        source_confirmed = 2,
        bridging = 3,
        destination_pending = 4,
        completed = 5,
        failed = 6,
        refunded = 7,
    };

    pub fn init(
        allocator: std.mem.Allocator,
        eth_service: *eth_bridge.EthBridgeService,
        stellar_service: *stellar_bridge.StellarBridgeService,
        ghostchain_handler: ?*grpc.Handler,
    ) !Self {
        return Self{
            .allocator = allocator,
            .eth_service = eth_service,
            .stellar_service = stellar_service,
            .ghostchain_handler = ghostchain_handler,
            .active_transfers = std.AutoHashMap([]const u8, TransferState).init(allocator),
        };
    }

    pub fn deinit(self: *Self) void {
        // Clean up active transfers
        var iter = self.active_transfers.iterator();
        while (iter.next()) |entry| {
            self.allocator.free(entry.key_ptr.*);
        }
        self.active_transfers.deinit();
    }

    pub fn registerService(self: *Self, grpc_handler: *grpc.Handler) !void {
        try grpc_handler.registerService("ghost.crosschain.v1.CrossChainService", .{
            .CrossChainTransfer = crossChainTransfer,
            .CrossChainIdentity = crossChainIdentity,
            .CrossChainDomainLookup = crossChainDomainLookup,
            .GetTransferStatus = getTransferStatus,
            .GetSupportedChains = getSupportedChains,
            .EstimateTransferFee = estimateTransferFee,
            .SubscribeTransferEvents = subscribeTransferEvents,
        });
        
        // Store service instance for method handlers
        try grpc_handler.setServiceData("ghost.crosschain.v1.CrossChainService", self);
    }

    // Cross-chain transfer implementation
    fn crossChainTransfer(context: *grpc.Context) ![]const u8 {
        const request = try protobuf.decode(context.allocator, protobuf.CrossChainTransferRequest, context.request_data);
        defer request.deinit();

        std.log.info("CrossChainTransfer request: {s} -> {s} amount={s} asset={s}", .{ 
            @tagName(request.source_chain),
            @tagName(request.destination_chain),
            request.amount,
            request.asset_code,
        });

        // Generate transfer ID
        const transfer_id = try generateTransferId(context.allocator);
        
        // Create transfer state
        const transfer_state = TransferState{
            .transfer_id = transfer_id,
            .source_chain = request.source_chain,
            .destination_chain = request.destination_chain,
            .status = .pending,
            .source_tx_hash = null,
            .destination_tx_hash = null,
            .created_at = std.time.timestamp(),
            .updated_at = std.time.timestamp(),
            .request = request,
        };

        // Store transfer state (in a real implementation, this would be persisted)
        // try self.active_transfers.put(transfer_id, transfer_state);

        // Mock response - return transfer initiated
        const response = protobuf.CrossChainTransferResponse{
            .transfer_id = transfer_id,
            .source_transaction_hash = "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef",
            .destination_transaction_hash = "",
            .status = .pending,
            .estimated_completion_time = "300", // 5 minutes
            .bridge_fee = "0.01",
            .network_fee = "0.005",
            .error = null,
        };

        return try protobuf.encode(context.allocator, response);
    }

    fn crossChainIdentity(context: *grpc.Context) ![]const u8 {
        const request = try protobuf.decode(context.allocator, protobuf.CrossChainIdentityRequest, context.request_data);
        defer request.deinit();

        std.log.info("CrossChainIdentity request: identifier={s} chains={any}", .{ 
            request.identifier,
            request.chains,
        });

        // Mock response - return cross-chain identity
        const response = protobuf.CrossChainIdentityResponse{
            .primary_identifier = request.identifier,
            .identities = &[_]protobuf.ChainIdentity{
                .{
                    .chain = .ghostchain,
                    .address = "ghost1234567890abcdef",
                    .did = "did:ghost:1234567890abcdef",
                    .ghost_id = "ghost1234567890",
                    .metadata = std.HashMap([]const u8, []const u8).init(context.allocator),
                    .verification_method = "ed25519",
                    .last_updated = @intCast(std.time.timestamp()),
                },
                .{
                    .chain = .ethereum,
                    .address = "0x742d35Cc6634C0532925a3b8D78f78c3bfABC6d5",
                    .did = "",
                    .ghost_id = "",
                    .metadata = std.HashMap([]const u8, []const u8).init(context.allocator),
                    .verification_method = "secp256k1",
                    .last_updated = @intCast(std.time.timestamp()),
                },
            },
            .linked_domains = &[_][]const u8{
                "example.ghost",
                "example.eth",
            },
            .proofs = &[_]protobuf.IdentityProof{},
            .error = null,
        };

        return try protobuf.encode(context.allocator, response);
    }

    fn crossChainDomainLookup(context: *grpc.Context) ![]const u8 {
        const request = try protobuf.decode(context.allocator, protobuf.CrossChainDomainRequest, context.request_data);
        defer request.deinit();

        std.log.info("CrossChainDomainLookup request: domain={s} chains={any}", .{ 
            request.domain,
            request.chains,
        });

        // Mock response - return cross-chain domain records
        const response = protobuf.CrossChainDomainResponse{
            .domain = request.domain,
            .records = &[_]protobuf.ChainDomainRecord{
                .{
                    .chain = .ghostchain,
                    .resolved_address = "ghost1234567890abcdef",
                    .owner = "ghost1234567890abcdef",
                    .records = std.HashMap([]const u8, []const u8).init(context.allocator),
                    .expiry = @intCast(std.time.timestamp() + 31536000), // 1 year
                    .registry_contract = "",
                },
                .{
                    .chain = .ethereum,
                    .resolved_address = "0x742d35Cc6634C0532925a3b8D78f78c3bfABC6d5",
                    .owner = "0x742d35Cc6634C0532925a3b8D78f78c3bfABC6d5",
                    .records = std.HashMap([]const u8, []const u8).init(context.allocator),
                    .expiry = @intCast(std.time.timestamp() + 31536000),
                    .registry_contract = "0x00000000000C2E074eC69A0dFb2997BA6C7d2e1e",
                },
            },
            .primary_owner = "ghost1234567890abcdef",
            .linked_addresses = &[_][]const u8{
                "ghost1234567890abcdef",
                "0x742d35Cc6634C0532925a3b8D78f78c3bfABC6d5",
            },
            .error = null,
        };

        return try protobuf.encode(context.allocator, response);
    }

    fn getTransferStatus(context: *grpc.Context) ![]const u8 {
        const request = try protobuf.decode(context.allocator, protobuf.TransferStatusRequest, context.request_data);
        defer request.deinit();

        std.log.info("GetTransferStatus request: transfer_id={s}", .{request.transfer_id});

        // Mock response - return transfer status
        const response = protobuf.TransferStatusResponse{
            .transfer_id = request.transfer_id,
            .original_request = protobuf.CrossChainTransferRequest{
                .source_chain = .ethereum,
                .destination_chain = .stellar,
                .source_address = "0x742d35Cc6634C0532925a3b8D78f78c3bfABC6d5",
                .destination_address = "GAHK7EEG2WWHVKDNT4CEQFZGKF2LGDSW2IVM4S5DP42RBW3K6BTODB4A",
                .amount = "100.0",
                .asset_code = "USDC",
                .asset_issuer = "",
                .memo = "Cross-chain transfer",
                .gas_limit = "21000",
                .gas_price = "20000000000",
                .network = "mainnet",
            },
            .status = .completed,
            .source_transaction_hash = "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef",
            .destination_transaction_hash = "fedcba0987654321fedcba0987654321fedcba0987654321fedcba0987654321",
            .source_confirmations = 12,
            .destination_confirmations = 1,
            .estimated_completion_time = "0",
            .steps = &[_]protobuf.TransferStep{
                .{
                    .step_name = "Source transaction confirmed",
                    .status = .completed,
                    .transaction_hash = "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef",
                    .timestamp = @intCast(std.time.timestamp() - 300),
                    .details = "Transaction confirmed with 12 confirmations",
                },
                .{
                    .step_name = "Bridge processing",
                    .status = .completed,
                    .transaction_hash = "",
                    .timestamp = @intCast(std.time.timestamp() - 240),
                    .details = "Asset locked and bridge initiated",
                },
                .{
                    .step_name = "Destination transaction submitted",
                    .status = .completed,
                    .transaction_hash = "fedcba0987654321fedcba0987654321fedcba0987654321fedcba0987654321",
                    .timestamp = @intCast(std.time.timestamp() - 60),
                    .details = "Transaction submitted to destination chain",
                },
            },
            .error = null,
        };

        return try protobuf.encode(context.allocator, response);
    }

    fn getSupportedChains(context: *grpc.Context) ![]const u8 {
        _ = context.request_data;
        
        std.log.info("GetSupportedChains request", .{});

        // Mock response - return supported chains
        const response = protobuf.SupportedChainsResponse{
            .chains = &[_]protobuf.ChainInfo{
                .{
                    .chain = .ghostchain,
                    .name = "Ghostchain",
                    .network = "mainnet",
                    .supported_assets = &[_][]const u8{ "GHOST", "USDC" },
                    .native_currency = "GHOST",
                    .block_time = 5,
                    .confirmations_required = 6,
                    .is_active = true,
                },
                .{
                    .chain = .ethereum,
                    .name = "Ethereum",
                    .network = "mainnet",
                    .supported_assets = &[_][]const u8{ "ETH", "USDC", "USDT", "DAI" },
                    .native_currency = "ETH",
                    .block_time = 12,
                    .confirmations_required = 12,
                    .is_active = true,
                },
                .{
                    .chain = .stellar,
                    .name = "Stellar",
                    .network = "mainnet",
                    .supported_assets = &[_][]const u8{ "XLM", "USDC", "USDT" },
                    .native_currency = "XLM",
                    .block_time = 5,
                    .confirmations_required = 1,
                    .is_active = true,
                },
            },
        };

        return try protobuf.encode(context.allocator, response);
    }

    fn estimateTransferFee(context: *grpc.Context) ![]const u8 {
        const request = try protobuf.decode(context.allocator, protobuf.TransferFeeRequest, context.request_data);
        defer request.deinit();

        std.log.info("EstimateTransferFee request: {s} -> {s} amount={s} asset={s}", .{ 
            @tagName(request.source_chain),
            @tagName(request.destination_chain),
            request.amount,
            request.asset_code,
        });

        // Mock response - return fee estimate
        const response = protobuf.TransferFeeResponse{
            .bridge_fee = "0.01", // 1% bridge fee
            .source_network_fee = "0.005", // Source chain fee
            .destination_network_fee = "0.001", // Destination chain fee
            .total_fee = "0.016", // Total fee
            .fee_currency = request.asset_code,
            .error = null,
        };

        return try protobuf.encode(context.allocator, response);
    }

    fn generateTransferId(allocator: std.mem.Allocator) ![]const u8 {
        const timestamp = std.time.timestamp();
        const random = std.crypto.random.int(u32);
        
        return try std.fmt.allocPrint(allocator, "transfer_{d}_{d}", .{ timestamp, random });
    }

    // Streaming RPC implementations
    fn subscribeTransferEvents(context: *grpc.Context) ![]const u8 {
        const request = try protobuf.decode(context.allocator, protobuf.TransferEventFilter, context.request_data);
        defer request.deinit();

        std.log.info("SubscribeTransferEvents stream started with filter", .{});

        // Mock streaming response - return a test transfer event
        const response = protobuf.CrossChainTransferEvent{
            .transfer_id = "transfer_1234567890_987654321",
            .event_type = "confirmed",
            .chain = .ethereum,
            .transaction_hash = "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef",
            .block_number = "0x12345",
            .timestamp = @intCast(std.time.timestamp()),
            .details = std.HashMap([]const u8, []const u8).init(context.allocator),
            .error = null,
        };

        return try protobuf.encode(context.allocator, response);
    }
};