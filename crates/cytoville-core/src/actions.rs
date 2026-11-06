// SPDX-License-Identifier: AGPL-3.0-or-later
use crate::{Action, Actor, Content, Game, budget, rand, rules::*};

// A single action constructor keeps costs, lock reasons and UI labels together.
#[allow(clippy::too_many_arguments)]
fn option(
    a: &Actor,
    id: &str,
    label: &str,
    tag: &str,
    loc: &str,
    atp: u32,
    biomass: u32,
    lock: Option<String>,
    value: f64,
) -> Action {
    let reason = lock.or_else(|| {
        if a.atp < atp {
            Some(format!("Needs {atp} ATP"))
        } else if a.biomass < biomass {
            Some(format!("Needs {biomass} biomass"))
        } else {
            None
        }
    });
    Action {
        id: id.into(),
        label: label.into(),
        description: String::new(),
        tag: tag.into(),
        location: loc.into(),
        atp,
        biomass,
        reason,
        value,
    }
}
fn needs(condition: bool, message: &str) -> Option<String> {
    if condition {
        Some(message.into())
    } else {
        None
    }
}

pub fn list(c: &Content, g: &Game, who: usize, loc: &str) -> Vec<Action> {
    let Some(a) = g.actors.get(who) else {
        return vec![];
    };
    let other = &g.actors[budget::other_seat(who as u32) as usize];
    let mut out = vec![];
    let mut add = |id: &str, label: &str, tag: &str, atp, biomass, lock, value| {
        out.push(option(a, id, label, tag, loc, atp, biomass, lock, value))
    };
    if loc == "home" {
        add(
            "home:rest",
            "Quiescent Rest",
            "rest",
            6,
            0,
            needs(a.stress <= 0.0, "No stress to clear"),
            if a.stress > 30.0 { 10.0 } else { 0.0 },
        );
        if a.specimen == "euglena" {
            for p in ["balanced", "photo", "hetero"] {
                add(
                    &format!("home:pheno:{p}"),
                    &format!("Express {p} phenotype"),
                    "phenotype",
                    4,
                    0,
                    needs(a.phenotype == p, "Already expressed"),
                    0.0,
                );
            }
        }
    }
    if loc == "university" {
        for course in &c.all_courses {
            let lock = needs(a.courses.contains(&course.id), "Course complete").or_else(|| {
                needs(
                    course.tier > a.transcription + 1,
                    "Complete the preceding transcription tier",
                )
            });
            add(
                &format!("study:{}", course.id),
                &format!(
                    "Tier {}: {} ({}/{})",
                    course.tier,
                    course.name,
                    a.course_progress.get(&course.id).unwrap_or(&0),
                    course.sessions
                ),
                "study",
                course.atp,
                course.biomass,
                lock,
                40.0,
            );
        }
    }
    for (track, d) in &c.careers {
        if d.location != loc {
            continue;
        }
        let p = pay(c, a, track, &g.weather) as f64;
        let lock = career_lock(a, track);
        add(
            &format!("work:{track}"),
            &format!("Work: {} (+{} biomass)", d.name, p),
            "work",
            d.atp,
            0,
            lock.clone(),
            p,
        );
        add(
            &format!("light:{track}"),
            "Light shift",
            "work",
            ((d.atp as f64 * 0.5).round() as u32).max(8),
            0,
            lock.clone(),
            p * 0.55,
        );
        add(
            &format!("overtime:{track}"),
            "Overtime: double shift",
            "work",
            d.atp * 2,
            0,
            lock.or_else(|| needs(a.careers[track].level < 2, "Needs Junior rank")),
            p * 2.1,
        );
        if (1..4).contains(&a.careers[track].level) {
            add(
                &format!("promote:{track}"),
                "Request promotion",
                "promote",
                5,
                0,
                promotion_lock(a, track),
                60.0,
            );
        }
        add(
            &format!("drill:{track}"),
            "Specialist workplace training",
            "train",
            10,
            8,
            None,
            12.0,
        );
        if track == "solar" {
            add(
                "solar:status",
                "Read irradiance forecast",
                "signal",
                5,
                0,
                None,
                6.0,
            );
        }
    }
    if loc == "rift" {
        add(
            "rift:signal",
            "Quorum cross-talk",
            "signal",
            8,
            4,
            None,
            10.0,
        );
        add(
            "rift:crossfeed",
            "Cross-feed metabolites",
            "food",
            10,
            8,
            None,
            14.0,
        );
        add(
            "rift:exclude",
            "Competitive exclusion",
            "signal",
            12,
            0,
            needs(a.motility < 15.0, "Needs Motility 15"),
            20.0,
        );
        add(
            "rift:deter",
            "Contact inhibition display",
            "signal",
            10,
            5,
            needs(!a.armor(), "Needs defensive organelle"),
            15.0,
        );
        add(
            "rift:truce",
            "Propose a signalling truce",
            "signal",
            12,
            12,
            needs(g.relationship < 0, "Relationship must be nonnegative"),
            12.0,
        );
        let lock = needs(!a.phagocytic(), "Needs phagocytic apparatus")
            .or_else(|| needs(!a.has("lysosome"), "Needs lysosome"))
            .or_else(|| {
                needs(
                    a.size < other.size * 0.8,
                    "Needs size at least 80% of rival",
                )
            });
        add("rift:prey", "Attempt predation", "food", 20, 0, lock, 25.0);
    }
    if loc == "boutique" {
        for (id, d) in &c.organelles {
            if a.has(id) || (!d.only.is_empty() && !d.only.contains(&a.specimen)) {
                continue;
            }
            let lock = needs(
                a.transcription < d.min_transcription,
                "Needs higher transcription tier",
            )
            .or_else(|| needs(a.size < d.min_size, "Needs larger cell"))
            .or_else(|| {
                needs(
                    d.requires.iter().any(|r| !a.has(r)),
                    "Missing prerequisite organelle",
                )
            });
            add(
                &format!("synth:{id}"),
                &d.name,
                "synth",
                d.atp,
                d.cost,
                lock,
                0.0,
            );
        }
    }
    if loc == "diner" {
        let mult = c.weather[&g.weather].diner_mult;
        for (id, label, cost, atp, gain) in [
            ("sugar", "Simple Sugars", 8.0, 3, 25.0),
            ("bacteria", "Engulfed Bacteria", 18.0, 5, 45.0),
            ("broth", "Premium Nutrient Broth", 42.0, 5, 80.0),
        ] {
            add(
                &format!("eat:{id}"),
                label,
                "food",
                atp,
                (cost * mult).round() as u32,
                needs(a.nutrition >= 100.0, "Already full"),
                gain,
            );
        }
        add(
            "stock",
            "Bulk Nutrient Pack (5 units)",
            "stock",
            6,
            (40.0 * mult).round() as u32,
            needs(!a.has("storage"), "Needs Storage Granules")
                .or_else(|| needs(a.storage_stock >= 6, "Granules full")),
            20.0,
        );
    }
    if loc == "gym" {
        add(
            "gym:motility",
            "Intraflagellar Transport Run",
            "train",
            14,
            15,
            needs(a.motility >= 100.0, "Maxed out"),
            10.0,
        );
        add(
            "gym:integrity",
            "Microtubule Lattice Training",
            "train",
            14,
            15,
            needs(a.integrity >= 100.0, "Maxed out"),
            10.0,
        );
        add(
            "spa:soak",
            "Lysosome Steam Soak",
            "detox",
            8,
            15,
            needs(a.spa_used, "Already used this cycle")
                .or_else(|| needs(a.stress <= 0.0, "No stress to clear")),
            25.0,
        );
        add(
            "spa:clinic",
            "Peroxisome Clinic Treatment",
            "detox",
            16,
            90,
            needs(a.stress <= 0.0, "No stress to clear"),
            55.0,
        );
    }
    if loc == "checkpoint" {
        add(
            "cp:g1s",
            "Pass G1/S Restriction Point",
            "checkpoint",
            20,
            150,
            needs(a.phase != "G1", "Already past G1")
                .or_else(|| needs(a.size < 50.0, "Needs size 50%"))
                .or_else(|| needs(a.transcription < 2, "Needs transcription tier 2")),
            200.0,
        );
        add(
            "cp:sphase",
            "S-Phase DNA Synthesis",
            "checkpoint",
            25,
            40,
            needs(a.phase != "S", "Requires S phase")
                .or_else(|| needs(a.transcription < 4, "Needs transcription tier 4")),
            180.0,
        );
        let s = &c.scenarios[&g.scenario_id];
        let lock = needs(!s.allow_mitosis, "Not in this scenario")
            .or_else(|| needs(a.phase != "G2", "Needs mature G2"))
            .or_else(|| needs(a.size < 90.0, "Needs size 90%"))
            .or_else(|| needs(a.stress > 60.0, "Stress must be 60% or less"))
            .or_else(|| {
                needs(
                    s.quests
                        .iter()
                        .any(|q| q.kind != "mitosis" && !a.done.contains(&q.id)),
                    "Finish all other goals",
                )
            });
        add(
            "cp:mitosis",
            "Mitosis and Binary Fission",
            "checkpoint",
            30,
            100,
            lock,
            500.0,
        );
        add(
            "cp:signal",
            "Register Cell-Surface Signals",
            "signal",
            10,
            20,
            needs(a.signaling >= 100.0, "Maxed out"),
            8.0,
        );
    }
    out
}

