// SPDX-License-Identifier: AGPL-3.0-or-later
#[cfg(creusot)]
use creusot_std::prelude::*;

/// Used by every action and travel payment; rejection cannot underflow.
#[cfg_attr(creusot, ensures(match result { Some(left) => cost@ <= available@ && left@ + cost@ == available@, None => cost@ > available@ }))]
pub fn spend(available: u32, cost: u32) -> Option<u32> {
    if cost <= available {
        Some(available - cost)
    } else {
        None
    }
}

/// Conserves the available resource while capping a requested withdrawal.
#[cfg_attr(creusot, ensures(result.0@ + result.1@ == available@))]
#[cfg_attr(creusot, ensures(result.1@ <= requested@ && result.1@ <= available@))]
pub fn take_up_to(available: u32, requested: u32) -> (u32, u32) {
    let taken = if requested > available {
        available
    } else {
        requested
    };
    (available - taken, taken)
}

#[cfg_attr(creusot, requires(seat@ < 2))]
#[cfg_attr(creusot, ensures(result@ < 2 && result != seat))]
pub fn other_seat(seat: u32) -> u32 {
    1 - seat
}
