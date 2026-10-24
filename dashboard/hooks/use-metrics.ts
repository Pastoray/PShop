"use client";

import { useEffect, useRef, useState } from "react";
import type { Snapshot } from "@/lib/types";

export type ConnState = "connecting" | "live" | "reconnecting";

const HISTORY = 120; // ~2 min at 1 Hz

function wsUrl(): string {
  const explicit = process.env.NEXT_PUBLIC_WS_URL;
  if (explicit) return explicit;
  const proto = window.location.protocol === "https:" ? "wss" : "ws";
  return `${proto}://${window.location.host}/ws`;
}

/**
 * Streams snapshots from the metrics-gateway WebSocket.
 * - Reconnects with capped exponential backoff + jitter (a gateway restart must not need a page reload).
 * - Keeps a bounded ring of history for sparklines (no unbounded memory growth in a tab left open).
 * - Treats >5 s without a frame as stale, so a half-open TCP connection can't show frozen numbers as "live".
 */
export function useMetrics() {
  const [history, setHistory] = useState<Snapshot[]>([]);
  const [conn, setConn] = useState<ConnState>("connecting");
  const [stale, setStale] = useState(false);
  const lastFrame = useRef(0);

  useEffect(() => {
    let ws: WebSocket | null = null;
    let timer: ReturnType<typeof setTimeout> | undefined;
    let attempt = 0;
    let closed = false;

    const connect = () => {
      if (closed) return;
      setConn(attempt === 0 ? "connecting" : "reconnecting");
      ws = new WebSocket(wsUrl());
      ws.onopen = () => {
        attempt = 0;
        setConn("live");
      };
      ws.onmessage = (ev) => {
        try {
          const snap = JSON.parse(ev.data as string) as Snapshot;
          lastFrame.current = Date.now();
          setStale(false);
          setHistory((h) => (h.length >= HISTORY ? [...h.slice(1), snap] : [...h, snap]));
        } catch {
          /* ignore malformed frame */
        }
      };
      ws.onclose = () => {
        if (closed) return;
        setConn("reconnecting");
        const delay = Math.min(10_000, 500 * 2 ** attempt) * (0.7 + Math.random() * 0.6);
        attempt += 1;
        timer = setTimeout(connect, delay);
      };
      ws.onerror = () => ws?.close();
    };

    connect();
    const watchdog = setInterval(() => {
      if (lastFrame.current && Date.now() - lastFrame.current > 5000) setStale(true);
    }, 1000);

    return () => {
      closed = true;
      clearTimeout(timer);
      clearInterval(watchdog);
      ws?.close();
    };
  }, []);

  return { history, latest: history.at(-1) ?? null, conn, stale };
}
