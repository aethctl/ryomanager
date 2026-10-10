<script lang="ts">
  import type { History, NetworkInfo, Snapshot } from "../types";
  import "../styles/performance.css";
  import CpuPanel from "./performance/CpuPanel.svelte";
  import DiskPanel from "./performance/DiskPanel.svelte";
  import EnergyPanel from "./performance/EnergyPanel.svelte";
  import GpuPanel from "./performance/GpuPanel.svelte";
  import MemoryPanel from "./performance/MemoryPanel.svelte";
  import NetworkPanel from "./performance/NetworkPanel.svelte";
  import Rail from "./performance/Rail.svelte";
  import ThermalPanel from "./performance/ThermalPanel.svelte";
  import { combineSeries, formatBytes, formatPercent, formatRate, formatTemperature, formatWatts, reasonFor, type RailItem } from "./performance/format";

  let {
    snapshot,
    history,
    onOpenProcess,
  }: {
    snapshot: Snapshot | null;
    history: History;
    onOpenProcess: (key: string) => void;
  } = $props();

  let selected = $state("cpu");

  const interfaceIsUp = (network: NetworkInfo) => {
    const state = network.state.toLowerCase();
    if (/(^|\s)(up|connected|activated|routable)(\s|$)/.test(state)) return true;
    return (network.kind === "virtual" || network.kind === "loopback")
      && !/(down|notpresent|lowerlayerdown)/.test(state);
  };

  let onlineNetworks = $derived(snapshot?.networks.filter(interfaceIsUp) ?? []);
  let directNetworks = $derived(onlineNetworks.filter((network) => network.kind !== "virtual" && network.kind !== "loopback"));
  let foldedNetworks = $derived(onlineNetworks.filter((network) => network.kind === "virtual" || network.kind === "loopback"));
  let memoryPercent = $derived(snapshot && snapshot.memory.total > 0 ? (snapshot.memory.used / snapshot.memory.total) * 100 : 0);
  let railItems = $derived.by((): RailItem[] => [
    {
      id: "cpu",
      label: "CPU",
      meta: snapshot?.cpu.model ?? "Processor",
      headline: snapshot ? formatPercent(snapshot.cpu.usage) : null,
      values: history.cpu,
      max: 100,
    },
    {
      id: "memory",
      label: "Memory",
      meta: snapshot ? `${formatBytes(snapshot.memory.used)} / ${formatBytes(snapshot.memory.total)}` : "Physical memory",
      headline: snapshot ? formatPercent(memoryPercent) : null,
      values: history.memory,
      max: 100,
    },
    ...(snapshot?.disks.map((disk) => ({
      id: `disk:${disk.name}`,
      label: disk.name,
      meta: disk.model ?? `${disk.kind} · ${formatBytes(disk.capacity)}`,
      headline: formatPercent(disk.activePercent),
      values: history.disks[disk.name]?.active ?? [],
      max: 100 as const,
    })) ?? []),
    ...directNetworks.map((network) => ({
      id: `network:${network.name}`,
      label: network.name,
      meta: network.kind === "wifi" ? network.ssid ?? "Wi-Fi" : network.kind === "ethernet" ? "Ethernet" : "Network",
      headline: formatRate(network.rxRate + network.txRate),
      values: history.networks[network.name]?.rx ?? [],
      secondary: history.networks[network.name]?.tx ?? [],
      max: "auto" as const,
    })),
    ...(foldedNetworks.length > 0 ? [{
      id: "network:other",
      label: "Other",
      meta: `${foldedNetworks.length} virtual / loopback`,
      headline: formatRate(foldedNetworks.reduce((total, network) => total + network.rxRate + network.txRate, 0)),
      values: combineSeries(foldedNetworks.map((network) => history.networks[network.name]?.rx ?? [])),
      secondary: combineSeries(foldedNetworks.map((network) => history.networks[network.name]?.tx ?? [])),
      max: "auto" as const,
    }] : []),
    ...(snapshot?.gpus.map((gpu) => ({
      id: `gpu:${gpu.index}`,
      label: `GPU ${gpu.index + 1}`,
      meta: gpu.name,
      headline: gpu.usage === null ? null : formatPercent(gpu.usage),
      reason: gpu.usage === null ? reasonFor(gpu.limits, ["usage", "gpu.usage"], "GPU utilization is unsupported by this driver.") : undefined,
      values: history.gpus[gpu.index]?.usage ?? [],
      max: 100 as const,
    })) ?? []),
    {
      id: "energy",
      label: "Energy",
      meta: snapshot?.energy.source === "ac" ? "AC power" : snapshot?.energy.source === "battery" ? "Battery" : "Power",
      headline: snapshot?.energy.batteryPower !== null && snapshot?.energy.batteryPower !== undefined
        ? formatWatts(snapshot.energy.batteryPower)
        : snapshot?.energy.batteryPercent !== null && snapshot?.energy.batteryPercent !== undefined
          ? formatPercent(snapshot.energy.batteryPercent)
          : null,
      reason: reasonFor(snapshot?.energy.limits, ["batteryPower", "battery"], "Battery power is unsupported by this system."),
      values: history.batteryPower,
      max: "auto",
    },
    {
      id: "thermals",
      label: "Thermals",
      meta: snapshot ? `${snapshot.thermal.sensors.length} sensors` : "Hardware sensors",
      headline: snapshot?.thermal.hotspot ? formatTemperature(snapshot.thermal.hotspot.temperature) : null,
      reason: reasonFor(snapshot?.limits, ["thermal.hotspot", "thermals"], "No supported hotspot sensor was found."),
      values: snapshot?.thermal.hotspot ? history.thermals[snapshot.thermal.hotspot.id] ?? [] : [],
      max: "auto",
    },
  ]);

  $effect(() => {
    if (!railItems.some((item) => item.id === selected)) {
      selected = railItems[0]?.id ?? "cpu";
    }
  });

  let selectedDisk = $derived(selected.startsWith("disk:") ? snapshot?.disks.find((disk) => `disk:${disk.name}` === selected) ?? null : null);
  let selectedGpu = $derived(selected.startsWith("gpu:") ? snapshot?.gpus.find((gpu) => `gpu:${gpu.index}` === selected) ?? null : null);
  let selectedNetworks = $derived(
    selected === "network:other"
      ? foldedNetworks
      : selected.startsWith("network:")
        ? directNetworks.filter((network) => `network:${network.name}` === selected)
        : [],
  );
