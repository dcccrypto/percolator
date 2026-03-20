import { NextRequest, NextResponse } from "next/server";
import { PublicKey } from "@solana/web3.js";
import { getServiceClient } from "@/lib/supabase";
import { validateSlabParam } from "@/lib/route-validators";
import { SLUG_ALIASES } from "@/lib/symbol-utils";
import { isBlockedSlab } from "@/lib/blocklist";
import * as Sentry from "@sentry/nextjs";

/**
 * GH#1405: Sanitize price fields from the DB. Returns null for corrupt/garbage values.
 * Matches the MAX_SANE_PRICE_USD guard in /api/markets route.ts ($1M ceiling).
 * Prevents raw unscaled admin oracle values (e.g. 10001100011 for DfLoAzny) from
 * being returned to callers of the individual slab endpoint.
 */
const MAX_SANE_PRICE_USD = 1_000_000; // $1M — matches bulk /api/markets guard
function sanitizePrice(v: unknown): number | null {
  if (v == null || typeof v !== "number") return null;
  if (!Number.isFinite(v) || v <= 0 || v > MAX_SANE_PRICE_USD) return null;
  return v;
}
export const dynamic = "force-dynamic";

function isValidPublicKey(s: string): boolean {
  try {
    new PublicKey(s);
    return true;
  } catch {
    return false;
  }
}

/**
 * GET /api/markets/[slab]
 * Accepts either a base58 slab address OR a market slug (e.g. "SOL-PERP", "SOL").
 * When a slug is given, resolves it by matching the `symbol` column (case-insensitive,
 * with optional "-PERP" suffix stripped).
 * 
 * MEDIUM-003: Added slab parameter validation.
 */
export async function GET(
  _req: NextRequest,
  { params }: { params: Promise<{ slab: string }> }
) {
  const { slab } = await params;

  // Validate slab parameter format (unless it's a slug like "SOL" or "SOL-PERP")
  // Slugs are 1-6 chars alphanumeric+dash; PublicKey addresses are 43-44 chars base58
  if (slab.length > 10) {
    const validation = validateSlabParam(slab);
    if (!validation.valid) {
      return validation.response;
    }
  }

  // GH#1417: Blocked slabs must return 404 even when addressed directly.
  // The bulk /api/markets endpoint already filters via BLOCKED_SLAB_ADDRESSES but
  // this individual endpoint had no guard, allowing blocked addresses (e.g. 8eFFEFBY)
  // to return HTTP 200 with phantom data.
  if (isBlockedSlab(slab)) {
    return NextResponse.json({ error: "Market not found" }, { status: 404 });
  }

  try {
    const supabase = getServiceClient();
    let data: Record<string, unknown> | null = null;

    if (isValidPublicKey(slab)) {
      // Standard lookup by slab address
      const { data: row, error } = await supabase
        .from("markets_with_stats")
        .select("*")
        .eq("slab_address", slab)
        .maybeSingle();

      if (error) {
        Sentry.captureException(error, {
          tags: { endpoint: "/api/markets/[slab]", method: "GET", slab },
        });
        return NextResponse.json({ error: error.message }, { status: 500 });
      }
      data = row;
    } else {
      // Slug resolution: strip "-PERP" suffix and match symbol case-insensitively
      const slugNorm = slab.toUpperCase().replace(/-PERP$/, "");

      // Fetch all markets and filter in JS to avoid needing ilike + function indexes
      const { data: rows, error } = await supabase
        .from("markets_with_stats")
        .select("*");

      if (error) {
        Sentry.captureException(error, {
          tags: { endpoint: "/api/markets/[slab]", method: "GET", slab },
        });
        return NextResponse.json({ error: error.message }, { status: 500 });
      }

      // Sort to prefer the most active slab when multiple markets share the same
      // symbol / mint (e.g. 25 SOL devnet markets).
      // Rule: treat volume_24h=0 and volume_24h=null identically as "no volume" (-1)
      // so a dead slab with explicit vol=0 never beats a fresh slab with vol=null.
      // Tiebreakers: total_open_interest DESC, then created_at DESC (newest wins).
      const sorted = (rows ?? []).slice().sort((a: Record<string, unknown>, b: Record<string, unknown>) => {
        const va = typeof a.volume_24h === "number" && (a.volume_24h as number) > 0 ? (a.volume_24h as number) : -1;
        const vb = typeof b.volume_24h === "number" && (b.volume_24h as number) > 0 ? (b.volume_24h as number) : -1;
        if (vb !== va) return vb - va;
        const oa = typeof a.total_open_interest === "number" && (a.total_open_interest as number) > 0 ? (a.total_open_interest as number) : -1;
        const ob = typeof b.total_open_interest === "number" && (b.total_open_interest as number) > 0 ? (b.total_open_interest as number) : -1;
        if (ob !== oa) return ob - oa;
        return new Date(String(b.created_at ?? 0)).getTime() - new Date(String(a.created_at ?? 0)).getTime();
      });

      // 1. Try symbol match (e.g. DB symbol = "SOL-PERP")
      let match = sorted.find((m: Record<string, unknown>) => {
        const sym = String(m.symbol ?? "").toUpperCase().replace(/-PERP$/, "");
        return sym === slugNorm || String(m.symbol ?? "").toUpperCase() === slab.toUpperCase();
      });

      // 2. Fallback: well-known mint alias (e.g. SOL → So111...)
      if (!match) {
        const aliasMint = SLUG_ALIASES[slugNorm];
        if (aliasMint) {
          match = sorted.find((m: Record<string, unknown>) => m.mint_address === aliasMint);
        }
      }

      data = match ?? null;
    }

    if (!data) {
      return NextResponse.json({ error: "Market not found" }, { status: 404 });
    }

    // GH#1444: Re-check blocklist after symbol/slug resolution.
    // The initial guard at the top only checks the raw URL param (e.g. "DfLoAzny"),
    // which may not be in the blocklist. After DB lookup, `data.slab_address` holds the
    // resolved on-chain address (e.g. "8eFFEFBY3...") which IS blocked. Without this
    // second check, symbol-addressed requests bypass the blocklist entirely.
    if (isBlockedSlab(String(data.slab_address ?? ""))) {
      return NextResponse.json({ error: "Market not found" }, { status: 404 });
    }

    // GH#1405: Sanitize price fields before returning — raw DB values from admin-mode
    // markets may be unscaled u64 authorityPriceE6 values (e.g. DfLoAzny: 10001100011).
    // Matches the sanitizePrice guard in the bulk /api/markets endpoint.
    const sanitized = {
      ...data,
      last_price: sanitizePrice(data.last_price),
      mark_price: sanitizePrice(data.mark_price),
      index_price: sanitizePrice(data.index_price),
    };

    return NextResponse.json({ market: sanitized });
  } catch (error) {
    Sentry.captureException(error, {
      tags: { endpoint: "/api/markets/[slab]", method: "GET", slab },
    });
    return NextResponse.json(
      { error: "Internal server error" },
      { status: 500 }
    );
  }
}
