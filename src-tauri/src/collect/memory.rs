// Memory and swap totals come from `/proc/meminfo`, with PSI from `/proc/pressure/memory`, on
// every snapshot because both files are cheap kernel summaries.
use super::{pressure, Availability, MemoryInfo};
use std::collections::HashMap;
use std::fs;

fn meminfo() -> HashMap<String, u64> {
    fs::read_to_string("/proc/meminfo")
        .unwrap_or_default()
        .lines()
        .filter_map(|line| {
            let (key, value) = line.split_once(':')?;
            Some((
                key.into(),
                value.split_whitespace().next()?.parse::<u64>().ok()? * 1024,
            ))
        })
        .collect()
}

pub fn collect() -> MemoryInfo {
    let memory = meminfo();
    let value = |key: &str| *memory.get(key).unwrap_or(&0);
    let cached = value("Cached") + value("SReclaimable");
    let used = value("MemTotal").saturating_sub(value("MemFree") + value("Buffers") + cached);
    let mut limits = Availability::new();
    limits.insert("hardware".into(), "memory speed and slots need root".into());

    MemoryInfo {
        total: value("MemTotal"),
        used,
        available: value("MemAvailable"),
        free: value("MemFree"),
        buffers: value("Buffers"),
        cached,
        shared: value("Shmem"),
        dirty: value("Dirty"),
        mapped: value("Mapped"),
        committed: value("Committed_AS"),
        commit_limit: value("CommitLimit"),
        swap_total: value("SwapTotal"),
        swap_used: value("SwapTotal").saturating_sub(value("SwapFree")),
        swap_cached: value("SwapCached"),
        zswap: memory.get("Zswap").copied(),
        pressure: pressure("/proc/pressure/memory"),
        limits,
    }
}
