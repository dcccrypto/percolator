-- #HYPERP: Create oracle_markets table for explicit oracle config registration.
--
-- Problem: All markets created via Quick Launch on devnet have oracle_mode='admin'
-- because the Create Market wizard forces admin mode for devnet-mirrored mainnet
-- tokens (isDevnetMirror=true). This means the oracle-keeper's Supabase discovery
-- finds zero HYPERP markets even though oracle_keeper PR #1123 added UpdateHyperpMark
-- support.
--
-- Solution: A standalone oracle_markets table that the oracle-keeper checks as an
-- explicit override/supplement. DevOps can INSERT rows here to register known HYPERP
-- markets (or any oracle config) without touching the markets table directly.
-- oracle_markets rows take precedence over markets.oracle_mode for the keeper.
--
-- Usage:
--   INSERT INTO oracle_markets (slab_address, oracle_type, dex_pool_address)
--   VALUES ('<slab_pubkey>', 'hyperp', '<dex_pool_pubkey>');
--
-- The oracle-keeper reads this table every DISCOVERY_INTERVAL_MS (30s) and will
-- immediately start cranking UpdateHyperpMark for any hyperp entries.

CREATE TABLE IF NOT EXISTS oracle_markets (
  slab_address TEXT PRIMARY KEY REFERENCES markets(slab_address) ON DELETE CASCADE,
  oracle_type TEXT NOT NULL CHECK (oracle_type IN ('pyth', 'hyperp', 'admin')),
  dex_pool_address TEXT,
  pyth_feed_id TEXT,
  enabled BOOLEAN NOT NULL DEFAULT true,
  notes TEXT,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

  -- Ensure hyperp entries always have a pool address
  CONSTRAINT oracle_markets_hyperp_needs_pool
    CHECK (oracle_type != 'hyperp' OR dex_pool_address IS NOT NULL)
);

-- Index for keeper queries (hyperp, enabled-only)
CREATE INDEX IF NOT EXISTS idx_oracle_markets_type_enabled
  ON oracle_markets (oracle_type)
  WHERE enabled = true;

-- Trigger: keep updated_at current
CREATE OR REPLACE FUNCTION oracle_markets_set_updated_at()
RETURNS TRIGGER AS $$
BEGIN
  NEW.updated_at = NOW();
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER oracle_markets_updated_at
  BEFORE UPDATE ON oracle_markets
  FOR EACH ROW EXECUTE FUNCTION oracle_markets_set_updated_at();

-- RLS
ALTER TABLE oracle_markets ENABLE ROW LEVEL SECURITY;

-- Public read (oracle-keeper uses service role but expose for admin UI)
CREATE POLICY "oracle_markets_public_read"
  ON oracle_markets FOR SELECT USING (true);

-- Only service role can mutate
CREATE POLICY "oracle_markets_service_write"
  ON oracle_markets FOR ALL
  TO service_role
  USING (true)
  WITH CHECK (true);

-- Reload PostgREST schema cache so the new table is immediately queryable
NOTIFY pgrst, 'reload schema';

-- ── Seed ──────────────────────────────────────────────────────────────────────
-- Insert known HYPERP devnet markets below.
-- Obtain slab_address + dex_pool_address from the markets table or Quick Launch logs.
-- Example (uncomment + replace with real addresses):
--
-- INSERT INTO oracle_markets (slab_address, oracle_type, dex_pool_address, notes)
-- VALUES
--   ('SLAB_PUBKEY_1', 'hyperp', 'DEX_POOL_PUBKEY_1', 'BONK-PERP PumpSwap pool'),
--   ('SLAB_PUBKEY_2', 'hyperp', 'DEX_POOL_PUBKEY_2', 'WIF-PERP Raydium CLMM pool')
-- ON CONFLICT (slab_address) DO UPDATE
--   SET oracle_type      = EXCLUDED.oracle_type,
--       dex_pool_address = EXCLUDED.dex_pool_address,
--       notes            = EXCLUDED.notes,
--       enabled          = EXCLUDED.enabled,
--       updated_at       = NOW();
