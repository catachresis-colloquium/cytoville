// SPDX-License-Identifier: AGPL-3.0-or-later
//! Opt-in CPU timings. Frame intervals include presentation/waiting, not GPU timestamps.
use std::{
    collections::BTreeMap,
    path::Path,
    sync::{Mutex, OnceLock},
    time::Instant,
};

static SAMPLES: OnceLock<Mutex<BTreeMap<&'static str, Vec<f64>>>> = OnceLock::new();
const LIMIT: usize = 6000;

pub fn enable() {
    let _ = SAMPLES.set(Mutex::new(BTreeMap::new()));
}

pub fn record(name: &'static str, milliseconds: f64) {
    if let Some(samples) = SAMPLES.get() {
        let mut samples = samples.lock().expect("performance samples");
        let values = samples.entry(name).or_default();
        if values.len() < LIMIT {
            values.push(milliseconds);
        }
    }
}

pub struct Span(&'static str, Option<Instant>);
impl Span {
    pub fn new(name: &'static str) -> Self {
        Self(name, SAMPLES.get().map(|_| Instant::now()))
    }
}
impl Drop for Span {
    fn drop(&mut self) {
        if let Some(start) = self.1 {
            record(self.0, start.elapsed().as_secs_f64() * 1000.0);
        }
    }
}

pub fn frame(
    mut previous: bevy::prelude::Local<Option<Instant>>,
    mut warmup: bevy::prelude::Local<u32>,
) {
    if SAMPLES.get().is_none() {
        return;
    }
    let now = Instant::now();
    if let Some(last) = previous.replace(now) {
        *warmup += 1;
        if *warmup > 120 {
            record(
                "frame_interval_ms",
                now.duration_since(last).as_secs_f64() * 1000.0,
            );
        }
    }
}

pub fn write(path: &Path) -> std::io::Result<()> {
    let Some(samples) = SAMPLES.get() else {
        return Ok(());
    };
    let mut summaries = BTreeMap::new();
    for (name, values) in samples.lock().expect("performance samples").iter() {
        if values.is_empty() {
            continue;
        }
        let mut sorted = values.clone();
        sorted.sort_by(f64::total_cmp);
        summaries.insert(
            *name,
            serde_json::json!({
                "count": sorted.len(), "median": sorted[sorted.len()/2],
                "p95": sorted[((sorted.len()-1) as f64 * 0.95).ceil() as usize],
                "max": sorted[sorted.len()-1]
            }),
        );
    }
    // BTreeMap preserves lexical key order for the estate's JSON profile.
    std::fs::write(path, serde_json::to_vec(&summaries)?)
}
