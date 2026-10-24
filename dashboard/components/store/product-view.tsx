"use client";

import Link from "next/link";
import { useState } from "react";
import { BuyPanel } from "@/components/store/buy-panel";
import { ProductArt } from "@/components/store/product-art";
import { splitName } from "@/components/store/art/colourways";
import { usePoll } from "@/hooks/use-poll";
import { getProduct, type Product } from "@/lib/api";
import { specsFor } from "@/lib/specs";

type State = { kind: "loading" } | { kind: "missing" } | { kind: "error" } | { kind: "ready"; product: Product };

export function ProductView({ sku }: { sku: string }) {
  const [state, setState] = useState<State>({ kind: "loading" });

  // Real-time GET on the search index: the stock line follows other shoppers' purchases.
  usePoll(
    async (signal) => {
      try {
        const p = await getProduct(sku, signal);
        setState(p ? { kind: "ready", product: p } : { kind: "missing" });
      } catch (e) {
        if (e instanceof DOMException && e.name === "AbortError") return;
        setState((s) => (s.kind === "ready" ? s : { kind: "error" }));
      }
    },
    3000,
    { key: sku },
  );

  if (state.kind === "missing" || state.kind === "error") {
    return (
      <div className="max-w-lg py-10">
        <h1 className="display text-5xl">{state.kind === "missing" ? "We couldn’t find that product." : "This product won’t load."}</h1>
        <p className="text-ink-soft mt-4">{state.kind === "missing" ? "It may have been removed." : "We keep trying. You can also refresh the page."}</p>
        <Link href="/" className="text-signal mt-4 inline-block underline underline-offset-4">Back to all gear</Link>
      </div>
    );
  }
  if (state.kind === "loading") {
    return (
      <div className="grid gap-10 md:grid-cols-2" aria-hidden>
        <div className="aspect-square rounded-3xl bg-white motion-safe:animate-pulse" />
        <div className="space-y-4">
          <div className="h-14 w-3/4 rounded bg-white motion-safe:animate-pulse" />
          <div className="h-8 w-40 rounded bg-white motion-safe:animate-pulse" />
        </div>
      </div>
    );
  }

  const p = state.product;
  const { range, item } = splitName(p.name);
  return (
    <article className="grid gap-10 md:grid-cols-2 md:gap-14">
      <div className="md:sticky md:top-24 md:self-start">
        <nav aria-label="Breadcrumb" className="text-ink-soft mb-4 text-sm">
          <Link href="/#shop" className="hover:text-ink underline underline-offset-4">All gear</Link>
          <span aria-hidden> / </span>
          <Link href={`/?category=${p.category}#shop`} className="hover:text-ink underline underline-offset-4">{p.category}</Link>
        </nav>
        <ProductArt name={p.name} decorative={false} className={`aspect-square w-full rounded-3xl ${p.stock <= 0 ? "opacity-60 saturate-0" : ""}`} />
      </div>
      <div className="md:pt-9">
        <p className="text-ink-soft text-sm">{range} range · {item}</p>
        <h1 className="display mt-1 text-[clamp(3rem,7vw,5.2rem)] text-balance">{p.name}</h1>
        <p className="text-ink-soft mt-5 max-w-md text-lg leading-snug">{p.description}</p>
        <BuyPanel product={p} />

        <dl className="border-line mt-10 divide-y divide-[var(--color-line)] border-y border-[var(--color-line)] text-sm">
          {specsFor(p.category).map(([k, v]) => (
            <div key={k} className="flex justify-between gap-6 py-3">
              <dt className="text-ink-soft">{k}</dt>
              <dd className="text-right font-medium">{v}</dd>
            </div>
          ))}
          <div className="flex justify-between gap-6 py-3">
            <dt className="text-ink-soft">Delivery</dt>
            <dd className="text-right font-medium">Free, ships in two working days</dd>
          </div>
        </dl>
      </div>
    </article>
  );
}
