// Thin client for the PShop HTTP API. All paths are same-origin (/v1/...): in the compose and
// Kubernetes stacks the ingress routes them; under `next dev` set API_ORIGIN (see next.config.ts).

export interface Product {
  sku: string;
  name: string;
  category: string;
  description: string;
  price_cents: number;
  stock: number;
  stock_version: number;
  updated_at: string;
}

export interface SearchResult {
  total: number;
  hits: Product[];
}

export interface OrderStatus {
  order_id: string;
  status: string;
  sku: string;
  qty: number;
  unit_price_cents: number;
  total_cents: number;
  accepted_at: string;
  paid_at: string;
  invoice_url: string;
}

async function getJson<T>(url: string, signal?: AbortSignal): Promise<T> {
  const res = await fetch(url, { signal, cache: "no-store" });
  if (!res.ok) throw new Error(`${url} -> ${res.status}`);
  return (await res.json()) as T;
}

export function searchProducts(
  p: { q?: string; category?: string; limit?: number; offset?: number },
  signal?: AbortSignal,
): Promise<SearchResult> {
  const qs = new URLSearchParams();
  if (p.q) qs.set("q", p.q);
  if (p.category) qs.set("category", p.category);
  qs.set("limit", String(p.limit ?? 24));
  if (p.offset) qs.set("offset", String(p.offset));
  return getJson<SearchResult>(`/v1/search?${qs}`, signal);
}

/** `null` = no such product. Throws on transport / server errors. */
export async function getProduct(sku: string, signal?: AbortSignal): Promise<Product | null> {
  const res = await fetch(`/v1/products/${encodeURIComponent(sku)}`, { signal, cache: "no-store" });
  if (res.status === 404 || res.status === 400) return null;
  if (!res.ok) throw new Error(`product ${sku} -> ${res.status}`);
  return (await res.json()) as Product;
}

export type PlaceResult =
  | { ok: true; orderId: string; duplicate: boolean }
  | { ok: false; reason: "sold_out" | "busy" | "unknown_product" | "invalid" | "network" };

/**
 * Place an order. The Idempotency-Key makes a retry safe: if the first attempt's outcome was
 * unknown (timeout, 503 after reservation) the server answers the retry with the original order
 * instead of reserving stock twice. Keep the same key for retries of the same cart.
 */
export async function placeOrder(
  body: { user_id: string; sku: string; qty: number },
  idempotencyKey: string,
): Promise<PlaceResult> {
  let res: Response;
  try {
    res = await fetch("/v1/orders", {
      method: "POST",
      headers: { "content-type": "application/json", "idempotency-key": idempotencyKey },
      body: JSON.stringify(body),
    });
  } catch {
    return { ok: false, reason: "network" };
  }
  if (res.status === 202) {
    const j = (await res.json()) as { order_id: string; status: string };
    return { ok: true, orderId: j.order_id, duplicate: j.status !== "accepted" };
  }
  if (res.status === 409) return { ok: false, reason: "sold_out" };
  if (res.status === 404) return { ok: false, reason: "unknown_product" };
  if (res.status === 400 || res.status === 422) return { ok: false, reason: "invalid" };
  return { ok: false, reason: "busy" }; // 429 / 503 / anything else: back-pressure, retry later
}

export type StatusResult =
  | { kind: "paid"; order: OrderStatus }
  | { kind: "queued" } // 404: accepted, but not settled in Postgres yet
  | { kind: "unreachable" };

export async function getOrderStatus(id: string, signal?: AbortSignal): Promise<StatusResult> {
  let res: Response;
  try {
    res = await fetch(`/v1/order-status/${encodeURIComponent(id)}`, { signal, cache: "no-store" });
  } catch (e) {
    if (e instanceof DOMException && e.name === "AbortError") throw e;
    return { kind: "unreachable" };
  }
  if (res.status === 200) return { kind: "paid", order: (await res.json()) as OrderStatus };
  if (res.status === 404) return { kind: "queued" };
  return { kind: "unreachable" };
}

export type ProviderMode = "normal" | "slow" | "down";

/** Demo control plane. `null` = disabled on this deployment (404) or unreachable. */
export async function getProviderMode(signal?: AbortSignal): Promise<ProviderMode | null> {
  try {
    const res = await fetch("/v1/control/payments", { signal, cache: "no-store" });
    if (!res.ok) return null;
    return ((await res.json()) as { mode: ProviderMode }).mode;
  } catch {
    return null;
  }
}

export async function setProviderMode(mode: ProviderMode): Promise<boolean> {
  try {
    const res = await fetch("/v1/control/payments", {
      method: "PUT",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ mode }),
    });
    return res.ok;
  } catch {
    return false;
  }
}
