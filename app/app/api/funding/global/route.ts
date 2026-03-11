import { NextResponse } from "next/server";
import { getServiceClient } from "@/lib/supabase";

export const dynamic = "force-dynamic";

/**
 * GET /api/funding/global
 *
 * Returns top markets by funding rate magnitude for the Funding Rates dashboard widget.
 * Reads from market_stats + markets tables.
 *
 * Query params:
 *   limit — max number of results (default 10, max 50)
 */

const MAX_FUNDING_RATE_BPS = 10_000;
const SLOTS_PER_HOUR = 9000;

function sanitize(raw: unknown): number {
  const n = Number(raw ?? 0);
  if (!Number.isFinite(n) || Math.abs(n) > MAX_FUNDING_RATE_BPS) return 0;
  return n;
}

export interface FundingGlobalEntry {
  slabAddress: string;
  baseSymbol: string | null;
  rateBpsPerSlot: number;
  hourlyRatePercent: number;
  dailyRatePercent: number;
  direction: "long_pays_short" | "short_pays_long" | "neutral";
}

export async function GET(req: Request) {
  try {
    const url = new URL(req.url);
    const limit = Math.min(50, Math.max(1, Number(url.searchParams.get("limit") ?? "10")));

    const db = getServiceClient();

    // Join market_stats with markets to get base symbol
    const { data, error } = await db
      .from("market_stats")
      .select("slab_address, funding_rate, markets(base_symbol)")
      .not("funding_rate", "is", null)
      .order("funding_rate", { ascending: false });

    if (error) {
      console.error("[/api/funding/global] DB error:", error);
      return NextResponse.json({ error: "Database error" }, { status: 500 });
    }

    type Row = {
      slab_address: string;
      funding_rate: number | null;
      markets: { base_symbol: string | null } | { base_symbol: string | null }[] | null;
    };

    const rows = (data ?? []) as Row[];

    // Build entries, sanitize, compute derived fields
    const entries: FundingGlobalEntry[] = rows.map((row) => {
      const rateBps = sanitize(row.funding_rate);
      const hourlyRatePercent = (rateBps * SLOTS_PER_HOUR) / 10000;
      const m = Array.isArray(row.markets) ? row.markets[0] : row.markets;
      return {
        slabAddress: row.slab_address,
        baseSymbol: m?.base_symbol ?? null,
        rateBpsPerSlot: rateBps,
        hourlyRatePercent,
        dailyRatePercent: hourlyRatePercent * 24,
        direction:
          rateBps > 0
            ? "long_pays_short"
            : rateBps < 0
              ? "short_pays_long"
              : "neutral",
      };
    });

    // Sort by absolute rate (highest first), take top N
    const sorted = entries
      .sort((a, b) => Math.abs(b.rateBpsPerSlot) - Math.abs(a.rateBpsPerSlot))
      .slice(0, limit);

    return NextResponse.json({ markets: sorted });
  } catch (e) {
    console.error("[/api/funding/global] Error:", e);
    return NextResponse.json({ error: "Internal error" }, { status: 500 });
  }
}
