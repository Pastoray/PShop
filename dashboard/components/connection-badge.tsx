import { Badge } from "@/components/ui/badge";
import type { ConnState } from "@/hooks/use-metrics";

export function ConnectionBadge({ conn, stale, prometheusOk }: { conn: ConnState; stale: boolean; prometheusOk: boolean | null }) {
  if (conn !== "live") {
    return <Badge variant="warn">{conn === "connecting" ? "Connecting…" : "Reconnecting…"}</Badge>;
  }
  if (stale) return <Badge variant="warn">Stream stalled</Badge>;
  if (prometheusOk === false) return <Badge variant="destructive">Prometheus unreachable</Badge>;
  return (
    <Badge variant="good">
      <span className="bg-good size-1.5 animate-pulse rounded-full" /> Live
    </Badge>
  );
}
