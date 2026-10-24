"use client";

import Link from "next/link";
import { useEffect, useState } from "react";
import { usePoll } from "@/hooks/use-poll";
import { getOrderStatus, getProduct, type OrderStatus } from "@/lib/api";
import { loadOrders, type LocalOrder } from "@/lib/local-store";
import { money } from "@/lib/money";
import { cn } from "@/lib/utils";

type StepState = "done" | "active" | "todo";

function Step({ state, title, children, last }: { state: StepState; title: string; children?: React.ReactNode; last?: boolean }) {
  return (
    <li className="relative flex gap-4 pb-8 last:pb-0">
      {!last && (
        <span
          aria-hidden
          className={cn("absolute top-8 left-[13px] h-[calc(100%-2rem)] w-0.5", state === "done" ? "bg-signal" : "bg-line")}
        />
      )}
      <span
        aria-hidden
        className={cn(
          "relative mt-0.5 grid size-7 shrink-0 place-items-center rounded-full border-2",
          state === "done" && "border-signal bg-signal text-white",
          state === "active" && "border-signal bg-white",
          state === "todo" && "border-line bg-white",
        )}
      >
        {state === "done" && (
          <svg viewBox="0 0 16 16" className="size-3.5" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
            <path d="M3 8.5l3.2 3L13 4.5" />
          </svg>
        )}
        {state === "active" && <span className="bg-signal size-2.5 rounded-full motion-safe:animate-pulse" />}
      </span>
      <div className="min-w-0">
        <h3 className={cn("display-md text-xl leading-7", state === "todo" && "text-ink-soft")}>{title}</h3>
        <div className="text-ink-soft mt-0.5 text-[15px] leading-relaxed">{children}</div>
      </div>
    </li>
  );
}

const clock = (ms: number | string) =>
  new Date(ms).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" });

function waited(sec: number) {
  return sec < 90 ? `${sec} s` : `${Math.floor(sec / 60)} min ${sec % 60} s`;
}

export function OrderTracker({ id }: { id: string }) {
  const [local, setLocal] = useState<LocalOrder | null>(null);
  const [paid, setPaid] = useState<OrderStatus | null>(null);
  const [unreachable, setUnreachable] = useState(false);
  const [productName, setProductName] = useState<string | null>(null);
  const [now, setNow] = useState(() => Date.now());

  useEffect(() => {
    setLocal(loadOrders().find((o) => o.id === id) ?? null);
  }, [id]);

  // Poll the real order status until the payment queue has settled this order.
  usePoll(
    async (signal) => {
      const r = await getOrderStatus(id, signal);
      setUnreachable(r.kind === "unreachable");
      if (r.kind === "paid") setPaid(r.order);
    },
    2000,
    { enabled: !paid, key: id },
  );

  // Orders opened from another browser have no local record: take the name from the catalogue.
  useEffect(() => {
    if (local?.name) return setProductName(local.name);
    if (!paid) return;
    getProduct(paid.sku)
      .then((p) => setProductName(p?.name ?? paid.sku))
      .catch(() => setProductName(paid.sku));
  }, [local, paid]);

  useEffect(() => {
    if (paid) return;
    const t = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(t);
  }, [paid]);

  const sku = paid?.sku ?? local?.sku;
  const qty = paid?.qty ?? local?.qty;
  const unit = paid?.unit_price_cents ?? local?.unitCents;
  const total = paid?.total_cents ?? (unit !== undefined && qty !== undefined ? unit * qty : undefined);
  const accepted = local?.acceptedAt ?? (paid ? Date.parse(paid.accepted_at) : undefined);
  const waitedSec = accepted ? Math.max(0, Math.round((now - accepted) / 1000)) : 0;

  return (
    <div className="grid gap-12 md:grid-cols-[1fr_20rem] md:gap-16">
      <section aria-labelledby="h">
        <h1 id="h" className="display text-6xl">
          {paid ? "Your order is paid." : "Your order is in."}
        </h1>
        <p className="text-ink-soft mt-3 max-w-lg text-lg">
          {paid
            ? "Thanks. Your invoice is ready to download."
            : "We’ve reserved your items. Payments are processed in the order they were placed."}
        </p>

        <ol className="mt-10 max-w-lg" aria-live="polite">
          <Step state="done" title="Order accepted">
            {accepted ? `Items reserved at ${clock(accepted)}.` : "Your items are reserved."}
          </Step>
          <Step state={paid ? "done" : "active"} title={paid ? "Payment complete" : "Waiting for payment"}>
            {paid ? (
              <>Paid at {clock(paid.paid_at)}.</>
            ) : (
              <>
                In the payment queue{accepted ? ` for ${waited(waitedSec)}` : ""}.
                {waitedSec > 15 && " It’s busy right now; nothing is lost and you don’t need to stay on this page."}
              </>
            )}
          </Step>
          <Step state={paid ? "done" : "todo"} title="Invoice" last>
            {paid ? (
              <a
                href={paid.invoice_url}
                target="_blank"
                rel="noreferrer"
                className="bg-ink text-paper focus-visible:ring-signal mt-2 inline-block rounded-full px-5 py-2 text-sm font-semibold outline-none hover:bg-black focus-visible:ring-2 focus-visible:ring-offset-2 focus-visible:ring-offset-[var(--color-paper)]"
              >
                Download invoice (PDF)
              </a>
            ) : (
              "Created as soon as payment completes."
            )}
          </Step>
        </ol>

        {unreachable && !paid && (
          <p role="status" className="text-amber mt-8 text-sm">
            We can’t reach order tracking right now. Your order is safe; we’ll keep trying.
          </p>
        )}
      </section>

      <aside aria-label="Order summary" className="md:pt-3">
        <div className="border-line rounded-xl border bg-white p-5">
          <h2 className="display-md text-xl">Summary</h2>
          <dl className="mt-4 space-y-3 text-[15px]">
            <div className="flex justify-between gap-4">
              <dt className="text-ink-soft">Item</dt>
              <dd className="text-right font-medium">
                {sku ? (
                  <Link href={`/products/${sku}`} className="hover:underline">
                    {productName ?? sku}
                  </Link>
                ) : (
                  "…"
                )}
              </dd>
            </div>
            <div className="flex justify-between gap-4">
              <dt className="text-ink-soft">Quantity</dt>
              <dd className="font-medium tabular-nums">{qty ?? "…"}</dd>
            </div>
            <div className="border-line flex justify-between gap-4 border-t pt-3">
              <dt className="text-ink-soft">Total</dt>
              <dd className="display-md text-3xl tabular-nums">{total !== undefined ? money(total) : "…"}</dd>
            </div>
          </dl>
          <p className="text-ink-soft mt-5 text-xs">Order <span className="font-mono break-all">{id.slice(0, 8)}…{id.slice(-4)}</span></p>
        </div>
        <Link href="/" className="text-signal mt-5 inline-block underline underline-offset-4">
          Keep shopping
        </Link>
      </aside>
    </div>
  );
}
