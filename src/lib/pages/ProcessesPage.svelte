<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import ProcessIcon from "../components/ProcessIcon.svelte";
  import SparkGraph from "../components/SparkGraph.svelte";
  import type {
    Category,
    EndProcessArgs,
    History,
    ProcessGroup,
    ProcessInfo,
    ProcessDetailArgs,
    ProcessRef,
    SetPriorityArgs,
    SignalProcessArgs,
    Snapshot,
  } from "../types";

  type Interval = "live" | "minute" | "three";
  type ColumnKey = "status" | "cpu" | "memory" | "disk" | "gpu" | "energy" | "pid" | "threads" | "user" | "nice" | "started" | "unit" | "connections" | "swap" | "read" | "write" | "command";
  type RowNav = { id: string; group: ProcessGroup; member: ProcessInfo | null };
  type PreparedGroup = {
    group: ProcessGroup;
    members: ProcessInfo[];
    expanded: boolean;
    zombieCount: number;
    displayName: string;
    subtitle: string;
  };
  type ProcessSection = {
    category: Category;
    label: string;
    groups: PreparedGroup[];
    collapsed: boolean;
  };
  interface Props {
    snapshot?: Snapshot | null;
    history: History;
    interval?: Interval;
    selectedGroupKey?: string | null;
    selectedProcess?: ProcessRef | null;
    onIntervalChange?: (next: Interval) => void;
    onSelectionChange?: (groupKey: string | null, process: ProcessRef | null) => void;
    onError?: (message: string) => void;
  }

  let {
    snapshot = null,
    history,
    interval = "live",
    selectedGroupKey = null,
    selectedProcess = null,
    onIntervalChange = () => {},
    onSelectionChange = () => {},
    onError = () => {},
  }: Props = $props();

  const intervals: { id: Interval; label: string }[] = [
    { id: "live", label: "LIVE" },
    { id: "minute", label: "1 MIN" },
    { id: "three", label: "3 MIN" },
  ];

  const columns: { id: ColumnKey; label: string; width: string; default: boolean }[] = [
    { id: "status", label: "Status", width: "56px", default: true },
    { id: "cpu", label: "CPU", width: "64px", default: true },
    { id: "memory", label: "Memory", width: "72px", default: true },
    { id: "disk", label: "Disk", width: "64px", default: true },
    { id: "gpu", label: "GPU", width: "64px", default: true },
    { id: "energy", label: "Energy", width: "64px", default: true },
    { id: "pid", label: "PID", width: "64px", default: true },
    { id: "threads", label: "Threads", width: "72px", default: false },
    { id: "user", label: "User", width: "104px", default: false },
    { id: "nice", label: "Nice", width: "64px", default: false },
    { id: "started", label: "Started", width: "112px", default: false },
    { id: "unit", label: "Unit", width: "160px", default: false },
    { id: "connections", label: "Connections", width: "96px", default: false },
    { id: "swap", label: "Swap", width: "88px", default: false },
    { id: "read", label: "Read/s", width: "88px", default: false },
    { id: "write", label: "Write/s", width: "88px", default: false },
    { id: "command", label: "Command", width: "260px", default: false },
  ];
  const heatColumns = new Set<ColumnKey>(["cpu", "memory", "disk", "gpu", "energy"]);
  const energyHeat: Record<ProcessInfo["energy"], number> = {
    none: 0,
    "very-low": 0.1,
    low: 0.25,
    moderate: 0.5,
    high: 0.75,
    "very-high": 1,
  };
  const mebibyte = 1024 * 1024;
  const diskHeatCeilingMib = 50;
  const memoryExplanation = "Memory is private memory (resident minus shared file pages), so a library counted once per process does not inflate a group; PSS is the exact proportional figure shown for the inspected process.";
  const groupDetailReason = "exact memory and counters are read per process; select a member";

  const categoryOrder: Category[] = ["apps", "background", "system"];
  const categoryLabels: Record<Category, string> = { apps: "APPS", background: "BACKGROUND", system: "SYSTEM" };
  let enabledColumns = $state<ColumnKey[]>(columns.filter((column) => column.default).map((column) => column.id));
  let query = $state("");
  let groupByType = $state(true);
  let expanded = $state(new Set<string>());
  let collapsedSections = $state(new Set<Category>());
  let sortKey = $state<"name" | ColumnKey>("cpu");
  let sortDescending = $state(true);
  let busy = $state(false);
  let pendingEnd = $state<"end" | "force" | null>(null);
  let actionMessage = $state("");
  let priorityNice = $state(0);
  let priorityIdentity = $state("");
  let nameClickToggled = $state(false);
  let processDetail = $state<ProcessInfo | null>(null);

  const formatBytes = (bytes: number) => {
    const units = ["B", "KB", "MB", "GB", "TB"];
    let value = Math.max(0, bytes);
    let index = 0;
    while (value >= 1024 && index < units.length - 1) {
      value /= 1024;
      index++;
    }
    return `${value >= 10 || index < 2 ? value.toFixed(0) : value.toFixed(1)} ${units[index]}`;
  };

  const formatRate = (value: number) => `${formatBytes(value)}/s`;
  const pct = (value: number) => `${Math.max(0, value).toFixed(value >= 10 ? 0 : 1)}%`;
  const clamp = (value: number) => Math.min(1, Math.max(0, value));
  const rowId = (group: ProcessGroup, member: ProcessInfo | null) => member ? `p:${group.key}:${member.pid}:${member.startTime}` : `g:${group.key}`;
  const processRef = (member: ProcessInfo): ProcessRef => ({ pid: member.pid, startTime: member.startTime });
  const sameProcess = (left: ProcessRef | null, right: ProcessInfo) => left?.pid === right.pid && left.startTime === right.startTime;
  const leader = (group: ProcessGroup) => group.members.find((member) => member.pid === group.leaderPid) ?? group.members[0] ?? null;

  function memberMatches(member: ProcessInfo, needle: string) {
    return member.name.toLowerCase().includes(needle) || member.command.toLowerCase().includes(needle) ||
      String(member.pid).includes(needle) || member.windows.some((window) => window.title.toLowerCase().includes(needle));
  }

  function groupMatches(group: ProcessGroup, needle = query.trim().toLowerCase()) {
    if (!needle) return true;
    return group.name.toLowerCase().includes(needle) || group.subtitle.toLowerCase().includes(needle) ||
      group.unit?.toLowerCase().includes(needle) || group.windows.some((window) => window.title.toLowerCase().includes(needle)) ||
      group.members.some((member) => memberMatches(member, needle));
  }

  function ownGroupMatch(group: ProcessGroup, needle: string) {
    return group.name.toLowerCase().includes(needle) || group.subtitle.toLowerCase().includes(needle) ||
      group.unit?.toLowerCase().includes(needle) || group.windows.some((window) => window.title.toLowerCase().includes(needle));
  }

  function groupValue(group: ProcessGroup, key: "name" | ColumnKey): string | number {
    const root = leader(group);
    switch (key) {
      case "name": return group.name;
      case "status": return root?.state ?? "";
      case "cpu": return group.cpu;
      case "memory": return group.memory;
      case "disk": return group.diskRead === null && group.diskWrite === null ? -1 : (group.diskRead ?? 0) + (group.diskWrite ?? 0);
      case "gpu": return group.gpu ?? -1;
      case "energy": return group.energyScore;
      case "pid": return group.leaderPid;
      case "threads": return group.threads;
      case "user": return root?.user ?? "";
      case "nice": return root?.nice ?? 0;
      case "started": return root?.startTime ?? 0;
      case "unit": return group.unit ?? "";
      case "connections": return group.connections ?? -1;
      case "swap": return sumMeasured(group.members.map((member) => member.swap)) ?? -1;
      case "read": return group.diskRead ?? -1;
      case "write": return group.diskWrite ?? -1;
      case "command": return root?.command ?? "";
    }
  }

  function memberValue(member: ProcessInfo, key: "name" | ColumnKey): string | number {
    switch (key) {
      case "name": return member.name;
      case "status": return member.state;
      case "cpu": return member.cpu;
      case "memory": return member.memory;
      case "disk": return member.diskRead === null && member.diskWrite === null ? -1 : (member.diskRead ?? 0) + (member.diskWrite ?? 0);
      case "gpu": return member.gpu ?? -1;
      case "energy": return member.energyScore;
      case "pid": return member.pid;
      case "threads": return member.threads;
      case "user": return member.user;
      case "nice": return member.nice;
      case "started": return member.startTime;
      case "unit": return member.unit ?? "";
      case "connections": return member.connections ?? -1;
      case "swap": return member.swap ?? -1;
      case "read": return member.diskRead ?? -1;
      case "write": return member.diskWrite ?? -1;
      case "command": return member.command;
    }
  }

  function compareValues(left: string | number, right: string | number) {
    const result = typeof left === "number" && typeof right === "number"
      ? left - right
      : String(left).localeCompare(String(right), undefined, { numeric: true, sensitivity: "base" });
    return sortDescending ? -result : result;
  }

  function sortedGroups(groups: ProcessGroup[]) {
    return [...groups].sort((a, b) => compareValues(groupValue(a, sortKey), groupValue(b, sortKey)));
  }

  function sortedMembers(group: ProcessGroup) {
    const members = query.trim() && !ownGroupMatch(group, query.trim().toLowerCase())
      ? group.members.filter((member) => memberMatches(member, query.trim().toLowerCase()))
      : group.members;
    const included = new Set(members.map((member) => member.pid));
    const children = new Map<number | null, ProcessInfo[]>();
    for (const member of members) {
      const parent = member.parentPid !== null && included.has(member.parentPid) ? member.parentPid : null;
      children.set(parent, [...(children.get(parent) ?? []), member]);
    }
    for (const siblings of children.values()) {
      siblings.sort((a, b) => compareValues(memberValue(a, sortKey), memberValue(b, sortKey)));
    }
    const ordered: ProcessInfo[] = [];
    const visit = (parent: number | null) => {
      for (const member of children.get(parent) ?? []) {
        ordered.push(member);
        visit(member.pid);
      }
    };
    visit(null);
    return ordered;
  }

  function isExpanded(group: ProcessGroup) {
    const needle = query.trim().toLowerCase();
    return expanded.has(group.key) || Boolean(needle && group.members.some((member) => memberMatches(member, needle)));
  }

  function displayGroupName(group: ProcessGroup) {
    return group.name.replace(/\.(service|scope)$/i, "");
  }

  function displayGroupSubtitle(group: ProcessGroup) {
    let subtitle = group.subtitle;
    if (/\.(service|scope)$/i.test(group.name) && !subtitle.includes(group.name)) {
      subtitle = subtitle ? `${subtitle} · ${group.name}` : group.name;
    }
    if (group.instances > 1) {
      subtitle = subtitle ? `${subtitle} · ${group.instances} instances` : `${group.instances} instances`;
    }
    return subtitle;
  }

  function toggleExpanded(key: string) {
    const next = new Set(expanded);
    if (next.has(key)) next.delete(key); else next.add(key);
    expanded = next;
  }

  function toggleSection(category: Category) {
    const next = new Set(collapsedSections);
    if (next.has(category)) next.delete(category); else next.add(category);
    collapsedSections = next;
  }

  function handleSectionKey(event: KeyboardEvent, category: Category) {
    const collapsed = collapsedSections.has(category);
    if ((event.key === "ArrowRight" && collapsed) || (event.key === "ArrowLeft" && !collapsed)) {
      event.preventDefault();
      toggleSection(category);
    }
  }

  function toggleColumn(key: ColumnKey) {
    enabledColumns = enabledColumns.includes(key)
      ? enabledColumns.filter((column) => column !== key)
      : [...enabledColumns, key];
  }

  function sortBy(key: "name" | ColumnKey) {
    if (sortKey === key) sortDescending = !sortDescending;
    else {
      sortKey = key;
      sortDescending = key !== "name" && key !== "status" && key !== "user" && key !== "unit" && key !== "command";
    }
  }

  function select(group: ProcessGroup, member: ProcessInfo | null) {
    pendingEnd = null;
    actionMessage = "";
    onSelectionChange(group.key, member ? processRef(member) : null);
  }

  function handleGroupNameClick(event: MouseEvent, group: ProcessGroup) {
    event.stopPropagation();
    if (event.detail > 1) return;
    nameClickToggled = selectedGroupKey === group.key && selectedProcess === null;
    if (nameClickToggled) toggleExpanded(group.key);
    select(group, null);
  }

  function handleGroupNameDoubleClick(event: MouseEvent, group: ProcessGroup) {
    event.stopPropagation();
    if (!nameClickToggled) toggleExpanded(group.key);
    nameClickToggled = false;
    select(group, null);
  }

  function buildVisibleRows(groups: PreparedGroup[], sections: ProcessSection[]) {
    const rows: RowNav[] = [];
    const add = (items: PreparedGroup[]) => {
      for (const item of items) {
        rows.push({ id: rowId(item.group, null), group: item.group, member: null });
        if (item.expanded) {
          for (const member of item.members) rows.push({ id: rowId(item.group, member), group: item.group, member });
        }
      }
    };
    if (!groupByType) add(groups);
    else {
      for (const section of sections) {
        if (!section.collapsed) add(section.groups);
      }
    }
    return rows;
  }

  function selectedRowIndex() {
    return visibleRows.findIndex((row) => row.group.key === selectedGroupKey &&
      (row.member ? sameProcess(selectedProcess, row.member) : selectedProcess === null));
  }

  function handleKeyboard(event: KeyboardEvent) {
    if (event.target instanceof HTMLInputElement || event.target instanceof HTMLButtonElement) return;
    if (!["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight", "Enter", "Delete"].includes(event.key)) return;
    event.preventDefault();
    const index = selectedRowIndex();
    if (event.key === "ArrowUp" || event.key === "ArrowDown") {
      const delta = event.key === "ArrowUp" ? -1 : 1;
      const next = visibleRows[Math.min(visibleRows.length - 1, Math.max(0, index < 0 ? 0 : index + delta))];
      if (next) select(next.group, next.member);
      return;
    }
    const row = visibleRows[index];
    if (!row) return;
    if (event.key === "ArrowRight" && !row.member && !isExpanded(row.group)) toggleExpanded(row.group.key);
    if (event.key === "ArrowLeft") {
      if (row.member) select(row.group, null);
      else if (isExpanded(row.group)) toggleExpanded(row.group.key);
    }
    if (event.key === "Enter") select(row.group, row.member);
    if (event.key === "Delete") pendingEnd = "end";
  }

  function sumMeasured(values: (number | null)[]) {
    const measured = values.filter((value): value is number => value !== null);
    return measured.length ? measured.reduce((sum, value) => sum + value, 0) : null;
  }

  function groupKind(group: ProcessGroup): "app" | "unit" | "kernel" {
    if (group.key === "kernel") return "kernel";
    return group.unit ? "unit" : "app";
  }

  function displayState(state: string) {
    if (state === "stopped") return "Suspended";
    if (state === "zombie") return "Zombie";
    return "";
  }

  function limitReason(column: ColumnKey) {
    if (!snapshot) return "Not measured yet.";
    const keys: Partial<Record<ColumnKey, string[]>> = {
      disk: ["io"], read: ["io"], write: ["io"], gpu: ["gpu.process", "process"],
      connections: ["connections", "network.processBytes"], swap: ["pss"],
    };
    for (const key of keys[column] ?? []) {
      if (snapshot.limits[key]) return snapshot.limits[key];
      for (const gpu of snapshot.gpus) if (gpu.limits[key]) return gpu.limits[key];
    }
    return "This reading is not available for the process.";
  }

  function columnRaw(group: ProcessGroup, member: ProcessInfo | null, column: ColumnKey): number | string | null {
    const root = leader(group);
    if (member) {
      switch (column) {
        case "status": return displayState(member.state);
        case "cpu": return member.cpu;
        case "memory": return member.memory;
        case "disk": return member.diskRead === null && member.diskWrite === null ? null : (member.diskRead ?? 0) + (member.diskWrite ?? 0);
        case "gpu": return member.gpu;
        case "energy": return member.energy;
        case "pid": return member.pid;
        case "threads": return member.threads;
        case "user": return member.user;
        case "nice": return member.nice;
        case "started": return member.startTime;
        case "unit": return member.unit;
        case "connections": return member.connections;
        case "swap": return member.swap;
        case "read": return member.diskRead;
        case "write": return member.diskWrite;
        case "command": return member.command;
      }
    }
    switch (column) {
      case "status": return displayState(root?.state ?? "");
      case "cpu": return group.cpu;
      case "memory": return group.memory;
      case "disk": return group.diskRead === null && group.diskWrite === null ? null : (group.diskRead ?? 0) + (group.diskWrite ?? 0);
      case "gpu": return group.gpu;
      case "energy": return group.energy;
      case "pid": return group.leaderPid;
      case "threads": return group.threads;
      case "user": return root?.user ?? null;
      case "nice": return root?.nice ?? null;
      case "started": return root?.startTime ?? null;
      case "unit": return group.unit;
      case "connections": return group.connections;
      case "swap": return sumMeasured(group.members.map((item) => item.swap));
      case "read": return group.diskRead;
      case "write": return group.diskWrite;
      case "command": return root?.command ?? null;
    }
  }

  function columnText(column: ColumnKey, value: number | string | null) {
    if (value === null || value === "") return "—";
    if (column === "cpu" || column === "gpu") return pct(Number(value));
    if (["memory", "disk", "swap", "read", "write"].includes(column)) return column === "memory" || column === "swap" ? formatBytes(Number(value)) : formatRate(Number(value));
    if (column === "started") return new Date(Number(value) * 1000).toLocaleString([], { month: "short", day: "numeric", hour: "2-digit", minute: "2-digit" });
    if (column === "energy") return String(value).replace("-", " ");
    return String(value);
  }

  function cellHeat(group: ProcessGroup, member: ProcessInfo | null, column: ColumnKey) {
    if (column === "energy") return energyHeat[member?.energy ?? group.energy];
    const raw = columnRaw(group, member, column);
    if (typeof raw !== "number") return 0;
    if (column === "cpu" || column === "gpu") return clamp(raw / 100);
    if (column === "memory") return clamp(raw / Math.max(1, snapshot?.memory.total ?? 1));
    if (column === "disk") {
      return clamp(Math.log1p(Math.max(0, raw) / mebibyte) / Math.log1p(diskHeatCeilingMib));
    }
    return 0;
  }

  function headerTotal(column: ColumnKey) {
    if (!snapshot) return "";
    if (column === "cpu") return pct(snapshot.cpu.usage);
    if (column === "memory") return pct(snapshot.memory.total ? snapshot.memory.used / snapshot.memory.total * 100 : 0);
    if (column === "disk") return pct(Math.max(0, ...snapshot.disks.map((disk) => disk.activePercent)));
    if (column === "gpu") {
      const values = snapshot.gpus.map((gpu) => gpu.usage).filter((value): value is number => value !== null);
      return values.length ? pct(Math.max(...values)) : "—";
    }
    return "";
  }

  function requestEnd(kind: "end" | "force") {
    if (selectedGroup) pendingEnd = kind;
  }

  async function confirmEnd() {
    if (!selectedGroup || !pendingEnd || busy) return;
    busy = true;
    const force = pendingEnd === "force";
    try {
      if (selectedMember) {
        const args: EndProcessArgs = { ...processRef(selectedMember), force };
        await invoke("end_process", args);
      } else {
        await invoke("end_group", { key: selectedGroup.key, force });
      }
      actionMessage = force ? "Force stop sent." : "End request sent.";
      pendingEnd = null;
    } catch (cause) {
      onError(String(cause));
    } finally {
      busy = false;
    }
  }

  async function signal(action: "suspend" | "resume") {
    if (!selectedGroup || busy) return;
    busy = true;
    try {
      const targets = selectedMember ? [selectedMember] : selectedGroup.members;
      await Promise.all(targets.map((member) => {
        const args: SignalProcessArgs = { ...processRef(member), action };
        return invoke("signal_process", args);
      }));
      actionMessage = action === "suspend" ? "Suspend sent." : "Resume sent.";
    } catch (cause) {
      onError(String(cause));
    } finally {
      busy = false;
    }
  }

  async function setNice(next: number) {
    if (!selectedGroup || busy) return;
    const value = Math.min(19, Math.max(-20, next));
    busy = true;
    try {
      const targets = selectedMember ? [selectedMember] : selectedGroup.members;
      await Promise.all(targets.map((member) => {
        const args: SetPriorityArgs = { ...processRef(member), nice: value };
        return invoke("set_priority", args);
      }));
      priorityNice = value;
      actionMessage = "Priority updated.";
    } catch (cause) {
      onError(String(cause));
    } finally {
      busy = false;
    }
  }

  async function openLocation() {
    if (!inspectedProcess || busy) return;
    try {
      await invoke("open_location", processRef(inspectedProcess));
    } catch (cause) {
      onError(String(cause));
    }
  }

  async function copyCommand() {
    if (!inspectedProcess) return;
    try {
      await navigator.clipboard.writeText(inspectedProcess.command);
      actionMessage = "Command copied.";
    } catch (cause) {
      onError(String(cause));
    }
  }

  function detailReason(key: "pss" | "fds" | "ctxSwitches" | "oomScore") {
    if (!selectedMember) return groupDetailReason;
    return snapshot?.limits[key] ?? "Exact process details have not arrived yet.";
  }

  function formatUptime(startTime: number) {
    let seconds = Math.max(0, Math.floor(Date.now() / 1000 - startTime));
    const days = Math.floor(seconds / 86400);
    seconds %= 86400;
    const hours = Math.floor(seconds / 3600);
    const minutes = Math.floor((seconds % 3600) / 60);
    return days ? `${days}d ${hours}h` : hours ? `${hours}h ${minutes}m` : `${minutes}m`;
  }

  let visibleColumns = $derived(columns.filter((column) => enabledColumns.includes(column.id)));
  let gridTemplate = $derived(`minmax(240px, 1fr) ${visibleColumns.map((column) => column.width).join(" ")}`);
  let tableMinWidth = $derived(240 + visibleColumns.reduce((total, column) => total + Number.parseInt(column.width, 10), 0));
  let filteredGroups = $derived((snapshot?.groups ?? []).filter((group) => groupMatches(group)));
  let preparedGroups = $derived.by((): PreparedGroup[] => sortedGroups(filteredGroups).map((group) => ({
    group,
    members: sortedMembers(group),
    expanded: isExpanded(group),
    zombieCount: group.members.filter((member) => member.state === "zombie").length,
    displayName: displayGroupName(group),
    subtitle: displayGroupSubtitle(group),
  })));
  let sections = $derived.by((): ProcessSection[] => categoryOrder.map((category) => ({
    category,
    label: categoryLabels[category],
    groups: preparedGroups.filter((item) => item.group.category === category),
    collapsed: collapsedSections.has(category) && !query.trim(),
  })));
  let visibleRows = $derived(buildVisibleRows(preparedGroups, sections));
  let selectedGroup = $derived(selectedGroupKey ? (snapshot?.groups.find((group) => group.key === selectedGroupKey) ?? null) : null);
  let selectedMember = $derived(selectedGroup && selectedProcess
    ? (selectedGroup.members.find((member) => sameProcess(selectedProcess, member)) ?? null)
    : null);
  let selectedPid = $derived(selectedMember?.pid ?? null);
  let selectedStartTime = $derived(selectedMember?.startTime ?? null);
  let detailIsCurrent = $derived(processDetail !== null &&
    processDetail.pid === selectedPid && processDetail.startTime === selectedStartTime);
  let inspectedProcess = $derived(selectedMember
    ? (detailIsCurrent ? { ...selectedMember, ...processDetail! } : selectedMember)
    : (selectedGroup ? leader(selectedGroup) : null));
  let selectedIdentity = $derived(inspectedProcess ? `${inspectedProcess.pid}:${inspectedProcess.startTime}` : "");
  let inspectorHistory = $derived(selectedMember ? history.selectedProcess : history.selectedGroup);
  let listMemory = $derived(selectedGroup ? (selectedMember ? selectedMember.memory : selectedGroup.memory) : 0);
  let inspectorMemory = $derived(detailIsCurrent ? processDetail!.memory : listMemory);
  let inspectorPss = $derived(detailIsCurrent ? processDetail!.memory : null);
  let inspectorAnon = $derived(detailIsCurrent ? processDetail!.rssAnon : null);
  let inspectorFile = $derived(detailIsCurrent ? processDetail!.rssFile : null);
  let inspectorShared = $derived(detailIsCurrent ? processDetail!.rssShmem : null);
  let inspectorSwap = $derived(detailIsCurrent ? processDetail!.swap : null);
  let inspectorDiskRead = $derived(selectedGroup ? (selectedMember ? selectedMember.diskRead : selectedGroup.diskRead) : null);
  let inspectorDiskWrite = $derived(selectedGroup ? (selectedMember ? selectedMember.diskWrite : selectedGroup.diskWrite) : null);
  let inspectorGpu = $derived(selectedGroup ? (selectedMember ? selectedMember.gpu : selectedGroup.gpu) : null);
  let inspectorFds = $derived(detailIsCurrent ? processDetail!.fds : null);
  let inspectorCtxSwitches = $derived(detailIsCurrent ? processDetail!.ctxSwitches : null);
  let inspectorOomScore = $derived(detailIsCurrent ? processDetail!.oomScore : null);
  let compositionTotal = $derived((inspectorAnon ?? 0) + (inspectorFile ?? 0) + (inspectorShared ?? 0) + (inspectorSwap ?? 0));
  let inspectorWindows = $derived(selectedMember ? selectedMember.windows : selectedGroup?.windows ?? []);
  let isSuspended = $derived(selectedMember
    ? selectedMember.state === "stopped"
    : selectedGroup ? leader(selectedGroup)?.state === "stopped" : false);

  $effect(() => {
    const pid = selectedPid;
    const startTime = selectedStartTime;
    processDetail = null;
    if (pid === null || startTime === null) return;

    const args: ProcessDetailArgs = { pid, startTime };
    let cancelled = false;
    let loading = false;
    const load = async () => {
      if (loading) return;
      loading = true;
      try {
        const detail = await invoke<ProcessInfo>("process_detail", args);
        if (!cancelled && detail.pid === pid && detail.startTime === startTime) processDetail = detail;
      } catch {
        // A process can disappear between the list sample and this request.
      } finally {
        loading = false;
      }
    };

    void load();
    const timer = window.setInterval(() => void load(), 2_000);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
  });

  $effect(() => {
    if (selectedIdentity !== priorityIdentity) {
      priorityIdentity = selectedIdentity;
      priorityNice = inspectedProcess?.nice ?? 0;
    }
  });
