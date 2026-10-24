import { OBJECTS } from "@/components/store/art/objects";
import { colourwayFor, splitName } from "@/components/store/art/colourways";

/** Illustrated product on its colourway stage. Pure SVG: no image requests, crisp at any size. */
export function ProductArt({
  name,
  className,
  decorative = true,
}: {
  name: string;
  className?: string;
  decorative?: boolean;
}) {
  const { range, item } = splitName(name);
  const c = colourwayFor(range);
  const Draw = OBJECTS[item];
  return (
    <svg
      viewBox="0 0 400 400"
      className={className}
      role={decorative ? "presentation" : "img"}
      aria-label={decorative ? undefined : name}
      preserveAspectRatio="xMidYMid slice"
    >
      <rect width="400" height="400" fill={c.bg} />
      <ellipse cx="200" cy="334" rx="140" ry="13" fill={c.dark} opacity="0.14" />
      <g>{Draw ? Draw(c) : <circle cx="200" cy="200" r="90" fill={c.body} />}</g>
    </svg>
  );
}
