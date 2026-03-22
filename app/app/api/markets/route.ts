import { NextRequest, NextResponse } from "next/server";
// requireAuth removed from POST — on-chain admin verification is sufficient
import { Connection, PublicKey } from "@solana/web3.js";
import { validateNumericParam } from "@/lib/route-validators";
import { parseHeader } from "@percolator/sdk";
import { getServiceClient } from "@/lib/supabase";
import { getConfig } from "@/lib/config";
import * as Sentry from "@sentry/nextjs";
import { isSaneMarketValue, isActiveMarket, isZombieMarket } from "@/lib/activeMarketFilter";
import { isPhantomOpenInterest } from "@/lib/phantom-oi";
import { BLOCKED_SLAB_ADDRESSES } from "@/lib/blocklist";
import { SLUG_ALIASES } from "@/lib/symbol-utils";

/**
 * GH#1526: Map frontend oracle_mode filter values to DB-stored values.
 * The UI displays "manual" and "live feed" but the DB stores "admin" and "hyperp".
 * Without this map the filter returns 0 results for any value except "admin".
 */
const ORACLE_MODE_FRONTEND_TO_DB: Record<string, string> = {
  manual: "admin",
  live_feed: "hyperp",
  // Pass-through values (already DB canonical)
  admin: "admin",
  hyperp: "hyperp",
  pyth: "pyth",
};

/**
 * GH#1527: Build a reverse lookup from mint address → well-known ticker symbol.
 * Used to make search match "SOL" even when DB stores symbol="So111111".
 * Derived from SLUG_ALIASES (single source of truth).
 */
const MINT_TO_KNOWN_SYMBOL: Map<string, string> = new Map(
  Object.entries(SLUG_ALIASES).map(([symbol, mint]) => [mint, symbol]),
);

/**
 * Maximum valid funding rate in bps/slot (matches on-chain guard).
 * Raw DB values outside [-MAX, MAX] are garbage from uninitialized slabs.
 */
const FUNDING_RATE_BPS_MAX = 10_000;

/** Cap per-market USD contribution — prevents sentinel leakage ($10B > any real market). */
const MAX_PER_MARKET_USD = 10_000_000_000;

/**
 * GH#1208: Cap for c_tot raw value.
 * c_tot is LP collateral in token micro-units. Even the deepest devnet vault
 * would not exceed $100M USD at any reasonable token price. Raw cap at 5e17
 * catches near-sentinel corrupted values (e.g. 7.997e17) that slip through
 * the isSaneMarketValue 1e18 threshold.
 */
const MAX_SANE_C_TOT = 5e17;

/**
 * Return null for c_tot values that are clearly corrupted.
 * Does NOT convert to USD — just guards the raw value.
 */
function sanitizeCtot(v: number | null | undefined): number | null {
  if (v == null) return null;
  if (!Number.isFinite(v) || v < 0 || v > MAX_SANE_C_TOT) return null;
  return v;
}

/**
 * Convert a raw on-chain token micro-unit amount to USD.
 * Returns null when the raw value is a sentinel/garbage or no price is available.
 * (#1160: expose a pre-computed USD field so API consumers don't have to divide by 10^decimals themselves)
 */
function rawToUsd(raw: number | null | undefined, decimals: number | null | undefined, priceUsd: number | null | undefined): number | null {
  if (!isSaneMarketValue(raw)) return null;
  const d = Math.min(Math.max(decimals ?? 6, 0), 18);
  const p = priceUsd ?? 0;
  if (p <= 0) return null;
  const usd = (raw! / 10 ** d) * p;
  return usd > MAX_PER_MARKET_USD ? null : usd;
}

/** Sanitize a numeric funding_rate from the DB view. Returns null for garbage values. */
function sanitizeFundingRate(v: number | null | undefined): number | null {
  if (v == null) return null;
  if (!Number.isFinite(v) || Math.abs(v) > FUNDING_RATE_BPS_MAX) return null;
  return v;
}

/**
 * Maximum sane mark/last price in USD for API output.
 * Set at $1M — well above any real crypto price today (BTC ~$100K) but below
 * the unscaled admin-set test garbage values (e.g. $100M, $900M, $7.9T).
 * Note: Rust MAX_ORACLE_PRICE is $1B; this is a stricter display-layer guard. (#856)
 */
