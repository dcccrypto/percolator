"use client";

import { FC, useState, useMemo, useCallback, useRef, useEffect } from "react";
import { useSlabState } from "@/components/providers/SlabProvider";
import { useLivePrice } from "@/hooks/useLivePrice";
import { useTokenChart } from "@/hooks/useTokenChart";
import { ChartEmptyState } from "./ChartEmptyState";

type ChartType = "line" | "candle";
type Timeframe = "1h" | "4h" | "1d" | "7d" | "30d";

interface PricePoint {
  timestamp: number;
  price: number;
}

interface CandleData {
  timestamp: number;
  open: number;
  high: number;
  low: number;
  close: number;
  volume: number;
}

const TIMEFRAME_MS: Record<Timeframe, number> = {
  "1h": 60 * 60 * 1000,
  "4h": 4 * 60 * 60 * 1000,
  "1d": 24 * 60 * 60 * 1000,
  "7d": 7 * 24 * 60 * 60 * 1000,
  "30d": 30 * 24 * 60 * 60 * 1000,
};

const CANDLE_INTERVAL_MS = 5 * 60 * 1000; // 5-minute candles

function aggregateCandles(prices: PricePoint[], intervalMs: number): CandleData[] {
  if (prices.length === 0) return [];
  
  const candles: CandleData[] = [];
  let currentCandle: CandleData | null = null;
  
  prices.forEach((point) => {
    const candleStart = Math.floor(point.timestamp / intervalMs) * intervalMs;
    
    if (!currentCandle || currentCandle.timestamp !== candleStart) {
      if (currentCandle) candles.push(currentCandle);
      currentCandle = {
        timestamp: candleStart,
        open: point.price,
        high: point.price,
        low: point.price,
        close: point.price,
        volume: 0,
      };
    } else {
      currentCandle.high = Math.max(currentCandle.high, point.price);
      currentCandle.low = Math.min(currentCandle.low, point.price);
      currentCandle.close = point.price;
    }
  });
  
  if (currentCandle) candles.push(currentCandle);
  return candles;
}

const W = 800;
const H = 400;
const CHART_H = 300;
const VOLUME_H = 60;
const PAD = { top: 20, bottom: 40, left: 60, right: 20 };

