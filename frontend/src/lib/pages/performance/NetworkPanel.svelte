<script lang="ts">
  import SparkGraph from "../../components/SparkGraph.svelte";
  import type { History, NetworkInfo, Snapshot } from "../../types";
  import DetailGrid from "./DetailGrid.svelte";
  import DrivingList from "./DrivingList.svelte";
  import { combineSeries, formatBytes, formatList, formatNumber, formatPercent, formatRate, reasonFor, type DetailItem } from "./format";

  let {
    snapshot,
    history,
    networks = [],
    onOpenProcess,
  }: {
    snapshot: Snapshot | null;
    history: History;
    networks?: NetworkInfo[];
    onOpenProcess: (key: string) => void;
  } = $props();

  let grouped = $derived(networks.length > 1);
  let primary = $derived(networks[0] ?? null);
  let label = $derived(grouped ? "Other" : primary?.name ?? "Network");
  let receiveRate = $derived(networks.reduce((total, network) => total + network.rxRate, 0));
  let sendRate = $derived(networks.reduce((total, network) => total + network.txRate, 0));
  let receiveTotal = $derived(networks.reduce((total, network) => total + network.rxTotal, 0));
  let sendTotal = $derived(networks.reduce((total, network) => total + network.txTotal, 0));
  let receiveHistory = $derived(combineSeries(networks.map((network) => history.networks[network.name]?.rx ?? [])));
  let sendHistory = $derived(combineSeries(networks.map((network) => history.networks[network.name]?.tx ?? [])));
  let ipv4 = $derived(networks.flatMap((network) => network.ipv4));
  let ipv6 = $derived(networks.flatMap((network) => network.ipv6));
  let details = $derived.by((): DetailItem[] => primary ? [
    { label: grouped ? "Adapters" : "Adapter", value: networks.map((network) => network.name).join(", ") },
    { label: "Type", value: grouped ? "Virtual / loopback" : primary.kind === "unknown" ? "Unknown" : primary.kind === "wifi" ? "Wi-Fi" : primary.kind[0].toUpperCase() + primary.kind.slice(1) },
    { label: "State", value: grouped ? [...new Set(networks.map((network) => network.state))].join(", ") : primary.state },
    { label: "Link speed", value: grouped ? "Multiple adapters" : primary.speedMbps === null ? null : `${formatNumber(primary.speedMbps)} Mbps`, reason: reasonFor(snapshot?.limits, [`network.${primary.name}.speed`, "network.speed"], "Link speed is unavailable for this adapter.") },
    { label: "IPv4", value: formatList(ipv4), wide: true },
    { label: "IPv6", value: formatList(ipv6), wide: true },
    { label: "MAC address", value: grouped ? "Multiple adapters" : primary.mac, reason: reasonFor(snapshot?.limits, [`network.${primary.name}.mac`, "network.mac"], "The adapter MAC address is unavailable.") },
    { label: "MTU", value: grouped ? "Multiple adapters" : primary.mtu === null ? null : formatNumber(primary.mtu), reason: reasonFor(snapshot?.limits, [`network.${primary.name}.mtu`, "network.mtu"], "The adapter MTU is unavailable.") },
    { label: "Driver", value: grouped ? [...new Set(networks.map((network) => network.driver).filter((driver): driver is string => driver !== null))].join(", ") || null : primary.driver, reason: reasonFor(snapshot?.limits, [`network.${primary.name}.driver`, "network.driver"], "The network driver is unavailable.") },
    { label: "Received", value: formatRate(receiveRate), note: `${formatRate(sendRate)} sent` },
    { label: "Totals", value: `${formatBytes(receiveTotal)} received`, note: `${formatBytes(sendTotal)} sent` },
  ] : []);
  let wifiDetails = $derived.by((): DetailItem[] => !grouped && primary?.kind === "wifi" ? [
    { label: "Wi-Fi network", value: primary.ssid, reason: reasonFor(snapshot?.limits, [`network.${primary.name}.ssid`, "network.wifi"], "Wi-Fi network details are unavailable.") },
    { label: "Signal", value: primary.signal === null ? null : formatPercent(primary.signal), reason: reasonFor(snapshot?.limits, [`network.${primary.name}.signal`, "network.wifi"], "Wi-Fi signal strength is unavailable.") },
    { label: "Frequency", value: primary.frequencyMhz === null ? null : `${formatNumber(primary.frequencyMhz)} MHz`, reason: reasonFor(snapshot?.limits, [`network.${primary.name}.frequency`, "network.wifi"], "Wi-Fi frequency is unavailable.") },
  ] : []);
</script>

<section class="performance-panel" aria-labelledby="network-panel-title">
  <header class="performance-panel-head">
    <div>
      <span>Network adapter</span>
      <h2 id="network-panel-title">{label}</h2>
      <p>{grouped ? `${networks.length} virtual and loopback interfaces` : primary ? `${primary.kind} · ${primary.state}` : "Waiting for network details…"}</p>
    </div>
    <strong>{primary ? formatRate(receiveRate + sendRate) : "—"}</strong>
  </header>

  <div class="perf-graph-block perf-graph-primary">
    <div class="perf-graph-label"><span>Throughput</span><small>Receive {formatRate(receiveRate)} · Send {formatRate(sendRate)}</small></div>
    <SparkGraph values={receiveHistory} secondary={sendHistory} max="auto" tall label={`${label} receive and send history`} />
    <div class="perf-legend"><span>Receive</span><span class="secondary">Send</span></div>
  </div>

  {#if wifiDetails.length > 0}<DetailGrid items={wifiDetails} />{/if}
  <DetailGrid items={details} />
  <DrivingList metrics={[]} emptyReason="Linux does not expose per-process network byte counters." {onOpenProcess} />
</section>
