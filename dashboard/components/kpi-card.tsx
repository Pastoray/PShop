import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Sparkline } from "@/components/sparkline";
import { cn } from "@/lib/utils";

export function KpiCard({
  title,
  value,
  hint,
  series,
  tone = "default",
  sparkClass,
}: {
  title: string;
  value: string;
  hint?: string;
  series?: (number | null)[];
  tone?: "default" | "good" | "warn" | "bad";
  sparkClass?: string;
}) {
  return (
    <Card className="gap-3">
      <CardHeader>
        <CardDescription>{title}</CardDescription>
        <CardTitle
          className={cn(
            "text-3xl font-semibold tracking-tight",
            tone === "good" && "text-good",
            tone === "warn" && "text-warn",
            tone === "bad" && "text-destructive",
          )}
        >
          {value}
        </CardTitle>
      </CardHeader>
      <CardContent className="space-y-2">
        {series && <Sparkline values={series} className={sparkClass} />}
        {hint && <p className="text-muted-foreground text-xs">{hint}</p>}
      </CardContent>
    </Card>
  );
}
