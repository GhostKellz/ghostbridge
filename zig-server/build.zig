const std = @import("std");

pub fn build(b: *std.Build) void {
    const target = b.standardTargetOptions(.{});
    const optimize = b.standardOptimizeOption(.{});

    const zquic = b.dependency("zquic", .{
        .target = target,
        .optimize = optimize,
    });

    // TODO: Re-enable when dependencies are properly configured
    // const tokioz = b.dependency("tokioz", .{
    //     .target = target,
    //     .optimize = optimize,
    // });

    // const realid = b.dependency("realid", .{
    //     .target = target,
    //     .optimize = optimize,
    // });

    const exe = b.addExecutable(.{
        .name = "ghostbridge",
        .root_source_file = b.path("src/main.zig"),
        .target = target,
        .optimize = optimize,
    });

    // Add working dependencies
    exe.root_module.addImport("zquic", zquic.module("zquic"));
    
    // TODO: Add proper TokioZ and realID modules when dependencies are fixed
    // For now, we'll use local stubs to maintain compilation

    b.installArtifact(exe);

    const run_cmd = b.addRunArtifact(exe);
    run_cmd.step.dependOn(b.getInstallStep());

    if (b.args) |args| {
        run_cmd.addArgs(args);
    }

    const run_step = b.step("run", "Run the ghostbridge server");
    run_step.dependOn(&run_cmd.step);

    const test_step = b.step("test", "Run unit tests");
    const tests = b.addTest(.{
        .root_source_file = b.path("src/main.zig"),
        .target = target,
        .optimize = optimize,
    });
    test_step.dependOn(&tests.step);
}