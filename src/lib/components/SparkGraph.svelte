<script lang="ts">
  import type { Series } from "../types";

  interface Props {
    values?: Series;
    secondary?: Series;
    max?: number | "auto";
    label?: string;
    tall?: boolean;
  }

  let {
    values = [],
    secondary = undefined,
    max = 100,
    label = "",
    tall = false,
  }: Props = $props();

  function finiteValues(series: Series | undefined) {
    return (series ?? []).filter((value): value is number => value !== null && Number.isFinite(value));
  }

  function lineSegments(series: Series, ceiling: number) {
    const segments: string[] = [];
    let current: string[] = [];
    const denominator = Math.max(1, series.length - 1);

    series.forEach((value, index) => {
      if (value === null || !Number.isFinite(value)) {
        if (current.length) segments.push(current.join(" "));
        current = [];
        return;
      }

      const x = (index / denominator) * 100;
      const y = 40 - (Math.min(ceiling, Math.max(0, value)) / ceiling) * 40;
      current.push(`${x.toFixed(2)},${y.toFixed(2)}`);
    });

    if (current.length) segments.push(current.join(" "));
    return segments;
  }

  let samples = $derived([...finiteValues(values), ...finiteValues(secondary)]);
  let ceiling = $derived(max === "auto" ? Math.max(1, ...samples) : Math.max(1, max));
  let primarySegments = $derived(lineSegments(values, ceiling));
  let secondarySegments = $derived(lineSegments(secondary ?? [], ceiling));
</script>

<div class:tall class="graph" aria-label={label}>
  <svg viewBox="0 0 100 40" preserveAspectRatio="none" role="img">
    <line x1="0" y1="10" x2="100" y2="10" class="grid" />
    <line x1="0" y1="20" x2="100" y2="20" class="grid" />
    <line x1="0" y1="30" x2="100" y2="30" class="grid" />
    {#each primarySegments as points}
      <polyline {points} class="line primary" />
    {/each}
    {#each secondarySegments as points}
      <polyline {points} class="line secondary" />
    {/each}
  </svg>
</div>

<style>
  .graph {
    width: 100%;
    height: 42px;
  }

  .graph.tall {
    height: 220px;
  }

  svg {
    display: block;
    width: 100%;
    height: 100%;
    overflow: visible;
  }

  .grid {
    stroke: var(--ryo-line-soft);
    stroke-width: .28;
    vector-effect: non-scaling-stroke;
  }

  .line {
    fill: none;
    stroke-width: .85;
    vector-effect: non-scaling-stroke;
    stroke-linejoin: miter;
    stroke-linecap: square;
  }

  .primary {
    stroke: var(--ryo-signal);
  }

  .secondary {
    stroke: color-mix(in srgb, var(--ryo-signal) 48%, var(--ryo-ink));
    stroke-dasharray: 2 1.5;
  }
</style>
