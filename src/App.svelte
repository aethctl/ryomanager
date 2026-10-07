<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import SparkGraph from "./lib/components/SparkGraph.svelte";
  import type { Snapshot } from "./lib/types";

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
  $: cpuLeaders = [...processes].sort((a, b) => b.cpu - a.cpu).slice(0, 5);
  $: memoryLeaders = [...processes].sort((a, b) => b.memory - a.memory).slice(0, 5);

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
          <span class="nav-prefix">//</span><span>Processes</span><span class="nav-jp">処理</span>
        </button>
        <button class:active={page === "performance"} onclick={() => page = "performance"}>
          <span class="nav-prefix">//</span><span>Performance</span><span class="nav-jp">性能</span>
        </button>
        <button class:active={page === "system"} onclick={() => page = "system"}>
          <span class="nav-prefix">//</span><span>System</span><span class="nav-jp">機体</span>
        </button>
      </div>

      <div class="nav-group">
        <div class="nav-heading"><span>02</span><b>CONTROL</b><i></i></div>
        <button class="future" disabled>
          <span class="nav-prefix">//</span><span>Startup</span><span class="nav-jp">起動</span><small>NEXT</small>
        </button>
        <button class="future" disabled>
          <span class="nav-prefix">//</span><span>Services</span><span class="nav-jp">服務</span><small>NEXT</small>
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

        <div class="summary-strip panel-ticks">
          <div><span>CPU LOAD</span><strong>{snapshot ? pct(snapshot.cpu) : "—"}</strong><SparkGraph values={cpuHistory} /></div>
          <div><span>MEMORY FIELD</span><strong>{snapshot ? `${formatBytes(snapshot.usedMemory)} / ${formatBytes(snapshot.totalMemory)}` : "—"}</strong><SparkGraph values={memoryHistory} /></div>
          <div><span>VISIBLE PROCESSES</span><strong>{snapshot?.processCount ?? "—"}</strong><small>1 s acquisition interval</small></div>
        </div>

        <div class="process-workspace">
          <div class="process-table-shell">
            <div class="table-kicker">
              <span>PROCESS INDEX // LIVE</span>
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

          <aside class="process-inspector" class:empty={!selected}>
            {#if selected}
              <div class="inspector-head">
                <span>SELECTED PROCESS</span>
                <strong>{selected.name}</strong>
                <small>PID {selected.pid}</small>
              </div>
              <div class="inspector-metrics">
                <div><span>CPU</span><strong>{pct(selected.cpu)}</strong></div>
                <div><span>MEMORY</span><strong>{formatBytes(selected.memory)}</strong></div>
                <div><span>STATE</span><strong>{selected.status}</strong></div>
                <div><span>PARENT</span><strong>{selected.parentPid ?? "—"}</strong></div>
              </div>
              <div class="inspector-command">
                <span>COMMAND</span>
                <code>{selected.command || "No command line exposed"}</code>
              </div>
              <div class="inspector-actions">
                <button class="button" disabled={busy} onclick={() => endSelected(false)}>End task</button>
                <button class="button danger" disabled={busy} onclick={() => endSelected(true)}>Force stop</button>
              </div>
            {:else}
              <div class="inspector-empty-mark">力</div>
              <strong>Select a process</strong>
              <p>Inspect CPU, memory, state and command details without leaving the process list.</p>
              <div class="inspector-overview">
                <span><small>CPU</small><b>{snapshot ? pct(snapshot.cpu) : "—"}</b></span>
                <span><small>MEM</small><b>{snapshot ? pct(memoryPercent) : "—"}</b></span>
                <span><small>PROC</small><b>{snapshot?.processCount ?? "—"}</b></span>
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
            <p>Live system telemetry with sixty seconds of readable history.</p>
          </div>
        </div>

        <div class="performance-layout">
          <div class="metric-rail">
            <button class:active={selectedMetric === "cpu"} onclick={() => selectedMetric = "cpu"}>
              <div><span>CPU</span><strong>{snapshot ? pct(snapshot.cpu) : "—"}</strong></div>
              <SparkGraph values={cpuHistory} />
              <small>処理装置 / PROCESSOR</small>
            </button>
            <button class:active={selectedMetric === "memory"} onclick={() => selectedMetric = "memory"}>
              <div><span>MEMORY</span><strong>{snapshot ? pct(memoryPercent) : "—"}</strong></div>
              <SparkGraph values={memoryHistory} />
              <small>記憶領域 / MEMORY</small>
            </button>
          </div>

          <div class="performance-main panel-ticks">
            {#if selectedMetric === "cpu"}
              <div class="metric-title"><div><div class="section-label">PROCESSOR // CPU</div><h2>Compute load</h2></div><span>{snapshot?.system.cpuModel ?? "Detecting processor…"}</span></div>
              <div class="big-number">{snapshot ? pct(snapshot.cpu) : "—"}</div>
              <div class="chart-frame"><span class="chart-top">100%</span><span class="chart-bottom">0%</span><SparkGraph values={cpuHistory} tall /></div>
              <div class="detail-grid">
                <div><span>PROCESSES</span><strong>{snapshot?.processCount ?? "—"}</strong></div>
                <div><span>PHYSICAL CORES</span><strong>{snapshot?.system.physicalCores ?? "—"}</strong></div>
                <div><span>LOGICAL THREADS</span><strong>{snapshot?.system.logicalCpus ?? "—"}</strong></div>
                <div><span>ACQUISITION</span><strong>1 s</strong></div>
              </div>
            {:else}
              <div class="metric-title"><div><div class="section-label">PHYSICAL MEMORY // RAM</div><h2>Memory field</h2></div><span>{snapshot ? formatBytes(snapshot.totalMemory) : "—"} installed</span></div>
              <div class="big-number">{snapshot ? pct(memoryPercent) : "—"}</div>
              <div class="chart-frame"><span class="chart-top">100%</span><span class="chart-bottom">0%</span><SparkGraph values={memoryHistory} tall /></div>
              <div class="detail-grid">
                <div><span>IN USE</span><strong>{snapshot ? formatBytes(snapshot.usedMemory) : "—"}</strong></div>
                <div><span>AVAILABLE</span><strong>{snapshot ? formatBytes(snapshot.totalMemory - snapshot.usedMemory) : "—"}</strong></div>
                <div><span>SWAP</span><strong>{snapshot ? `${formatBytes(snapshot.usedSwap)} / ${formatBytes(snapshot.totalSwap)}` : "—"}</strong></div>
                <div><span>UTILISATION</span><strong>{snapshot ? pct(memoryPercent) : "—"}</strong></div>
              </div>
            {/if}
          </div>
        </div>

        <div class="performance-secondary">
          <div class="leader-card">
            <div class="card-register"><span>LOAD LEADERS</span><small>CPU // LIVE</small></div>
            <div class="leader-list">
              {#each cpuLeaders.slice(0, 4) as process (process.pid)}
                <div class="leader-row">
                  <span><b>{process.name}</b><small>PID {process.pid}</small></span>
                  <i style={`--leader:${Math.min(100, process.cpu)}%`}></i>
                  <strong>{pct(process.cpu)}</strong>
                </div>
              {/each}
            </div>
          </div>
          <div class="telemetry-card memory-card">
            <div class="telemetry-head"><span>MEMORY COMPOSITION</span><strong>{snapshot ? pct(memoryPercent) : "—"}</strong></div>
            <div class="memory-composition"><i style={`--used:${Math.min(100, memoryPercent)}%`}></i></div>
            <div class="memory-breakdown">
              <span><small>IN USE</small><b>{snapshot ? formatBytes(snapshot.usedMemory) : "—"}</b></span>
              <span><small>AVAILABLE</small><b>{snapshot ? formatBytes(snapshot.totalMemory - snapshot.usedMemory) : "—"}</b></span>
            </div>
          </div>
          <div class="telemetry-card telemetry-register">
            <span>PROCESS FIELD</span>
            <strong>{snapshot?.processCount ?? "—"}</strong>
            <small>visible processes / 1 s acquisition</small>
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
          <div class="system-telemetry panel-ticks">
            <div class="system-telemetry-head">
              <div><span>LIVE TELEMETRY</span><strong>Resource state</strong></div>
              <small>60 SECOND WINDOW // 1 S SAMPLE</small>
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
