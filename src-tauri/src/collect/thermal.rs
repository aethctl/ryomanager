// Thermal readings come from hwmon sysfs. Most chips answer in well under a
// millisecond, but a few run a device query behind the read (the NVMe SMART
// log, the SPD hub on the DIMMs, the Wi-Fi radio) and cost 2 to 12 ms each,
// which was a third of a whole snapshot. A chip that answered slowly is read
// again only every ten seconds and its last reading is kept in between.
use super::{read_f64, read_trimmed, ThermalInfo, ThermalSensor};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};

const SLOW_READ: Duration = Duration::from_millis(1);
const SLOW_CHIP_REFRESH: Duration = Duration::from_secs(10);

struct ChipCache {
    sensors: Vec<ThermalSensor>,
    read_at: Instant,
    slow: bool,
}

#[derive(Default)]
pub struct ThermalCollector {
    chips: HashMap<PathBuf, ChipCache>,
}

fn read_chip(chip_path: &PathBuf) -> Vec<ThermalSensor> {
    let chip_name = read_trimmed(chip_path.join("name")).unwrap_or_else(|| "sensor".into());
    let mut sensors = Vec::new();
    let Ok(files) = fs::read_dir(chip_path) else {
        return sensors;
    };
    for sensor_file in files.flatten() {
        let file_name = sensor_file.file_name().to_string_lossy().into_owned();
        let Some(index) = file_name
            .strip_prefix("temp")
            .and_then(|value| value.strip_suffix("_input"))
        else {
            continue;
        };
        let Some(temperature) = read_f64(sensor_file.path()).map(|value| value / 1000.0) else {
            continue;
        };
        let label = read_trimmed(chip_path.join(format!("temp{index}_label")))
            .unwrap_or_else(|| format!("Temperature {index}"));
        sensors.push(ThermalSensor {
            id: format!("{chip_name}:{index}"),
            chip: chip_name.clone(),
            label,
            temperature,
            max: read_f64(chip_path.join(format!("temp{index}_max"))).map(|value| value / 1000.0),
            critical: read_f64(chip_path.join(format!("temp{index}_crit")))
                .map(|value| value / 1000.0),
        });
    }
    sensors
}

impl ThermalCollector {
    pub fn collect(&mut self) -> ThermalInfo {
        let mut seen = Vec::new();
        let mut sensors = Vec::new();
        if let Ok(chips) = fs::read_dir("/sys/class/hwmon") {
            for chip in chips.flatten() {
                let chip_path = chip.path();
                seen.push(chip_path.clone());
                let fresh = match self.chips.get(&chip_path) {
                    Some(cache) => !cache.slow || cache.read_at.elapsed() < SLOW_CHIP_REFRESH,
                    None => false,
                };
                if !fresh {
                    let started = Instant::now();
                    let read = read_chip(&chip_path);
                    self.chips.insert(
                        chip_path.clone(),
                        ChipCache {
                            sensors: read,
                            read_at: Instant::now(),
                            slow: started.elapsed() > SLOW_READ,
                        },
                    );
                } else if let Some(cache) = self.chips.get_mut(&chip_path) {
                    if !cache.slow {
                        cache.sensors = read_chip(&chip_path);
                        cache.read_at = Instant::now();
                    }
                }
                if let Some(cache) = self.chips.get(&chip_path) {
                    sensors.extend(cache.sensors.iter().cloned());
                }
            }
        }
        self.chips.retain(|path, _| seen.contains(path));
        sensors.sort_by(|left, right| right.temperature.total_cmp(&left.temperature));

        ThermalInfo {
            hotspot: sensors.first().cloned(),
            sensors,
        }
    }
}
