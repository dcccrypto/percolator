-- Migration 047: Zero stale OI for markets with total_accounts=0 — GH#1250
--
-- Context: 6 coin-margined markets show non-zero open interest in USD but
-- vault_balance=0 and total_accounts=0. This is an inconsistent state caused by
-- the on-chain totalOpenInterest counter not being decremented when positions are
-- force-closed or accounts are reclaimed (PERC-511 path).
--
-- Affected slabs (as of 2026-03-15):
--   LOBSTAR: FCusfsg4uzcLSdRbj9Ez5okcrS1MwvKHvDbmcwrnSWvL
--   CHET:    8KU63GiDjJ2BqTMK49qU4TuNPgEyrebvYfjZwvTHgXEz
--   WCLD:    Dk5YUN7XivX9mz9EnpFxQdw1zD1MjuGNZbhhjLxCP78E
--   S2:      917mzk4DhzHfbk53zMYjG7YTDgrhqPm87Aj4j4UL3XEM
--   JOS:     BgdMZb6ocHfvRWV8z8oh24gB4Mg94Joi4KvhMDEcxCs9
--   BTC:     AB3ZN1vxbBEh8FZRfrL55QQUUaLCwawqvCYzTDpgbuLF
--
-- Fix: Zero out OI columns for all market_stats rows where total_accounts = 0.
-- Applied as a general predicate (not slab-specific) so future occurrences are
-- also covered if this migration is re-run.
--
-- The indexer is also patched (StatsCollector.ts, GH#1250) to use oiLong + oiShort
-- (from parsed accounts) instead of engine.totalOpenInterest, so re-population of
-- stale OI will not recur after the indexer is redeployed.

UPDATE market_stats
SET
  open_interest_long  = 0,
  open_interest_short = 0,
  total_open_interest = 0
WHERE total_accounts = 0
  AND total_open_interest > 0;

-- Verify: no rows should remain with accounts=0 and OI>0
-- SELECT slab_address, total_accounts, total_open_interest
-- FROM market_stats
-- WHERE total_accounts = 0 AND total_open_interest > 0;
