import { type NextRequest, NextResponse } from "next/server";
import { proxyToApi } from "@/lib/api-proxy";
import { isBlockedSlab } from "@/lib/blocklist";

export const dynamic = "force-dynamic";

/**
 * Re-exported for backwards-compat: components that import this type from this
 * route module continue to compile after the route became a proxy (GH#1066).
 */
export interface FundingGlobalEntry {
  slabAddress: string;
  baseSymbol: string | null;
  rateBpsPerSlot: number;
  hourlyRatePercent: number;
  dailyRatePercent: number;
  dailyRateAbs?: number;
  netLpPos?: number;
}

/**
 * GET /api/funding/global
 *
 * Proxies to percolator-api GET /funding/global, then applies an additional
 * defense-in-depth filter to strip any blocked slabs from the response.
 *
 * GH#1461: Even when the Railway API fix (PR #1460) applies isBlockedSlab()
 * server-side, deploy lag or env-var misconfiguration can let blocked slabs
 * slip through. This layer guarantees they are stripped before Vercel serves
 * the response to the browser.
 *
 * Removed standalone Supabase impl (GH#1066 — arch cleanup).
 */
export async function GET(req: NextRequest) {
  const upstream = await proxyToApi(req, "/funding/global");

  // Only post-process successful JSON responses (2xx).
  // For errors/timeouts, pass the upstream status through unchanged.
  if (!upstream.ok) return upstream;

  let data: Record<string, unknown>;
  try {
    data = await upstream.clone().json();
  } catch {
    // If the body isn't JSON (shouldn't happen but be safe), pass through.
    return upstream;
  }

  // GH#1461: Strip blocked slabs from the global list.
  // This is a defense-in-depth guard — the Railway API applies the same filter
  // (PR #1460) but Vercel-layer filtering ensures correctness even when Railway
  // hasn't redeployed or env BLOCKED_MARKET_ADDRESSES is misconfigured.
  if (Array.isArray(data.markets)) {
    const filtered = (data.markets as Array<{ slabAddress?: string }>).filter(
      (m) => !isBlockedSlab(m.slabAddress)
    );
    data = { ...data, markets: filtered, count: filtered.length };
  }

  const upstreamCacheControl =
    upstream.headers.get("Cache-Control") ?? "no-store, max-age=0";

  return NextResponse.json(data, {
    status: upstream.status,
    headers: {
      "Cache-Control": upstreamCacheControl,
    },
  });
}
