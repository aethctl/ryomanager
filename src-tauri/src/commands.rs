use crate::collect::procs::{process_detail as read_process_detail, process_start_time};
use crate::collect::ProcessInfo;
use crate::AppState;
use std::path::PathBuf;
use std::process::{Command, Stdio};
fn protected(pid: u32) -> Result<(), String> {
    if pid <= 1 || pid == std::process::id() {
        Err("RyoManager will not terminate this protected process".into())
    } else {
        Ok(())
    }
}

fn identity(pid: u32, start_time: u64) -> Result<(), String> {
    match process_start_time(pid) {
        Some(actual) if actual == start_time => Ok(()),
        Some(_) => Err("that process has exited; the PID now belongs to another".into()),
        None => Err("that process has exited".into()),
    }
}

#[tauri::command(rename_all = "camelCase")]
pub fn process_detail(
    state: tauri::State<'_, AppState>,
    pid: u32,
    start_time: u64,
) -> Result<ProcessInfo, String> {
    identity(pid, start_time)?;
    let process = state
        .collector
        .lock()
        .map_err(|_| "collector lock poisoned".to_string())?
        .process(pid, start_time)
        .ok_or_else(|| "that process is not in the latest snapshot".to_string())?;
    Ok(read_process_detail(process))
}

fn send(pid: u32, start_time: u64, signal: i32) -> Result<(), String> {
    protected(pid)?;
    identity(pid, start_time)?;
    if unsafe { libc::kill(pid as libc::pid_t, signal) } == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error().to_string())
    }
}

#[tauri::command(rename_all = "camelCase")]
pub fn end_process(pid: u32, start_time: u64, force: bool) -> Result<(), String> {
    send(
        pid,
        start_time,
        if force { libc::SIGKILL } else { libc::SIGTERM },
    )
}

#[tauri::command(rename_all = "camelCase")]
pub fn end_group(
    state: tauri::State<'_, AppState>,
    key: String,
    force: bool,
) -> Result<(), String> {
    let members = {
        let collector = state
            .collector
            .lock()
            .map_err(|_| "collector lock poisoned".to_string())?;
        collector
            .group_members(&key)
            .ok_or_else(|| "that process group has exited".to_string())?
    };
    for (pid, start) in &members {
        protected(*pid)?;
        identity(*pid, *start)?;
    }
    for (pid, start) in members {
        send(
            pid,
            start,
            if force { libc::SIGKILL } else { libc::SIGTERM },
        )?;
    }
    Ok(())
}

#[tauri::command(rename_all = "camelCase")]
pub fn signal_process(pid: u32, start_time: u64, action: String) -> Result<(), String> {
    let signal = match action.as_str() {
        "suspend" => libc::SIGSTOP,
        "resume" => libc::SIGCONT,
        _ => return Err("unknown process action".into()),
    };
    send(pid, start_time, signal)
}

#[tauri::command(rename_all = "camelCase")]
pub fn set_priority(pid: u32, start_time: u64, nice: i32) -> Result<(), String> {
    protected(pid)?;
    identity(pid, start_time)?;
    let result = unsafe { libc::setpriority(libc::PRIO_PROCESS, pid, nice) };
    if result == 0 {
        return Ok(());
    }
    let error = std::io::Error::last_os_error();
    if matches!(error.raw_os_error(), Some(libc::EACCES) | Some(libc::EPERM)) {
        Err("raising priority needs root".into())
    } else {
        Err(error.to_string())
    }
}

#[tauri::command(rename_all = "camelCase")]
pub fn open_location(pid: u32, start_time: u64) -> Result<(), String> {
    identity(pid, start_time)?;
    let executable_directory = std::fs::read_link(format!("/proc/{pid}/exe"))
        .ok()
        .and_then(|path| path.parent().map(PathBuf::from));
    let working_directory = std::fs::read_link(format!("/proc/{pid}/cwd")).ok();
    let path = executable_directory
        .or(working_directory)
        .ok_or_else(|| "this process location is not readable".to_string())?;

    Command::new("xdg-open")
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn app_icon(state: tauri::State<'_, AppState>, key: String) -> Option<String> {
    let mut collector = state.collector.lock().ok()?;
    collector.app_icon(&key)
}
