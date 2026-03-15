-- Migration 050: Zero phantom OI for markets at the creation-deposit boundary — PERC-817
--
-- Context: Migrations 047–049 left 39 markets with phantom OI. Root cause:
--
--   1. vault_balance = 1,000,000 (creation-deposit seeded markets):
--      Migration 049 used vault_balance < 1,000,000 (strictly less than), missing
--      markets whose vault is exactly 1,000,000 — the amount the program seeds into
--      the vault at market creation (PERC-623: 70/30 keeper/vault split).
--      These markets received no real LP deposits: vault == 1,000,000 = creation dust.
--      Having OI (up to 2,660,054,000,000 atoms) with only a creation-deposit vault
--      is physically impossible via the on-chain program; this is parsed phantom data
--      from wrong slab layout reads during early indexer versions.
--
--   2. vault_balance = 0 with OI surviving migration 048:
--      Four markets (LOBSTAR, WCLD, JOS, BTC) have vault=0, accounts=0 but still
--      show non-zero OI. These slabs are no longer discovered by the indexer so
--      StatsCollector never runs its per-tick guard, and the DB rows retain stale
--      values from before migrations 047–048 zeroed the bulk.
--
-- Fix: widen the dust threshold to vault_balance <= 1,000,000 (inclusive).
-- 39 rows are affected. The StatsCollector guard is updated in the same commit
-- to enforce `engine.vault <= MIN_VAULT_FOR_OI` on future writes.

DO $$
DECLARE
  MIN_VAULT_INCLUSIVE CONSTANT NUMERIC := 1000000;  -- creation-deposit boundary (inclusive)
  rows_updated INT;
BEGIN
  UPDATE market_stats
  SET
    open_interest_long  = 0,
    open_interest_short = 0,
    total_open_interest = 0
  WHERE total_open_interest > 0
    AND vault_balance <= MIN_VAULT_INCLUSIVE;

  GET DIAGNOSTICS rows_updated = ROW_COUNT;
  RAISE NOTICE 'Migration 050: zeroed phantom OI for % market_stats rows (vault <= %)', rows_updated, MIN_VAULT_INCLUSIVE;
END $$;

-- Sanity check: should return 0 rows after apply
-- SELECT slab_address, vault_balance, total_accounts, total_open_interest
-- FROM market_stats
-- WHERE total_open_interest > 0
--   AND vault_balance <= 1000000;
