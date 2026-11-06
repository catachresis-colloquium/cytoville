// SPDX-License-Identifier: AGPL-3.0-or-later
use crate::content::Choice;
use crate::{Actor, Career, Content, Game, Look, rand, rules::*};

pub fn new_game(
    c: &Content,
    specimen: &str,
    scenario: &str,
    difficulty: &str,
    seed: u32,
    multiplayer: bool,
) -> Result<Game, String> {
    c.validate()?;
    let sp = c.specimens.get(specimen).ok_or("Unknown specimen")?;
    let sc = c.scenarios.get(scenario).ok_or("Unknown scenario")?;
    if !c.difficulty.contains_key(difficulty) {
        return Err("Unknown difficulty".into());
    }
    let i = c
        .specimen_order
        .iter()
        .position(|x| x == specimen)
        .ok_or("Specimen absent from order")?;
    let rival = &c.specimen_order[(i + sc.rival_offset) % c.specimen_order.len()];
    let create = |id: &str| {
        let s = &c.specimens[id];
        Actor {
            name: s.name.clone(),
            specimen: id.into(),
            location: "home".into(),
            atp: 100,
            atp_max: 100,
            biomass: sc.start_biomass,
            total_earned: 0,
            nutrition: 70.0,
            stress: 10.0,
            size: sc.start_size,
            phase: "G1".into(),
            s_progress: 0,
            transcription: 0,
            course_progress: Default::default(),
            courses: vec![],
            motility: s.start_motility,
            integrity: s.start_integrity,
            signaling: s.start_signaling,
            organelles: s.start_organelles.clone(),
            synthesized: 0,
            storage_stock: 0,
            careers: ["solar", "detritus", "bacterivore"]
                .into_iter()
                .map(|x| (x.into(), Career::default()))
                .collect(),
            phenotype: "balanced".into(),
            done: vec![],
            divided: false,
            pending_encyst: false,
            encystments: 0,
            hazards_blocked: 0,
            homeo_streak: 0,
            spa_used: false,
        }
    };
    let look = |s: &crate::content::Specimen| Look {
        name: s.name.clone(),
        membrane: s.body.clone(),
        accent: s.accent.clone(),
        pattern: "plain".into(),
    };
    let mut g = Game {
        format: "cytoville-save/1".into(),
        scenario_id: scenario.into(),
        difficulty: difficulty.into(),
        turn: 1,
        rng_state: seed,
        weather: "calm".into(),
        forecast: "calm".into(),
        actors: [create(specimen), create(rival)],
        looks: [look(sp), look(&c.specimens[rival])],
        active: 0,
        multiplayer,
        revision: 0,
        status: "playing".into(),
        winner: None,
        log: vec![],
        report: vec![],
        rival_trail: vec![],
        pending_event: None,
        event_history: vec![],
        relationship: 0,
        deterrence: 0,
        truce_turns: 0,
        intel: false,
    };
    if scenario != "fresh-daughter" {
        g.weather = weather(c, &mut g);
    }
    g.forecast = weather(c, &mut g);
    start(c, &mut g, 0);
    start(c, &mut g, 1);
    g.message(format!("Cycle 1: {}", sc.name));
    g.validate(c)?;
    Ok(g)
}

fn weather(c: &Content, g: &mut Game) -> String {
    let weights = &c.scenarios[&g.scenario_id].weather;
    let total: f64 = weights
        .iter()
        .filter(|(k, _)| **k != g.weather)
        .map(|(_, w)| w)
        .sum();
    let mut draw = rand(g) * total;
    let mut last = "calm";
    for id in [
        "calm", "sunny", "uv", "bloom", "osmotic", "cloudy", "anoxic",
    ] {
        let w = weights.get(id).copied().unwrap_or(0.0);
        if id == g.weather || w <= 0.0 {
            continue;
        }
        last = id;
        draw -= w;
        if draw <= 0.0 {
            return id.into();
        }
    }
    last.into()
}

