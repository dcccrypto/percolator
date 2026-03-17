import { PublicKey } from "@solana/web3.js";
import type { Context, Next } from "hono";
import { sanitizeSlabAddress } from "@percolator/shared";

/**
 * Known-bad slab addresses that cause backend 500 errors (empty vault / phantom OI).
 *
 * These are the same addresses hardcoded in the Next.js app's BLOCKED_SLAB_ADDRESSES
 * (app/lib/blocklist.ts). They are repeated here so the backend API returns 404
 * even when called directly (bypassing the Next.js proxy layer).
 *
 * GH#1357 / PR#1377 / Sentry follow-up (devops 2026-03-17).
 * Extend via BLOCKED_MARKET_ADDRESSES env var (comma-separated pubkeys).
 */
const HARDCODED_BLOCKED_SLABS: ReadonlySet<string> = new Set([
  // SEX/USD — devnet-only token, empty vault, phantom OI (migration 048)
  "3bmCyPee8GWJR5aPGTyN5EyyQJLzYyD8Wkg9m1Afd1SD",
  // Empty-vault phantom-OI slab (migration 048)
  "3YDqCJGz88xGiPBiRvx4vrM51mWTiTZPZ95hxYDZqKpJ",
  // Empty-vault phantom-OI slab (no on-chain liquidity)
  "3ZKKwsKoo5UP28cYmMpvGpwoFpWLVgEWLQJCejJnECQn",
]);

/**
 * Runtime-configurable blocklist loaded once at startup from BLOCKED_MARKET_ADDRESSES.
 * Allows ops to block new addresses without a code deploy.
 */
const ENV_BLOCKED_SLABS: ReadonlySet<string> = new Set(
  (process.env.BLOCKED_MARKET_ADDRESSES ?? "")
    .split(",")
    .map((s) => s.trim())
    .filter(Boolean),
);

function isBlocked(slab: string): boolean {
  return HARDCODED_BLOCKED_SLABS.has(slab) || ENV_BLOCKED_SLABS.has(slab);
}

/**
 * Hono middleware that validates the `:slab` route param is a valid Solana public key.
 * Returns 400 if invalid, 404 if the address is on the backend blocklist.
 */
export async function validateSlab(c: Context, next: Next) {
  const slab = c.req.param("slab");
  if (!slab) return next();

  // First sanitize the input
  const sanitized = sanitizeSlabAddress(slab);
  if (!sanitized) {
    return c.json({ error: "Invalid slab address" }, 400);
  }

  // Then validate it's a valid Solana public key
  try {
    new PublicKey(sanitized);
  } catch {
    return c.json({ error: "Invalid slab address" }, 400);
  }

  // Blocklist check — return 404 for known-bad/empty slabs instead of proxying
  // to DB queries that return 500 (phantom OI / no market_stats rows).
  if (isBlocked(sanitized)) {
    return c.json({ error: "Market not found" }, 404);
  }

  return next();
}
