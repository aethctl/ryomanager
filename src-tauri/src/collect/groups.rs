// Process groups are rebuilt from each `/proc` snapshot; cgroups define roots while desktop
// entries and cached unit descriptions provide stable identities and presentation names.
use super::desktop::DesktopIndex;
use super::units::UnitDescriptions;
use super::{ProcessGroup, ProcessInfo, WindowRef};
use std::collections::{HashMap, HashSet};

fn root_candidate(process: &ProcessInfo) -> bool {
    process
        .unit
        .as_deref()
        .map(|unit| {
            unit.starts_with("app-")
                || unit.ends_with(".service")
                || (unit.starts_with("session-") && unit.ends_with(".scope"))
        })
        .unwrap_or(false)
}

fn user_manager_uid(process: &ProcessInfo) -> Option<u32> {
    if !process.cgroup.ends_with("/init.scope") {
        return None;
    }

    process.cgroup.split('/').find_map(|segment| {
        segment
            .strip_prefix("user@")?
            .strip_suffix(".service")?
            .parse()
            .ok()
    })
}

fn is_user_manager(process: &ProcessInfo) -> bool {
    user_manager_uid(process).is_some()
}

fn system_process(process: &ProcessInfo) -> bool {
    process.kernel_thread
        || process.cgroup.starts_with("/system.slice/")
        || process.cgroup == "/init.scope"
        || is_user_manager(process)
        || process
            .unit
            .as_deref()
            .map(|unit| unit.starts_with("session-") && unit.ends_with(".scope"))
            .unwrap_or(false)
}

fn assign_roots(processes: &mut [ProcessInfo]) {
    let root_ids = {
        let process_view: &[ProcessInfo] = processes;
        let index_by_pid: HashMap<u32, usize> = process_view
            .iter()
            .enumerate()
            .map(|(index, process)| (process.pid, index))
            .collect();
        let mut user_manager_roots = HashMap::<u32, u32>::new();

        for process in process_view {
            if let Some(uid) = user_manager_uid(process) {
                user_manager_roots
                    .entry(uid)
                    .and_modify(|root_pid| *root_pid = (*root_pid).min(process.pid))
                    .or_insert(process.pid);
            }
        }

        process_view
            .iter()
            .map(|process| {
                if process.kernel_thread {
                    return 0;
                }
                if process.cgroup == "/init.scope" {
                    return 1;
                }
                if let Some(uid) = user_manager_uid(process) {
                    return user_manager_roots.get(&uid).copied().unwrap_or(process.pid);
                }
                if root_candidate(process) {
                    let unit = process.unit.as_deref();
                    let mut root_pid = process.pid;
                    let mut current = process;

                    while let Some(parent) = current.parent_pid.and_then(|parent_pid| {
                        index_by_pid
                            .get(&parent_pid)
                            .map(|index| &process_view[*index])
                    }) {
                        if parent.unit.as_deref() != unit {
                            break;
                        }
                        root_pid = parent.pid;
                        current = parent;
                    }

                    return root_pid;
                }

                let mut root_pid = process.pid;
                let mut current = process;
                let mut seen = HashSet::new();

                while let Some(parent) = current.parent_pid.and_then(|parent_pid| {
                    index_by_pid
                        .get(&parent_pid)
                        .map(|index| &process_view[*index])
                }) {
                    if !seen.insert(parent.pid) {
                        break;
                    }
                    if root_candidate(parent) {
                        root_pid = parent.pid;
                        let unit = parent.unit.as_deref();
                        let mut top = parent;

                        while let Some(next) = top.parent_pid.and_then(|parent_pid| {
                            index_by_pid
                                .get(&parent_pid)
                                .map(|index| &process_view[*index])
                        }) {
                            if next.unit.as_deref() != unit {
                                break;
                            }
                            root_pid = next.pid;
                            top = next;
                        }
                        break;
                    }
                    current = parent;
                }

                root_pid
            })
            .collect::<Vec<_>>()
    };

    for (process, root_id) in processes.iter_mut().zip(root_ids) {
        process.root_id = root_id;
    }
}

pub fn energy_score(cpu: f64, gpu: Option<f64>, read: Option<f64>, write: Option<f64>) -> f64 {
    cpu + 0.8 * gpu.unwrap_or(0.0)
        + 30.0 * ((read.unwrap_or(0.0) + write.unwrap_or(0.0)) / (50.0 * 1024.0 * 1024.0)).min(1.0)
}

