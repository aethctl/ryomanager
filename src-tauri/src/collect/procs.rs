// Process metrics come from `/proc`; the sample path reads only cheap counters.
// Static identity data is cached, and exact memory and descriptor data is read on demand.
use super::{ProcessInfo, WindowRef};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::time::{Duration, Instant};

#[cfg(test)]
pub static PERF_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[derive(Clone, Debug, Default)]
struct Counters {
    user: u64,
    system: u64,
    read: u64,
    write: u64,
}

#[derive(Clone, Debug, Default)]
struct Smaps {
    pss: Option<u64>,
    anon: Option<u64>,
    file: Option<u64>,
    shmem: Option<u64>,
    swap: Option<u64>,
}

#[derive(Clone, Debug)]
struct ProcessDetails {
    exe: Option<String>,
    cwd: Option<String>,
    command: String,
    cgroup: String,
    cgroup_sampled_at: Instant,
}

#[derive(Default)]
pub struct ProcessCollector {
    previous: HashMap<(u32, u64), Counters>,
    details: HashMap<(u32, u64), ProcessDetails>,
    users: HashMap<u32, String>,
    boot_time: Option<f64>,
}

#[derive(Debug)]
struct Stat {
    comm: String,
    state: char,
    ppid: u32,
    flags: u64,
    user: u64,
    system: u64,
    priority: i64,
    nice: i64,
    threads: u64,
    start_ticks: u64,
    last_cpu: Option<u32>,
}

#[derive(Debug)]
struct Statm {
    virtual_pages: u64,
    resident_pages: u64,
    shared_pages: u64,
}

#[derive(Debug, Default)]
struct Status {
    rss_anon: Option<u64>,
    rss_file: Option<u64>,
    rss_shmem: Option<u64>,
    swap: Option<u64>,
    voluntary_context_switches: Option<u64>,
    involuntary_context_switches: Option<u64>,
}

struct RawProcess {
    pid: u32,
    stat: Stat,
    statm: Statm,
    uid: u32,
    io: Option<(u64, u64)>,
}

pub fn parse_stat(
    text: &str,
) -> Option<(
    u32,
    String,
    char,
    u32,
    u64,
    u64,
    u64,
    i64,
    i64,
    u64,
    u64,
    Option<u32>,
)> {
    let open = text.find('(')?;
    let close = text.rfind(") ")?;
    let pid = text[..open].trim().parse().ok()?;
    let comm = text[open + 1..close].to_string();
    let mut selected = [None; 10];
    let mut field_count = 0;
    for (index, field) in text[close + 2..].split_whitespace().enumerate() {
        field_count = index + 1;
        let slot = match index {
            0 => Some(0),
            1 => Some(1),
            6 => Some(2),
            11 => Some(3),
            12 => Some(4),
            15 => Some(5),
            16 => Some(6),
            17 => Some(7),
            19 => Some(8),
            36 => Some(9),
            _ => None,
        };
        if let Some(slot) = slot {
            selected[slot] = Some(field);
        }
        if index == 36 {
            break;
        }
    }
    if field_count < 37 {
        return None;
    }
    Some((
        pid,
        comm,
        selected[0]?.chars().next()?,
        selected[1]?.parse().ok()?,
        selected[2]?.parse().ok()?,
        selected[3]?.parse().ok()?,
        selected[4]?.parse().ok()?,
        selected[5]?.parse().ok()?,
        selected[6]?.parse().ok()?,
        selected[7]?.parse().ok()?,
        selected[8]?.parse().ok()?,
        selected[9]?.parse().ok(),
    ))
}

pub fn process_start_time(pid: u32) -> Option<u64> {
    let ticks = process_start_ticks(pid)?;
    let clock_ticks = unsafe { libc::sysconf(libc::_SC_CLK_TCK) }.max(1) as f64;
    Some((boot_epoch() + ticks as f64 / clock_ticks).max(0.0) as u64)
}

