"use client";

import Link from "next/link";
import { useSearchParams } from "next/navigation";
import { useEffect, useRef, useState } from "react";
import { ProductArt } from "@/components/store/product-art";
import { usePoll } from "@/hooks/use-poll";
import { getOrderStatus, type OrderStatus } from "@/lib/api";
import { loadOrders, type LocalOrder } from "@/lib/local-store";
import { money } from "@/lib/money";

const MAX_TRACKED = 30;

function waited(sec: number) {
  return sec < 90 ? `${sec} s` : `${Math.floor(sec / 60)} min ${sec % 60} s`;
}

export function OrderList() {
  const placed = Number(useSearchParams().get("placed") ?? 0);
  const [orders, setOrders] = useState<LocalOrder[] | null>(null);
  const [paid, setPaid] = useState<Record<string, OrderStatus>>({});
  const [now, setNow] = useState(() => Date.now());
  const paidRef = useRef(paid);
  paidRef.current = paid;

  useEffect(() => {
    const read = () => setOrders(loadOrders());
    read();
    window.addEventListener("pshop-orders", read);
    return () => window.removeEventListener("pshop-orders", read);
  }, []);
  useEffect(() => {
    const t = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(t);
  }, []);

  // One loop for the whole page: ask about every order that is not paid yet.
  usePoll(
    async (signal) => {
      const open = (orders ?? []).slice(0, MAX_TRACKED).filter((o) => !paidRef.current[o.id]);
      const found = await Promise.all(
        open.map(async (o) => {
          const r = await getOrderStatus(o.id, signal);
          return r.kind === "paid" ? ([o.id, r.order] as const) : null;
        }),
      );
      const add = found.filter((x): x is readonly [string, OrderStatus] => x !== null);
      if (add.length) setPaid((p) => ({ ...p, ...Object.fromEntries(add) }));
    },
    2000,
    { enabled: (orders?.length ?? 0) > 0, key: String(orders?.length ?? 0) },
  );

  const waiting = (orders ?? []).filter((o) => !paid[o.id]).length;
  return (
    <div className="max-w-3xl">
      <h1 className="display text-6xl">Orders</h1>
      {placed > 0 && (
        <p role="status" className="bg-signal-wash text-signal-deep mt-6 rounded-2xl px-5 py-4 text-[15px] leading-snug">
          {placed === 1 ? "Your item is" : `All ${placed} items are`} reserved. Payment happens in the background and this page updates by itself.
        </p>
      )}
      {orders === null ? null : orders.length === 0 ? (
        <div className="mt-8">
          <p className="text-lg">No orders from this browser yet.</p>
          <Link href="/#shop" className="text-signal mt-2 inline-block underline underline-offset-4">Browse the gear</Link>
        </div>
      ) : (
        <>
          <p className="text-ink-soft mt-4" aria-live="polite">
            {waiting === 0 ? "Everything is paid." : `${waiting} waiting for payment.`}
          </p>
          <ul className="border-line divide-line mt-6 divide-y border-y">
            {orders.map((o) => {
              const s = paid[o.id];
              const sec = Math.max(0, Math.round((now - o.acceptedAt) / 1000));
              return (
                <li key={o.id} className="flex items-center gap-4 py-4">
                  <ProductArt name={o.name} className="size-16 shrink-0 rounded-xl" />
                  <div className="min-w-0 flex-1">
                    <p className="truncate font-medium">{o.name}</p>
                    <p className="text-ink-soft text-sm tabular-nums">
                      {o.qty} × {money(o.unitCents)} · {new Date(o.acceptedAt).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}
                    </p>
                    <p className="mt-1 flex items-center gap-2 text-sm">
                      {s ? (
                        <>
                          <span className="bg-ok inline-block size-2 rounded-full" />
                          <span className="font-medium">Paid</span>
                          <a href={s.invoice_url} target="_blank" rel="noreferrer" className="text-signal underline underline-offset-4">Invoice (PDF)</a>
                        </>
                      ) : (
                        <>
                          <span className="bg-tag pulse-dot inline-block size-2 rounded-full" />
                          <span className="font-medium">Waiting for payment</span>
                          <span className="text-ink-soft tabular-nums">{waited(sec)}</span>
                        </>
                      )}
                    </p>
                  </div>
                  <div className="text-right">
                    <p className="display-md text-xl tabular-nums">{money(o.unitCents * o.qty)}</p>
                    <Link href={`/orders/${o.id}`} className="text-signal text-sm underline underline-offset-4">Details</Link>
                  </div>
                </li>
              );
            })}
          </ul>
        </>
      )}
    </div>
  );
}
