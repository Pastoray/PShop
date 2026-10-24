"use client";

import Link from "next/link";
import { useEffect, useRef, useState } from "react";
import { X } from "lucide-react";
import { useEngine } from "@/components/engine/engine-provider";
import { Sparkline } from "@/components/engine/sparkline";
import { startCrowd, stopCrowd, useCrowd } from "@/lib/crowd";
import { getProviderMode, setProviderMode, type ProviderMode } from "@/lib/api";
import { fmtMs, fmtNum } from "@/lib/format";
import type { Snapshot } from "@/lib/types";

const waiting = (s: Snapshot) =>
  s.queues.rabbit_ready === null && s.queues.rabbit_unacked === null ? null : (s.queues.rabbit_ready ?? 0) + (s.queues.rabbit_unacked ?? 0);

function Stat({ label, value, unit, trend, hint }: { label: string; value: string; unit?: string; trend: (number | null)[]; hint: string }) {
  return (
    <div className="border-line rounded-xl border bg-white p-3">
      <p className="text-ink-soft text-xs">{label}</p>
      <p className="display-md mt-1 text-3xl tabular-nums">
        {value}
        {unit && <span className="text-ink-soft ml-1 text-sm font-medium">{unit}</span>}
      </p>
      <Sparkline values={trend} className="text-signal mt-1 h-6 w-full" stroke="var(--color-signal)" />
      <p className="text-ink-soft mt-1 text-xs leading-snug">{hint}</p>
    </div>
  );
}

const PROVIDER: { mode: ProviderMode; label: string; text: string }[] = [
  { mode: "normal", label: "Normal", text: "Payments clear in about 20 ms." },
  { mode: "slow", label: "Slow", text: "Each payment takes about 250 ms. Orders are still accepted instantly; the wait for payment grows." },
  { mode: "down", label: "Down", text: "Payments stop. Orders keep being accepted and queue up. Nothing is lost, and the queue drains when you switch back." },
];

function Provider({ open }: { open: boolean }) {
  const [mode, setMode] = useState<ProviderMode | null | undefined>(undefined); // undefined = loading
  const [busy, setBusy] = useState(false);
  // Re-read whenever the panel opens: another tab or `make` may have changed it.
  useEffect(() => {
    if (!open) return;
    const c = new AbortController();
    getProviderMode(c.signal).then(setMode);
    return () => c.abort();
  }, [open]);
  if (mode === undefined) return <p className="text-ink-soft text-sm">Checking…</p>;
  if (mode === null)
    return <p className="text-ink-soft text-sm">Not available here. The switch exists only in the demo stack (billing-service with CONTROL_ENABLED=true).</p>;
  const cur = PROVIDER.find((p) => p.mode === mode)!;
  return (
    <>
      <div role="radiogroup" aria-label="Payment provider" className="bg-paper grid grid-cols-3 gap-1 rounded-full p-1">
        {PROVIDER.map((p) => (
          <button
            key={p.mode}
            role="radio"
            aria-checked={mode === p.mode}
            disabled={busy}
            onClick={async () => {
              setBusy(true);
              if (await setProviderMode(p.mode)) setMode(p.mode);
              setBusy(false);
            }}
            className={`focus-visible:ring-signal rounded-full px-3 py-1.5 text-sm font-medium outline-none focus-visible:ring-2 ${
              mode === p.mode ? (p.mode === "normal" ? "bg-ink text-white" : "bg-tag text-white") : "hover:bg-white"
            }`}
          >
            {p.label}
          </button>
        ))}
      </div>
      <p className="text-ink-soft mt-2 text-sm leading-snug" aria-live="polite">{cur.text}</p>
    </>
  );
}

