# Kani Proof Strength Audit Results

Generated: 2026-05-15

Source prompt: `scripts/audit-proof-strength.md`.

Kani version: `cargo-kani 0.66.0`.

## Full Kani Timing Sweep

Command:

```text
scripts/run_kani_full_audit.sh
```

The v13 cutover removed the v12 slab and retired the v12 slab-specific proof
inventory. This sweep parses the remaining `tests/proofs_*.rs` files and runs
each v13 harness one-by-one with exact harness selection and a `600s` timeout.

```text
SUMMARY: 39 passed, 0 failed/timeout (0 timeout) out of 39
```

Timing artifacts:

```text
kani_audit_full.tsv
kani_audit_final.tsv
```

Aggregate timing:

| Metric | Value |
|---|---:|
| Harnesses | 39 |
| Pass | 39 |
| Fail | 0 |
| Timeout | 0 |
| Total wall-clock harness time | 1485s |
| Slowest harness | `proof_v13_bankrupt_liquidation_cannot_free_exposure_before_residual_durable` |
| Slowest harness time | 401s |

## Harness Timings

| Harness | Time | Status |
|---|---:|---|
| `proof_v13_bankrupt_liquidation_cannot_free_exposure_before_residual_durable` | 401s | PASS |
| `proof_v13_trade_fee_conservation_and_oi_symmetry` | 159s | PASS |
| `proof_v13_account_b_chunk_either_advances_or_fails_closed` | 121s | PASS |
| `proof_v13_rebalance_reduce_position_preserves_senior_claims_and_reduces_risk` | 115s | PASS |
| `proof_v13_hlock_allows_pure_risk_reducing_trade_with_principal_margin` | 105s | PASS |
| `proof_v13_resolved_close_partial_b_settlement_makes_progress_without_closing` | 97s | PASS |
| `proof_v13_bankrupt_liquidation_excludes_fee_from_residual_and_spends_insurance_once` | 70s | PASS |
| `proof_v13_bankrupt_liquidation_consumes_insurance_before_social_loss` | 58s | PASS |
| `proof_v13_permissionless_refresh_returns_partial_b_progress_without_accrual` | 51s | PASS |
| `proof_v13_attach_then_clear_leg_restores_account_local_counters_for_long` | 42s | PASS |
| `proof_v13_b_residual_booking_makes_durable_progress_or_fails_closed` | 35s | PASS |
| `proof_v13_resolved_positive_payout_snapshot_is_order_stable` | 26s | PASS |
| `proof_v13_hlock_withdraw_uses_no_positive_credit_lane` | 23s | PASS |
| `proof_v13_risk_notional_flat_zero_and_monotone_in_price` | 22s | PASS |
| `proof_v13_loss_stale_blocks_nonflat_withdrawal` | 20s | PASS |
| `proof_v13_quantity_adl_preserves_oi_symmetry_after_close` | 15s | PASS |
| `proof_v13_deposit_then_withdraw_roundtrip_preserves_accounting` | 14s | PASS |
| `proof_v13_hlock_rejects_risk_increasing_trade_before_mutation` | 10s | PASS |
| `proof_v13_released_pnl_conversion_is_residual_bounded_and_conserves_vault` | 10s | PASS |
| `proof_v13_target_effective_lag_rejects_risk_increasing_trade_before_mutation` | 10s | PASS |
| `proof_v13_side_reset_prior_epoch_account_can_clear_without_oi_underflow` | 9s | PASS |
| `proof_v13_liquidation_progress_rejects_non_reducing_scores` | 8s | PASS |
| `proof_v13_equity_active_accrual_advances_at_most_one_bounded_segment` | 7s | PASS |
| `proof_v13_equity_active_accrual_requires_protective_progress` | 7s | PASS |
| `proof_v13_close_portfolio_account_requires_clean_local_state` | 6s | PASS |
| `proof_v13_favorable_action_requires_current_full_refresh` | 6s | PASS |
| `proof_v13_fee_charge_settles_loss_before_fee` | 6s | PASS |
| `proof_v13_full_refresh_clears_stale_certificate` | 5s | PASS |
| `proof_v13_multiple_deposits_aggregate_c_tot_and_vault` | 5s | PASS |
| `proof_v13_stale_counter_transitions_are_idempotent` | 4s | PASS |
| `proof_v13_trade_dynamic_fee_cap_is_enforced_before_mutation` | 4s | PASS |
| `proof_v13_hidden_leg_rejected_by_bitmap_authority` | 3s | PASS |
| `proof_v13_hlock_is_exactly_hmin_or_hmax` | 3s | PASS |
| `proof_v13_hmin_zero_remains_available_when_no_lock_state_exists` | 2s | PASS |
| `proof_v13_oversize_position_rejected_before_oi_mutation` | 2s | PASS |
| `proof_v13_account_equity_rejects_i128_min_persistent_pnl` | 1s | PASS |
| `proof_v13_account_equity_rejects_malformed_fee_credits` | 1s | PASS |
| `proof_v13_funding_rate_above_cap_rejects_before_mutation` | 1s | PASS |
| `proof_v13_public_config_rejects_invalid_user_fund_shapes` | 1s | PASS |

## V12 Property Migration

The old v12 proof inventory had 416 Kani harnesses, but many were about the
removed slab, fixed account capacity, global cursor scan, v12 reserve queues,
or wrapper-era entrypoints. The migrated v13 suite keeps the properties that
still apply to the new account-local architecture and proves them against the
real v13 production methods.

