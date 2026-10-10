<script lang="ts">
  import SparkGraph from "../../components/SparkGraph.svelte";
  import type { History, Snapshot } from "../../types";
  import DetailGrid from "./DetailGrid.svelte";
  import DrivingList from "./DrivingList.svelte";
  import { formatTemperature, reasonFor, type DetailItem } from "./format";

  let {
    snapshot,
    history,
    onOpenProcess,
  }: {
    snapshot: Snapshot | null;
    history: History;
    onOpenProcess: (key: string) => void;
  } = $props();

  let thermal = $derived(snapshot?.thermal ?? null);
  let hotspot = $derived(thermal?.hotspot ?? null);
  let hotspotReason = $derived(reasonFor(snapshot?.limits, ["thermal.hotspot", "thermals", "temperature"], "No supported hotspot sensor was found."));
  let details = $derived.by((): DetailItem[] => hotspot ? [
    { label: "Hotspot", value: formatTemperature(hotspot.temperature) },
    { label: "Sensor", value: `${hotspot.chip} · ${hotspot.label}` },
    { label: "Maximum", value: hotspot.max === null ? null : formatTemperature(hotspot.max), reason: reasonFor(snapshot?.limits, [`thermal.${hotspot.id}.max`, "thermal.max"], "This sensor does not publish a maximum temperature.") },
    { label: "Critical", value: hotspot.critical === null ? null : formatTemperature(hotspot.critical), reason: reasonFor(snapshot?.limits, [`thermal.${hotspot.id}.critical`, "thermal.critical"], "This sensor does not publish a critical temperature.") },
  ] : [
    { label: "Hotspot", value: null, reason: hotspotReason },
  ]);
</script>

<section class="performance-panel" aria-labelledby="thermal-panel-title">
  <header class="performance-panel-head">
    <div>
      <span>Hardware sensors</span>
      <h2 id="thermal-panel-title">Thermals</h2>
      <p>{thermal ? `${thermal.sensors.length} sensor${thermal.sensors.length === 1 ? "" : "s"} reporting` : "Waiting for sensor details…"}</p>
    </div>
    <strong class:metric-missing={hotspot === null} title={hotspot === null ? hotspotReason : undefined}>{hotspot ? formatTemperature(hotspot.temperature) : "—"}</strong>
  </header>

  <div class="perf-graph-block perf-graph-primary">
    <div class="perf-graph-label"><span>Hotspot</span><small class:metric-missing={hotspot === null} title={hotspot === null ? hotspotReason : undefined}>{hotspot ? formatTemperature(hotspot.temperature) : "— Unsupported"}</small></div>
    <SparkGraph values={hotspot ? history.thermals[hotspot.id] ?? [] : []} max="auto" tall label="Thermal hotspot history" />
  </div>

  <DetailGrid items={details} />

  <section class="sensor-table" aria-label="Temperature sensors">
    <header><span>Sensors</span><small>Current / max / critical</small></header>
    <div class="sensor-table-head"><span>Sensor</span><span>Current</span><span>Maximum</span><span>Critical</span></div>
    {#each thermal?.sensors ?? [] as sensor (sensor.id)}
      <div class="sensor-row">
        <span><b>{sensor.label}</b><small>{sensor.chip}</small></span>
        <strong>{formatTemperature(sensor.temperature)}</strong>
        <strong class:metric-missing={sensor.max === null} title={sensor.max === null ? reasonFor(snapshot?.limits, [`thermal.${sensor.id}.max`, "thermal.max"], "This sensor does not publish a maximum temperature.") : undefined}>{sensor.max === null ? "—" : formatTemperature(sensor.max)}</strong>
        <strong class:metric-missing={sensor.critical === null} title={sensor.critical === null ? reasonFor(snapshot?.limits, [`thermal.${sensor.id}.critical`, "thermal.critical"], "This sensor does not publish a critical temperature.") : undefined}>{sensor.critical === null ? "—" : formatTemperature(sensor.critical)}</strong>
      </div>
    {:else}
      <p class="perf-empty">No supported temperature sensors were found.</p>
    {/each}
  </section>

  <DrivingList metrics={[]} emptyReason="Temperature sensors cannot attribute heat to individual processes." {onOpenProcess} />
</section>