const MAX_SANE_PRICE_USD = 1_000_000; // $1M

/**
 * Sanitize a price field from the DB (USD float). Returns null for corrupt/garbage values.
 * Logs a Sentry warning when sanitization fires so we can track data quality. (#882)
 *
 * Fingerprinted per (field, slab) so repeated sanitizations from the same bad market
 * collapse into ONE Sentry issue rather than one event per API poll cycle (#PERC-801).
 *
 * Known causes of sanitization:
 *  - Admin-mode markets with on-chain authorityPriceE6 set to garbage/test values
 *    (e.g. value > MAX_SANE_PRICE_USD × 1e6 on-chain, e.g. GYpukkn94, 2Zta2EPRR)
 *  - HYPERP markets without oracle_markets entries — oracle-keeper can't crank them,
 *    stale/uninitialised lastEffectivePriceE6 leaks through StatsCollector
 *  Fix: seed oracle_markets table (migration 041) for HYPERP markets, or correct the
 *  admin oracle price via the admin UI (pushPrice action with correct price_e6).
 */
function sanitizePrice(v: number | null | undefined, field?: string, slabAddress?: string): number | null {
  if (v == null) return null;
  if (!Number.isFinite(v) || v <= 0 || v > MAX_SANE_PRICE_USD) {
    Sentry.captureMessage(
      `Price sanitization: ${field ?? "price"} nulled for slab ${slabAddress ?? "unknown"} (value=${v})`,
      {
        level: "warning",
        tags: { endpoint: "/api/markets", sanitization: "price", field: field ?? "price" },
        // Fingerprint collapses all events for the same bad (field, slab) pair into a
        // single Sentry issue instead of one event per poll cycle.
        fingerprint: ["price-sanitization", field ?? "price", slabAddress ?? "unknown"],
        extra: { rawValue: v, field, slabAddress, maxSanePriceUsd: MAX_SANE_PRICE_USD },
      },
    );
    return null;
  }
  return v;
}

// #868: Blocklist for markets with corrupt state or wrong oracle_authority (e.g. issue #837).
// GH#1539: Now uses the unified BLOCKED_SLAB_ADDRESSES from lib/blocklist.ts which
// includes both hardcoded addresses and env var overrides. No local merge needed.

export const dynamic = "force-dynamic";

