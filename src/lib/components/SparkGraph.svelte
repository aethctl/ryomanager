<script lang="ts">
  export let values: number[] = [];
  export let max = 100;
  export let label = "";
  export let tall = false;

  $: ceiling = Math.max(1, max);
  $: points = values.length > 1
    ? values.map((value, index) => {
        const x = (index / (values.length - 1)) * 100;
        const y = 40 - (Math.min(ceiling, Math.max(0, value)) / ceiling) * 40;
        return `${x.toFixed(2)},${y.toFixed(2)}`;
      }).join(" ")
    : "0,40 100,40";
  $: area = `0,40 ${points} 100,40`;
</script>

<div class:tall class="graph" aria-label={label}>
  <svg viewBox="0 0 100 40" preserveAspectRatio="none" role="img">
    <line x1="0" y1="10" x2="100" y2="10" class="grid" />
    <line x1="0" y1="20" x2="100" y2="20" class="grid" />
    <line x1="0" y1="30" x2="100" y2="30" class="grid" />
    <polygon points={area} class="fill" />
    <polyline points={points} class="line" />
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
  .fill {
    fill: color-mix(in srgb, var(--ryo-ink) 5%, transparent);
  }
  .line {
    fill: none;
    stroke: var(--ryo-ink);
    stroke-width: .8;
    vector-effect: non-scaling-stroke;
    stroke-linejoin: round;
    stroke-linecap: round;
  }
</style>
