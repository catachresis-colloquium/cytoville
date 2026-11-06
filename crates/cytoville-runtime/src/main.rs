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
        json!({"game":rt.game,"content":rt.content,"actions":actions::list(&rt.content,&rt.game,rt.game.active,&rt.game.actors[rt.game.active].location),"nativeDecisions":rt.ai_tick}),
    )
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
