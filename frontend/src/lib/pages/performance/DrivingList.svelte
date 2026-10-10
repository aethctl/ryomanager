<script lang="ts">
  import type { DrivingMetric } from "./format";

  let {
    title = "Driving it",
    metrics = [],
    emptyReason = "No process-level attribution is available.",
    onOpenProcess,
  }: {
    title?: string;
    metrics?: DrivingMetric[];
    emptyReason?: string;
    onOpenProcess: (key: string) => void;
  } = $props();

  let peak = $derived(Math.max(1, ...metrics.map((entry) => entry.value)));
</script>

<section class="perf-drivers" aria-label={title}>
  <header>
    <div>
      <span>{title}</span>
      <small>Top process groups</small>
    </div>
    <b>{metrics.length}</b>
  </header>
  {#if metrics.length > 0}
    <div class="perf-driver-list">
      {#each metrics as entry, index (entry.group.key)}
        <button type="button" class="perf-driver-row" onclick={() => onOpenProcess(entry.group.key)}>
          <span class="driver-rank">{String(index + 1).padStart(2, "0")}</span>
          <span class="perf-driver-name">
            <b>{entry.group.name}</b>
            <small>{entry.group.subtitle}</small>
          </span>
          <i aria-hidden="true" style={`--leader:${Math.max(2, (entry.value / peak) * 100)}%`}></i>
          <strong>{entry.label}</strong>
        </button>
      {/each}
    </div>
  {:else}
    <p class="perf-driver-empty">{emptyReason}</p>
  {/if}
</section>
