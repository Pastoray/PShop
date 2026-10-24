// Mirrors `Snapshot` in crates/metrics-gateway/src/main.rs. Null = Prometheus had no data
// for that query (service not up yet, or no traffic) — render "—", never 0.
export type Num = number | null;

export interface Snapshot {
  ts_ms: number;
  prometheus_ok: boolean;
  throughput: {
    accepted_rps: Num;
    rejected_rps: Num;
    payments_ok_rps: Num;
    events_published_rps: Num;
  };
  latency: {
    accept_p50_ms: Num;
    accept_p99_ms: Num;
    db_write_p95_ms: Num;
    end_to_end_p95_s: Num;
  };
  queues: {
    rabbit_ready: Num;
    rabbit_unacked: Num;
    outbox_pending: Num;
    es_sync_lag_s: Num;
    dlq_5m: Num;
  };
  redis: { ops_per_sec: Num; memory_bytes: Num };
  rejected_by_reason: Record<string, number>;
  memory_by_service: Record<string, number>;
  health: Health[];
}

export interface Health {
  name: string;
  up: number;
  total: number;
  replicas_available: Num;
  replicas_desired: Num;
}