pub fn energy_band(score: f64) -> String {
    if score == 0.0 {
        "none"
    } else if score < 2.0 {
        "very-low"
    } else if score < 8.0 {
        "low"
    } else if score < 20.0 {
        "moderate"
    } else if score < 45.0 {
        "high"
    } else {
        "very-high"
    }
    .into()
}

#[derive(Clone)]
struct RootMeta {
    key: String,
    name: String,
    icon: Option<String>,
    unit: Option<String>,
    unit_subtitle: Option<String>,
    desktop: bool,
    root: u32,
}

fn label_like_description(description: &str) -> bool {
    description.chars().count() <= 26
        && description.split_whitespace().count() <= 3
        && !["A ", "An ", "The "]
            .iter()
            .any(|article| description.starts_with(article))
}

fn service_presentation(unit_name: &str, description: Option<&str>) -> (String, Option<String>) {
    let fallback_name = unit_name
        .trim_end_matches(".service")
        .trim_end_matches(".scope")
        .to_string();

    match description {
        Some(description) if label_like_description(description) => (description.to_string(), None),
        Some(description) => (fallback_name, Some(format!("{description} · {unit_name}"))),
        None => (fallback_name, None),
    }
}

fn app_unit_desktop_id(unit_name: &str) -> Option<String> {
    let encoded_id = unit_name.strip_prefix("app-")?;
    let encoded_id = if let Some(desktop_id) = encoded_id.strip_suffix("@autostart.service") {
        desktop_id
    } else if let Some(scope) = encoded_id.strip_suffix(".scope") {
        let (desktop_id, pid) = scope.rsplit_once('-')?;
        pid.chars()
            .all(|character| character.is_ascii_digit())
            .then_some(desktop_id)?
    } else {
        return None;
    };

    Some(encoded_id.replace("\\x2d", "-"))
}

fn root_meta(
    root: &ProcessInfo,
    app_ids: &[String],
    desktop: &DesktopIndex,
    units: &UnitDescriptions,
) -> RootMeta {
    if root.kernel_thread {
        return RootMeta {
            key: "kernel".into(),
            name: "Kernel threads".into(),
            icon: None,
            unit: None,
            unit_subtitle: None,
            desktop: false,
            root: 0,
        };
    }
    if is_user_manager(root) {
        return RootMeta {
            key: "unit:user-manager".into(),
            name: "systemd (user session)".into(),
            icon: None,
            unit: Some("user-manager".into()),
            unit_subtitle: None,
            desktop: false,
            root: root.pid,
        };
    }
    if root.cgroup == "/init.scope" {
        return RootMeta {
            key: "unit:init.scope".into(),
            name: "systemd".into(),
            icon: None,
            unit: Some("init.scope".into()),
            unit_subtitle: None,
            desktop: false,
            root: 1,
        };
    }
    let unit = root.unit.clone();
    if let Some(desktop_id) = unit.as_deref().and_then(app_unit_desktop_id) {
        let entry = desktop
            .resolve(std::slice::from_ref(&desktop_id), None, root.exe.as_deref())
            .cloned();
        let key_id = entry
            .as_ref()
            .map(|desktop_entry| desktop_entry.id.clone())
            .unwrap_or_else(|| desktop_id.clone());
        // "org.chromium.Chromium" with no entry on disk still reads as "Chromium".
        let fallback_name = desktop_id
            .rsplit('.')
            .next()
            .filter(|segment| !segment.is_empty())
            .unwrap_or(&desktop_id)
            .to_string();
        return RootMeta {
            key: format!("app:{key_id}"),
            name: entry
                .as_ref()
                .map(|desktop_entry| desktop_entry.name.clone())
                .unwrap_or(fallback_name),
            icon: entry.and_then(|desktop_entry| desktop_entry.icon.map(|_| key_id)),
            unit,
            unit_subtitle: None,
            desktop: true,
            root: root.pid,
        };
    }

    if root.cgroup.starts_with("/system.slice/")
        || unit
            .as_deref()
            .map(|unit_name| unit_name.ends_with(".service"))
            .unwrap_or(false)
    {
        if let Some(unit_name) = &unit {
            let (name, unit_subtitle) = if unit_name.starts_with("docker-") {
                (
                    format!(
                        "Container {}",
                        unit_name
                            .trim_start_matches("docker-")
                            .chars()
                            .take(12)
                            .collect::<String>()
                    ),
                    None,
                )
            } else {
                service_presentation(unit_name, units.get(unit_name))
            };
            return RootMeta {
                key: format!("unit:{unit_name}"),
                name,
                icon: None,
                unit,
                unit_subtitle,
                desktop: false,
                root: root.pid,
            };
        }
    }
    let mut apps: Vec<String> = app_ids.to_vec();
    apps.extend(
        root.windows
            .iter()
            .map(|window| window.app_id.clone())
            .filter(|app_id| !app_id.is_empty()),
    );
    if let Some(entry) = desktop
        .resolve(&apps, unit.as_deref(), root.exe.as_deref())
        .cloned()
    {
        return RootMeta {
            key: format!("app:{}", entry.id),
            name: entry.name,
            icon: entry.icon.map(|_| entry.id),
            unit,
            unit_subtitle: None,
            desktop: true,
            root: root.pid,
        };
    }
    if let Some(executable) = &root.exe {
        return RootMeta {
            key: format!("exe:{executable}"),
            name: root.name.clone(),
            icon: None,
            unit,
            unit_subtitle: None,
            desktop: false,
            root: root.pid,
        };
    }
    RootMeta {
        key: format!("name:{}", root.name),
        name: root.name.clone(),
        icon: None,
        unit,
        unit_subtitle: None,
        desktop: false,
        root: root.pid,
    }
}

