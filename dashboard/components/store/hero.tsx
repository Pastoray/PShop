"use client";

import Link from "next/link";
import { useEffect, useRef, useState } from "react";
import { useEngine } from "@/components/engine/engine-provider";
import { ProductArt } from "@/components/store/product-art";
import { StockLabel } from "@/components/store/stock";
import { addToCart } from "@/lib/cart";
import { fmtMs, fmtNum } from "@/lib/format";
import { money } from "@/lib/money";
import type { Product } from "@/lib/api";

const HOT = new Set(["SKU-0001", "SKU-0002", "SKU-0003", "SKU-0004", "SKU-0005"]);

/** The first five products take most of the crowd's traffic, so the one with the least left is the one to watch. */
export function pickFeatured(products: Product[]): Product | null {
  const hot = products.filter((p) => HOT.has(p.sku));
  const pool = hot.length ? hot : products;
  if (!pool.length) return null;
  const buyable = pool.filter((p) => p.stock > 0);
  return (buyable.length ? buyable : pool).reduce((a, b) => (b.stock < a.stock ? b : a));
}

const peak = new Map<string, number>();

export function Hero({ featured }: { featured: Product | null }) {
  const { setOpen } = useEngine();
  const [added, setAdded] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined);
  useEffect(() => () => clearTimeout(timer.current), []);

  if (featured) peak.set(featured.sku, Math.max(peak.get(featured.sku) ?? 0, featured.stock));
  const max = Math.max(50, featured ? peak.get(featured.sku) || 1 : 1); // bar = how much is left of a healthy shelf
  const pct = featured ? Math.max(0, Math.min(100, (featured.stock / max) * 100)) : 0;

  return (
    <section aria-labelledby="hero-h" className="grid items-center gap-10 pt-2 pb-12 lg:grid-cols-[1.15fr_1fr] lg:gap-14 lg:pb-16">
      <div>
        <h1 id="hero-h" className="display text-[clamp(3.4rem,9vw,7rem)] text-balance">
          If it says in stock, it’s yours.
        </h1>
        <p className="text-ink-soft mt-6 max-w-md text-lg leading-snug">
          Every count on this page is live. When a crowd goes for the same product at once, the number stays honest.
        </p>
        <div className="mt-8 flex flex-wrap gap-3">
          <a href="#shop" className="bg-ink focus-visible:ring-signal rounded-full px-6 py-3 font-semibold text-white outline-none hover:bg-black focus-visible:ring-2 focus-visible:ring-offset-2">
            Shop all gear
          </a>
          <button onClick={() => setOpen(true)} className="border-ink focus-visible:ring-signal rounded-full border-2 px-6 py-3 font-semibold outline-none hover:bg-white focus-visible:ring-2 focus-visible:ring-offset-2">
            Watch the engine
          </button>
        </div>
      </div>

      {featured ? (
        <div className="relative">
          <Link href={`/products/${featured.sku}`} className="focus-visible:ring-signal group block rounded-3xl outline-none focus-visible:ring-2 focus-visible:ring-offset-4">
            <ProductArt name={featured.name} className="aspect-square w-full rounded-3xl" />
          </Link>
          <div className="bg-tag absolute -top-3 right-3 rotate-[5deg] rounded-xl px-4 py-2 text-white shadow-md sm:right-6">
            <p className="display-md text-3xl tabular-nums">{money(featured.price_cents)}</p>
          </div>
          <div className="border-line mt-4 rounded-2xl border bg-white p-4">
            <div className="flex items-baseline justify-between gap-3">
              <div>
                <p className="text-ink-soft text-sm">Everyone’s after this one</p>
                <Link href={`/products/${featured.sku}`} className="display-md text-2xl hover:underline">{featured.name}</Link>
              </div>
              <StockLabel stock={featured.stock} className="text-sm whitespace-nowrap" />
            </div>
            <div className="bg-paper mt-3 h-2 overflow-hidden rounded-full" role="img" aria-label={`${featured.stock} left`}>
              <div className={`${featured.stock <= 10 ? "bg-tag" : "bg-signal"} h-full rounded-full transition-[width] duration-700 motion-reduce:transition-none`} style={{ width: `${pct}%` }} />
            </div>
            <button
              disabled={featured.stock <= 0}
              onClick={() => {
                addToCart({ sku: featured.sku, name: featured.name, category: featured.category, unitCents: featured.price_cents }, 1);
                setAdded(true);
                clearTimeout(timer.current);
                timer.current = setTimeout(() => setAdded(false), 2500);
              }}
              className="bg-signal hover:bg-signal-deep focus-visible:ring-signal mt-4 w-full rounded-full py-2.5 font-semibold text-white outline-none focus-visible:ring-2 focus-visible:ring-offset-2 disabled:bg-ink-soft"
            >
              {featured.stock <= 0 ? "Sold out" : added ? "Added to cart" : "Add to cart"}
            </button>
          </div>
        </div>
      ) : (
        <div className="bg-line/50 aspect-square w-full rounded-3xl motion-safe:animate-pulse" aria-hidden />
      )}
    </section>
  );
}

/** One line of live numbers: the engine, in shopper words. */
export function LiveStrip() {
  const { latest, conn, stale, setOpen } = useEngine();
  const live = conn === "live" && !stale && latest?.prometheus_ok;
  const waiting = latest ? (latest.queues.rabbit_ready ?? 0) + (latest.queues.rabbit_unacked ?? 0) : null;
  const item = (value: string, label: string) => (
    <div>
      <p className="display-md text-3xl text-white tabular-nums sm:text-4xl">{value}</p>
      <p className="text-sm text-white/70">{label}</p>
    </div>
  );
  return (
    <section aria-label="Live shop activity" className="bg-ink mb-14 rounded-3xl px-6 py-6 sm:px-8">
      <div className="flex flex-wrap items-center gap-x-10 gap-y-5">
        <p className="flex items-center gap-2 text-sm font-medium text-white">
          <span className={`pulse-dot inline-block size-2.5 rounded-full ${live ? "bg-[#3ddc97]" : "bg-tag"}`} />
          {live ? "Right now" : "Live numbers unavailable"}
        </p>
        {live && (
          <>
            {item(fmtNum(latest!.throughput.accepted_rps ?? null), "orders a second")}
            {item(fmtNum(waiting), "waiting for payment")}
            {item(fmtMs(latest!.latency.accept_p99_ms ?? null), "to answer 99% of orders")}
          </>
        )}
        <button onClick={() => setOpen(true)} className="focus-visible:ring-signal ml-auto rounded-full bg-white px-5 py-2.5 text-sm font-semibold outline-none hover:bg-white/90 focus-visible:ring-2 focus-visible:ring-offset-2 focus-visible:ring-offset-[var(--color-ink)]">
          {live ? "Open the engine" : "What is this?"}
        </button>
      </div>
    </section>
  );
}
