// SPDX-License-Identifier: AGPL-3.0-or-later
use cytoville_core::{Command, Content, turn};
use cytoville_runtime::RivalSettings;
use cytoville_runtime::Runtime;

#[test]
fn older_saves_and_incremental_states_keep_the_original_rival() {
    let original = Runtime::new(
        Content::builtin(),
        "euglena",
        "fresh-daughter",
        "standard",
        7,
        false,
    )
    .unwrap();
    let mut value = serde_json::to_value(&original).unwrap();
    value.as_object_mut().unwrap().remove("rival_settings");
    let mut restored: Runtime = serde_json::from_value(value).unwrap();
    restored.validate().unwrap();
    assert_eq!(restored.rival_settings, RivalSettings::default());
    let mut state = serde_json::to_value(original.state()).unwrap();
    state.as_object_mut().unwrap().remove("rival_settings");
    restored
        .apply_state(serde_json::from_value(state).unwrap())
        .unwrap();
    restored.dispatch(0, &Command::EndTurn).unwrap();
    assert!(restored.ai_tick > 0);
}

#[test]
fn baseline_rival_and_native_rival_are_distinct_and_configuration_survives_save() {
    let mut baseline = Runtime::new(
        Content::builtin(),
        "euglena",
        "fresh-daughter",
        "standard",
        7,
        false,
    )
    .unwrap();
    baseline
        .configure_rival(RivalSettings {
            inject_weevil: false,
            capability: 0,
            name: Some("Ada".into()),
        })
        .unwrap();
    baseline.dispatch(0, &Command::EndTurn).unwrap();
    assert_eq!(baseline.game.actors[1].name, "Ada");
    assert_eq!(
        baseline.ai_tick, 0,
        "baseline rival called the native engine"
    );
    assert!(
        baseline
            .trace
            .tropes
            .iter()
            .any(|t| t.kind == "baseline-intent")
    );
    assert!(
        !baseline
            .trace
            .tropes
            .iter()
            .any(|t| t.kind == "weevil-intent")
    );
    let restored: Runtime =
        serde_json::from_slice(&serde_json::to_vec(&baseline).unwrap()).unwrap();
    restored.validate().unwrap();
    assert_eq!(restored.rival_settings, baseline.rival_settings);
    assert!(baseline.configure_rival(RivalSettings::default()).is_err());
}

#[test]
fn invalid_ai_configuration_is_rejected_before_indexing_or_mutation() {
    let mut runtime = Runtime::new(
        Content::builtin(),
        "euglena",
        "fresh-daughter",
        "standard",
        7,
        false,
    )
    .unwrap();
    let before = runtime.game.clone();
    assert!(
        runtime
            .configure_rival(RivalSettings {
                capability: 255,
                ..Default::default()
            })
            .is_err()
    );
    assert!(
        runtime
            .configure_rival(RivalSettings {
                name: Some("\n".into()),
                ..Default::default()
            })
            .is_err()
    );
    assert_eq!(runtime.game, before);
    runtime.rival_settings.capability = 255;
    assert!(runtime.dispatch(0, &Command::EndTurn).is_err());
    assert_eq!(runtime.game, before);
}
fn clear_event(r: &mut Runtime) {
    if let Some(id) = &r.game.pending_event {
        let e = r.content.events.iter().find(|e| &e.id == id).unwrap();
        let choice = e
            .choices
            .iter()
            .find(|c| turn::choice_lock(&r.game.actors[0], c).is_none())
            .unwrap();
        r.dispatch(
            0,
            &Command::Choose {
                id: choice.id.clone(),
            },
        )
        .unwrap();
    }
}
#[test]
fn real_kernel_replays_and_save_resume_preserves_memory_and_trace() {
    let c = Content::builtin();
    let mut a = Runtime::new(c, "euglena", "full-cycle", "standard", 7, false).unwrap();
    let mut b = a.clone();
    for _ in 0..8 {
        for r in [&mut a, &mut b] {
            clear_event(r);
            r.dispatch(0, &Command::EndTurn).unwrap();
        }
        b = serde_json::from_slice(&serde_json::to_vec(&b).unwrap()).unwrap();
        b.validate().unwrap();
        assert_eq!(a.game, b.game);
        assert_eq!(a.memory, b.memory);
        assert_eq!(a.trace, b.trace);
    }
    assert!(a.ai_tick > 0);
    assert!(!a.trace.events.is_empty());
    a.trace.validate().unwrap();
}
#[test]
fn all_specimens_and_scenarios_finish_without_invalid_state() {
    for sp in Content::builtin().specimen_order {
        for sc in Content::builtin().scenario_order {
            let mut r = Runtime::new(Content::builtin(), &sp, &sc, "standard", 29, false).unwrap();
            for _ in 0..60 {
                if r.game.status != "playing" {
                    break;
                }
                clear_event(&mut r);
                if r.game.status == "playing" {
                    r.dispatch(0, &Command::EndTurn).unwrap();
                }
            }
            assert_ne!(r.game.status, "playing");
            r.validate().unwrap();
        }
    }
}
#[test]
fn multiplayer_does_not_run_ai_and_hands_off_seats() {
    let mut r = Runtime::new(
        Content::builtin(),
        "amoeba",
        "plasticity",
        "standard",
        7,
        true,
    )
    .unwrap();
    r.dispatch(0, &Command::EndTurn).unwrap();
    assert_eq!(r.game.active, 1);
    assert_eq!(r.ai_tick, 0);
    let before = r.game.clone();
    assert!(r.dispatch(0, &Command::EndTurn).is_err());
    assert_eq!(before, r.game);
    r.dispatch(1, &Command::EndTurn).unwrap();
    assert_eq!(r.game.turn, 2);
    assert_eq!(r.game.active, 0);
    assert_eq!(r.ai_tick, 0);
}