</script>

<svelte:window onkeydown={handleKeyboard} />

<section class="page processes-page">
  <div class="page-head processes-head">
    <div class="title-block">
      <div class="section-label"><i></i><span class="section-mark">力</span> SYSTEM / PROCESSES <span>処理</span><b>01</b></div>
      <h1>Processes</h1>
      <p>Apps and services grouped by identity, with every reading tied to a live process.</p>
    </div>
    <div class="head-actions process-tools">
      <label class="search"><span>⌕</span><input bind:value={query} placeholder="Search name, command, PID or window" /></label>
      <label class="group-toggle"><input type="checkbox" bind:checked={groupByType} /><span>Group by type</span></label>
      <details class="column-chooser">
        <summary aria-label="Choose columns">•••</summary>
        <div class="column-menu">
          <strong>Columns</strong>
          {#each columns as column (column.id)}
            <label><input type="checkbox" checked={enabledColumns.includes(column.id)} onchange={() => toggleColumn(column.id)} /><span>{column.label}</span></label>
          {/each}
        </div>
      </details>
    </div>
  </div>

  <div class="process-workspace" class:inspecting={selectedGroup !== null}>
    <div class="process-table-shell">
      <div class="table-kicker">
        <span>{filteredGroups.length} GROUPS / {snapshot?.processCount ?? "—"} PROCESSES</span>
        <div class="interval-switch" role="group" aria-label="Refresh interval">
          {#each intervals as option (option.id)}
            <button class:active={interval === option.id} onclick={() => onIntervalChange(option.id)}>{option.label}</button>
          {/each}
        </div>
        <span>{snapshot?.windowsSource === "shell" ? "WINDOWS LINKED" : "WINDOW DATA UNAVAILABLE"}</span>
      </div>

      <div class="process-scroll">
        <div class="process-table" style={`--process-grid:${gridTemplate};--process-min:${tableMinWidth}px`}>
          <div class="process-header process-data-grid">
            <button class="name-column" onclick={() => sortBy("name")}><span></span><b>Name</b><i>{sortKey === "name" ? (sortDescending ? "↓" : "↑") : ""}</i></button>
            {#each visibleColumns as column (column.id)}
              <button onclick={() => sortBy(column.id)} title={column.id === "memory" ? memoryExplanation : undefined}>
                <span>{headerTotal(column.id)}</span><b>{column.label}</b><i>{sortKey === column.id ? (sortDescending ? "↓" : "↑") : ""}</i>
              </button>
            {/each}
          </div>

          {#snippet groupRows(items: PreparedGroup[])}
            {#each items as item (item.group.key)}
              {@const group = item.group}
              <div class="process-row process-data-grid group-row" role="button" tabindex="-1" class:selected={selectedGroupKey === group.key && selectedProcess === null} onclick={() => select(group, null)} onkeydown={(event) => { if (event.key === "Enter" || event.key === " ") select(group, null); }} title={item.subtitle}>
                <div class="process-identity group-identity">
                  <button class="row-caret" class:open={item.expanded} onclick={(event) => { event.stopPropagation(); toggleExpanded(group.key); }} aria-label={item.expanded ? "Collapse group" : "Expand group"}>›</button>
                  <ProcessIcon iconKey={group.iconKey} name={item.displayName} kind={groupKind(group)} />
                  <button type="button" class="process-name group-name-button" onclick={(event) => handleGroupNameClick(event, group)} ondblclick={(event) => handleGroupNameDoubleClick(event, group)}>
                    <b>{item.displayName}{#if group.members.length > 1}<em class="count-chip">{group.members.length}</em>{/if}</b>
                    <small><span class="subtitle-text">{item.subtitle}</span>{#if item.zombieCount}<em class="zombie-chip">{item.zombieCount} zombie{item.zombieCount === 1 ? "" : "s"}</em>{/if}</small>
                  </button>
                </div>
                {#each visibleColumns as column (column.id)}
                  {@const value = columnRaw(group, null, column.id)}
                  <span class:metric-cell={heatColumns.has(column.id)} class="process-cell" class:status-cell={column.id === "status"} style={`--heat:${cellHeat(group, null, column.id)}`} title={value === null ? limitReason(column.id) : ""}>
                    {#if column.id === "status"}{#if value}<span class="status-badge">{value}</span>{/if}{:else}{columnText(column.id, value)}{/if}
                  </span>
                {/each}
              </div>
              {#if item.expanded}
                {#each item.members as member (member.pid + ":" + member.startTime)}
                  <div class="process-row process-data-grid member-row" role="button" tabindex="-1" class:selected={selectedGroupKey === group.key && sameProcess(selectedProcess, member)} onclick={() => select(group, member)} onkeydown={(event) => { if (event.key === "Enter" || event.key === " ") select(group, member); }} title={member.command || member.name}>
                    <div class="process-identity member-identity">
                      <span class="tree-elbow">└</span><span class="member-dot"></span>
                      <span class="process-name"><b>{member.name}</b><small>{#if member.windows[0]}<em class="window-chip">{member.windows[0].title}</em>{:else}{member.command || `PID ${member.pid}`}{/if}</small></span>
                    </div>
                    {#each visibleColumns as column (column.id)}
                      {@const value = columnRaw(group, member, column.id)}
                      <span class:metric-cell={heatColumns.has(column.id)} class="process-cell" class:status-cell={column.id === "status"} style={`--heat:${cellHeat(group, member, column.id)}`} title={value === null ? limitReason(column.id) : ""}>
                        {#if column.id === "status"}{#if value}<span class="status-badge" class:danger={value === "Zombie"}>{value}</span>{/if}{:else}{columnText(column.id, value)}{/if}
                      </span>
                    {/each}
                  </div>
                {/each}
              {/if}
            {/each}
          {/snippet}

          {#if groupByType}
            {#each sections as section (section.category)}
              <div class="process-section">
                <button class="section-header" onclick={() => toggleSection(section.category)} onkeydown={(event) => handleSectionKey(event, section.category)}>
                  <span class="section-caret" class:closed={section.collapsed}>⌄</span>
                  <strong>{section.label}</strong><span>{section.groups.length}</span><i></i>
                </button>
                {#if !section.collapsed}
                  {@render groupRows(section.groups)}
                {/if}
              </div>
            {/each}
          {:else}
            {@render groupRows(preparedGroups)}
          {/if}

          {#if filteredGroups.length === 0}
            <div class="process-empty"><strong>No matching processes</strong><span>Try another name, command, PID or window title.</span></div>
          {/if}
        </div>
      </div>
    </div>

    <aside class="process-inspector" class:open={selectedGroup !== null}>
      {#if selectedGroup && inspectedProcess}
        <div class="inspector-scroll">
          <div class="inspector-head inspector-identity">
            <ProcessIcon iconKey={selectedGroup.iconKey} name={selectedGroup.name} kind={groupKind(selectedGroup)} size="large" />
            <div><span>{selectedMember ? "PROCESS" : selectedGroup.category.toUpperCase()}</span><strong>{selectedMember?.name ?? selectedGroup.name}</strong><small>PID {inspectedProcess.pid} · {selectedMember ? "member" : `${selectedGroup.members.length} process${selectedGroup.members.length === 1 ? "" : "es"}`}</small></div>
            <button class="inspector-close" onclick={() => onSelectionChange(null, null)} aria-label="Close inspector">×</button>
          </div>

          <div class="inspector-graph">
            <div class="inspector-graph-head"><span>CPU / 60 SAMPLES</span><strong>{pct(selectedMember?.cpu ?? selectedGroup.cpu)}</strong></div>
            <SparkGraph values={inspectorHistory} label="Selected process CPU history" />
          </div>

          <div class="inspector-share">
            <div class="inspector-graph-head">
              <span>MEMORY COMPOSITION</span>
              <strong>{formatBytes(inspectorMemory)}{#if snapshot?.memory.total}<small> of {formatBytes(snapshot.memory.total)}</small>{/if}</strong>
            </div>
            <div class="memory-composition" title={detailIsCurrent ? "Anonymous / file / shared / swap" : "Private memory / total memory"}>
              {#if detailIsCurrent}
                <i class="anon" style={`--share:${(inspectorAnon ?? 0) / Math.max(1, compositionTotal) * 100}%`}></i>
                <i class="file" style={`--share:${(inspectorFile ?? 0) / Math.max(1, compositionTotal) * 100}%`}></i>
                <i class="shared" style={`--share:${(inspectorShared ?? 0) / Math.max(1, compositionTotal) * 100}%`}></i>
                <i class="swap" style={`--share:${(inspectorSwap ?? 0) / Math.max(1, compositionTotal) * 100}%`}></i>
              {:else}
                <i class="private" style={`--share:${clamp(inspectorMemory / Math.max(1, snapshot?.memory.total ?? 0)) * 100}%`}></i>
              {/if}
            </div>
            <div class="memory-detail-grid">
              <div><span>PSS</span><strong title={inspectorPss === null ? detailReason("pss") : ""}>{inspectorPss === null ? "—" : formatBytes(inspectorPss)}</strong></div>
              <div><span>ANON</span><strong title={inspectorAnon === null ? detailReason("pss") : ""}>{inspectorAnon === null ? "—" : formatBytes(inspectorAnon)}</strong></div>
              <div><span>FILE</span><strong title={inspectorFile === null ? detailReason("pss") : ""}>{inspectorFile === null ? "—" : formatBytes(inspectorFile)}</strong></div>
              <div><span>SHARED</span><strong title={inspectorShared === null ? detailReason("pss") : ""}>{inspectorShared === null ? "—" : formatBytes(inspectorShared)}</strong></div>
              <div><span>SWAP</span><strong title={inspectorSwap === null ? detailReason("pss") : ""}>{inspectorSwap === null ? "—" : formatBytes(inspectorSwap)}</strong></div>
            </div>
            <small>{memoryExplanation}</small>
          </div>

          <div class="inspector-metrics">
            <div><span>READ</span><strong title={inspectorDiskRead === null ? limitReason("read") : ""}>{inspectorDiskRead === null ? "—" : formatRate(inspectorDiskRead)}</strong><small>disk throughput</small></div>
            <div><span>WRITE</span><strong title={inspectorDiskWrite === null ? limitReason("write") : ""}>{inspectorDiskWrite === null ? "—" : formatRate(inspectorDiskWrite)}</strong><small>disk throughput</small></div>
            <div><span>GPU</span><strong title={inspectorGpu === null ? limitReason("gpu") : ""}>{inspectorGpu === null ? "—" : pct(inspectorGpu)}</strong><small>engine time</small></div>
            <div><span>ENERGY</span><strong>{(selectedMember ? selectedMember.energy : selectedGroup.energy).replace("-", " ")}</strong><small>estimated impact</small></div>
          </div>

          <div class="inspector-registry identity-registry">
            <div><span>USER</span><strong>{inspectedProcess.user}</strong></div>
            <div><span>STARTED</span><strong>{new Date(inspectedProcess.startTime * 1000).toLocaleString()}</strong></div>
            <div><span>UPTIME</span><strong>{formatUptime(inspectedProcess.startTime)}</strong></div>
            <div><span>UNIT</span><strong title={inspectedProcess.unit ?? "No systemd unit"}>{inspectedProcess.unit ?? "—"}</strong></div>
            <div class="wide"><span>EXECUTABLE</span><code>{inspectedProcess.exe ?? "—"}</code></div>
            <div class="wide"><span>WORKING DIRECTORY</span><code>{inspectedProcess.cwd ?? "—"}</code></div>
            <div class="wide"><span>COMMAND</span><code>{inspectedProcess.command || "—"}</code></div>
          </div>

          <div class="counter-grid">
            <div><span>THREADS</span><strong>{selectedMember ? selectedMember.threads : selectedGroup.threads}</strong></div>
            <div><span>OPEN FILES</span><strong title={inspectorFds === null ? detailReason("fds") : ""}>{inspectorFds ?? "—"}</strong></div>
            <div><span>CONTEXT SWITCHES</span><strong title={inspectorCtxSwitches === null ? detailReason("ctxSwitches") : ""}>{inspectorCtxSwitches ?? "—"}</strong></div>
            <div><span>OOM SCORE</span><strong title={inspectorOomScore === null ? detailReason("oomScore") : ""}>{inspectorOomScore ?? "—"}</strong></div>
            <div><span>NICE / PRIORITY</span><strong>{inspectedProcess.nice} / {inspectedProcess.priority}</strong></div>
            <div><span>LAST CPU</span><strong>{inspectedProcess.lastCpu ?? "—"}</strong></div>
          </div>

          {#if inspectorWindows.length}
            <div class="inspector-list"><h3>Windows <span>{inspectorWindows.length}</span></h3>{#each inspectorWindows as window (window.id)}<div><strong>{window.title || window.appId}</strong><small>{window.workspace} · {window.output}</small></div>{/each}</div>
          {/if}

          {#if !selectedMember && selectedGroup.members.length > 1}
            <div class="inspector-list member-list"><h3>Members <span>{selectedGroup.members.length}</span></h3>{#each selectedGroup.members as member (member.pid + ":" + member.startTime)}<button onclick={() => select(selectedGroup!, member)}><span><strong>{member.name}</strong><small>PID {member.pid}</small></span><em>{pct(member.cpu)}</em></button>{/each}</div>
          {/if}
        </div>

        <div class="inspector-footer">
          {#if pendingEnd}
            <div class="action-confirm"><span>{pendingEnd === "force" ? "Force stop" : "End"} {selectedMember ? `PID ${selectedMember.pid}` : `all ${selectedGroup.members.length} members`}?</span><button disabled={busy} onclick={confirmEnd}>Confirm</button><button disabled={busy} onclick={() => pendingEnd = null}>Cancel</button></div>
          {/if}
          {#if actionMessage}<div class="action-message">{actionMessage}</div>{/if}
          <div class="priority-control"><span>Priority <small>Raising it may require root.</small></span><div><button disabled={busy || priorityNice <= -20} onclick={() => setNice(priorityNice - 1)}>−</button><strong>{priorityNice}</strong><button disabled={busy || priorityNice >= 19} onclick={() => setNice(priorityNice + 1)}>+</button></div></div>
          <div class="inspector-actions">
            <button class="button" disabled={busy} onclick={() => signal(isSuspended ? "resume" : "suspend")}>{isSuspended ? "Resume" : "Suspend"}</button>
            <button class="button" disabled={busy || !inspectedProcess.exe} onclick={openLocation}>Open location</button>
            <button class="button" disabled={!inspectedProcess.command} onclick={copyCommand}>Copy command</button>
            <button class="button" disabled={busy} onclick={() => requestEnd("end")}>End task</button>
            <button class="button danger" disabled={busy} onclick={() => requestEnd("force")}>Force stop</button>
          </div>
        </div>
      {/if}
    </aside>
  </div>
</section>
