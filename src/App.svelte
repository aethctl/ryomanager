<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import SparkGraph from "./lib/components/SparkGraph.svelte";
  import type { ProcessInfo, Snapshot } from "./lib/types";

  type Page = "processes" | "performance" | "system";
  type SortKey = "name" | "cpu" | "memory" | "pid";
  type Interval = "live" | "minute" | "three";

  let page: Page = "processes";
  let snapshot: Snapshot | null = null;
  let cpuHistory: number[] = [];
  let memoryHistory: number[] = [];
  let query = "";
  let selectedPid: number | null = null;
  let sortKey: SortKey = "cpu";
  let sortDescending = true;
  let busy = false;
  let error = "";
  let selectedMetric: "cpu" | "memory" = "cpu";
  let interval: Interval = "live";
  let inspected: ProcessInfo | null = null;
  let inspectedHistory: number[] = [];
  let historyPid: number | null = null;

  const historyLimit = 60;

  const intervals: { id: Interval; label: string; ms: number }[] = [
    { id: "live", label: "LIVE", ms: 1000 },
    { id: "minute", label: "1 MIN", ms: 60_000 },
    { id: "three", label: "3 MIN", ms: 180_000 },
  ];

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

  const pct = (value: number) => `${Math.max(0, value).toFixed(value >= 10 ? 0 : 1)}%`;

  $: period = intervals.find((option) => option.id === interval) ?? intervals[0];
  $: historySeconds = (period.ms * historyLimit) / 1000;

  const formatWindow = (seconds: number) =>
    seconds < 90 ? `${seconds} s` : `${Math.round(seconds / 60)} min`;

  async function refresh() {
    try {
      const next = await invoke<Snapshot>("snapshot");
      snapshot = next;
      cpuHistory = [...cpuHistory, next.cpu].slice(-historyLimit);
      const memoryPct = next.totalMemory > 0 ? (next.usedMemory / next.totalMemory) * 100 : 0;
      memoryHistory = [...memoryHistory, memoryPct].slice(-historyLimit);
      if (selectedPid !== historyPid) inspectedHistory = [];
      historyPid = selectedPid;
      const current = selectedPid === null
        ? undefined
        : next.processes.find((process) => process.pid === selectedPid);
      inspectedHistory = current ? [...inspectedHistory, current.cpu].slice(-historyLimit) : [];
      error = "";
    } catch (cause) {
      error = String(cause);
    }
  }

  async function endSelected(force = false) {
    if (selectedPid === null || busy) return;
    busy = true;
    try {
      await invoke("end_process", { pid: selectedPid, force });
      selectedPid = null;
      await refresh();
    } catch (cause) {
      error = String(cause);
    } finally {
      busy = false;
    }
  }

  function sortBy(key: SortKey) {
    if (sortKey === key) sortDescending = !sortDescending;
    else {
      sortKey = key;
      sortDescending = key !== "name";
    }
  }

  function openProcess(pid: number) {
    selectedPid = pid;
    page = "processes";
  }

  $: processes = snapshot?.processes ?? [];
  $: filtered = processes
    .filter((process) => {
      const needle = query.trim().toLowerCase();
      return !needle || process.name.toLowerCase().includes(needle) ||
        process.command.toLowerCase().includes(needle) || String(process.pid).includes(needle);
    })
    .sort((a, b) => {
      let result = 0;
      if (sortKey === "name") result = a.name.localeCompare(b.name);
      if (sortKey === "cpu") result = a.cpu - b.cpu;
      if (sortKey === "memory") result = a.memory - b.memory;
      if (sortKey === "pid") result = a.pid - b.pid;
      return sortDescending ? -result : result;
    });
  $: selected = selectedPid === null ? null : processes.find((process) => process.pid === selectedPid) ?? null;
  $: if (selected) inspected = selected;
  $: memoryPercent = snapshot && snapshot.totalMemory > 0 ? (snapshot.usedMemory / snapshot.totalMemory) * 100 : 0;
  $: cpuLeaders = [...processes].sort((a, b) => b.cpu - a.cpu).slice(0, 5);
  $: memoryLeaders = [...processes].sort((a, b) => b.memory - a.memory).slice(0, 5);

  onMount(() => {
    let stopped = false;
    let timer = 0;
    let lastRefresh = 0;

    async function tick() {
      const now = Date.now();
      if (!stopped && document.visibilityState === "visible" && now - lastRefresh >= period.ms) {
        lastRefresh = now;
        await refresh();
      }
      if (!stopped) timer = window.setTimeout(tick, 1000);
    }

    tick();
    return () => {
      stopped = true;
      window.clearTimeout(timer);
    };
  });
</script>