export const TradingChart: FC<{ slabAddress: string; mintAddress?: string }> = ({
  slabAddress,
  mintAddress,
}) => {
  const { config } = useSlabState();
  const { priceUsd } = useLivePrice();
  const [chartType, setChartType] = useState<ChartType>("line");
  const [timeframe, setTimeframe] = useState<Timeframe>("1d");
  const [oraclePrices, setOraclePrices] = useState<PricePoint[]>([]);
  const [hoveredCandle, setHoveredCandle] = useState<CandleData | null>(null);
  const svgRef = useRef<SVGSVGElement>(null);

  // PERC-512: Fetch external token OHLCV from GeckoTerminal (free, no key)
  const {
    candles: externalCandles,
    status: externalStatus,
    poolAddress,
  } = useTokenChart(mintAddress ?? null, timeframe);

  const hasExternalData = externalStatus === "success" && externalCandles.length > 0;

  // Fetch oracle price history (always — used when external data unavailable)
  useEffect(() => {
    fetch(`/api/markets/${slabAddress}/prices`)
      .then((r) => r.json())
      .then((d) => {
        const apiPrices = (d.prices ?? []).map((p: { price_e6: string; timestamp: number }) => ({
          timestamp: p.timestamp,
          price: parseInt(p.price_e6) / 1e6,
        }));
        setOraclePrices(apiPrices);
      })
      .catch(() => {});
  }, [slabAddress]);

  // Add live price updates to oracle prices
  useEffect(() => {
    if (!config || !priceUsd) return;
    const now = Date.now();
    setOraclePrices((prev) => {
      const last = prev[prev.length - 1];
      if (last && now - last.timestamp < 5000) return prev;
      return [...prev, { timestamp: now, price: priceUsd }].slice(-1000);
    });
  }, [config, priceUsd]);

  // Derive oracle-based price array (filtered by timeframe)
  const oracleFiltered = useMemo(() => {
    const cutoff = Date.now() - TIMEFRAME_MS[timeframe];
    return oraclePrices.filter((p) => p.timestamp >= cutoff);
  }, [oraclePrices, timeframe]);

  // Merge external + oracle: external data is preferred when available.
  // For line chart, convert external candle close prices to PricePoints.
  const { candles, lineData } = useMemo(() => {
    if (hasExternalData) {
      // External data available: use it for both candle and line views
      const externalLine: PricePoint[] = externalCandles.map((c) => ({
        timestamp: c.timestamp,
        price: c.close,
      }));
      if (chartType === "candle") {
        return { candles: externalCandles as CandleData[], lineData: [] };
      }
      return { candles: [], lineData: externalLine };
    }

    // Fallback: oracle prices only
    if (chartType === "candle") {
      return {
        candles: aggregateCandles(oracleFiltered, CANDLE_INTERVAL_MS),
        lineData: [],
      };
    }
    return { candles: [], lineData: oracleFiltered };
  }, [hasExternalData, externalCandles, oracleFiltered, chartType]);

  // Calculate chart bounds
  const { minPrice, maxPrice, minTime, maxTime, priceRange } = useMemo(() => {
    const data = chartType === "candle" ? candles : lineData;
    if (data.length === 0) {
      return { minPrice: 0, maxPrice: 0, minTime: 0, maxTime: 0, priceRange: 0 };
    }

    let min = Infinity;
    let max = -Infinity;
    let tMin = Infinity;
    let tMax = -Infinity;

    data.forEach((d) => {
      if ("high" in d && "low" in d) {
        min = Math.min(min, d.low);
        max = Math.max(max, d.high);
      } else if ("price" in d) {
        min = Math.min(min, d.price);
        max = Math.max(max, d.price);
      }
      tMin = Math.min(tMin, d.timestamp);
      tMax = Math.max(tMax, d.timestamp);
    });

    // Add padding if price is stable
    const rawRange = max - min;
    const avg = (min + max) / 2;
    if (rawRange < avg * 0.001 || rawRange === 0) {
      const padding = avg * 0.01;
      min = avg - padding;
      max = avg + padding;
    }

    return {
      minPrice: min,
      maxPrice: max,
      minTime: tMin,
      maxTime: tMax,
      priceRange: max - min,
    };
  }, [candles, lineData, chartType]);

  const CHART_W = W - PAD.left - PAD.right;

  // Render line chart
  const linePath = useMemo(() => {
    if (chartType !== "line" || lineData.length === 0) return "";
    
    const timeRange = maxTime - minTime || 1;
    const safePriceRange = priceRange || 1;
    const points = lineData.map((p) => {
      const x = PAD.left + ((p.timestamp - minTime) / timeRange) * CHART_W;
      const y = PAD.top + ((maxPrice - p.price) / safePriceRange) * CHART_H;
      return `${x},${y}`;
    });
    
    return points.join(" ");
  }, [lineData, minTime, maxTime, minPrice, maxPrice, priceRange, CHART_W, chartType]);

  // Y-axis labels
  const yLabels = useMemo(() => {
    const labels: { y: number; value: number }[] = [];
    const count = 5;
    for (let i = 0; i <= count; i++) {
      const price = maxPrice - (priceRange * i) / count;
      const y = PAD.top + (i / count) * CHART_H;
      labels.push({ y, value: price });
    }
    return labels;
  }, [minPrice, maxPrice, priceRange]);

  // X-axis labels
  const xLabels = useMemo(() => {
    const labels: { x: number; time: string }[] = [];
    const count = 6;
    const timeRange = maxTime - minTime || 1;
    for (let i = 0; i <= count; i++) {
      const t = minTime + (timeRange * i) / count;
      const date = new Date(t);
      const x = PAD.left + (i / count) * CHART_W;
      const format =
        timeframe === "1h" || timeframe === "4h" || timeframe === "1d"
          ? date.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })
          : date.toLocaleDateString([], { month: "short", day: "numeric" });
      labels.push({ x, time: format });
    }
    return labels;
  }, [minTime, maxTime, timeframe, CHART_W]);

  // Use external line data or oracle line data for price stats
  const activeLineData = lineData.length > 0 ? lineData : oracleFiltered;
  const currentPrice = activeLineData[activeLineData.length - 1]?.price ?? priceUsd ?? 0;
  const firstPrice = activeLineData[0]?.price ?? currentPrice;
  const priceChange = currentPrice - firstPrice;
  const priceChangePercent = firstPrice > 0 ? (priceChange / firstPrice) * 100 : 0;
  const isUp = priceChange >= 0;

  // Show empty state only if BOTH external and oracle data are missing
  const totalDataPoints = lineData.length + candles.length;
  if (totalDataPoints === 0) {
    return (
      <ChartEmptyState
        currentPrice={priceUsd ?? undefined}
        heightClass="h-[200px] sm:h-[400px]"
      />
    );
  }

  return (
    <div className="rounded-none border border-[var(--border)] bg-[var(--bg)] p-3">
      {/* Header — wraps on small mobile so controls don't overflow viewport (#860) */}
      <div className="mb-3 flex flex-wrap items-start justify-between gap-y-2">
        <div className="min-w-0">
          <div className="text-2xl font-bold" style={{ fontFamily: "var(--font-mono)", fontVariantNumeric: "tabular-nums", color: isUp ? "var(--long)" : "var(--short)" }}>
            ${currentPrice.toFixed(currentPrice < 1 ? 4 : 2)}
          </div>
          <div className="flex items-center gap-2">
            <span className="text-xs" style={{ color: isUp ? "var(--long)" : "var(--short)" }}>
              {isUp ? "+" : ""}{priceChange.toFixed(4)} ({isUp ? "+" : ""}{priceChangePercent.toFixed(2)}%)
            </span>
            {/* PERC-512: Data source badge */}
            {hasExternalData ? (
              <span
                className="text-[9px] font-medium uppercase tracking-[0.08em] px-1.5 py-0.5 rounded-sm"
                style={{ background: "var(--accent)/0.1", color: "var(--accent)", border: "1px solid color-mix(in srgb, var(--accent) 30%, transparent)" }}
                title={poolAddress ? `GeckoTerminal pool: ${poolAddress}` : "Source: GeckoTerminal"}
              >
                DEX
              </span>
            ) : (
              mintAddress && externalStatus !== "idle" && (
                <span
                  className="text-[9px] font-medium uppercase tracking-[0.08em] px-1.5 py-0.5 rounded-sm"
                  style={{ background: "var(--bg-elevated)", color: "var(--text-dim)", border: "1px solid var(--border)" }}
                  title="Showing oracle price history (no DEX data found)"
                >
                  Oracle
                </span>
              )
            )}
          </div>
        </div>

        {/* Controls — shrink-wrap so they don't force parent wider than viewport */}
        <div className="flex flex-wrap items-center gap-2">
          {/* Chart type */}
          <div className="flex gap-1 rounded-none border border-[var(--border)] bg-[var(--bg-elevated)] p-0.5">
            <button
              onClick={() => setChartType("line")}
              className={`rounded-none px-2 py-1 text-xs transition-colors ${
                chartType === "line"
                  ? "bg-[var(--accent)]/10 text-[var(--accent)]"
                  : "text-[var(--text-dim)] hover:text-[var(--text-secondary)]"
              }`}
            >
              Line
            </button>
            <button
              onClick={() => setChartType("candle")}
              className={`rounded-none px-2 py-1 text-xs transition-colors ${
                chartType === "candle"
                  ? "bg-[var(--accent)]/10 text-[var(--accent)]"
                  : "text-[var(--text-dim)] hover:text-[var(--text-secondary)]"
              }`}
            >
              Candle
            </button>
          </div>

          {/* Timeframe */}
          <div className="flex gap-1 rounded-none border border-[var(--border)] bg-[var(--bg-elevated)] p-0.5">
            {(["1h", "4h", "1d", "7d", "30d"] as Timeframe[]).map((tf) => (
              <button
                key={tf}
                onClick={() => setTimeframe(tf)}
                className={`rounded-none px-2 py-1 text-xs transition-colors ${
                  timeframe === tf
                    ? "bg-[var(--accent)]/10 text-[var(--accent)]"
                    : "text-[var(--text-dim)] hover:text-[var(--text-secondary)]"
                }`}
              >
                {tf}
              </button>
            ))}
          </div>
        </div>
      </div>

      {/* Chart */}
      <svg ref={svgRef} viewBox={`0 0 ${W} ${H}`} className="w-full h-auto max-h-[200px] sm:max-h-[400px]" preserveAspectRatio="xMidYMid meet">
        <defs>
          <linearGradient id="lineGradient" x1="0" x2="0" y1="0" y2="1">
            <stop offset="0%" stopColor={isUp ? "var(--long)" : "var(--short)"} stopOpacity="0.3" />
            <stop offset="100%" stopColor={isUp ? "var(--long)" : "var(--short)"} stopOpacity="0" />
          </linearGradient>
        </defs>

        {/* Grid lines */}
        {yLabels.map((label, i) => (
          <line
            key={`grid-y-${i}`}
            x1={PAD.left}
            x2={W - PAD.right}
            y1={label.y}
            y2={label.y}
            stroke="var(--border)"
            strokeWidth="1"
            strokeDasharray="4 4"
            opacity="0.3"
          />
        ))}

        {/* Y-axis labels */}
        {yLabels.map((label, i) => (
          <text
            key={`label-y-${i}`}
            x={PAD.left - 10}
            y={label.y + 4}
            textAnchor="end"
            fontSize="10"
            fill="var(--text-dim)"
            fontFamily="var(--font-mono)"
          >
            ${label.value.toFixed(label.value < 1 ? 4 : 2)}
          </text>
        ))}

        {/* X-axis labels */}
        {xLabels.map((label, i) => (
          <text
            key={`label-x-${i}`}
            x={label.x}
            y={PAD.top + CHART_H + 20}
            textAnchor="middle"
            fontSize="10"
            fill="var(--text-dim)"
          >
            {label.time}
          </text>
        ))}

        {/* Line chart */}
        {chartType === "line" && linePath && (
          <>
            <polygon
              points={`${linePath} ${W - PAD.right},${PAD.top + CHART_H} ${PAD.left},${PAD.top + CHART_H}`}
              fill="url(#lineGradient)"
            />
            <polyline
              points={linePath}
              fill="none"
              stroke={isUp ? "var(--long)" : "var(--short)"}
              strokeWidth="2"
              strokeLinejoin="round"
            />
          </>
        )}

        {/* Candlestick chart */}
        {chartType === "candle" &&
          candles.map((candle, i) => {
            const timeRange = maxTime - minTime || 1;
            const x = PAD.left + ((candle.timestamp - minTime) / timeRange) * CHART_W;
            const safePriceRange = priceRange || 1;
            const yOpen = PAD.top + ((maxPrice - candle.open) / safePriceRange) * CHART_H;
            const yClose = PAD.top + ((maxPrice - candle.close) / safePriceRange) * CHART_H;
            const yHigh = PAD.top + ((maxPrice - candle.high) / safePriceRange) * CHART_H;
            const yLow = PAD.top + ((maxPrice - candle.low) / safePriceRange) * CHART_H;
            const candleW = Math.max(2, CHART_W / candles.length - 2);
            const isGreen = candle.close >= candle.open;
            const color = isGreen ? "var(--long)" : "var(--short)";

            return (
              <g key={i}>
                {/* Wick */}
                <line x1={x} x2={x} y1={yHigh} y2={yLow} stroke={color} strokeWidth="1" />
                {/* Body */}
                <rect
                  x={x - candleW / 2}
                  y={Math.min(yOpen, yClose)}
                  width={candleW}
                  height={Math.max(1, Math.abs(yClose - yOpen))}
                  fill={color}
                  opacity="0.9"
                />
              </g>
            );
          })}
      </svg>
    </div>
  );
};