function Crowd() {
  const s = useCrowd();
  const [rps, setRps] = useState(200);
  const [dur, setDur] = useState(30);
  const bars: { label: string; n: number; tone: string }[] = [
    { label: "Accepted", n: s.accepted, tone: "bg-signal" },
    { label: "Sold out", n: s.soldOut, tone: "bg-ink-soft" },
    { label: "Rate limited at the edge", n: s.limited, tone: "bg-tag" },
    { label: "Shed by back-pressure", n: s.overloaded, tone: "bg-tag-deep" },
    { label: "Failed", n: s.failed, tone: "bg-sold" },
  ];
  const total = Math.max(1, s.accepted + s.soldOut + s.limited + s.overloaded + s.failed);
  return (
    <>
      <label className="flex items-baseline justify-between text-sm" htmlFor="rps">
        <span>Shoppers per second</span>
        <output className="display-md text-xl tabular-nums">{rps}</output>
      </label>
      <input id="rps" type="range" min={50} max={600} step={50} value={rps} disabled={s.running} onChange={(e) => setRps(+e.target.value)} className="accent-signal mt-1 w-full" />
      <div className="mt-3 flex items-center gap-2">
        <label className="sr-only" htmlFor="dur">Duration</label>
        <select id="dur" value={dur} disabled={s.running} onChange={(e) => setDur(+e.target.value)} className="border-line rounded-full border bg-white px-3 py-2 text-sm">
          <option value={10}>10 seconds</option>
          <option value={30}>30 seconds</option>
          <option value={60}>60 seconds</option>
        </select>
        {s.running ? (
          <button onClick={stopCrowd} className="bg-ink focus-visible:ring-signal flex-1 rounded-full px-4 py-2 text-sm font-semibold text-white outline-none focus-visible:ring-2">
            Stop ({Math.max(0, Math.ceil(s.duration - s.elapsed))} s left)
          </button>
        ) : (
          <button onClick={() => startCrowd(rps, dur)} className="bg-tag hover:bg-tag-deep focus-visible:ring-signal flex-1 rounded-full px-4 py-2 text-sm font-semibold text-white outline-none focus-visible:ring-2">
            Send the crowd
          </button>
        )}
      </div>
      {s.sent > 0 && (
        <div className="mt-4">
          <div className="flex h-2.5 overflow-hidden rounded-full bg-white" aria-hidden>
            {bars.map((b) => (<div key={b.label} className={b.tone} style={{ width: `${(b.n / total) * 100}%` }} />))}
          </div>
          <dl className="mt-3 space-y-1 text-sm">
            {bars.filter((b) => b.n > 0 || b.label === "Accepted").map((b) => (
              <div key={b.label} className="flex items-center justify-between">
                <dt className="flex items-center gap-2"><span className={`${b.tone} inline-block size-2.5 rounded-sm`} />{b.label}</dt>
                <dd className="tabular-nums">{fmtNum(b.n)}</dd>
              </div>
            ))}
          </dl>
          <p className="text-ink-soft mt-3 text-xs leading-snug">
            Seen from this browser: reply time p50 {fmtMs(s.p50)}, p99 {fmtMs(s.p99)} (includes your network and the browser's own queueing).
            Sent {fmtNum(s.achievedRps)}/s.
            {s.skipped > 0 && ` The browser couldn't keep up with the target (${fmtNum(s.skipped)} skipped); run make load for more.`}
          </p>
        </div>
      )}
    </>
  );
}

export function EnginePanel() {
  const { open, setOpen, latest, history, conn, stale } = useEngine();
  const closeRef = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    if (!open) return;
    closeRef.current?.focus();
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, setOpen]);

  const series = (f: (s: Snapshot) => number | null) => history.map(f);
  const live = conn === "live" && !stale;

  return (
    <aside
      aria-label="Engine"
      aria-hidden={!open}
      inert={!open}
      className={`border-line bg-paper fixed inset-y-0 right-0 z-50 flex w-full max-w-md flex-col border-l transition-transform duration-200 motion-reduce:transition-none ${open ? "translate-x-0 shadow-2xl" : "translate-x-full"}`}
    >
      <header className="border-line flex items-center justify-between border-b bg-white px-5 py-4">
        <div>
          <h2 className="display-md text-2xl">Engine</h2>
          <p className="text-ink-soft flex items-center gap-1.5 text-sm">
            <span className={`pulse-dot inline-block size-2 rounded-full ${live ? "bg-ok" : "bg-tag"}`} />
            {live ? "Live from the services" : conn === "connecting" ? "Connecting…" : "No live data. Retrying…"}
          </p>
        </div>
        <button ref={closeRef} onClick={() => setOpen(false)} aria-label="Close engine panel" className="focus-visible:ring-signal hover:bg-paper rounded-full p-2 outline-none focus-visible:ring-2">
          <X className="size-5" />
        </button>
      </header>

      <div className="flex-1 space-y-7 overflow-y-auto px-5 py-5">
        <section aria-labelledby="eng-live">
          <h3 id="eng-live" className="sr-only">Live numbers</h3>
          <div className="grid grid-cols-2 gap-3">
            <Stat label="Orders accepted" value={fmtNum(latest?.throughput.accepted_rps ?? null, 0)} unit="/s" trend={series((s) => s.throughput.accepted_rps)} hint="Stock reserved and queued" />
            <Stat label="Payments completed" value={fmtNum(latest?.throughput.payments_ok_rps ?? null, 0)} unit="/s" trend={series((s) => s.throughput.payments_ok_rps)} hint="Throttled to protect the database" />
            <Stat label="Waiting for payment" value={fmtNum(latest ? waiting(latest) : null)} trend={series(waiting)} hint="Orders in the queue right now" />
            <Stat label="Reply time" value={fmtMs(latest?.latency.accept_p99_ms ?? null)} trend={series((s) => s.latency.accept_p99_ms)} hint="99% of orders answered within" />
          </div>
        </section>

        <section aria-labelledby="eng-crowd">
          <h3 id="eng-crowd" className="display-md mb-1 text-xl">Send a crowd</h3>
          <p className="text-ink-soft mb-3 text-sm leading-snug">Real orders for random products, mostly the first five. Watch stock fall on the shop and the queue grow above.</p>
          <Crowd />
        </section>

        <section aria-labelledby="eng-prov">
          <h3 id="eng-prov" className="display-md mb-1 text-xl">Break the payment provider</h3>
          <p className="text-ink-soft mb-3 text-sm leading-snug">Flip it, then place an order yourself and watch it wait in the queue behind the crowd.</p>
          <Provider open={open} />
        </section>
      </div>

      <footer className="border-line border-t bg-white px-5 py-3 text-sm">
        <Link href="/admin" className="text-signal hover:text-signal-deep underline underline-offset-2">Open the full operations dashboard</Link>
      </footer>
    </aside>
  );
}
