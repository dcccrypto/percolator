-- Migration 049: Zero phantom OI for markets with dust vault_balance — PERC-816
--
-- Context: Migrations 047 and 048 zeroed OI for markets where:
--   047: total_accounts = 0   AND total_open_interest > 0
--   048: vault_balance = 0    AND total_open_interest > 0
--
-- 25 markets remain with phantom OI despite near-zero (dust) vault_balance.
-- Root cause: These markets have vault_balance > 0 but below any meaningful
-- LP threshold — likely from creation-deposit splits (70/30 keeper/vault from
-- PERC-623) or parser artifacts from wrong slab layout reads.
--
-- The Percolator program enforces: no position can be opened without sufficient
-- vault liquidity. A vault_balance below 1,000,000 micro-units (< 1 USDC at
-- 6 decimals, < 0.001 SOL at 9 decimals) cannot sustain any real open interest.
--
-- Fix: zero OI whenever vault is dust OR total_accounts = 0 (belt-and-suspenders
-- in case migrations 047/048 were applied before the indexer updated those rows).
-- The StatsCollector is also patched to enforce this invariant on all future writes
-- so re-population of phantom OI cannot recur.

DO $$
DECLARE
  MIN_VAULT_FOR_OI CONSTANT NUMERIC := 1000000;  -- 1 micro-token (< 1 USDC at 6dp)
  rows_updated INT;
BEGIN
  UPDATE market_stats
  SET
    open_interest_long  = 0,
    open_interest_short = 0,
    total_open_interest = 0
  WHERE total_open_interest > 0
    AND (
      -- Dust vault: non-zero but below minimum meaningful LP threshold
      (vault_balance > 0 AND vault_balance < MIN_VAULT_FOR_OI)
      -- Belt-and-suspenders: no accounts → no open positions → OI must be 0
      OR total_accounts = 0
    );

  GET DIAGNOSTICS rows_updated = ROW_COUNT;
  RAISE NOTICE 'Migration 049: zeroed phantom OI for % market_stats rows', rows_updated;
END $$;

-- Sanity check (uncomment to verify after applying):
-- SELECT slab_address, vault_balance, total_accounts, total_open_interest
-- FROM market_stats
-- WHERE total_open_interest > 0
--   AND (total_accounts = 0 OR (vault_balance > 0 AND vault_balance < 1000000));
