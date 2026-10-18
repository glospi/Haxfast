//! Page ↔ host bridge: local WebSocket on 127.0.0.1 with an ephemeral port and token.
//! The host injects the address into the page (`window.__HAXFAST_BOOT`).
//!
//! Methods: `hello`, `prefs.get`, `prefs.set`, `log`, `metrics.fps`.

use std::io::Read as _;
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;

use tungstenite::{Message, WebSocket};

use crate::log;
use crate::paths;

#[derive(Clone)]
pub struct BridgeInfo {
    pub port: u16,
    pub token: String,
}

impl BridgeInfo {
    pub fn ws_url(&self) -> String {
        format!("ws://127.0.0.1:{}/?token={}", self.port, self.token)
    }
}

/// Starts the local server and returns where the page must connect to.
pub fn start() -> std::io::Result<BridgeInfo> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let info = BridgeInfo {
        port: listener.local_addr()?.port(),
        token: random_token(),
    };

    let token = Arc::new(info.token.clone());
    std::thread::Builder::new()
        .name("haxfast-bridge".into())
        .spawn(move || {
            for stream in listener.incoming().flatten() {
                let token = token.clone();
                // Expected connections: one (the page). A thread per connection is enough.
                std::thread::spawn(move || match serve(stream, &token) {
                    Ok(reason) => log::info(&format!("bridge: connection closed ({reason})")),
                    Err(error) => log::warn(&format!("bridge: connection error: {error}")),
                });
            }
        })?;

    Ok(info)
}

fn serve(stream: TcpStream, expected_token: &str) -> Result<String, String> {
    let mut socket = tungstenite::accept(stream).map_err(|e| e.to_string())?;

    // Mandatory first message: {"method":"hello","params":{"token":"…"}}
    let hello = read_json(&mut socket)?;
    let token_ok = hello.get("method").and_then(|v| v.as_str()) == Some("hello")
        && hello
            .get("params")
            .and_then(|v| v.get("token"))
            .and_then(|v| v.as_str())
            == Some(expected_token);
    if !token_ok {
        let _ = socket.close(None);
        return Err("invalid token".into());
    }

    loop {
        let request = match read_json(&mut socket) {
            Ok(request) => request,
            Err(reason) => return Ok(reason),
        };

        let method = request
            .get("method")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let params = request.get("params").cloned().unwrap_or(serde_json::Value::Null);

        let result = dispatch(&method, params);
        if let Some(id) = request.get("id") {
            let reply = match result {
                Ok(value) => serde_json::json!({ "id": id, "result": value }),
                Err(error) => serde_json::json!({ "id": id, "error": error }),
            };
            send(&mut socket, &reply)?;
        }
    }
}

fn dispatch(method: &str, params: serde_json::Value) -> Result<serde_json::Value, String> {
    match method {
        "hello" => Ok(serde_json::json!({
            "ok": true,
            "version": haxfast_core::version(),
        })),

        "prefs.get" => {
            let path = paths::prefs_file();
            let (prefs, warning) = haxfast_core::prefs::Preferences::load(&path);
            if let Some(warning) = warning {
                log::warn(&format!("prefs: {warning}"));
            }
            serde_json::to_value(prefs).map_err(|e| e.to_string())
        }

        "prefs.set" => {
            let prefs: haxfast_core::prefs::Preferences =
                serde_json::from_value(params).map_err(|e| format!("invalid prefs: {e}"))?;
            prefs.validate()?;
            prefs
                .save(&paths::prefs_file())
                .map_err(|e| e.to_string())?;
            log::info("prefs: saved from the page");
            Ok(serde_json::json!({ "ok": true }))
        }

        "log" => {
            let message = params
                .get("msg")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            match params.get("data").filter(|v| !v.is_null()) {
                Some(data) => log::info(&format!("[page] {message} {data}")),
                None => log::info(&format!("[page] {message}")),
            }
            Ok(serde_json::Value::Null)
        }

        "metrics.fps" => {
            log::info(&format!("[page] fps {params}"));
            Ok(serde_json::Value::Null)
        }

        other => Err(format!("unknown method: {other}")),
    }
}

/// Reads the next JSON text message. `Err` only when the connection is over.
fn read_json(socket: &mut WebSocket<TcpStream>) -> Result<serde_json::Value, String> {
    loop {
        let message = socket.read().map_err(|e| e.to_string())?;
        if message.is_close() {
            return Err("the page closed the connection".into());
        }
        let Ok(text) = message.to_text() else {
            continue;
        };
        match serde_json::from_str(text) {
            Ok(value) => return Ok(value),
            Err(_) => continue, // unreadable message: ignore it
        }
    }
}

fn send(socket: &mut WebSocket<TcpStream>, value: &serde_json::Value) -> Result<(), String> {
    socket
        .send(Message::Text(value.to_string().into()))
        .map_err(|e| e.to_string())
}

/// Random 16-byte token. No `rand` because the only requirement is that it can't
/// be easily guessed from another local process; if /dev/urandom fails it falls
/// back to clock+pid.
fn random_token() -> String {
    let mut bytes = [0u8; 16];
    if std::fs::File::open("/dev/urandom")
        .and_then(|mut file| file.read_exact(&mut bytes))
        .is_err()
    {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);
        bytes[..8].copy_from_slice(&nanos.to_le_bytes());
        bytes[8..12].copy_from_slice(&std::process::id().to_le_bytes());
    }
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
