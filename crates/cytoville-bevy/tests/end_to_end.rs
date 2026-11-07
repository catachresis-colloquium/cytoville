// SPDX-License-Identifier: AGPL-3.0-or-later
//! Run through scripts/cytoville-e2e.sh against the real Elixir host.
use cytoville_bevy::network::Network;
use cytoville_core::Command;
use cytoville_runtime::Runtime;
use serde_json::json;
use std::time::{Duration, Instant};

fn await_revision(client: &Network, revision: u64) -> Runtime {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .expect("server made no progress");
        match client
            .receive
            .lock()
            .unwrap()
            .recv_timeout(remaining)
            .expect("server timeout")
        {
            Ok(state) if state.game.revision >= revision => return state,
            Ok(_) => {}
            Err(error) => panic!("network failure: {error}"),
        }
    }
}

#[test]
#[ignore = "requires real Elixir host; scripts/cytoville-e2e.sh"]
fn ums_to_elixir_to_bevy_transport_and_native_rival() {
    let address = std::env::var("CYTOVILLE_E2E_SERVER").expect("set server address");
    let create = |id: &str, multiplayer| json!({"op":"create","session":id,"specimen":"euglena","scenario":"fresh-daughter","difficulty":"standard","seed":7,"multiplayer":multiplayer});
    let (host, initial) = Network::start(address.clone(), create("e2e-human", true)).unwrap();
    assert_eq!(
        initial.content.specimens["euglena"].name,
        "UMS end-to-end cell"
    );
    let (guest, joined) = Network::start(
        address.clone(),
        json!({"op":"join","session":"e2e-human","key":host.invitation}),
    )
    .unwrap();
    assert_eq!(initial.game, joined.game);
    host.command(0, &Command::EndTurn).unwrap();
    let state = await_revision(&guest, 1);
    assert_eq!(state.game.active, 1);
    assert_eq!(state.ai_tick, 0);
    assert!(!state.trace.events.is_empty());
    let ticket = guest.resume.clone();
    drop(guest);
    let (resumed, restored) = Network::start(address.clone(), ticket["request"].clone()).unwrap();
    assert_eq!(state.trace, restored.trace);
    resumed.command(1, &Command::EndTurn).unwrap();
    let state = await_revision(&host, 2);
    assert_eq!(state.game.turn, 2);
    state.validate().unwrap();
    let (solo, _) = Network::start(address, create("e2e-native", false)).unwrap();
    solo.command(0, &Command::EndTurn).unwrap();
    let state = await_revision(&solo, 1);
    assert!(state.ai_tick > 0, "real Zig rival must execute");
    assert_eq!(state.game.active, 0);
    assert!(state.trace.events.iter().any(|e| !e.caused_by.is_empty()));
    state.validate().unwrap();
}
