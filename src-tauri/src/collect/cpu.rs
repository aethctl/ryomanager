// CPU load comes from `/proc/stat` every sample; static topology, cache, and capability data
// comes from `/proc/cpuinfo` and sysfs and is loaded once per collector.
use super::{pressure, read_f64, read_trimmed, read_u64, CpuCache, CpuCore, CpuInfo, ProcessInfo};
use std::collections::{HashMap, HashSet};
use std::fs;

#[derive(Clone, Default)]
struct Times {
    user: u64,
    nice: u64,
    system: u64,
    idle: u64,
    iowait: u64,
    irq: u64,
    softirq: u64,
    steal: u64,
}

impl Times {
    fn total(&self) -> u64 {
        self.user
            + self.nice
            + self.system
            + self.idle
            + self.iowait
            + self.irq
            + self.softirq
            + self.steal
    }
    fn busy(&self) -> u64 {
        self.total() - self.idle - self.iowait
    }
}

#[derive(Default)]
pub struct CpuCollector {
    previous: HashMap<String, Times>,
    model: Option<String>,
    caches: Vec<CpuCache>,
    sockets: u64,
    physical: u64,
    virtualization: Option<String>,
}

fn times() -> HashMap<String, Times> {
    let mut times_by_cpu = HashMap::new();
    if let Ok(text) = fs::read_to_string("/proc/stat") {
        for line in text.lines().filter(|line| line.starts_with("cpu")) {
            let mut fields = line.split_whitespace();
            let Some(name) = fields.next() else {
                continue;
            };
            let values: Vec<u64> = fields
                .take(8)
                .map(|value| value.parse().unwrap_or(0))
                .collect();
            if values.len() >= 8 {
                times_by_cpu.insert(
                    name.into(),
                    Times {
                        user: values[0],
                        nice: values[1],
                        system: values[2],
                        idle: values[3],
                        iowait: values[4],
                        irq: values[5],
                        softirq: values[6],
                        steal: values[7],
                    },
                );
            }
        }
    }

    times_by_cpu
}

fn delta_pct(current: &Times, previous: Option<&Times>) -> (f64, f64, f64, f64, f64) {
    let baseline = previous.cloned().unwrap_or_default();
    let total_delta = current.total().saturating_sub(baseline.total()).max(1) as f64;
    let percentage = |current_value: u64, previous_value: u64| {
        current_value.saturating_sub(previous_value) as f64 * 100.0 / total_delta
    };

    (
        percentage(current.busy(), baseline.busy()),
        percentage(current.user + current.nice, baseline.user + baseline.nice),
        percentage(current.system, baseline.system),
        percentage(current.iowait, baseline.iowait),
        percentage(
            current.irq + current.softirq,
            baseline.irq + baseline.softirq,
        ),
    )
}

fn cpu_info_static() -> (String, u64, u64, Option<String>) {
    let text = fs::read_to_string("/proc/cpuinfo").unwrap_or_default();
    let mut model = "Unknown CPU".to_string();
    let mut sockets = HashSet::new();
    let mut cores = HashSet::new();
    let mut socket = "0".to_string();
    let mut flags = "";
    for line in text.lines() {
        if let Some((key, value)) = line.split_once(':') {
            let key = key.trim();
            let value = value.trim();
            match key {
                "model name" => {
                    if model == "Unknown CPU" {
                        model = value.into()
                    }
                }
                "physical id" => {
                    socket = value.into();
                    sockets.insert(value.to_string());
                }
                "core id" => {
                    cores.insert(format!("{socket}:{value}"));
                }
                "flags" => flags = value,
                _ => {}
            }
        }
    }
    let virtualization = if flags.split_whitespace().any(|flag| flag == "svm") {
        Some("AMD-V".into())
    } else if flags.split_whitespace().any(|flag| flag == "vmx") {
        Some("VT-x".into())
    } else {
        None
    };
    (
        model,
        sockets.len().max(1) as u64,
        cores.len().max(1) as u64,
        virtualization,
    )
}

fn format_cache_size(raw_size: &str) -> Option<String> {
    let normalized = raw_size.trim().to_ascii_uppercase();
    let (number, multiplier) = if let Some(number) = normalized.strip_suffix("KIB") {
        (number, 1024)
    } else if let Some(number) = normalized.strip_suffix('K') {
        (number, 1024)
    } else if let Some(number) = normalized.strip_suffix("MIB") {
        (number, 1024 * 1024)
    } else if let Some(number) = normalized.strip_suffix('M') {
        (number, 1024 * 1024)
    } else {
        (normalized.as_str(), 1)
    };
    let bytes = number.trim().parse::<u64>().ok()?.checked_mul(multiplier)?;

    if bytes >= 1024 * 1024 && bytes.is_multiple_of(1024 * 1024) {
        Some(format!("{} MB", bytes / (1024 * 1024)))
    } else if bytes >= 1024 && bytes.is_multiple_of(1024) {
        Some(format!("{} KB", bytes / 1024))
    } else {
        Some(format!("{bytes} B"))
    }
}

fn cache_sort_key(level: &str) -> (u32, u32) {
    let level_number = level
        .trim_start_matches('L')
        .trim_end_matches(['d', 'i'])
        .parse()
        .unwrap_or(u32::MAX);
    let kind_order = match level.chars().last() {
        Some('d') => 0,
        Some('i') => 1,
        _ => 2,
    };

    (level_number, kind_order)
}

