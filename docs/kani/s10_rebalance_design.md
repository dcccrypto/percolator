# S10: unclaimed-backing rebalance, Kani design (NOT RUN)

Function: `rebalance_unclaimed_backing_across_asset_domains_not_atomic(asset_index)`
(`src/v16.rs`), called once at the end of every K/F settlement entry
(`apply_account_kf_settlement_entry`). Pure re-attribution between the two source domains of ONE
asset. No token, no account field, no claim changes.

## Obligations (to prove once, on the final code, per the run-once rule)

1. Conservation: for the asset, `fresh_reserved(long) + fresh_reserved(short)` and
   `valid_liened + impaired_liened` are unchanged; `vault`, `c_tot`, `insurance`, every account,
   every `positive_claim_bound_num` and `exact_positive_claim_num` are unchanged.
2. Bound: `moved <= min(fresh_unliened(src), available(src) - claims(src))` and
   `moved <= claims(dst) - available(dst)` (so the source's credited claimants keep rate 1 if they
   had it, and the destination's rate never exceeds 1: `available(dst) + moved <= claims(dst)`).
3. Granularity: `moved` is a multiple of `BOUND_SCALE` (whole atoms), so
   `validate_shape` vault accounting (atoms) is unaffected.
4. Ledger: `validate_source_domain_ledger` and `reservation_encumbrance_proof_for_domain` hold for
   both domains after the move (the two `prepare_*` deltas are the existing withdraw and add deltas,
   so their own proofs apply; the new content is the composition and the bound in 2).
5. Expiry: the destination bucket's expiry after the move equals what
   `fresh_counterparty_backing_expiry_slot(dst)` returned before it (an existing Fresh bucket keeps
   its expiry, an Empty or Expired one opens at now + horizon), i.e. the rule
   `book_realized_loss_backing_for_domain_not_atomic` uses. No bucket's expiry ever moves later
   because of a move into it except by that rule.
6. Idempotence: a second call with no intervening change moves 0 (`excess` or `shortfall` is 0
   after the first call, in both directions).
7. Never reverts a settlement: the function is only reached after the entry's own state is
   consistent; its `Err` paths are the existing delta errors and fail closed (SVM rollback).

## Contract shim

Add `kani_rebalance_unclaimed_backing_across_asset_domains_not_atomic` next to the R1 shims; the
settle-entry proofs that assert "no state change outside the account and its source domains" need
the two domains of the asset added to their frame.

## Not proved, by design

Which accounts benefit (the rebalance is attribution-free). That every claim ends at rate 1 in
every world is a property of the zero-sum K/F ledger, shown by simulation (600 reversal worlds,
all 16 #457 masks), not by this function in isolation.
