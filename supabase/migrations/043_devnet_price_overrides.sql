-- Migration: 043_devnet_price_overrides
-- Table for static USD price overrides for devnet-only tokens that have no
-- mainnet equivalent (e.g. SEX slab 3bmCyPee). oracle-keeper checks this
-- table as the last-resort price source after DexScreener, Jupiter, and the
-- devnet_mints mainnet_ca lookup all fail.
--
-- Usage:
--   INSERT INTO devnet_price_overrides (devnet_mint, price_usd, notes)
--   VALUES ('<mint_pubkey>', 1.00, 'SEX token — fixed $1 devnet price');
--
-- The oracle-keeper polls this table every DISCOVERY_INTERVAL_MS (5min) and
-- caches the result in memory. Price is returned as the authoritative source
-- with source='devnet-override' when all other sources fail.

CREATE TABLE IF NOT EXISTS devnet_price_overrides (
  devnet_mint   TEXT          PRIMARY KEY,
  price_usd     NUMERIC       NOT NULL CHECK (price_usd > 0),
  notes         TEXT,
  created_at    TIMESTAMPTZ   NOT NULL DEFAULT NOW(),
  updated_at    TIMESTAMPTZ   NOT NULL DEFAULT NOW()
);

-- Auto-update updated_at on row change
CREATE OR REPLACE FUNCTION update_devnet_price_overrides_updated_at()
RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
  NEW.updated_at = NOW();
  RETURN NEW;
END;
$$;

CREATE OR REPLACE TRIGGER trg_devnet_price_overrides_updated_at
  BEFORE UPDATE ON devnet_price_overrides
  FOR EACH ROW EXECUTE FUNCTION update_devnet_price_overrides_updated_at();

-- Service role access only (same pattern as devnet_mints)
ALTER TABLE devnet_price_overrides ENABLE ROW LEVEL SECURITY;
-- No policies: service_role bypasses RLS, anon/authenticated have no grants.

-- Example: seed the SEX slab collateral mint with a $1.00 placeholder.
-- Replace <SEX_COLLATERAL_MINT> with the actual mint address (query:
--   SELECT mint_address FROM markets WHERE slab_address = '3bmCyPee...' LIMIT 1;)
-- DevOps / PM can UPDATE price_usd once a real reference price is known.
--
-- INSERT INTO devnet_price_overrides (devnet_mint, price_usd, notes)
-- VALUES (
--   '<SEX_COLLATERAL_MINT>',
--   1.00,
--   'SEX token — devnet-only, no mainnet CA. Fixed $1 placeholder.'
-- )
-- ON CONFLICT (devnet_mint) DO NOTHING;
