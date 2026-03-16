"use client";

import { useEffect, useRef, useState, useCallback } from "react";
import { useSlabState } from "@/components/providers/SlabProvider";
import { detectOracleMode, type OracleMode } from "@/lib/oraclePrice";

// GH#1338: "unavailable" = oracle has never been cranked (no valid price exists on-chain).
// Distinct from "stale" (had a price, but it's old). Unavailable → hard block on trading.
export type FreshnessLevel = "fresh" | "aging" | "stale" | "unavailable";

export interface OracleFreshnessState {
  /** Oracle mode for this market */
  mode: OracleMode | null;
  /** Display label for the oracle mode */
  modeLabel: string;
  /** Seconds since last oracle price update */
  elapsedSecs: number;
  /** Freshness level based on thresholds */
  level: FreshnessLevel;
  /** CSS color variable for the current freshness level */
  color: string;
  /** Number of active publishers (if known) */
  publisherCount: number | null;
  /** Total publishers (if known) */
  publisherTotal: number | null;
  /** Whether we have valid oracle data */
  ready: boolean;
  /** Last update timestamp (ms) */
  lastUpdateMs: number | null;
}

/** Freshness thresholds in seconds */
const FRESH_THRESHOLD = 5;
const AGING_THRESHOLD = 30;

function getFreshnessLevel(elapsedSecs: number): FreshnessLevel {
  if (elapsedSecs < FRESH_THRESHOLD) return "fresh";
  if (elapsedSecs <= AGING_THRESHOLD) return "aging";
  return "stale";
}

function getFreshnessColor(level: FreshnessLevel): string {
  switch (level) {
    case "fresh":
      return "#22c55e";
    case "aging":
      return "#eab308";
    case "stale":
      return "#ef4444";
    case "unavailable":
      return "#ef4444";
  }
}

function getModeLabel(mode: OracleMode): string {
  switch (mode) {
    case "hyperp":
      return "HYPERP";
    case "pyth-pinned":
      return "PYTH";
    case "admin":
      return "ADMIN";
  }
}

/**
 * Track oracle price freshness for the current market.
 *
 * For admin mode: uses authorityTimestamp (real unix timestamp).
 * For hyperp/pyth modes: tracks when lastEffectivePriceE6 last changed.
 */
export function useOracleFreshness(): OracleFreshnessState {
  const { config } = useSlabState();
  const [elapsedSecs, setElapsedSecs] = useState(0);
  const [lastUpdateMs, setLastUpdateMs] = useState<number | null>(null);
  const prevPriceRef = useRef<bigint | null>(null);
  const tickRef = useRef<ReturnType<typeof setInterval>>(undefined);

  // Guard: detectOracleMode requires both PublicKey fields — skip if either is missing
  // (e.g. partial mock configs in test environments).
  const hasOracleKeys = config?.oracleAuthority != null && config?.indexFeedId != null;
  const mode = (config && hasOracleKeys)
    ? detectOracleMode(config)
    : null;

  // Track price changes to detect updates
  useEffect(() => {
    if (!config || !hasOracleKeys) return;
    const currentMode = detectOracleMode(config);

    if (currentMode === "admin") {
      // Admin mode: authorityTimestamp is a real unix timestamp
      const ts = config.authorityTimestamp;
      if (ts > 0n) {
        setLastUpdateMs(Number(ts) * 1000);
      } else {
        // Fallback: admin price is set but timestamp is zero (legacy/static markets).
        // Use authorityPriceE6 (the canonical admin price, matching resolveMarketPriceE6)
        // and fall back to lastEffectivePriceE6 only if authority price is zero.
        const adminPrice = config.authorityPriceE6 > 0n
          ? config.authorityPriceE6
          : config.lastEffectivePriceE6;
        if (adminPrice > 0n) {
          // Reset freshness on each observed price change so the elapsed timer restarts.
          if (prevPriceRef.current !== null && adminPrice !== prevPriceRef.current) {
            setLastUpdateMs(Date.now());
          } else if (prevPriceRef.current === null) {
            // First load — assume relatively fresh
            setLastUpdateMs(Date.now());
          }
          prevPriceRef.current = adminPrice;
        }
      }
    } else {
      // Hyperp: track authorityPriceE6 (updated by UpdateHyperpMark instruction).
      // Pyth-pinned: track lastEffectivePriceE6 (updated by KeeperCrank).
      // NOTE: Do NOT use authorityTimestamp here — for Hyperp mode that field
      // stores the funding rate, not a unix timestamp.
      const currentPrice = currentMode === "hyperp"
        ? config.authorityPriceE6
        : config.lastEffectivePriceE6;

      // GH#1338: For hyperp mode, also check lastEffectivePriceE6 (index price from
      // on-chain crank). If it's 0, the oracle-keeper has never cranked this market —
      // the oracle is genuinely unavailable, not just stale. Don't assume "first load = fresh".
      if (currentMode === "hyperp" && config.lastEffectivePriceE6 === 0n) {
        // Oracle has never been cranked — mark as unavailable (lastUpdateMs stays null)
        prevPriceRef.current = currentPrice;
        return;
      }

      if (prevPriceRef.current !== null && currentPrice !== prevPriceRef.current) {
        setLastUpdateMs(Date.now());
      } else if (prevPriceRef.current === null && currentPrice > 0n) {
        // First load — assume relatively fresh
        setLastUpdateMs(Date.now());
      }
      prevPriceRef.current = currentPrice;
    }
  }, [config]);

  // Tick every second to update elapsed time
  useEffect(() => {
    if (lastUpdateMs === null) return;

    const tick = () => {
      const elapsed = Math.max(0, Math.floor((Date.now() - lastUpdateMs) / 1000));
      setElapsedSecs(elapsed);
    };
    tick();
    tickRef.current = setInterval(tick, 1000);
    return () => clearInterval(tickRef.current);
  }, [lastUpdateMs]);

  // GH#1338: If mode is detected but we never got a lastUpdateMs, the oracle is unavailable
  // (e.g. hyperp market never cranked). This is distinct from stale (had a price but it's old).
  const isUnavailable = mode !== null && lastUpdateMs === null;
  const level = isUnavailable ? "unavailable" as FreshnessLevel : getFreshnessLevel(elapsedSecs);

  return {
    mode,
    modeLabel: mode ? getModeLabel(mode) : "",
    elapsedSecs,
    level,
    color: getFreshnessColor(level),
    // Publisher data now fetched dynamically by useOraclePublishers hook (PERC-371)
    publisherCount: null,
    publisherTotal: null,
    ready: mode !== null && lastUpdateMs !== null,
    lastUpdateMs,
  };
}
