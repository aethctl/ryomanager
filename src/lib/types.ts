export type ProcessInfo = {
  pid: number;
  parentPid: number | null;
  name: string;
  command: string;
  status: string;
  cpu: number;
  memory: number;
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
  cpu: number;
  totalMemory: number;
  usedMemory: number;
  totalSwap: number;
  usedSwap: number;
  processCount: number;
  processes: ProcessInfo[];
  system: SystemInfo;
};
