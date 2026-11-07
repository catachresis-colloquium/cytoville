// SPDX-License-Identifier: AGPL-3.0-or-later
//! The complete authoritative host shared by Bevy and the Elixir port worker.
use cytoville_core::{Action, Command, Content, Game, actions, apply, rules, turn};
use cytoville_native::{Memory, Observation, decide};
use enaction_trace::{Domain, Revision, Trace, TraceEvent, Trope, TropeId};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Runtime {
    pub content: Content,
    pub game: Game,
    pub memory: Memory,
    pub ai_tick: u64,
    pub trace: Trace,
}

pub const MAX_TRACE_EVENTS: usize = 100_000;
pub const MAX_NATIVE_TICKS: u64 = 1_000_000;

/// Mutable session state; immutable content is delivered during the handshake.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RuntimeState {
    pub game: Game,
    pub memory: Memory,
    pub ai_tick: u64,
    pub trace: Trace,
}

impl RuntimeState {
    pub fn validate(&self, content: &Content) -> Result<(), String> {
        validate_state(content, &self.game, self.ai_tick, &self.trace)
    }
}

pub fn content_id(content: &Content) -> Result<String, String> {
    let value = serde_json::to_value(content).map_err(|e| e.to_string())?;
    Ok(cytoville_core::package::Package::compile(value)?.content_sha256)
}

fn validate_state(
    content: &Content,
    game: &Game,
    ai_tick: u64,
    trace: &Trace,
) -> Result<(), String> {
    if trace.events.len() > MAX_TRACE_EVENTS {
        return Err(format!(
            "Session trace capacity exceeded ({MAX_TRACE_EVENTS} events); export the last acknowledged state"
        ));
    }
    if ai_tick > MAX_NATIVE_TICKS {
        return Err(format!(
            "Session native-tick capacity exceeded ({MAX_NATIVE_TICKS}); export the last acknowledged state"
        ));
    }
    content.validate()?;
    game.validate(content)?;
    trace.validate().map_err(|e| e.join("; "))?;
    Ok(())
}

