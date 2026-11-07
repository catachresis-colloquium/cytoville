// SPDX-License-Identifier: AGPL-3.0-or-later
use cytoville_core::{Command, Content, actions};
use cytoville_runtime::Runtime;
use serde::Deserialize;
use serde_json::{Value, json};
use std::io::{self, BufRead, Read, Write};

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
enum Request {
    New {
        specimen: String,
        scenario: String,
        difficulty: String,
        seed: u32,
        #[serde(default)]
        multiplayer: bool,
    },
    State,
    Command {
        seat: usize,
        expected_revision: u64,
        command: Command,
    },
    Save,
    Load {
        save: Box<Runtime>,
    },
    Content {
        content: Box<Content>,
    },
}
fn respond(
    r: Request,
    runtime: &mut Option<Runtime>,
    content: &mut Content,
) -> Result<Value, String> {
    // Reserve space for the host's join/resume envelope inside its 8 MiB
    // transport limit. A transition is acknowledged only if it can be sent.
    let mut next_runtime = runtime.clone();
    let mut next_content = content.clone();
    let value = respond_inner(r, &mut next_runtime, &mut next_content)?;
    let bytes = serde_json::to_vec(&json!({"ok":true,"data":&value})).map_err(|e| e.to_string())?;
    if bytes.len() > 8 * 1024 * 1024 - 1024 {
        return Err("Session response exceeds transport limit".into());
    }
    *runtime = next_runtime;
    *content = next_content;
    Ok(value)
}

fn respond_inner(
    r: Request,
    runtime: &mut Option<Runtime>,
    content: &mut Content,
) -> Result<Value, String> {
    match r {
        Request::New {
            specimen,
            scenario,
            difficulty,
            seed,
            multiplayer,
        } => {
            *runtime = Some(Runtime::new(
                content.clone(),
                &specimen,
                &scenario,
                &difficulty,
                seed,
                multiplayer,
            )?);
        }
        Request::Command {
            seat,
            expected_revision,
            command,
        } => {
            let rt = runtime.as_mut().ok_or("Start a game first")?;
            if rt.game.revision != expected_revision {
                return Err("Stale revision".into());
            }
            rt.dispatch(seat, &command)?;
        }
        Request::Load { save } => {
            save.validate()?;
            *runtime = Some(*save);
        }
        Request::Save => {
            return serde_json::to_value(runtime.as_ref().ok_or("No game")?)
                .map_err(|e| e.to_string());
        }
        Request::Content { content: new } => {
            new.validate()?;
            *content = *new;
            return Ok(json!({"content":"accepted"}));
        }
        Request::State => {}
    }
    let rt = runtime.as_ref().ok_or("No game")?;
    Ok(
        json!({"runtime":rt,"actions":actions::list(&rt.content,&rt.game,rt.game.active,&rt.game.actors[rt.game.active].location),"nativeDecisions":rt.ai_tick}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new_game() -> Request {
        Request::New {
            specimen: "euglena".into(),
            scenario: "fresh-daughter".into(),
            difficulty: "standard".into(),
            seed: 7,
            multiplayer: true,
        }
    }

    #[test]
    fn large_valid_content_is_sent_once_and_oversized_transitions_are_atomic() {
        let mut content = Content::builtin();
        content.specimens.get_mut("euglena").unwrap().name = "x".repeat(5 * 1024 * 1024);
        content.validate().unwrap();
        let mut runtime = None;
        let reply = respond(new_game(), &mut runtime, &mut content).unwrap();
        assert!(reply.get("content").is_none());
        assert!(reply.get("game").is_none());
        assert!(serde_json::to_vec(&reply).unwrap().len() < 8 * 1024 * 1024 - 1024);
        let previous = serde_json::to_value(&runtime).unwrap();
        content.specimens.get_mut("euglena").unwrap().name = "x".repeat(8 * 1024 * 1024);
        assert_eq!(
            respond(new_game(), &mut runtime, &mut content).unwrap_err(),
            "Session response exceeds transport limit"
        );
        assert_eq!(serde_json::to_value(&runtime).unwrap(), previous);
    }
}
fn main() {
    let mut runtime = None;
    let mut content = Content::builtin();
    if let Ok(path) = std::env::var("CYTOVILLE_PACKAGE") {
        let load = || -> Result<Content, String> {
            let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
            if bytes.len() > 8 * 1024 * 1024 {
                return Err("Package too large".into());
            }
            let p: cytoville_core::package::Package =
                serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
            p.load()
        };
        content = load().unwrap_or_else(|e| {
            eprintln!("Invalid server package: {e}");
            std::process::exit(1)
        });
    }
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let mut stdout = io::stdout().lock();
    loop {
        let mut bytes = Vec::new();
        // Bound a single request before allocation can grow without limit.
        match (&mut input)
            .take(8 * 1024 * 1024 + 1)
            .read_until(b'\n', &mut bytes)
        {
            Ok(0) => break,
            Ok(_) if bytes.len() > 8 * 1024 * 1024 => {
                let _ = writeln!(stdout, "{{\"ok\":false,\"error\":\"Request too large\"}}");
                break;
            }
            Err(_) => break,
            _ => {}
        }
        let result = serde_json::from_slice::<Request>(&bytes)
            .map_err(|e| e.to_string())
            .and_then(|r| respond(r, &mut runtime, &mut content));
        let reply = match result {
            Ok(value) => json!({"ok":true,"data":value}),
            Err(error) => json!({"ok":false,"error":error}),
        };
        if writeln!(stdout, "{reply}")
            .and_then(|_| stdout.flush())
            .is_err()
        {
            break;
        }
    }
}
