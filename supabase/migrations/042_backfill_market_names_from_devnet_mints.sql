-- Migration: 042_backfill_market_names_from_devnet_mints
-- GH#1132: Markets search broken for BTC/perp keywords
--
-- Root cause: markets created via Quick Launch with a devnet-native token address
-- (not the mirror-mint flow) have placeholder names/symbols:
--   - symbol = first 8 chars of mint address (e.g. "CJUyV594")
--   - name   = "Market " + first 8 chars of slab (e.g. "Market GGU89iQL")
--
-- Fix 1: Backfill name/symbol from devnet_mints table where the devnet mint
--         matches the market's mint_address and the DB name is a placeholder.
--
-- Fix 2: Directly fix known markets whose collateral token has on-chain metadata
--         but no devnet_mints entry (e.g. user-created BTC tokens on devnet).
--
-- After this migration, the markets page search for "BTC", "perp", "bonk", etc.
-- will work for previously-anonymous markets.

-- ── Step 1: Backfill from devnet_mints ──────────────────────────────────────
-- Update markets where name matches "Market [slab_prefix]" pattern AND
-- a matching devnet_mints row exists for the same devnet mint address.
UPDATE markets m
SET
  symbol = dm.symbol,
  name   = dm.name,
  updated_at = NOW()
FROM devnet_mints dm
WHERE
  m.mint_address = dm.devnet_mint
  -- Only overwrite placeholder names (auto-generated from slab address)
  AND m.name ~ '^Market [A-Za-z0-9]{6,}$'
  -- Only overwrite placeholder symbols (truncated mint address: first 8 chars)
  AND m.symbol = LEFT(m.mint_address, 8)
  -- devnet_mints must have a proper human-readable symbol (not a truncated address)
  AND dm.symbol IS NOT NULL
  AND LENGTH(dm.symbol) > 1
  AND dm.symbol != LEFT(dm.devnet_mint, 8);

-- ── Step 2: Backfill symbol only for markets with placeholder symbol ──────────
-- Some markets may have a custom name (not the "Market XXXX" pattern) but still
-- have a truncated mint address as their symbol.
UPDATE markets m
SET
  symbol = dm.symbol,
  updated_at = NOW()
FROM devnet_mints dm
WHERE
  m.mint_address = dm.devnet_mint
  AND m.symbol = LEFT(m.mint_address, 8)
  AND dm.symbol IS NOT NULL
  AND LENGTH(dm.symbol) > 1
  AND dm.symbol != LEFT(dm.devnet_mint, 8)
  -- Exclude already-updated rows from Step 1
  AND NOT (m.name ~ '^Market [A-Za-z0-9]{6,}$');

-- ── Step 3: Fix specific markets whose token is a user-created devnet token ───
-- These markets use CJUyV594JzJpK2BUakNpm6NbmCkhQoPJWkKjfKTvxJ3C which is a
-- user-created BTC-equivalent token with no metadata or devnet_mints entry.
-- Setting symbol/name directly so search for "BTC" and "bitcoin" finds them.
UPDATE markets
SET
  symbol = 'BTC',
  name   = 'Bitcoin',
  updated_at = NOW()
WHERE
  mint_address = 'CJUyV594JzJpK2BUakNpm6NbmCkhQoPJWkKjfKTvxJ3C'
  AND symbol = 'CJUyV594';  -- only overwrite if still a placeholder