pub fn start(c: &Content, g: &mut Game, who: usize) {
    let a = &mut g.actors[who];
    a.location = "home".into();
    a.spa_used = false;
    if g.weather == "osmotic" && !a.has("vacuole") {
        stress(a, 18.0);
    }
    if g.weather == "uv" && !a.has("pellicle") && !a.has("frustule") {
        stress(a, if a.specimen == "stentor" { 6.0 } else { 14.0 });
    }
    if a.has("vacuole") {
        stress(a, -6.0);
    }
    if a.has("frustule") {
        stress(a, -2.0);
    }
    if a.specimen == "stentor" {
        stress(a, -4.0);
    }
    if a.has("storage") && a.storage_stock > 0 && a.nutrition < 65.0 {
        a.storage_stock -= 1;
        a.nutrition = (a.nutrition + 45.0).min(100.0);
    }
    let mut penalty = 0;
    let turnover = a.organelles.len().saturating_sub(5) as u32 * 2;
    if let Some(left) = crate::budget::spend(a.biomass, turnover) {
        a.biomass = left;
    } else {
        penalty += 4;
        stress(a, 6.0);
    }
    let nuclei = ["macronucleus", "micronucleus"]
        .iter()
        .filter(|x| a.has(x))
        .count() as u32
        * 4;
    if let Some(left) = crate::budget::spend(a.biomass, nuclei) {
        a.biomass = left;
    } else {
        penalty += 8;
    }
    let mult = if who == 1 && !g.multiplayer {
        c.difficulty[&g.difficulty].rival_atp
    } else {
        1.0
    };
    a.atp_max = atp(a, penalty, mult);
    if g.weather == "anoxic" {
        if a.has("hydrogenosome") {
            a.atp_max += 10;
            stress(a, -4.0);
        } else {
            a.atp_max = a.atp_max.saturating_sub(10).max(30);
        }
    }
    a.atp = a.atp_max;
    encyst(a);
    refresh(c, a, &g.scenario_id);
}

fn close(c: &Content, g: &mut Game, who: usize) {
    let mut a = g.actors[who].clone();
    if a.divided {
        return;
    }
    if a.location == "home" {
        let clear = (a.atp / 4).min(20) as f64;
        stress(&mut a, -clear);
    } else if rand(g) < 0.3 {
        if a.armor() {
            a.hazards_blocked += 1;
            g.message(format!("{} blocked a street hazard", a.name));
        } else {
            let r = rand(g);
            if r < 0.34 {
                let loss = ((a.biomass as f64 * 0.1).round() as u32).min(60);
                a.biomass -= loss;
                g.message(format!("Street raid: {} lost {loss} biomass", a.name));
            } else if r < 0.67 {
                stress(&mut a, 18.0);
                g.message("Toxic plume: stress rose");
            } else {
                a.nutrition = (a.nutrition - 15.0).max(0.0);
                g.message("Shear current swept food away");
            }
        }
    }
    a.homeo_streak = if a.stress <= 35.0 && a.nutrition >= 50.0 {
        a.homeo_streak + 1
    } else {
        0
    };
    let mut growth: f64 = if a.nutrition >= 70.0 {
        5.0
    } else if a.nutrition >= 50.0 {
        3.0
    } else if a.nutrition >= 30.0 {
        1.0
    } else if a.nutrition < 20.0 {
        -1.0
    } else {
        0.0
    };
    if a.stress > 70.0 && growth > 0.0 {
        growth = (growth / 2.0).floor();
    }
    a.size = (a.size + growth).clamp(0.0, 100.0);
    a.nutrition = (a.nutrition - 30.0).max(0.0);
    refresh(c, &mut a, &g.scenario_id);
    g.actors[who] = a;
}

pub fn victory(c: &Content, g: &mut Game) {
    if g.status != "playing" {
        return;
    }
    for who in 0..2 {
        if all_done(c, &g.actors[who], &g.scenario_id) {
            g.status = if who == 0 || g.multiplayer {
                "won"
            } else {
                "lost"
            }
            .into();
            g.winner = Some(who);
            g.message(format!(
                "{} completed the ontogenetic checklist!",
                g.actors[who].name
            ));
            break;
        }
    }
}

