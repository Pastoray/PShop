"use client";

import Link from "next/link";
import { useSearchParams } from "next/navigation";
import { useMemo, useState } from "react";
import { Hero, LiveStrip, pickFeatured } from "@/components/store/hero";
import { ProductTile } from "@/components/store/product-tile";
import { usePoll } from "@/hooks/use-poll";
import { searchProducts, type Product, type SearchResult } from "@/lib/api";
import { CATEGORIES } from "@/lib/categories";
import { cn } from "@/lib/utils";

const PAGE = 24;
type Sort = "featured" | "price-asc" | "price-desc" | "name" | "scarce";
const SORTS: { v: Sort; label: string }[] = [
  { v: "featured", label: "Featured" },
  { v: "price-asc", label: "Price, low to high" },
  { v: "price-desc", label: "Price, high to low" },
  { v: "name", label: "Name, A to Z" },
  { v: "scarce", label: "Fewest left" },
];
const PRICES: { v: string; label: string; test: (c: number) => boolean }[] = [
  { v: "any", label: "Any price", test: () => true },
  { v: "u50", label: "Under $50", test: (c) => c < 5000 },
  { v: "50-150", label: "$50 to $150", test: (c) => c >= 5000 && c < 15000 },
  { v: "150-300", label: "$150 to $300", test: (c) => c >= 15000 && c < 30000 },
  { v: "o300", label: "Over $300", test: (c) => c >= 30000 },
];

function Skeleton() {
  return (
    <ul className="grid grid-cols-2 gap-x-4 gap-y-8 sm:gap-x-6 md:grid-cols-3" aria-hidden>
      {Array.from({ length: 9 }, (_, i) => (
        <li key={i}>
          <div className="aspect-square rounded-2xl bg-white motion-safe:animate-pulse" />
          <div className="mt-3 h-5 w-2/3 rounded bg-white motion-safe:animate-pulse" />
        </li>
      ))}
    </ul>
  );
}

const sorters: Record<Sort, (a: Product, b: Product) => number> = {
  featured: (a, b) => a.sku.localeCompare(b.sku),
  "price-asc": (a, b) => a.price_cents - b.price_cents,
  "price-desc": (a, b) => b.price_cents - a.price_cents,
  name: (a, b) => a.name.localeCompare(b.name),
  scarce: (a, b) => (a.stock <= 0 ? 1e9 : a.stock) - (b.stock <= 0 ? 1e9 : b.stock),
};

