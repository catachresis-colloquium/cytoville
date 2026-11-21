// SPDX-License-Identifier: AGPL-3.0-or-later
//! Offline command/save timings; no renderer, public listener or user save access.
use cytoville_core::{Command, Content, turn};
use cytoville_runtime::{RivalSettings, Runtime};
use serde::Serialize;
use std::{collections::BTreeMap, path::Path, time::Instant};

#[derive(Default, Serialize)]
struct Samples(BTreeMap<&'static str, Vec<f64>>);
impl Samples {
    fn measure<T>(&mut self, name: &'static str, f: impl FnOnce() -> T) -> T {
        let start = Instant::now();
        let result = f();
        self.0
            .entry(name)
            .or_default()
            .push(start.elapsed().as_secs_f64() * 1000.);
        result
    }
}

fn step(
    runtime: &mut Runtime,
    command: &Command,
    path: &Path,
    samples: &mut Samples,
) -> Result<(), Box<dyn std::error::Error>> {
    let start = Instant::now();
    samples.measure("dispatch_ms", || runtime.dispatch(0, command))?;
    let bytes = samples.measure("serialize_ms", || serde_json::to_vec(runtime))?;
    let temp = path.with_extension("tmp");
    samples.measure("write_ms", || std::fs::write(&temp, &bytes))?;
    samples.measure("replace_ms", || std::fs::rename(&temp, path))?;
    samples
        .0
        .entry("command_and_save_ms")
        .or_default()
        .push(start.elapsed().as_secs_f64() * 1000.);
    // The same sequential write/rename semantics as the current client; this
    // is not a power-loss durability test. Validation/readback is outside timing.
    let saved = std::fs::read(path)?;
    if saved != bytes {
        return Err("Saved bytes differ from the acknowledged runtime".into());
    }
    let restored: Runtime = serde_json::from_slice(&saved)?;
    restored.validate()?;
    Ok(())
}

fn run(path: &Path, native: bool) -> Result<Samples, Box<dyn std::error::Error>> {
    let mut samples = Samples::default();
    for seed in [7, 29, 101] {
        let mut runtime = Runtime::new(
            Content::builtin(),
            "euglena",
            "full-cycle",
            "standard",
            seed,
            false,
        )?;
        runtime.configure_rival(RivalSettings {
            inject_weevil: native,
            ..Default::default()
        })?;
        for _ in 0..24 {
            if runtime.game.status != "playing" {
                break;
            }
            if let Some(id) = &runtime.game.pending_event {
                let event = runtime
                    .content
                    .events
                    .iter()
                    .find(|e| &e.id == id)
                    .ok_or("Missing pending event")?;
                let choice = event
                    .choices
                    .iter()
                    .find(|c| turn::choice_lock(&runtime.game.actors[0], c).is_none())
                    .ok_or("No legal event choice")?;
                let command = Command::Choose {
                    id: choice.id.clone(),
                };
                step(&mut runtime, &command, path, &mut samples)?;
            }
            if runtime.game.status == "playing" {
                step(&mut runtime, &Command::EndTurn, path, &mut samples)?;
            }
        }
    }
    Ok(samples)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = std::env::args_os()
        .nth(1)
        .ok_or("Usage: command_profile NEW-SCRATCH-DIRECTORY (must not exist)")?;
    let directory = Path::new(&directory);
    // Refuse existing directories so a mistyped path cannot overwrite a save.
    std::fs::create_dir(directory)?;
    let baseline = run(&directory.join("baseline.json"), false)?;
    let native = run(&directory.join("native.json"), true)?;
    let receipt = serde_json::json!({
        "schema": 1,
        "workload": "euglena/full-cycle/standard; seeds 7,29,101; up to 24 cycles; first legal event choice; save/readback after every accepted command",
        "scope": "offline CPU and write/rename timings; excludes rendering, input/display latency, Elixir and power-loss durability",
        "baseline": baseline,
        "native": native
    });
    serde_json::to_writer(std::io::stdout().lock(), &receipt)?;
    println!();
    Ok(())
}
