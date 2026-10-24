import Link from "next/link";
import { ProductArt } from "@/components/store/product-art";
import { stockState } from "@/components/store/stock";
import { splitName } from "@/components/store/art/colourways";
import { money } from "@/lib/money";
import type { Product } from "@/lib/api";

export function ProductTile({ p }: { p: Product }) {
  const s = stockState(p.stock);
  const { item } = splitName(p.name);
  return (
    <li>
      <Link
        href={`/products/${p.sku}`}
        className="group focus-visible:ring-signal block rounded-2xl outline-none focus-visible:ring-2 focus-visible:ring-offset-4 focus-visible:ring-offset-[var(--color-paper)]"
      >
        <div className="relative overflow-hidden rounded-2xl">
          <ProductArt name={p.name} className={`aspect-square w-full ${s === "out" ? "opacity-60 saturate-0" : ""}`} />
          {s !== "in" && (
            <span className={`absolute top-3 left-3 rounded-full px-2.5 py-1 text-xs font-semibold ${s === "out" ? "bg-ink text-white" : "bg-tag text-white"}`}>
              {s === "out" ? "Sold out" : `Only ${p.stock} left`}
            </span>
          )}
        </div>
        <div className="mt-3 flex items-baseline justify-between gap-3">
          <h3 className="font-medium leading-tight group-hover:underline group-hover:underline-offset-4">{p.name}</h3>
          <span className={`display-md text-xl tabular-nums ${s === "out" ? "text-ink-soft line-through" : ""}`}>{money(p.price_cents)}</span>
        </div>
        <p className="text-ink-soft text-sm">{p.category} · {item}</p>
      </Link>
    </li>
  );
}
