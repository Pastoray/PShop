"use client";

import { ConnectionBadge } from "@/components/connection-badge";
import { HealthPanel } from "@/components/health-panel";
import { KpiCard } from "@/components/kpi-card";
import { PipelinePanel } from "@/components/pipeline-panel";
import { RejectionsPanel } from "@/components/rejections-panel";
import { useMetrics } from "@/hooks/use-metrics";
import { fmtBytes, fmtCompact, fmtMs, fmtNum } from "@/lib/format";

const SLO_MS = 5;

export default function Page() {
  const { history, latest: s, conn, stale } = useMetrics();
  const series = <T,>(pick: (x: NonNullable<typeof s>) => T | null) => history.map((h) => pick(h) as number | null);

  const p99 = s?.latency.accept_p99_ms ?? null;
  const p99Tone = p99 === null ? "default" : p99 <= SLO_MS ? "good" : p99 <= SLO_MS * 2 ? "warn" : "bad";
  const dbMs = s?.latency.db_write_p95_ms ?? null;

  return (
    <main className="mx-auto max-w-7xl space-y-6 px-4 py-6 sm:px-6">
      <header className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h1 className="text-xl font-semibold tracking-tight">PShop Engine</h1>
          <p className="text-muted-foreground text-sm">Order pipeline — live from Prometheus, 1 s resolution</p>
        </div>
        <ConnectionBadge conn={conn} stale={stale} prometheusOk={s ? s.prometheus_ok : null} />
      </header>

      <section aria-label="Key metrics" className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-4">
        <KpiCard
          title="Orders accepted / s"
          value={fmtNum(s?.throughput.accepted_rps ?? null)}
          hint={`${fmtNum(s?.throughput.rejected_rps ?? null)}/s rejected`}
          series={series((x) => x.throughput.accepted_rps)}
          sparkClass="text-info"
        />
        <KpiCard
          title="Accept latency p99"
          value={fmtMs(p99)}
          hint={`SLO < ${SLO_MS} ms · p50 ${fmtMs(s?.latency.accept_p50_ms ?? null)}`}
          series={series((x) => x.latency.accept_p99_ms)}
          tone={p99Tone}
          sparkClass={p99Tone === "bad" ? "text-destructive" : p99Tone === "warn" ? "text-warn" : "text-good"}
        />
        <KpiCard
          title="Payments settled / s"
          value={fmtNum(s?.throughput.payments_ok_rps ?? null)}
          hint={`${fmtNum(s?.throughput.events_published_rps ?? null)}/s OrderCompleted → Kafka`}
          series={series((x) => x.throughput.payments_ok_rps)}
          sparkClass="text-good"
        />
        <KpiCard
          title="Postgres write p95"
          value={fmtMs(dbMs)}
          hint="order + outbox transaction (ingestion latency)"
          series={series((x) => x.latency.db_write_p95_ms)}
          tone={dbMs === null ? "default" : dbMs < 50 ? "good" : dbMs < 250 ? "warn" : "bad"}
          sparkClass="text-warn"
        />
      </section>

      <section className="grid grid-cols-1 gap-4 lg:grid-cols-3">
        <div className="lg:col-span-2">
          <PipelinePanel s={s} />
        </div>
        <KpiCard
          title="Queue depth (payments.process)"
          value={fmtCompact(s?.queues.rabbit_ready ?? null)}
          hint={`Redis ${fmtCompact(s?.redis.ops_per_sec ?? null)} ops/s · ${fmtBytes(s?.redis.memory_bytes ?? null)}`}
          series={series((x) => x.queues.rabbit_ready)}
          sparkClass="text-warn"
        />
      </section>

      <section className="grid grid-cols-1 gap-4 lg:grid-cols-2">
        <HealthPanel s={s} />
        <RejectionsPanel s={s} />
      </section>
    </main>
  );
}
