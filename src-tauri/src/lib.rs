mod collect;
mod commands;

use collect::cpu::CpuCollector;
use collect::desktop::DesktopIndex;
use collect::disks::DiskCollector;
use collect::energy::EnergyCollector;
use collect::gpu::GpuCollector;
use collect::groups::group_processes;
use collect::network::NetworkCollector;
use collect::procs::ProcessCollector;
use collect::units::UnitDescriptions;
use collect::windows::WindowFeed;
use collect::{Availability, ProcessInfo, Snapshot, SystemInfo};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

pub(crate) struct AppState {
    pub(crate) collector: Mutex<Collector>,
}

struct Collector {
    last_sample: Option<Instant>,
    processes: ProcessCollector,
    cpu: CpuCollector,
    disks: DiskCollector,
    network: NetworkCollector,
    gpu: GpuCollector,
    energy: EnergyCollector,
    thermal: collect::thermal::ThermalCollector,
    desktop: DesktopIndex,
    units: UnitDescriptions,
    windows: WindowFeed,
    latest_groups: HashMap<String, Vec<(u32, u64)>>,
    latest_processes: HashMap<(u32, u64), ProcessInfo>,
}

impl Collector {
    fn new() -> Self {
        Self {
            last_sample: None,
            processes: ProcessCollector::default(),
            cpu: CpuCollector::default(),
            disks: DiskCollector::default(),
            network: NetworkCollector::default(),
            gpu: GpuCollector::default(),
            energy: EnergyCollector::default(),
            thermal: collect::thermal::ThermalCollector::default(),
            desktop: DesktopIndex::default(),
            units: UnitDescriptions::default(),
            windows: WindowFeed::start(),
            latest_groups: HashMap::new(),
            latest_processes: HashMap::new(),
        }
    }
    fn collect(&mut self) -> Snapshot {
        let now = Instant::now();
        let sample_ms = self
            .last_sample
            .map(|previous| now.duration_since(previous).as_secs_f64() * 1000.0)
            .unwrap_or(1000.0);
        self.last_sample = Some(now);
        let (source, windows) = self.windows.snapshot();
        let logical = std::thread::available_parallelism()
            .map(|value| value.get() as u64)
            .unwrap_or(1);
        let mut processes = self.processes.collect(sample_ms, logical, &windows);
        let gpus = self.gpu.collect(&mut processes, sample_ms);
        let process_count = processes.len();
        let cpu = self.cpu.collect(&processes);
        let memory = collect::memory::collect();
        let disks = self.disks.collect(sample_ms);
        let networks = self.network.collect(sample_ms);
        let energy = self.energy.collect();
        let thermal = self.thermal.collect();
        let groups = group_processes(&mut processes, &mut self.desktop, &mut self.units, &source);
        self.latest_groups = groups
            .iter()
            .map(|group| {
                (
                    group.key.clone(),
                    group
                        .members
                        .iter()
                        .map(|process| (process.pid, process.start_time))
                        .collect(),
                )
            })
            .collect();
        self.latest_processes = groups
            .iter()
            .flat_map(|group| group.members.iter())
            .map(|process| ((process.pid, process.start_time), process.clone()))
            .collect();
        let system = SystemInfo {
            hostname: collect::read_trimmed("/proc/sys/kernel/hostname")
                .unwrap_or_else(|| "Unknown".into()),
            os: os_name(),
            kernel: collect::read_trimmed("/proc/sys/kernel/osrelease")
                .unwrap_or_else(|| "Unknown".into()),
            cpu_model: cpu.model.clone(),
            logical_cpus: cpu.logical_cpus,
            physical_cores: cpu.physical_cores,
        };
        let mut limits = Availability::new();
        let detail_reason = "exact memory and counters are read for the inspected process";
        for (key, reason) in [
            (
                "io",
                "I/O counters are readable only for your own processes",
            ),
            ("pss", detail_reason),
            (
                "gpu.process",
                "per-process GPU usage is unavailable when the driver does not expose engine time",
            ),
            (
                "network.processBytes",
                "Linux keeps no per-process network byte counters",
            ),
            ("connections", detail_reason),
            ("fds", detail_reason),
            ("ctxSwitches", detail_reason),
            ("oomScore", detail_reason),
            ("memory.hardware", "memory speed and slots need root"),
            ("swap", detail_reason),
            (
                "gpu.memory",
                "per-process GPU memory is not exposed by this driver",
            ),
            (
                "cpu.frequency",
                "CPU frequency is not exposed by this driver",
            ),
            (
                "cpu.coreKind",
                "the kernel does not identify performance and efficiency cores",
            ),
            (
                "memory.zswap",
                "zswap is not enabled or exposed by this kernel",
            ),
            (
                "disk.response",
                "response time needs an interval with completed I/O",
            ),
            (
                "network.speed",
                "the network driver does not expose link speed",
            ),
            (
                "network.wifi",
                "Wi-Fi details need an active NetworkManager connection",
            ),
            ("network.driver", "the network driver name is not readable"),
            ("energy.battery", "no battery reading is available"),
            (
                "thermal",
                "no readable hardware temperature sensors were found",
            ),
        ] {
            limits.insert(key.into(), reason.into());
        }
        if let Some(reason) = energy.limits.get("packagePower") {
            limits.insert("energy.packagePower".into(), reason.clone());
        }
        Snapshot {
            timestamp_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis(),
            sample_ms,
            accent: ryoku_accent(),
            windows_source: source,
            self_pid: std::process::id(),
            cpu,
            memory,
            disks,
            networks,
            gpus,
            energy,
            thermal,
            process_count,
            groups,
            system,
            limits,
        }
    }
    pub(crate) fn group_members(&self, key: &str) -> Option<Vec<(u32, u64)>> {
        self.latest_groups.get(key).cloned()
    }
    pub(crate) fn process(&self, pid: u32, start_time: u64) -> Option<ProcessInfo> {
        self.latest_processes.get(&(pid, start_time)).cloned()
    }
    pub(crate) fn app_icon(&mut self, key: &str) -> Option<String> {
        self.desktop.refresh();
        self.desktop.icon_for_key(key)
    }
}

