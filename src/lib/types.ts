// The wire contract between the Rust collector (src-tauri/src) and the pages.
//
// Readings that the system can refuse to give (another user's I/O counters,
// the root-only energy counters, a GPU the driver cannot account for) are
// `number | null`. Null means "not measured" and renders as a dash with the
// reason from the nearest `limits` map; it is never written as zero, and zero
// always means the counter read zero.

export type Availability = Record<string, string>;

export type Category = "apps" | "background" | "system";

export type ProcessState =
  | "running"
  | "sleeping"
  | "waiting"
  | "stopped"
  | "zombie"
  | "idle"
  | "dead"
  | "unknown";

export type EnergyBand = "none" | "very-low" | "low" | "moderate" | "high" | "very-high";

// "private" is resident memory minus file-backed shared pages (statm), the
// cheap per-sample figure that does not count a shared library once per
// process; "pss" is the exact proportional figure, read on demand for one
// process; "rss" when nothing better was readable.
export type MemoryKind = "private" | "pss" | "rss" | "mixed";

export type WindowRef = {
  id: string;
  title: string;
  appId: string;
  workspace: string;
  output: string;
  focused: boolean;
};

export type ProcessInfo = {
  pid: number;
  parentPid: number | null;
  // Seconds since the epoch. With the pid it is the process identity: every
  // action sends both, and the backend refuses when the pid has been reused.
  startTime: number;
  name: string;
  exe: string | null;
  command: string;
  cwd: string | null;
  user: string;
  uid: number;
  state: ProcessState;
  kernelThread: boolean;
  unit: string | null;
  // Percent of the whole machine: every core busy is 100.
  cpu: number;
  cpuUser: number;
  cpuSystem: number;
  // Bytes. PSS when the kernel lets us read it, otherwise RSS.
  memory: number;
  memoryKind: MemoryKind;
  rss: number;
  rssAnon: number | null;
  rssFile: number | null;
  rssShmem: number | null;
  swap: number | null;
  virtual: number;
  // Bytes per second over the last sample.
  diskRead: number | null;
  diskWrite: number | null;
  diskReadTotal: number | null;
  diskWriteTotal: number | null;
  // Percent of one GPU's engine time, and bytes of its memory.
  gpu: number | null;
  gpuMemory: number | null;
  gpuIndex: number | null;
  // Open TCP and UDP sockets. Linux keeps no per-process byte counters.
  connections: number | null;
  // Estimated from CPU, GPU and disk activity; never a measured wattage.
  energy: EnergyBand;
  energyScore: number;
  threads: number;
  fds: number | null;
  nice: number;
  priority: number;
  ctxSwitches: number | null;
  oomScore: number | null;
  lastCpu: number | null;
  windows: WindowRef[];
};

export type ProcessGroup = {
  // Stable across samples: "app:<desktop id>", "unit:<systemd unit>",
  // "exe:<path>", "name:<comm>" or "kernel".
  key: string;
  category: Category;
  name: string;
  subtitle: string;
  // Pass to app_icon() for a data URL; null when no icon applies.
  iconKey: string | null;
  unit: string | null;
  leaderPid: number;
  // Separate launches merged under one identity.
  instances: number;
  windows: WindowRef[];
  state: ProcessState;
  cpu: number;
  memory: number;
  memoryKind: MemoryKind;
  diskRead: number | null;
  diskWrite: number | null;
  gpu: number | null;
  gpuMemory: number | null;
  connections: number | null;
  energy: EnergyBand;
  energyScore: number;
  threads: number;
  members: ProcessInfo[];
};

export type Pressure = {
  some10: number;
  some60: number;
  some300: number;
  full10: number | null;
  full60: number | null;
  full300: number | null;
};

export type CpuCore = {
  index: number;
  usage: number;
  freqMhz: number | null;
  kind: "performance" | "efficiency" | null;
};

export type CpuCache = { level: string; size: string };

export type CpuInfo = {
  usage: number;
  user: number;
  system: number;
  iowait: number;
  irq: number;
  cores: CpuCore[];
  freqMhz: number | null;
  maxFreqMhz: number | null;
  governor: string | null;
  loadAvg: [number, number, number];
  uptimeSeconds: number;
  processes: number;
  threads: number;
  openFiles: number | null;
  openFilesMax: number | null;
  model: string;
  sockets: number;
  physicalCores: number;
  logicalCpus: number;
  virtualization: string | null;
  caches: CpuCache[];
  pressure: Pressure | null;
};

