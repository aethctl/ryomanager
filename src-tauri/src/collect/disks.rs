// Disk counters come from `/proc/diskstats` every sample; lsblk metadata refreshes once per
// minute and mount capacity is read from `/proc/mounts` with statvfs.
use super::{read_u64, DiskInfo, DiskMount};
use serde_json::Value;
use std::collections::HashMap;
use std::ffi::CString;
use std::fs;
use std::process::Command;
use std::time::{Duration, Instant};

#[derive(Clone, Debug, Default)]
pub struct DiskCounters {
    pub reads: u64,
    pub read_sectors: u64,
    pub writes: u64,
    pub write_sectors: u64,
    pub io_ms: u64,
    pub weighted_ms: u64,
}

#[derive(Clone, Default)]
struct Meta {
    model: Option<String>,
    kind: String,
    capacity: u64,
}

#[derive(Default)]
pub struct DiskCollector {
    previous: HashMap<String, DiskCounters>,
    meta: HashMap<String, Meta>,
    meta_at: Option<Instant>,
}

pub fn parse_diskstats(text: &str) -> HashMap<String, DiskCounters> {
    let mut counters_by_device = HashMap::new();
    for line in text.lines() {
        let fields: Vec<_> = line.split_whitespace().collect();
        if fields.len() < 14 {
            continue;
        }
        let name = fields[2].to_string();
        let number = |index: usize| {
            fields
                .get(index)
                .and_then(|value| value.parse().ok())
                .unwrap_or(0)
        };
        counters_by_device.insert(
            name,
            DiskCounters {
                reads: number(3),
                read_sectors: number(5),
                writes: number(7),
                write_sectors: number(9),
                io_ms: number(12),
                weighted_ms: number(13),
            },
        );
    }

    counters_by_device
}

fn flatten(value: &Value, metadata: &mut HashMap<String, Meta>) {
    if let Some(items) = value.as_array() {
        for item in items {
            flatten(item, metadata)
        }
        return;
    }
    let Some(object) = value.as_object() else {
        return;
    };
    if let Some(name) = object.get("name").and_then(Value::as_str) {
        let boolean = |key: &str| {
            object
                .get(key)
                .and_then(|value| {
                    value
                        .as_bool()
                        .or_else(|| value.as_u64().map(|number| number != 0))
                })
                .unwrap_or(false)
        };
        let rotational = boolean("rota");
        let removable = boolean("rm");
        let transport = object.get("tran").and_then(Value::as_str).unwrap_or("");
        let kind = if removable {
            "removable"
        } else if transport == "nvme" || name.starts_with("nvme") {
            "nvme"
        } else if !rotational {
            "ssd"
        } else {
            "hdd"
        };
        let capacity = object
            .get("size")
            .and_then(|value| value.as_u64().or_else(|| value.as_str()?.parse().ok()))
            .unwrap_or(0);
        let model = object
            .get("model")
            .and_then(Value::as_str)
            .map(|model| model.trim().to_string())
            .filter(|model| !model.is_empty());
        metadata.insert(
            name.into(),
            Meta {
                model,
                kind: kind.into(),
                capacity,
            },
        );
    }
    if let Some(children) = object.get("children") {
        flatten(children, metadata)
    }
}

fn refresh_meta() -> HashMap<String, Meta> {
    let mut metadata = HashMap::new();
    if let Ok(output) = Command::new("lsblk")
        .args([
            "-J",
            "-b",
            "-o",
            "NAME,TYPE,SIZE,ROTA,MODEL,MOUNTPOINTS,TRAN,RM",
        ])
        .output()
    {
        if output.status.success() {
            if let Ok(payload) = serde_json::from_slice::<Value>(&output.stdout) {
                flatten(
                    payload.get("blockdevices").unwrap_or(&Value::Null),
                    &mut metadata,
                )
            }
        }
    }

    metadata
}

