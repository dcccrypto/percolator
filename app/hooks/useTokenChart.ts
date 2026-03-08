"use client";

import { useEffect, useState, useCallback, useRef } from "react";
import type { CandleData } from "@/app/api/chart/[mint]/route";

export type ChartDataStatus = "idle" | "loading" | "success" | "error" | "empty";

export interface UseTokenChartResult {
  candles: CandleData[];
  poolAddress: string | null;
  status: ChartDataStatus;
  error: string | null;
  refresh: () => void;
}

type Timeframe = "1h" | "4h" | "1d" | "7d" | "30d";

const TIMEFRAME_TO_API: Record<
  Timeframe,
  { timeframe: "minute" | "hour" | "day"; aggregate: number; limit: number }
> = {
  "1h": { timeframe: "minute", aggregate: 5, limit: 12 },     // 5-min candles, 1h of data
  "4h": { timeframe: "minute", aggregate: 15, limit: 16 },    // 15-min candles, 4h of data
  "1d": { timeframe: "hour", aggregate: 1, limit: 24 },       // 1-hour candles, 1d of data
  "7d": { timeframe: "hour", aggregate: 4, limit: 42 },       // 4-hour candles, 7d of data
  "30d": { timeframe: "day", aggregate: 1, limit: 30 },       // 1-day candles, 30d of data
};

/** Fetch interval: 60s for short timeframes, 5min for daily */
const POLL_INTERVAL_MS = 60 * 1000;

/**
 * PERC-512: Hook that fetches external OHLCV candle data for a Solana token.
 *
 * Data source: /api/chart/[mint] → GeckoTerminal (free, no API key)
 * Falls back gracefully when no data is available (chart shows oracle prices).
 *
 * @param mintAddress - SPL token mint address (null/undefined = no fetch)
 * @param timeframe   - Chart timeframe (controls candle size and count)
 */
export function useTokenChart(
  mintAddress: string | null | undefined,
  timeframe: Timeframe = "1d"
): UseTokenChartResult {
  const [candles, setCandles] = useState<CandleData[]>([]);
  const [poolAddress, setPoolAddress] = useState<string | null>(null);
  const [status, setStatus] = useState<ChartDataStatus>("idle");
  const [error, setError] = useState<string | null>(null);

  // Track current mint+timeframe to avoid stale updates
  const fetchKeyRef = useRef<string>("");

  const fetchData = useCallback(
    async (mint: string, tf: Timeframe) => {
      const { timeframe: apiTf, aggregate, limit } = TIMEFRAME_TO_API[tf];
      const url = `/api/chart/${mint}?timeframe=${apiTf}&aggregate=${aggregate}&limit=${limit}`;
      const key = `${mint}:${tf}`;
      fetchKeyRef.current = key;

      setStatus("loading");
      setError(null);

      try {
        const res = await fetch(url);
        if (!res.ok) throw new Error(`HTTP ${res.status}`);
        const json = await res.json();

        // Guard stale updates: only apply if this is still the current fetch
        if (fetchKeyRef.current !== key) return;

        const fetchedCandles: CandleData[] = json.candles ?? [];
        setCandles(fetchedCandles);
        setPoolAddress(json.poolAddress ?? null);
        setStatus(fetchedCandles.length > 0 ? "success" : "empty");
      } catch (err) {
        if (fetchKeyRef.current !== key) return;
        console.warn("[useTokenChart] fetch error:", err);
        setError(err instanceof Error ? err.message : "Unknown error");
        setStatus("error");
      }
    },
    []
  );

  // Initial fetch + timeframe changes
  useEffect(() => {
    if (!mintAddress) {
      setCandles([]);
      setPoolAddress(null);
      setStatus("idle");
      setError(null);
      return;
    }

    fetchData(mintAddress, timeframe);

    // Poll for fresh data every 60 seconds (only short timeframes benefit)
    if (timeframe === "1h" || timeframe === "4h" || timeframe === "1d") {
      const interval = setInterval(() => {
        fetchData(mintAddress, timeframe);
      }, POLL_INTERVAL_MS);
      return () => clearInterval(interval);
    }
  }, [mintAddress, timeframe, fetchData]);

  const refresh = useCallback(() => {
    if (mintAddress) fetchData(mintAddress, timeframe);
  }, [mintAddress, timeframe, fetchData]);

  return { candles, poolAddress, status, error, refresh };
}
