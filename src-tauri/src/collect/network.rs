// Interface counters come from `/proc/net/dev` every sample; addresses and Wi-Fi metadata
// come from `ip`, NetworkManager, and sysfs on a slower ten-second cadence.
use super::{read_f64, read_trimmed, read_u64, NetworkInfo};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::process::Command;
use std::time::{Duration, Instant};

#[derive(Clone, Debug, Default)]
pub struct NetCounters {
    pub rx: u64,
    pub tx: u64,
}

#[derive(Clone, Default)]
struct Detail {
    ipv4: Vec<String>,
    ipv6: Vec<String>,
    ssid: Option<String>,
    signal: Option<f64>,
    freq: Option<f64>,
}

#[derive(Default)]
pub struct NetworkCollector {
    previous: HashMap<String, NetCounters>,
    details: HashMap<String, Detail>,
    details_at: Option<Instant>,
}

pub fn parse_netdev(text: &str) -> HashMap<String, NetCounters> {
    let mut counters_by_interface = HashMap::new();
    for line in text.lines().skip(2) {
        let Some((name, remainder)) = line.split_once(':') else {
            continue;
        };
        let fields: Vec<_> = remainder.split_whitespace().collect();
        if fields.len() >= 9 {
            counters_by_interface.insert(
                name.trim().into(),
                NetCounters {
                    rx: fields[0].parse().unwrap_or(0),
                    tx: fields[8].parse().unwrap_or(0),
                },
            );
        }
    }

    counters_by_interface
}

fn classify_interface(name: &str, has_device: bool, has_wireless: bool) -> String {
    if name == "lo" {
        "loopback"
    } else if !has_device {
        "virtual"
    } else if has_wireless || name.starts_with("wl") {
        "wifi"
    } else if name.starts_with("en") || name.starts_with("eth") {
        "ethernet"
    } else if name.starts_with("vir")
        || name.starts_with("veth")
        || name.starts_with("docker")
        || name.starts_with("br-")
        || name.starts_with("tun")
        || name.starts_with("wg")
    {
        "virtual"
    } else {
        "unknown"
    }
    .into()
}

fn kind(name: &str) -> String {
    let base = std::path::PathBuf::from(format!("/sys/class/net/{name}"));
    classify_interface(
        name,
        base.join("device").exists(),
        base.join("wireless").exists(),
    )
}

fn refresh_details() -> HashMap<String, Detail> {
    let mut details_by_interface: HashMap<String, Detail> = HashMap::new();
    if let Ok(output) = Command::new("ip").args(["-j", "addr"]).output() {
        if let Ok(Value::Array(rows)) = serde_json::from_slice::<Value>(&output.stdout) {
            for row in rows {
                let Some(name) = row.get("ifname").and_then(Value::as_str) else {
                    continue;
                };
                let detail = details_by_interface
                    .entry(name.into())
                    .or_insert_with(Detail::default);
                if let Some(addresses) = row.get("addr_info").and_then(Value::as_array) {
                    for address in addresses {
                        let Some(local) = address.get("local").and_then(Value::as_str) else {
                            continue;
                        };
                        match address.get("family").and_then(Value::as_str) {
                            Some("inet") => detail.ipv4.push(local.into()),
                            Some("inet6") => detail.ipv6.push(local.into()),
                            _ => {}
                        }
                    }
                }
            }
        }
    }
    let wifi = details_by_interface
        .keys()
        .find(|name| kind(name) == "wifi")
        .cloned();
    if let (Some(name), Ok(output)) = (
        wifi,
        Command::new("nmcli")
            .args(["-t", "-f", "ACTIVE,SSID,SIGNAL,FREQ", "dev", "wifi"])
            .output(),
    ) {
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            let fields: Vec<_> = line.split(':').collect();
            if matches!(fields.first().copied(), Some("yes" | "true" | "*")) {
                let detail = details_by_interface.entry(name.clone()).or_default();
                detail.ssid = fields
                    .get(1)
                    .map(|value| value.to_string())
                    .filter(|value| !value.is_empty());
                detail.signal = fields.get(2).and_then(|value| value.parse().ok());
                detail.freq = fields.get(3).and_then(|value| value.parse().ok());
                break;
            }
        }
    }
    details_by_interface
}

impl NetworkCollector {
    pub fn collect(&mut self, sample_ms: f64) -> Vec<NetworkInfo> {
        if self
            .details_at
            .map(|refreshed| refreshed.elapsed() >= Duration::from_secs(10))
            .unwrap_or(true)
        {
            self.details = refresh_details();
            self.details_at = Some(Instant::now());
        }
        let now = parse_netdev(&fs::read_to_string("/proc/net/dev").unwrap_or_default());
        let dt = (sample_ms / 1000.0).max(0.001);
        let mut interfaces = Vec::new();
        for (name, counters) in &now {
            let previous = self.previous.get(name);
            let detail = self.details.get(name).cloned().unwrap_or_default();
            let base = format!("/sys/class/net/{name}");
            let driver = fs::read_link(format!("{base}/device/driver"))
                .ok()
                .and_then(|path| {
                    path.file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                });
            interfaces.push(NetworkInfo {
                name: name.clone(),
                kind: kind(name),
                state: read_trimmed(format!("{base}/operstate"))
                    .unwrap_or_else(|| "unknown".into()),
                rx_rate: previous
                    .map(|counters_before| {
                        counters.rx.saturating_sub(counters_before.rx) as f64 / dt
                    })
                    .unwrap_or(0.0),
                tx_rate: previous
                    .map(|counters_before| {
                        counters.tx.saturating_sub(counters_before.tx) as f64 / dt
                    })
                    .unwrap_or(0.0),
                rx_total: counters.rx,
                tx_total: counters.tx,
                ipv4: detail.ipv4,
                ipv6: detail.ipv6,
                mac: read_trimmed(format!("{base}/address")),
                mtu: read_u64(format!("{base}/mtu")),
                speed_mbps: read_f64(format!("{base}/speed")).filter(|speed| *speed >= 0.0),
                ssid: detail.ssid,
                signal: detail.signal,
                frequency_mhz: detail.freq,
                driver,
            });
        }
        self.previous = now;
        interfaces.sort_by(|left, right| left.name.cmp(&right.name));
        interfaces
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn netdev_delta_rates() {
        let before = parse_netdev("Inter-|\n face |\n eth0: 100 0 0 0 0 0 0 0 200 0 0 0 0 0 0 0");
        let after = parse_netdev("Inter-|\n face |\n eth0: 160 0 0 0 0 0 0 0 260 0 0 0 0 0 0 0");
        let seconds = 2.0;
        assert_eq!(
            after["eth0"].rx.saturating_sub(before["eth0"].rx) as f64 / seconds,
            30.0
        );
        assert_eq!(
            after["eth0"].tx.saturating_sub(before["eth0"].tx) as f64 / seconds,
            30.0
        );
    }

    #[test]
    fn device_less_interfaces_are_virtual() {
        assert_eq!(classify_interface("tailscale0", false, false), "virtual");
        assert_eq!(classify_interface("custom0", false, false), "virtual");
        assert_eq!(classify_interface("lo", false, false), "loopback");
    }
}
