"use client";

import Link from "next/link";
import { usePathname, useRouter, useSearchParams } from "next/navigation";
import { Suspense, useEffect, useState } from "react";
import { ShoppingBag } from "lucide-react";
import { useEngine } from "@/components/engine/engine-provider";
import { useCart } from "@/lib/cart";
import { fmtNum } from "@/lib/format";
import { loadOrders } from "@/lib/local-store";

const focus = "focus-visible:ring-signal outline-none focus-visible:ring-2";

function SearchBox() {
  const router = useRouter();
  const params = useSearchParams();
  const [q, setQ] = useState(params.get("q") ?? "");
  useEffect(() => setQ(params.get("q") ?? ""), [params]);
  return (
    <form
      role="search"
      onSubmit={(e) => {
        e.preventDefault();
        const v = q.trim();
        router.push(v ? `/?q=${encodeURIComponent(v)}#shop` : "/#shop");
      }}
    >
      <label htmlFor="q" className="sr-only">Search products</label>
      <input
        id="q"
        value={q}
        onChange={(e) => setQ(e.target.value)}
        placeholder="Search headphones, keyboards, lamps"
        className={`border-line placeholder:text-ink-soft w-full rounded-full border bg-white px-4 py-2 text-sm ${focus}`}
      />
    </form>
  );
}

function useOrderCount() {
  const [n, setN] = useState(0);
  useEffect(() => {
    const read = () => setN(loadOrders().length);
    read();
    window.addEventListener("pshop-orders", read);
    window.addEventListener("storage", read);
    return () => {
      window.removeEventListener("pshop-orders", read);
      window.removeEventListener("storage", read);
    };
  }, []);
  return n;
}

function EnginePill() {
  const { latest, conn, stale, open, setOpen } = useEngine();
  const live = conn === "live" && !stale;
  const rps = latest?.throughput.accepted_rps ?? null;
  return (
    <button
      onClick={() => setOpen(!open)}
      aria-expanded={open}
      className={`border-signal/30 bg-signal-wash text-signal-deep hover:bg-signal hover:text-white flex items-center gap-2 rounded-full border px-3 py-1.5 text-sm font-semibold whitespace-nowrap ${focus}`}
    >
      <span className={`pulse-dot inline-block size-2 rounded-full ${live ? "bg-ok" : "bg-tag"}`} />
      Engine
      <span className="hidden tabular-nums sm:inline">{live && rps !== null ? `${fmtNum(rps)}/s` : ""}</span>
    </button>
  );
}

export function SiteHeader() {
  const orders = useOrderCount();
  const { count } = useCart();
  const path = usePathname();
  const link = (href: string, active: boolean) =>
    `rounded px-1 py-1 text-sm font-medium whitespace-nowrap ${focus} ${active ? "text-ink underline decoration-tag decoration-2 underline-offset-8" : "text-ink-soft hover:text-ink"}`;
  return (
    <header className="border-line sticky top-0 z-30 border-b bg-white/95 backdrop-blur">
      <div className="mx-auto flex max-w-6xl items-center gap-3 px-4 py-3 sm:gap-6 sm:px-6">
        <Link href="/" className={`display rounded text-[2rem] ${focus}`} aria-label="PShop home">
          PShop<span className="text-tag">.</span>
        </Link>
        <div className="hidden max-w-sm flex-1 md:block">
          <Suspense fallback={<div className="h-9" />}>
            <SearchBox />
          </Suspense>
        </div>
        <nav aria-label="Main" className="ml-auto flex items-center gap-3 sm:gap-5">
          <Link href="/orders" className={link("/orders", path.startsWith("/orders"))}>
            Orders{orders > 0 && <span className="bg-ink ml-1.5 rounded-full px-1.5 py-0.5 text-xs text-white tabular-nums">{orders}</span>}
          </Link>
          <Link href="/cart" aria-label={`Cart, ${count} items`} className={`relative flex items-center gap-1.5 ${link("/cart", path === "/cart" || path === "/checkout")}`}>
            <ShoppingBag className="size-5" />
            <span className="hidden sm:inline">Cart</span>
            {count > 0 && <span className="bg-tag rounded-full px-1.5 py-0.5 text-xs font-semibold text-white tabular-nums">{count}</span>}
          </Link>
          <EnginePill />
        </nav>
      </div>
      <div className="px-4 pb-3 md:hidden">
        <Suspense fallback={<div className="h-9" />}>
          <SearchBox />
        </Suspense>
      </div>
    </header>
  );
}
