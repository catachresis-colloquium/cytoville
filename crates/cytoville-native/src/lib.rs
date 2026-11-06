// SPDX-License-Identifier: AGPL-3.0-or-later
//! Safe, typed owner of the Zig API. Native memory is explicit and serializable.
use serde::{Deserialize, Serialize};
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Memory {
    pub rng_state: u64,
    pub tick_last: u64,
    pub memory_x: i32,
    pub memory_y: i32,
    pub memory_age: u32,
    pub cached_action: u32,
    pub cached_target: u32,
    pub budget_used: u32,
    pub degraded_ticks: u32,
    pub flags: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Observation {
    pub tick: u64,
    pub seed: u64,
    pub health: i32,
    pub energy: i32,
    pub threat: i32,
    pub distance: i32,
    pub x: i32,
    pub y: i32,
    pub contact: u32,
    pub reserved: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Intent {
    pub action: u32,
    pub target: u32,
    pub flags: u32,
    pub budget: u32,
}
include!("../../../src/interface/generated/layout.rs");
include!("../../../src/interface/generated/offsets.rs");
const _: () = {
    assert!(std::mem::offset_of!(Observation, health) == HEALTH_OFFSET);
    assert!(std::mem::offset_of!(Observation, contact) == CONTACT_OFFSET);
    assert!(std::mem::offset_of!(Memory, cached_action) == ACTION_OFFSET);
    assert!(std::mem::offset_of!(Intent, target) == TARGET_OFFSET);
};
const _: () = {
    assert!(size_of::<Observation>() == OBSERVATION_SIZE);
    assert!(size_of::<Memory>() == MEMORY_SIZE);
    assert!(size_of::<Intent>() == INTENT_SIZE);
};
unsafe extern "C" {
    fn cytoville_abi_version() -> u32;
    fn cytoville_decide(
        observation: *const Observation,
        memory: *mut Memory,
        intent: *mut Intent,
    ) -> u32;
}
pub fn decide(obs: &Observation, memory: &mut Memory) -> Result<Intent, String> {
    let mut out = Intent::default();
    // SAFETY: pointers reference live, aligned repr(C) values, remain borrowed
    // throughout the synchronous call, and the Zig boundary retains none.
    unsafe {
        if cytoville_abi_version() != ABI_VERSION {
            return Err("Cytoville ABI version mismatch".into());
        }
        let code = cytoville_decide(obs, memory, &mut out);
        if code != 0 {
            return Err(format!("Evil Weevil rejected snapshot: {code}"));
        }
    }
    Ok(out)
}