fn own_identity_key(
    process: &ProcessInfo,
    desktop: &DesktopIndex,
    units: &UnitDescriptions,
) -> String {
    if process.kernel_thread || is_user_manager(process) || process.cgroup == "/init.scope" {
        return root_meta(process, &[], desktop, units).key;
    }

    let unit = process.unit.as_deref();
    if unit.and_then(app_unit_desktop_id).is_some() {
        return root_meta(process, &[], desktop, units).key;
    }

    if process.cgroup.starts_with("/system.slice/")
        || unit
            .map(|unit_name| unit_name.ends_with(".service"))
            .unwrap_or(false)
    {
        return root_meta(process, &[], desktop, units).key;
    }

    let identity_scope = root_candidate(process).then_some(unit).flatten();
    if let Some(entry) = desktop.resolve(&[], identity_scope, process.exe.as_deref()) {
        return format!("app:{}", entry.id);
    }
    if let Some(executable) = &process.exe {
        return format!("exe:{executable}");
    }

    format!("name:{}", process.name)
}

fn instance_count(
    group_key: &str,
    members: &[ProcessInfo],
    desktop: &DesktopIndex,
    units: &UnitDescriptions,
) -> u64 {
    if group_key == "kernel" {
        return 1;
    }

    members
        .iter()
        .filter(|process| process.pid == process.root_id)
        .filter(|process| own_identity_key(process, desktop, units) == group_key)
        .map(|process| process.root_id)
        .collect::<HashSet<_>>()
        .len() as u64
}

