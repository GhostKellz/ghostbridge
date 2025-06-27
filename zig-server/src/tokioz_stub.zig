/// TokioZ stub for compilation
/// Will be replaced with actual TokioZ import when dependency is fixed

const std = @import("std");

pub const runtime = struct {
    pub fn run(func: anytype) !void {
        // For now, just execute synchronously
        try func();
    }
};

pub fn spawn(func: anytype) void {
    // For now, just execute synchronously
    func() catch |err| {
        std.log.err("Async task failed: {}", .{err});
    };
}

pub const time = struct {
    pub const Duration = struct {
        millis: u64,
        
        pub fn fromMillis(ms: u64) Duration {
            return .{ .millis = ms };
        }
    };
    
    pub fn sleep(duration: Duration) !void {
        std.time.sleep(duration.millis * 1_000_000);
    }
};