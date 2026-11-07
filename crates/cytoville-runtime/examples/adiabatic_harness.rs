// SPDX-License-Identifier: AGPL-3.0-or-later
//! Emits a reproducible local reference receipt. No hardware/cloud access.
use cytoville_core::{Content, rules};
use cytoville_runtime::{Runtime, research::run_local_harness};
fn main() -> Result<(), String> {
    let runtime = Runtime::new(
        Content::builtin(),
        "euglena",
        "fresh-daughter",
        "standard",
        7,
        false,
    )?;
    let costs = ["university", "solar", "detritus", "diner"].map(|district| {
        rules::travel_cost(&runtime.content, &runtime.game.actors[0], district)
            .expect("built-in district")
    });
    let receipt = run_local_harness(costs)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&receipt).map_err(|e| e.to_string())?
    );
    Ok(())
}