pub fn travel(c: &Content, g: &mut Game, who: usize, to: &str) -> Result<(), String> {
    let a = &mut g.actors[who];
    let cost = travel_cost(c, a, to).ok_or("Unknown district")?;
    a.atp = budget::spend(a.atp, cost).ok_or("Insufficient ATP for travel")?;
    a.location = to.into();
    Ok(())
}

pub fn execute(c: &Content, g: &mut Game, who: usize, id: &str) -> Result<(), String> {
    let action = list(c, g, who, &g.actors[who].location)
        .into_iter()
        .find(|a| a.id == id)
        .ok_or("Action unavailable here")?;
    if let Some(reason) = action.reason {
        return Err(reason);
    }
    // Take an owned working copy so interactions cannot alias the other actor.
    let mut a = g.actors[who].clone();
    a.atp = budget::spend(a.atp, action.atp).ok_or("Insufficient ATP")?;
    a.biomass = budget::spend(a.biomass, action.biomass).ok_or("Insufficient biomass")?;
    let (kind, param) = id.split_once(':').unwrap_or((id, ""));
    match kind {
        "home" if param == "rest" => stress(&mut a, -14.0),
        "home" => {
            a.phenotype = param
                .strip_prefix("pheno:")
                .ok_or("Invalid phenotype")?
                .into()
        }
        "study" => {
            let course = c
                .all_courses
                .iter()
                .find(|x| x.id == param)
                .ok_or("Unknown course")?;
            let progress = a.course_progress.entry(param.into()).or_default();
            *progress += 1;
            if *progress >= course.sessions {
                a.courses.push(param.into());
                a.transcription = a.transcription.max(course.tier);
                if param.starts_with("cy") {
                    a.motility = (a.motility + 4.0).min(100.0);
                }
                if param.starts_with("gr") {
                    a.signaling = (a.signaling + 2.0).min(100.0);
                }
                if param.starts_with("bc") {
                    a.size = (a.size + 1.0).min(100.0);
                }
                g.message(format!("Course completed: {}", course.name));
            }
        }
        "work" | "light" | "overtime" => {
            let d = &c.careers[param];
            let multiplier = if kind == "light" {
                0.55
            } else if kind == "overtime" {
                2.1
            } else {
                1.0
            };
            let earned = (pay(c, &a, param, &g.weather) as f64 * multiplier).round() as u32;
            a.biomass = a.biomass.saturating_add(earned);
            a.total_earned = a.total_earned.saturating_add(earned);
            let p = a.careers.get_mut(param).ok_or("Unknown career")?;
            p.level = p.level.max(1);
            p.exp = p.exp.saturating_add(if kind == "light" {
                6
            } else if kind == "overtime" {
                30
            } else {
                12
            });
            p.shifts += if kind == "overtime" { 2 } else { 1 };
            a.size = (a.size
                + if kind == "light" {
                    0.25
                } else if kind == "overtime" {
                    2.0
                } else {
                    0.5
                })
            .min(100.0);
            let s = if kind == "light" {
                (d.stress * 0.5).ceil()
            } else if kind == "overtime" {
                d.stress * 2.0 + 10.0
            } else if param == "detritus" && a.has("lysosome") {
                (d.stress - 3.0).max(2.0)
            } else {
                d.stress
            };
            stress(&mut a, s);
            if kind == "work" {
                let n = if param == "solar" && ["euglena", "diatom"].contains(&a.specimen.as_str())
                {
                    6.0
                } else {
                    d.nutrition
                };
                nutrition(&mut a, n, false);
            }
        }
        "promote" => {
            a.careers.get_mut(param).ok_or("Unknown career")?.level += 1;
            a.signaling = (a.signaling + 4.0).min(100.0);
        }
        "drill" => match param {
            "solar" => {
                a.signaling = (a.signaling + 2.0).min(100.0);
                stress(&mut a, -3.0);
            }
            "detritus" => stress(&mut a, -8.0),
            _ => {
                a.motility = (a.motility + 4.0).min(100.0);
                a.signaling = (a.signaling + 1.0).min(100.0);
                stress(&mut a, 2.0);
            }
        },
        "solar" => {
            a.signaling = (a.signaling + 1.0).min(100.0);
            g.intel = true;
        }
        "synth" => {
            a.organelles.push(param.into());
            a.synthesized += 1;
            if ["pellicle", "micronucleus"].contains(&param) {
                a.signaling = (a.signaling + 3.0).min(100.0);
            }
        }
        "eat" => {
            let (gain, grow, clear) = match param {
                "sugar" => (25.0, 0.0, 0.0),
                "bacteria" => (45.0, 1.0, 0.0),
                _ => (80.0, 3.0, -5.0),
            };
            nutrition(&mut a, gain, true);
            a.size = (a.size + grow).min(100.0);
            stress(&mut a, clear);
        }
        "stock" => a.storage_stock = (a.storage_stock + 5).min(6),
        "gym" => {
            let stat = if param == "motility" {
                &mut a.motility
            } else {
                &mut a.integrity
            };
            *stat = (*stat + if *stat >= 80.0 { 3.0 } else { 6.0 }).min(100.0);
            stress(&mut a, 2.0);
        }
        "spa" => {
            stress(&mut a, if param == "soak" { -25.0 } else { -55.0 });
            if param == "soak" {
                a.spa_used = true;
            }
        }
        "cp" => match param {
            "g1s" => {
                a.phase = "S".into();
                a.s_progress = 0;
            }
            "sphase" => {
                a.s_progress += 1;
                if a.s_progress >= 3 {
                    a.phase = "G2".into();
                    a.size = (a.size + 8.0).min(100.0);
                }
            }
            "mitosis" => {
                a.divided = true;
                a.phase = "M".into();
            }
            _ => {
                a.signaling = (a.signaling + 8.0).min(100.0);
                g.intel = true;
            }
        },
        "rift" => match param {
            "signal" => {
                a.signaling = (a.signaling + 6.0).min(100.0);
                g.relationship = (g.relationship + 10).min(100);
                g.intel = true;
            }
            "crossfeed" => {
                nutrition(&mut a, 16.0, false);
                nutrition(
                    &mut g.actors[budget::other_seat(who as u32) as usize],
                    16.0,
                    false,
                );
                g.relationship = (g.relationship + 12).min(100);
            }
            "exclude" => {
                let other = &mut g.actors[budget::other_seat(who as u32) as usize];
                let steal = ((other.biomass as f64 * 0.08).round() as u32).min(30);
                other.biomass -= steal;
                a.biomass = a.biomass.saturating_add(steal);
                stress(&mut a, 5.0);
                g.relationship = (g.relationship - 12).max(-100);
            }
            "deter" => {
                g.deterrence = (g.deterrence + 2).min(5);
                g.relationship = (g.relationship - 6).max(-100);
                stress(&mut a, -2.0);
            }
            "truce" => {
                g.truce_turns = 2;
                g.deterrence = (g.deterrence + 1).min(5);
                g.relationship = (g.relationship + 20).min(100);
            }
            "prey" => {
                let chance = if a.has("toxicyst") || a.specimen == "didinium" {
                    0.85
                } else {
                    0.6
                };
                if rand(g) < chance {
                    let other = &mut g.actors[budget::other_seat(who as u32) as usize];
                    // Original could take more biomass than the victim owned. Transfer conserves it.
                    let gain = ((((other.biomass as f64 * 0.12).round() + 20.0)
                        * if a.has("proboscis") { 1.5 } else { 1.0 })
                    .round() as u32)
                        .min(90)
                        .min(u32::MAX - a.biomass);
                    let (remaining, gain) = budget::take_up_to(other.biomass, gain);
                    other.biomass = remaining;
                    other.size = (other.size - 3.0).max(0.0);
                    a.biomass = a.biomass.saturating_add(gain);
                    nutrition(&mut a, 20.0, false);
                    a.size = (a.size + 1.0).min(100.0);
                    g.relationship = (g.relationship - 30).max(-100);
                } else {
                    a.atp = a.atp.saturating_sub(10);
                    stress(&mut a, 10.0);
                    g.relationship = (g.relationship - 20).max(-100);
                }
            }
            _ => return Err("Unknown interaction".into()),
        },
        _ => return Err("Unknown action".into()),
    }
    if encyst(&mut a) {
        g.message(format!("{} encysted: stress hit 100%", a.name));
    }
    refresh(c, &mut a, &g.scenario_id);
    g.message(format!("{}: {}", a.name, action.label));
    g.actors[who] = a;
    Ok(())
}
