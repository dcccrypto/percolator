-- Migration 051: Zero phantom OI for vault=0, accounts=0 markets (LP fully withdrawn)
--
-- Context: Migrations 047–050 covered phantom OI from creation-deposit markets
-- (vault=1_000_000 or near-zero dust vaults). However, they all ran BEFORE some
-- markets reached vault=0 — specifically markets where an LP deposited real
-- liquidity and later withdrew it entirely, draining vault to 0.
--
-- At the time migrations 047–050 executed, these markets had vault_balance > 1_000_000
-- (real LP deposits), so the dust-vault predicate correctly excluded them. After the
-- LP withdrew, vault_balance dropped to 0 and total_accounts to 0, but the
-- StatsCollector had already stopped tracking the slab (slab removed from discovery
-- after LP close), so no future write ever applied the per-tick vault guard.
--
-- Confirmed affected market (GH#1290):
--   LOBSTAR/USD: slab FCusfsg4uzcLSdRbj9Ez5okcrS1MwvKHvDbmcwrnSWvL
--   vault_balance=0, total_accounts=0, total_open_interest=4000018000000 (phantom)
--
-- Predicate: vault_balance = 0 AND total_accounts = 0 AND total_open_interest > 0
-- This is the definitive "drained market" signature: no LP, no traders, yet stale OI.
--
-- Migration 048 used the same vault_balance = 0 predicate but ran before these markets
-- were drained. This migration mops up the remaining cases.

DO $$
DECLARE
  rows_updated INT;
BEGIN
  UPDATE market_stats
  SET
    open_interest_long  = 0,
    open_interest_short = 0,
    total_open_interest = 0
  WHERE vault_balance = 0
    AND total_accounts = 0
    AND total_open_interest > 0;

  GET DIAGNOSTICS rows_updated = ROW_COUNT;
  RAISE NOTICE 'Migration 051: zeroed phantom OI for % market_stats rows (vault=0, accounts=0)', rows_updated;
END $$;

-- Sanity check: should return 0 rows after apply
-- SELECT slab_address, vault_balance, total_accounts, total_open_interest
-- FROM market_stats
-- WHERE vault_balance = 0 AND total_accounts = 0 AND total_open_interest > 0;
