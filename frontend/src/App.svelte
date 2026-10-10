<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "./lib/backend";
  import ProcessesPage from "./lib/pages/ProcessesPage.svelte";
  import PerformancePage from "./lib/pages/PerformancePage.svelte";
  import SystemPage from "./lib/pages/SystemPage.svelte";
  import "./lib/styles/performance.css";
  import type { History, ProcessRef, Series, Snapshot } from "./lib/types";

  type Page = "processes" | "performance" | "system";
  type Interval = "live" | "minute" | "three";

  const historyLimit = 60;
  const intervals: Record<Interval, number> = { live: 1000, minute: 60_000, three: 180_000 };

  let page = $state<Page>("processes");
  let snapshot = $state<Snapshot | null>(null);
  let interval = $state<Interval>("live");
  let selectedGroupKey = $state<string | null>(null);
  let selectedProcess = $state<ProcessRef | null>(null);
  let error = $state("");
  let scheduleRevision = $state(0);

  let history = $state<History>({
    cpu: [], cpuUser: [], cpuSystem: [], cores: [], memory: [], swap: [],
    disks: {}, networks: {}, gpus: {}, batteryPower: [], thermals: {},
    selectedGroup: [], selectedProcess: [],
  });

  const append = (series: Series, value: number | null) => [...series, value].slice(-historyLimit);
  const pad = (length: number): Series => Array(Math.min(historyLimit, length)).fill(null);
  const pct = (value: number) => `${Math.max(0, value).toFixed(value >= 10 ? 0 : 1)}%`;

  function appendSnapshot(next: Snapshot) {
    const previousLength = history.cpu.length;
    const memoryPercent = next.memory.total > 0 ? next.memory.used / next.memory.total * 100 : 0;
    const swapPercent = next.memory.swapTotal > 0 ? next.memory.swapUsed / next.memory.swapTotal * 100 : 0;
    const coreCount = Math.max(history.cores.length, next.cpu.cores.length);
    const cores = Array.from({ length: coreCount }, (_, index) => append(history.cores[index] ?? pad(previousLength), next.cpu.cores[index]?.usage ?? null));

    const disks: History["disks"] = {};
    const diskNames = new Set([...Object.keys(history.disks), ...next.disks.map((disk) => disk.name)]);
    for (const name of diskNames) {
      const prior = history.disks[name] ?? { active: pad(previousLength), read: pad(previousLength), write: pad(previousLength) };
      const disk = next.disks.find((candidate) => candidate.name === name);
      disks[name] = {
        active: append(prior.active, disk?.activePercent ?? null),
        read: append(prior.read, disk?.readRate ?? null),
        write: append(prior.write, disk?.writeRate ?? null),
      };
    }

    const networks: History["networks"] = {};
    const networkNames = new Set([...Object.keys(history.networks), ...next.networks.map((network) => network.name)]);
    for (const name of networkNames) {
      const prior = history.networks[name] ?? { rx: pad(previousLength), tx: pad(previousLength) };
      const network = next.networks.find((candidate) => candidate.name === name);
      networks[name] = { rx: append(prior.rx, network?.rxRate ?? null), tx: append(prior.tx, network?.txRate ?? null) };
    }

    const gpus: History["gpus"] = {};
    const gpuIndexes = new Set([...Object.keys(history.gpus).map(Number), ...next.gpus.map((gpu) => gpu.index)]);
    for (const index of gpuIndexes) {
      const prior = history.gpus[index] ?? { usage: pad(previousLength), memory: pad(previousLength), encoder: pad(previousLength), decoder: pad(previousLength) };
      const gpu = next.gpus.find((candidate) => candidate.index === index);
      gpus[index] = {
        usage: append(prior.usage, gpu?.usage ?? null),
        memory: append(prior.memory, gpu?.memoryUsage ?? null),
        encoder: append(prior.encoder, gpu?.encoder ?? null),
        decoder: append(prior.decoder, gpu?.decoder ?? null),
      };
    }

    const thermals: History["thermals"] = {};
    const sensorIds = new Set([...Object.keys(history.thermals), ...next.thermal.sensors.map((sensor) => sensor.id)]);
    for (const id of sensorIds) {
      const sensor = next.thermal.sensors.find((candidate) => candidate.id === id);
      thermals[id] = append(history.thermals[id] ?? pad(previousLength), sensor?.temperature ?? null);
    }

    const selectedGroup = selectedGroupKey ? next.groups.find((group) => group.key === selectedGroupKey) : null;
    const selectedMember = selectedGroup && selectedProcess
      ? selectedGroup.members.find((member) => member.pid === selectedProcess?.pid && member.startTime === selectedProcess?.startTime)
      : null;

    history = {
      cpu: append(history.cpu, next.cpu.usage),
      cpuUser: append(history.cpuUser, next.cpu.user),
      cpuSystem: append(history.cpuSystem, next.cpu.system),
      cores,
      memory: append(history.memory, memoryPercent),
      swap: append(history.swap, swapPercent),
      disks,
      networks,
      gpus,
      batteryPower: append(history.batteryPower, next.energy.batteryPower),
      thermals,
      selectedGroup: selectedGroupKey ? append(history.selectedGroup, selectedGroup?.cpu ?? null) : [],
      selectedProcess: selectedProcess ? append(history.selectedProcess, selectedMember?.cpu ?? null) : [],
    };
  }

  function appendGap() {
    const mapSeries = <T extends Record<string, Series>>(record: T) => Object.fromEntries(
      Object.entries(record).map(([key, series]) => [key, append(series, null)]),
    ) as T;

    history = {
      cpu: append(history.cpu, null),
      cpuUser: append(history.cpuUser, null),
      cpuSystem: append(history.cpuSystem, null),
      cores: history.cores.map((series) => append(series, null)),
      memory: append(history.memory, null),
      swap: append(history.swap, null),
      disks: Object.fromEntries(Object.entries(history.disks).map(([key, series]) => [key, {
        active: append(series.active, null), read: append(series.read, null), write: append(series.write, null),
      }])),
      networks: Object.fromEntries(Object.entries(history.networks).map(([key, series]) => [key, {
        rx: append(series.rx, null), tx: append(series.tx, null),
      }])),
      gpus: Object.fromEntries(Object.entries(history.gpus).map(([key, series]) => [key, {
        usage: append(series.usage, null), memory: append(series.memory, null), encoder: append(series.encoder, null), decoder: append(series.decoder, null),
      }])),
      batteryPower: append(history.batteryPower, null),
      thermals: mapSeries(history.thermals),
      selectedGroup: selectedGroupKey ? append(history.selectedGroup, null) : [],
      selectedProcess: selectedProcess ? append(history.selectedProcess, null) : [],
    };
  }

  async function refresh() {
    try {
      const next = await invoke<Snapshot>("snapshot");
      snapshot = next;
      appendSnapshot(next);
      error = "";
    } catch (cause) {
      appendGap();
      error = String(cause);
    }
  }

  function changeInterval(next: Interval) {
    if (interval === next) return;
    interval = next;
    scheduleRevision++;
  }

  function changeSelection(groupKey: string | null, process: ProcessRef | null) {
    const changed = selectedGroupKey !== groupKey || selectedProcess?.pid !== process?.pid || selectedProcess?.startTime !== process?.startTime;
    selectedGroupKey = groupKey;
    selectedProcess = process;
    if (!changed) return;
    const group = groupKey ? snapshot?.groups.find((candidate) => candidate.key === groupKey) : null;
    const member = group && process ? group.members.find((candidate) => candidate.pid === process.pid && candidate.startTime === process.startTime) : null;
    history = {
      ...history,
      selectedGroup: group ? [group.cpu] : [],
      selectedProcess: member ? [member.cpu] : [],
    };
  }

  function openProcess(key: string) {
    changeSelection(key, null);
    page = "processes";
  }

  function reportError(message: string) {
    error = message;
  }

  let memoryPercent = $derived(snapshot?.memory.total ? snapshot.memory.used / snapshot.memory.total * 100 : 0);
  let intervalMs = $derived(intervals[interval]);

  onMount(() => {
    let stopped = false;
    let timer = 0;
    let nextDue = Date.now();
    let seenRevision = scheduleRevision;

    async function tick() {
      if (stopped) return;
      const now = Date.now();
      if (seenRevision !== scheduleRevision) {
        seenRevision = scheduleRevision;
        nextDue = now;
      }
      const period = intervals[interval];
      if (now >= nextDue) {
        const due = Math.floor((now - nextDue) / period) + 1;
        for (let index = 1; index < due; index++) appendGap();
        nextDue += due * period;
        if (document.visibilityState === "visible") await refresh();
        else appendGap();
      }
      if (!stopped) timer = window.setTimeout(tick, Math.min(1000, Math.max(100, nextDue - Date.now())));
    }

    tick();
    return () => {
      stopped = true;
      window.clearTimeout(timer);
    };
  });
