// SPDX-License-Identifier: AGPL-3.0-or-later
use cytoville_core::Command;
use cytoville_runtime::Runtime;
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Write},
    net::TcpStream,
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::Duration,
};
pub struct Network {
    pub send: Sender<Value>,
    pub receive: std::sync::Mutex<Receiver<Result<Runtime, String>>>,
    pub seat: usize,
    pub invitation: String,
    pub resume: Value,
}
fn exchange(reader: &mut BufReader<TcpStream>, request: &Value) -> Result<Value, String> {
    writeln!(reader.get_mut(), "{request}").map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    std::io::Read::take(&mut *reader, 8 * 1024 * 1024 + 1)
        .read_until(b'\n', &mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 8 * 1024 * 1024 {
        return Err("Oversized server response".into());
    }
    let reply: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    if reply["ok"] != true {
        return Err(reply["error"]
            .as_str()
            .unwrap_or("Server rejected request")
            .into());
    }
    Ok(reply["data"].clone())
}
fn connect(address: &str) -> Result<BufReader<TcpStream>, String> {
    let stream = TcpStream::connect(address).map_err(|e| e.to_string())?;
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .map_err(|e| e.to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(3)))
        .map_err(|e| e.to_string())?;
    Ok(BufReader::new(stream))
}
impl Network {
    pub fn start(address: String, request: Value) -> Result<(Self, Runtime), String> {
        let mut reader = connect(&address)?;
        let joined = exchange(&mut reader, &request)?;
        let runtime = snapshot(&joined["data"])?;
        let seat = joined["seat"].as_u64().ok_or("Missing server seat")? as usize;
        if seat > 1 {
            return Err("Invalid server seat".into());
        }
        let key = joined["resumeKey"]
            .as_str()
            .ok_or("Missing resume key")?
            .to_string();
        let invitation = joined["inviteKey"].as_str().unwrap_or("").to_string();
        let resume = json!({"op":"resume","session":request["session"],"key":key});
        let ticket = json!({"server":address,"request":resume});
        let checked_content = serde_json::to_value(&runtime.content).map_err(|e| e.to_string())?;
        let (send, commands) = mpsc::channel::<Value>();
        let (events, receive) = mpsc::channel();
        thread::spawn(move || {
            loop {
                let request = match commands.recv_timeout(Duration::from_millis(500)) {
                    Ok(r) => r,
                    Err(mpsc::RecvTimeoutError::Timeout) => json!({"op":"state"}),
                    Err(_) => break,
                };
                match exchange(&mut reader, &request).and_then(|v| {
                    let state = snapshot(&v)?;
                    if serde_json::to_value(&state.content).map_err(|e| e.to_string())?
                        != checked_content
                    {
                        return Err("Server changed session content".into());
                    }
                    Ok(state)
                }) {
                    Ok(game) => {
                        if events.send(Ok(game)).is_err() {
                            break;
                        }
                    }
                    Err(error) => {
                        if events.send(Err(error)).is_err() {
                            break;
                        }
                        // Resume the seat, without replaying a command whose ack was lost.
                        if let Ok(mut new_reader) = connect(&address)
                            && let Ok(v) = exchange(&mut new_reader, &resume)
                            && let Ok(state) = snapshot(&v["data"])
                            && serde_json::to_value(&state.content).ok().as_ref()
                                == Some(&checked_content)
                        {
                            let _ = events.send(Ok(state));
                            reader = new_reader;
                        }
                    }
                }
            }
        });
        Ok((
            Self {
                send,
                receive: std::sync::Mutex::new(receive),
                seat,
                invitation,
                resume: ticket,
            },
            runtime,
        ))
    }
    pub fn command(&self, revision: u64, command: &Command) -> Result<(), String> {
        self.send
            .send(json!({"op":"command","expected_revision":revision,"command":command}))
            .map_err(|e| e.to_string())
    }
}

fn snapshot(data: &Value) -> Result<Runtime, String> {
    let runtime: Runtime =
        serde_json::from_value(data["runtime"].clone()).map_err(|e| e.to_string())?;
    runtime.validate()?;
    Ok(runtime)
}
