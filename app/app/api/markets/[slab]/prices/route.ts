import { NextRequest, NextResponse } from "next/server";
import { getServiceClient } from "@/lib/supabase";

export const dynamic = "force-dynamic";

/**
 * GET /api/markets/[slab]/prices
 *
 * Returns price history for a market slab address.
 * Reads from oracle_prices or market_stats tables.
 */
export async function GET(
  _req: NextRequest,
  { params }: { params: Promise<{ slab: string }> }
) {
  try {
    const { slab } = await params;

    if (!slab || slab.length < 20) {
      return NextResponse.json({ prices: [] });
    }

    const db = getServiceClient();
    if (!db) {
      return NextResponse.json({ prices: [] });
    }

    // 1. Check oracle_prices (stats collector writes here)
    const { data: oraclePrices, error: oracleError } = await (db as any)
      .from("oracle_prices")
      .select("price_e6, timestamp")
      .eq("slab_address", slab)
      .order("timestamp", { ascending: true })
      .limit(500);

    if (!oracleError && oraclePrices && oraclePrices.length > 0) {
      return NextResponse.json({
        prices: oraclePrices.map((p: any) => ({
          price_e6: String(p.price_e6),
          timestamp: p.timestamp,
        })),
      });
    }

    // 2. Fallback: market_stats for the most recent price
    const { data: stats } = await (db as any)
      .from("market_stats")
      .select("mark_price, updated_at")
      .eq("slab_address", slab)
      .order("updated_at", { ascending: false })
      .limit(1);

    if (stats && stats.length > 0) {
      const markPriceUsd = stats[0].mark_price as number | null | undefined;
      const updatedAt = stats[0].updated_at as string | null | undefined;

      // Convert USD price -> e6 integer string (match oracle_prices schema)
      const priceE6 =
        typeof markPriceUsd === "number" && Number.isFinite(markPriceUsd)
          ? String(Math.round(markPriceUsd * 1_000_000))
          : "0";

      const ts = updatedAt ? new Date(updatedAt).getTime() : 0;

      return NextResponse.json({
        prices: [{ price_e6: priceE6, timestamp: ts }],
      });
    }

    return NextResponse.json({ prices: [] });
  } catch (err) {
    console.error("[prices] Error:", err);
    return NextResponse.json({ prices: [] });
  }
}
