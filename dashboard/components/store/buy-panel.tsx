"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useEffect, useRef, useState } from "react";
import { Minus, Plus } from "lucide-react";
import { StockLabel } from "@/components/store/stock";
import type { Product } from "@/lib/api";
import { addToCart, MAX_QTY } from "@/lib/cart";
import { money } from "@/lib/money";

export function BuyPanel({ product }: { product: Product }) {
  const router = useRouter();
  const [qty, setQty] = useState(1);
  const [added, setAdded] = useState(0);
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined);
  useEffect(() => () => clearTimeout(timer.current), []);

  const sold = product.stock <= 0;
  const max = Math.max(1, Math.min(MAX_QTY, product.stock));
  const q = Math.min(qty, max);
  const item = { sku: product.sku, name: product.name, category: product.category, unitCents: product.price_cents };

  const step = "focus-visible:ring-signal grid size-11 place-items-center rounded-full outline-none hover:bg-paper focus-visible:ring-2 disabled:opacity-35";
  return (
    <div className="mt-8">
      <div className="flex items-baseline gap-4">
        <span className="display-md text-5xl tabular-nums">{money(product.price_cents * q)}</span>
        <StockLabel stock={product.stock} />
      </div>
      {q > 1 && <p className="text-ink-soft mt-1 text-sm tabular-nums">{money(product.price_cents)} each</p>}

      <div className="mt-6 flex flex-wrap items-stretch gap-3">
        <div className="border-line flex items-center rounded-full border bg-white" role="group" aria-label="Quantity">
          <button className={step} onClick={() => setQty(Math.max(1, q - 1))} disabled={sold || q <= 1} aria-label="Decrease quantity"><Minus className="size-4" /></button>
          <span className="w-8 text-center font-semibold tabular-nums" aria-live="polite">{sold ? 0 : q}</span>
          <button className={step} onClick={() => setQty(Math.min(max, q + 1))} disabled={sold || q >= max} aria-label="Increase quantity"><Plus className="size-4" /></button>
        </div>
        <button
          disabled={sold}
          onClick={() => {
            addToCart(item, q);
            setAdded(q);
            clearTimeout(timer.current);
            timer.current = setTimeout(() => setAdded(0), 4000);
          }}
          className="bg-signal hover:bg-signal-deep focus-visible:ring-signal disabled:bg-ink-soft min-w-40 flex-1 rounded-full px-6 py-3 font-semibold text-white outline-none focus-visible:ring-2 focus-visible:ring-offset-2 disabled:cursor-not-allowed"
        >
          {sold ? "Sold out" : "Add to cart"}
        </button>
        {!sold && (
          <button
            onClick={() => {
              addToCart(item, q);
              router.push("/checkout");
            }}
            className="border-ink focus-visible:ring-signal rounded-full border-2 px-6 py-3 font-semibold outline-none hover:bg-white focus-visible:ring-2 focus-visible:ring-offset-2"
          >
            Buy now
          </button>
        )}
      </div>
      <p className="mt-3 min-h-6 text-sm" aria-live="polite">
        {added > 0 && (
          <>
            Added {added} to your cart. <Link href="/cart" className="text-signal underline underline-offset-4">View cart</Link>
          </>
        )}
        {sold && <span className="text-ink-soft">If someone’s order falls through, the units come back here on their own.</span>}
      </p>
    </div>
  );
}
