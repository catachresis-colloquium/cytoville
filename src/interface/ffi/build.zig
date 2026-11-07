// SPDX-License-Identifier: AGPL-3.0-or-later
const std = @import("std");
pub fn build(b: *std.Build) void {
    const target = b.standardTargetOptions(.{});
    const optimize = b.standardOptimizeOption(.{});
    const layout = b.createModule(.{ .root_source_file = b.path("../../../external/evil-weevil/src/interface/generated/ee_layout.zig") });
    const abi = b.createModule(.{ .root_source_file = b.path("../../../external/evil-weevil/src/interface/ffi/src/abi.zig"), .target = target, .optimize = optimize });
    abi.addImport("layout", layout);
    const kernel = b.createModule(.{ .root_source_file = b.path("../../../external/evil-weevil/src/core/kernel.zig"), .target = target, .optimize = optimize });
    kernel.addImport("abi", abi);
    const ee = b.createModule(.{ .root_source_file = b.path("../../../external/evil-weevil/src/interface/ffi/src/main.zig"), .target = target, .optimize = optimize });
    ee.addImport("abi", abi);
    ee.addImport("kernel", kernel);
    const own_layout = b.createModule(.{ .root_source_file = b.path("../generated/layout.zig") });
    const root = b.createModule(.{ .root_source_file = b.path("src/main.zig"), .target = target, .optimize = optimize, .pic = true });
    root.addImport("ee", ee);
    root.addImport("abi", abi);
    root.addImport("cyto_layout", own_layout);
    root.addImport("cyto_offsets", b.createModule(.{ .root_source_file = b.path("../generated/offsets.zig") }));
    const lib = b.addLibrary(.{ .name = "cytoville_native", .linkage = .static, .root_module = root });
    lib.bundle_compiler_rt = b.option(bool, "bundle-compiler-rt", "Include compiler runtime in the static archive") orelse true;
    b.installArtifact(lib);
    const tests = b.addTest(.{ .root_module = root });
    b.step("test", "Run boundary and real-kernel tests").dependOn(&b.addRunArtifact(tests).step);
}
