<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import SparkGraph from "./lib/components/SparkGraph.svelte";
  import type { ProcessInfo, Snapshot } from "./lib/types";

  type Page = "processes" | "performance" | "system";
  type SortKey = "name" | "cpu" | "memory" | "pid";

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

  const historyLimit = 60;

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

  async function refresh() {
    try {
      const next = await invoke<Snapshot>("snapshot");
      snapshot = next;
      cpuHistory = [...cpuHistory, next.cpu].slice(-historyLimit);
      const memoryPct = next.totalMemory > 0 ? (next.usedMemory / next.totalMemory) * 100 : 0;
      memoryHistory = [...memoryHistory, memoryPct].slice(-historyLimit);
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
  $: memoryPercent = snapshot && snapshot.totalMemory > 0 ? (snapshot.usedMemory / snapshot.totalMemory) * 100 : 0;

  onMount(() => {
    let stopped = false;
    let timer = 0;

    async function tick() {
      if (!stopped && document.visibilityState === "visible") await refresh();
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

<div class="app-shell">
  <aside class="sidebar">
    <div class="brand">
      <div class="brand-mark">緑</div>
      <div>
        <div class="brand-name">RYOMANAGER</div>
        <div class="brand-sub">SYSTEM INSTRUMENT / 0.1</div>
      </div>
    </div>

    <nav class="nav" aria-label="RyoManager sections">
      <button class:active={page === "processes"} onclick={() => page = "processes"}>
        <span class="nav-index">01</span><span>Processes</span>
      </button>
      <button class:active={page === "performance"} onclick={() => page = "performance"}>
        <span class="nav-index">02</span><span>Performance</span>
      </button>
      <button class:active={page === "system"} onclick={() => page = "system"}>
        <span class="nav-index">03</span><span>System</span>
      </button>
      <button class="future" disabled><span class="nav-index">04</span><span>Startup</span><small>NEXT</small></button>
      <button class="future" disabled><span class="nav-index">05</span><span>Services</span><small>NEXT</small></button>
    </nav>

    <div class="sidebar-status">
      <div class="status-rule"></div>
      <div class="status-row"><span>CPU</span><strong>{snapshot ? pct(snapshot.cpu) : "—"}</strong></div>
      <div class="status-row"><span>MEM</span><strong>{snapshot ? pct(memoryPercent) : "—"}</strong></div>
      <div class="status-row"><span>PROC</span><strong>{snapshot?.processCount ?? "—"}</strong></div>
      <div class="host">{snapshot?.system.hostname ?? "COLLECTING…"}</div>
    </div>
  </aside>

  <main class="main">
    <header class="topbar">
      <div class="eyebrow"><span class="eyebrow-rule"></span><span>RYOKU / SYSTEM</span><span class="jp">管理</span></div>
      <div class="top-metrics">
        <span>{snapshot?.system.os ?? "Linux"}</span>
        <span class="dot">•</span>
        <span>{snapshot?.system.kernel ?? "—"}</span>
      </div>
    </header>

    {#if error}
      <div class="error-strip">{error}</div>
    {/if}

    {#if page === "processes"}
      <section class="page processes-page">
        <div class="page-head">
          <div>
            <div class="section-label">LIVE PROCESS TABLE</div>
            <h1>Processes</h1>
            <p>Understand what is using the machine, then act without translating a PID table in your head.</p>
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
          <div><span>CPU</span><strong>{snapshot ? pct(snapshot.cpu) : "—"}</strong><SparkGraph values={cpuHistory} /></div>
          <div><span>MEMORY</span><strong>{snapshot ? `${formatBytes(snapshot.usedMemory)} / ${formatBytes(snapshot.totalMemory)}` : "—"}</strong><SparkGraph values={memoryHistory} /></div>
          <div><span>PROCESSES</span><strong>{snapshot?.processCount ?? "—"}</strong><small>1 second live refresh</small></div>
        </div>

        <div class="process-table-shell">
          <div class="process-header process-grid">
            <button onclick={() => sortBy("name")}>NAME {sortKey === "name" ? (sortDescending ? "↓" : "↑") : ""}</button>
            <button onclick={() => sortBy("pid")}>PID {sortKey === "pid" ? (sortDescending ? "↓" : "↑") : ""}</button>
            <button onclick={() => sortBy("cpu")}>CPU {sortKey === "cpu" ? (sortDescending ? "↓" : "↑") : ""}</button>
            <button onclick={() => sortBy("memory")}>MEMORY {sortKey === "memory" ? (sortDescending ? "↓" : "↑") : ""}</button>
            <span>STATUS</span>
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

        {#if selected}
          <div class="selection-bar">
            <span>SELECTED</span><strong>{selected.name}</strong><span>PID {selected.pid}</span><span>{pct(selected.cpu)} CPU</span><span>{formatBytes(selected.memory)}</span>
          </div>
        {/if}
      </section>
    {:else if page === "performance"}
      <section class="page performance-page">
        <div class="page-head compact">
          <div>
            <div class="section-label">60 SECOND HISTORY</div>
            <h1>Performance</h1>
            <p>Readable first, detailed second. The machine should not require a decoder ring.</p>
          </div>
        </div>

        <div class="performance-layout">
          <div class="metric-rail">
            <button class:active={selectedMetric === "cpu"} onclick={() => selectedMetric = "cpu"}>
              <div><span>CPU</span><strong>{snapshot ? pct(snapshot.cpu) : "—"}</strong></div>
              <SparkGraph values={cpuHistory} />
            </button>
            <button class:active={selectedMetric === "memory"} onclick={() => selectedMetric = "memory"}>
              <div><span>MEMORY</span><strong>{snapshot ? pct(memoryPercent) : "—"}</strong></div>
              <SparkGraph values={memoryHistory} />
            </button>
          </div>

          <div class="performance-main">
            {#if selectedMetric === "cpu"}
              <div class="metric-title"><div><div class="section-label">PROCESSOR</div><h2>CPU</h2></div><span>{snapshot?.system.cpuModel ?? "Detecting processor…"}</span></div>
              <div class="big-number">{snapshot ? pct(snapshot.cpu) : "—"}</div>
              <div class="chart-frame"><span class="chart-top">100%</span><span class="chart-bottom">0%</span><SparkGraph values={cpuHistory} tall /></div>
              <div class="detail-grid">
                <div><span>Processes</span><strong>{snapshot?.processCount ?? "—"}</strong></div>
                <div><span>Physical cores</span><strong>{snapshot?.system.physicalCores ?? "—"}</strong></div>
                <div><span>Logical processors</span><strong>{snapshot?.system.logicalCpus ?? "—"}</strong></div>
                <div><span>Refresh</span><strong>1 s</strong></div>
              </div>
            {:else}
              <div class="metric-title"><div><div class="section-label">PHYSICAL MEMORY</div><h2>Memory</h2></div><span>{snapshot ? formatBytes(snapshot.totalMemory) : "—"} installed</span></div>
              <div class="big-number">{snapshot ? pct(memoryPercent) : "—"}</div>
              <div class="chart-frame"><span class="chart-top">100%</span><span class="chart-bottom">0%</span><SparkGraph values={memoryHistory} tall /></div>
              <div class="detail-grid">
                <div><span>In use</span><strong>{snapshot ? formatBytes(snapshot.usedMemory) : "—"}</strong></div>
                <div><span>Available</span><strong>{snapshot ? formatBytes(snapshot.totalMemory - snapshot.usedMemory) : "—"}</strong></div>
                <div><span>Swap</span><strong>{snapshot ? `${formatBytes(snapshot.usedSwap)} / ${formatBytes(snapshot.totalSwap)}` : "—"}</strong></div>
                <div><span>Utilisation</span><strong>{snapshot ? pct(memoryPercent) : "—"}</strong></div>
              </div>
            {/if}
          </div>
        </div>
      </section>
    {:else}
      <section class="page system-page">
        <div class="page-head compact">
          <div>
            <div class="section-label">MACHINE IDENTITY</div>
            <h1>System</h1>
            <p>The useful facts, without making you inspect six different pseudo-filesystems.</p>
          </div>
        </div>

        <div class="system-hero">
          <div class="system-mark">緑</div>
          <div><span>RYOKU WORKSTATION</span><h2>{snapshot?.system.hostname ?? "Collecting…"}</h2><p>{snapshot?.system.os ?? "Linux"}</p></div>
        </div>
        <div class="system-grid">
          <div><span>Processor</span><strong>{snapshot?.system.cpuModel ?? "—"}</strong><small>{snapshot?.system.physicalCores ?? "—"} cores / {snapshot?.system.logicalCpus ?? "—"} threads</small></div>
          <div><span>Memory</span><strong>{snapshot ? formatBytes(snapshot.totalMemory) : "—"}</strong><small>{snapshot ? `${formatBytes(snapshot.usedMemory)} currently in use` : ""}</small></div>
          <div><span>Kernel</span><strong>{snapshot?.system.kernel ?? "—"}</strong><small>Linux kernel</small></div>
          <div><span>Live processes</span><strong>{snapshot?.processCount ?? "—"}</strong><small>Visible to your user session</small></div>
        </div>
      </section>
    {/if}
  </main>
</div>
