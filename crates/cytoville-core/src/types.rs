// SPDX-License-Identifier: AGPL-3.0-or-later
use crate::Content;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Career {
    pub level: u32,
    pub exp: u32,
    pub shifts: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Actor {
    pub name: String,
    pub specimen: String,
    pub location: String,
    pub atp: u32,
    pub atp_max: u32,
    pub biomass: u32,
    pub total_earned: u32,
    pub nutrition: f64,
    pub stress: f64,
    pub size: f64,
    pub phase: String,
    pub s_progress: u32,
    pub transcription: u32,
    pub course_progress: BTreeMap<String, u32>,
    pub courses: Vec<String>,
    pub motility: f64,
    pub integrity: f64,
    pub signaling: f64,
    pub organelles: Vec<String>,
    pub synthesized: u32,
    pub storage_stock: u32,
    pub careers: BTreeMap<String, Career>,
    pub phenotype: String,
    pub done: Vec<String>,
    pub divided: bool,
    pub pending_encyst: bool,
    pub encystments: u32,
    pub hazards_blocked: u32,
    pub homeo_streak: u32,
    pub spa_used: bool,
}
impl Actor {
    pub fn has(&self, organelle: &str) -> bool {
        self.organelles.iter().any(|o| o == organelle)
    }
    pub fn armor(&self) -> bool {
        ["pellicle", "trichocyst", "myoneme", "frustule"]
            .iter()
            .any(|o| self.has(o))
    }
    pub fn phagocytic(&self) -> bool {
        self.has("oral") || self.has("pseudopod")
    }
    pub fn course_count(&self, route: &str) -> usize {
        self.courses.iter().filter(|c| c.starts_with(route)).count()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Look {
    pub name: String,
    pub membrane: String,
    pub accent: String,
    pub pattern: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Game {
    pub format: String,
    pub scenario_id: String,
    pub difficulty: String,
    pub turn: u32,
    pub rng_state: u32,
    pub weather: String,
    pub forecast: String,
    pub actors: [Actor; 2],
    pub looks: [Look; 2],
    pub active: usize,
    pub multiplayer: bool,
    pub revision: u64,
    pub status: String,
    pub winner: Option<usize>,
    pub log: Vec<String>,
    pub report: Vec<String>,
    pub rival_trail: Vec<String>,
    pub pending_event: Option<String>,
    pub event_history: Vec<String>,
    pub relationship: i32,
    pub deterrence: u32,
    pub truce_turns: u32,
    pub intel: bool,
}
impl Game {
    pub fn message(&mut self, text: impl Into<String>) {
        self.log.push(text.into());
        if self.log.len() > 120 {
            self.log.remove(0);
        }
    }
    pub fn validate(&self, c: &Content) -> Result<(), String> {
        if self.format != "cytoville-save/1"
            || self.active > 1
            || !c.scenarios.contains_key(&self.scenario_id)
            || !c.difficulty.contains_key(&self.difficulty)
            || !c.weather.contains_key(&self.weather)
            || !c.weather.contains_key(&self.forecast)
            || self.turn == 0
            || self.turn > 10000
            || !["playing", "won", "lost", "draw"].contains(&self.status.as_str())
            || self.winner.is_some_and(|s| s > 1)
            || !(-100..=100).contains(&self.relationship)
            || self.truce_turns > 10000
            || self.deterrence > 10000
            || self.log.len() > 120
            || self.report.len() > 120
            || self.event_history.len() > 20000
            || self
                .pending_event
                .as_ref()
                .is_some_and(|id| !c.events.iter().any(|e| &e.id == id))
        {
            return Err("Invalid save header".into());
        }
        for a in &self.actors {
            if !c.specimens.contains_key(&a.specimen)
                || !c.locations.contains_key(&a.location)
                || !["G1", "S", "G2", "M"].contains(&a.phase.as_str())
                || a.transcription > 4
                || a.s_progress > 3
                || a.storage_stock > 6
                || a.atp > 1000
                || a.atp_max > 1000
                || !["balanced", "photo", "hetero"].contains(&a.phenotype.as_str())
                || a.organelles.iter().any(|id| !c.organelles.contains_key(id))
                || a.courses
                    .iter()
                    .any(|id| !c.all_courses.iter().any(|course| &course.id == id))
                || ["solar", "detritus", "bacterivore"]
                    .iter()
                    .any(|id| !a.careers.contains_key(*id))
                || a.careers.values().any(|p| p.level > 4)
                || a.careers
                    .values()
                    .any(|p| p.exp > 1_000_000 || p.shifts > 1_000_000)
                || [
                    a.synthesized,
                    a.encystments,
                    a.hazards_blocked,
                    a.homeo_streak,
                ]
                .iter()
                .any(|v| *v > 1_000_000)
                || a.course_progress.iter().any(|(id, n)| {
                    !c.all_courses
                        .iter()
                        .any(|course| &course.id == id && *n <= course.sessions)
                })
                || [
                    a.nutrition,
                    a.stress,
                    a.size,
                    a.motility,
                    a.integrity,
                    a.signaling,
                ]
                .iter()
                .any(|x| !x.is_finite() || !(0.0..=100.0).contains(x))
            {
                return Err("Invalid actor state".into());
            }
        }
        for look in &self.looks {
            if look.name.chars().count() > 40
                || !["plain", "spots", "stripes", "glow"].contains(&look.pattern.as_str())
                || [&look.membrane, &look.accent].iter().any(|color| {
                    color.len() != 7
                        || !color.starts_with('#')
                        || !color[1..].bytes().all(|b| b.is_ascii_hexdigit())
                })
            {
                return Err("Invalid cell appearance".into());
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    Travel {
        location: String,
    },
    Act {
        id: String,
    },
    Choose {
        id: String,
    },
    EndTurn,
    Customize {
        name: String,
        membrane: String,
        accent: String,
        pattern: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Action {
    pub id: String,
    pub label: String,
    pub description: String,
    pub tag: String,
    pub location: String,
    pub atp: u32,
    pub biomass: u32,
    pub reason: Option<String>,
    pub value: f64,
}
impl Action {
    pub fn enabled(&self) -> bool {
        self.reason.is_none()
    }
}