fn stat(pid: u32) -> Option<Stat> {
    let text = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let (
        _,
        comm,
        state,
        parent_pid,
        flags,
        user,
        system,
        priority,
        nice,
        threads,
        start_ticks,
        last_cpu,
    ) = parse_stat(&text)?;

    Some(Stat {
        comm,
        state,
        ppid: parent_pid,
        flags,
        user,
        system,
        priority,
        nice,
        threads,
        start_ticks,
        last_cpu,
    })
}

pub fn process_start_ticks(pid: u32) -> Option<u64> {
    stat(pid).map(|stats| stats.start_ticks)
}

fn statm(pid: u32) -> Option<Statm> {
    let text = fs::read_to_string(format!("/proc/{pid}/statm")).ok()?;
    let mut fields = text.split_whitespace();
    Some(Statm {
        virtual_pages: fields.next()?.parse().ok()?,
        resident_pages: fields.next()?.parse().ok()?,
        shared_pages: fields.next()?.parse().ok()?,
    })
}

fn status(pid: u32) -> Status {
    let mut status = Status::default();
    let Ok(text) = fs::read_to_string(format!("/proc/{pid}/status")) else {
        return status;
    };
    for line in text.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let value = value
            .split_whitespace()
            .next()
            .and_then(|value| value.parse::<u64>().ok());
        match key {
            "RssAnon" => status.rss_anon = value.map(|value| value * 1024),
            "RssFile" => status.rss_file = value.map(|value| value * 1024),
            "RssShmem" => status.rss_shmem = value.map(|value| value * 1024),
            "VmSwap" => status.swap = value.map(|value| value * 1024),
            "voluntary_ctxt_switches" => status.voluntary_context_switches = value,
            "nonvoluntary_ctxt_switches" => status.involuntary_context_switches = value,
            _ => {}
        }
    }
    status
}

fn io(pid: u32) -> Option<(u64, u64)> {
    let text = fs::read_to_string(format!("/proc/{pid}/io")).ok()?;
    let mut read_bytes = None;
    let mut write_bytes = None;
    for line in text.lines() {
        if let Some(value) = line.strip_prefix("read_bytes:") {
            read_bytes = value.trim().parse().ok()
        }
        if let Some(value) = line.strip_prefix("write_bytes:") {
            write_bytes = value.trim().parse().ok()
        }
    }

    Some((read_bytes?, write_bytes?))
}

fn smaps(pid: u32) -> Option<Smaps> {
    let text = fs::read_to_string(format!("/proc/{pid}/smaps_rollup")).ok()?;
    let mut smaps = Smaps::default();
    for line in text.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let Some(bytes) = value
            .split_whitespace()
            .next()
            .and_then(|value| value.parse::<u64>().ok())
            .map(|value| value * 1024)
        else {
            continue;
        };
        match key {
            "Pss" => smaps.pss = Some(bytes),
            "Pss_Anon" => smaps.anon = Some(bytes),
            "Pss_File" => smaps.file = Some(bytes),
            "Pss_Shmem" => smaps.shmem = Some(bytes),
            "Swap" => smaps.swap = Some(bytes),
            _ => {}
        }
    }

    Some(smaps)
}

fn cgroup(pid: u32) -> String {
    fs::read_to_string(format!("/proc/{pid}/cgroup"))
        .ok()
        .and_then(|text| {
            text.lines()
                .find_map(|line| line.split_once("::").map(|(_, cgroup)| cgroup.to_string()))
        })
        .unwrap_or_default()
}

pub fn unit_from_cgroup(cgroup: &str) -> Option<String> {
    cgroup
        .rsplit('/')
        .find(|part| part.ends_with(".service") || part.ends_with(".scope"))
        .map(str::to_string)
}

fn state_name(state: char) -> String {
    match state {
        'R' => "running",
        'S' => "sleeping",
        'D' => "waiting",
        'T' | 't' => "stopped",
        'Z' => "zombie",
        'I' => "idle",
        'X' | 'x' => "dead",
        _ => "unknown",
    }
    .into()
}

fn boot_epoch() -> f64 {
    fs::read_to_string("/proc/stat")
        .ok()
        .and_then(|text| {
            text.lines()
                .find_map(|line| line.strip_prefix("btime ")?.parse::<f64>().ok())
        })
        .unwrap_or(0.0)
}

