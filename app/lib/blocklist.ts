/**
 * Client-side blocklist for known-bad / stale market slab addresses.
 *
 * These are markets that have been blocked in the API route
 * (app/api/markets/route.ts HARDCODED_BLOCKED_MARKETS) but whose rows
 * are still visible via the Supabase anon client in
 * markets_with_stats. Any hook or page that queries the view directly
 * MUST filter these out before rendering or aggregating values.
 *
 * Keep in sync with the server-side HARDCODED_BLOCKED_MARKETS set in
 * app/api/markets/route.ts.
 */
export const BLOCKED_SLAB_ADDRESSES: ReadonlySet<string> = new Set([
  // Stale SOL/USD slab — on-chain slab no longer exists; shows $100 last_price
  // causing "Failed to load market" on click. Blocked via PR #1179.
  "BxJPaMaCfEGTBsjZ8wfj3Yfzf4wpasmxKAEvqZZRcGPP",
  // GH#837: wrong oracle_authority — price manipulation risk. Blocked via security review.
  "HjBePQZnoZVftg9B52gyeuHGjBvt2f8FNCVP4FeoP3YT",
  // GH#1218: NL/USD slab — corrupt on-chain OI state (9e12 micro-units per side → $89.2M
  // false total OI). Migration 045 zeroed the DB but the indexer re-synced from on-chain.
  // Blocked permanently until on-chain state is corrected. PR #1219.
  "H5Vunzd2yAMygnpFiGUASDSx2s8P3bfPTzjCfrRsPeph",
  // GH#1357 / PR#1362: no-liquidity slabs causing /funding/ 500 errors (Sentry).
  // Previously expected in BLOCKED_MARKET_ADDRESSES env var; hardcoded here so the
  // middleware guard (pre-rewrite) blocks them even in fresh deployments without env config.
  // SEX/USD — devnet-only token, empty vault, phantom OI (migration 048). PR #1377.
  "3bmCyPee8GWJR5aPGTyN5EyyQJLzYyD8Wkg9m1Afd1SD",
  // Empty-vault phantom-OI slab (migration 048). PR #1377.
  "3YDqCJGz88xGiPBiRvx4vrM51mWTiTZPZ95hxYDZqKpJ",
  // Empty-vault phantom-OI slab (no on-chain liquidity). PR #1377.
  "3ZKKwsKoo5UP28cYmMpvGpwoFpWLVgEWLQJCejJnECQn",
  // GH#1398: Garbage test market — symbol "11111111", 333x max_leverage,
  // oracle_authority = system program (11111111...), cannot receive price updates.
  // Deployer = DEVNET_MINT_AUTHORITY_KEYPAIR (accidental test deployment).
  "CRJH9Gtk7qQDdjzDufnAZdfa7AHisfvxCmVVvzpzQN9v",
  // GH#1398 follow-up (PR #1404): Remaining 11 phantom slabs with oracle_authority =
  // system program (11111111...). These cannot receive oracle price pushes, have no
  // real liquidity, and cause /funding/[slab] → 500 errors via backend proxy.
  // Addresses queried from markets_with_stats where oracle_authority = system program.
  "J6UU4VHbYXpCAACr5o5xjUVmquagiP2NGbbMp68VUCX9",
  "8L47yqvQRLxZ6PzW3b9jawEM79CmokBvUzeLR7mvtyuU",
  "8kkED3uZznGzSidr8kYJPd3VhzSh7LVngNUx2V1qnW9L",
  "8pKtAV3z6iTKekieF9EenQ4tk1rkAVa9oYsqe7h1PGjx",
  "Eekuz2TgXRPq3rsp5brRW5hofxLdwt6KUXbLUQCKHK9G",
  "Av3zVrW5deLpLo1qZZ7yNJ5Lq5ja4Z9ixijVhV4MuRzE",
  "CrbDmfiooBUTFfGyMhJ1hpToCrBLAXXKySBwEnLHV6kj",
  "FhpPmmuh5UDAjvEjrYBPFwmj4CP4otvsYMxtTb46p1Ss",
  "7xozYEbKhEdjQn5pCAV8bUDQGugZttqZTduPeHkoqRb8",
  "3dp3e288oPjs5w92fg26cVYQMHGuUpsj8YbSFn6wrzp4",
  "8nzjXMvdkC4fRF491QkpKE6aFTLmEcpXEnbh4wQT4iUA",
  // GH#1410: phantom slab returning HTTP 200 from /api/funding despite 404 on
  // /api/open-interest and /api/insurance. Not covered by prior blocklist entries.
  // SEX/USD devnet — empty vault, no real liquidity, causes misleading zero-filled
  // funding responses. Verified 2026-03-19 UTC.
  "3bmCyPeeDwAfLbhfnRpYJHkWVqAf3Q5JaWXGfZjbmjNp",
  // GH#1413: DfLoAzny/USD slab — phantom market with vault_balance=1M (at threshold),
  // stale on-chain OI (2T micro-units ≈ 2,000,000 tokens). Not in prior blocklist so
  // /api/open-interest/8eFFEFBY returns 200 with raw phantom data. Block to return 404.
  // Also covers /api/funding/8eFFEFBY which was returning 200 with stale zero-rate data.
  "8eFFEFBY3HHbBgzxJJP5hyxdzMNMAumnYNhkWXErBM4c",
]);

/**
 * Returns true if the slab address should be excluded from UI rendering.
 */
export function isBlockedSlab(slabAddress: string | null | undefined): boolean {
  if (!slabAddress) return false;
  return BLOCKED_SLAB_ADDRESSES.has(slabAddress);
}
