import type { Num } from "./types";

const dash = "—";

export function fmtNum(v: Num, digits = 0): string {
  if (v === null || !Number.isFinite(v)) return dash;
  return new Intl.NumberFormat("en-US", { maximumFractionDigits: digits, minimumFractionDigits: digits }).format(v);
}

export function fmtCompact(v: Num): string {
  if (v === null || !Number.isFinite(v)) return dash;
  return new Intl.NumberFormat("en-US", { notation: "compact", maximumFractionDigits: 1 }).format(v);
}

export function fmtMs(v: Num): string {
  if (v === null || !Number.isFinite(v)) return dash;
  return v < 10 ? `${v.toFixed(2)} ms` : v < 1000 ? `${v.toFixed(0)} ms` : `${(v / 1000).toFixed(2)} s`;
}

export function fmtSeconds(v: Num): string {
  if (v === null || !Number.isFinite(v)) return dash;
  if (v < 1) return `${(v * 1000).toFixed(0)} ms`;
  if (v < 120) return `${v.toFixed(1)} s`;
  return `${(v / 60).toFixed(1)} min`;
}

export function fmtBytes(v: Num): string {
  if (v === null || !Number.isFinite(v)) return dash;
  const units = ["B", "KB", "MB", "GB"];
  let n = v;
  let i = 0;
  while (n >= 1024 && i < units.length - 1) {
    n /= 1024;
    i++;
  }
  return `${n.toFixed(n >= 100 || i === 0 ? 0 : 1)} ${units[i]}`;
}