impl Runtime {
    pub fn new(
        content: Content,
        specimen: &str,
        scenario: &str,
        difficulty: &str,
        seed: u32,
        multiplayer: bool,
    ) -> Result<Self, String> {
        let game = turn::new_game(&content, specimen, scenario, difficulty, seed, multiplayer)?;
        Ok(Self {
            content,
            game,
            memory: Memory {
                rng_state: seed as u64,
                ..Default::default()
            },
            ai_tick: 0,
            trace: Trace::default(),
        })
    }
    pub fn dispatch(&mut self, seat: usize, command: &Command) -> Result<(), String> {
        let mut next = self.clone();
        next.game = apply(&next.content, &next.game, seat, command)?;
        // Record accepted human commands too: multiplayer has no native rival
        // to produce the causal trace on its behalf.
        let cause = next.trace.events.last().map(|e| e.id).into_iter().collect();
        next.record(
            Domain::BehaviouralSelection,
            &serde_json::to_string(command).map_err(|e| e.to_string())?,
            seat as i32,
            cause,
        );
        next.trace
            .tropes
            .last_mut()
            .ok_or("Missing command trace")?
            .bearer = format!("cell-{seat}");
        if !next.game.multiplayer && next.game.active == 1 && next.game.status == "playing" {
            next.run_rival()?;
        }
        next.validate()?;
        *self = next;
        Ok(())
    }
    pub fn validate(&self) -> Result<(), String> {
        validate_state(&self.content, &self.game, self.ai_tick, &self.trace)
    }
    pub fn state(&self) -> RuntimeState {
        RuntimeState {
            game: self.game.clone(),
            memory: self.memory,
            ai_tick: self.ai_tick,
            trace: self.trace.clone(),
        }
    }
    pub fn into_state(self) -> RuntimeState {
        RuntimeState {
            game: self.game,
            memory: self.memory,
            ai_tick: self.ai_tick,
            trace: self.trace,
        }
    }
    pub fn apply_state(&mut self, state: RuntimeState) -> Result<(), String> {
        state.validate(&self.content)?;
        self.game = state.game;
        self.memory = state.memory;
        self.ai_tick = state.ai_tick;
        self.trace = state.trace;
        Ok(())
    }
    fn record(&mut self, domain: Domain, kind: &str, value: i32, cause: Vec<u64>) -> u64 {
        let id = self.trace.events.len() as u64 + 1;
        self.trace.tropes.push(Trope {
            id: TropeId(id),
            holders: vec![],
            bearer: "rival-cell".into(),
            about: Some("biofilm-metropolis".into()),
            kind: kind.into(),
            domain,
        });
        self.trace.events.push(TraceEvent {
            id,
            tick: self.game.revision,
            trope: TropeId(id),
            value_milli: value,
            confidence: None,
            revision: Revision::Assert,
            caused_by: cause,
        });
        id
    }
    fn run_rival(&mut self) -> Result<(), String> {
        for _ in 0..80 {
            if self.game.status != "playing" {
                break;
            }
            let a = &self.game.actors[1];
            let other = &self.game.actors[0];
            self.ai_tick += 1;
            let near = a.location == other.location || a.location == "rift";
            let loc = &self.content.locations[&a.location];
            let obs = Observation {
                tick: self.ai_tick,
                seed: self.game.rng_state as u64,
                health: ((100.0 - a.stress) / 100.0 * 65536.0).round() as i32,
                energy: if a.nutrition < 40.0 { 0 } else { 65536 },
                threat: if self.game.relationship < 0 {
                    65536
                } else {
                    32768
                },
                distance: if near { 32768 } else { 196608 },
                x: (loc.x * 65536.0) as i32,
                y: (loc.y * 65536.0) as i32,
                contact: u32::from(near),
                reserved: 0,
            };
            let intent = decide(&obs, &mut self.memory)?;
            let stress_event = self.record(
                Domain::Affective,
                "oxidative-distress",
                (self.game.actors[1].stress * 10.0) as i32,
                self.trace.events.last().map(|e| e.id).into_iter().collect(),
            );
            let motive = self.record(
                Domain::Conative,
                "weevil-intent",
                intent.action as i32,
                vec![stress_event],
            );
            let Some((location, action)) = self.select(intent.action) else {
                break;
            };
            self.record(Domain::BehaviouralSelection, &action.id, 1000, vec![motive]);
            if self.game.actors[1].location != location {
                self.game = apply(
                    &self.content,
                    &self.game,
                    1,
                    &Command::Travel {
                        location: location.clone(),
                    },
                )?;
                self.game.rival_trail.push(location);
            }
            self.game = apply(
                &self.content,
                &self.game,
                1,
                &Command::Act {
                    id: action.id.clone(),
                },
            )?;
        }
        if self.game.status == "playing" {
            self.game = apply(&self.content, &self.game, 1, &Command::EndTurn)?;
        }
        Ok(())
    }
    pub fn select(&self, intent: u32) -> Option<(String, Action)> {
        let a = &self.game.actors[1];
        let scenario = &self.content.scenarios[&self.game.scenario_id];
        let remaining: Vec<_> = scenario
            .quests
            .iter()
            .filter(|q| !a.done.contains(&q.id))
            .collect();
        let need_t = remaining
            .iter()
            .filter(|q| q.kind == "transcription")
            .map(|q| q.target as u32)
            .max()
            .unwrap_or(0)
            .max(if remaining.iter().any(|q| q.kind == "career") {
                3
            } else {
                0
            });
        let mut best: Option<(f64, String, Action)> = None;
        for loc in &self.content.location_order {
            let travel = rules::travel_cost(&self.content, a, loc)?;
            for action in actions::list(&self.content, &self.game, 1, loc) {
                if !action.enabled() || action.atp + travel > a.atp {
                    continue;
                }
                let mut value = match action.tag.as_str() {
                    "food" => {
                        if a.nutrition < 65.0 {
                            action.value.min(100.0 - a.nutrition) * 2.6
                        } else {
                            0.0
                        }
                    }
                    "work" => {
                        if a.biomass < 220 {
                            action.value * 1.7
                        } else if remaining.iter().any(|q| q.kind == "earned") {
                            action.value * 0.7
                        } else {
                            action.value * 0.25
                        }
                    }
                    "study" => {
                        if a.transcription < need_t {
                            100.0
                        } else {
                            0.0
                        }
                    }
                    "promote" => {
                        if remaining.iter().any(|q| q.kind == "career") {
                            200.0
                        } else {
                            60.0
                        }
                    }
                    "rest" | "detox" => {
                        if a.stress > 30.0 {
                            action.value.max(14.0) * a.stress / 30.0
                        } else {
                            0.0
                        }
                    }
                    "checkpoint" => {
                        if remaining
                            .iter()
                            .any(|q| q.kind == "phase" || q.kind == "mitosis")
                        {
                            action.value * 2.0
                        } else {
                            0.0
                        }
                    }
                    "synth" => {
                        let organelle = action.id.strip_prefix("synth:").unwrap_or("");
                        if organelle == "vacuole" || organelle == "storage" {
                            160.0
                        } else if remaining
                            .iter()
                            .any(|q| q.kind == "organelle" && q.param.as_deref() == Some(organelle))
                        {
                            200.0
                        } else if remaining.iter().any(|q| q.kind == "signature")
                            && self.content.specimens[&a.specimen]
                                .signature
                                .iter()
                                .any(|x| x == organelle)
                        {
                            180.0
                        } else if remaining.iter().any(|q| q.kind == "organelles") {
                            100.0
                        } else {
                            0.0
                        }
                    }
                    "stock" => {
                        if a.storage_stock < 2 {
                            80.0
                        } else {
                            0.0
                        }
                    }
                    "train" if a.motility < 15.0 && action.id == "gym:motility" => 60.0,
                    _ => 0.0,
                };
                // Native intents change the actual selected command, while host
                // legality, costs and biological ontology remain authoritative.
                if intent == 5 && action.tag == "food" {
                    value *= 3.0;
                }
                if intent == 3 && (action.tag == "rest" || action.tag == "detox") {
                    value *= 3.0;
                }
                if intent == 4
                    && ["rift:prey", "rift:exclude"].contains(&action.id.as_str())
                    && self.game.truce_turns == 0
                {
                    value += 90.0;
                }
                let score = value / (action.atp + travel + 2) as f64;
                if score > 0.01 && best.as_ref().is_none_or(|b| score > b.0) {
                    best = Some((score, loc.clone(), action));
                }
            }
        }
        best.map(|(_, l, a)| (l, a))
    }
}

#[cfg(test)]
mod capacity_tests {
    use super::*;

    #[test]
    fn trace_budget_accepts_the_boundary_then_rejects_atomically_without_losing_history() {
        let mut runtime = Runtime::new(
            Content::builtin(),
            "euglena",
            "fresh-daughter",
            "standard",
            7,
            true,
        )
        .unwrap();
        for _ in 0..MAX_TRACE_EVENTS - 1 {
            runtime.record(Domain::BehaviouralSelection, "capacity-fixture", 0, vec![]);
        }
        runtime.dispatch(0, &Command::EndTurn).unwrap();
        assert_eq!(runtime.trace.events.len(), MAX_TRACE_EVENTS);
        let before = runtime.game.clone();
        let last = runtime.trace.events.last().unwrap().clone();
        let error = runtime.dispatch(1, &Command::EndTurn).unwrap_err();
        assert!(error.contains("trace capacity"), "{error}");
        assert_eq!(runtime.game, before);
        assert_eq!(runtime.trace.events.len(), MAX_TRACE_EVENTS);
        assert_eq!(runtime.trace.events.last(), Some(&last));
        runtime.validate().unwrap();
        serde_json::to_writer(std::io::sink(), &runtime).unwrap();
    }
}
