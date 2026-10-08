// Window data streams from the Ryoku shell Unix socket and updates whenever the shell emits a
// frame; broken connections retry with a short, capped backoff.
use super::WindowRef;
use serde_json::Value;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

#[derive(Default)]
struct Inner {
    connected: bool,
    received: bool,
    windows: Vec<WindowRef>,
}

#[derive(Clone)]
pub struct WindowFeed {
    inner: Arc<Mutex<Inner>>,
}

fn text(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(text)) => text.clone(),
        Some(value) if value.is_number() => value.to_string(),
        _ => String::new(),
    }
}

fn parse(line: &str) -> Option<Vec<WindowRef>> {
    let payload: Value = serde_json::from_str(line).ok()?;
    let windows = payload
        .get("windows")
        .or_else(|| payload.get("data").and_then(|data| data.get("windows")))?
        .as_array()?;

    Some(
        windows
            .iter()
            .map(|window| {
                let focus_order = window.get("focusOrder").and_then(Value::as_i64);
                WindowRef {
                    id: text(window.get("id")),
                    title: text(window.get("title")),
                    app_id: text(window.get("appId").or_else(|| window.get("app_id"))),
                    workspace: text(window.get("workspace")),
                    output: text(window.get("output")),
                    focused: window
                        .get("focused")
                        .and_then(Value::as_bool)
                        .unwrap_or(focus_order == Some(0)),
                    pid: window
                        .get("pid")
                        .and_then(Value::as_u64)
                        .map(|pid| pid as u32),
                }
            })
            .collect(),
    )
}

impl WindowFeed {
    pub fn start() -> Self {
        let inner = Arc::new(Mutex::new(Inner::default()));
        let shared = inner.clone();
        thread::Builder::new()
            .name("ryoku-window-feed".into())
            .spawn(move || {
                let mut delay = Duration::from_millis(250);
                loop {
                    let Some(runtime) = std::env::var_os("XDG_RUNTIME_DIR") else {
                        thread::sleep(Duration::from_secs(2));
                        continue;
                    };
                    let path = PathBuf::from(runtime).join("ryoku-shell.sock");
                    match UnixStream::connect(path) {
                        Ok(mut stream) => {
                            let _ = stream.set_read_timeout(Some(Duration::from_secs(30)));
                            if stream.write_all(b"subscribe wm\n").is_err() {
                                continue;
                            }
                            if let Ok(mut state) = shared.lock() {
                                state.connected = true;
                            }
                            delay = Duration::from_millis(250);
                            let mut reader = BufReader::new(stream);
                            loop {
                                let mut line = String::new();
                                match reader.read_line(&mut line) {
                                    Ok(0) => break,
                                    Ok(_) => {
                                        if let Some(windows) = parse(&line) {
                                            if let Ok(mut state) = shared.lock() {
                                                state.received = true;
                                                state.windows = windows;
                                            }
                                        }
                                    }
                                    Err(error)
                                        if error.kind() == std::io::ErrorKind::WouldBlock
                                            || error.kind() == std::io::ErrorKind::TimedOut =>
                                    {
                                        continue
                                    }
                                    Err(_) => break,
                                }
                            }
                            if let Ok(mut state) = shared.lock() {
                                state.connected = false;
                            }
                        }
                        Err(_) => {
                            thread::sleep(delay);
                            delay = (delay * 2).min(Duration::from_secs(5));
                        }
                    }
                }
            })
            .ok();
        Self { inner }
    }

    pub fn snapshot(&self) -> (String, Vec<WindowRef>) {
        let Ok(state) = self.inner.lock() else {
            return ("none".into(), Vec::new());
        };
        if state.connected && state.received {
            ("shell".into(), state.windows.clone())
        } else {
            ("none".into(), Vec::new())
        }
    }
}
