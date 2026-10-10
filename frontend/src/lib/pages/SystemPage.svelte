<script lang="ts">
  import SparkGraph from "../components/SparkGraph.svelte";
  import type { History, Snapshot } from "../types";

  interface Props {
    snapshot?: Snapshot | null;
    history: History;
    intervalMs?: number;
  }

  let {
    snapshot = null,
    history,
    intervalMs = 1000,
  }: Props = $props();

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
  const formatWindow = (seconds: number) => seconds < 90 ? `${seconds} s` : `${Math.round(seconds / 60)} min`;

  let historySeconds = $derived((intervalMs * 60) / 1000);
  let memoryPercent = $derived(snapshot?.memory.total ? (snapshot.memory.used / snapshot.memory.total) * 100 : 0);
  let memoryLeaders = $derived([...(snapshot?.groups ?? [])].sort((a, b) => b.memory - a.memory).slice(0, 5));
</script>

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
    <div><span>MEMORY</span><strong>{snapshot ? formatBytes(snapshot.memory.total) : "—"}</strong><small>{snapshot ? `${formatBytes(snapshot.memory.used)} currently in use` : ""}</small></div>
    <div><span>KERNEL</span><strong>{snapshot?.system.kernel ?? "—"}</strong><small>Linux / Ryoku host</small></div>
    <div><span>LIVE PROCESSES</span><strong>{snapshot?.processCount ?? "—"}</strong><small>Visible to the current user session</small></div>
  </div>

  <div class="system-bottom-grid">
    <div class="system-telemetry">
      <div class="system-telemetry-head">
        <div><span>LIVE TELEMETRY</span><strong>Resource state</strong></div>
        <small>{formatWindow(historySeconds)} WINDOW // {formatWindow(intervalMs / 1000)} SAMPLE</small>
      </div>
      <div class="system-telemetry-grid">
        <div>
          <div class="telemetry-head"><span>CPU</span><strong>{snapshot ? pct(snapshot.cpu.usage) : "—"}</strong></div>
          <SparkGraph values={history.cpu} />
        </div>
        <div>
          <div class="telemetry-head"><span>MEMORY</span><strong>{snapshot ? pct(memoryPercent) : "—"}</strong></div>
          <SparkGraph values={history.memory} />
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
        {#each memoryLeaders as group (group.key)}
          <div>
            <span><b>{group.name}</b><small>PID {group.leaderPid}</small></span>
            <strong>{formatBytes(group.memory)}</strong>
          </div>
        {/each}
      </div>
    </div>
  </div>
</section>
