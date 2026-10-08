// DRM metrics come from sysfs every sample; NVIDIA tools poll every two seconds, process file
// descriptors rescan every ten seconds, and PCI product names are resolved only once per card.
use super::{read_f64, read_trimmed, read_u64, Availability, GpuInfo, ProcessInfo};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

#[derive(Clone)]
struct DrmCard {
    index: u32,
    device: PathBuf,
    vendor: String,
    vendor_id: String,
    device_id: String,
    slot: Option<String>,
}

type NvidiaPoll = (Vec<GpuInfo>, HashMap<u32, (f64, Option<u64>, u32)>);

pub struct GpuCollector {
    last_poll: Option<Instant>,
    last_engine: Option<Instant>,
    last_fd_scan: Option<Instant>,
    fd_paths: HashMap<u32, Vec<PathBuf>>,
    engine_prev: HashMap<(u32, u32), u64>,
    product_names: HashMap<PathBuf, String>,
    cached: Vec<GpuInfo>,
    proc_gpu: HashMap<u32, (f64, Option<u64>, u32)>,
    nvidia: Vec<GpuInfo>,
    poll_inflight: bool,
    poll_tx: Sender<NvidiaPoll>,
    poll_rx: Receiver<NvidiaPoll>,
}

impl Default for GpuCollector {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            last_poll: None,
            last_engine: None,
            last_fd_scan: None,
            fd_paths: HashMap::new(),
            engine_prev: HashMap::new(),
            product_names: HashMap::new(),
            cached: Vec::new(),
            proc_gpu: HashMap::new(),
            nvidia: Vec::new(),
            poll_inflight: false,
            poll_tx: tx,
            poll_rx: rx,
        }
    }
}

fn drm_cards() -> Vec<DrmCard> {
    let mut cards = Vec::new();
    if let Ok(entries) = fs::read_dir("/sys/class/drm") {
        for entry in entries.flatten() {
            let file_name = entry.file_name().to_string_lossy().into_owned();
            let Some(index) = file_name
                .strip_prefix("card")
                .and_then(|value| value.parse().ok())
            else {
                continue;
            };
            if file_name.contains('-') {
                continue;
            }

            let device = entry.path().join("device");
            let vendor_id = read_trimmed(device.join("vendor"))
                .unwrap_or_default()
                .trim_start_matches("0x")
                .to_ascii_lowercase();
            let device_id = read_trimmed(device.join("device"))
                .unwrap_or_default()
                .trim_start_matches("0x")
                .to_ascii_lowercase();
            let vendor = match vendor_id.as_str() {
                "10de" => "nvidia",
                "1002" => "amd",
                "8086" => "intel",
                _ => "unknown",
            };
            let slot = fs::canonicalize(&device).ok().and_then(|path| {
                path.file_name()
                    .map(|name| name.to_string_lossy().into_owned())
            });

            cards.push(DrmCard {
                index,
                device,
                vendor: vendor.into(),
                vendor_id,
                device_id,
                slot,
            });
        }
    }
    cards.sort_by_key(|card| card.index);

    cards
}

fn nvidia_present() -> bool {
    drm_cards().iter().any(|card| card.vendor == "nvidia")
}

fn pci_product_name(database: &str, vendor_id: &str, device_id: &str) -> Option<String> {
    let mut matching_vendor = false;

    for line in database.lines() {
        if line
            .as_bytes()
            .first()
            .map(|byte| !byte.is_ascii_whitespace())
            .unwrap_or(false)
        {
            matching_vendor = line
                .split_whitespace()
                .next()
                .map(|id| id.eq_ignore_ascii_case(vendor_id))
                .unwrap_or(false);
            continue;
        }
        if !matching_vendor || !line.starts_with('\t') || line.starts_with("\t\t") {
            continue;
        }

        let mut fields = line.split_whitespace();
        if fields
            .next()
            .map(|id| id.eq_ignore_ascii_case(device_id))
            .unwrap_or(false)
        {
            let product = fields.collect::<Vec<_>>().join(" ");
            return (!product.is_empty()).then_some(product);
        }
    }

    None
}

