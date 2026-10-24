"use client";

import Link from "next/link";
import { Minus, Plus } from "lucide-react";
import { ProductArt } from "@/components/store/product-art";
import { MAX_QTY, removeLines, setQty, useCart } from "@/lib/cart";
import { money } from "@/lib/money";

export function CartView() {
  const { lines, count, totalCents } = useCart();
  const step = "focus-visible:ring-signal grid size-9 place-items-center rounded-full outline-none hover:bg-paper focus-visible:ring-2";

  if (lines.length === 0) {
    return (
      <div className="py-10">
        <h1 className="display text-6xl">Your cart is empty.</h1>
        <p className="text-ink-soft mt-4 max-w-md text-lg">Stock is live, so what you add is reserved only when you place the order.</p>
        <Link href="/#shop" className="bg-ink mt-8 inline-block rounded-full px-6 py-3 font-semibold text-white hover:bg-black">Browse the gear</Link>
      </div>
    );
  }
  return (
    <>
      <h1 className="display text-6xl">Your cart</h1>
      <div className="mt-10 grid gap-10 lg:grid-cols-[1fr_22rem] lg:gap-14">
        <ul className="divide-line border-line divide-y border-y">
          {lines.map((l) => (
            <li key={l.sku} className="flex gap-4 py-5">
              <Link href={`/products/${l.sku}`} className="w-24 shrink-0 sm:w-28" aria-label={l.name}>
                <ProductArt name={l.name} className="aspect-square w-full rounded-xl" />
              </Link>
              <div className="flex min-w-0 flex-1 flex-col justify-between gap-3 sm:flex-row sm:items-start">
                <div>
                  <Link href={`/products/${l.sku}`} className="font-medium hover:underline">{l.name}</Link>
                  <p className="text-ink-soft text-sm">{l.category} · {money(l.unitCents)} each</p>
                  <button onClick={() => removeLines([l.sku])} className="text-ink-soft hover:text-ink focus-visible:ring-signal mt-2 rounded text-sm underline underline-offset-4 outline-none focus-visible:ring-2">
                    Remove
                  </button>
                </div>
                <div className="flex items-center gap-5">
                  <div className="border-line flex items-center rounded-full border bg-white" role="group" aria-label={`Quantity of ${l.name}`}>
                    <button className={step} onClick={() => setQty(l.sku, l.qty - 1)} aria-label="Decrease quantity"><Minus className="size-4" /></button>
                    <span className="w-7 text-center text-sm font-semibold tabular-nums">{l.qty}</span>
                    <button className={step} onClick={() => setQty(l.sku, l.qty + 1)} disabled={l.qty >= MAX_QTY} aria-label="Increase quantity"><Plus className="size-4" /></button>
                  </div>
                  <span className="display-md w-24 text-right text-xl tabular-nums">{money(l.unitCents * l.qty)}</span>
                </div>
              </div>
            </li>
          ))}
        </ul>

        <aside aria-label="Order summary" className="border-line h-fit rounded-3xl border bg-white p-6 lg:sticky lg:top-24">
          <dl className="space-y-2 text-sm">
            <div className="flex justify-between"><dt className="text-ink-soft">Items ({count})</dt><dd className="tabular-nums">{money(totalCents)}</dd></div>
            <div className="flex justify-between"><dt className="text-ink-soft">Delivery</dt><dd>Free</dd></div>
          </dl>
          <p className="border-line mt-4 flex items-baseline justify-between border-t pt-4">
            <span className="font-semibold">Total</span>
            <span className="display-md text-4xl tabular-nums">{money(totalCents)}</span>
          </p>
          <Link href="/checkout" className="bg-signal hover:bg-signal-deep focus-visible:ring-signal mt-6 block rounded-full py-3 text-center font-semibold text-white outline-none focus-visible:ring-2 focus-visible:ring-offset-2">
            Check out
          </Link>
          <p className="text-ink-soft mt-3 text-xs leading-snug">Stock is reserved the moment you place the order, not before.</p>
        </aside>
      </div>
    </>
  );
}
