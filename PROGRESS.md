## Last Heartbeat
2026-03-17 14:03 UTC

## Current Task
PR #1383 — fix pushed, awaiting CI + security re-approval

## Status
- PR #1383 OPEN — pushed 9602decb: added recentBlockhash + feePayer + lastValidBlockHeight to USDC mint path
- PR #1381 (GH#1380 env var fix) — **MERGED** ✅
- No backlog tasks assigned.

## Changes Made This Session
1. Fixed USDC mint path: set `tx.recentBlockhash` and `tx.feePayer = mintAuthPk` before `signTransaction()`
2. Updated `confirmTransaction` to use `{ signature, blockhash, lastValidBlockHeight }` form
3. All faucet tests passing
4. Notified security, pm, devops via Collector API

## Next Steps
1. Wait for CI green on new commit
2. Security re-approval on PR #1383
3. PM merge
4. Pick up next task from Collector API

## Blockers
- Awaiting security re-review of PR #1383

## Recent Decisions
- NEXT_PUBLIC_DEFAULT_NETWORK is the canonical network env var
- Sealed signer pattern (getDevnetMintSigner) is the standard for all devnet mint operations
- On-chain authority check before MintTo = 400 not 500 (GH#1382 pattern)
- oracle_markets table uses slab_address as PK (not mint_address)
- recentBlockhash + feePayer must always be set before signTransaction when using sendRawTransaction