fn lspci_product(slot: &str) -> Option<String> {
    let output = Command::new("lspci")
        .args(["-mm", "-s", slot])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let quoted_fields: Vec<&str> = text.split('"').skip(1).step_by(2).collect();
    quoted_fields
        .get(2)
        .map(|product| product.trim().to_string())
        .filter(|product| !product.is_empty())
}

fn friendly_gpu_name(vendor: &str, product: &str) -> String {
    let preferred_product = product
        .split(['[', ']'])
        .find(|part| {
            let lower = part.to_ascii_lowercase();
            lower.contains("radeon") || lower.contains("intel arc")
        })
        .map(str::trim)
        .unwrap_or(product);
    let prefix = match vendor {
        "amd" => "AMD",
        "intel" => "Intel",
        _ => "",
    };

    if prefix.is_empty()
        || preferred_product
            .to_ascii_lowercase()
            .starts_with(&prefix.to_ascii_lowercase())
    {
        preferred_product.to_string()
    } else {
        format!("{prefix} {preferred_product}")
    }
}

fn card_product_name(card: &DrmCard) -> String {
    let pci_name = fs::read_to_string("/usr/share/hwdata/pci.ids")
        .ok()
        .and_then(|database| pci_product_name(&database, &card.vendor_id, &card.device_id))
        .or_else(|| card.slot.as_deref().and_then(lspci_product));
    let product = pci_name.unwrap_or_else(|| format!("{} GPU", card.vendor.to_uppercase()));

    friendly_gpu_name(&card.vendor, &product)
}

// "00000000:01:00.0" from nvidia-smi against the "0000:01:00.0" slot sysfs names.
fn drm_index_for_bus(cards: &[DrmCard], bus_id: &str) -> Option<u32> {
    let wanted = bus_id.to_ascii_lowercase();
    cards
        .iter()
        .find(|card| {
            card.slot
                .as_deref()
                .map(|slot| wanted.ends_with(&slot.to_ascii_lowercase()))
                .unwrap_or(false)
        })
        .map(|card| card.index)
}

// nvidia-smi numbers GPUs on its own, which collides with the DRM card numbers
// used for every other vendor (an NVIDIA "0" beside AMD's card0). The PCI bus id
// in each row maps it back to its DRM card so one machine-wide index exists.
fn nvidia_gpus() -> Vec<GpuInfo> {
    let query = "name,utilization.gpu,utilization.memory,utilization.decoder,utilization.encoder,memory.used,memory.total,temperature.gpu,power.draw,power.limit,clocks.sm,clocks.mem,driver_version,pstate,pci.bus_id";
    let Ok(output) = Command::new("nvidia-smi")
        .args([
            format!("--query-gpu={query}"),
            "--format=csv,noheader,nounits".into(),
        ])
        .output()
    else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }

    let cards = drm_cards();
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .enumerate()
        .map(|(row, line)| {
            let fields: Vec<_> = line.split(',').map(str::trim).collect();
            let index = fields
                .get(14)
                .and_then(|bus_id| drm_index_for_bus(&cards, bus_id))
                .unwrap_or(1000 + row as u32);
            let numeric = |field_index: usize| {
                fields
                    .get(field_index)
                    .and_then(|value| value.parse::<f64>().ok())
            };
            let mut limits = Availability::new();
            limits.insert(
                "process".into(),
                "GPU process usage depends on the NVIDIA accounting engine".into(),
            );
            GpuInfo {
                index,
                name: fields.first().unwrap_or(&"NVIDIA GPU").to_string(),
                vendor: "nvidia".into(),
                usage: numeric(1),
                memory_usage: numeric(2),
                decoder: numeric(3),
                encoder: numeric(4),
                memory_used: numeric(5).map(|value| (value * 1024.0 * 1024.0) as u64),
                memory_total: numeric(6).map(|value| (value * 1024.0 * 1024.0) as u64),
                temperature: numeric(7),
                power: numeric(8),
                power_limit: numeric(9),
                clock_mhz: numeric(10),
                memory_clock_mhz: numeric(11),
                driver: fields.get(12).map(|value| value.to_string()),
                pstate: fields.get(13).map(|value| value.to_string()),
                limits,
            }
        })
        .collect()
}

