-- Migration 052: Register AWbcen87 (Percolator/USD) in oracle_markets — GH#1376
--
-- Context: Market AWbcen87WbyqfvD3onLYxtRyJi7adtpxC4heZqZbSdLP (Percolator/USD)
-- has oracle_mode=admin in the markets table. The oracle-keeper's admin-oracle path
-- (keeper_crank, tag=235281) sends pool.sol_reserve raw (~454,074,932,992 lamports)
-- as the oracle_price arg instead of the computed price_e6.
--
-- The on-chain program rejects 454,074,932,992 as out-of-range → mark_price stays 0.
-- Consequence: PnL shows --, liq price shows --, funding calculation is broken,
-- liquidation engine is blind for all positions on this market.
--
-- Fix: Register this slab in oracle_markets as oracle_type='hyperp' with the
-- pumpswap pool address. The oracle-keeper discovery loop checks oracle_markets
-- first (every DISCOVERY_INTERVAL_MS=30s). When a hyperp row is found, the keeper
-- switches to UpdateHyperpMark path which uses parseDexPool from @percolator/sdk
-- to compute price_e6 = (sol_reserve_lamports / token_reserve_base_units) * sol_usd_price * 10^6.
-- Expected result: ~919 (≈ $0.000919 × 10^6) matching UI display price.
--
-- Pool: Ebs3mXAzqZfzHfsdinTNw7gPy4uNyEAywcCiJxzLRrBW (pumpswap, devnet)
-- Mint: DJKjmSbWjhx925kuk1fS1BENCBnqXCfwUJjb9EKwSEnV (Percolator token)
--
-- Note: other oracle_markets entries with this pool address (5MEEy1..., 4U1aJB..., etc.)
-- are disabled — those are PERCOLATOR-PERP admin markets that were misconfigured.
-- AWbcen87 is the ACTUAL Percolator token market and this pool is its real price source.
--
-- References: GH#1376, PM collector message 2026-03-17

INSERT INTO oracle_markets (slab_address, oracle_type, dex_pool_address, notes)
VALUES (
  'AWbcen87WbyqfvD3onLYxtRyJi7adtpxC4heZqZbSdLP',
  'hyperp',
  'Ebs3mXAzqZfzHfsdinTNw7gPy4uNyEAywcCiJxzLRrBW',
  'Percolator/USD V1_LEGACY — pumpswap pool price source. Registered 2026-03-17 to fix keeper sending raw SOL reserve (GH#1376).'
)
ON CONFLICT (slab_address) DO UPDATE
  SET oracle_type      = EXCLUDED.oracle_type,
      dex_pool_address = EXCLUDED.dex_pool_address,
      notes            = EXCLUDED.notes,
      enabled          = true,
      updated_at       = NOW();

-- Verify: this row should now exist and be enabled
-- SELECT slab_address, oracle_type, dex_pool_address, enabled
-- FROM oracle_markets
-- WHERE slab_address = 'AWbcen87WbyqfvD3onLYxtRyJi7adtpxC4heZqZbSdLP';
