/**
 * Shared filter logic for counting "active" markets.
 *
 * A market is active if it has at least one non-zero, non-sentinel stat
 * (price, volume, or open interest). Sentinel values ≈ u64::MAX (1.844e19)
 * are treated as zero because they come from uninitialized on-chain fields.
 *
 * SINGLE SOURCE OF TRUTH: used by homepage, /api/stats, and markets page
 * to ensure consistent market counts across the platform.
 */

/** Returns true if a numeric value is sane (positive, finite, not a u64::MAX sentinel). */
export function isSaneMarketValue(v: number | null | undefined): boolean {
  if (v == null) return false;
  return v > 0 && v < 1e18 && Number.isFinite(v);
}

/**
 * Determine if a market row (from markets_with_stats) is "active".
 * A market is active if it has at least one sane metric.
 */
export function isActiveMarket(row: {
  last_price?: number | null;
  volume_24h?: number | null;
  total_open_interest?: number | null;
  open_interest_long?: number | null;
  open_interest_short?: number | null;
}): boolean {
  if (isSaneMarketValue(row.last_price)) return true;
  if (isSaneMarketValue(row.volume_24h)) return true;
  if (isSaneMarketValue(row.total_open_interest)) return true;
  // Fallback: sum of long + short OI
  const combinedOI = (row.open_interest_long ?? 0) + (row.open_interest_short ?? 0);
  if (isSaneMarketValue(combinedOI)) return true;
  return false;
}

/**
 * Determine if a market row is a "zombie" — has no LP liquidity and no real activity.
 *
 * Two zombie conditions (GH#1420 + GH#1427):
 *   1. vault_balance === 0  → explicitly drained vault, no liquidity.
 *   2. vault_balance === null AND no sane stats AND total_accounts === 0
 *      → phantom market that was never indexed or funded.
 *
 * SINGLE SOURCE OF TRUTH: used by /api/markets and /api/stats to ensure
 * consistent zombie exclusion across the platform. Previously duplicated
 * inline in both routes (CodeRabbit PR #1466 nitpick).
 */
export function isZombieMarket(row: {
  vault_balance?: number | null;
  c_tot?: number | null;
  last_price?: number | null;
  volume_24h?: number | null;
  total_open_interest?: number | null;
  total_accounts?: number | null;
}): boolean {
  const vaultBal = row.vault_balance ?? null;
  const cTot = row.c_tot ?? null;

  // If on-chain collateral total (c_tot) is positive, market has real funds
  // even if vault_balance reads 0. This happens because c_tot tracks collateral
  // inside the slab data, while vault_balance reads the separate vault ATA.
  // FF7K keeper markets store collateral in the slab, not the vault ATA.
  if (cTot !== null && cTot > 0) return false;

  if (vaultBal !== null && vaultBal === 0) return true;
  if (vaultBal === null) {
    const hasNoStats =
      !isSaneMarketValue(row.last_price) &&
      !isSaneMarketValue(row.volume_24h) &&
      !isSaneMarketValue(row.total_open_interest) &&
      (row.total_accounts ?? 0) === 0;
    if (hasNoStats) return true;
  }
  return false;
}