fn hwmon_values(device: &Path) -> (Option<f64>, Option<f64>, Option<f64>) {
    let Some(hwmon) = fs::read_dir(device.join("hwmon"))
        .ok()
        .and_then(|entries| entries.flatten().map(|entry| entry.path()).next())
    else {
        return (None, None, None);
    };

    (
        read_f64(hwmon.join("temp1_input")).map(|value| value / 1000.0),
        read_f64(hwmon.join("power1_average")).map(|value| value / 1_000_000.0),
        read_f64(hwmon.join("power1_cap")).map(|value| value / 1_000_000.0),
    )
}

fn current_clock(device: &std::path::Path) -> Option<f64> {
    let text = fs::read_to_string(device.join("pp_dpm_sclk")).ok()?;
    text.lines()
        .find(|line| line.contains('*'))
        .and_then(|line| {
            line.split_whitespace().find_map(|word| {
                word.trim_end_matches("Mhz")
                    .trim_end_matches("MHz")
                    .parse()
                    .ok()
            })
        })
}

fn other_gpus(product_names: &mut HashMap<PathBuf, String>) -> Vec<GpuInfo> {
    drm_cards()
        .into_iter()
        .filter(|card| card.vendor != "nvidia")
        .map(|card| {
            let name = product_names
                .entry(card.device.clone())
                .or_insert_with(|| card_product_name(&card))
                .clone();
            let usage = read_f64(card.device.join("gpu_busy_percent"));
            let memory_used = read_u64(card.device.join("mem_info_vram_used"));
            let memory_total = read_u64(card.device.join("mem_info_vram_total"));
            let (temperature, power, power_limit) = hwmon_values(&card.device);
            let driver = fs::read_link(card.device.join("driver"))
                .ok()
                .and_then(|path| {
                    path.file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                });

            GpuInfo {
                index: card.index,
                name,
                vendor: card.vendor,
                usage,
                memory_usage: match (memory_used, memory_total) {
                    (Some(used), Some(total)) if total > 0 => {
                        Some(used as f64 * 100.0 / total as f64)
                    }
                    _ => None,
                },
                encoder: None,
                decoder: None,
                memory_used,
                memory_total,
                temperature,
                power,
                power_limit,
                clock_mhz: current_clock(&card.device),
                memory_clock_mhz: None,
                driver,
                pstate: None,
                limits: Availability::new(),
            }
        })
        .collect()
}

// drm_indexes[n] is the machine-wide index of nvidia-smi's GPU n.
fn pmon(drm_indexes: &[u32]) -> HashMap<u32, (f64, Option<u64>, u32)> {
    let mut per_process = HashMap::new();
    let Ok(output) = Command::new("nvidia-smi")
        .args(["pmon", "-c", "1"])
        .output()
    else {
        return per_process;
    };

    for line in String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| !line.starts_with('#'))
    {
        let fields: Vec<_> = line.split_whitespace().collect();
        if fields.len() < 8 {
            continue;
        }
        let Some(nvidia_index) = fields[0].parse::<usize>().ok() else {
            continue;
        };
        let Some(pid) = fields[1].parse().ok() else {
            continue;
        };
        let usage = [fields[3], fields[5], fields[6]]
            .into_iter()
            .filter_map(|value| value.parse::<f64>().ok())
            .reduce(f64::max);
        if let Some(usage) = usage {
            let gpu_index = drm_indexes
                .get(nvidia_index)
                .copied()
                .unwrap_or(1000 + nvidia_index as u32);
            per_process.insert(pid, (usage.min(100.0), None, gpu_index));
        }
    }

    per_process
}

