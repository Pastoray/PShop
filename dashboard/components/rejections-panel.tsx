import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { fmtNum } from "@/lib/format";
import type { Snapshot } from "@/lib/types";

const EXPLAIN: Record<string, string> = {
  sold_out: "stock exhausted — correct behaviour",
  overload: "in-flight cap hit — load shedding",
  queue_full: "billing backlog at cap — back-pressure",
  queue_unavailable: "broker down or outcome unknown",
  redis_unavailable: "stock store down — refusing blind accepts",
  unknown_sku: "client error",
  invalid: "client error",
};

export function RejectionsPanel({ s }: { s: Snapshot | null }) {
  const rows = Object.entries(s?.rejected_by_reason ?? {})
    .filter(([, v]) => v > 0.01)
    .sort((a, b) => b[1] - a[1]);
  return (
    <Card>
      <CardHeader>
        <CardTitle>Rejections by reason</CardTitle>
        <CardDescription>Most of these are designed back-pressure, not failures</CardDescription>
      </CardHeader>
      <CardContent>
        {rows.length === 0 ? (
          <p className="text-muted-foreground text-sm">Nothing rejected right now.</p>
        ) : (
          <ul className="space-y-2">
            {rows.map(([reason, rps]) => (
              <li key={reason} className="flex items-baseline justify-between gap-4 text-sm">
                <span>
                  <code className="text-xs">{reason}</code>
                  <span className="text-muted-foreground ml-2 text-xs">{EXPLAIN[reason] ?? ""}</span>
                </span>
                <span className="font-medium tabular-nums">{fmtNum(rps, 1)}/s</span>
              </li>
            ))}
          </ul>
        )}
      </CardContent>
    </Card>
  );
}