<svelte:head>
  <title>RyoManager</title>
</svelte:head>

<div class="app-shell" style={`--ryo-signal:${snapshot?.accent ?? "#7898a5"}`}>
  <aside class="sidebar">
    <div class="rail-head">
      <div class="brand-plate">
        <div class="brand-mark">力</div>
        <div class="brand-copy">
          <div class="brand-name">RYOMANAGER</div>
          <div class="brand-sub">//SYSTEM_CONTROL_</div>
        </div>
        <div class="brand-kanji">管</div>
      </div>
    </div>

    <nav class="nav" aria-label="RyoManager sections">
      <div class="nav-group">
        <div class="nav-heading"><span>01</span><b>MONITOR</b><i></i></div>
        <button class:active={page === "processes"} onclick={() => page = "processes"}>
          <span>Processes</span><span class="nav-jp">処理</span>
        </button>
        <button class:active={page === "performance"} onclick={() => page = "performance"}>
          <span>Performance</span><span class="nav-jp">性能</span>
        </button>
        <button class:active={page === "system"} onclick={() => page = "system"}>
          <span>System</span><span class="nav-jp">機体</span>
        </button>
      </div>

      <div class="nav-group">
        <div class="nav-heading"><span>02</span><b>CONTROL</b><i></i></div>
          <button class="future" disabled>
            <span>Startup</span><span class="nav-jp">起動</span><small>NEXT</small>
          </button>
          <button class="future" disabled>
            <span>Services</span><span class="nav-jp">服務</span><small>NEXT</small>
          </button>
      </div>
    </nav>

    <div class="sidebar-status">
      <div class="status-rule"></div>
      <div class="status-row"><span>CPU</span><strong>{snapshot ? pct(snapshot.cpu) : "—"}</strong></div>
      <div class="status-row"><span>MEM</span><strong>{snapshot ? pct(memoryPercent) : "—"}</strong></div>
      <div class="status-row"><span>PROC</span><strong>{snapshot?.processCount ?? "—"}</strong></div>
      <div class="host">{snapshot?.system.hostname ?? "COLLECTING…"}</div>
      <div class="rail-foot-meta">
        <div><span>ED.</span><strong>001</strong></div>
        <div class="barcode" aria-hidden="true"></div>
        <span class="rail-plus">+</span>
      </div>
    </div>
  </aside>

  <main class="main">
    <header class="topbar">
      <div class="eyebrow"><span class="eyebrow-rule"></span><span>RYOKU DESKTOP / RYOMANAGER</span><span class="jp">管理</span></div>
      <div class="top-metrics">
        <span>CPU {snapshot ? pct(snapshot.cpu) : "—"}</span>
        <span class="dot">/</span>
        <span>MEM {snapshot ? pct(memoryPercent) : "—"}</span>
        <span class="dot">/</span>
        <span class="top-runtime">{snapshot?.system.kernel ?? "—"}</span>
      </div>
    </header>

    {#if error}
      <div class="error-strip"><span>FAULT</span>{error}</div>
    {/if}

    {#if page === "processes"}
      <section class="page processes-page">
        <div class="page-head">
          <div class="title-block">
            <div class="section-label"><i></i><span class="section-mark">力</span> SYSTEM / PROCESSES <span>処理</span><b>01</b></div>
            <h1>Processes</h1>
            <p>Live process activity, resource use and control in one clear view.</p>
          </div>
          <div class="head-actions">
            <label class="search">
              <span>⌕</span>
              <input bind:value={query} placeholder="Search process, command or PID" />
            </label>
            <button class="button" disabled={!selected || busy} onclick={() => endSelected(false)}>END TASK</button>
            <button class="button danger" disabled={!selected || busy} onclick={() => endSelected(true)}>FORCE STOP</button>
          </div>
        </div>

        <div class="summary-strip">
          <div><span>CPU LOAD</span><strong>{snapshot ? pct(snapshot.cpu) : "—"}</strong><SparkGraph values={cpuHistory} /></div>
          <div><span>MEMORY FIELD</span><strong>{snapshot ? `${formatBytes(snapshot.usedMemory)} / ${formatBytes(snapshot.totalMemory)}` : "—"}</strong><SparkGraph values={memoryHistory} /></div>
          <div><span>VISIBLE PROCESSES</span><strong>{snapshot?.processCount ?? "—"}</strong><small>refresh {formatWindow(period.ms / 1000)}</small></div>
        </div>

        <div class="process-workspace" class:inspecting={selected !== null}>
          <div class="process-table-shell">
            <div class="table-kicker">
              <span>PROCESS INDEX</span>
              <div class="interval-switch" role="group" aria-label="Refresh interval">
                {#each intervals as option (option.id)}
                  <button class:active={interval === option.id} onclick={() => interval = option.id}>{option.label}</button>
                {/each}
              </div>
              <span>{filtered.length} VISIBLE</span>
            </div>
            <div class="process-header process-grid">
              <button onclick={() => sortBy("name")}>NAME {sortKey === "name" ? (sortDescending ? "↓" : "↑") : ""}</button>
              <button onclick={() => sortBy("pid")}>PID {sortKey === "pid" ? (sortDescending ? "↓" : "↑") : ""}</button>
              <button onclick={() => sortBy("cpu")}>CPU {sortKey === "cpu" ? (sortDescending ? "↓" : "↑") : ""}</button>
              <button onclick={() => sortBy("memory")}>MEMORY {sortKey === "memory" ? (sortDescending ? "↓" : "↑") : ""}</button>
              <span>STATE</span>
            </div>
            <div class="process-list">
              {#each filtered as process (process.pid)}
                <button
                  class="process-row process-grid"
                  class:selected={selectedPid === process.pid}
                  onclick={() => selectedPid = selectedPid === process.pid ? null : process.pid}
                  title={process.command || process.name}
                >
                  <span class="process-name"><b>{process.name}</b><small>{process.command || "No command line exposed"}</small></span>
                  <span class="mono">{process.pid}</span>
                  <span class="metric-cell"><b>{pct(process.cpu)}</b><i style={`--value:${Math.min(100, process.cpu)}%`}></i></span>
                  <span class="metric-cell"><b>{formatBytes(process.memory)}</b><i style={`--value:${snapshot?.totalMemory ? Math.min(100, (process.memory / snapshot.totalMemory) * 100) : 0}%`}></i></span>
                  <span class="status">{process.status}</span>
                </button>
              {/each}
            </div>
          </div>

          <aside class="process-inspector" class:open={selected !== null}>
            {#if inspected}
              <div class="inspector-head">
                <span>SELECTED PROCESS</span>
                <strong>{inspected.name}</strong>
                <small>PID {inspected.pid}</small>
                <div class="inspector-actions">
                  <button class="button" disabled={!selected || busy} onclick={() => endSelected(false)}>End task</button>
                  <button class="button danger" disabled={!selected || busy} onclick={() => endSelected(true)}>Force stop</button>
                </div>
              </div>
              <div class="inspector-graph">
                <div class="inspector-graph-head"><span>CPU</span><strong>{pct(inspected.cpu)}</strong></div>
                <SparkGraph values={inspectedHistory} label="Process CPU history" />
              </div>
              <div class="inspector-share">
                <div class="inspector-graph-head"><span>MEMORY SHARE</span><strong>{formatBytes(inspected.memory)}</strong></div>
                <i style={`--share:${snapshot?.totalMemory ? Math.min(100, (inspected.memory / snapshot.totalMemory) * 100) : 0}%`}></i>
                <small>{snapshot && snapshot.totalMemory ? pct((inspected.memory / snapshot.totalMemory) * 100) : "—"} of {snapshot ? formatBytes(snapshot.totalMemory) : "—"}</small>
              </div>
              <div class="inspector-registry">
                <div><span>STATE</span><strong>{inspected.status}</strong></div>
                <div><span>PARENT PID</span><strong>{inspected.parentPid ?? "—"}</strong></div>
                <div class="wide"><span>COMMAND</span><code>{inspected.command || "No command line exposed"}</code></div>
              </div>
            {/if}
          </aside>
        </div>
      </section>
    {:else if page === "performance"}
      <section class="page performance-page">
        <div class="page-head compact">
          <div class="title-block">
            <div class="section-label"><i></i><span class="section-mark">力</span> SYSTEM / PERFORMANCE <span>性能</span><b>02</b></div>
            <h1>Performance</h1>
            <p>CPU and memory over the last {formatWindow(historySeconds)}, and the processes driving them.</p>
          </div>
        </div>

        <div class="performance-grid">
          <div class="perf-card">
            <div class="perf-card-head"><span>CPU</span><strong>{snapshot ? pct(snapshot.cpu) : "—"}</strong></div>
            <SparkGraph values={cpuHistory} tall label="CPU history" />
            <div class="perf-card-foot">
              <span>{snapshot?.system.cpuModel ?? "Detecting processor…"}</span>
              <span>{snapshot?.system.physicalCores ?? "—"} cores / {snapshot?.system.logicalCpus ?? "—"} threads</span>
            </div>
          </div>
          <div class="perf-card">
            <div class="perf-card-head"><span>MEMORY</span><strong>{snapshot ? pct(memoryPercent) : "—"}</strong></div>
            <SparkGraph values={memoryHistory} tall label="Memory history" />
            <div class="perf-card-foot">
              <span>{snapshot ? formatBytes(snapshot.usedMemory) : "—"} in use</span>
              <span>{snapshot ? formatBytes(snapshot.totalMemory - snapshot.usedMemory) : "—"} available</span>
            </div>
          </div>
        </div>

        <div class="perf-leaders">
          <div class="perf-leaders-head">
            <button class:active={selectedMetric === "cpu"} onclick={() => selectedMetric = "cpu"}>TOP CPU</button>
            <button class:active={selectedMetric === "memory"} onclick={() => selectedMetric = "memory"}>TOP MEMORY</button>
            <span>select a row to inspect it in Processes</span>
          </div>
          <div class="perf-leaders-list">
            {#each selectedMetric === "cpu" ? cpuLeaders : memoryLeaders as process (process.pid)}
              <button class="perf-leader-row" onclick={() => openProcess(process.pid)}>
                <span class="perf-leader-name"><b>{process.name}</b><small>PID {process.pid}</small></span>
                <i style={`--leader:${Math.min(100, selectedMetric === "cpu" ? process.cpu : snapshot?.totalMemory ? (process.memory / snapshot.totalMemory) * 100 : 0)}%`}></i>
                <strong>{selectedMetric === "cpu" ? pct(process.cpu) : formatBytes(process.memory)}</strong>
              </button>
            {/each}
          </div>
        </div>
      </section>
    {:else}
      <section class="page system-page">
        <div class="page-head compact">
          <div class="title-block">
            <div class="section-label"><i></i><span class="section-mark">力</span> SYSTEM / OVERVIEW <span>機体</span><b>03</b></div>
            <h1>System</h1>
            <p>Hardware, kernel and live resource state for this Ryoku workstation.</p>
          </div>
        </div>

        <div class="system-hero panel-ticks">
          <div class="system-mark">力</div>
          <div class="system-copy"><span>RYOKU WORKSTATION // 機体</span><h2>{snapshot?.system.hostname ?? "Collecting…"}</h2><p>{snapshot?.system.os ?? "Linux"}</p></div>
          <div class="system-seal">緑<br /><small>RM</small></div>
        </div>
        <div class="system-grid">
          <div><span>PROCESSOR</span><strong>{snapshot?.system.cpuModel ?? "—"}</strong><small>{snapshot?.system.physicalCores ?? "—"} cores / {snapshot?.system.logicalCpus ?? "—"} threads</small></div>
          <div><span>MEMORY</span><strong>{snapshot ? formatBytes(snapshot.totalMemory) : "—"}</strong><small>{snapshot ? `${formatBytes(snapshot.usedMemory)} currently in use` : ""}</small></div>
          <div><span>KERNEL</span><strong>{snapshot?.system.kernel ?? "—"}</strong><small>Linux / Ryoku host</small></div>
          <div><span>LIVE PROCESSES</span><strong>{snapshot?.processCount ?? "—"}</strong><small>Visible to the current user session</small></div>
        </div>

        <div class="system-bottom-grid">
          <div class="system-telemetry">
            <div class="system-telemetry-head">
              <div><span>LIVE TELEMETRY</span><strong>Resource state</strong></div>
              <small>{formatWindow(historySeconds)} WINDOW // {formatWindow(period.ms / 1000)} SAMPLE</small>
            </div>
            <div class="system-telemetry-grid">
              <div>
                <div class="telemetry-head"><span>CPU</span><strong>{snapshot ? pct(snapshot.cpu) : "—"}</strong></div>
                <SparkGraph values={cpuHistory} />
              </div>
              <div>
                <div class="telemetry-head"><span>MEMORY</span><strong>{snapshot ? pct(memoryPercent) : "—"}</strong></div>
                <SparkGraph values={memoryHistory} />
              </div>
              <div class="runtime-register">
                <span>RUNTIME</span>
                <strong>{snapshot?.system.os ?? "Linux"}</strong>
                <small>{snapshot?.system.kernel ?? "—"}</small>
              </div>
            </div>
          </div>

          <div class="system-process-card">
            <div class="system-process-head">
              <div><span>ACTIVE PROCESS FIELD</span><strong>Highest memory</strong></div>
              <small>LIVE</small>
            </div>
            <div class="system-process-list">
              {#each memoryLeaders.slice(0, 5) as process (process.pid)}
                <div>
                  <span><b>{process.name}</b><small>PID {process.pid}</small></span>
                  <strong>{formatBytes(process.memory)}</strong>
                </div>
              {/each}
            </div>
          </div>
        </div>
      </section>
    {/if}
  </main>
</div>