fn os_name() -> String {
    let text = std::fs::read_to_string("/etc/os-release").unwrap_or_default();
    text.lines()
        .find_map(|line| {
            line.strip_prefix("PRETTY_NAME=")
                .map(|value| value.trim_matches('"').to_string())
        })
        .unwrap_or_else(|| "Linux".into())
}

fn ryoku_palette_path() -> Option<PathBuf> {
    if let Some(cache) = std::env::var_os("XDG_CACHE_HOME") {
        return Some(PathBuf::from(cache).join("ryoku/colors.json"));
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
        .filter(|color| {
            color.len() == 7
                && color.starts_with('#')
                && color[1..]
                    .chars()
                    .all(|character| character.is_ascii_hexdigit())
        })
        .unwrap_or(FALLBACK)
        .into()
}

#[tauri::command]
fn snapshot(state: tauri::State<'_, AppState>) -> Result<Snapshot, String> {
    state
        .collector
        .lock()
        .map_err(|_| "collector lock poisoned".to_string())
        .map(|mut collector| collector.collect())
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
    normalize_gtk_dpi();
    tauri::Builder::default()
        .manage(AppState {
            collector: Mutex::new(Collector::new()),
        })
        .invoke_handler(tauri::generate_handler![
            snapshot,
            commands::app_icon,
            commands::process_detail,
            commands::end_process,
            commands::end_group,
            commands::signal_process,
            commands::set_priority,
            commands::open_location
        ])
        .run(tauri::generate_context!())
        .expect("error while running RyoManager");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn process_cpu_time() -> std::time::Duration {
        let mut time = libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        let result = unsafe { libc::clock_gettime(libc::CLOCK_PROCESS_CPUTIME_ID, &mut time) };
        assert_eq!(result, 0);
        std::time::Duration::new(time.tv_sec as u64, time.tv_nsec as u32)
    }

    #[test]
    fn snapshot_wall_time() {
        let _guard = collect::procs::PERF_TEST_LOCK
            .lock()
            .expect("performance test lock poisoned");
        let mut collector = Collector::new();
        for _ in 0..3 {
            let _ = collector.collect();
        }
        let logical = std::thread::available_parallelism()
            .map(|parallelism| parallelism.get() as u64)
            .unwrap_or(1);
        let mut stage_totals = [std::time::Duration::ZERO; 4];
        let mut max_elapsed = std::time::Duration::ZERO;
        for sample in 1..=10 {
            std::thread::sleep(std::time::Duration::from_millis(20));
            let total = Instant::now();
            let (source, windows) = collector.windows.snapshot();
            let started_at = Instant::now();
            let mut processes = collector.processes.collect(1000.0, logical, &windows);
            let process_time = started_at.elapsed();
            let started_at = Instant::now();
            let _gpus = collector.gpu.collect(&mut processes, 1000.0);
            let gpu_time = started_at.elapsed();
            let started_at = Instant::now();
            let _cpu = collector.cpu.collect(&processes);
            let _memory = collect::memory::collect();
            let _disks = collector.disks.collect(1000.0);
            let _networks = collector.network.collect(1000.0);
            let _energy = collector.energy.collect();
            let _thermal = collector.thermal.collect();
            let metric_time = started_at.elapsed();
            let started_at = Instant::now();
            let groups = group_processes(
                &mut processes,
                &mut collector.desktop,
                &mut collector.units,
                &source,
            );
            let group_time = started_at.elapsed();
            let elapsed = total.elapsed();
            assert!(groups.iter().all(|group| {
                group.memory_kind == "private"
                    && group
                        .members
                        .iter()
                        .all(|process| process.memory_kind == "private")
            }));
            eprintln!(
                "warm snapshot {sample}: {:.2} ms (processes {:.2}, gpu {:.2}, metrics {:.2}, groups {:.2})",
                elapsed.as_secs_f64() * 1000.0,
                process_time.as_secs_f64() * 1000.0,
                gpu_time.as_secs_f64() * 1000.0,
                metric_time.as_secs_f64() * 1000.0,
                group_time.as_secs_f64() * 1000.0,
            );
            for (total, elapsed) in
                stage_totals
                    .iter_mut()
                    .zip([process_time, gpu_time, metric_time, group_time])
            {
                *total += elapsed;
            }
            max_elapsed = max_elapsed.max(elapsed);
        }
        eprintln!(
            "average stages: processes {:.2} ms, gpu {:.2} ms, metrics {:.2} ms, groups {:.2} ms",
            stage_totals[0].as_secs_f64() * 100.0,
            stage_totals[1].as_secs_f64() * 100.0,
            stage_totals[2].as_secs_f64() * 100.0,
            stage_totals[3].as_secs_f64() * 100.0,
        );
        if !cfg!(debug_assertions) {
            assert!(
                max_elapsed < std::time::Duration::from_millis(12),
                "slowest warm snapshot took {max_elapsed:?}"
            );
        }
        let cpu_started_at = process_cpu_time();
        for _ in 0..10 {
            let _ = collector.collect();
        }
        let cpu_time = process_cpu_time().saturating_sub(cpu_started_at);
        let self_cpu = cpu_time.as_secs_f64() * 10.0;
        eprintln!(
            "10 snapshots: {:.2} ms process CPU, {:.2}% of one core at 1 Hz",
            cpu_time.as_secs_f64() * 1000.0,
            self_cpu,
        );
        if !cfg!(debug_assertions) {
            assert!(
                self_cpu < 2.0,
                "10 snapshots used {self_cpu:.2}% of one core at 1 Hz"
            );
        }
    }
}
