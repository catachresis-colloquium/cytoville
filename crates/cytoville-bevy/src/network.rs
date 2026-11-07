// SPDX-License-Identifier: AGPL-3.0-or-later
use cytoville_core::{Command, Content};
use cytoville_runtime::{Runtime, RuntimeState, content_id};
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::{TcpStream, ToSocketAddrs},
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::Duration,
};
pub struct Network {
    pub send: Sender<Value>,
    pub receive: std::sync::Mutex<Receiver<Result<RuntimeState, String>>>,
    pub seat: usize,
    pub invitation: String,
    pub resume: Value,
}
trait Connection: Read + Write + Send {}
impl<T: Read + Write + Send> Connection for T {}
type Reader = BufReader<Box<dyn Connection>>;
fn exchange(reader: &mut Reader, request: &Value) -> Result<Value, String> {
    writeln!(reader.get_mut(), "{request}").map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    std::io::Read::take(&mut *reader, 8 * 1024 * 1024 + 1)
        .read_until(b'\n', &mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 8 * 1024 * 1024 {
        return Err("Oversized server response".into());
    }
    let mut reply: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    if reply["ok"] != true {
        return Err(reply["error"]
            .as_str()
            .unwrap_or("Server rejected request")
            .into());
    }
    Ok(reply["data"].take())
}
fn connect(address: &str) -> Result<Reader, String> {
    let tls = address.starts_with("tls://");
    let endpoint = address.strip_prefix("tls://").unwrap_or(address);
    let addresses: Vec<_> = endpoint
        .to_socket_addrs()
        .map_err(|e| e.to_string())?
        .collect();
    if !tls && addresses.iter().any(|a| !a.ip().is_loopback()) {
        return Err("Remote multiplayer requires tls://".into());
    }
    let stream = addresses
        .iter()
        .find_map(|a| TcpStream::connect_timeout(a, Duration::from_secs(3)).ok())
        .ok_or("Cannot connect to server")?;
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .map_err(|e| e.to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(3)))
        .map_err(|e| e.to_string())?;
    if tls {
        let host = endpoint
            .rsplit_once(':')
            .ok_or("Missing TLS port")?
            .0
            .trim_matches(['[', ']']);
        let mut builder = native_tls::TlsConnector::builder();
        builder.min_protocol_version(Some(native_tls::Protocol::Tlsv12));
        if let Ok(path) = std::env::var("CYTOVILLE_CA_CERT") {
            let pem = std::fs::read(path).map_err(|e| e.to_string())?;
            builder.add_root_certificate(
                native_tls::Certificate::from_pem(&pem).map_err(|e| e.to_string())?,
            );
        }
        let connector = builder.build().map_err(|e| e.to_string())?;
        let secured = connector
            .connect(host, stream)
            .map_err(|e| format!("TLS handshake failed: {e}"))?;
        Ok(BufReader::new(Box::new(secured)))
    } else {
        Ok(BufReader::new(Box::new(stream)))
    }
}
impl Network {
    pub fn start(address: String, request: Value) -> Result<(Self, Runtime), String> {
        let mut reader = connect(&address)?;
        let mut joined = exchange(&mut reader, &request)?;
        let (runtime, checked_id) = initial_snapshot(joined["data"].take())?;
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
        let checked_content = runtime.content.clone();
        let (send, commands) = mpsc::channel::<Value>();
        let (events, receive) = mpsc::channel();
        thread::spawn(move || {
            loop {
                let request = match commands.recv_timeout(Duration::from_millis(500)) {
                    Ok(r) => r,
                    Err(mpsc::RecvTimeoutError::Timeout) => json!({"op":"state"}),
                    Err(_) => break,
                };
                match exchange(&mut reader, &request)
                    .and_then(|v| snapshot(v, &checked_content, &checked_id))
                {
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
                            && let Ok(mut v) = exchange(&mut new_reader, &resume)
                            && let Ok(state) =
                                snapshot(v["data"].take(), &checked_content, &checked_id)
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

fn initial_snapshot(mut data: Value) -> Result<(Runtime, String), String> {
    let id = data["contentId"]
        .as_str()
        .ok_or("Missing content fingerprint")?
        .to_string();
    let runtime: Runtime =
        serde_json::from_value(data["runtime"].take()).map_err(|e| e.to_string())?;
    runtime.validate()?;
    if content_id(&runtime.content)? != id {
        return Err("Invalid session content fingerprint".into());
    }
    Ok((runtime, id))
}

fn snapshot(mut data: Value, content: &Content, expected_id: &str) -> Result<RuntimeState, String> {
    if data["contentId"].as_str() != Some(expected_id) {
        return Err("Server changed session content".into());
    }
    let mut body = data["runtime"].take();
    if let Some(received) = body
        .as_object_mut()
        .ok_or("Missing runtime state")?
        .remove("content")
    {
        let received: Content = serde_json::from_value(received).map_err(|e| e.to_string())?;
        if content_id(&received)? != expected_id {
            return Err("Server changed session content".into());
        }
    }
    let state: RuntimeState = serde_json::from_value(body).map_err(|e| e.to_string())?;
    state.validate(content)?;
    Ok(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn incremental_states_require_the_handshake_content_fingerprint() {
        let runtime = Runtime::new(
            Content::builtin(),
            "euglena",
            "fresh-daughter",
            "standard",
            7,
            true,
        )
        .unwrap();
        let id = content_id(&runtime.content).unwrap();
        let thin = json!({"contentId": id, "runtime": runtime.state()});
        let state = snapshot(thin.clone(), &runtime.content, &id).unwrap();
        assert_eq!(state.game, runtime.game);
        assert_eq!(state.trace, runtime.trace);
        let mut wrong = thin;
        wrong["contentId"] = json!("wrong");
        assert!(snapshot(wrong, &runtime.content, &id).is_err());
        let mut full = json!({"contentId": id, "runtime": runtime});
        full["runtime"]["content"]["SPECIMENS"]["euglena"]["blurb"] = json!("changed");
        assert!(snapshot(full, &runtime.content, &id).is_err());
    }
}