Migrated property families added in this pass:

| v12 property family | v13 coverage |
|---|---|
| Deposit/withdraw accounting roundtrip | `proof_v13_deposit_then_withdraw_roundtrip_preserves_accounting` and `v13_deposit_withdraw_roundtrip_preserves_accounting` |
| Multiple deposits aggregate into senior totals | `proof_v13_multiple_deposits_aggregate_c_tot_and_vault` |
| Account close/reclaim requires clean local state | `proof_v13_close_portfolio_account_requires_clean_local_state` and `v13_close_portfolio_account_requires_clean_local_state` |
| Malformed signed fee-credit state fails closed | `proof_v13_account_equity_rejects_malformed_fee_credits` |
| Conservative risk-notional arithmetic | `proof_v13_risk_notional_flat_zero_and_monotone_in_price` |
| Position bounds reject before OI mutation | `proof_v13_oversize_position_rejected_before_oi_mutation` and `v13_oversize_position_is_rejected_before_oi_mutation` |
| Funding cap rejects before state mutation | `proof_v13_funding_rate_above_cap_rejects_before_mutation` and `v13_funding_rate_above_cap_rejects_before_state_mutation` |
| Dynamic trade-fee conservation and OI symmetry | `proof_v13_trade_fee_conservation_and_oi_symmetry` and `v13_trade_fee_conserves_vault_and_keeps_oi_symmetric` |
| Released PnL conversion cannot mint beyond residual | `proof_v13_released_pnl_conversion_is_residual_bounded_and_conserves_vault` and `v13_released_pnl_conversion_is_bounded_by_residual_not_profit_only` |
| Permissionless refresh must return partial B progress | `proof_v13_permissionless_refresh_returns_partial_b_progress_without_accrual` and `v13_permissionless_refresh_returns_partial_b_progress_without_failing` |
| Public user-fund config must keep recovery/profile guarantees enabled | `proof_v13_public_config_rejects_invalid_user_fund_shapes` and `v13_public_init_rejects_disabled_recovery_profile` |

Slab-only v12 proofs were not ported because v13 removed the global account slab
and full-market cursor sweep from the safety model.

## Static Strength Scan

Inventory by file:

| File | Harnesses |
|---|---:|
| `tests/proofs_v13.rs` | 39 |

Strength indicators:

| Check | Result |
|---|---:|
| Harnesses over v13 production code | 39 |
| Harnesses with symbolic inputs or symbolic branch choices | 18 |
| Harnesses with `kani::cover!` reachability checks | 20 |
| Explicit `kani::assume(false)` / `assume(false)` findings | 0 |
| Confirmed vacuous harnesses | 0 |
| Confirmed weak harnesses | 0 |

Current classification:

| Classification | Status |
|---|---|
| Non-vacuity | No confirmed vacuous harnesses found. Cover checks exercise h-min/h-max, stale set/clear, hidden-leg rejection, B-chunk progress paths, malformed fee-credit states, invalid config branches, aggregate deposit branches, bankrupt residual recovery, zero/partial insurance paths, permissionless partial-B refresh, released-PnL zero/positive conversion paths, resolved partial-B close progress, and both covered rebalance reduction paths. |
| Weak proofs | No confirmed weak proofs in the v13 inventory. |
| Inductive strength | The stale-counter proof is close to an account-local inductive transition proof. The remaining proofs are strong production-code safety/liveness harnesses, not a complete arbitrary-state inductive proof system. |
| Practical proof boundary | The suite proves key v13 account-local invariants over the real production methods: h-lock state selection, provenance/hidden-leg fail-closed behavior, stale counter idempotence and refresh clearing, i128::MIN and malformed fee-credit rejection, deposit/withdraw accounting, aggregate senior accounting, close-account local-state gating, risk-notional monotonicity, position-bound fail-before-mutation, B-chunk progress/fail-closed behavior, full-refresh gating, monotonic liquidation-score rejection, loss-before-fee ordering, account-free equity-active accrual protective-progress gating, one-segment bounded catchup, funding-rate cap fail-before-mutation, dynamic trade-fee cap enforcement, dynamic trade-fee conservation and OI symmetry, target/effective lag risk-increase rejection, h-lock risk-increase rejection, h-lock risk-reducing trade liveness under no-positive-credit margin, h-lock withdrawal no-positive-credit gating, released-PnL conversion bounded by residual, loss-stale nonflat withdrawal blocking, bankrupt liquidation insurance-before-social-loss ordering, bankrupt residual durability before exposure release, uncollectible liquidation-fee exclusion from residual loss, resolved partial-B close liveness, resolved positive-payout snapshot fairness, durable B residual booking, prior-epoch reset clearing, quantity-ADL OI symmetry, and rebalance strict risk-progress with senior-claim preservation. |

## Rust Test Matrix

| Command | Result |
|---|---|
| `cargo test` | PASS |
| `cargo test --features test` | PASS |

The Rust suite currently covers the v13 spec regression tests, including 49
v13 spec tests in `tests/v13_spec_tests.rs`.

## Audit Conclusion

All v13 Kani proofs pass within the 10-minute per-harness cap, and no weak or
vacuous proof was identified in this pass. The v12 proof inventory has been
reviewed by property family; slab-specific proofs remain retired, and the
applicable accounting, fee, risk-notional, position-bound, config, close,
conversion, and permissionless progress properties have v13 production-code
tests/proofs.