fn scan_fds(processes: &[ProcessInfo]) -> HashMap<u32, Vec<PathBuf>> {
    let mut paths_by_pid = HashMap::new();
    for process in processes {
        if process.uid != unsafe { libc::geteuid() } {
            continue;
        }
        let Ok(entries) = fs::read_dir(format!("/proc/{}/fd", process.pid)) else {
            continue;
        };
        for entry in entries.flatten() {
            if let Ok(target) = fs::read_link(entry.path()) {
                if target.to_string_lossy().contains("/dev/dri/") {
                    paths_by_pid
                        .entry(process.pid)
                        .or_insert_with(Vec::new)
                        .push(PathBuf::from(format!(
                            "/proc/{}/fdinfo/{}",
                            process.pid,
                            entry.file_name().to_string_lossy()
                        )));
                }
            }
        }
    }

    paths_by_pid
}

fn engine_total(paths: &[PathBuf]) -> Option<u64> {
    let mut total = 0;
    let mut found = false;
    for path in paths {
        let Ok(text) = fs::read_to_string(path) else {
            continue;
        };
        for line in text.lines() {
            let Some((key, value)) = line.split_once(':') else {
                continue;
            };
            if !key.starts_with("drm-engine-") {
                continue;
            }
            if let Some(value) = value
                .split_whitespace()
                .next()
                .and_then(|value| value.parse::<u64>().ok())
            {
                total += value;
                found = true;
            }
        }
    }
    found.then_some(total)
}

fn explain_missing(gpu: &mut GpuInfo) {
    let reason = "this driver does not expose this reading";
    for (missing, key) in [
        (gpu.usage.is_none(), "usage"),
        (gpu.memory_usage.is_none(), "memory"),
        (gpu.encoder.is_none(), "encoder"),
        (gpu.decoder.is_none(), "decoder"),
        (gpu.temperature.is_none(), "temperature"),
        (gpu.power.is_none(), "power"),
        (gpu.power_limit.is_none(), "powerLimit"),
        (gpu.clock_mhz.is_none(), "clock"),
        (gpu.memory_clock_mhz.is_none(), "memoryClock"),
        (gpu.driver.is_none(), "driver"),
        (gpu.pstate.is_none(), "pstate"),
    ] {
        if missing {
            gpu.limits
                .entry(key.into())
                .or_insert_with(|| reason.into());
        }
    }
    if gpu.memory_used.is_none() || gpu.memory_total.is_none() {
        gpu.limits
            .entry("vram".into())
            .or_insert_with(|| reason.into());
    }
}

