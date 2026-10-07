use serde::Serialize;
use serde_json::Value;
use std::{path::PathBuf, sync::Mutex};
use sysinfo::{
    CpuRefreshKind, MemoryRefreshKind, ProcessRefreshKind, RefreshKind, System, UpdateKind,
};

struct AppState {
    system: Mutex<System>,
}

fn live_refresh_kind() -> RefreshKind {
    RefreshKind::nothing()
        .with_cpu(CpuRefreshKind::nothing().with_cpu_usage())
        .with_memory(MemoryRefreshKind::everything())
        .with_processes(
            ProcessRefreshKind::nothing()
                .with_cpu()
                .with_memory()
                .with_cmd(UpdateKind::OnlyIfNotSet)
                .without_tasks(),
        )
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProcessInfo {
    pid: u32,
    parent_pid: Option<u32>,
    name: String,
    command: String,
    status: String,
    cpu: f32,
    memory: u64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SystemInfo {
    hostname: String,
    os: String,
    kernel: String,
    cpu_model: String,
    logical_cpus: usize,
    physical_cores: usize,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Snapshot {
    timestamp_ms: u128,
    accent: String,
    cpu: f32,
    total_memory: u64,
    used_memory: u64,
    total_swap: u64,
    used_swap: u64,
    process_count: usize,
    processes: Vec<ProcessInfo>,
    system: SystemInfo,
}


fn ryoku_palette_path() -> Option<PathBuf> {
    if let Some(cache_home) = std::env::var_os("XDG_CACHE_HOME") {
        return Some(PathBuf::from(cache_home).join("ryoku/colors.json"));
    }

    std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join(".cache/ryoku/colors.json"))
}

fn ryoku_accent() -> String {
    const FALLBACK: &str = "#7898a5";

    let Some(path) = ryoku_palette_path() else {
        return FALLBACK.into();
    };

    let Ok(raw) = std::fs::read_to_string(path) else {
        return FALLBACK.into();
    };

    let Ok(palette) = serde_json::from_str::<Value>(&raw) else {
        return FALLBACK.into();
    };

    palette
        .get("primary")
        .and_then(Value::as_str)
        .filter(|value| {
            value.len() == 7
                && value.starts_with('#')
                && value[1..].chars().all(|c| c.is_ascii_hexdigit())
        })
        .unwrap_or(FALLBACK)
        .to_string()
}

#[tauri::command]
fn snapshot(state: tauri::State<'_, AppState>) -> Snapshot {
    let mut system = state.system.lock().expect("system collector lock poisoned");
    system.refresh_specifics(live_refresh_kind());

    let mut processes: Vec<ProcessInfo> = system
        .processes()
        .values()
        .map(|process| ProcessInfo {
            pid: process.pid().as_u32(),
            parent_pid: process.parent().map(|pid| pid.as_u32()),
            name: process.name().to_string_lossy().into_owned(),
            command: process
                .cmd()
                .iter()
                .map(|part| part.to_string_lossy())
                .collect::<Vec<_>>()
                .join(" "),
            status: format!("{:?}", process.status()),
            cpu: process.cpu_usage(),
            memory: process.memory(),
        })
        .collect();

    processes.sort_by(|a, b| {
        b.cpu
            .partial_cmp(&a.cpu)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.memory.cmp(&a.memory))
    });

    let cpu_model = system
        .cpus()
        .first()
        .map(|cpu| cpu.brand().trim().to_string())
        .unwrap_or_else(|| "Unknown CPU".into());

    Snapshot {
        timestamp_ms: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis(),
        accent: ryoku_accent(),
        cpu: system.global_cpu_usage(),
        total_memory: system.total_memory(),
        used_memory: system.used_memory(),
        total_swap: system.total_swap(),
        used_swap: system.used_swap(),
        process_count: processes.len(),
        processes,
        system: SystemInfo {
            hostname: System::host_name().unwrap_or_else(|| "Unknown".into()),
            os: System::long_os_version().unwrap_or_else(|| "Linux".into()),
            kernel: System::kernel_version().unwrap_or_else(|| "Unknown".into()),
            cpu_model,
            logical_cpus: system.cpus().len(),
            physical_cores: System::physical_core_count().unwrap_or(0),
        },
    }
}

#[tauri::command]
fn end_process(pid: u32, force: bool) -> Result<(), String> {
    if pid <= 1 || pid == std::process::id() {
        return Err("RyoManager will not terminate this protected process".into());
    }

    let signal = if force { libc::SIGKILL } else { libc::SIGTERM };
    let result = unsafe { libc::kill(pid as libc::pid_t, signal) };
    if result == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error().to_string())
    }
}

#[cfg(target_os = "linux")]
fn normalize_gtk_dpi() {
    use gtk::prelude::*;

    if gtk::init().is_ok() {
        if let Some(settings) = gtk::Settings::default() {
            settings.set_property("gtk-xft-dpi", 96 * 1024i32);
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn normalize_gtk_dpi() {}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // WebKitGTK can inherit a wildly inflated Xft DPI on some Wayland sessions,
    // shrinking CSS layout to a fraction of the intended size. Normalise only
    // this process before Tauri creates its WebView, matching Ryoku's other
    // WebKit surfaces without changing the user's desktop DPI.
    normalize_gtk_dpi();

    tauri::Builder::default()
        .manage(AppState {
            system: Mutex::new(System::new_with_specifics(live_refresh_kind())),
        })
        .invoke_handler(tauri::generate_handler![snapshot, end_process])
        .run(tauri::generate_context!())
        .expect("error while running RyoManager");
}