// GET /api/markets — list all active markets with stats
export async function GET(request: NextRequest) {
  try {
    const supabase = getServiceClient();
    const { data, error } = await supabase
      .from("markets_with_stats")
      .select(
        "slab_address,mint_address,symbol,name,decimals,deployer,logo_url,max_leverage,trading_fee_bps," +
        "last_price,mark_price,index_price,volume_24h,trade_count_24h,open_interest_long,open_interest_short,total_open_interest," +
        "insurance_fund,insurance_balance,total_accounts,funding_rate,net_lp_pos,lp_sum_abs,c_tot," +
        "vault_balance,created_at,stats_updated_at,oracle_mode,dex_pool_address,mainnet_ca,oracle_authority"
      );

    if (error) {
      Sentry.captureException(error, {
        tags: { endpoint: "/api/markets", method: "GET" },
      });
      return NextResponse.json({ error: error.message }, { status: 500 });
    }

    // Sanitize funding_rate: raw DB values from uninitialized slabs can be
    // garbage (e.g. 17733189824741436). Clamp to valid bps range. (#817)
    // Also: oracle_mode was not populated for markets created before migration 035.
    // Derive from oracle_authority: zero pubkey → pyth-pinned, else admin/hyperp.
    // Default to "admin" when unknown — safest assumption for old devnet markets.
    const ZERO_PUBKEY = "11111111111111111111111111111111";
    // GH#1420: Parse ?include_zombie=true to opt-in to zombie markets in the response.
    // By default, markets with vault_balance=0 are excluded as they have no LP liquidity
    // and return garbage/stale prices (e.g. BTC@$148, SOL@$0.60).
    const includeZombie = request?.nextUrl?.searchParams?.get("include_zombie") === "true";

    const sanitized = ((data ?? []) as unknown as Record<string, unknown>[])
      .filter((m) => !BLOCKED_SLAB_ADDRESSES.has(m.slab_address as string))
      .map((m) => {
      let oracle_mode = m.oracle_mode as string | null;
      if (!oracle_mode) {
        const auth = m.oracle_authority as string | null;
        if (auth && auth !== ZERO_PUBKEY) {
          oracle_mode = "admin";
        } else if (auth === ZERO_PUBKEY) {
          oracle_mode = "pyth";
        } else {
          oracle_mode = "admin"; // safe default
        }
      }
      // #1160: Compute a USD-denominated OI field so consumers don't need to divide
      // by 10^decimals manually. Derived from total_open_interest when sane, falls
      // back to open_interest_long + open_interest_short. Raw fields are preserved.
      const sanitizedPrice = sanitizePrice(m.last_price as number | null, "last_price", m.slab_address as string);
      const rawOi = isSaneMarketValue(m.total_open_interest as number | null)
        ? m.total_open_interest as number
        : (() => {
            const combined = (m.open_interest_long as number ?? 0) + (m.open_interest_short as number ?? 0);
            return isSaneMarketValue(combined) ? combined : null;
          })();
      const total_open_interest_usd = rawToUsd(rawOi, m.decimals as number | null, sanitizedPrice);

      // GH#1250: If total_accounts == 0, OI must be stale/orphaned — suppress from display.
      // Root cause: the on-chain totalOpenInterest counter is not decremented when positions
      // are force-closed or accounts are reclaimed (PERC-511 path). This guard prevents
      // misleading solvency signals (OI > 0 with vault = 0 and no accounts).
      // Indexer-level fix (StatsCollector.ts) will clear OI for future syncs; this is a
      // defensive display-layer fallback.
      // GH#1271: Also suppress when vault_balance = 0 (no LP liquidity → no real positions).
      // PERC-816: Extend to suppress for dust vault_balance (0 < vault < 1,000,000 micro-units).
      // Mirrors the invariant enforced by StatsCollector and migration 049.
      // GH#1290 / PERC-570: Phantom OI guard — suppress all OI fields (USD and raw atoms)
      // when vault is dust/empty or no accounts exist. Matches StatsCollector invariant
      // and migration 051. Suppressing only total_open_interest_usd left the raw
      // total_open_interest atom value in the response, which fed phantom OI into
      // computeMarketHealthFromStats and the markets page sort/filter.
      // GH#1438: Aligned to strict < via shared isPhantomOpenInterest() helper in lib/phantom-oi.ts
      // so /api/markets and /api/stats are guaranteed to use the same predicate (single source of truth).
      // GH#1494: coerce NUMERIC (string from Supabase) to number before arithmetic comparisons.
      const accountsCount = Number(m.total_accounts ?? 0);
      const vaultBal = Number(m.vault_balance ?? 0);
      const isPhantomOI = isPhantomOpenInterest(accountsCount, vaultBal);
      const displayOiUsd = isPhantomOI ? null : total_open_interest_usd;

      // GH#1270: Pre-compute volume_24h in USD so consumers (e.g. Watchlist) don't need
      // to divide by 10^decimals manually. Mirrors the total_open_interest_usd pattern.
      // Raw volume_24h is preserved in the response for backward compatibility.
      const volume_24h_usd = rawToUsd(
        m.volume_24h as number | null,
        m.decimals as number | null,
        sanitizedPrice,
      );

      // GH#1420 + GH#1427: Mark zombie markets using shared isZombieMarket() helper.
      // (CodeRabbit #1466: extracted from inline predicate in stats route to avoid drift.)
      // Zombie markets have no LP liquidity; their prices are stale/garbage from
      // when the vault drained (e.g. BTC@$148, SOL@$0.60 — prices from months ago).
      // We tag them with is_zombie=true and exclude them from the default response
      // (opt-in via ?include_zombie=true). See isZombieMarket() in activeMarketFilter.ts
      // for the two conditions: vault=0 (drained) or vault=null+no-stats (phantom).
      //
      // GH#1494: Supabase returns NUMERIC columns (vault_balance, total_open_interest,
      // volume_24h) as strings at runtime. TypeScript `as number | null` is compile-time
      // only and does NOT coerce the value. Without Number() coercion, the strict equality
      // check `vaultBal === 0` in isZombieMarket() compares string "0" to number 0 →
      // always false → is_zombie is never set to true despite zombieCount=73.
      // Fix: coerce all NUMERIC fields to number|null before passing to isZombieMarket().
      const numericOrNull = (v: unknown): number | null => {
        if (v == null) return null;
        const n = Number(v);
        return Number.isFinite(n) ? n : null;
      };
      // GH#1506: Use sanitizedPrice (already capped at MAX_SANE_PRICE_USD=$1M) for the
      // zombie check instead of numericOrNull(m.last_price). Raw DB prices can be stale
      // garbage values that pass isSaneMarketValue (< 1e18) but exceed the display cap.
      // NNOB had a stale raw last_price > $1M in DB — sanitizePrice nulled it for output,
      // but passing numericOrNull(m.last_price) to isZombieMarket() made hasActivity=true
      // → c_tot>0 exemption fired → is_zombie=false, even though the API returned null.
      // Using sanitizedPrice keeps the zombie check consistent with what consumers receive.
      const is_zombie = isZombieMarket({
        vault_balance: numericOrNull(m.vault_balance),
        c_tot: numericOrNull(m.c_tot),
        last_price: sanitizedPrice,
        volume_24h: numericOrNull(m.volume_24h),
        total_open_interest: numericOrNull(m.total_open_interest),
        total_accounts: numericOrNull(m.total_accounts),
      });

      return {
        ...m,
        oracle_mode,
        is_zombie,
        funding_rate: sanitizeFundingRate(m.funding_rate as number | null),
        // #856: Null out corrupt admin-set test prices (raw unscaled u64 values or billions/trillions).
        // Matches Rust MAX_ORACLE_PRICE = $1B USD ceiling.
        // GH#1420: Also null out prices for zombie markets — stale prices with no liquidity are misleading.
        last_price: is_zombie ? null : sanitizedPrice,
        mark_price: is_zombie ? null : sanitizePrice(m.mark_price as number | null, "mark_price", m.slab_address as string),
        // #855: Apply same sanitization to index_price — same DB column type and
        // corruption vector as last_price/mark_price. Inconsistent sanitization
        // means a corrupt index price still reaches consumers.
        index_price: is_zombie ? null : sanitizePrice(m.index_price as number | null, "index_price", m.slab_address as string),
        // #1160 / GH#1290 / PERC-570: OI fields — USD and raw atoms.
        // Raw atom fields (total_open_interest, open_interest_long, open_interest_short) are
        // zeroed (not just the USD conversion) when the phantom OI guard fires.
        // Previously only total_open_interest_usd was suppressed, leaving the raw atom value
        // in the response and feeding phantom OI into health calculations and sort/filter.
        // GH#1250/1271/PERC-816/GH#1290: Suppressed when total_accounts == 0 or vault_balance < 1_000_000.
        total_open_interest: isPhantomOI ? 0 : (m.total_open_interest as number ?? 0),
        open_interest_long: isPhantomOI ? 0 : (m.open_interest_long as number ?? 0),
        open_interest_short: isPhantomOI ? 0 : (m.open_interest_short as number ?? 0),
        total_open_interest_usd: displayOiUsd,
        // GH#1270: Pre-converted 24h volume in USD. Null when price unavailable or raw
        // value is a sentinel. Raw volume_24h preserved for backward compatibility.
        volume_24h_usd,
        // GH#1208: Sanitize c_tot — near-sentinel values (e.g. 7.997e17) pass the
        // isSaneMarketValue 1e18 check but are clearly corrupt LP collateral totals.
        c_tot: sanitizeCtot(m.c_tot as number | null),
      };
    });

    // GH#1420: Filter zombie markets (vault_balance=0) unless ?include_zombie=true
    const nonZombie = sanitized.filter((m) => includeZombie || !(m as Record<string, unknown>).is_zombie);
    // GH#1429: Compute zombieCount from sanitized array BEFORE the zombie filter, not from
    // the difference sanitized.length - nonZombie.length. When include_zombie=true, nonZombie
    // includes all markets (including zombies), making the difference always 0. Computing
    // directly from the tagged is_zombie field gives the correct count regardless of the flag.
    const zombieCount = sanitized.filter((m) => (m as Record<string, unknown>).is_zombie === true).length;

    // #1168: Include total count so API consumers can get market count without
    // fetching all records. Reflects post-filter count (blocked markets excluded).
    // #1172: Add activeTotal — markets with at least one sane stat (price/volume/OI).
    // This matches the count shown by /api/stats totalMarkets.
    // GH#1455: Always compute activeTotal from non-zombie markets only, regardless of
    // include_zombie flag. Previously, when include_zombie=true, nonZombie contained ALL
    // markets (including zombies), so activeTotal counted zombie markets that passed
    // isActiveMarket() — producing 71 instead of 69. Computing from the zombie-excluded
    // set ensures consistency with /api/stats.
    const nonZombieOnly = sanitized.filter((m) => !(m as Record<string, unknown>).is_zombie);
    const activeTotal = nonZombieOnly.filter((m) => isActiveMarket(m as Parameters<typeof isActiveMarket>[0])).length;

    // GH#1512: Apply search filter — case-insensitive substring match on symbol or name.
    // GH#1527: Also resolve the query against SLUG_ALIASES so searching "SOL" matches
    // markets whose DB symbol is a truncated address (e.g. "So111111") but whose
    // mint_address or mainnet_ca is the SOL mint. This bridges the gap between the
    // human-readable token names shown in the UI (via token-metadata enrichment) and
    // the raw DB values that the search runs against.
    const searchParam = request?.nextUrl?.searchParams?.get("search") ?? null;
    const searchTrimmed = searchParam ? searchParam.trim() : null;
    const searchFiltered = searchTrimmed
      ? (() => {
          const q = searchTrimmed.toLowerCase();
          // Collect mint addresses whose well-known symbol matches the query
          // (e.g. q="sol" matches MINT_TO_KNOWN_SYMBOL entry "SOL" → So111...112)
          const matchingMints = new Set<string>();
          for (const [mint, knownSymbol] of MINT_TO_KNOWN_SYMBOL) {
            if (knownSymbol.toLowerCase().includes(q)) {
              matchingMints.add(mint);
            }
          }
          return nonZombie.filter((m) => {
            const sym = ((m as Record<string, unknown>).symbol as string | null) ?? "";
            const name = ((m as Record<string, unknown>).name as string | null) ?? "";
            // Direct DB field match (existing behaviour — handles WENDYS, etc.)
            if (sym.toLowerCase().includes(q) || name.toLowerCase().includes(q)) return true;
            // GH#1527: Known-symbol match via mint_address or mainnet_ca
            const mintAddress = ((m as Record<string, unknown>).mint_address as string | null) ?? "";
            const mainnetCa = ((m as Record<string, unknown>).mainnet_ca as string | null) ?? "";
            if (matchingMints.has(mintAddress) || matchingMints.has(mainnetCa)) return true;
            return false;
          });
        })()
      : nonZombie;

    // GH#1512: Apply oracle_mode filter.
    // GH#1526: Map frontend display values ("manual", "live_feed") to DB canonical
    // values ("admin", "hyperp") before filtering. Previously the filter did an exact
    // match, so passing "manual" or "live_feed" (the values the UI uses) always returned
    // 0 results because the DB stores "admin" and "hyperp" respectively.
    const oracleModeParam = request?.nextUrl?.searchParams?.get("oracle_mode") ?? null;
    const oracleModeFiltered = oracleModeParam
      ? (() => {
          const dbValue = ORACLE_MODE_FRONTEND_TO_DB[oracleModeParam] ?? oracleModeParam;
          return searchFiltered.filter(
            (m) => ((m as Record<string, unknown>).oracle_mode as string | null) === dbValue,
          );
        })()
      : searchFiltered;

    // GH#1512: Apply sort + order. Supported sort keys: symbol, last_price, volume_24h,
    // total_open_interest_usd, funding_rate. Default: no sort (DB order).
    const sortParam = request?.nextUrl?.searchParams?.get("sort") ?? null;
    const orderParam = (request?.nextUrl?.searchParams?.get("order") ?? "asc").toLowerCase();
    const sortDir = orderParam === "desc" ? -1 : 1;
    // GH#1524: Expanded sortable field set to include all fields callers actually use.
    // Previously only 5 fields were allowlisted; sort=total_open_interest, sort=mark_price,
    // and sort=created_at all silently fell through to the else branch (no sort applied),
    // causing asc == desc == no-sort for those fields.
    const SORTABLE_FIELDS = new Set([
      "symbol",
      "last_price",
      "mark_price",
      "index_price",
      "volume_24h",
      "volume_24h_usd",
      "total_open_interest",
      "total_open_interest_usd",
      "funding_rate",
      "created_at",
      "stats_updated_at",
      "trade_count_24h",
      "insurance_fund",
      "insurance_balance",
      "total_accounts",
    ]);
    const sorted =
      sortParam && SORTABLE_FIELDS.has(sortParam)
        ? [...oracleModeFiltered].sort((a, b) => {
            const av = (a as Record<string, unknown>)[sortParam] ?? null;
            const bv = (b as Record<string, unknown>)[sortParam] ?? null;
            // Nulls last regardless of order direction.
            if (av === null && bv === null) return 0;
            if (av === null) return 1;
            if (bv === null) return -1;
            if (typeof av === "string" && typeof bv === "string") {
              return sortDir * av.localeCompare(bv);
            }
            return sortDir * ((av as number) - (bv as number));
          })
        : oracleModeFiltered;

    // GH#1348: Respect ?limit= query param to avoid returning 100+ markets
    // GH#1490: Validate limit (must be 1–500) and offset (must be >= 0) using
    // validateNumericParam() from route-validators.ts. Previously limit=-1/0/999999
    // all returned the full dataset and non-numeric offset was silently ignored.
    // Follow-up: use validated .value directly (not re-parsed) to reject "1.5"/"20abc".
    const MAX_LIMIT = 500;
    const limitParam = request?.nextUrl?.searchParams?.get("limit") ?? null;
    let limitNum = 0;
    if (limitParam !== null) {
      const limitValidation = validateNumericParam(limitParam, { min: 1, max: MAX_LIMIT });
      if (!limitValidation.valid) return limitValidation.response;
      limitNum = limitValidation.value;
    }

    const offsetParam = request?.nextUrl?.searchParams?.get("offset") ?? null;
    let offsetNum = 0;
    if (offsetParam !== null) {
      const offsetValidation = validateNumericParam(offsetParam, { min: 0 });
      if (!offsetValidation.valid) return offsetValidation.response;
      offsetNum = offsetValidation.value;
    }

    const paged = offsetNum > 0 ? sorted.slice(offsetNum) : sorted;
    const limited = limitNum > 0 ? paged.slice(0, limitNum) : paged;

    return NextResponse.json({ total: sorted.length, activeTotal, zombieCount, markets: limited }, {
      headers: {
        "Cache-Control": "public, s-maxage=10, stale-while-revalidate=30",
      },
    });
  } catch (error) {
    Sentry.captureException(error, {
      tags: { endpoint: "/api/markets", method: "GET" },
    });
    return NextResponse.json(
      { error: "Internal server error" },
      { status: 500 }
    );
  }
}

