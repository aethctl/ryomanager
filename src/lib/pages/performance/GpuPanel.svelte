<script lang="ts">
  import SparkGraph from "../../components/SparkGraph.svelte";
  import type { GpuInfo, History, Snapshot } from "../../types";
  import DetailGrid from "./DetailGrid.svelte";
  import DrivingList from "./DrivingList.svelte";
  import { formatBytes, formatFrequency, formatGpuName, formatPercent, formatTemperature, formatWatts, reasonFor, topGroups, type DetailItem } from "./format";

  let {
    snapshot,
    history,
    gpu,
    onOpenProcess,
  }: {
    snapshot: Snapshot | null;
    history: History;
    gpu: GpuInfo | null;
    onOpenProcess: (key: string) => void;
  } = $props();

  const unsupported = (field: string, fallback: string) => reasonFor(gpu?.limits, [field, `gpu.${field}`], fallback);

  let gpuHistory = $derived(gpu ? history.gpus[gpu.index] : undefined);
  let gpuDisplayName = $derived(gpu ? formatGpuName(gpu.name) : null);
  let details = $derived.by((): DetailItem[] => gpu ? [
    { label: "Name", value: gpuDisplayName, wide: true },
    { label: "Vendor", value: gpu.vendor === "unknown" ? "Unknown" : gpu.vendor.toUpperCase() },
    { label: "Driver", value: gpu.driver, reason: unsupported("driver", "The graphics driver version is unavailable.") },
    { label: "VRAM", value: gpu.memoryUsed === null || gpu.memoryTotal === null ? null : `${formatBytes(gpu.memoryUsed)} / ${formatBytes(gpu.memoryTotal)}`, reason: unsupported("memory", "Dedicated graphics memory is unsupported on this GPU.") },
    { label: "Temperature", value: gpu.temperature === null ? null : formatTemperature(gpu.temperature), reason: unsupported("temperature", "The GPU does not expose a temperature reading.") },
    { label: "Power", value: gpu.power === null ? null : gpu.powerLimit === null ? formatWatts(gpu.power) : `${formatWatts(gpu.power)} / ${formatWatts(gpu.powerLimit)}`, reason: unsupported("power", "The GPU does not expose board power.") },
    { label: "Graphics clock", value: gpu.clockMhz === null ? null : formatFrequency(gpu.clockMhz), reason: unsupported("clock", "The graphics clock is unsupported.") },
    { label: "Memory clock", value: gpu.memoryClockMhz === null ? null : formatFrequency(gpu.memoryClockMhz), reason: unsupported("memoryClock", "The memory clock is unsupported.") },
    { label: "Performance state", value: gpu.pstate, reason: unsupported("pstate", "The GPU performance state is unsupported.") },
    { label: "Encoder", value: gpu.encoder === null ? null : formatPercent(gpu.encoder), reason: unsupported("encoder", "Encoder utilization is unsupported by this driver.") },
    { label: "Decoder", value: gpu.decoder === null ? null : formatPercent(gpu.decoder), reason: unsupported("decoder", "Decoder utilization is unsupported by this driver.") },
  ] : []);
  let drivers = $derived(topGroups(snapshot?.groups ?? [], (group) => {
    if (!gpu) return null;
    const readings = group.members
      .filter((member) => member.gpuIndex === gpu.index && member.gpu !== null)
      .map((member) => member.gpu as number);
    return readings.length > 0 ? readings.reduce((total, usage) => total + usage, 0) : null;
  }, formatPercent));
</script>

<section class="performance-panel" aria-labelledby="gpu-panel-title">
  <header class="performance-panel-head">
    <div>
      <span>Graphics processor {gpu ? gpu.index + 1 : ""}</span>
      <h2 id="gpu-panel-title">GPU</h2>
      <p>{gpuDisplayName ?? "Waiting for graphics details…"}</p>
    </div>
    <strong class:metric-missing={gpu?.usage === null} title={gpu?.usage === null ? unsupported("usage", "GPU utilization is unsupported by this driver.") : undefined}>
      {gpu?.usage === null || gpu === null ? "—" : formatPercent(gpu.usage)}
    </strong>
  </header>

  <div class="perf-graph-block perf-graph-primary">
    <div class="perf-graph-label"><span>3D utilization</span><small class:metric-missing={gpu?.usage === null} title={gpu?.usage === null ? unsupported("usage", "GPU utilization is unsupported by this driver.") : undefined}>{gpu?.usage === null || gpu === null ? "— Unsupported" : formatPercent(gpu.usage)}</small></div>
    <SparkGraph values={gpuHistory?.usage ?? []} max={100} tall label={`${gpuDisplayName ?? "GPU"} utilization history`} />
  </div>

  <div class="performance-dual-graphs">
    <div class="perf-graph-block perf-graph-secondary">
      <div class="perf-graph-label"><span>Encoder / decoder</span><small class:metric-missing={gpu?.encoder === null && gpu?.decoder === null} title={gpu?.encoder === null && gpu?.decoder === null ? unsupported("encoder", "Encoder and decoder utilization are unsupported by this driver.") : undefined}>{gpu?.encoder === null && gpu?.decoder === null ? "— Unsupported" : "100%"}</small></div>
      <SparkGraph values={gpuHistory?.encoder ?? []} secondary={gpuHistory?.decoder} max={100} label={`${gpuDisplayName ?? "GPU"} encoder and decoder history`} />
      <div class="perf-legend"><span>Encoder</span><span class="secondary">Decoder</span></div>
    </div>
    <div class="perf-graph-block perf-graph-secondary">
      <div class="perf-graph-label"><span>GPU memory</span><small class:metric-missing={gpu?.memoryUsage === null} title={gpu?.memoryUsage === null ? unsupported("memoryUsage", "GPU memory utilization is unsupported by this driver.") : undefined}>{gpu?.memoryUsage === null || gpu === null ? "— Unsupported" : formatPercent(gpu.memoryUsage)}</small></div>
      <SparkGraph values={gpuHistory?.memory ?? []} max={100} label={`${gpuDisplayName ?? "GPU"} memory history`} />
    </div>
  </div>

  <DetailGrid items={details} />
  <DrivingList metrics={drivers} emptyReason="Per-process GPU engine data is unavailable." {onOpenProcess} />
</section>
