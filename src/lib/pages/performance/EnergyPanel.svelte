<script lang="ts">
  import SparkGraph from "../../components/SparkGraph.svelte";
  import type { History, Snapshot } from "../../types";
  import DetailGrid from "./DetailGrid.svelte";
  import DrivingList from "./DrivingList.svelte";
  import { formatDuration, formatPercent, formatWatts, reasonFor, topGroups, type DetailItem } from "./format";

  let {
    snapshot,
    history,
    onOpenProcess,
  }: {
    snapshot: Snapshot | null;
    history: History;
    onOpenProcess: (key: string) => void;
  } = $props();

  let energy = $derived(snapshot?.energy ?? null);
  let batteryReason = $derived(reasonFor(energy?.limits, ["batteryPower", "battery.power"], energy?.source === "ac" ? "Battery draw is measured only while discharging." : "Battery power is unsupported by this system."));
  let packageReason = $derived(reasonFor(energy?.limits, ["packagePower", "powercap", "energy.package"], "Power access restricted. Linux protects CPU package energy counters on this system."));
  let health = $derived(
    energy?.batteryEnergyFull !== null && energy?.batteryEnergyFull !== undefined
      && energy.batteryEnergyDesign !== null && energy.batteryEnergyDesign !== undefined
      && energy.batteryEnergyDesign > 0
      ? (energy.batteryEnergyFull / energy.batteryEnergyDesign) * 100
      : null,
  );
  let details = $derived.by((): DetailItem[] => energy ? [
    { label: "Power source", value: energy.source === "ac" ? "AC power" : energy.source === "battery" ? "Battery" : "Unknown" },
    { label: "Battery", value: energy.batteryPercent === null ? null : formatPercent(energy.batteryPercent), reason: reasonFor(energy.limits, ["batteryPercent", "battery"], "No supported battery was found.") },
    { label: "Status", value: energy.batteryStatus, reason: reasonFor(energy.limits, ["batteryStatus", "battery"], "Battery status is unavailable.") },
    { label: "Battery draw", value: energy.batteryPower === null ? null : formatWatts(energy.batteryPower), reason: batteryReason },
    { label: "Time remaining", value: energy.timeToEmptySeconds === null ? null : formatDuration(energy.timeToEmptySeconds), reason: reasonFor(energy.limits, ["timeToEmpty", "battery.time"], "Time remaining is unavailable until the battery reports a stable discharge rate.") },
    { label: "Battery health", value: health === null ? null : formatPercent(health), note: energy.batteryEnergyFull === null || energy.batteryEnergyDesign === null ? undefined : `${energy.batteryEnergyFull.toFixed(1)} / ${energy.batteryEnergyDesign.toFixed(1)} Wh`, reason: reasonFor(energy.limits, ["batteryHealth", "battery.energy"], "Battery full and design capacities are unavailable.") },
    { label: "Cycles", value: energy.cycleCount === null ? null : String(energy.cycleCount), reason: reasonFor(energy.limits, ["cycleCount", "battery.cycles"], "This battery does not expose its cycle count.") },
    { label: "Package power", value: energy.packagePower === null ? null : formatWatts(energy.packagePower), reason: packageReason },
  ] : []);
  let drivers = $derived(topGroups(snapshot?.groups ?? [], (group) => group.energyScore, (value) => value === 0 ? "Idle" : `${value.toFixed(1)} impact`));
</script>

<section class="performance-panel" aria-labelledby="energy-panel-title">
  <header class="performance-panel-head">
    <div>
      <span>Power and battery</span>
      <h2 id="energy-panel-title">Energy</h2>
      <p>{energy ? `${energy.source === "ac" ? "AC power" : energy.source === "battery" ? "On battery" : "Power source unknown"}${energy.batteryStatus ? ` · ${energy.batteryStatus}` : ""}` : "Waiting for power details…"}</p>
    </div>
    <strong class:metric-missing={energy?.batteryPower === null} title={energy?.batteryPower === null ? batteryReason : undefined}>
      {energy?.batteryPower === null || energy === null ? "—" : formatWatts(energy.batteryPower)}
    </strong>
  </header>

  <div class="perf-graph-block perf-graph-primary">
    <div class="perf-graph-label"><span>Battery draw</span><small class:metric-missing={energy?.batteryPower === null} title={energy?.batteryPower === null ? batteryReason : undefined}>{energy?.batteryPower === null || energy === null ? "— Unsupported" : formatWatts(energy.batteryPower)}</small></div>
    <SparkGraph values={history.batteryPower} max="auto" tall label="Battery power history" />
  </div>

  {#if energy?.packagePower === null}
    <aside class="access-note" title={packageReason}>
      <span>Power access restricted</span>
      <p>{packageReason}</p>
    </aside>
  {/if}

  <DetailGrid items={details} />
  <p class="perf-limit-note">Process energy impact is an estimate from CPU, GPU, and disk activity. It is not measured in watts.</p>
  <DrivingList title="Energy impact" metrics={drivers} {onOpenProcess} />
</section>
