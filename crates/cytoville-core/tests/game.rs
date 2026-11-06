// SPDX-License-Identifier: AGPL-3.0-or-later
use cytoville_core::{Command, Content, actions, apply, budget, package::Package, rules, turn};
use serde_json::Value;

#[test]
fn matches_original_browser_for_63_starts_and_378_commands() {
    let fixtures: Vec<Value> = serde_json::from_str(include_str!(
        "../../../verification/fixtures/browser-oracle.json"
    ))
    .unwrap();
    let c = Content::builtin();
    for f in fixtures {
        let mut g = turn::new_game(
            &c,
            f["specimen"].as_str().unwrap(),
            f["scenario"].as_str().unwrap(),
            f["difficulty"].as_str().unwrap(),
            7,
            false,
        )
        .unwrap();
        assert_eq!(g.weather, f["weather"]);
        assert_eq!(g.forecast, f["forecast"]);
        compare(&serde_json::to_value(&g.actors[0]).unwrap(), &f["initial"]);
        for step in f["steps"].as_array().unwrap() {
            let cmd: Command = serde_json::from_value(step["command"].clone()).unwrap();
            let result = apply(&c, &g, 0, &cmd);
            assert_eq!(result.is_ok(), step["ok"].as_bool().unwrap());
            if let Ok(next) = result {
                g = next;
            }
            compare(&serde_json::to_value(&g.actors[0]).unwrap(), &step["actor"]);
        }
    }
}
fn compare(actual: &Value, expected: &Value) {
    for (key, value) in actual.as_object().unwrap() {
        if value.is_number() {
            assert_eq!(value.as_f64(), expected[key].as_f64(), "actor field {key}");
        } else {
            assert_eq!(value, &expected[key], "actor field {key}");
        }
    }
}
#[test]
fn invalid_commands_are_atomic_and_cannot_bypass_event_requirements() {
    let c = Content::builtin();
    let mut g = turn::new_game(&c, "euglena", "fresh-daughter", "standard", 7, true).unwrap();
    let before = g.clone();
    assert!(apply(&c, &g, 1, &Command::EndTurn).is_err());
    assert_eq!(g, before);
    assert!(
        apply(
            &c,
            &g,
            0,
            &Command::Act {
                id: "cp:mitosis".into()
            }
        )
        .is_err()
    );
    let event = c
        .events
        .iter()
        .find(|e| {
            e.choices
                .iter()
                .any(|x| turn::choice_lock(&g.actors[0], x).is_some())
        })
        .unwrap();
    g.pending_event = Some(event.id.clone());
    let choice = event
        .choices
        .iter()
        .find(|x| turn::choice_lock(&g.actors[0], x).is_some())
        .unwrap();
    assert!(
        apply(
            &c,
            &g,
            0,
            &Command::Choose {
                id: choice.id.clone()
            }
        )
        .is_err()
    );
    assert!(apply(&c, &g, 0, &Command::EndTurn).is_err());
}
#[test]
fn every_available_action_in_every_district_preserves_actor_bounds() {
    let c = Content::builtin();
    for sp in &c.specimen_order {
        for sc in &c.scenario_order {
            let mut g = turn::new_game(&c, sp, sc, "standard", 42, true).unwrap();
            g.actors[0].biomass = 10000;
            g.actors[0].atp = 900;
            g.actors[0].transcription = 4;
            g.actors[0].size = 90.0;
            g.actors[0].motility = 80.0;
            g.actors[0].integrity = 80.0;
            g.actors[0].stress = 60.0;
            for loc in &c.location_order {
                let mut base = g.clone();
                base.actors[0].location = loc.clone();
                for action in actions::list(&c, &base, 0, loc) {
                    if action.enabled() {
                        let next = apply(
                            &c,
                            &base,
                            0,
                            &Command::Act {
                                id: action.id.clone(),
                            },
                        )
                        .unwrap();
                        next.validate(&c).unwrap();
                        assert!(next.actors[0].atp <= base.actors[0].atp, "{}", action.id);
                    }
                }
            }
        }
    }
}
#[test]
fn mitosis_requires_replication_and_every_other_goal() {
    let c = Content::builtin();
    let mut g = turn::new_game(&c, "didinium", "full-cycle", "standard", 7, true).unwrap();
    let a = &mut g.actors[0];
    a.location = "checkpoint".into();
    a.atp = 100;
    a.biomass = 1000;
    a.size = 100.0;
    a.transcription = 4;
    assert!(
        apply(
            &c,
            &g,
            0,
            &Command::Act {
                id: "cp:mitosis".into()
            }
        )
        .is_err()
    );
    g = apply(
        &c,
        &g,
        0,
        &Command::Act {
            id: "cp:g1s".into(),
        },
    )
    .unwrap();
    for _ in 0..3 {
        g.actors[0].atp = 100;
        g = apply(
            &c,
            &g,
            0,
            &Command::Act {
                id: "cp:sphase".into(),
            },
        )
        .unwrap();
    }
    assert_eq!(g.actors[0].phase, "G2");
    assert!(
        apply(
            &c,
            &g,
            0,
            &Command::Act {
                id: "cp:mitosis".into()
            }
        )
        .is_err()
    );
    g.actors[0].done = c.scenarios["full-cycle"]
        .quests
        .iter()
        .filter(|q| q.kind != "mitosis")
        .map(|q| q.id.clone())
        .collect();
    g.actors[0].atp = 100;
    g = apply(
        &c,
        &g,
        0,
        &Command::Act {
            id: "cp:mitosis".into(),
        },
    )
    .unwrap();
    assert_eq!(g.winner, Some(0));
    assert!(g.actors[0].divided);
}
#[test]
fn predation_cannot_create_negative_biomass() {
    let c = Content::builtin();
    let mut g = turn::new_game(&c, "didinium", "plasticity", "standard", 7, true).unwrap();
    g.actors[0].location = "rift".into();
    g.actors[0].size = 100.0;
    g.actors[1].biomass = 0;
    g.actors[0].organelles.push("lysosome".into());
    g.actors[0].organelles.push("pseudopod".into());
    let next = apply(
        &c,
        &g,
        0,
        &Command::Act {
            id: "rift:prey".into(),
        },
    )
    .unwrap();
    assert_eq!(next.actors[1].biomass, 0);
    assert_eq!(next.actors[0].biomass, g.actors[0].biomass);
}
#[test]
fn resource_primitive_and_encystment_boundaries() {
    assert_eq!(budget::spend(0, 1), None);
    assert_eq!(budget::spend(u32::MAX, u32::MAX), Some(0));
    let c = Content::builtin();
    let mut g = turn::new_game(&c, "euglena", "fresh-daughter", "standard", 0, false).unwrap();
    rules::stress(&mut g.actors[0], 200.0);
    assert!(rules::encyst(&mut g.actors[0]));
    assert_eq!(g.actors[0].stress, 60.0);
    assert!(!rules::encyst(&mut g.actors[0]));
}
#[test]
fn mod_packages_are_validated_and_tampering_is_rejected() {
    let data: Value =
        serde_json::from_str(include_str!("../../../assets/content/cytoville.json")).unwrap();
    let mut p = Package::compile(data).unwrap();
    p.load().unwrap();
    p.content["SCENARIOS"]["fresh-daughter"]["turnLimit"] = serde_json::json!(0);
    assert!(p.load().is_err());
    assert!(Package::compile(p.content).is_err());
}
