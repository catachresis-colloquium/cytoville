// SPDX-License-Identifier: AGPL-3.0-or-later
const std = @import("std");
const ee = @import("ee");
const abi = @import("abi");
const layout = @import("cyto_layout");
const offsets = @import("cyto_offsets");
const Observation = extern struct { tick: u64, seed: u64, health: i32, energy: i32, threat: i32, distance: i32, x: i32, y: i32, contact: u32, reserved: u32 };
const Intent = extern struct { action: u32, target: u32, flags: u32, budget: u32 };
comptime {
    if (@sizeOf(Observation) != layout.observation_size or @sizeOf(abi.EeAgent) != layout.memory_size or @sizeOf(Intent) != layout.intent_size) @compileError("Idris ABI size mismatch");
    if (@offsetOf(Observation, "health") != offsets.health_offset or @offsetOf(Observation, "contact") != offsets.contact_offset or @offsetOf(abi.EeAgent, "cached_action") != offsets.action_offset or @offsetOf(Intent, "target") != offsets.target_offset) @compileError("Idris ABI offset mismatch");
}
pub export fn cytoville_abi_version() u32 {
    return layout.abi_version;
}
pub export fn cytoville_decide(obs: ?*const Observation, memory: ?*abi.EeAgent, out: ?*Intent) u32 {
    const o = obs orelse return abi.STATUS_BAD_PARAM;
    const m = memory orelse return abi.STATUS_BAD_PARAM;
    const result = out orelse return abi.STATUS_BAD_PARAM;
    if (o.reserved != 0 or o.contact > 1 or o.health < 0 or o.health > 65536 or o.energy < 0 or o.energy > 65536 or o.distance < 0 or o.threat < 0 or o.threat > 65536) return abi.STATUS_BAD_PARAM;
    var ctx: abi.EeContext = undefined;
    const init: abi.EeInitDesc = .{ .struct_size = @sizeOf(abi.EeInitDesc), .abi_major = abi.ABI_MAJOR, .abi_minor = abi.ABI_MINOR, .capabilities = abi.CAP_LINE_OF_SIGHT | abi.CAP_DAMAGE_EVENTS | abi.CAP_WAYPOINT_GRAPH, .max_agents = 1, .flags = 0, .host_tick_num = 1, .host_tick_den = 1, .rng_seed = o.seed, .abi_fingerprint = abi.ABI_FINGERPRINT, .reserved0 = 0 };
    const status = ee.ee_init(&init, &ctx);
    if (status != 0) return status;
    var snap = std.mem.zeroes(abi.EeSnapshot);
    snap.tick = o.tick;
    snap.capability_mask = init.capabilities;
    snap.agent_id = 1;
    snap.self_x = o.x;
    snap.self_y = o.y;
    snap.health = o.health;
    snap.ammo = o.energy;
    snap.contact_count = @intCast(o.contact);
    snap.contact0 = .{ .id = 2, .flags = 0, .bearing_cos = 32767, .bearing_sin = 0, .distance = o.distance, .threat = o.threat };
    var intent: abi.EeIntent = undefined;
    const code = ee.ee_tick(&ctx, &snap, m, &intent);
    if (code != 0) return code;
    result.* = .{ .action = intent.action, .target = intent.target_id, .flags = intent.flags, .budget = intent.budget_used };
    return 0;
}
test "null pointers rejected" {
    try std.testing.expectEqual(@as(u32, 1), cytoville_decide(null, null, null));
}
test "real kernel distinguishes starvation and a nearby rival" {
    var m = std.mem.zeroes(abi.EeAgent);
    var out: Intent = undefined;
    var o = Observation{ .tick = 1, .seed = 7, .health = 65536, .energy = 0, .threat = 65536, .distance = 32768, .x = 0, .y = 0, .contact = 1, .reserved = 0 };
    try std.testing.expectEqual(@as(u32, 0), cytoville_decide(&o, &m, &out));
    try std.testing.expectEqual(abi.ACTION_RELOAD, out.action);
    o.tick = 2;
    o.energy = 65536;
    try std.testing.expectEqual(@as(u32, 0), cytoville_decide(&o, &m, &out));
    try std.testing.expectEqual(abi.ACTION_ATTACK, out.action);
}
