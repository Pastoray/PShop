import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Progress } from "@/components/ui/progress";
import { fmtCompact, fmtSeconds } from "@/lib/format";
import type { Snapshot } from "@/lib/types";

const QUEUE_CAP = 500_000; // keep in sync with QUEUE_MAX_LEN

function Row({ label, value, sub }: { label: string; value: string; sub?: string }) {
  return (
    <div className="flex items-baseline justify-between gap-4 text-sm">
      <span className="text-muted-foreground">{label}</span>
      <span className="font-medium tabular-nums">
        {value}
        {sub && <span className="text-muted-foreground ml-1.5 text-xs font-normal">{sub}</span>}
      </span>
    </div>
  );
}

export function PipelinePanel({ s }: { s: Snapshot | null }) {
  const ready = s?.queues.rabbit_ready ?? null;
  const fill = ready === null ? 0 : (ready / QUEUE_CAP) * 100;
  const dlq = s?.queues.dlq_5m ?? 0;
  return (
    <Card>
      <CardHeader>
        <CardTitle>Pipeline backlog</CardTitle>
        <CardDescription>Where the spike waits while billing drains at its throttled rate</CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">
        <div className="space-y-2">
          <div className="flex items-baseline justify-between text-sm">
            <span className="text-muted-foreground">payments.process</span>
            <span className="font-medium tabular-nums">
              {fmtCompact(ready)} <span className="text-muted-foreground text-xs font-normal">/ {fmtCompact(QUEUE_CAP)} cap</span>
            </span>
          </div>
          <Progress value={fill} barClassName={fill > 80 ? "bg-destructive" : fill > 40 ? "bg-warn" : "bg-good"} />
          <p className="text-muted-foreground text-xs">At 100 % the broker rejects publishes and order-service answers 503 (<code>queue_full</code>).</p>
        </div>
        <div className="space-y-2 border-t pt-4">
          <Row label="Being processed (unacked)" value={fmtCompact(s?.queues.rabbit_unacked ?? null)} />
          <Row label="Outbox → Kafka pending" value={fmtCompact(s?.queues.outbox_pending ?? null)} />
          <Row label="Elasticsearch sync lag" value={fmtSeconds(s?.queues.es_sync_lag_s ?? null)} />
          <Row label="Dead-lettered (5 min)" value={fmtCompact(s?.queues.dlq_5m ?? null)} sub={dlq > 0 ? "SNS alert sent" : undefined} />
          <Row label="Time from 202 to persisted (p95)" value={fmtSeconds(s?.latency.end_to_end_p95_s ?? null)} />
        </div>
      </CardContent>
    </Card>
  );
}
