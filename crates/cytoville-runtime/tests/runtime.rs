// SPDX-License-Identifier: AGPL-3.0-or-later
use cytoville_core::{Command, Content, turn};
use cytoville_runtime::Runtime;
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
