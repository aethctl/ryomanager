// Collector modules combine kernel pseudo-files, sysfs, and slow helper commands into one
// serializable snapshot; each module owns its source-specific refresh cadence.
pub mod cpu;
pub mod desktop;
pub mod disks;
pub mod energy;
pub mod gpu;
pub mod groups;
pub mod memory;
pub mod network;
pub mod procs;
pub mod thermal;
pub mod units;
pub mod windows;

use serde::Serialize;
use std::collections::HashMap;

pub type Availability = HashMap<String, String>;

#[derive(Clone, Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WindowRef {
    pub id: String,
    pub title: String,
    pub app_id: String,
    pub workspace: String,
    pub output: String,
    pub focused: bool,
    #[serde(skip_serializing)]
    pub pid: Option<u32>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessInfo {
    pub pid: u32,
    pub parent_pid: Option<u32>,
    pub start_time: u64,
    pub name: String,
    pub exe: Option<String>,
    pub command: String,
    pub cwd: Option<String>,
    pub user: String,
    pub uid: u32,
    pub state: String,
    pub kernel_thread: bool,
    pub unit: Option<String>,
    pub cpu: f64,
    pub cpu_user: f64,
    pub cpu_system: f64,
    pub memory: u64,
    pub memory_kind: String,
    pub rss: u64,
    pub rss_anon: Option<u64>,
    pub rss_file: Option<u64>,
    pub rss_shmem: Option<u64>,
    pub swap: Option<u64>,
    #[serde(rename = "virtual")]
    pub virtual_: u64,
    pub disk_read: Option<f64>,
    pub disk_write: Option<f64>,
    pub disk_read_total: Option<u64>,
    pub disk_write_total: Option<u64>,
    pub gpu: Option<f64>,
    pub gpu_memory: Option<u64>,
    pub gpu_index: Option<u32>,
    pub connections: Option<u64>,
    pub energy: String,
    pub energy_score: f64,
    pub threads: u64,
    pub fds: Option<u64>,
    pub nice: i64,
    pub priority: i64,
    pub ctx_switches: Option<u64>,
    pub oom_score: Option<i64>,
    pub last_cpu: Option<u32>,
    pub windows: Vec<WindowRef>,
    #[serde(skip_serializing)]
    pub cgroup: String,
    #[serde(skip_serializing)]
    pub root_id: u32,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessGroup {
    pub key: String,
    pub category: String,
    pub name: String,
    pub subtitle: String,
    pub icon_key: Option<String>,
    pub unit: Option<String>,
    pub leader_pid: u32,
    pub instances: u64,
    pub windows: Vec<WindowRef>,
    pub state: String,
    pub cpu: f64,
    pub memory: u64,
    pub memory_kind: String,
    pub disk_read: Option<f64>,
    pub disk_write: Option<f64>,
    pub gpu: Option<f64>,
    pub gpu_memory: Option<u64>,
    pub connections: Option<u64>,
    pub energy: String,
    pub energy_score: f64,
    pub threads: u64,
    pub members: Vec<ProcessInfo>,
}

#[derive(Clone, Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Pressure {
    pub some10: f64,
    pub some60: f64,
    pub some300: f64,
    pub full10: Option<f64>,
    pub full60: Option<f64>,
    pub full300: Option<f64>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CpuCore {
    pub index: u32,
    pub usage: f64,
    pub freq_mhz: Option<f64>,
    pub kind: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct CpuCache {
    pub level: String,
    pub size: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CpuInfo {
    pub usage: f64,
    pub user: f64,
    pub system: f64,
    pub iowait: f64,
    pub irq: f64,
    pub cores: Vec<CpuCore>,
    pub freq_mhz: Option<f64>,
    pub max_freq_mhz: Option<f64>,
    pub governor: Option<String>,
    pub load_avg: [f64; 3],
    pub uptime_seconds: f64,
    pub processes: u64,
    pub threads: u64,
    pub open_files: Option<u64>,
    pub open_files_max: Option<u64>,
    pub model: String,
    pub sockets: u64,
    pub physical_cores: u64,
    pub logical_cpus: u64,
    pub virtualization: Option<String>,
    pub caches: Vec<CpuCache>,
    pub pressure: Option<Pressure>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryInfo {
    pub total: u64,
    pub used: u64,
    pub available: u64,
    pub free: u64,
    pub buffers: u64,
    pub cached: u64,
    pub shared: u64,
    pub dirty: u64,
    pub mapped: u64,
    pub committed: u64,
    pub commit_limit: u64,
    pub swap_total: u64,
    pub swap_used: u64,
    pub swap_cached: u64,
    pub zswap: Option<u64>,
    pub pressure: Option<Pressure>,
    pub limits: Availability,
}

#[derive(Clone, Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DiskMount {
    pub path: String,
    pub fs: String,
    pub used: u64,
    pub total: u64,
    pub system: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiskInfo {
    pub name: String,
    pub model: Option<String>,
    pub kind: String,
    pub capacity: u64,
    pub read_rate: f64,
    pub write_rate: f64,
    pub read_iops: f64,
    pub write_iops: f64,
    pub active_percent: f64,
    pub response_ms: Option<f64>,
    pub read_total: u64,
    pub write_total: u64,
    pub mounts: Vec<DiskMount>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkInfo {
    pub name: String,
    pub kind: String,
    pub state: String,
    pub rx_rate: f64,
    pub tx_rate: f64,
    pub rx_total: u64,
    pub tx_total: u64,
    pub ipv4: Vec<String>,
    pub ipv6: Vec<String>,
    pub mac: Option<String>,
    pub mtu: Option<u64>,
    pub speed_mbps: Option<f64>,
    pub ssid: Option<String>,
    pub signal: Option<f64>,
    pub frequency_mhz: Option<f64>,
    pub driver: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuInfo {
    pub index: u32,
    pub name: String,
    pub vendor: String,
    pub usage: Option<f64>,
    pub memory_usage: Option<f64>,
    pub encoder: Option<f64>,
    pub decoder: Option<f64>,
    pub memory_used: Option<u64>,
    pub memory_total: Option<u64>,
    pub temperature: Option<f64>,
    pub power: Option<f64>,
    pub power_limit: Option<f64>,
    pub clock_mhz: Option<f64>,
    pub memory_clock_mhz: Option<f64>,
    pub driver: Option<String>,
    pub pstate: Option<String>,
    pub limits: Availability,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnergyInfo {
    pub source: String,
    pub battery_percent: Option<f64>,
    pub battery_status: Option<String>,
    pub battery_power: Option<f64>,
    pub battery_energy_now: Option<f64>,
    pub battery_energy_full: Option<f64>,
    pub battery_energy_design: Option<f64>,
    pub time_to_empty_seconds: Option<f64>,
    pub cycle_count: Option<u64>,
    pub package_power: Option<f64>,
    pub limits: Availability,
}

#[derive(Clone, Debug, Serialize)]
pub struct ThermalSensor {
    pub id: String,
    pub chip: String,
    pub label: String,
    pub temperature: f64,
    pub max: Option<f64>,
    pub critical: Option<f64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ThermalInfo {
    pub hotspot: Option<ThermalSensor>,
    pub sensors: Vec<ThermalSensor>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfo {
    pub hostname: String,
    pub os: String,
    pub kernel: String,
    pub cpu_model: String,
    pub logical_cpus: u64,
    pub physical_cores: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub timestamp_ms: u128,
    pub sample_ms: f64,
    pub accent: String,
    pub windows_source: String,
    pub self_pid: u32,
    pub cpu: CpuInfo,
    pub memory: MemoryInfo,
    pub disks: Vec<DiskInfo>,
    pub networks: Vec<NetworkInfo>,
    pub gpus: Vec<GpuInfo>,
    pub energy: EnergyInfo,
    pub thermal: ThermalInfo,
    pub process_count: usize,
    pub groups: Vec<ProcessGroup>,
    pub system: SystemInfo,
    pub limits: Availability,
}

pub fn read_trimmed(path: impl AsRef<std::path::Path>) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|text| text.trim().to_string())
}

pub fn read_u64(path: impl AsRef<std::path::Path>) -> Option<u64> {
    read_trimmed(path)?.parse().ok()
}

pub fn read_f64(path: impl AsRef<std::path::Path>) -> Option<f64> {
    read_trimmed(path)?.parse().ok()
}

pub fn pressure(path: &str) -> Option<Pressure> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut pressure = Pressure::default();
    for line in text.lines() {
        let mut parts = line.split_whitespace();
        let class = parts.next()?;
        let mut values = HashMap::new();
        for part in parts {
            let (key, value) = part.split_once('=')?;
            if let Ok(number) = value.parse::<f64>() {
                values.insert(key, number);
            }
        }
        if class == "some" {
            pressure.some10 = *values.get("avg10").unwrap_or(&0.0);
            pressure.some60 = *values.get("avg60").unwrap_or(&0.0);
            pressure.some300 = *values.get("avg300").unwrap_or(&0.0);
        }
        if class == "full" {
            pressure.full10 = values.get("avg10").copied();
            pressure.full60 = values.get("avg60").copied();
            pressure.full300 = values.get("avg300").copied();
        }
    }

    Some(pressure)
}
