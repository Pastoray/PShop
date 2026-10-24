// Browser-side "account": a guest id and the orders placed from this browser.
// localStorage can be blocked or full (private windows), so every access is guarded and the
// app keeps working from memory when it fails.

export interface LocalOrder {
  id: string;
  sku: string;
  name: string;
  qty: number;
  unitCents: number;
  acceptedAt: number; // epoch ms
  category?: string;
  group?: string; // checkout that created it
}

const ORDERS = "pshop.orders.v1";
const GUEST = "pshop.guest.v1";
let memoryGuest: string | null = null;
let memoryOrders: LocalOrder[] = [];

export function uid(): string {
  const c = globalThis.crypto;
  if (c?.randomUUID) return c.randomUUID(); // only exists in secure contexts (https / localhost)
  return "xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx".replace(/[xy]/g, (ch) => {
    const r = (Math.random() * 16) | 0;
    return (ch === "x" ? r : (r & 0x3) | 0x8).toString(16);
  });
}

export function guestId(): string {
  try {
    let g = localStorage.getItem(GUEST);
    if (!g) {
      g = `guest-${uid().slice(0, 8)}`;
      localStorage.setItem(GUEST, g);
    }
    return g;
  } catch {
    memoryGuest ??= `guest-${uid().slice(0, 8)}`;
    return memoryGuest;
  }
}

export function loadOrders(): LocalOrder[] {
  try {
    const raw = localStorage.getItem(ORDERS);
    return raw ? (JSON.parse(raw) as LocalOrder[]) : [];
  } catch {
    return memoryOrders;
  }
}

export function saveOrder(o: LocalOrder): void {
  const next = [o, ...loadOrders().filter((x) => x.id !== o.id)].slice(0, 50);
  memoryOrders = next;
  try {
    localStorage.setItem(ORDERS, JSON.stringify(next));
  } catch {
    /* kept in memory */
  }
  window.dispatchEvent(new Event("pshop-orders"));
}
