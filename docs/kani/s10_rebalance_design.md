# S10: unclaimed-backing rebalance, Kani design (NOT RUN; one run on the final code)

Function: `rebalance_unclaimed_backing_across_asset_domains_not_atomic(asset_index)` (`src/v16.rs`),
called at the end of `apply_account_kf_settlement_entry`, after the leg's own cohort discharge
(`kernel_settle_kf_stale_cohort`) and `set_asset_state`, and only under the V1 guard. A pure
re-attribution between the two source domains of ONE asset. No token, no account field, no claim
changes.

## Rule (what changed after the security review of #286)

1. **V1 guard (caller):** fire only when `asset.stale_account_count_long == 0 &&
   asset.stale_account_count_short == 0`, read from the asset state the entry just stored. Zero
   means every stored leg of the asset has settled to the current K/F cohort: no unsettled winner
   can still claim, no unsettled loser can still pay, so the remaining excess and shortfall are
   real. Idle cost: two integer compares.
2. **Ring-fence:** `loss_booked_unclaimed_{long,short}` (engine slot tail, `BOUND_SCALE` units).
   Incremented only by `book_realized_loss_backing_for_domain_not_atomic` (a loser's realised loss
   or re-booked claim support) and by the move itself (destination). Taken down by
   `set_backing_bucket_for_domain` by every reduction of `fresh_unliened_backing_num` (the loser-
   booked part is assumed to go first, so the counter can only under-count) and clamped to it.
   Invariant `counter <= fresh_unliened_backing_num`, enforced by `validate_shape`. Only this part
   may move: provider deposits (`deposit_fresh_counterparty_backing_not_atomic`) never enter it.
3. **Amount:** `min(counter(src), fresh_unliened(src), available(src) - claims(src), claims(dst) -
   available(dst))`, rounded down to whole atoms, both directions (long->short then short->long).
4. **Guards inside:** source `Fresh` and `expiry_slot > now` (else the withdraw delta returns
   `LockActive`), destination accepts a booking (`loss_domain_accepts_realized_backing`) and is not
   `Impaired` (else the add delta returns `LockActive`). Bucket and counter are read before any
   source-credit state, so the idle path skips the U256-free but wide `SourceCreditStateV16` decode.
5. **Moved backing's expiry:** it joins the destination bucket and takes the DESTINATION's expiry
   (an existing Fresh bucket keeps its own; an Empty or Expired one opens at `now + horizon`,
   `fresh_counterparty_backing_expiry_slot`). It loses the source bucket's own lifetime: in
   random worlds the destination's expiry was later than the source's in about 55-68 percent of
   firings and earlier in 27-35 percent (security review), so a move can defer or advance a lapse
   of that backing to the junior residual. The all-cranked ideal does the same when a claim burn
   re-books its support into the loss domain.
6. **Receivable refill:** the add delta first repays the destination's `provider_receivable`
   (`consumed_liened`), as any booking does; the source side never has a principal change.

## Proof obligations (the reviewer's eight, plus the ring-fence counter), to be proved once

1. Conservation: for the asset, total `fresh_unliened` and `fresh_reserved` across the two domains,
   `valid_liened`, `impaired_liened`, insurance credit reserved, `spent_backing`, claims and exact
   claims, vault, insurance, `c_tot` and every account field are unchanged; the destination
   `provider_receivable` never grows.
2. Bound: `moved <= min(counter(src), fresh_unliened(src), available(src) - claims(src),
   claims(dst) - available(dst))`, `moved % BOUND_SCALE == 0`, destination available never above
   claims, source covered claimants never lose coverage.
3. Guard (V1 + ring-fence): `moved > 0` implies `stale_account_count_long == 0 &&
   stale_account_count_short == 0` (when reached through the settle entry), and `moved <=
   counter(src)`; `counter <= fresh_unliened` after EVERY mutating path (book, move, set bucket,
   withdraw, lien create, lien release, expiry, activation reset).
4. Totality: for every state that passes `validate_shape` the function returns `Ok` (no new
   fail-closed path inside a settle); covers the expired source, Impaired destination,
   Empty/Expired destination and receivable cases.
5. Idempotence and symmetry: a second call moves 0.
6. Ledger and expiry: `validate_source_domain_ledger` and `reservation_encumbrance_proof_for_domain`
   hold for both domains; the destination expiry rule (item 5 above); the source bucket status
   transition (Fresh to Empty/Expired when drained) matches `prepare_counterparty_backing_withdraw_delta`.
7. Frame: the settle-entry proofs gain both domains of the asset and the two counters in the frame,
   nothing else (no change to `kf_pending_credit` beyond the existing clamp).
8. Covers: the move is reachable (`moved > 0`) in both directions, and the V1-false branch is
   reachable (the vacuity detector).
9. Provider attribution (ring-fence): a deposit leaves the counter unchanged; a provider withdraw
   never increases it; a source reduction never leaves `counter > fresh_unliened`.

Contract shim to add: `kani_rebalance_unclaimed_backing_across_asset_domains_not_atomic` next to the R1
shims (the test seam `rebalance_unclaimed_backing_for_test_not_atomic` already skips the V1 guard).

## Not provable by Kani (kept as simulation gates)

Every account at or below ideal in random-cadence, attacker-schedule, cash-out and LP-victim sweeps,
and the 600 / 900 / 16-mask worlds. See `finding-stranded-backing-2026-10-07.md`.
