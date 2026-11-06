// SPDX-License-Identifier: AGPL-3.0-or-later
//! Deterministic game rules. No renderer, wall clock, network or native pointers.
#![forbid(unsafe_code)]
pub mod actions;
pub use cytoville_budget as budget;
pub mod content;
pub mod package;
pub mod rules;
pub mod turn;
pub mod types;
pub use content::Content;
pub use types::*;

/// Mulberry32: the original archive's wrapping 32-bit generator.
pub fn rand(g: &mut Game) -> f64 {
    g.rng_state = g.rng_state.wrapping_add(0x6d2b79f5);
    let mut t = g.rng_state;
    t = (t ^ (t >> 15)).wrapping_mul(t | 1);
    t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
    f64::from(t ^ (t >> 14)) / 4294967296.0
}

pub fn apply(
    content: &Content,
    game: &Game,
    who: usize,
    command: &Command,
) -> Result<Game, String> {
    if who > 1 {
        return Err("Unknown seat".into());
    }
    if game.status != "playing" {
        return Err("The scenario has ended".into());
    }
    if who != game.active {
        return Err("Wait for your turn".into());
    }
    if game.pending_event.is_some() && !matches!(command, Command::Choose { .. }) {
        return Err("Resolve the encounter first".into());
    }
    let mut next = game.clone();
    match command {
        Command::Travel { location } => actions::travel(content, &mut next, who, location)?,
        Command::Act { id } => actions::execute(content, &mut next, who, id)?,
        Command::Choose { id } => turn::choose(content, &mut next, who, id)?,
        Command::EndTurn => turn::end_seat(content, &mut next, who)?,
        Command::Customize {
            name,
            membrane,
            accent,
            pattern,
        } => {
            if name.trim().is_empty()
                || name.chars().count() > 40
                || !["plain", "spots", "stripes", "glow"].contains(&pattern.as_str())
            {
                return Err("Invalid cell appearance".into());
            }
            let valid_color = |s: &str| {
                s.len() == 7 && s.starts_with('#') && s[1..].bytes().all(|b| b.is_ascii_hexdigit())
            };
            if !valid_color(membrane) || !valid_color(accent) {
                return Err("Use #RRGGBB pigments".into());
            }
            next.actors[who].name = name.clone();
            next.looks[who] = Look {
                name: name.clone(),
                membrane: membrane.clone(),
                accent: accent.clone(),
                pattern: pattern.clone(),
            };
        }
    }
    turn::victory(content, &mut next);
    next.revision = next.revision.checked_add(1).ok_or("Revision exhausted")?;
    next.validate(content)?;
    Ok(next)
}