pub fn group_processes(
    processes: &mut Vec<ProcessInfo>,
    desktop: &mut DesktopIndex,
    units: &mut UnitDescriptions,
    windows_source: &str,
) -> Vec<ProcessGroup> {
    desktop.refresh();
    assign_roots(processes);
    units.refresh(processes.iter().filter_map(|process| process.unit.as_ref()));
    let by_pid: HashMap<u32, usize> = processes
        .iter()
        .enumerate()
        .map(|(index, process)| (process.pid, index))
        .collect();
    let mut root_apps: HashMap<u32, Vec<String>> = HashMap::new();
    for process in processes.iter() {
        root_apps.entry(process.root_id).or_default().extend(
            process
                .windows
                .iter()
                .map(|window| window.app_id.clone())
                .filter(|app_id| !app_id.is_empty()),
        );
    }
    let mut metas = HashMap::new();
    for process in processes.iter() {
        if process.root_id == 0 {
            metas.entry(0).or_insert_with(|| {
                root_meta(
                    process,
                    root_apps.get(&0).map(Vec::as_slice).unwrap_or(&[]),
                    desktop,
                    units,
                )
            });
        } else if let Some(index) = by_pid.get(&process.root_id) {
            let root = &processes[*index];
            metas.entry(process.root_id).or_insert_with(|| {
                root_meta(
                    root,
                    root_apps
                        .get(&process.root_id)
                        .map(Vec::as_slice)
                        .unwrap_or(&[]),
                    desktop,
                    units,
                )
            });
        }
    }
    let mut buckets: HashMap<String, (RootMeta, Vec<ProcessInfo>)> = HashMap::new();
    for mut process in processes.drain(..) {
        let metadata = metas
            .get(&process.root_id)
            .cloned()
            .unwrap_or_else(|| root_meta(&process, &[], desktop, units));
        process.energy_score = energy_score(
            process.cpu,
            process.gpu,
            process.disk_read,
            process.disk_write,
        );
        process.energy = energy_band(process.energy_score);
        let bucket = buckets
            .entry(metadata.key.clone())
            .or_insert_with(|| (metadata.clone(), Vec::new()));
        bucket.1.push(process);
    }
    let mut out = Vec::new();
    for (_, (metadata, mut members)) in buckets {
        members.sort_by(|left, right| right.cpu.total_cmp(&left.cpu));
        let windows: Vec<WindowRef> = members
            .iter()
            .flat_map(|process| process.windows.clone())
            .collect();
        let category = if !windows.is_empty()
            || (windows_source == "none"
                && metadata.desktop
                && members.iter().any(|process| {
                    process
                        .unit
                        .as_deref()
                        .map(|unit| unit.starts_with("app-"))
                        .unwrap_or(false)
                })) {
            "apps"
        } else if members.iter().any(system_process) {
            "system"
        } else {
            "background"
        };
        let memory_kind = "private";
        let sum_optional_f64 = |values: Vec<Option<f64>>| {
            let present: Vec<f64> = values.into_iter().flatten().collect();
            (!present.is_empty()).then(|| present.iter().sum())
        };
        let sum_optional_u64 = |values: Vec<Option<u64>>| {
            let present: Vec<u64> = values.into_iter().flatten().collect();
            (!present.is_empty()).then(|| present.iter().sum())
        };
        let score: f64 = members.iter().map(|process| process.energy_score).sum();
        let state = if metadata.root == 0 {
            members
                .first()
                .map(|process| process.state.as_str())
                .unwrap_or("unknown")
        } else {
            members
                .iter()
                .find(|process| process.pid == metadata.root)
                .map(|process| process.state.as_str())
                .unwrap_or("unknown")
        };
        let subtitle = if category == "apps" {
            if windows.is_empty() {
                members
                    .iter()
                    .find(|process| process.pid == metadata.root)
                    .map(|process| process.command.clone())
                    .unwrap_or_default()
            } else {
                format!(
                    "{} window{} · {} process{}",
                    windows.len(),
                    if windows.len() == 1 { "" } else { "s" },
                    members.len(),
                    if members.len() == 1 { "" } else { "es" }
                )
            }
        } else if metadata.key == "kernel" {
            format!("{} threads", members.len())
        } else if let Some(unit_subtitle) = &metadata.unit_subtitle {
            unit_subtitle.clone()
        } else if let Some(unit) = &metadata.unit {
            unit.clone()
        } else {
            members
                .iter()
                .find(|process| process.pid == metadata.root)
                .map(|process| process.command.clone())
                .unwrap_or_default()
        };
        let instances = instance_count(&metadata.key, &members, desktop, units);
        out.push(ProcessGroup {
            key: metadata.key,
            category: category.into(),
            name: metadata.name,
            subtitle,
            icon_key: metadata.icon,
            unit: metadata.unit,
            leader_pid: if metadata.root == 0 {
                members.first().map(|process| process.pid).unwrap_or(0)
            } else {
                metadata.root
            },
            instances,
            windows,
            state: state.into(),
            cpu: members.iter().map(|process| process.cpu).sum(),
            memory: members.iter().map(|process| process.memory).sum(),
            memory_kind: memory_kind.into(),
            disk_read: sum_optional_f64(members.iter().map(|process| process.disk_read).collect()),
            disk_write: sum_optional_f64(
                members.iter().map(|process| process.disk_write).collect(),
            ),
            gpu: sum_optional_f64(members.iter().map(|process| process.gpu).collect()),
            gpu_memory: sum_optional_u64(
                members.iter().map(|process| process.gpu_memory).collect(),
            ),
            connections: sum_optional_u64(
                members.iter().map(|process| process.connections).collect(),
            ),
            energy: energy_band(score),
            energy_score: score,
            threads: members.iter().map(|process| process.threads).sum(),
            members,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn process(
        pid: u32,
        parent_pid: Option<u32>,
        unit: Option<&str>,
        cgroup: &str,
        name: &str,
    ) -> ProcessInfo {
        ProcessInfo {
            pid,
            parent_pid,
            start_time: 10,
            name: name.into(),
            exe: Some(format!("/usr/bin/{name}")),
            command: name.into(),
            cwd: None,
            user: "user".into(),
            uid: 1000,
            state: "sleeping".into(),
            kernel_thread: false,
            unit: unit.map(str::to_string),
            cpu: 0.0,
            cpu_user: 0.0,
            cpu_system: 0.0,
            memory: 1,
            memory_kind: "private".into(),
            rss: 1,
            rss_anon: None,
            rss_file: None,
            rss_shmem: None,
            swap: None,
            virtual_: 1,
            disk_read: None,
            disk_write: None,
            disk_read_total: None,
            disk_write_total: None,
            gpu: None,
            gpu_memory: None,
            gpu_index: None,
            connections: None,
            energy: "none".into(),
            energy_score: 0.0,
            threads: 1,
            fds: None,
            nice: 0,
            priority: 20,
            ctx_switches: None,
            oom_score: None,
            last_cpu: None,
            windows: Vec::new(),
            cgroup: cgroup.into(),
            root_id: pid,
        }
    }

    #[test]
    fn bands() {
        for (value, band) in [
            (0.0, "none"),
            (1.0, "very-low"),
            (3.0, "low"),
            (10.0, "moderate"),
            (30.0, "high"),
            (50.0, "very-high"),
        ] {
            assert_eq!(energy_band(value), band);
        }
    }

    #[test]
    fn identity_merges_launches_and_keeps_system_units() {
        let mut processes = vec![
            process(
                10,
                None,
                Some("app-editor-10.scope"),
                "/user.slice/app.slice/app-editor-10.scope",
                "editor",
            ),
            process(
                11,
                Some(10),
                None,
                "/user.slice/app.slice/helper.scope",
                "helper",
            ),
            process(
                20,
                None,
                Some("app-editor-20.scope"),
                "/user.slice/app.slice/app-editor-20.scope",
                "editor",
            ),
            process(
                30,
                Some(1),
                Some("dbus.service"),
                "/system.slice/dbus.service",
                "dbus",
            ),
        ];
        processes[1].windows.push(WindowRef {
            id: "w1".into(),
            title: "Draft".into(),
            app_id: "editor".into(),
            workspace: "1".into(),
            output: "eDP-1".into(),
            focused: true,
            pid: Some(11),
        });
        let mut desktop = DesktopIndex::default();
        let mut units = UnitDescriptions::default();
        let groups = group_processes(&mut processes, &mut desktop, &mut units, "shell");
        let editor = groups
            .iter()
            .find(|group| group.key == "app:editor")
            .unwrap();
        assert_eq!(editor.instances, 2);
        assert_eq!(editor.members.len(), 3);
        assert_eq!(editor.category, "apps");
        assert_eq!(editor.windows.len(), 1);
        assert_eq!(
            groups
                .iter()
                .find(|group| group.key == "unit:dbus.service")
                .unwrap()
                .category,
            "system"
        );
    }

    #[test]
    fn fallback_identity_is_stable() {
        let process = process(400, None, None, "/", "worker");
        assert_eq!(
            root_meta(
                &process,
                &[],
                &DesktopIndex::default(),
                &UnitDescriptions::default()
            )
            .key,
            "exe:/usr/bin/worker"
        );
    }

    #[test]
    fn sub_scope_shells_do_not_count_as_kitty_instances() {
        let mut processes = vec![
            process(
                100,
                None,
                Some("app-kitty-100.scope"),
                "/user.slice/app.slice/app-kitty-100.scope",
                "kitty",
            ),
            process(
                101,
                None,
                Some("kitty-100-101.scope"),
                "/user.slice/app.slice/app-kitty-100.scope/kitty-100-101.scope",
                "bash",
            ),
            process(
                102,
                None,
                Some("kitty-100-102.scope"),
                "/user.slice/app.slice/app-kitty-100.scope/kitty-100-102.scope",
                "fish",
            ),
            process(
                103,
                None,
                Some("kitty-100-103.scope"),
                "/user.slice/app.slice/app-kitty-100.scope/kitty-100-103.scope",
                "zsh",
            ),
        ];
        let mut desktop = DesktopIndex::from_entries(vec![crate::collect::desktop::DesktopEntry {
            id: "kitty".into(),
            name: "kitty".into(),
            icon: None,
            startup_class: None,
            exec_base: Some("kitty".into()),
        }]);
        let mut units = UnitDescriptions::default();
        let groups = group_processes(&mut processes, &mut desktop, &mut units, "none");
        let kitty = groups
            .iter()
            .find(|group| group.key == "app:kitty")
            .unwrap();

        assert_eq!(kitty.members.len(), 4);
        assert_eq!(kitty.instances, 1);
    }

    #[test]
    fn xdg_autostart_service_merges_with_manual_app_scope() {
        let mut processes = vec![
            process(
                200,
                None,
                Some("app-vesktop@autostart.service"),
                "/user.slice/app.slice/app-vesktop@autostart.service",
                "vesktop",
            ),
            process(
                201,
                Some(200),
                Some("app-vesktop@autostart.service"),
                "/user.slice/app.slice/app-vesktop@autostart.service",
                "helper",
            ),
            process(
                300,
                None,
                Some("app-vesktop-300.scope"),
                "/user.slice/app.slice/app-vesktop-300.scope",
                "vesktop",
            ),
            process(
                301,
                Some(300),
                Some("app-vesktop-300.scope"),
                "/user.slice/app.slice/app-vesktop-300.scope",
                "helper",
            ),
        ];
        processes[3].windows.push(WindowRef {
            id: "vesktop-window".into(),
            title: "Vesktop".into(),
            app_id: "vesktop".into(),
            workspace: "1".into(),
            output: "eDP-1".into(),
            focused: true,
            pid: Some(301),
        });
        let mut desktop = DesktopIndex::from_entries(vec![crate::collect::desktop::DesktopEntry {
            id: "vesktop".into(),
            name: "Vesktop".into(),
            icon: Some("vesktop".into()),
            startup_class: Some("vesktop".into()),
            exec_base: Some("vesktop".into()),
        }]);
        let mut units = UnitDescriptions::default();
        let groups = group_processes(&mut processes, &mut desktop, &mut units, "shell");
        let vesktop_groups: Vec<_> = groups
            .iter()
            .filter(|group| group.key == "app:vesktop")
            .collect();

        assert_eq!(vesktop_groups.len(), 1);
        assert_eq!(vesktop_groups[0].members.len(), 4);
        assert_eq!(vesktop_groups[0].instances, 2);
        assert_eq!(vesktop_groups[0].windows.len(), 1);
        assert_eq!(vesktop_groups[0].category, "apps");
        assert_eq!(
            app_unit_desktop_id(r"app-org\x2dexample@autostart.service").as_deref(),
            Some("org-example")
        );
    }

    #[test]
    fn group_state_comes_from_root_not_zombie_member() {
        let mut processes = vec![
            process(200, None, None, "/", "worker"),
            process(201, Some(200), None, "/", "worker"),
        ];
        processes[1].state = "zombie".into();
        processes[1].root_id = 200;

        let mut desktop = DesktopIndex::default();
        let mut units = UnitDescriptions::default();
        let groups = group_processes(&mut processes, &mut desktop, &mut units, "none");
        let group = groups
            .iter()
            .find(|group| group.key == "exe:/usr/bin/worker")
            .unwrap();

        assert_eq!(group.state, "sleeping");
        assert_eq!(
            group
                .members
                .iter()
                .find(|member| member.pid == 201)
                .unwrap()
                .state,
            "zombie"
        );
    }

    #[test]
    fn user_manager_processes_share_one_system_group() {
        let manager_cgroup = "/user.slice/user-1000.slice/user@1000.service/init.scope";
        let mut processes = vec![
            process(300, Some(1), Some("init.scope"), manager_cgroup, "systemd"),
            process(
                301,
                Some(300),
                Some("init.scope"),
                manager_cgroup,
                "(sd-pam)",
            ),
        ];
        processes[0].command = "/usr/lib/systemd/systemd --user".into();
        processes[1].exe = None;

        let mut desktop = DesktopIndex::default();
        let mut units = UnitDescriptions::default();
        let groups = group_processes(&mut processes, &mut desktop, &mut units, "none");
        let group = groups
            .iter()
            .find(|group| group.key == "unit:user-manager")
            .unwrap();

        assert_eq!(group.name, "systemd (user session)");
        assert_eq!(group.category, "system");
        assert_eq!(group.members.len(), 2);
        assert_eq!(group.instances, 1);
    }

    #[test]
    fn service_names_use_only_short_label_like_descriptions() {
        assert_eq!(
            service_presentation("NetworkManager.service", Some("Network Manager")),
            ("Network Manager".into(), None)
        );
        assert_eq!(
            service_presentation("niri.service", Some("Scrollable tiling Wayland compositor")),
            (
                "niri".into(),
                Some("Scrollable tiling Wayland compositor · niri.service".into())
            )
        );
        assert_eq!(
            service_presentation("niri.service", Some("A Wayland compositor")),
            (
                "niri".into(),
                Some("A Wayland compositor · niri.service".into())
            )
        );
    }
}