</script>

<section class="page performance-page performance-workspace-page">
  <header class="page-head compact">
    <div class="title-block">
      <div class="section-label"><i></i><span class="section-mark">性能</span><span>PERFORMANCE</span></div>
      <h1>Performance</h1>
      <p>Live hardware readings with gaps left visible when collection pauses.</p>
    </div>
    <div class="performance-sample-meta">
      <span>Sample</span>
      <strong>{snapshot ? `${(snapshot.sampleMs / 1000).toFixed(1)} s` : "—"}</strong>
    </div>
  </header>

  <div class="performance-workspace">
    <Rail items={railItems} {selected} onSelect={(id) => selected = id} />
    <main class="performance-detail">
      {#if selected === "cpu"}
        <CpuPanel {snapshot} {history} {onOpenProcess} />
      {:else if selected === "memory"}
        <MemoryPanel {snapshot} {history} {onOpenProcess} />
      {:else if selectedDisk}
        <DiskPanel {snapshot} {history} disk={selectedDisk} {onOpenProcess} />
      {:else if selectedNetworks.length > 0}
        <NetworkPanel {snapshot} {history} networks={selectedNetworks} {onOpenProcess} />
      {:else if selectedGpu}
        <GpuPanel {snapshot} {history} gpu={selectedGpu} {onOpenProcess} />
      {:else if selected === "energy"}
        <EnergyPanel {snapshot} {history} {onOpenProcess} />
      {:else if selected === "thermals"}
        <ThermalPanel {snapshot} {history} {onOpenProcess} />
      {/if}
    </main>
  </div>
</section>
