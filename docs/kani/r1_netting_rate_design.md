# Kani design: R1 netting rate and the pending-credit counter (NOT RUN)

Branch `fix/v22-r1-equity-cadence` (PR #282). Design only, per the "design first, run once on
final code" rule. Nothing here has been compiled under Kani.

## Changed code under proof

1. `V16Core::source_credit_netting_rate(state, booked_loss, extra_claims_num, pending_other_num)`
   and `V16Core::source_credit_protective_rate(state, pending_num)`: pure `u128` / `U256` functions.
2. `consume_validated_account_source_credit_for_loss_not_atomic` (the `netting: Some(..)` arm): picks
   a rate per consumed domain, then the existing consumption.
3. `kf_pending_credit_{long,short}` (`V16PodI128`, appended last in `EngineAssetSlotV16Account`),
   written only by `add_kf_pending_credit` from the K/F settle entry and the forfeit path, read
   only by `kf_pending_credit_num`.
4. `kani_eq_engine_asset_slot_v16_account` now also compares the two counters.

## Properties (all small, no loops)

### P1 rate bounds: `proof_netting_rate_bounds` (pure)
For any valid `SourceCreditStateV16` (shape from `validate_source_credit_state_shape_static`) and any
`booked_loss <= 2^64`, `extra_claims_num`, `pending_other_num < 2^100`:
`state.credit_rate_num <= netting_rate <= CREDIT_RATE_SCALE`, and the same for the protective rate.
Cases: `claims == 0` returns the stored rate; `denominator == 0 || numerator >= denominator` returns
`CREDIT_RATE_SCALE`; otherwise `floor(num * 1e12 / den) < 1e12` then `max(stored, .)`.
Lemma needed: `U256::checked_mul/checked_div` on `u128 * 1e12` never returns `None` (product < 2^168).
Arithmetic: use a contract on `mul_div_floor_u256` (`result*den <= num*scale < (result+1)*den`) instead of
unrolling the 256-bit division; same technique as the #277 lemma set.

### P2 monotonicity: `proof_netting_rate_monotone`
For fixed `state`, `extra`, `pending_other`: `booked_a <= booked_b => rate(a) <= rate(b)`; for fixed
`booked`: `extra_a <= extra_b => rate(b) <= rate(a)`; `pending_other_a <= pending_other_b =>
rate(a) <= rate(b)`. Cross-multiplication form: compare `n1*d2` vs `n2*d1` in `U256`, so no division
is involved in the statement (division only in the rate itself; apply the P1 floor lemma twice).

### P3 never creates support: `proof_netting_consumption_capped_by_available`
Frame for `consume_validated_account_source_credit_for_loss_not_atomic` with `netting = Some(ctx)`
and ONE occupied source slot: `counterparty_credit_consumed + insurance_credit_consumed <=
available_backing / BOUND_SCALE` of that domain, and `face_burn_num <= unliened` of the slot. This
is the structural guarantee that a larger rate prices burns but cannot extract more value:
`consumable = min(floor(unliened * rate / 1e12) / BOUND_SCALE, available / BOUND_SCALE)` and `take
<= consumable`. Proof by the two `min` bounds; the existing proof of the `None` arm is reused for
the consumption body by showing the `Some` arm only changes the local `rate`.

### P4 `None` arm unchanged: `proof_netting_none_arm_is_the_stored_rate`
`netting = None` selects `source_credit.credit_rate_num` (Resolved conversion, forfeit, convert):
`kani_account_source_realizable_support` and the existing conversion proofs stay valid unchanged.

### P5 pending counter, writer discipline: `proof_pending_credit_add_saturating`
`add_kf_pending_credit(domain, delta)` for any `domain < configured_domains`: no panic, the other
side's counter is unchanged, the target counter equals `old.saturating_add(delta)`. Reader:
`kf_pending_credit_num` returns `max(counter, 0)` as `u128`.

### P6 slot equality shim: `proof_engine_asset_slot_eq_covers_pending`
`kani_eq_engine_asset_slot_v16_account(a, b)` is reflexive, symmetric, and for two slots equal except
for `kf_pending_credit_long` (resp. `_short`) returns `false`. Standard "discriminates every field"
shape used for the other fields; one harness per new field with `kani::any::<[u8;16]>()` for the
differing bytes. Also the empty-slot predicate `is_empty_for_activation` requires both counters 0.

### P7 flow balance (the part Kani is NOT asked to prove)
That the counter equals credited-minus-realized claims over a whole market lifetime is a statement
over unbounded sequences of settlements and is NOT proved. It is covered by the executable
reconciliation in `tests/v22_r1_multi_claimant.rs` (`reconcile_pending`: counter <= claim stock after
every settle, <= rounding atoms at quiescence) and by mutation tests. A Kani inductive step is possible
for the single-settle transition (`entry` adds exactly `claims_after - claims_before` or subtracts
exactly `net.unsigned_abs() * BOUND_SCALE`): harness `proof_settle_entry_updates_pending_by_exact_flow`
on a one-leg account, two cases (net > 0, net < 0). Design cost moderate (the entry is large; stub the
asset update with `kani::stub` as the #277 proofs do).

## What would invalidate existing proofs
- Any proof that builds `EngineAssetSlotV16Account` by struct literal (the `Default`/`from` helpers
  cover it) or compares slots field by field: re-run after adding the two fields.
- `proof_..._settle_*` harnesses that call `apply_account_kf_settlement_entry`: the loss arm now calls
  `apply_haircut_bounded_close_loss_to_pnl_in_loss_domain` instead of `apply_signed_kf_delta_to_pnl`
  (same behaviour when `netting` is `None`); `kani_apply_signed_kf_delta_to_pnl` is unchanged.

## Run plan
One run on the final tree: P1, P2, P4, P5, P6 are seconds each (pure or tiny frames); P3 and P7-step are
the expensive ones (budget like `proof_v16_kernel_*` consumption proofs, unwind 70 is not needed, no
loops). Check cover properties for every harness (`kani::cover!`) so no proof is vacuous.

## Round 4 additions (still NOT run)

- `source_credit_domain_has_locked_claims(state)`: pure; `proof_domain_lock_is_the_or_of_four_lien_counters`
  (true iff one of `valid/impaired_liened_backing_num`, `valid/impaired_liened_insurance_num` is non-zero).
- `clamp_kf_pending_credit_to_claims(domain)` and the clamped reader `kf_pending_credit_num`:
  `proof_pending_clamp`: after the clamp `counter <= claims`; the other side's counter and every
  other field of the slot are unchanged; the reader returns `min(max(counter, 0), claims)`.
- S9 invariant `counter <= claims` at the end of `apply_account_kf_settlement_entry` and the forfeit
  path: single-settle harness extension of `proof_settle_entry_updates_pending_by_exact_flow` with
  the post-condition. The unbounded-sequence statement stays executable
  (`tests/v22_r1_reversal.rs`, `tests/v22_r1_multi_claimant.rs::reconcile_pending`).
- Protective branch selection is now a function of the DOMAIN state only: add to P3 that the rate
  chosen for a domain does not depend on which account is netting (same inputs -> same rate for any
  two account entries), which the earlier per-account `locked` did not satisfy.