pub fn end_seat(c: &Content, g: &mut Game, who: usize) -> Result<(), String> {
    close(c, g, who);
    if who == 0 {
        g.active = 1;
        g.rival_trail.clear();
        if g.multiplayer && g.turn > 1 && rand(g) < 0.55 {
            let i = (rand(g) * c.events.len() as f64) as usize;
            g.pending_event = Some(c.events[i].id.clone());
        }
        return Ok(());
    }
    if !g.multiplayer {
        let raid = c.difficulty[&g.difficulty].raid
            * (1.0 - (g.relationship as f64 + 100.0) / 300.0)
            * if g.deterrence > 0 { 0.35 } else { 1.0 }
            * if g.truce_turns > 0 { 0.0 } else { 1.0 };
        if g.turn >= 3 && !g.actors[1].divided && rand(g) < raid {
            if g.actors[0].armor() {
                g.actors[0].hazards_blocked += 1;
                g.message("Rival raid repelled by armour");
            } else if rand(g) < 0.5 {
                let steal = ((g.actors[0].biomass as f64 * 0.08).round() as u32).min(50);
                g.actors[0].biomass -= steal;
                g.actors[1].biomass = g.actors[1].biomass.saturating_add(steal);
                g.message(format!("Rival poached {steal} biomass"));
            } else {
                stress(&mut g.actors[0], 12.0);
                g.message("Rival jammed signalling");
            }
        }
    }
    g.truce_turns = g.truce_turns.saturating_sub(1);
    g.deterrence = g.deterrence.saturating_sub(1);
    victory(c, g);
    g.report = g.log.iter().rev().take(16).cloned().collect::<Vec<_>>();
    g.report.reverse();
    if g.status != "playing" {
        return Ok(());
    }
    if g.turn >= c.scenarios[&g.scenario_id].turn_limit {
        g.status = if g.multiplayer { "draw" } else { "lost" }.into();
        g.message("The biofilm ran out of nutrients before the checklist was complete");
        return Ok(());
    }
    g.turn += 1;
    g.active = 0;
    g.weather = g.forecast.clone();
    g.forecast = weather(c, g);
    start(c, g, 0);
    start(c, g, 1);
    if rand(g) < 0.55 {
        let i = (rand(g) * c.events.len() as f64) as usize;
        g.pending_event = Some(c.events[i].id.clone());
        g.message(format!("Encounter: {}", c.events[i].title));
    }
    g.message(format!("Cycle {}: {}", g.turn, c.weather[&g.weather].name));
    Ok(())
}

pub fn choice_lock(a: &Actor, choice: &Choice) -> Option<String> {
    if let Some(r) = &choice.requires {
        if let Some(o) = r.get("organelle").and_then(|v| v.as_str())
            && !a.has(o)
        {
            return Some(format!("Needs {o}"));
        }
        for (key, stat) in [
            ("minMotility", a.motility),
            ("minTranscription", a.transcription as f64),
            ("minBiomass", a.biomass as f64),
        ] {
            if r.get(key)
                .and_then(|v| v.as_f64())
                .is_some_and(|min| stat < min)
            {
                return Some(format!("Requirement: {key}"));
            }
        }
    }
    None
}
pub fn choose(c: &Content, g: &mut Game, who: usize, id: &str) -> Result<(), String> {
    let event = c
        .events
        .iter()
        .find(|e| Some(&e.id) == g.pending_event.as_ref())
        .ok_or("No encounter pending")?;
    let choice = event
        .choices
        .iter()
        .find(|x| x.id == id)
        .ok_or("Unknown encounter choice")?;
    if let Some(reason) = choice_lock(&g.actors[who], choice) {
        return Err(reason);
    }
    let a = &mut g.actors[who];
    let e = &choice.effects;
    let effect = |key: &str| *e.get(key).unwrap_or(&0.0);
    a.atp = (a.atp as f64 + effect("atp")).clamp(0.0, 1000.0) as u32;
    a.biomass = (a.biomass as f64 + effect("biomass")).clamp(0.0, u32::MAX as f64) as u32;
    a.total_earned = a
        .total_earned
        .saturating_add(effect("biomass").max(0.0) as u32);
    nutrition(a, effect("nutrition"), false);
    stress(a, effect("stress"));
    a.size = (a.size + effect("size")).clamp(0.0, 100.0);
    a.signaling = (a.signaling + effect("signaling")).clamp(0.0, 100.0);
    a.motility = (a.motility + effect("motility")).clamp(0.0, 100.0);
    a.integrity = (a.integrity + effect("integrity")).clamp(0.0, 100.0);
    a.storage_stock = (a.storage_stock as f64 + effect("storage")).clamp(0.0, 6.0) as u32;
    g.relationship = (g.relationship + effect("relationship") as i32).clamp(-100, 100);
    g.deterrence = (g.deterrence as f64 + effect("deterrence")).clamp(0.0, 5.0) as u32;
    g.truce_turns = g.truce_turns.max(effect("truce") as u32);
    encyst(a);
    refresh(c, a, &g.scenario_id);
    g.event_history.push(event.id.clone());
    g.pending_event = None;
    g.message(format!("{}: {}", event.title, choice.result));
    Ok(())
}
