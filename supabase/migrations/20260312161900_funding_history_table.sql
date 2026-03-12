-- Migration 20260312161900: Create funding_history table
--
-- Context: The funding_rates migration (20260214190000_funding_rates.sql.skip)
-- was intentionally skipped in the automated migration pipeline because some
-- of its view DDL conflicted with migration 037. This migration was applied
-- directly to production Supabase on 2026-03-12 to resolve API 500 errors
-- on GET /funding/:slab (missing funding_history table).
--
-- Supabase recorded the remote application with timestamp 20260312161900.
-- This local file aligns the local migration history so supabase db push/pull
-- work again. It is idempotent — safe to run even if applied previously.
--
-- The market_stats columns (funding_rate, net_lp_pos etc.) were already
-- present from migration 037 and do not need re-adding here.

-- ============================================================================
-- funding_history table for time-series funding rate data
-- ============================================================================

CREATE TABLE IF NOT EXISTS funding_history (
  id UUID DEFAULT gen_random_uuid() PRIMARY KEY,
  market_slab TEXT NOT NULL REFERENCES markets(slab_address) ON DELETE CASCADE,
  slot BIGINT NOT NULL,
  timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  rate_bps_per_slot BIGINT NOT NULL,
  net_lp_pos TEXT NOT NULL,
  price_e6 BIGINT NOT NULL,
  funding_index_qpb_e6 TEXT NOT NULL,
  created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Efficient time-series queries by market + time
CREATE INDEX IF NOT EXISTS idx_funding_history_market_time
  ON funding_history(market_slab, timestamp DESC);

CREATE INDEX IF NOT EXISTS idx_funding_history_slot
  ON funding_history(market_slab, slot DESC);

-- Public read (consistent with market_stats access pattern)
ALTER TABLE funding_history ENABLE ROW LEVEL SECURITY;
DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM pg_policies
    WHERE tablename = 'funding_history'
      AND policyname = 'Public read access'
  ) THEN
    CREATE POLICY "Public read access" ON funding_history FOR SELECT USING (true);
  END IF;
END $$;

-- Notify PostgREST to reload schema cache
NOTIFY pgrst, 'reload schema';
