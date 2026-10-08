<script lang="ts">
  import SparkGraph from "../../components/SparkGraph.svelte";
  import type { History, Snapshot } from "../../types";
  import DetailGrid from "./DetailGrid.svelte";
  import DrivingList from "./DrivingList.svelte";
  import {
    formatFrequency,
    formatNumber,
    formatPercent,
    formatDuration,
    reasonFor,
    topGroups,
    type DetailItem,
  } from "./format";

  let {
    snapshot,
    history,
    onOpenProcess,
  }: {
    snapshot: Snapshot | null;
    history: History;
    onOpenProcess: (key: string) => void;
  } = $props();

  let view = $state<"overall" | "logical">("overall");
  let showKernel = $state(false);

  let cpu = $derived(snapshot?.cpu ?? null);
  let caches = $derived(new Map(cpu?.caches.map((cache) => [cache.level.toLowerCase(), cache.size]) ?? []));
  let openFilesReason = $derived(reasonFor(snapshot?.limits, ["cpu.openFiles", "openFiles"], "The system-wide open file count is unavailable."));
  let cacheReason = $derived(reasonFor(snapshot?.limits, ["cpu.caches", "cpuCache"], "CPU cache sizes are unavailable."));
  let details = $derived.by((): DetailItem[] => cpu ? [
    { label: "Utilization", value: formatPercent(cpu.usage) },
    { label: "Speed", value: cpu.freqMhz === null ? null : formatFrequency(cpu.freqMhz), reason: reasonFor(snapshot?.limits, ["cpu.freqMhz", "cpuFrequency"], "Current CPU frequency is unavailable.") },
    { label: "Maximum speed", value: cpu.maxFreqMhz === null ? null : formatFrequency(cpu.maxFreqMhz), reason: reasonFor(snapshot?.limits, ["cpu.maxFreqMhz", "cpuFrequency"], "Maximum CPU frequency is unavailable.") },
    { label: "Governor", value: cpu.governor, reason: reasonFor(snapshot?.limits, ["cpu.governor", "cpuFrequency"], "The CPU governor is unavailable.") },
    { label: "Processes", value: formatNumber(cpu.processes) },
    { label: "Threads", value: formatNumber(cpu.threads) },
    {
      label: "Open files",
      value: cpu.openFiles === null
        ? null
        : cpu.openFilesMax === null || cpu.openFilesMax > 1_000_000_000
          ? formatNumber(cpu.openFiles)
          : `${formatNumber(cpu.openFiles)} / ${formatNumber(cpu.openFilesMax)}`,
      reason: openFilesReason,
    },
    { label: "Uptime", value: formatDuration(cpu.uptimeSeconds) },
    { label: "Load average", value: cpu.loadAvg.map((value) => value.toFixed(2)).join("  /  "), note: "1 / 5 / 15 minutes" },
    { label: "Sockets · cores", value: `${formatNumber(cpu.sockets)} · ${formatNumber(cpu.physicalCores)}` },
    { label: "Logical processors", value: formatNumber(cpu.logicalCpus) },
    { label: "Virtualization", value: cpu.virtualization, reason: reasonFor(snapshot?.limits, ["cpu.virtualization", "virtualization"], "Virtualization support was not reported by the processor.") },
    { label: "L1d", value: caches.get("l1d") ?? null, reason: cacheReason },
    { label: "L2", value: caches.get("l2") ?? null, reason: cacheReason },
    { label: "L3", value: caches.get("l3") ?? null, reason: cacheReason },
    { label: "CPU pressure", value: cpu.pressure ? `${cpu.pressure.some10.toFixed(2)}% / ${cpu.pressure.some60.toFixed(2)}%` : null, note: "some avg10 / avg60", reason: reasonFor(snapshot?.limits, ["cpu.pressure", "pressure"], "CPU pressure data is unsupported by this kernel.") },
    { label: "Kernel time", value: formatPercent(cpu.system), note: `User ${formatPercent(cpu.user)} · I/O wait ${formatPercent(cpu.iowait)}` },
  ] : []);
  let drivers = $derived(topGroups(snapshot?.groups ?? [], (group) => group.cpu, formatPercent));
</script>

<section class="performance-panel" aria-labelledby="cpu-panel-title">
  <header class="performance-panel-head">
    <div>
      <span>Processor</span>
      <h2 id="cpu-panel-title">CPU</h2>
      <p>{cpu?.model ?? "Waiting for processor details…"}</p>
    </div>
    <strong>{cpu ? formatPercent(cpu.usage) : "—"}</strong>
  </header>

  <div class="perf-toolbar" aria-label="CPU graph options">
    <div class="perf-segmented">
      <button type="button" class:active={view === "overall"} onclick={() => view = "overall"}>Overall</button>
      <button type="button" class:active={view === "logical"} onclick={() => view = "logical"}>Logical processors</button>
    </div>
    <label class="perf-check"><input type="checkbox" bind:checked={showKernel} /><span>Kernel time</span></label>
  </div>

  {#if view === "overall"}
    <div class="perf-graph-block perf-graph-primary">
      <div class="perf-graph-label">
        <span>{showKernel ? "User / kernel utilization" : "Overall utilization"}</span>
        <small>60 samples · 100%</small>
      </div>
      <SparkGraph
        values={showKernel ? history.cpuUser : history.cpu}
        secondary={showKernel ? history.cpuSystem : undefined}
        max={100}
        tall
        label={showKernel ? "CPU user and kernel utilization history" : "Overall CPU utilization history"}
      />
      {#if showKernel}<div class="perf-legend"><span>User</span><span class="secondary">Kernel</span></div>{/if}
    </div>
  {:else}
    <div class="core-grid" aria-label="Logical processor utilization">
      {#each cpu?.cores ?? [] as core (core.index)}
        <div class:performance-core={core.kind === "performance"} class:efficiency-core={core.kind === "efficiency"}>
          <header><span>CPU {core.index}</span><b>{formatPercent(core.usage)}</b></header>
          <SparkGraph values={history.cores[core.index] ?? []} max={100} label={`Logical processor ${core.index} history`} />
          <small>{core.kind === "performance" ? "P-core" : core.kind === "efficiency" ? "E-core" : "Logical"}{core.freqMhz === null ? "" : ` · ${formatFrequency(core.freqMhz)}`}</small>
        </div>
      {:else}
        <p class="perf-empty">Logical processor readings are not available yet.</p>
      {/each}
    </div>
  {/if}

  <DetailGrid items={details} />
  <DrivingList metrics={drivers} {onOpenProcess} />
</section>
