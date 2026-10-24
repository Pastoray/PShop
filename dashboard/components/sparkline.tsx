"use client";

import { useId } from "react";

/** Dependency-free area sparkline. `null`s break the line instead of plotting as zero. */
export function Sparkline({
  values,
  height = 40,
  className = "text-info",
  min = 0,
}: {
  values: (number | null)[];
  height?: number;
  className?: string;
  min?: number;
}) {
  const id = useId();
  const w = 200;
  const pts = values.filter((v): v is number => v !== null && Number.isFinite(v));
  if (pts.length < 2) return <div style={{ height }} aria-hidden />;
  const max = Math.max(...pts, min + 1e-9);
  const lo = Math.min(min, ...pts);
  const x = (i: number) => (i / (values.length - 1)) * w;
  const y = (v: number) => height - 2 - ((v - lo) / (max - lo || 1)) * (height - 4);

  const segs: string[][] = [[]];
  values.forEach((v, i) => {
    if (v === null || !Number.isFinite(v)) segs.push([]);
    else segs[segs.length - 1].push(`${x(i).toFixed(1)},${y(v).toFixed(1)}`);
  });
  const lines = segs.filter((s) => s.length > 1);

  return (
    <svg viewBox={`0 0 ${w} ${height}`} preserveAspectRatio="none" className={`w-full ${className}`} style={{ height }} aria-hidden>
      <defs>
        <linearGradient id={id} x1="0" x2="0" y1="0" y2="1">
          <stop offset="0%" stopColor="currentColor" stopOpacity="0.35" />
          <stop offset="100%" stopColor="currentColor" stopOpacity="0" />
        </linearGradient>
      </defs>
      {lines.map((s, i) => (
        <g key={i}>
          <polygon points={`${s[0].split(",")[0]},${height} ${s.join(" ")} ${s.at(-1)!.split(",")[0]},${height}`} fill={`url(#${id})`} />
          <polyline points={s.join(" ")} fill="none" stroke="currentColor" strokeWidth="1.5" vectorEffect="non-scaling-stroke" />
        </g>
      ))}
    </svg>
  );
}
