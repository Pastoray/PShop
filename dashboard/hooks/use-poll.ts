"use client";

import { useEffect, useRef } from "react";

/**
 * Runs `fn` now and then every `ms` after each run finishes (never overlapping).
 * Change `key` to restart (and fetch immediately). Pauses while the tab is hidden and runs again as soon as it becomes visible.
 */
export function usePoll(
  fn: (signal: AbortSignal) => Promise<void>,
  ms: number,
  { enabled = true, key = "" }: { enabled?: boolean; key?: string } = {},
) {
  const latest = useRef(fn);
  useEffect(() => {
    latest.current = fn;
  });

  useEffect(() => {
    if (!enabled) return;
    let stopped = false;
    let running = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const ctrl = new AbortController();

    const tick = async () => {
      if (stopped || running) return;
      running = true;
      if (!document.hidden) {
        try {
          await latest.current(ctrl.signal);
        } catch {
          /* the caller decides how to surface errors; polling just continues */
        }
      }
      running = false;
      if (!stopped) timer = setTimeout(tick, ms);
    };
    const onVisible = () => {
      if (!document.hidden && !running) {
        clearTimeout(timer);
        void tick();
      }
    };

    void tick();
    document.addEventListener("visibilitychange", onVisible);
    return () => {
      stopped = true;
      ctrl.abort();
      clearTimeout(timer);
      document.removeEventListener("visibilitychange", onVisible);
    };
  }, [ms, enabled, key]); // a new `key` restarts the loop immediately (e.g. filters changed)
}
