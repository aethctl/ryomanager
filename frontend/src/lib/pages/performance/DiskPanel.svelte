<script lang="ts">
  import SparkGraph from "../../components/SparkGraph.svelte";
  import type { DiskInfo, History, Snapshot } from "../../types";
  import DetailGrid from "./DetailGrid.svelte";
  import DrivingList from "./DrivingList.svelte";
  import { formatBytes, formatNumber, formatPercent, formatRate, reasonFor, topGroups, type DetailItem } from "./format";

  let {
    snapshot,
    history,
    disk,
    onOpenProcess,
  }: {
    snapshot: Snapshot | null;
    history: History;
    disk: DiskInfo | null;
    onOpenProcess: (key: string) => void;
  } = $props();

  let diskHistory = $derived(disk ? history.disks[disk.name] : undefined);
  let details = $derived.by((): DetailItem[] => disk ? [
    { label: "Active time", value: formatPercent(disk.activePercent) },
    { label: "Response time", value: disk.responseMs === null ? null : `${disk.responseMs.toFixed(disk.responseMs >= 10 ? 0 : 1)} ms`, reason: reasonFor(snapshot?.limits, [`disk.${disk.name}.responseMs`, "disk.responseMs"], "This device does not report response time.") },
    { label: "Read speed", value: formatRate(disk.readRate) },
    { label: "Write speed", value: formatRate(disk.writeRate) },
    { label: "Read operations", value: `${formatNumber(disk.readIops, 1)} IOPS` },
    { label: "Write operations", value: `${formatNumber(disk.writeIops, 1)} IOPS` },
    { label: "Capacity", value: formatBytes(disk.capacity) },
    { label: "Type", value: disk.kind === "unknown" ? "Unknown" : disk.kind.toUpperCase() },
    { label: "Model", value: disk.model, wide: true, reason: reasonFor(snapshot?.limits, [`disk.${disk.name}.model`, "disk.model"], "The device model is unavailable.") },
    { label: "Total read", value: formatBytes(disk.readTotal) },
    { label: "Total written", value: formatBytes(disk.writeTotal) },
  ] : []);
  let drivers = $derived(topGroups(
    snapshot?.groups ?? [],
    (group) => group.diskRead === null && group.diskWrite === null ? null : (group.diskRead ?? 0) + (group.diskWrite ?? 0),
    formatRate,
  ));
</script>

<section class="performance-panel" aria-labelledby="disk-panel-title">
  <header class="performance-panel-head">
    <div>
      <span>Storage device</span>
      <h2 id="disk-panel-title">{disk?.name ?? "Disk"}</h2>
      <p>{disk?.model ?? (disk ? `${disk.kind} storage` : "Waiting for disk details…")}</p>
    </div>
    <strong>{disk ? formatPercent(disk.activePercent) : "—"}</strong>
  </header>

  <div class="perf-graph-block perf-graph-primary">
    <div class="perf-graph-label"><span>Active time</span><small>100%</small></div>
    <SparkGraph values={diskHistory?.active ?? []} max={100} tall label={`${disk?.name ?? "Disk"} active time history`} />
  </div>

  <div class="perf-graph-block perf-graph-secondary">
    <div class="perf-graph-label">
      <span>Transfer rate</span>
      <small>{disk ? `Read ${formatRate(disk.readRate)} · Write ${formatRate(disk.writeRate)}` : "—"}</small>
    </div>
    <SparkGraph values={diskHistory?.read ?? []} secondary={diskHistory?.write} max="auto" label={`${disk?.name ?? "Disk"} read and write history`} />
    <div class="perf-legend"><span>Read</span><span class="secondary">Write</span></div>
  </div>

  <DetailGrid items={details} />

  <section class="mount-table" aria-label="Mounted filesystems">
    <header><span>Volumes</span><small>{disk?.mounts.length ?? 0} mounted</small></header>
    <div class="mount-table-head"><span>Mount</span><span>Filesystem</span><span>Used</span><span>Capacity</span></div>
    {#each disk?.mounts ?? [] as mount (mount.path)}
      <div class="mount-row">
        <strong>{mount.path}</strong><span>{mount.fs}</span><span>{formatBytes(mount.used)}</span><span>{formatBytes(mount.total)}</span>
      </div>
    {:else}
      <p class="perf-empty">No mounted filesystems on this device.</p>
    {/each}
  </section>

  <DrivingList metrics={drivers} emptyReason="Disk activity cannot be attributed to readable process counters." {onOpenProcess} />
</section>
