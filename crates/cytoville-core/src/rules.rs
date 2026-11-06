// SPDX-License-Identifier: AGPL-3.0-or-later
use crate::content::Quest;
use crate::{Actor, Content};

pub fn stress(a: &mut Actor, amount: f64) {
    let eff = if amount > 0.0 {
        (amount * (1.0 - a.integrity / 300.0)).round()
    } else {
        amount
    };
    a.stress = (a.stress + eff).clamp(0.0, 100.0);
    if a.stress >= 100.0 {
        a.pending_encyst = true;
    }
}
pub fn nutrition(a: &mut Actor, amount: f64, food: bool) {
    let mut n = amount;
    if food && a.has("oral") {
        n *= 1.2;
    }
    if food && a.specimen == "stentor" {
        n *= 1.15;
    }
    a.nutrition = (a.nutrition + n).round().clamp(0.0, 100.0);
}
pub fn encyst(a: &mut Actor) -> bool {
    if !a.pending_encyst {
        return false;
    }
    a.pending_encyst = false;
    a.atp = a.atp.saturating_sub(a.atp_max / 2);
    a.stress = 60.0;
    a.encystments += 1;
    true
}
pub fn travel_cost(c: &Content, a: &Actor, to: &str) -> Option<u32> {
    let dest = c.locations.get(to)?;
    let src = c.locations.get(&a.location)?;
    if a.location == to {
        return Some(0);
    }
    let tier = if a.has("flagellum") {
        3
    } else if a.has("cilia") {
        2
    } else if a.has("basal") {
        1
    } else {
        0
    };
    let mut m = 1.0 - [0.0, 0.25, 0.5, 0.75][tier];
    m *= match a.specimen.as_str() {
        "vorticella" => {
            if a.has("stalk") {
                1.25
            } else {
                0.75
            }
        }
        "stentor" => {
            if a.has("holdfast") {
                1.25
            } else {
                0.9
            }
        }
        "paramecium" => 0.9,
        "diatom" => 1.1,
        "amoeba" => 1.08,
        "didinium" => 0.95,
        _ => 1.0,
    };
    m *= (1.0 - a.course_count("cy") as f64 * 0.02) * (1.0 - a.motility / 500.0);
    let dx = (src.x - dest.x) * 1.6;
    let dy = src.y - dest.y;
    Some(((dx * dx + dy * dy).sqrt() * 0.18 * m).round().max(2.0) as u32)
}
pub fn atp(a: &Actor, penalty: u32, mult: f64) -> u32 {
    let mut total =
        100.0 + (a.integrity / 8.0).floor().min(12.0) + (a.size / 10.0).floor().min(10.0);
    if a.has("myoneme") {
        total += 5.0;
    }
    total -= if a.nutrition < 20.0 {
        30.0
    } else if a.nutrition < 40.0 {
        10.0
    } else {
        0.0
    };
    total -= if a.stress >= 75.0 {
        15.0
    } else if a.stress >= 50.0 {
        6.0
    } else {
        0.0
    };
    ((total - f64::from(penalty)).max(40.0) * mult).round() as u32
}
pub fn career_lock(a: &Actor, track: &str) -> Option<String> {
    if track == "solar" && !a.has("chloroplast") {
        return Some("Needs a chloroplast".into());
    }
    if track == "bacterivore" {
        if !a.phagocytic() {
            return Some("Needs phagocytic apparatus".into());
        }
        if !a.has("lysosome") {
            return Some("Needs Lysosome Battery".into());
        }
        if a.motility < 15.0 {
            return Some("Needs Motility 15".into());
        }
    }
    None
}
pub fn pay(c: &Content, a: &Actor, track: &str, weather: &str) -> u32 {
    let rank = a.careers[track].level.max(1) as usize;
    let mut p = c.careers[track].pay[rank - 1] * c.weather[weather].pay[track];
    if a.specimen == "euglena" {
        p *= match (a.phenotype.as_str(), track) {
            ("photo", "solar") => 1.25,
            ("photo", "detritus") => 0.9,
            ("photo", _) => 0.85,
            ("hetero", "solar") => 0.8,
            ("hetero", "detritus") => 1.1,
            ("hetero", _) => 1.2,
            _ => 1.0,
        };
        if a.has("chloroplast") && a.phagocytic() {
            p *= 1.1;
        }
        if track == "solar" && a.has("eyespot") {
            p *= 1.1;
        }
    }
    if a.specimen == "vorticella" {
        p *= if a.has("stalk") { 1.35 } else { 0.85 };
    }
    if a.specimen == "paramecium" && a.has("macronucleus") {
        p *= 1.05;
    }
    if a.specimen == "amoeba" {
        p *= if track == "bacterivore" {
            1.2
        } else if track == "detritus" {
            1.1
        } else {
            1.0
        };
    }
    if a.specimen == "stentor" && a.has("holdfast") {
        p *= 1.35;
    }
    if a.specimen == "diatom" && track == "solar" {
        p *= 1.2;
    }
    if a.specimen == "diatom" && a.has("frustule") {
        p *= 1.03;
    }
    if a.specimen == "didinium" && track == "bacterivore" {
        p *= 1.25;
    }
    p *= 1.0 + a.course_count("bc") as f64 * 0.04;
    p *= 1.0 + a.signaling * 0.004;
    if a.stress >= 70.0 {
        p *= 0.85;
    }
    p.round() as u32
}
pub fn promotion_lock(a: &Actor, track: &str) -> Option<String> {
    let p = &a.careers[track];
    let next = p.level as usize + 1;
    if next == 1 {
        return Some("Work a shift first".into());
    }
    if next > 4 {
        return Some("Already Apex".into());
    }
    if p.exp < [0, 0, 30, 90, 180][next] {
        return Some(format!("Needs {} experience", [0, 0, 30, 90, 180][next]));
    }
    if a.transcription < [0, 0, 1, 2, 3][next] {
        return Some(format!(
            "Needs transcription tier {}",
            [0, 0, 1, 2, 3][next]
        ));
    }
    if next >= 4 && a.course_count("gr") == 0 {
        return Some("Needs Genetic Regulation course".into());
    }
    None
}
pub fn quest_value(c: &Content, a: &Actor, q: &Quest) -> f64 {
    match q.kind.as_str() {
        "size" => a.size,
        "earned" => a.total_earned as f64,
        "career" => a.careers.values().map(|x| x.level).max().unwrap_or(0) as f64,
        "organelles" => a.synthesized as f64,
        "organelle" => {
            if a.has(q.param.as_deref().unwrap_or("")) {
                q.target
            } else {
                0.0
            }
        }
        "transcription" => a.transcription as f64,
        "phase" => match a.phase.as_str() {
            "S" => 1.0,
            "G2" => 2.0,
            "M" => 3.0,
            _ => 0.0,
        },
        "signature" => {
            let sig = &c.specimens[&a.specimen].signature;
            if sig.iter().all(|s| a.has(s)) {
                q.target
            } else {
                0.0
            }
        }
        "homeostasis" => a.homeo_streak as f64,
        "pathways" => a.careers.values().filter(|x| x.shifts > 0).count() as f64,
        "mitosis" if a.divided => q.target,
        _ => 0.0,
    }
}
pub fn refresh(c: &Content, a: &mut Actor, scenario: &str) {
    for q in &c.scenarios[scenario].quests {
        if !a.done.contains(&q.id) && quest_value(c, a, q) >= q.target {
            a.done.push(q.id.clone());
        }
    }
}
pub fn all_done(c: &Content, a: &Actor, scenario: &str) -> bool {
    c.scenarios[scenario]
        .quests
        .iter()
        .all(|q| a.done.contains(&q.id))
}
