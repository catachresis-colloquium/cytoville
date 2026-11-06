/* SPDX-License-Identifier: AGPL-3.0-or-later */
/* C API v1. Size constants are checked against Idris-generated Rust/Zig. */
#ifndef CYTOVILLE_H
#define CYTOVILLE_H
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif
typedef struct {
  uint64_t tick, seed;
  int32_t health, energy, threat, distance, x, y;
  uint32_t contact, reserved;
} CytovilleObservation;
typedef struct {
  uint64_t rng_state, tick_last;
  int32_t memory_x, memory_y;
  uint32_t memory_age, cached_action, cached_target, budget_used, degraded_ticks, flags;
} CytovilleMemory;
typedef struct { uint32_t action, target, flags, budget; } CytovilleIntent;
uint32_t cytoville_abi_version(void);
/* Q16 health/energy/threat in [0,65536], distance >=0, contact in {0,1}.
 * Caller owns aligned, distinct live buffers; no pointer is retained.
 * 0 = success, 1 = bad parameter; remaining errors propagate from Evil Weevil. */
uint32_t cytoville_decide(const CytovilleObservation *, CytovilleMemory *, CytovilleIntent *);
#ifdef __cplusplus
}
#endif
#endif