fn caches() -> Vec<CpuCache> {
    let mut by_level = HashMap::new();
    if let Ok(entries) = fs::read_dir("/sys/devices/system/cpu/cpu0/cache") {
        for entry in entries.flatten() {
            let path = entry.path();
            let level = read_trimmed(path.join("level"));
            let cache_type = read_trimmed(path.join("type"));
            let size = read_trimmed(path.join("size")).and_then(|value| format_cache_size(&value));

            if let (Some(level), Some(cache_type), Some(size)) = (level, cache_type, size) {
                let label = format!(
                    "L{level}{}",
                    if cache_type == "Data" {
                        "d"
                    } else if cache_type == "Instruction" {
                        "i"
                    } else {
                        ""
                    }
                );
                by_level.entry(label).or_insert(size);
            }
        }
    }

    let mut caches: Vec<CpuCache> = by_level
        .into_iter()
        .map(|(level, size)| CpuCache { level, size })
        .collect();
    caches.sort_by_key(|cache| cache_sort_key(&cache.level));
    caches
}

fn core_kind(index: u32) -> Option<String> {
    for (kind, path) in [
        ("performance", "/sys/devices/cpu_core/cpus"),
        ("efficiency", "/sys/devices/cpu_atom/cpus"),
    ] {
        if let Some(cpu_ranges) = read_trimmed(path) {
            for range in cpu_ranges.split(',') {
                let (start, end) = range
                    .split_once('-')
                    .map(|(start, end)| {
                        (start.parse().unwrap_or(0), end.parse().unwrap_or(u32::MAX))
                    })
                    .unwrap_or_else(|| {
                        let value = range.parse().unwrap_or(u32::MAX);
                        (value, value)
                    });
                if index >= start && index <= end {
                    return Some(kind.into());
                }
            }
        }
    }

    None
}

impl CpuCollector {
    pub fn collect(&mut self, processes: &[ProcessInfo]) -> CpuInfo {
        if self.model.is_none() {
            let (model, sockets, physical_cores, virtualization) = cpu_info_static();
            self.model = Some(model);
            self.sockets = sockets;
            self.physical = physical_cores;
            self.virtualization = virtualization;
            self.caches = caches();
        }
        let current_times = times();
        let overall = current_times.get("cpu").cloned().unwrap_or_default();
        let (usage, user, system, iowait, irq) = delta_pct(&overall, self.previous.get("cpu"));
        let mut cores = Vec::new();
        for (name, core_times) in current_times
            .iter()
            .filter(|(name, _)| name.starts_with("cpu") && *name != "cpu")
        {
            let index = name[3..].parse::<u32>().unwrap_or(0);
            let (usage, _, _, _, _) = delta_pct(core_times, self.previous.get(name));
            let frequency = read_f64(format!(
                "/sys/devices/system/cpu/cpu{index}/cpufreq/scaling_cur_freq"
            ))
            .map(|value| value / 1000.0);
            cores.push(CpuCore {
                index,
                usage,
                freq_mhz: frequency,
                kind: core_kind(index),
            });
        }
        cores.sort_by_key(|core| core.index);
        self.previous = current_times;
        let frequencies: Vec<f64> = cores.iter().filter_map(|core| core.freq_mhz).collect();
        let freq_mhz = (!frequencies.is_empty())
            .then(|| frequencies.iter().sum::<f64>() / frequencies.len() as f64);
        let max_freq_mhz = read_f64("/sys/devices/system/cpu/cpu0/cpufreq/cpuinfo_max_freq")
            .map(|value| value / 1000.0);
        let governor = read_trimmed("/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor");
        let load: Vec<f64> = fs::read_to_string("/proc/loadavg")
            .unwrap_or_default()
            .split_whitespace()
            .take(3)
            .map(|value| value.parse().unwrap_or(0.0))
            .collect();
        let uptime = fs::read_to_string("/proc/uptime")
            .ok()
            .and_then(|text| text.split_whitespace().next()?.parse().ok())
            .unwrap_or(0.0);
        let file_nr = fs::read_to_string("/proc/sys/fs/file-nr")
            .unwrap_or_default()
            .split_whitespace()
            .filter_map(|value| value.parse::<u64>().ok())
            .collect::<Vec<_>>();
        let logical_cpus = cores.len() as u64;
        CpuInfo {
            usage,
            user,
            system,
            iowait,
            irq,
            cores,
            freq_mhz,
            max_freq_mhz,
            governor,
            load_avg: [
                *load.first().unwrap_or(&0.0),
                *load.get(1).unwrap_or(&0.0),
                *load.get(2).unwrap_or(&0.0),
            ],
            uptime_seconds: uptime,
            processes: processes.len() as u64,
            threads: processes.iter().map(|process| process.threads).sum(),
            open_files: file_nr.first().copied(),
            open_files_max: read_u64("/proc/sys/fs/file-max"),
            model: self.model.clone().unwrap(),
            sockets: self.sockets,
            physical_cores: self.physical,
            logical_cpus,
            virtualization: self.virtualization.clone(),
            caches: self.caches.clone(),
            pressure: pressure("/proc/pressure/cpu"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_cache_sizes_for_display() {
        assert_eq!(format_cache_size("32K").as_deref(), Some("32 KB"));
        assert_eq!(format_cache_size("1024K").as_deref(), Some("1 MB"));
        assert_eq!(format_cache_size("16384K").as_deref(), Some("16 MB"));
    }

    #[test]
    fn cache_levels_sort_data_before_instruction() {
        let mut levels = vec!["L3", "L1i", "L2", "L1d"];
        levels.sort_by_key(|level| cache_sort_key(level));

        assert_eq!(levels, vec!["L1d", "L1i", "L2", "L3"]);
    }
}
