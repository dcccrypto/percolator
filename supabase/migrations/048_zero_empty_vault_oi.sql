-- Migration 048: Zero stale OI for markets with empty vault — GH#1271
--
-- Context: Migration 047 zeroed OI for markets where total_accounts = 0.
-- However, 9+ markets still show phantom trillion-scale OI (2000000000000)
-- because their vault_balance = 0 but total_accounts > 0 at the time 047
-- ran — so those rows were skipped.
--
-- Root cause: The on-chain totalOpenInterest counter is not decremented when
-- positions are force-closed or accounts are reclaimed (PERC-511 path). The
-- indexer historically wrote engine.totalOpenInterest directly (now fixed in
-- StatsCollector.ts via GH#1250 to use oiLong + oiShort from parsed accounts).
--
-- Definitive predicate: vault_balance = 0 means NO LP has deposited liquidity.
-- Without any vault balance, no position can ever be opened — the on-chain
-- program enforces this — so any non-zero OI in these markets is stale/phantom.
--
-- Affected markets confirmed in GH#1271 (partial list):
--   8eFFEFBY3HHbBgzxJJP5hyxdzMNMAumnYNhkWXErBM4c
--   3bmCyPee8GWJR5aPGTyN5EyyQJLzYyD8Wkg9m1Afd1SD
--   Aiwhg31d3sgC3PS9ciorxzcbGFE5g4NjbtAz8EA5mcW5
--   GYpukkn94KKDU9ufNURjDZVMGPp3LTadZrdoPtE2cdc1
--   3YDqCJGz88xGiPBiRvx4vrM51mWTiTZPZ95hxYDZqKpJ
--   F3YUro7KXNVfNZ6FJmMCm25uQ2nxpfypxJ91wxobxBUT
--   2Zta2EPRR444Hp2WbH2L9vfM38Stwr9chDpNk66eevzU
--   5pX7ycPtKwr7xfTxoUUgcWirq8tSz8B1Sq8PJShfFstt
--   5S4gkqX8Jz9MQPmtQ3qCU3698PnY5dFnyTdeq7fu12sW
--
-- Applied as a general vault_balance = 0 predicate (not slab-specific)
-- so any future market that launches empty is also covered.

UPDATE market_stats
SET
  open_interest_long  = 0,
  open_interest_short = 0,
  total_open_interest = 0
WHERE vault_balance = 0
  AND total_open_interest > 0;

-- Verify: no rows should remain with vault=0 and OI>0
-- SELECT slab_address, vault_balance, total_accounts, total_open_interest
-- FROM market_stats
-- WHERE vault_balance = 0 AND total_open_interest > 0;
