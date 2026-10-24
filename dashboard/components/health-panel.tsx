import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { fmtBytes } from "@/lib/format";
import type { Snapshot } from "@/lib/types";

export function HealthPanel({ s }: { s: Snapshot | null }) {
  const rows = [...(s?.health ?? [])].sort((a, b) => a.name.localeCompare(b.name));
  return (
    <Card>
      <CardHeader>
        <CardTitle>Service & container health</CardTitle>
        <CardDescription>Prometheus targets; replica counts come from Kubernetes when running there</CardDescription>
      </CardHeader>
      <CardContent>
        {rows.length === 0 ? (
          <p className="text-muted-foreground text-sm">No targets reported yet.</p>
        ) : (
          <ul className="divide-y">
            {rows.map((h) => {
              const allUp = h.up >= h.total;
              const none = h.up === 0;
              const mem = s?.memory_by_service[h.name];
              return (
                <li key={h.name} className="flex items-center justify-between gap-3 py-2 text-sm">
                  <span className="font-medium">{h.name}</span>
                  <span className="flex items-center gap-2">
                    {mem !== undefined && <span className="text-muted-foreground text-xs tabular-nums">{fmtBytes(mem)} RSS</span>}
                    {h.replicas_desired !== null && (
                      <span className="text-muted-foreground text-xs tabular-nums">
                        {h.replicas_available ?? 0}/{h.replicas_desired} pods
                      </span>
                    )}
                    <Badge variant={none ? "destructive" : allUp ? "good" : "warn"}>
                      {h.up}/{h.total} up
                    </Badge>
                  </span>
                </li>
              );
            })}
          </ul>
        )}
      </CardContent>
    </Card>
  );
}
