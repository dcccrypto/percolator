import { type NextRequest } from "next/server";
import { proxyToApi } from "@/lib/api-proxy";

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
 * Proxies to percolator-api GET /funding/global
 * Removed standalone Supabase impl (GH#1066 — arch cleanup).
 */
export async function GET(req: NextRequest) {
  return proxyToApi(req, "/funding/global");
}