// POST /api/markets — register a new market after deployment
// Auth: on-chain verification (deployer == slab admin) is the real gate.
// No API key required — the client calls this after successful on-chain deployment.
export async function POST(req: NextRequest) {
  try {
    const body = await req.json();

  const {
    slab_address,
    mint_address,
    symbol,
    name,
    decimals,
    deployer,
    oracle_authority,
    initial_price_e6,
    max_leverage,
    trading_fee_bps,
    lp_collateral,
    matcher_context,
    logo_url,
    mainnet_ca,
    oracle_mode,
    dex_pool_address,
  } = body;

  if (!slab_address || !mint_address || !deployer) {
    return NextResponse.json(
      { error: "Missing required fields: slab_address, mint_address, deployer" },
      { status: 400 }
    );
  }

  // GH#1398: Reject markets with unreasonably high max_leverage.
  // 333x (and similar) garbage test markets have been observed on devnet.
  // Cap at 100x — any higher is almost certainly a misconfiguration or test artifact.
  // The on-chain program may allow higher values, but we reject at the API layer to
  // keep the market list clean and prevent user-facing extreme-leverage exposure.
  const MAX_ALLOWED_LEVERAGE = 100;
  if (max_leverage != null && max_leverage > MAX_ALLOWED_LEVERAGE) {
    return NextResponse.json(
      { error: `max_leverage exceeds allowed maximum of ${MAX_ALLOWED_LEVERAGE}x` },
      { status: 400 }
    );
  }

  // #813: Validate oracle_mode enum
  const VALID_ORACLE_MODES = ["pyth", "hyperp", "admin"] as const;
  type OracleMode = typeof VALID_ORACLE_MODES[number];
  const resolvedOracleMode: OracleMode = oracle_mode ?? "admin";
  if (!VALID_ORACLE_MODES.includes(resolvedOracleMode)) {
    return NextResponse.json(
      { error: `Invalid oracle_mode. Must be one of: ${VALID_ORACLE_MODES.join(", ")}` },
      { status: 400 }
    );
  }

  // #813: Validate dex_pool_address is a valid Solana pubkey (when provided)
  if (dex_pool_address) {
    try {
      new PublicKey(dex_pool_address);
    } catch {
      return NextResponse.json(
        { error: "Invalid dex_pool_address: must be a valid Solana public key" },
        { status: 400 }
      );
    }
  }

  // Verify slab account exists on-chain and is owned by our program
  try {
    const cfg = getConfig();
    const connection = new Connection(cfg.rpcUrl, "confirmed");
    const slabPubkey = new PublicKey(slab_address);
    const accountInfo = await connection.getAccountInfo(slabPubkey);
    if (!accountInfo) {
      return NextResponse.json({ error: "Slab account does not exist on-chain" }, { status: 400 });
    }
    const validPrograms = new Set<string>([cfg.programId]);
    const tiers = (cfg as Record<string, unknown>).programsBySlabTier as Record<string, string> | undefined;
    if (tiers) Object.values(tiers).forEach((id) => validPrograms.add(id));
    if (!validPrograms.has(accountInfo.owner.toBase58())) {
      return NextResponse.json({ error: "Slab account not owned by a known percolator program" }, { status: 400 });
    }

    // R2-S8: Verify deployer matches the on-chain admin
    try {
      const header = parseHeader(accountInfo.data);
      if (header.admin.toBase58() !== deployer) {
        return NextResponse.json(
          { error: "Deployer does not match slab admin" },
          { status: 403 },
        );
      }
    } catch {
      return NextResponse.json({ error: "Failed to parse slab header" }, { status: 400 });
    }
  } catch (err) {
    return NextResponse.json({ error: "Failed to verify slab on-chain" }, { status: 400 });
  }

  const supabase = getServiceClient();

  // Insert market
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const { data: market, error: marketError } = await (supabase
    .from("markets") as any)
    .insert({
      slab_address,
      mint_address,
      symbol: symbol || mint_address.slice(0, 4).toUpperCase(),
      name: name || `Token ${mint_address.slice(0, 8)}`,
      decimals: decimals || 6,
      deployer,
      oracle_authority: oracle_authority || deployer,
      initial_price_e6,
      max_leverage: max_leverage || 10,
      trading_fee_bps: trading_fee_bps || 10,
      lp_collateral,
      matcher_context,
      logo_url: logo_url || null,
      mainnet_ca: mainnet_ca || null,
      oracle_mode: resolvedOracleMode,
      dex_pool_address: dex_pool_address || null,
    })
    .select()
    .single();

  if (marketError) {
    return NextResponse.json({ error: marketError.message }, { status: 500 });
  }

  // Create initial stats row
  await (supabase.from("market_stats") as any).insert({
    slab_address,
    last_price: initial_price_e6 ? initial_price_e6 / 1_000_000 : null,
  });

  // PERC-465: Hot-register with oracle keeper service (server-to-server, non-fatal)
  if (mainnet_ca && process.env.KEEPER_REGISTER_SECRET) {
    try {
      const keeperRegisterUrl = `${process.env.NEXT_PUBLIC_BASE_URL ?? ""}/api/oracle-keeper/register`;
      await fetch(keeperRegisterUrl, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          "x-keeper-secret": process.env.KEEPER_REGISTER_SECRET,
        },
        body: JSON.stringify({ slabAddress: slab_address, mainnetCA: mainnet_ca }),
        signal: AbortSignal.timeout(5000),
      }).catch(() => {});
    } catch {
      // Non-fatal — oracle keeper will discover via Supabase polling
    }
  }

    return NextResponse.json({ market }, { status: 201 });
  } catch (error) {
    Sentry.captureException(error, {
      tags: { endpoint: "/api/markets", method: "POST" },
    });
    return NextResponse.json(
      { error: "Internal server error" },
      { status: 500 }
    );
  }
}
