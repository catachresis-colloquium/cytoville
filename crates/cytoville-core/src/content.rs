// SPDX-License-Identifier: AGPL-3.0-or-later
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub struct Content {
    pub specimens: BTreeMap<String, Specimen>,
    pub specimen_order: Vec<String>,
    pub locations: BTreeMap<String, Location>,
    pub location_order: Vec<String>,
    pub organelles: BTreeMap<String, Organelle>,
    pub careers: BTreeMap<String, CareerDef>,
    pub scenarios: BTreeMap<String, Scenario>,
    pub scenario_order: Vec<String>,
    pub weather: BTreeMap<String, Weather>,
    pub difficulty: BTreeMap<String, Difficulty>,
    pub routes: Vec<Route>,
    pub all_courses: Vec<Course>,
    pub events: Vec<Event>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Specimen {
    pub id: String,
    pub name: String,
    pub latin: String,
    pub title: String,
    pub blurb: String,
    pub start_organelles: Vec<String>,
    pub signature: Vec<String>,
    pub start_motility: f64,
    pub start_integrity: f64,
    pub start_signaling: f64,
    pub body: String,
    pub accent: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Location {
    pub id: String,
    pub name: String,
    pub short: String,
    pub x: f64,
    pub y: f64,
    pub blurb: String,
    pub color: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Organelle {
    pub id: String,
    pub name: String,
    pub category: String,
    pub cost: u32,
    pub atp: u32,
    pub min_transcription: u32,
    #[serde(default)]
    pub requires: Vec<String>,
    #[serde(default)]
    pub only: Vec<String>,
    #[serde(default)]
    pub min_size: f64,
    pub effect: String,
    pub science: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CareerDef {
    pub id: String,
    pub location: String,
    pub name: String,
    pub blurb: String,
    pub atp: u32,
    pub pay: Vec<f64>,
    pub stress: f64,
    pub nutrition: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Scenario {
    pub id: String,
    pub name: String,
    pub intro: String,
    pub turn_limit: u32,
    pub allow_mitosis: bool,
    pub start_size: f64,
    pub start_biomass: u32,
    pub rival_offset: usize,
    pub quests: Vec<Quest>,
    pub weather: BTreeMap<String, f64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Quest {
    pub id: String,
    pub label: String,
    pub hint: String,
    pub kind: String,
    pub target: f64,
    pub param: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Weather {
    pub id: String,
    pub name: String,
    pub blurb: String,
    pub pay: BTreeMap<String, f64>,
    pub diner_mult: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Difficulty {
    pub rival_atp: f64,
    pub raid: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Route {
    pub id: String,
    pub name: String,
    pub blurb: String,
    pub color: String,
    pub courses: Vec<Course>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Course {
    pub id: String,
    pub name: String,
    pub tier: u32,
    pub sessions: u32,
    pub atp: u32,
    pub biomass: u32,
    pub blurb: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Event {
    pub id: String,
    pub title: String,
    pub choices: Vec<Choice>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Choice {
    pub id: String,
    pub label: String,
    pub result: String,
    pub effects: BTreeMap<String, f64>,
    pub requires: Option<serde_json::Value>,
}

impl Content {
    pub fn builtin() -> Self {
        serde_json::from_str(include_str!("../../../assets/content/cytoville.json"))
            .expect("checked built-in content")
    }
    pub fn validate(&self) -> Result<(), String> {
        fn bounded(v: f64, max: f64) -> bool {
            v.is_finite() && (0.0..=max).contains(&v)
        }
        fn ordered<T>(order: &[String], map: &BTreeMap<String, T>) -> bool {
            !order.is_empty()
                && order
                    .iter()
                    .collect::<std::collections::BTreeSet<_>>()
                    .len()
                    == order.len()
                && order.iter().all(|id| map.contains_key(id))
        }
        if !ordered(&self.specimen_order, &self.specimens)
            || !ordered(&self.location_order, &self.locations)
            || !ordered(&self.scenario_order, &self.scenarios)
            || self.specimen_order.len() != self.specimens.len()
            || self.scenario_order.len() != self.scenarios.len()
            || self.specimens.len() > 64
            || self.scenarios.len() > 64
            || self.organelles.len() > 256
            || self.events.len() > 128
            || self.all_courses.len() > 128
        {
            return Err("Invalid content ordering or collection bounds".into());
        }
        for id in [
            "calm", "sunny", "uv", "bloom", "osmotic", "cloudy", "anoxic",
        ] {
            if !self.weather.contains_key(id) {
                return Err(format!("Missing weather {id}"));
            }
        }
        if self.weather.len() != 7
            || self.difficulty.len() != 3
            || ["gentle", "standard", "fierce"]
                .iter()
                .any(|id| !self.difficulty.contains_key(*id))
        {
            return Err("Unsupported weather or difficulty ontology".into());
        }
        if self.careers.len() != 3
            || ["solar", "detritus", "bacterivore"]
                .iter()
                .any(|id| !self.careers.contains_key(*id))
        {
            return Err("Invalid career ontology".into());
        }
        for (id, career) in &self.careers {
            if id != &career.id
                || !self.locations.contains_key(&career.location)
                || career.pay.len() != 4
                || career.pay.iter().any(|v| !bounded(*v, 10000.0))
                || career.atp == 0
                || career.atp > 500
                || !bounded(career.stress, 100.0)
                || !bounded(career.nutrition, 100.0)
            {
                return Err("Invalid career definition".into());
            }
        }
        for (id, w) in &self.weather {
            if id != &w.id
                || !bounded(w.diner_mult, 10.0)
                || self
                    .careers
                    .keys()
                    .any(|k| !w.pay.get(k).is_some_and(|v| bounded(*v, 10.0)))
            {
                return Err("Invalid weather multipliers".into());
            }
        }
        for d in self.difficulty.values() {
            if !bounded(d.rival_atp, 2.0) || !bounded(d.raid, 1.0) {
                return Err("Invalid difficulty multipliers".into());
            }
        }
        for (id, l) in &self.locations {
            if id != &l.id || !bounded(l.x, 100.0) || !bounded(l.y, 100.0) {
                return Err("Invalid district".into());
            }
        }
        for (id, o) in &self.organelles {
            if id != &o.id
                || o.atp == 0
                || o.atp > 1000
                || o.cost > 1_000_000
                || o.min_transcription > 4
                || !bounded(o.min_size, 100.0)
                || o.requires
                    .iter()
                    .any(|r| r == id || !self.organelles.contains_key(r))
                || o.only.iter().any(|s| !self.specimens.contains_key(s))
            {
                return Err("Invalid organelle".into());
            }
        }
        let mut course_ids = std::collections::BTreeSet::new();
        for course in &self.all_courses {
            if !course_ids.insert(&course.id)
                || !(1..=4).contains(&course.tier)
                || course.sessions == 0
                || course.sessions > 100
                || course.atp == 0
                || course.atp > 1000
                || course.biomass > 1_000_000
                || !["cy", "gr", "bc"].iter().any(|p| course.id.starts_with(p))
            {
                return Err("Invalid gene expression course".into());
            }
        }
        let mut event_ids = std::collections::BTreeSet::new();
        for event in &self.events {
            if !event_ids.insert(&event.id)
                || event.choices.is_empty()
                || event.choices.len() > 16
                || !event.choices.iter().any(|c| c.requires.is_none())
            {
                return Err("Encounter needs unique identity and an unconditional choice".into());
            }
            let mut choices = std::collections::BTreeSet::new();
            for choice in &event.choices {
                if !choices.insert(&choice.id)
                    || choice.effects.iter().any(|(k, v)| {
                        !v.is_finite()
                            || v.abs() > 10000.0
                            || ![
                                "atp",
                                "biomass",
                                "nutrition",
                                "stress",
                                "size",
                                "signaling",
                                "motility",
                                "integrity",
                                "storage",
                                "relationship",
                                "deterrence",
                                "truce",
                            ]
                            .contains(&k.as_str())
                    })
                {
                    return Err("Invalid encounter effects".into());
                }
                if let Some(r) = &choice.requires {
                    let r = r
                        .as_object()
                        .ok_or("Encounter requirements must be an object")?;
                    for (k, v) in r {
                        match k.as_str() {
                            "organelle" => {
                                if !v.as_str().is_some_and(|s| self.organelles.contains_key(s)) {
                                    return Err("Unknown encounter organelle".into());
                                }
                            }
                            "minMotility" | "minTranscription" | "minBiomass" => {
                                if !v.as_f64().is_some_and(|n| bounded(n, 1_000_000.0)) {
                                    return Err("Invalid encounter threshold".into());
                                }
                            }
                            _ => return Err("Unknown encounter requirement".into()),
                        }
                    }
                }
            }
        }
        if self.specimens.is_empty()
            || self.scenarios.is_empty()
            || self.locations.len() > 64
            || self.events.is_empty()
        {
            return Err(
                "Content must provide specimens, scenarios, locations and encounters".into(),
            );
        }
        for id in [
            "home",
            "university",
            "trophic",
            "boutique",
            "diner",
            "gym",
            "checkpoint",
            "solar",
            "detritus",
            "security",
            "rift",
        ] {
            if !self.locations.contains_key(id) {
                return Err(format!("Missing district {id}"));
            }
        }
        for id in &self.specimen_order {
            if !self.specimens.contains_key(id) {
                return Err("Unknown specimen order entry".into());
            }
        }
        for id in &self.location_order {
            if !self.locations.contains_key(id) {
                return Err("Unknown location order entry".into());
            }
        }
        for id in &self.scenario_order {
            if !self.scenarios.contains_key(id) {
                return Err("Unknown scenario order entry".into());
            }
        }
        for (id, s) in &self.specimens {
            if id != &s.id
                || [s.start_motility, s.start_integrity, s.start_signaling]
                    .iter()
                    .any(|v| !bounded(*v, 100.0))
            {
                return Err("Invalid specimen".into());
            }
            if s.start_organelles
                .iter()
                .chain(&s.signature)
                .any(|o| !self.organelles.contains_key(o))
            {
                return Err("Unknown specimen organelle".into());
            }
        }
        for (id, s) in &self.scenarios {
            if id != &s.id
                || s.rival_offset >= self.specimen_order.len()
                || s.start_biomass > 1_000_000
                || s.quests.len() > 64
                || s.quests
                    .iter()
                    .map(|q| &q.id)
                    .collect::<std::collections::BTreeSet<_>>()
                    .len()
                    != s.quests.len()
            {
                return Err("Invalid scenario identity or bounds".into());
            }
            if s.turn_limit == 0
                || s.turn_limit > 10000
                || s.quests.is_empty()
                || !(0.0..=100.0).contains(&s.start_size)
                || s.weather.values().any(|w| !w.is_finite() || *w < 0.0)
                || s.weather.values().filter(|w| **w > 0.0).count() < 2
                || s.weather.keys().any(|w| !self.weather.contains_key(w))
            {
                return Err("Invalid scenario".into());
            }
            for q in &s.quests {
                if q.kind == "organelle"
                    && !q
                        .param
                        .as_ref()
                        .is_some_and(|o| self.organelles.contains_key(o))
                {
                    return Err("Unknown quest organelle".into());
                }
                if ![
                    "size",
                    "earned",
                    "career",
                    "organelles",
                    "organelle",
                    "transcription",
                    "phase",
                    "signature",
                    "homeostasis",
                    "pathways",
                    "mitosis",
                ]
                .contains(&q.kind.as_str())
                    || !q.target.is_finite()
                    || q.target < 0.0
                {
                    return Err("Invalid quest".into());
                }
            }
        }
        Ok(())
    }
}
