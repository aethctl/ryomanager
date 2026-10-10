import type { Availability, ProcessGroup, Series } from "../../types";

export type DetailItem = {
  label: string;
  value: string | null;
  reason?: string;
  note?: string;
  wide?: boolean;
};

export type DrivingMetric = {
  group: ProcessGroup;
  value: number;
  label: string;
};

export type RailItem = {
  id: string;
  label: string;
  meta: string;
  headline: string | null;
  reason?: string;
  values: Series;
  secondary?: Series;
  max: number | "auto";
};

export function formatNumber(value: number, digits = 0): string {
  return new Intl.NumberFormat(undefined, { maximumFractionDigits: digits }).format(value);
}

export function formatPercent(value: number): string {
  return `${value.toFixed(value >= 10 ? 0 : 1)}%`;
}

export function formatBytes(bytes: number): string {
  const units = ["B", "KB", "MB", "GB", "TB", "PB"];
  let value = Math.max(0, bytes);
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  const digits = value >= 10 || unit < 2 ? 0 : 1;
  return `${value.toFixed(digits)} ${units[unit]}`;
}

export function formatRate(bytes: number): string {
  return `${formatBytes(bytes)}/s`;
}

export function formatFrequency(mhz: number): string {
  return mhz >= 1000 ? `${(mhz / 1000).toFixed(2)} GHz` : `${Math.round(mhz)} MHz`;
}

export function formatDuration(seconds: number): string {
  if (seconds < 60) return `${Math.round(seconds)} sec`;
  const days = Math.floor(seconds / 86_400);
  const hours = Math.floor((seconds % 86_400) / 3_600);
  const minutes = Math.floor((seconds % 3_600) / 60);
  if (days > 0) return `${days}d ${hours}h ${minutes}m`;
  if (hours > 0) return `${hours}h ${minutes}m`;
  return `${minutes} min`;
}

export function formatTemperature(value: number): string {
  return `${value.toFixed(value >= 100 ? 0 : 1)} °C`;
}

export function formatWatts(value: number): string {
  return `${value.toFixed(value >= 10 ? 1 : 2)} W`;
}

export function formatList(values: string[]): string {
  return values.length > 0 ? values.join(", ") : "None";
}

export function formatGpuName(name: string): string {
  const displayName = name
    .replace(/^NVIDIA(?:\s+Corporation)?\s+(?:GeForce\s+)?/i, "")
    .replace(/^AMD\s+(?:Radeon\s+)?/i, "")
    .trim();
  return displayName || name;
}

export function reasonFor(
  limits: Availability | undefined,
  keys: string[],
  fallback = "This reading is unsupported on this system.",
): string {
  for (const key of keys) {
    const reason = limits?.[key];
    if (reason) return reason;
  }
  return fallback;
}

export function combineSeries(series: Series[]): Series {
  const length = Math.max(0, ...series.map((values) => values.length));
  return Array.from({ length }, (_, index) => {
    let total = 0;
    let measured = false;
    for (const values of series) {
      const offset = length - values.length;
      const value = values[index - offset];
      if (value !== null && value !== undefined) {
        total += value;
        measured = true;
      }
    }
    return measured ? total : null;
  });
}

export function topGroups(
  groups: ProcessGroup[],
  read: (group: ProcessGroup) => number | null,
  label: (value: number) => string,
): DrivingMetric[] {
  return groups
    .map((group) => ({ group, value: read(group) }))
    .filter((entry): entry is { group: ProcessGroup; value: number } => entry.value !== null)
    .sort((a, b) => b.value - a.value)
    .slice(0, 5)
    .map((entry) => ({ ...entry, label: label(entry.value) }));
}
