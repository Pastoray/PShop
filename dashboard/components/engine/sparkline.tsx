/** Tiny inline trend. Null samples break the line instead of drawing a fake zero. */
export function Sparkline({ values, className, stroke = "currentColor" }: { values: (number | null)[]; className?: string; stroke?: string }) {
  const nums = values.filter((v): v is number => v !== null && Number.isFinite(v));
  if (nums.length < 2) return <svg viewBox="0 0 100 28" className={className} aria-hidden />;
  const max = Math.max(...nums, 1e-9);
  const min = Math.min(...nums, 0);
  const span = max - min || 1;
  const step = 100 / Math.max(values.length - 1, 1);
  let d = "";
  let pen = false;
  values.forEach((v, i) => {
    if (v === null || !Number.isFinite(v)) {
      pen = false;
      return;
    }
    const y = 26 - ((v - min) / span) * 24;
    d += `${pen ? "L" : "M"}${(i * step).toFixed(1)} ${y.toFixed(1)}`;
    pen = true;
  });
  return (
    <svg viewBox="0 0 100 28" preserveAspectRatio="none" className={className} aria-hidden>
      <path d={d} fill="none" stroke={stroke} strokeWidth={1.6} vectorEffect="non-scaling-stroke" strokeLinejoin="round" />
    </svg>
  );
}
