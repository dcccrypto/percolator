-- Migration 039: Ensure devnet_airdrop_claims table exists + reload PostgREST schema
--
-- Context: Migration 038 added devnet_airdrop_claims but was never applied to
-- production Supabase, causing Sentry FE errors:
--   "Could not find table public.devnet_airdrop_claims in schema cache"
-- (9 events, last seen 2026-03-11, PERC-749)
--
-- This migration is idempotent (IF NOT EXISTS guards) so it is safe to run
-- even if 038 was previously applied. The trailing NOTIFY forces PostgREST
-- to reload its schema cache, resolving the "not in schema cache" error.

-- Create table (no-op if 038 was already applied)
CREATE TABLE IF NOT EXISTS devnet_airdrop_claims (
  id          BIGSERIAL     PRIMARY KEY,
  wallet      TEXT          NOT NULL,
  mint        TEXT          NOT NULL,
  claimed_at  TIMESTAMPTZ   NOT NULL DEFAULT NOW()
);

-- Unique index: enforces 1-claim-per-wallet-per-mint (INSERT-as-gate)
CREATE UNIQUE INDEX IF NOT EXISTS devnet_airdrop_claims_wallet_mint_idx
  ON devnet_airdrop_claims(wallet, mint);

-- Lookup index for the 24h window check
CREATE INDEX IF NOT EXISTS devnet_airdrop_claims_claimed_at_idx
  ON devnet_airdrop_claims(claimed_at);

-- No public access — service_role bypasses RLS; anon/authenticated have zero grants
ALTER TABLE devnet_airdrop_claims ENABLE ROW LEVEL SECURITY;

-- Force PostgREST to reload its schema cache so the table becomes visible immediately.
-- Without this, the new table may not appear in PostgREST's schema cache until the
-- next scheduled reload (up to 24h), causing the "not in schema cache" Sentry error.
NOTIFY pgrst, 'reload schema';
