import { cn } from "@/lib/utils";

/** Minimal shadcn-style progress bar (no Radix dependency needed for a read-only gauge). */
export function Progress({ value, className, barClassName }: { value: number; className?: string; barClassName?: string }) {
  const pct = Math.max(0, Math.min(100, value));
  return (
    <div
      role="progressbar"
      aria-valuenow={Math.round(pct)}
      aria-valuemin={0}
      aria-valuemax={100}
      className={cn("bg-secondary h-2 w-full overflow-hidden rounded-full", className)}
    >
      <div className={cn("bg-primary h-full rounded-full transition-all duration-500", barClassName)} style={{ width: `${pct}%` }} />
    </div>
  );
}