export function CatalogueView() {
  const params = useSearchParams();
  const q = params.get("q") ?? "";
  const category = params.get("category") ?? "";
  const [data, setData] = useState<SearchResult | null>(null);
  const [failed, setFailed] = useState(false);
  const [sort, setSort] = useState<Sort>("featured");
  const [price, setPrice] = useState("any");
  const [inStock, setInStock] = useState(false);
  const [limit, setLimit] = useState(PAGE);

  // One fetch of the whole (<=100) catalogue, re-fetched every few seconds so stock follows real
  // purchases; category, price and sort are applied locally so they respond instantly.
  usePoll(
    async (signal) => {
      try {
        setData(await searchProducts({ q, limit: 100 }, signal));
        setFailed(false);
      } catch (e) {
        if (!(e instanceof DOMException && e.name === "AbortError")) setFailed(true);
      }
    },
    5000,
    { key: q },
  );

  const all = data?.hits ?? [];
  const counts = useMemo(() => {
    const m: Record<string, number> = {};
    for (const p of all) m[p.category] = (m[p.category] ?? 0) + 1;
    return m;
  }, [all]);
  const shown = useMemo(() => {
    const t = PRICES.find((p) => p.v === price)!.test;
    return all
      .filter((p) => (!category || p.category === category) && t(p.price_cents) && (!inStock || p.stock > 0))
      .sort(sorters[sort]);
  }, [all, category, price, inStock, sort]);

  const href = (c: string) => {
    const qs = new URLSearchParams();
    if (q) qs.set("q", q);
    if (c) qs.set("category", c);
    const s = qs.toString();
    return `${s ? `/?${s}` : "/"}#shop`;
  };
  const browsing = !q && !category;
  const filtersActive = price !== "any" || inStock;

  return (
    <>
      {browsing && (
        <>
          <Hero featured={pickFeatured(all)} />
          <LiveStrip />
        </>
      )}

      <section id="shop" aria-labelledby="shop-h" className="scroll-mt-24">
        <div className="flex flex-wrap items-end justify-between gap-4">
          <h2 id="shop-h" className="display text-5xl sm:text-6xl">
            {q ? <>Results for “{q}”</> : category || "All gear"}
          </h2>
          <div className="flex items-center gap-2 text-sm">
            <label htmlFor="sort" className="text-ink-soft">Sort</label>
            <select id="sort" value={sort} onChange={(e) => setSort(e.target.value as Sort)} className="border-line focus-visible:ring-signal rounded-full border bg-white px-3 py-2 outline-none focus-visible:ring-2">
              {SORTS.map((s) => (<option key={s.v} value={s.v}>{s.label}</option>))}
            </select>
          </div>
        </div>

        <div className="mt-8 grid gap-8 lg:grid-cols-[13rem_1fr] lg:gap-12">
          <aside aria-label="Filters" className="space-y-6">
            <nav aria-label="Categories">
              <ul className="flex flex-wrap gap-2 lg:flex-col lg:gap-0.5">
                {["", ...CATEGORIES].map((c) => {
                  const active = c === category;
                  return (
                    <li key={c || "all"}>
                      <Link
                        href={href(c)}
                        aria-current={active ? "page" : undefined}
                        className={cn(
                          "focus-visible:ring-signal flex items-center justify-between gap-3 rounded-full border px-4 py-1.5 text-sm outline-none focus-visible:ring-2 lg:rounded-lg lg:border-transparent lg:px-3",
                          active ? "border-ink bg-ink font-semibold text-white" : "border-line bg-white hover:border-ink lg:bg-transparent lg:hover:bg-white",
                        )}
                      >
                        {c || "All"}
                        <span className={cn("text-xs tabular-nums", active ? "text-white/70" : "text-ink-soft")}>{c ? (counts[c] ?? 0) : all.length}</span>
                      </Link>
                    </li>
                  );
                })}
              </ul>
            </nav>
            <details className="group lg:open" open>
              <summary className="focus-visible:ring-signal cursor-pointer rounded text-sm font-semibold outline-none focus-visible:ring-2 lg:pointer-events-none">
                Filter{filtersActive && <span className="text-tag-deep ml-2 font-normal">active</span>}
              </summary>
              <fieldset className="mt-3 space-y-1.5">
                <legend className="sr-only">Price</legend>
                {PRICES.map((p) => (
                  <label key={p.v} className="flex cursor-pointer items-center gap-2 text-sm">
                    <input type="radio" name="price" checked={price === p.v} onChange={() => setPrice(p.v)} className="accent-signal" />
                    {p.label}
                  </label>
                ))}
              </fieldset>
              <label className="mt-4 flex cursor-pointer items-center gap-2 text-sm">
                <input type="checkbox" checked={inStock} onChange={(e) => setInStock(e.target.checked)} className="accent-signal" />
                In stock only
              </label>
            </details>
          </aside>

          <div aria-live="polite">
            {failed && !data ? (
              <div className="border-line rounded-2xl border bg-white p-6">
                <p className="font-semibold">We can’t load products right now.</p>
                <p className="text-ink-soft mt-1 text-sm">This page keeps retrying. If it doesn’t come back, check that the stack is running.</p>
              </div>
            ) : !data ? (
              <Skeleton />
            ) : shown.length === 0 ? (
              <div className="border-line rounded-2xl border bg-white p-6">
                <p className="font-semibold">Nothing matches.</p>
                <p className="text-ink-soft mt-1 text-sm">
                  {filtersActive ? "Loosen the filters, or " : "Try a shorter word, or "}
                  <Link href="/" className="text-signal underline underline-offset-4">browse everything</Link>.
                </p>
              </div>
            ) : (
              <>
                <p className="text-ink-soft mb-5 text-sm">{shown.length} {shown.length === 1 ? "product" : "products"}</p>
                <ul className="grid grid-cols-2 gap-x-4 gap-y-9 sm:gap-x-6 md:grid-cols-3">
                  {shown.slice(0, limit).map((p) => (<ProductTile key={p.sku} p={p} />))}
                </ul>
                {shown.length > limit && (
                  <div className="mt-12 text-center">
                    <button onClick={() => setLimit((l) => l + PAGE)} className="border-ink focus-visible:ring-signal rounded-full border-2 px-7 py-3 font-semibold outline-none hover:bg-white focus-visible:ring-2 focus-visible:ring-offset-2">
                      Show more ({shown.length - limit} left)
                    </button>
                  </div>
                )}
              </>
            )}
          </div>
        </div>
      </section>
    </>
  );
}
