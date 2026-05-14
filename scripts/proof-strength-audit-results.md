# Kani Proof Strength Audit Results

Generated: 2026-05-14

Source prompt: `scripts/audit-proof-strength.md`.

Kani version: `cargo-kani 0.66.0`.

## Full Kani Timing Sweep

Command:

```text
scripts/run_kani_full_audit.sh
```

The v13 cutover removed the v12 slab and retired the v12 proof inventory. The
sweep now parses the remaining `tests/proofs_*.rs` files and runs each v13
harness one-by-one with exact harness selection and a `600s` timeout.

```text
SUMMARY: 21 passed, 0 failed/timeout (0 timeout) out of 21
```

Timing artifacts:

```text
kani_audit_full.tsv
kani_audit_final.tsv
```

Aggregate timing:

| Metric | Value |
|---|---:|
| Harnesses | 21 |
| Pass | 21 |
| Fail | 0 |
| Timeout | 0 |
| Total wall-clock harness time | 439s |
| Slowest harness | `proof_v13_account_b_chunk_either_advances_or_fails_closed` |
| Slowest harness time | 122s |

## Harness Timings

| Harness | Time | Status |
|---|---:|---|
| `proof_v13_account_b_chunk_either_advances_or_fails_closed` | 122s | PASS |
| `proof_v13_hlock_allows_pure_risk_reducing_trade_with_principal_margin` | 106s | PASS |
| `proof_v13_attach_then_clear_leg_restores_account_local_counters_for_long` | 43s | PASS |
| `proof_v13_b_residual_booking_makes_durable_progress_or_fails_closed` | 38s | PASS |
| `proof_v13_loss_stale_blocks_nonflat_withdrawal` | 20s | PASS |
| `proof_v13_hlock_withdraw_uses_no_positive_credit_lane` | 19s | PASS |
| `proof_v13_quantity_adl_preserves_oi_symmetry_after_close` | 16s | PASS |
| `proof_v13_hlock_rejects_risk_increasing_trade_before_mutation` | 10s | PASS |
| `proof_v13_side_reset_prior_epoch_account_can_clear_without_oi_underflow` | 9s | PASS |
| `proof_v13_equity_active_accrual_advances_at_most_one_bounded_segment` | 8s | PASS |
| `proof_v13_liquidation_progress_rejects_non_reducing_scores` | 8s | PASS |
| `proof_v13_equity_active_accrual_requires_protective_progress` | 7s | PASS |
| `proof_v13_fee_charge_settles_loss_before_fee` | 6s | PASS |
| `proof_v13_favorable_action_requires_current_full_refresh` | 5s | PASS |
| `proof_v13_full_refresh_clears_stale_certificate` | 5s | PASS |
| `proof_v13_stale_counter_transitions_are_idempotent` | 4s | PASS |
| `proof_v13_trade_dynamic_fee_cap_is_enforced_before_mutation` | 4s | PASS |
| `proof_v13_hlock_is_exactly_hmin_or_hmax` | 3s | PASS |
| `proof_v13_hmin_zero_remains_available_when_no_lock_state_exists` | 3s | PASS |
| `proof_v13_hidden_leg_rejected_by_bitmap_authority` | 2s | PASS |
| `proof_v13_account_equity_rejects_i128_min_persistent_pnl` | 1s | PASS |

## Static Strength Scan

Inventory by file:

| File | Harnesses |
|---|---:|
| `tests/proofs_v13.rs` | 21 |

Strength indicators:

| Check | Result |
|---|---:|
| Harnesses over v13 production code | 21 |
| Harnesses with symbolic inputs or symbolic branch choices | 9 |
| Harnesses with `kani::cover!` reachability checks | 8 |
| Explicit `kani::assume(false)` / `assume(false)` findings | 0 |
| Confirmed vacuous harnesses | 0 |
| Confirmed weak harnesses | 0 |

Current classification:

| Classification | Status |
|---|---|
| Non-vacuity | No confirmed vacuous harnesses found. Cover checks exercise h-min/h-max, stale set/clear, hidden-leg rejection, and B-chunk progress paths. |
| Weak proofs | No confirmed weak proofs in the v13 inventory. |
| Inductive strength | The stale-counter proof is close to an account-local inductive transition proof. The remaining proofs are strong production-code safety/liveness harnesses, not a complete arbitrary-state inductive proof system. |
| Practical proof boundary | The suite proves key v13 account-local invariants over the real production methods: h-lock state selection, provenance/hidden-leg fail-closed behavior, stale counter idempotence and refresh clearing, i128::MIN rejection, B-chunk progress/fail-closed behavior, full-refresh gating, monotonic liquidation-score rejection, loss-before-fee ordering, account-free equity-active accrual protective-progress gating, one-segment bounded catchup, dynamic trade-fee cap enforcement, h-lock risk-increase rejection, h-lock risk-reducing trade liveness under no-positive-credit margin, h-lock withdrawal no-positive-credit gating, loss-stale nonflat withdrawal blocking, durable B residual booking, prior-epoch reset clearing, and quantity-ADL OI symmetry. |

## Rust Test Matrix

| Command | Result |
|---|---|
| `cargo test` | PASS |
| `cargo test --features test` | PASS |

The Rust suite currently covers 50 wide-math unit tests and 31 v13 spec tests.

## Audit Conclusion

All v13 Kani proofs pass within the 10-minute per-harness cap, and no weak or
vacuous proof was identified in this pass. The proof boundary is intentionally
v13 account-local and now covers the newly implemented production paths for
fee/loss ordering, accrual progress gating, one-segment bounded catchup,
residual B booking, stale-certificate refresh clearing, reset lifecycle,
quantity ADL, dynamic trade fees, h-lock risk-increase rejection, h-lock
risk-reducing trade liveness, h-lock withdrawal no-positive-credit admission,
loss-stale nonflat withdrawal blocking, and permissionless account-local progress. The
retired v12 slab proofs no longer apply after the architectural cutover.
