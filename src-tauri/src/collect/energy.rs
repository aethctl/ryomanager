// Battery, AC, and package-energy counters come from power-supply and powercap sysfs on every
// sample; the previous package counter turns cumulative energy into current power.
use super::{read_f64, read_trimmed, read_u64, Availability, EnergyInfo};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Default)]
pub struct EnergyCollector {
    last_package: Option<(u64, u64, Instant)>,
}

fn supply(kind: &str) -> Option<PathBuf> {
    fs::read_dir("/sys/class/power_supply")
        .ok()?
        .flatten()
        .find_map(|entry| {
            (read_trimmed(entry.path().join("type")).as_deref() == Some(kind))
                .then_some(entry.path())
        })
}

fn micro(path: &Path, name: &str) -> Option<f64> {
    read_f64(path.join(name)).map(|value| value / 1_000_000.0)
}

fn package_counter() -> Result<(u64, u64), String> {
    let root = Path::new("/sys/class/powercap");
    let Ok(entries) = fs::read_dir(root) else {
        return Err("CPU power counters are not available on this kernel".into());
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let name = read_trimmed(path.join("name")).unwrap_or_default();
        if name.contains("package") || name.contains("pkg") {
            let energy = fs::read_to_string(path.join("energy_uj"))
                .map_err(|_| "CPU power counters are root-only on this kernel".to_string())?
                .trim()
                .parse()
                .map_err(|_| "CPU power counters are unavailable".to_string())?;
            let maximum = read_u64(path.join("max_energy_range_uj")).unwrap_or(u64::MAX);
            return Ok((energy, maximum));
        }
    }

    Err("CPU package power is not exposed by this driver".into())
}

impl EnergyCollector {
    pub fn collect(&mut self) -> EnergyInfo {
        let battery = supply("Battery");
        let mains = supply("Mains");
        let status = battery
            .as_ref()
            .and_then(|path| read_trimmed(path.join("status")));
        let source = if status.as_deref() == Some("Discharging")
            || mains
                .as_ref()
                .and_then(|path| read_u64(path.join("online")))
                == Some(0)
        {
            "battery"
        } else if mains
            .as_ref()
            .and_then(|path| read_u64(path.join("online")))
            == Some(1)
        {
            "ac"
        } else {
            "unknown"
        };
        let battery_percent = battery
            .as_ref()
            .and_then(|path| read_f64(path.join("capacity")));
        let battery_power = battery
            .as_ref()
            .and_then(|path| micro(path, "power_now"))
            .filter(|_| status.as_deref() == Some("Discharging"));
        let energy_now = battery.as_ref().and_then(|path| micro(path, "energy_now"));
        let energy_full = battery.as_ref().and_then(|path| micro(path, "energy_full"));
        let energy_design = battery
            .as_ref()
            .and_then(|path| micro(path, "energy_full_design"));
        let time_to_empty = match (energy_now, battery_power) {
            (Some(energy), Some(watts)) if watts > 0.0 => Some(energy / watts * 3600.0),
            _ => None,
        };
        let mut limits = Availability::new();
        let package_power = match package_counter() {
            Ok((value, maximum)) => {
                let sampled_at = Instant::now();
                let watts = self
                    .last_package
                    .and_then(|(previous, _, previous_sample)| {
                        let delta = if value >= previous {
                            value - previous
                        } else {
                            maximum.saturating_sub(previous).saturating_add(value)
                        };
                        let elapsed = sampled_at.duration_since(previous_sample).as_secs_f64();
                        (elapsed > 0.0).then(|| delta as f64 / 1_000_000.0 / elapsed)
                    });
                self.last_package = Some((value, maximum, sampled_at));
                watts
            }
            Err(reason) => {
                limits.insert("packagePower".into(), reason);
                None
            }
        };

        EnergyInfo {
            source: source.into(),
            battery_percent,
            battery_status: status,
            battery_power,
            battery_energy_now: energy_now,
            battery_energy_full: energy_full,
            battery_energy_design: energy_design,
            time_to_empty_seconds: time_to_empty,
            cycle_count: battery
                .as_ref()
                .and_then(|path| read_u64(path.join("cycle_count"))),
            package_power,
            limits,
        }
    }
}
