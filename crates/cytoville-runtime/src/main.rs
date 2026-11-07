// SPDX-License-Identifier: AGPL-3.0-or-later
use cytoville_core::{Command, Content, actions};
use cytoville_runtime::{Runtime, content_id as fingerprint};
use serde::{Deserialize, Serialize};
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

#[derive(Serialize)]
#[serde(untagged)]
enum Reply {
    Success { data: Value, ok: bool },
    Failure { error: String, ok: bool },
}
fn respond(
    r: Request,
    runtime: &mut Option<Runtime>,
    content: &mut Content,
    content_id: &mut String,
) -> Result<Value, String> {
    let new_content = matches!(&r, Request::New { .. } | Request::Load { .. });
    let next = match r {
        Request::State => {
            return checked_response(snapshot(runtime.as_ref().ok_or("No game")?, content_id));
        }
        Request::Save => {
            return checked_response(
                serde_json::to_value(runtime.as_ref().ok_or("No game")?)
                    .map_err(|e| e.to_string())?,
            );
        }
        Request::Content { content: new } => {
            new.validate()?;
            let value = checked_response(json!({"content":"accepted"}))?;
            *content = *new;
            return Ok(value);
        }
        Request::New {
            specimen,
            scenario,
            difficulty,
            seed,
            multiplayer,
        } => Runtime::new(
            content.clone(),
            &specimen,
            &scenario,
            &difficulty,
            seed,
            multiplayer,
        )?,
        Request::Load { save } => {
            save.validate()?;
            *save
        }
        Request::Command {
            seat,
            expected_revision,
            command,
        } => {
            let current = runtime.as_ref().ok_or("Start a game first")?;
            if current.game.revision != expected_revision {
                return Err("Stale revision".into());
            }
            let mut next = current.clone();
            next.dispatch(seat, &command)?;
            next
        }
    };
    // Only mutations need a staged runtime. Polls and saves never clone either
    // the current runtime or the standalone content used to start new sessions.
    let next_id = if new_content {
        fingerprint(&next.content)?
    } else {
        content_id.clone()
    };
    let value = checked_response(snapshot(&next, &next_id))?;
    *runtime = Some(next);
    *content_id = next_id;
    Ok(value)
}

fn snapshot(rt: &Runtime, content_id: &str) -> Value {
    json!({"contentId":content_id,"runtime":rt,"actions":actions::list(&rt.content,&rt.game,rt.game.active,&rt.game.actors[rt.game.active].location),"nativeDecisions":rt.ai_tick})
}

fn checked_response(value: Value) -> Result<Value, String> {
    // Count the actual JSON bytes without cloning the value or allocating an
    // encoded response buffer. Reserve 1 KiB for the host's credential envelope.
    struct SizeLimit {
        remaining: usize,
    }
    impl Write for SizeLimit {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if bytes.len() > self.remaining {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Response too large",
                ));
            }
            self.remaining -= bytes.len();
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let envelope = br#"{"ok":true,"data":null}"#.len() - 4;
    let mut limit = SizeLimit {
        remaining: 8 * 1024 * 1024 - 1024 - envelope,
    };
    serde_json::to_writer(&mut limit, &value)
        .map_err(|_| String::from("Session response exceeds transport limit"))?;
    Ok(value)
}

fn main() {
    let mut runtime = None;
    let mut content_id = String::new();
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
            .and_then(|r| respond(r, &mut runtime, &mut content, &mut content_id));
        let reply = match result {
            Ok(value) => Reply::Success {
                data: value,
                ok: true,
            },
            Err(error) => Reply::Failure { error, ok: false },
        };
        if serde_json::to_writer(&mut stdout, &reply).is_err()
            || stdout
                .write_all(b"\n")
                .and_then(|_| stdout.flush())
                .is_err()
        {
            break;
        }
    }
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
        content.specimens.get_mut("euglena").unwrap().blurb = "x".repeat(5 * 1024 * 1024);
        content.validate().unwrap();
        let mut runtime = None;
        let mut content_id = String::new();
        let reply = respond(new_game(), &mut runtime, &mut content, &mut content_id).unwrap();
        assert!(reply.get("content").is_none());
        assert!(reply.get("game").is_none());
        assert!(serde_json::to_vec(&reply).unwrap().len() < 8 * 1024 * 1024 - 1024);
        let polled = respond(Request::State, &mut runtime, &mut content, &mut content_id).unwrap();
        assert!(
            polled == reply,
            "A read-only poll must preserve the snapshot"
        );
        let saved = respond(Request::Save, &mut runtime, &mut content, &mut content_id).unwrap();
        assert!(
            saved == reply["runtime"],
            "A save must preserve the runtime"
        );
        let previous = serde_json::to_value(&runtime).unwrap();
        content.specimens.get_mut("euglena").unwrap().blurb = "x".repeat(8 * 1024 * 1024);
        assert_eq!(
            respond(new_game(), &mut runtime, &mut content, &mut content_id).unwrap_err(),
            "Session response exceeds transport limit"
        );
        assert_eq!(serde_json::to_value(&runtime).unwrap(), previous);
    }
}