export type MemoryInfo = {
  total: number;
  used: number;
  available: number;
  free: number;
  buffers: number;
  cached: number;
  shared: number;
  dirty: number;
  mapped: number;
  committed: number;
  commitLimit: number;
  swapTotal: number;
  swapUsed: number;
  swapCached: number;
  zswap: number | null;
  pressure: Pressure | null;
  limits: Availability;
};

export type DiskMount = {
  path: string;
  fs: string;
  used: number;
  total: number;
  system: boolean;
};

export type DiskInfo = {
  name: string;
  model: string | null;
  kind: "nvme" | "ssd" | "hdd" | "removable" | "virtual" | "unknown";
  capacity: number;
  readRate: number;
  writeRate: number;
  readIops: number;
  writeIops: number;
  activePercent: number;
  responseMs: number | null;
  readTotal: number;
  writeTotal: number;
  mounts: DiskMount[];
};

export type NetworkInfo = {
  name: string;
  kind: "wifi" | "ethernet" | "virtual" | "loopback" | "unknown";
  state: string;
  rxRate: number;
  txRate: number;
  rxTotal: number;
  txTotal: number;
  ipv4: string[];
  ipv6: string[];
  mac: string | null;
  mtu: number | null;
  speedMbps: number | null;
  ssid: string | null;
  signal: number | null;
  frequencyMhz: number | null;
  driver: string | null;
};

export type GpuInfo = {
  index: number;
  name: string;
  vendor: "nvidia" | "amd" | "intel" | "unknown";
  usage: number | null;
  memoryUsage: number | null;
  encoder: number | null;
  decoder: number | null;
  memoryUsed: number | null;
  memoryTotal: number | null;
  temperature: number | null;
  power: number | null;
  powerLimit: number | null;
  clockMhz: number | null;
  memoryClockMhz: number | null;
  driver: string | null;
  pstate: string | null;
  limits: Availability;
};

export type EnergyInfo = {
  source: "battery" | "ac" | "unknown";
  batteryPercent: number | null;
  batteryStatus: string | null;
  // Watts drawn from the battery while discharging.
  batteryPower: number | null;
  batteryEnergyNow: number | null;
  batteryEnergyFull: number | null;
  batteryEnergyDesign: number | null;
  timeToEmptySeconds: number | null;
  cycleCount: number | null;
  // Watts from the CPU package counters; root-only on current kernels.
  packagePower: number | null;
  limits: Availability;
};

export type ThermalSensor = {
  id: string;
  chip: string;
  label: string;
  temperature: number;
  max: number | null;
  critical: number | null;
};

export type ThermalInfo = {
  hotspot: ThermalSensor | null;
  sensors: ThermalSensor[];
};

export type SystemInfo = {
  hostname: string;
  os: string;
  kernel: string;
  cpuModel: string;
  logicalCpus: number;
  physicalCores: number;
};

export type Snapshot = {
  timestampMs: number;
  // Milliseconds between this sample and the previous one; rates use it.
  sampleMs: number;
  accent: string;
  // "shell" when window ownership comes from the Ryoku shell socket.
  windowsSource: "shell" | "none";
  selfPid: number;
  cpu: CpuInfo;
  memory: MemoryInfo;
  disks: DiskInfo[];
  networks: NetworkInfo[];
  gpus: GpuInfo[];
  energy: EnergyInfo;
  thermal: ThermalInfo;
  processCount: number;
  groups: ProcessGroup[];
  system: SystemInfo;
  limits: Availability;
};

// Commands. Every action on a process carries its identity.
export type ProcessRef = { pid: number; startTime: number };
export type EndProcessArgs = ProcessRef & { force: boolean };
export type EndGroupArgs = { key: string; force: boolean };
export type SignalProcessArgs = ProcessRef & { action: "suspend" | "resume" };
export type SetPriorityArgs = ProcessRef & { nice: number };
// The expensive readings for one process (smaps_rollup, status, fd count),
// fetched for the inspected process only; a ProcessInfo with those fields
// filled and memory/memoryKind as PSS.
export type ProcessDetailArgs = ProcessRef;

// History the shell keeps for the pages, one ring buffer per series. A null
// entry is a skipped sample (the window was hidden) and draws as a gap.
export type Series = (number | null)[];

export type History = {
  cpu: Series;
  cpuUser: Series;
  cpuSystem: Series;
  cores: Series[];
  memory: Series;
  swap: Series;
  disks: Record<string, { active: Series; read: Series; write: Series }>;
  networks: Record<string, { rx: Series; tx: Series }>;
  gpus: Record<number, { usage: Series; memory: Series; encoder: Series; decoder: Series }>;
  batteryPower: Series;
  thermals: Record<string, Series>;
  // CPU history for the selected group and the selected member only.
  selectedGroup: Series;
  selectedProcess: Series;
};
