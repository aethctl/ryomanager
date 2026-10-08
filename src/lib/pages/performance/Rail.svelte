<script lang="ts">
  import SparkGraph from "../../components/SparkGraph.svelte";
  import { formatGpuName, type RailItem } from "./format";

  let {
    items = [],
    selected = "cpu",
    onSelect,
  }: {
    items?: RailItem[];
    selected?: string;
    onSelect: (id: string) => void;
  } = $props();
</script>

<nav class="performance-rail" aria-label="Performance resources">
  <div class="performance-rail-head">
    <span>Resources</span>
    <small>60 samples</small>
  </div>
  <div class="performance-rail-list">
    {#each items as item (item.id)}
      <button
        type="button"
        class:active={selected === item.id}
        aria-current={selected === item.id ? "page" : undefined}
        onclick={() => onSelect(item.id)}
      >
        <span class="rail-copy">
          <b>{item.label}</b>
          <small>{item.id.startsWith("gpu:") ? formatGpuName(item.meta) : item.meta}</small>
        </span>
        <span class="rail-mini" aria-hidden="true">
          <SparkGraph values={item.values} secondary={item.secondary} max={item.max} />
        </span>
        <strong class:metric-missing={item.headline === null} title={item.headline === null ? item.reason : undefined}>
          {item.headline ?? "—"}
        </strong>
      </button>
    {/each}
  </div>
</nav>
