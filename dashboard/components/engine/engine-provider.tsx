"use client";

import { createContext, useContext, useState } from "react";
import { useMetrics } from "@/hooks/use-metrics";

type Metrics = ReturnType<typeof useMetrics>;
interface EngineCtx extends Metrics {
  open: boolean;
  setOpen: (v: boolean) => void;
}

const Ctx = createContext<EngineCtx | null>(null);

/** One WebSocket for the whole storefront, shared by the header pill, live strip and the panel. */
export function EngineProvider({ children }: { children: React.ReactNode }) {
  const m = useMetrics();
  const [open, setOpen] = useState(false);
  return <Ctx.Provider value={{ ...m, open, setOpen }}>{children}</Ctx.Provider>;
}

export function useEngine(): EngineCtx {
  const v = useContext(Ctx);
  if (!v) throw new Error("useEngine outside EngineProvider");
  return v;
}