impl GpuCollector {
    pub fn collect(&mut self, processes: &mut [ProcessInfo], sample_ms: f64) -> Vec<GpuInfo> {
        while let Ok((gpus, per_process)) = self.poll_rx.try_recv() {
            self.nvidia = gpus;
            self.proc_gpu = per_process;
            self.poll_inflight = false;
        }
        let has_nvidia = nvidia_present();
        let poll_due = self
            .last_poll
            .map(|last_poll| last_poll.elapsed() >= Duration::from_secs(2))
            .unwrap_or(true);
        if has_nvidia && poll_due && !self.poll_inflight {
            let sender = self.poll_tx.clone();
            std::thread::spawn(move || {
                let gpus = nvidia_gpus();
                let drm_indexes: Vec<u32> = gpus.iter().map(|gpu| gpu.index).collect();
                let per_process = pmon(&drm_indexes);
                let _ = sender.send((gpus, per_process));
            });
            self.poll_inflight = true;
            self.last_poll = Some(Instant::now());
        }
        let mut gpus = self.nvidia.clone();
        if has_nvidia && gpus.is_empty() {
            for card in drm_cards()
                .into_iter()
                .filter(|card| card.vendor == "nvidia")
            {
                let mut limits = Availability::new();
                limits.insert("metrics".into(), "NVIDIA metrics are being measured".into());
                gpus.push(GpuInfo {
                    index: card.index,
                    name: "NVIDIA GPU".into(),
                    vendor: card.vendor,
                    usage: None,
                    memory_usage: None,
                    encoder: None,
                    decoder: None,
                    memory_used: None,
                    memory_total: None,
                    temperature: None,
                    power: None,
                    power_limit: None,
                    clock_mhz: None,
                    memory_clock_mhz: None,
                    driver: None,
                    pstate: None,
                    limits,
                });
            }
        }
        gpus.extend(other_gpus(&mut self.product_names));
        gpus.sort_by_key(|gpu| gpu.index);
        for gpu in &mut gpus {
            explain_missing(gpu);
        }
        self.cached = gpus;
        let engine_due = self
            .last_engine
            .map(|last_engine| last_engine.elapsed() >= Duration::from_secs(2))
            .unwrap_or(true);
        if engine_due {
            if self
                .last_fd_scan
                .map(|last_scan| last_scan.elapsed() >= Duration::from_secs(10))
                .unwrap_or(true)
            {
                self.fd_paths = scan_fds(processes);
                self.last_fd_scan = Some(Instant::now())
            }
            if let Some(index) = self
                .cached
                .iter()
                .find(|gpu| gpu.vendor == "amd" || gpu.vendor == "intel")
                .map(|gpu| gpu.index)
            {
                for (pid, paths) in &self.fd_paths {
                    let Some(total) = engine_total(paths) else {
                        continue;
                    };
                    let key = (*pid, index);
                    if let Some(previous_total) = self.engine_prev.insert(key, total) {
                        let usage = (total.saturating_sub(previous_total) as f64
                            / (sample_ms.max(1.0) * 1_000_000.0)
                            * 100.0)
                            .min(100.0);
                        self.proc_gpu.insert(*pid, (usage, None, index));
                    }
                }
            }
            self.last_engine = Some(Instant::now());
        }
        let live_processes: std::collections::HashSet<u32> =
            processes.iter().map(|process| process.pid).collect();
        self.proc_gpu.retain(|pid, _| live_processes.contains(pid));
        for process in processes {
            if let Some((usage, memory, index)) = self.proc_gpu.get(&process.pid) {
                process.gpu = Some(*usage);
                process.gpu_memory = *memory;
                process.gpu_index = Some(*index);
            }
        }
        self.cached.clone()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn maps_nvidia_bus_ids_onto_drm_cards() {
        let card = |index: u32, slot: &str| DrmCard {
            index,
            device: PathBuf::new(),
            vendor: "nvidia".into(),
            vendor_id: "10de".into(),
            device_id: "28e0".into(),
            slot: Some(slot.into()),
        };
        let cards = vec![card(0, "0000:65:00.0"), card(1, "0000:01:00.0")];
        assert_eq!(drm_index_for_bus(&cards, "00000000:01:00.0"), Some(1));
        assert_eq!(drm_index_for_bus(&cards, "00000000:65:00.0"), Some(0));
        assert_eq!(drm_index_for_bus(&cards, "00000000:02:00.0"), None);
    }

    use super::*;

    #[test]
    fn resolves_and_formats_pci_product_names() {
        let database = "1002  Advanced Micro Devices, Inc. [AMD/ATI]\n\
                        \t164e  Phoenix3 [Radeon 780M Graphics]\n\
                        8086  Intel Corporation\n\
                        \t7d55  Meteor Lake-P [Intel Arc Graphics]\n";

        let amd_product = pci_product_name(database, "1002", "164e").unwrap();
        let intel_product = pci_product_name(database, "8086", "7d55").unwrap();

        assert_eq!(
            friendly_gpu_name("amd", &amd_product),
            "AMD Radeon 780M Graphics"
        );
        assert_eq!(
            friendly_gpu_name("intel", &intel_product),
            "Intel Arc Graphics"
        );
    }
}
