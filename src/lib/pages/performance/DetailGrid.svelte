<script lang="ts">
  import type { DetailItem } from "./format";

  let { items = [] }: { items?: DetailItem[] } = $props();
</script>

<div class="detail-grid perf-detail-grid">
  {#each items as item}
    <div class:wide={item.wide}>
      <span>{item.label}</span>
      <strong class:metric-missing={item.value === null} title={item.value === null ? item.reason : undefined}>
        {#if item.value === null}
          <b>—</b><small>Unsupported</small>
        {:else}
          <b>{item.value}</b>
        {/if}
      </strong>
      {#if item.note}<small class="detail-note">{item.note}</small>{/if}
    </div>
  {/each}
</div>
