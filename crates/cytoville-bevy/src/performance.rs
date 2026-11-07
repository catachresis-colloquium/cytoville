// SPDX-License-Identifier: AGPL-3.0-or-later
//! Opt-in CPU timings. Frame intervals include presentation/waiting, not GPU timestamps.
use std::{
    collections::BTreeMap,
    path::Path,
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

static SAMPLES: OnceLock<Mutex<BTreeMap<&'static str, Vec<f64>>>> = OnceLock::new();
static INVALID: AtomicBool = AtomicBool::new(false);
const LIMIT: usize = 6000;

pub fn enable() {
    let _ = SAMPLES.set(Mutex::new(BTreeMap::new()));
}

pub fn record(name: &'static str, milliseconds: f64) {
    if let Some(samples) = SAMPLES.get() {
        let Ok(mut samples) = samples.lock() else {
            // Optional diagnostics must not panic in the render/command path.
            // Refuse the receipt at exit rather than silently reporting partial data.
            INVALID.store(true, Ordering::Relaxed);
            return;
        };
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
    if INVALID.load(Ordering::Relaxed) {
        return Err(std::io::Error::other(
            "performance sample collection failed; no receipt written",
        ));
    }
    let mut summaries = BTreeMap::new();
    let samples = samples.lock().map_err(|_| {
        std::io::Error::other("performance samples unavailable; no receipt written")
    })?;
    for (name, values) in samples.iter() {
        if let Some(summary) = summary(values) {
            summaries.insert(*name, summary);
        }
    }
    // BTreeMap preserves lexical key order for the estate's JSON profile.
    std::fs::write(path, serde_json::to_vec(&summaries)?)
}

fn summary(values: &[f64]) -> Option<serde_json::Value> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let n = sorted.len();
    let middle = n / 2;
    let median = if n.is_multiple_of(2) {
        sorted[middle - 1] * 0.5 + sorted[middle] * 0.5
    } else {
        sorted[middle]
    };
    // Nearest-rank percentile: ceil(0.95 * n), converted to a zero-based index.
    let p95 = sorted[(n * 95).div_ceil(100) - 1];
    Some(serde_json::json!({"count": n, "max": sorted[n-1], "median": median, "p95": p95}))
}

#[cfg(test)]
mod tests {
    use super::summary;
    #[test]
    fn even_sample_median_and_nearest_rank_percentile() {
        let receipt = summary(&(1..=20).rev().map(f64::from).collect::<Vec<_>>()).unwrap();
        assert_eq!(receipt["median"], 10.5);
        assert_eq!(receipt["p95"], 19.0);
        assert_eq!(receipt["count"], 20);
    }
    #[test]
    fn empty_singleton_and_odd_samples_are_well_defined() {
        assert!(summary(&[]).is_none());
        let singleton = summary(&[7.0]).unwrap();
        assert_eq!(singleton["median"], 7.0);
        assert_eq!(singleton["p95"], 7.0);
        assert_eq!(summary(&[9.0, 1.0, 5.0]).unwrap()["median"], 5.0);
    }
}
