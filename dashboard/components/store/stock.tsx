export const LOW_STOCK = 10;

export type StockState = "in" | "low" | "out";
export const stockState = (n: number): StockState => (n <= 0 ? "out" : n <= LOW_STOCK ? "low" : "in");

/** Plain-language stock text; colour is never the only signal. */
export function StockLabel({ stock, className = "" }: { stock: number; className?: string }) {
  const s = stockState(stock);
  if (s === "out") return <span className={`text-sold font-semibold ${className}`}>Sold out</span>;
  if (s === "low") return <span className={`text-tag-deep font-semibold ${className}`}>Only {stock} left</span>;
  return <span className={`text-ok font-medium ${className}`}>In stock</span>;
}
