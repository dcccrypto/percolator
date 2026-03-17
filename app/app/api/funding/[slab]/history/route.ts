import { type NextRequest, NextResponse } from "next/server";
import { proxyToApi } from "@/lib/api-proxy";
import { BLOCKED_SLAB_ADDRESSES } from "@/lib/blocklist";

export const dynamic = "force-dynamic";

/**
 * Blocked slab set: hardcoded list + env var runtime overrides.
 * Mirrors the guard in /api/markets/route.ts and /api/funding/[slab]/route.ts.
 */
const BLOCKED_MARKET_ADDRESSES: ReadonlySet<string> = new Set([
  ...BLOCKED_SLAB_ADDRESSES,
  ...(process.env.BLOCKED_MARKET_ADDRESSES ?? "")
    .split(",")
    .map((s) => s.trim())
    .filter(Boolean),
]);

/**
 * GET /api/funding/[slab]/history
 *
 * Proxies to percolator-api GET /funding/:slab/history
 * Removed standalone Supabase impl (GH#1066 — arch cleanup).
 *
 * GH#1357: Return 404 for blocklisted slabs instead of proxying to
 * backend which returns 500 for invalid/corrupt slab addresses.
 */
export async function GET(
  req: NextRequest,
  { params }: { params: Promise<{ slab: string }> }
) {
  const { slab } = await params;

  if (BLOCKED_MARKET_ADDRESSES.has(slab)) {
    return NextResponse.json(
      { error: "Market not found" },
      { status: 404 }
    );
  }

  return proxyToApi(req, `/funding/${slab}/history`);
}