fn command(pid: u32) -> String {
    fs::read(format!("/proc/{pid}/cmdline"))
        .ok()
        .map(|bytes| {
            String::from_utf8_lossy(&bytes)
                .split('\0')
                .filter(|argument| !argument.is_empty())
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_default()
}

fn link(pid: u32, name: &str) -> Option<String> {
    fs::read_link(format!("/proc/{pid}/{name}"))
        .ok()
        .map(|path| path.to_string_lossy().into_owned())
}

fn descriptor_counts(pid: u32) -> (Option<u64>, Option<u64>) {
    let Ok(entries) = fs::read_dir(format!("/proc/{pid}/fd")) else {
        return (None, None);
    };
    let mut fds = 0;
    let mut process_sockets: HashSet<u64> = HashSet::new();
    for entry in entries.flatten() {
        fds += 1;
        let Ok(path) = fs::read_link(entry.path()) else {
            continue;
        };
        let target = path.to_string_lossy();
        if let Some(inode) = target
            .strip_prefix("socket:[")
            .and_then(|value| value.strip_suffix(']'))
            .and_then(|value| value.parse().ok())
        {
            process_sockets.insert(inode);
        }
    }
    if process_sockets.is_empty() {
        return (Some(fds), Some(0));
    }

    let socket_count = process_sockets.len();
    for name in ["tcp", "tcp6", "udp", "udp6"] {
        if let Ok(text) = fs::read_to_string(format!("/proc/net/{name}")) {
            for line in text.lines().skip(1) {
                if let Some(inode) = line
                    .split_whitespace()
                    .nth(9)
                    .and_then(|value| value.parse().ok())
                {
                    process_sockets.remove(&inode);
                }
                if process_sockets.is_empty() {
                    return (Some(fds), Some(socket_count as u64));
                }
            }
        }
    }

    (
        Some(fds),
        Some(socket_count.saturating_sub(process_sockets.len()) as u64),
    )
}

fn read_users() -> HashMap<u32, String> {
    fs::read_to_string("/etc/passwd")
        .unwrap_or_default()
        .lines()
        .filter_map(|line| {
            let fields: Vec<_> = line.split(':').collect();
            Some((fields.get(2)?.parse().ok()?, fields.first()?.to_string()))
        })
        .collect()
}

pub fn process_detail(mut process: ProcessInfo) -> ProcessInfo {
    let pid = process.pid;
    let (status, smaps, descriptors, oom_score) = std::thread::scope(|scope| {
        let smaps = scope.spawn(|| smaps(pid));
        let status = status(pid);
        let oom_score = fs::read_to_string(format!("/proc/{pid}/oom_score"))
            .ok()
            .and_then(|value| value.trim().parse().ok());
        let descriptors = descriptor_counts(pid);
        (
            status,
            smaps.join().expect("smaps reader panicked"),
            descriptors,
            oom_score,
        )
    });
    if let Some(pss) = smaps.as_ref().and_then(|values| values.pss) {
        process.memory = pss;
        process.memory_kind = "pss".into();
    }
    process.rss_anon = smaps
        .as_ref()
        .and_then(|values| values.anon)
        .or(status.rss_anon);
    process.rss_file = smaps
        .as_ref()
        .and_then(|values| values.file)
        .or(status.rss_file);
    process.rss_shmem = smaps
        .as_ref()
        .and_then(|values| values.shmem)
        .or(status.rss_shmem);
    process.swap = smaps
        .as_ref()
        .and_then(|values| values.swap)
        .or(status.swap);
    process.ctx_switches = match (
        status.voluntary_context_switches,
        status.involuntary_context_switches,
    ) {
        (Some(voluntary), Some(involuntary)) => Some(voluntary + involuntary),
        _ => None,
    };
    process.oom_score = oom_score;
    (process.fds, process.connections) = descriptors;
    process
}

impl ProcessCollector {
    pub fn collect(
        &mut self,
        sample_ms: f64,
        logical_cpus: u64,
        windows: &[WindowRef],
    ) -> Vec<ProcessInfo> {
        if self.users.is_empty() {
            self.users = read_users();
        }
        let own_uid = unsafe { libc::geteuid() };
        let clock_ticks = unsafe { libc::sysconf(libc::_SC_CLK_TCK) }.max(1) as f64;
        let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) }.max(1) as u64;
        let boot_time = *self.boot_time.get_or_insert_with(boot_epoch);
        let elapsed_seconds = (sample_ms / 1000.0).max(0.001);
        let now = Instant::now();
        let pids: Vec<u32> = fs::read_dir("/proc")
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|entry| entry.file_name().to_str()?.parse().ok())
            .collect();
        let worker_count = (logical_cpus as usize).clamp(1, 4).min(pids.len().max(1));
        let chunk_size = pids.len().div_ceil(worker_count);
        let raw = if pids.is_empty() {
            Vec::new()
        } else {
            std::thread::scope(|scope| {
                let handles: Vec<_> = pids
                    .chunks(chunk_size)
                    .map(|chunk| {
                        scope.spawn(move || {
                            chunk
                                .iter()
                                .filter_map(|&pid| {
                                    let stat = stat(pid)?;
                                    let statm = statm(pid)?;
                                    let uid = fs::metadata(format!("/proc/{pid}")).ok()?.uid();
                                    Some(RawProcess {
                                        pid,
                                        stat,
                                        statm,
                                        uid,
                                        io: (uid == own_uid).then(|| io(pid)).flatten(),
                                    })
                                })
                                .collect::<Vec<_>>()
                        })
                    })
                    .collect();
                let mut raw = Vec::with_capacity(pids.len());
                for handle in handles {
                    raw.extend(handle.join().expect("process sampler panicked"));
                }
                raw
            })
        };
        let mut next = HashMap::with_capacity(raw.len());
        let mut out = Vec::with_capacity(raw.len());
        for raw in raw {
            let RawProcess {
                pid,
                stat: st,
                statm,
                uid,
                io: io_now,
            } = raw;
            let identity = (pid, st.start_ticks);
            let (exe, cwd, command, cgroup) = {
                let details = self
                    .details
                    .entry(identity)
                    .or_insert_with(|| ProcessDetails {
                        exe: link(pid, "exe"),
                        cwd: link(pid, "cwd"),
                        command: command(pid),
                        cgroup: cgroup(pid),
                        cgroup_sampled_at: now
                            .checked_sub(Duration::from_millis(pid as u64 % 30_000))
                            .unwrap_or(now),
                    });
                if now.duration_since(details.cgroup_sampled_at) >= Duration::from_secs(30) {
                    details.cgroup = cgroup(pid);
                    details.cgroup_sampled_at = now;
                }
                (
                    details.exe.clone(),
                    details.cwd.clone(),
                    details.command.clone(),
                    details.cgroup.clone(),
                )
            };
            let rss = statm.resident_pages.saturating_mul(page_size);
            let private_memory = statm
                .resident_pages
                .saturating_sub(statm.shared_pages)
                .saturating_mul(page_size);
            let counters = Counters {
                user: st.user,
                system: st.system,
                read: io_now.map(|values| values.0).unwrap_or(0),
                write: io_now.map(|values| values.1).unwrap_or(0),
            };
            let previous = self.previous.get(&identity);
            let cpu_scale = 100.0 / (clock_ticks * elapsed_seconds * logical_cpus.max(1) as f64);
            let cpu_user = previous
                .map(|counters| st.user.saturating_sub(counters.user) as f64 * cpu_scale)
                .unwrap_or(0.0);
            let cpu_system = previous
                .map(|counters| st.system.saturating_sub(counters.system) as f64 * cpu_scale)
                .unwrap_or(0.0);
            let disk_read = io_now.and_then(|(read, _)| {
                previous.map(|counters| read.saturating_sub(counters.read) as f64 / elapsed_seconds)
            });
            let disk_write = io_now.and_then(|(_, write)| {
                previous
                    .map(|counters| write.saturating_sub(counters.write) as f64 / elapsed_seconds)
            });
            next.insert(identity, counters);
            let process_windows: Vec<_> = windows
                .iter()
                .filter(|window| window.pid == Some(pid))
                .cloned()
                .collect();
            out.push(ProcessInfo {
                pid,
                parent_pid: (st.ppid > 0).then_some(st.ppid),
                start_time: (boot_time + st.start_ticks as f64 / clock_ticks).max(0.0) as u64,
                name: st.comm,
                exe,
                command,
                cwd,
                user: self
                    .users
                    .get(&uid)
                    .cloned()
                    .unwrap_or_else(|| uid.to_string()),
                uid,
                state: state_name(st.state),
                kernel_thread: st.flags & 0x20_0000 != 0,
                unit: unit_from_cgroup(&cgroup),
                cpu: cpu_user + cpu_system,
                cpu_user,
                cpu_system,
                memory: private_memory,
                memory_kind: "private".into(),
                rss,
                rss_anon: None,
                rss_file: None,
                rss_shmem: None,
                swap: None,
                virtual_: statm.virtual_pages.saturating_mul(page_size),
                disk_read,
                disk_write,
                disk_read_total: io_now.map(|counters| counters.0),
                disk_write_total: io_now.map(|counters| counters.1),
                gpu: None,
                gpu_memory: None,
                gpu_index: None,
                connections: None,
                energy: "none".into(),
                energy_score: 0.0,
                threads: st.threads,
                fds: None,
                nice: st.nice,
                priority: st.priority,
                ctx_switches: None,
                oom_score: None,
                last_cpu: st.last_cpu,
                windows: process_windows,
                cgroup,
                root_id: pid,
            });
        }
        self.previous = next;
        self.details
            .retain(|key, _| self.previous.contains_key(key));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stat_comm_can_contain_spaces_and_parentheses() {
        let mut fields = vec!["0"; 37];
        fields[0] = "S";
        fields[1] = "1";
        fields[6] = "2097152";
        fields[11] = "120";
        fields[12] = "30";
        fields[15] = "20";
        fields[16] = "5";
        fields[17] = "7";
        fields[19] = "900";
        fields[20] = "4096";
        fields[21] = "10";
        fields[36] = "4";
        let text = format!("123 (weird (worker) name) {}", fields.join(" "));
        let parsed = parse_stat(&text).unwrap();
        assert_eq!(parsed.1, "weird (worker) name");
        assert_eq!(parsed.2, 'S');
        assert_eq!(parsed.3, 1);
        assert_eq!(parsed.10, 900);
    }

    #[test]
    fn list_defers_exact_process_details() {
        let mut collector = ProcessCollector::default();
        let _guard = PERF_TEST_LOCK
            .lock()
            .expect("performance test lock poisoned");
        let processes = collector.collect(1000.0, 1, &[]);
        let process = processes
            .into_iter()
            .find(|process| process.pid == std::process::id())
            .expect("collector omitted itself");
        assert_eq!(process.memory_kind, "private");
        assert!(process.memory <= process.rss);
        assert_eq!(process.rss_anon, None);
        assert_eq!(process.rss_file, None);
        assert_eq!(process.rss_shmem, None);
        assert_eq!(process.swap, None);
        assert_eq!(process.fds, None);
        assert_eq!(process.ctx_switches, None);
        assert_eq!(process.oom_score, None);
        assert_eq!(process.connections, None);

        let _listener =
            std::net::TcpListener::bind(("127.0.0.1", 0)).expect("bind detail test socket");
        let started_at = Instant::now();
        let detail = process_detail(process);
        let elapsed = started_at.elapsed();
        eprintln!("process detail: {:.2} ms", elapsed.as_secs_f64() * 1000.0);
        assert_eq!(detail.memory_kind, "pss");
        assert!(detail.fds.is_some());
        assert!(detail.ctx_switches.is_some());
        assert!(detail.oom_score.is_some());
        assert!(detail
            .connections
            .is_some_and(|connections| connections >= 1));
        if !cfg!(debug_assertions) {
            assert!(
                elapsed < Duration::from_millis(5),
                "process detail took {elapsed:?}"
            );
        }
    }
}
