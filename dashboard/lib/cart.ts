"use client";
// Cart: lives in this browser only (no accounts). External-store pattern so every component
// (header badge, drawer, product page) sees the same state, with localStorage as persistence.

import { useSyncExternalStore } from "react";

export interface CartLine {
  sku: string;
  name: string;
  category: string;
  unitCents: number;
  qty: number;
}

export const MAX_QTY = 10;
const KEY = "pshop.cart.v1";
const EMPTY: CartLine[] = [];
let lines: CartLine[] = EMPTY;
let loaded = false;
const listeners = new Set<() => void>();

function load() {
  if (loaded) return;
  loaded = true;
  try {
    const raw = localStorage.getItem(KEY);
    if (raw) lines = JSON.parse(raw) as CartLine[];
  } catch {
    /* blocked storage: start empty, keep in memory */
  }
}

function commit(next: CartLine[]) {
  lines = next.length ? next : EMPTY;
  try {
    localStorage.setItem(KEY, JSON.stringify(lines));
  } catch {
    /* kept in memory */
  }
  listeners.forEach((l) => l());
}

export function addToCart(item: Omit<CartLine, "qty">, qty: number) {
  load();
  const existing = lines.find((l) => l.sku === item.sku);
  const total = Math.min(MAX_QTY, (existing?.qty ?? 0) + qty);
  commit(existing ? lines.map((l) => (l.sku === item.sku ? { ...l, ...item, qty: total } : l)) : [...lines, { ...item, qty: total }]);
}

export function setQty(sku: string, qty: number) {
  load();
  commit(qty <= 0 ? lines.filter((l) => l.sku !== sku) : lines.map((l) => (l.sku === sku ? { ...l, qty: Math.min(MAX_QTY, qty) } : l)));
}

export function removeLines(skus: string[]) {
  load();
  commit(lines.filter((l) => !skus.includes(l.sku)));
}

export function clearCart() {
  load();
  commit([]);
}

function subscribe(cb: () => void) {
  listeners.add(cb);
  const onStorage = (e: StorageEvent) => {
    if (e.key === KEY) {
      loaded = false;
      load();
      cb();
    }
  };
  window.addEventListener("storage", onStorage);
  return () => {
    listeners.delete(cb);
    window.removeEventListener("storage", onStorage);
  };
}

export function useCart(): { lines: CartLine[]; count: number; totalCents: number } {
  const snap = useSyncExternalStore(
    subscribe,
    () => {
      load();
      return lines;
    },
    () => EMPTY,
  );
  return {
    lines: snap,
    count: snap.reduce((n, l) => n + l.qty, 0),
    totalCents: snap.reduce((n, l) => n + l.qty * l.unitCents, 0),
  };
}
