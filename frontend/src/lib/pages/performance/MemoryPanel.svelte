<script lang="ts">
  import SparkGraph from "../../components/SparkGraph.svelte";
  import type { History, Snapshot } from "../../types";
  import DetailGrid from "./DetailGrid.svelte";
  import DrivingList from "./DrivingList.svelte";
  import { formatBytes, formatPercent, reasonFor, topGroups, type DetailItem } from "./format";

  let {
    snapshot,
    history,
    onOpenProcess,
  }: {
    snapshot: Snapshot | null;
    history: History;
    onOpenProcess: (key: string) => void;
  } = $props();

  let memory = $derived(snapshot?.memory ?? null);
  let usedPercent = $derived(memory && memory.total > 0 ? (memory.used / memory.total) * 100 : 0);
  let freePercent = $derived(memory && memory.total > 0 ? (memory.free / memory.total) * 100 : 0);
  let cacheAvailable = $derived(memory ? Math.max(0, memory.available - memory.free) : 0);
  let cachePercent = $derived(memory && memory.total > 0 ? (cacheAvailable / memory.total) * 100 : 0);
  let swapPercent = $derived(memory && memory.swapTotal > 0 ? (memory.swapUsed / memory.swapTotal) * 100 : 0);
  let hardwareReason = $derived(reasonFor(memory?.limits, ["hardware", "dimm", "memoryHardware"], "Memory speed and slot details require root access."));
  let details = $derived.by((): DetailItem[] => memory ? [
    { label: "In use", value: formatBytes(memory.used) },
    { label: "Available", value: formatBytes(memory.available) },
    { label: "Committed", value: `${formatBytes(memory.committed)} / ${formatBytes(memory.commitLimit)}` },
    { label: "Cached", value: formatBytes(memory.cached) },
    { label: "Buffers", value: formatBytes(memory.buffers) },
    { label: "Shared", value: formatBytes(memory.shared) },
    { label: "Dirty", value: formatBytes(memory.dirty) },
    { label: "Mapped", value: formatBytes(memory.mapped) },
    { label: "Swap", value: memory.swapTotal > 0 ? `${formatBytes(memory.swapUsed)} / ${formatBytes(memory.swapTotal)}` : "Not configured" },
    { label: "Swap cache", value: formatBytes(memory.swapCached) },
    { label: "Zswap", value: memory.zswap === null ? null : formatBytes(memory.zswap), reason: reasonFor(memory.limits, ["zswap", "memory.zswap"], "Zswap is unsupported or not enabled on this system.") },
    { label: "Memory pressure", value: memory.pressure ? `${memory.pressure.some10.toFixed(2)}% / ${memory.pressure.some60.toFixed(2)}%` : null, note: "some avg10 / avg60", reason: reasonFor(memory.limits, ["pressure", "memory.pressure"], "Memory pressure data is unsupported by this kernel.") },
  ] : []);
  let drivers = $derived(topGroups(snapshot?.groups ?? [], (group) => group.memory, formatBytes));
</script>

<section class="performance-panel" aria-labelledby="memory-panel-title">
  <header class="performance-panel-head">
    <div>
      <span>Physical memory</span>
      <h2 id="memory-panel-title">Memory</h2>
      <p>{memory ? `${formatBytes(memory.total)} installed` : "Waiting for memory details…"}</p>
    </div>
    <strong>{memory ? formatPercent(usedPercent) : "—"}</strong>
  </header>

  <div class="perf-graph-block perf-graph-primary">
    <div class="perf-graph-label"><span>Memory use</span><small>{memory ? formatBytes(memory.total) : "—"}</small></div>
    <SparkGraph values={history.memory} max={100} tall label="Physical memory use history" />
  </div>

  <section class="composition-block" aria-label="Memory composition">
    <header><span>Memory composition</span><small>{memory ? formatBytes(memory.available) : "—"} available</small></header>
    <div class="composition-bar" role="img" aria-label="In-use, reclaimable cache, and free memory">
      <i class="memory-used" style={`width:${Math.min(100, Math.max(0, usedPercent))}%`}></i>
      <i class="memory-cache" style={`width:${Math.min(100, Math.max(0, cachePercent))}%`}></i>
      <i class="memory-free" style={`width:${Math.min(100, Math.max(0, freePercent))}%`}></i>
    </div>
    <div class="composition-legend">
      <span><i class="memory-used"></i>In use <b>{memory ? formatBytes(memory.used) : "—"}</b></span>
      <span><i class="memory-cache"></i>Buffers / cache <b>{memory ? formatBytes(memory.buffers + memory.cached) : "—"}</b></span>
      <span><i class="memory-free"></i>Free <b>{memory ? formatBytes(memory.free) : "—"}</b></span>
      <span>Available <b>{memory ? formatBytes(memory.available) : "—"}</b></span>
    </div>
  </section>

  <section class="swap-block" aria-label="Swap use">
    <header><span>Swap</span><small>{memory && memory.swapTotal > 0 ? `${formatBytes(memory.swapUsed)} / ${formatBytes(memory.swapTotal)}` : "Not configured"}</small></header>
    <div class="meter"><i style={`width:${Math.min(100, Math.max(0, swapPercent))}%`}></i></div>
    <SparkGraph values={history.swap} max={100} label="Swap use history" />
  </section>

  <p class="perf-limit-note" title={hardwareReason}>{hardwareReason}</p>
  <DetailGrid items={details} />
  <DrivingList metrics={drivers} {onOpenProcess} />
</section>
