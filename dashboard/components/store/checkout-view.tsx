"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useEffect, useRef, useState } from "react";
import { ProductArt } from "@/components/store/product-art";
import { placeOrder, type PlaceResult } from "@/lib/api";
import { removeLines, useCart } from "@/lib/cart";
import { guestId, saveOrder, uid } from "@/lib/local-store";
import { money } from "@/lib/money";

const FIELDS = [
  { id: "name", label: "Full name", auto: "name" },
  { id: "email", label: "Email", auto: "email", type: "email" },
  { id: "street", label: "Street address", auto: "street-address" },
  { id: "city", label: "City", auto: "address-level2" },
  { id: "post", label: "Postcode", auto: "postal-code" },
] as const;
const DETAILS = "pshop.checkout.v1";

type Fail = Extract<PlaceResult, { ok: false }>["reason"];
const WHY: Record<Fail, string> = {
  sold_out: "Sold out while you were checking out. Remove it, or pick something else.",
  busy: "The shop is very busy. Try again in a moment; you won’t be charged twice.",
  network: "Couldn’t reach the shop. Check your connection and try again.",
  invalid: "That quantity isn’t available. Lower it and try again.",
  unknown_product: "No longer listed.",
};

export function CheckoutView() {
  const router = useRouter();
  const { lines, totalCents } = useCart();
  const [vals, setVals] = useState<Record<string, string>>({});
  const [sending, setSending] = useState(false);
  const [problems, setProblems] = useState<Record<string, Fail>>({});
  // One idempotency key per (product, quantity): pressing "Place order" again after a timeout reuses
  // it, so the server answers with the original order instead of reserving stock twice.
  const keys = useRef(new Map<string, string>());

  useEffect(() => {
    try {
      const raw = localStorage.getItem(DETAILS);
      if (raw) setVals(JSON.parse(raw));
    } catch {
      /* ignore */
    }
  }, []);

  if (lines.length === 0 && !sending) {
    return (
      <div className="py-10">
        <h1 className="display text-6xl">Nothing to check out.</h1>
        <Link href="/#shop" className="text-signal mt-4 inline-block underline underline-offset-4">Browse the gear</Link>
      </div>
    );
  }

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setSending(true);
    setProblems({});
    try {
      localStorage.setItem(DETAILS, JSON.stringify(vals));
    } catch {
      /* ignore */
    }
    const group = uid();
    const user = guestId();
    const results = await Promise.all(
      lines.map(async (l) => {
        const k = `${l.sku}:${l.qty}`;
        if (!keys.current.has(k)) keys.current.set(k, uid());
        return { l, r: await placeOrder({ user_id: user, sku: l.sku, qty: l.qty }, keys.current.get(k)!) };
      }),
    );
    const bad: Record<string, Fail> = {};
    const okSkus: string[] = [];
    for (const { l, r } of results) {
      if (r.ok) {
        saveOrder({ id: r.orderId, sku: l.sku, name: l.name, category: l.category, qty: l.qty, unitCents: l.unitCents, acceptedAt: Date.now(), group });
        okSkus.push(l.sku);
        keys.current.delete(`${l.sku}:${l.qty}`);
      } else bad[l.sku] = r.reason;
    }
    removeLines(okSkus);
    if (Object.keys(bad).length === 0) {
      router.push(`/orders?placed=${okSkus.length}`);
      return;
    }
    setProblems(bad);
    setSending(false);
  }

  const input = "border-line focus-visible:ring-signal mt-1 w-full rounded-xl border bg-white px-4 py-3 outline-none focus-visible:ring-2";
  const failed = Object.keys(problems).length;
  return (
    <>
      <h1 className="display text-6xl">Check out</h1>
      <form onSubmit={submit} className="mt-10 grid gap-10 lg:grid-cols-[1fr_24rem] lg:gap-14">
        <div className="space-y-10">
          <fieldset className="grid gap-4 sm:grid-cols-2">
            <legend className="display-md mb-4 text-2xl">Delivery</legend>
            {FIELDS.map((f) => (
              <div key={f.id} className={f.id === "street" || f.id === "name" || f.id === "email" ? "sm:col-span-2" : ""}>
                <label htmlFor={f.id} className="text-sm font-medium">{f.label}</label>
                <input
                  id={f.id}
                  required
                  autoComplete={f.auto}
                  type={"type" in f ? f.type : "text"}
                  value={vals[f.id] ?? ""}
                  onChange={(e) => setVals((v) => ({ ...v, [f.id]: e.target.value }))}
                  className={input}
                />
              </div>
            ))}
          </fieldset>
          <section aria-labelledby="pay-h">
            <h2 id="pay-h" className="display-md mb-3 text-2xl">Payment</h2>
            <p className="border-line max-w-lg rounded-2xl border bg-white p-4 text-sm leading-snug">
              No card needed. Your order is accepted straight away and payment is processed in the background, in the order it arrived. You can watch it happen on the Orders page.
            </p>
          </section>
        </div>

        <aside aria-label="Order summary" className="border-line h-fit rounded-3xl border bg-white p-6 lg:sticky lg:top-24">
          <ul className="space-y-4">
            {lines.map((l) => (
              <li key={l.sku}>
                <div className="flex items-center gap-3">
                  <ProductArt name={l.name} className="size-14 shrink-0 rounded-lg" />
                  <div className="min-w-0 flex-1 text-sm">
                    <p className="truncate font-medium">{l.name}</p>
                    <p className="text-ink-soft tabular-nums">{l.qty} × {money(l.unitCents)}</p>
                  </div>
                  <span className="font-semibold tabular-nums">{money(l.qty * l.unitCents)}</span>
                </div>
                {problems[l.sku] && (
                  <p role="alert" className="text-sold mt-1.5 text-sm">
                    {WHY[problems[l.sku]]}{" "}
                    <button type="button" onClick={() => removeLines([l.sku])} className="underline underline-offset-4">Remove</button>
                  </p>
                )}
              </li>
            ))}
          </ul>
          <p className="border-line mt-5 flex items-baseline justify-between border-t pt-4">
            <span className="font-semibold">Total</span>
            <span className="display-md text-4xl tabular-nums">{money(totalCents)}</span>
          </p>
          <button
            type="submit"
            disabled={sending}
            className="bg-tag hover:bg-tag-deep focus-visible:ring-signal disabled:bg-ink-soft mt-6 w-full rounded-full py-3 font-semibold text-white outline-none focus-visible:ring-2 focus-visible:ring-offset-2"
          >
            {sending ? "Placing order…" : failed ? "Try again" : "Place order"}
          </button>
          <p className="text-ink-soft mt-3 text-xs leading-snug" aria-live="polite">
            {failed ? "Items that went through are already on your Orders page." : "Stock is reserved the moment you place the order."}
          </p>
        </aside>
      </form>
    </>
  );
}