</script>

<svelte:head><title>RyoManager</title></svelte:head>

<div class="app-shell" style={`--ryo-signal:${snapshot?.accent ?? "#7898a5"}`}>
  <aside class="sidebar">
    <div class="rail-head">
      <div class="brand-plate">
        <div class="brand-mark">力</div>
        <div class="brand-copy"><div class="brand-name">RYOMANAGER</div><div class="brand-sub">//SYSTEM_CONTROL_</div></div>
        <div class="brand-kanji">管</div>
      </div>
    </div>

    <nav class="nav" aria-label="RyoManager sections">
      <div class="nav-group">
        <div class="nav-heading"><span>01</span><b>MONITOR</b><i></i></div>
        <button class:active={page === "processes"} onclick={() => page = "processes"}><span>Processes</span><span class="nav-jp">処理</span></button>
        <button class:active={page === "performance"} onclick={() => page = "performance"}><span>Performance</span><span class="nav-jp">性能</span></button>
        <button class:active={page === "system"} onclick={() => page = "system"}><span>System</span><span class="nav-jp">機体</span></button>
      </div>
      <div class="nav-group">
        <div class="nav-heading"><span>02</span><b>CONTROL</b><i></i></div>
        <button class="future" disabled><span>Startup</span><span class="nav-jp">起動</span><small>NEXT</small></button>
        <button class="future" disabled><span>Services</span><span class="nav-jp">服務</span><small>NEXT</small></button>
      </div>
    </nav>

    <div class="sidebar-status">
      <div class="status-rule"></div>
      <div class="status-row"><span>CPU</span><strong>{snapshot ? pct(snapshot.cpu.usage) : "—"}</strong></div>
      <div class="status-row"><span>MEM</span><strong>{snapshot ? pct(memoryPercent) : "—"}</strong></div>
      <div class="status-row"><span>PROC</span><strong>{snapshot?.processCount ?? "—"}</strong></div>
      <div class="host">{snapshot?.system.hostname ?? "COLLECTING…"}</div>
      <div class="rail-foot-meta"><div><span>ED.</span><strong>001</strong></div><div class="barcode" aria-hidden="true"></div><span class="rail-plus">+</span></div>
    </div>
  </aside>

  <main class="main">
    <header class="topbar">
      <div class="eyebrow"><span class="eyebrow-rule"></span><span>RYOKU DESKTOP / RYOMANAGER</span><span class="jp">管理</span></div>
      <div class="top-metrics"><span>CPU {snapshot ? pct(snapshot.cpu.usage) : "—"}</span><span class="dot">/</span><span>MEM {snapshot ? pct(memoryPercent) : "—"}</span><span class="dot">/</span><span class="top-runtime">{snapshot?.system.kernel ?? "—"}</span></div>
    </header>

    {#if error}<div class="error-strip"><span>FAULT</span>{error}<button onclick={() => error = ""} aria-label="Dismiss error">×</button></div>{/if}

    {#if page === "processes"}
      <ProcessesPage {snapshot} {history} {interval} {selectedGroupKey} {selectedProcess} onIntervalChange={changeInterval} onSelectionChange={changeSelection} onError={reportError} />
    {:else if page === "performance"}
      <PerformancePage {snapshot} {history} onOpenProcess={openProcess} />
    {:else}
      <SystemPage {snapshot} {history} {intervalMs} />
    {/if}
  </main>
</div>