fn mounts(name: &str) -> Vec<DiskMount> {
    let mut mounts = Vec::new();
    if let Ok(text) = fs::read_to_string("/proc/mounts") {
        for line in text.lines() {
            let fields: Vec<_> = line.split_whitespace().collect();
            if fields.len() < 3 || !fields[0].contains(name) {
                continue;
            }
            let Ok(path) = CString::new(fields[1]) else {
                continue;
            };
            let mut stats = std::mem::MaybeUninit::<libc::statvfs>::uninit();
            if unsafe { libc::statvfs(path.as_ptr(), stats.as_mut_ptr()) } == 0 {
                let stats = unsafe { stats.assume_init() };
                let total = stats.f_blocks as u64 * stats.f_frsize as u64;
                let available = stats.f_bavail as u64 * stats.f_frsize as u64;
                mounts.push(DiskMount {
                    path: fields[1].replace("\\040", " "),
                    fs: fields[2].into(),
                    used: total.saturating_sub(available),
                    total,
                    system: fields[1] == "/",
                });
            }
        }
    }

    mounts
}

impl DiskCollector {
    pub fn collect(&mut self, sample_ms: f64) -> Vec<DiskInfo> {
        if self
            .meta_at
            .map(|refreshed| refreshed.elapsed() >= Duration::from_secs(60))
            .unwrap_or(true)
        {
            self.meta = refresh_meta();
            self.meta_at = Some(Instant::now());
        }
        let current_counters =
            parse_diskstats(&fs::read_to_string("/proc/diskstats").unwrap_or_default());
        let elapsed_seconds = (sample_ms / 1000.0).max(0.001);
        let mut disks = Vec::new();
        for (name, current) in &current_counters {
            if !std::path::Path::new(&format!("/sys/block/{name}")).exists() {
                continue;
            }
            let Some(previous) = self.previous.get(name) else {
                continue;
            };
            let read_sectors = current.read_sectors.saturating_sub(previous.read_sectors);
            let write_sectors = current.write_sectors.saturating_sub(previous.write_sectors);
            let active_ms = current.io_ms.saturating_sub(previous.io_ms);
            let operations = current.reads.saturating_sub(previous.reads)
                + current.writes.saturating_sub(previous.writes);
            let metadata = self.meta.get(name).cloned().unwrap_or_else(|| Meta {
                kind: if name.starts_with("loop") || name.starts_with("zram") {
                    "virtual".into()
                } else {
                    "unknown".into()
                },
                capacity: read_u64(format!("/sys/block/{name}/size"))
                    .unwrap_or(0)
                    .saturating_mul(512),
                model: None,
            });
            disks.push(DiskInfo {
                name: name.clone(),
                model: metadata.model,
                kind: metadata.kind,
                capacity: metadata.capacity,
                read_rate: read_sectors as f64 * 512.0 / elapsed_seconds,
                write_rate: write_sectors as f64 * 512.0 / elapsed_seconds,
                read_iops: current.reads.saturating_sub(previous.reads) as f64 / elapsed_seconds,
                write_iops: current.writes.saturating_sub(previous.writes) as f64 / elapsed_seconds,
                active_percent: (active_ms as f64 / (elapsed_seconds * 10.0)).clamp(0.0, 100.0),
                response_ms: (operations > 0).then(|| {
                    current.weighted_ms.saturating_sub(previous.weighted_ms) as f64
                        / operations as f64
                }),
                read_total: current.read_sectors * 512,
                write_total: current.write_sectors * 512,
                mounts: mounts(name),
            });
        }
        self.previous = current_counters;
        disks.sort_by(|left, right| left.name.cmp(&right.name));
        disks
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disk_delta_rates() {
        let before = parse_diskstats("8 0 sda 10 0 100 10 20 0 200 20 0 50 60 0 0 0 0");
        let after = parse_diskstats("8 0 sda 12 0 120 12 23 0 240 24 0 70 80 0 0 0 0");
        let previous = &before["sda"];
        let current = &after["sda"];
        let seconds = 2.0;
        assert_eq!(
            current.read_sectors.saturating_sub(previous.read_sectors) as f64 * 512.0 / seconds,
            5120.0
        );
        assert_eq!(
            current.write_sectors.saturating_sub(previous.write_sectors) as f64 * 512.0 / seconds,
            10240.0
        );
        assert_eq!(
            current.writes.saturating_sub(previous.writes) as f64 / seconds,
            1.5
        );
    }
}
