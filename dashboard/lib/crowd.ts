"use client";
// Browser-side crowd: an open-loop load generator that places real orders against /v1/orders.
// Open loop = requests are scheduled by the clock, not by the previous response, so a slow
// server cannot make the "crowd" politely slow down (that is how real traffic behaves).
// A browser tops out at a few hundred requests a second; `make load` (Rust) goes much higher.

import { useSyncExternalStore } from "react";
import { uid } from "@/lib/local-store";

export interface CrowdStats {
  running: boolean;
  targetRps: number;
  duration: number;
  elapsed: number;
  sent: number;
  accepted: number;
  soldOut: number;
  limited: number; // 429, edge rate limit
  overloaded: number; // 503, back-pressure
  failed: number; // network errors / anything else
  skipped: number; // browser could not keep up with the target rate
  p50: number | null;
  p99: number | null;
  achievedRps: number;
}

const SKUS = 100;
const HOT = 5;
const MAX_INFLIGHT = 400;
const idle: CrowdStats = {
  running: false, targetRps: 0, duration: 0, elapsed: 0, sent: 0, accepted: 0, soldOut: 0,
  limited: 0, overloaded: 0, failed: 0, skipped: 0, p50: null, p99: null, achievedRps: 0,
};

let stats: CrowdStats = idle;
let timer: ReturnType<typeof setInterval> | undefined;
const listeners = new Set<() => void>();
const publish = (s: CrowdStats) => {
  stats = s;
  listeners.forEach((l) => l());
};

const pickSku = () => {
  const i = Math.random() < 0.7 ? 1 + Math.floor(Math.random() * HOT) : 1 + Math.floor(Math.random() * SKUS);
  return `SKU-${String(i).padStart(4, "0")}`;
};

function pct(sorted: number[], q: number): number | null {
  if (!sorted.length) return null;
  return sorted[Math.min(sorted.length - 1, Math.floor(q * sorted.length))];
}

export function startCrowd(rps: number, durationS: number) {
  if (stats.running) return;
  let inflight = 0;
  let tokens = 0;
  let last = performance.now();
  const t0 = last;
  const c = { ...idle, running: true, targetRps: rps, duration: durationS };
  const lat: number[] = [];
  let n = 0;

  const fire = () => {
    inflight++;
    c.sent++;
    const t = performance.now();
    fetch("/v1/orders", {
      method: "POST",
      headers: { "content-type": "application/json", "idempotency-key": uid() },
      body: JSON.stringify({ user_id: `crowd-${n++ % 5000}`, sku: pickSku(), qty: 1 }),
    })
      .then((r) => {
        if (r.status === 202) c.accepted++;
        else if (r.status === 409) c.soldOut++;
        else if (r.status === 429) c.limited++;
        else if (r.status === 503) c.overloaded++;
        else c.failed++;
        lat.push(performance.now() - t);
        if (lat.length > 3000) lat.shift();
        return r.body?.cancel();
      })
      .catch(() => {
        c.failed++;
      })
      .finally(() => {
        inflight--;
      });
  };

  timer = setInterval(() => {
    const now = performance.now();
    tokens += ((now - last) / 1000) * rps;
    last = now;
    while (tokens >= 1) {
      tokens -= 1;
      if (inflight >= MAX_INFLIGHT) c.skipped++;
      else fire();
    }
    const elapsed = (now - t0) / 1000;
    const sorted = [...lat].sort((a, b) => a - b);
    publish({ ...c, elapsed, p50: pct(sorted, 0.5), p99: pct(sorted, 0.99), achievedRps: elapsed > 0 ? c.sent / elapsed : 0 });
    if (elapsed >= durationS) stopCrowd();
  }, 50);
  publish({ ...c });
}

export function stopCrowd() {
  if (timer) clearInterval(timer);
  timer = undefined;
  publish({ ...stats, running: false });
}

export function useCrowd(): CrowdStats {
  return useSyncExternalStore(
    (cb) => {
      listeners.add(cb);
      return () => listeners.delete(cb);
    },
    () => stats,
    () => idle,
  );
}
