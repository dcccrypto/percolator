/**
 * Shared phantom-OI helpers.
 *
 * "Phantom OI" = open interest that is NOT backed by real positions:
 *   - markets with no accounts (vault was never seeded with traders), OR
 *   - markets whose vault_balance is below the creation-deposit threshold (1 USDC = 1_000_000 micro-units).
 *
 * This is the single source of truth used by both /api/markets and /api/stats.
 * Previously each route maintained its own copy of the constant and predicate,
 * which led to drift (GH#1432, GH#1435, GH#1438).
 *
 * Rule: vault_balance < MIN_VAULT_FOR_OI  →  phantom (strict <).
 *   vault=0       → phantom  (no LP deposit)
 *   vault=1–999_999 → phantom  (dust / creation not finalised)
 *   vault=1_000_000 → NOT phantom  (standard creation-deposit; all active devnet markets)
 *   vault>1_000_000 → NOT phantom  (real LP liquidity)
 */

/** Minimum vault_balance (micro-units) for a market to be considered non-phantom. */
export const MIN_VAULT_FOR_OI = 1_000_000;

/**
 * Returns true when a market's open interest should be treated as phantom
 * (suppressed / excluded from aggregates).
 *
 * @param accountsCount  Value of `total_accounts` from the market row (0 when null).
 * @param vaultBalance   Value of `vault_balance`   from the market row (0 when null).
 */
export function isPhantomOpenInterest(
  accountsCount: number,
  vaultBalance: number,
): boolean {
  return accountsCount === 0 || vaultBalance < MIN_VAULT_FOR_OI;
}
