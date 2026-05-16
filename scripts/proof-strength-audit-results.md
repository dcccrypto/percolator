# Kani Proof Strength Audit Results

Generated: 2026-05-16

Source prompt: `scripts/audit-proof-strength.md`.

## Current Inventory

Static inventory from the current `v14` tree:

| Item | Count |
|---|---:|
| Rust spec/fuzz tests | 125 |
| Kani proofs | 125 |
| Kani cover checks | 195 |
| Kani assumptions | 123 |

Breakdown:

| File | Tests | Kani proofs | Cover checks |
|---|---:|---:|---:|
| `tests/v14_spec_tests.rs` | 124 | 0 | 0 |
| `tests/v14_fuzzing.rs` | 1 | 0 | 0 |
| `tests/proofs_v14.rs` | 0 | 118 | 187 |
| `tests/proofs_v14_arithmetic.rs` | 0 | 7 | 8 |

The v14 suite is over production engine code and shared production arithmetic
helpers. It is not a model-only proof suite.

## Latest Completed Full Kani Timing Sweep

Command:

```text
scripts/run_kani_full_audit.sh
```

Last completed sweep date: 2026-05-15.

That sweep covered the then-current 57-proof inventory:

```text
SUMMARY: 57 passed, 0 failed/timeout (0 timeout) out of 57
```

Timing artifacts:

```text
kani_audit_full.tsv
kani_audit_final.tsv
```

Aggregate timing from that completed sweep:

| Metric | Value |
|---|---:|
| Harnesses | 57 |
| Pass | 57 |
| Fail | 0 |
| Timeout | 0 |
| Total wall-clock harness time | 2372s |
| Slowest harness | `proof_v14_bankrupt_liquidation_cannot_free_exposure_before_residual_durable` |
| Slowest harness time | 397s |

The current tree has 125 Kani proofs, so the timing artifacts must be regenerated
before using them as a current full-proof pass record.

Focused incremental proofs added after the last completed full sweep:

| Harness | Time | Status |
|---|---:|---|
| `proof_v14_market_wire_roundtrip_preserves_valid_runtime_state` | 71s | PASS |
| `proof_v14_portfolio_wire_roundtrip_preserves_valid_runtime_state` | 452s | PASS |
| `proof_v14_persisted_wire_rejects_i128_min_economic_fields` | 106s | PASS |
| `proof_v14_repeated_account_b_chunks_complete_bounded_small_residual` | 37s | PASS |
| `proof_v14_full_refresh_settles_and_scores_two_active_assets` | 33s | PASS |
| `proof_v14_non_deficit_public_paths_do_not_decrease_insurance` | 31s | PASS |
| `proof_v14_favorable_locks_block_released_pnl_conversion_before_mutation` | 32s | PASS |
| `proof_v14_persisted_wire_rejects_provenance_and_hidden_leg_smuggling` | 213s | PASS |
| `proof_v14_b_stale_trade_preflight_rolls_back_partial_side_effects` | 56s | PASS |
| `proof_v14_deposit_into_stale_or_b_stale_account_does_not_unlock_favorable_actions` | 7s | PASS |
| `proof_v14_quantity_adl_preserves_oi_symmetry_after_close` | 184s | PASS |
| `proof_v14_quantity_adl_monotonically_shrinks_opposing_a_or_resets` | 254s | PASS |
| `proof_v14_expired_close_progress_routes_recovery_before_durable_mutation` | 15s | PASS |
| `proof_v14_dead_leg_forfeit_does_not_credit_positive_kf_delta` | 22s | PASS |
| `proof_v14_dead_leg_forfeit_books_loss_to_opposing_domain_only` | 225s | PASS |
| `proof_v14_dead_leg_forfeit_haircuts_positive_support_when_junior_impaired` | 51s | PASS |
| `proof_v14_negative_kf_settlement_uses_haircut_support_not_face_netting` | 308s | PASS |
| `proof_v14_positive_kf_delta_cures_prior_loss_at_haircut_value` | 29s | PASS |

## Slowest Harnesses From Last Completed Sweep

All per-harness timings are recorded in `kani_audit_final.tsv`.

| Harness | Time | Status |
|---|---:|---|
| `proof_v14_bankrupt_liquidation_cannot_free_exposure_before_residual_durable` | 397s | PASS |
| `proof_v14_k_pair_mul_div_floor_matches_small_reference` | 193s | PASS |
| `proof_v14_trade_fee_conservation_and_oi_symmetry` | 160s | PASS |
| `proof_v14_sign_flip_trade_preserves_oi_symmetry_and_senior_accounting` | 150s | PASS |
| `proof_v14_account_b_chunk_either_advances_or_fails_closed` | 125s | PASS |
| `proof_v14_rebalance_reduce_position_preserves_senior_claims_and_reduces_risk` | 115s | PASS |
| `proof_v14_hlock_allows_pure_risk_reducing_trade_with_principal_margin` | 109s | PASS |
| `proof_v14_resolved_close_partial_b_settlement_makes_progress_without_closing` | 96s | PASS |
| `proof_v14_risk_increasing_trade_requires_initial_health_before_mutation` | 82s | PASS |
| `proof_v14_resolved_profit_close_pays_snapshot_residual_and_clears_claim` | 81s | PASS |
| `proof_v14_bankrupt_liquidation_excludes_fee_from_residual_and_spends_insurance_once` | 70s | PASS |
| `proof_v14_partial_liquidation_can_reduce_risk_without_forcing_full_close` | 64s | PASS |
| `proof_v14_bankrupt_liquidation_consumes_insurance_before_social_loss` | 59s | PASS |
| `proof_v14_permissionless_refresh_returns_partial_b_progress_without_accrual` | 50s | PASS |
| `proof_v14_funding_accrual_refresh_matches_sign_and_floor` | 47s | PASS |
| `proof_v14_price_accrual_refresh_matches_eager_mark_pnl` | 47s | PASS |
| `proof_v14_wide_signed_mul_div_floor_matches_small_reference` | 47s | PASS |
| `proof_v14_attach_then_clear_leg_restores_account_local_counters_for_long` | 44s | PASS |
| `proof_v14_mul_div_ceil_u256_is_floor_plus_remainder_indicator` | 40s | PASS |
| `proof_v14_b_residual_booking_makes_durable_progress_or_fails_closed` | 35s | PASS |

## Spec Section 15 Traceability

The current v14 source-of-truth spec requires the following proof/TDD coverage.
Each item below maps to production-code tests, Kani proofs, or both.

| Spec §15 item | Coverage |
|---|---|
| `unbounded_global_accounts_no_full_market_scan_required` | `v14_permissionless_crank_does_not_require_full_market_scan`; `proof_v14_permissionless_crank_does_not_require_full_market_scan` |
| `full_account_refresh_is_O_N_and_required_for_favorable_actions` | `v14_favorable_action_requires_current_full_account_refresh`; `proof_v14_favorable_action_requires_current_full_refresh`; `proof_v14_full_refresh_settles_and_scores_two_active_assets`; bounded `PortfolioLegV14` array coverage |
| `hinted_subset_cannot_hide_toxic_leg` | `v14_trade_hint_cannot_hide_toxic_portfolio_leg_on_other_asset`; `proof_v14_trade_hint_cannot_hide_toxic_portfolio_leg_on_other_asset` |
| `stale_certificate_loses_margin_credit` | `v14_full_refresh_clears_stale_certificate_but_not_b_stale_loss`; `proof_v14_full_refresh_clears_stale_certificate`; stale counter proofs |
| `stale_profitable_leg_cannot_support_risk_increase` | stale certificate and full-refresh gating tests/proofs; target/effective lag and h-lock no-positive-credit trade proofs |
| `rebalance_conserves_senior_claims` | `v14_rebalance_reduce_position_requires_strict_risk_progress_and_preserves_senior_claims`; `proof_v14_rebalance_reduce_position_preserves_senior_claims_and_reduces_risk` |
| `rebalance_cannot_double_count_collateral` | `v14_cross_margin_collateral_counted_once_and_not_below_loss_envelope`; `proof_v14_cross_margin_equity_counts_collateral_once_and_score_uses_full_envelope` |
| `cross_margin_offset_cap_never_below_loss_envelope` | `v14_cross_margin_collateral_counted_once_and_not_below_loss_envelope`; public config envelope proofs |
| `unhealthy_rebalance_requires_strict_risk_progress` | `v14_rebalance_rejects_missing_or_zero_progress`; `proof_v14_liquidation_progress_rejects_non_reducing_scores`; rebalance risk-progress proof |
| `cyclic_rescue_without_progress_reverts` | `v14_cyclic_rescue_without_scalar_progress_reverts`; non-progress liquidation/rebalance proofs |
| `B_stale_blocks_withdraw_convert_close_and_risk_increase` | `v14_b_stale_blocks_refresh_and_favorable_actions_without_scanning_market`; `proof_v14_b_stale_blocks_refresh_and_favorable_actions`; `proof_v14_favorable_locks_block_released_pnl_conversion_before_mutation`; `proof_v14_b_stale_trade_preflight_rolls_back_partial_side_effects`; `proof_v14_deposit_into_stale_or_b_stale_account_does_not_unlock_favorable_actions` |
| `account_B_settlement_chunks_huge_delta_without_market_scan` | `v14_account_b_chunk_makes_strict_account_local_progress_or_requires_recovery`; `proof_v14_account_b_chunk_either_advances_or_fails_closed` |
| `B_booking_exact_remainder_conservation` | `v14_b_residual_booking_is_bounded_and_remainder_conserving`; `proof_v14_b_residual_booking_makes_durable_progress_or_fails_closed` |
| `bankrupt_close_books_residual_without_opposing_scan` | bankrupt liquidation residual-durability tests/proofs; residual booking tests/proofs; no full-market scan crank proof |
| `bankrupt_close_cannot_clear_basis_before_residual_durable` | `v14_bankrupt_liquidation_requires_residual_durable_before_freeing_exposure`; `proof_v14_bankrupt_liquidation_cannot_free_exposure_before_residual_durable` |
| `staged_insurance_not_double_spent` | `v14_bankrupt_liquidation_consumes_insurance_before_social_loss`; `v14_bankrupt_liquidation_drops_uncollectible_fee_and_spends_insurance_once`; matching bankrupt-liquidation proofs |
| `bankruptcy_residual_excludes_protocol_fees` | `v14_bankrupt_liquidation_drops_uncollectible_fee_and_spends_insurance_once`; `proof_v14_bankrupt_liquidation_excludes_fee_from_residual_and_spends_insurance_once` |
| `uncollectible_fees_forgiven_not_socialized` | fee loss-seniority tests/proofs; wide fee sync test/proof; bankrupt liquidation fee-exclusion test/proof |
| `insurance_boundary_non_deficit_paths` | `proof_v14_non_deficit_public_paths_do_not_decrease_insurance`; bankrupt liquidation insurance-spend proofs |
| `account_free_equity_active_accrual_requires_protective_progress` | `v14_account_free_equity_active_accrual_requires_protective_progress`; `proof_v14_equity_active_accrual_requires_protective_progress` |
| `effective_price_raw_target_lag_no_free_option` | target/effective lag trade, withdraw, and conversion tests; `proof_v14_target_effective_lag_rejects_risk_increasing_trade_before_mutation`; `proof_v14_target_effective_lag_blocks_pnl_conversion_before_mutation`; `proof_v14_favorable_locks_block_released_pnl_conversion_before_mutation` |
| `loss_stale_catchup_blocks_risk_increase_until_current` | `v14_loss_stale_blocks_nonflat_withdrawal_even_if_no_positive_credit_suffices`; `v14_loss_stale_allows_pure_risk_reducing_trade_path`; `proof_v14_loss_stale_blocks_nonflat_withdrawal` |
| `resolved_close_one_account_bounded` | resolved flat/profit/active-position/partial-B tests; resolved close proofs |
| `permissionless_recovery_no_caller_chosen_price` | `v14_permissionless_recovery_is_declared_by_reason_not_caller_price`; `proof_v14_permissionless_recovery_declares_reason_or_fails_closed`; recovery crank proof |
| `explicit_loss_audit_overflow_does_not_trap_funds` | `v14_explicit_loss_audit_overflow_declares_recovery`; `proof_v14_explicit_loss_audit_overflow_declares_recovery_without_mutation` |
| `owner_dead_leg_forfeit_does_not_hostage_unrelated_collateral` | `v14_dead_leg_forfeit_is_unavailable_for_normal_live_leg`; `v14_dead_leg_forfeit_detaches_without_crediting_positive_pnl`; `v14_dead_leg_forfeit_books_negative_residual_to_opposing_domain_only`; `proof_v14_dead_leg_forfeit_does_not_credit_positive_kf_delta`; `proof_v14_dead_leg_forfeit_books_loss_to_opposing_domain_only` |
| `effective_support_consumption_burns_required_face_junior_claim` / `support_consumed_cannot_exceed_g_value_of_face_claim_burned` | `v14_dead_leg_forfeit_haircuts_positive_support_when_junior_impaired`; `proof_v14_dead_leg_forfeit_haircuts_positive_support_when_junior_impaired`; `v14_full_refresh_uses_haircut_bounded_support_for_negative_kf_delta_when_impaired`; `proof_v14_negative_kf_settlement_uses_haircut_support_not_face_netting`; `v14_full_refresh_uses_haircut_bounded_new_positive_kf_to_cure_prior_loss`; `proof_v14_positive_kf_delta_cures_prior_loss_at_haircut_value` |
| `authoritatively_flat_account_never_receives_B_loss` | `v14_authoritatively_flat_account_never_receives_b_loss`; `proof_v14_authoritatively_flat_account_never_receives_b_loss` |
| `no_single_instruction_full_market_requirement` | no-slab v14 architecture; no full-market scan crank test/proof; account-local crank and refresh tests/proofs |
| `worst_case_hinted_progress_totality` | `v14_worst_case_hinted_progress_actions_are_total_and_bounded`; `proof_v14_worst_case_hinted_progress_actions_are_total_and_bounded` |
| `global_accumulator_not_account_health_proof` | `v14_global_residual_is_not_account_health_proof`; `proof_v14_global_residual_is_not_account_health_proof` |
| `active_bitmap_canonical_no_hidden_legs` | `v14_active_bitmap_is_the_only_active_leg_authority`; `proof_v14_hidden_leg_rejected_by_bitmap_authority` |
| `N_too_large_rejected_at_public_user_fund_init` | `v14_public_init_rejects_unbounded_portfolio_width`; `proof_v14_configured_portfolio_width_rejects_out_of_range_leg`; public config proof |

No missing engine-side spec §15 coverage item was identified in this pass.

Additional Anchor v2 zero-copy persistence coverage:

| Property | Coverage |
|---|---|
| Persisted account/wire structs are `bytemuck::Pod` and `Zeroable` | `v14_persisted_account_wire_structs_are_bytemuck_pod` |
| Persisted account/wire structs are byte-aligned and bytemuck-readable | `v14_persisted_account_wire_structs_are_bytemuck_pod`; `v14_persisted_account_wire_roundtrips_runtime_state` |
| Persisted bool/enum/Option encodings fail closed | `v14_persisted_account_wire_rejects_invalid_bool_enum_and_option_encoding`; `proof_v14_persisted_wire_rejects_noncanonical_bool_enum_and_option` |
| Persisted signed economic fields reject `i128::MIN` | `proof_v14_persisted_wire_rejects_i128_min_economic_fields` |
| Persisted provenance, active bitmap, and hidden-leg smuggling fails closed | `proof_v14_persisted_wire_rejects_provenance_and_hidden_leg_smuggling` |
| Runtime/persisted conversion preserves validated state | `v14_persisted_account_wire_roundtrips_runtime_state`; `proof_v14_market_wire_roundtrip_preserves_valid_runtime_state`; `proof_v14_portfolio_wire_roundtrip_preserves_valid_runtime_state` |

## V12 Property Migration

The old v12 proof inventory had 416 Kani harnesses. Many were intentionally not
ported because v14 removed the slab, fixed account capacity, full-market cursor
scan, v12 reserve queues, and wrapper-era entrypoints. The applicable properties
were migrated to v14 production-code tests/proofs.

Migrated property families covered in the v14 suite:

| v12 property family | v14 coverage |
|---|---|
| Deposit/withdraw accounting roundtrip | `proof_v14_deposit_then_withdraw_roundtrip_preserves_accounting`, `proof_v14_partial_withdraw_can_leave_small_remainder` |
| Multiple deposits aggregate into senior totals | `proof_v14_multiple_deposits_aggregate_c_tot_and_vault` |
| Account close/reclaim requires clean local state | `proof_v14_close_portfolio_account_requires_clean_local_state` |
| Malformed signed fee-credit and PnL state fails closed | malformed account-shape tests/proofs and fee-credit/PnL proofs |
| Conservative risk-notional arithmetic | `proof_v14_risk_notional_flat_zero_and_monotone_in_price` |
| Shared wide arithmetic floor/ceil/K-diff semantics | `tests/proofs_v14_arithmetic.rs` |
| Position bounds reject before OI mutation | `proof_v14_oversize_position_rejected_before_oi_mutation` |
| Price/funding accrual matches eager account settlement | price and funding refresh tests/proofs |
| Same-slot exposed price move cannot mutate state | `proof_v14_same_slot_exposed_price_move_rejects_before_mutation` |
| Funding cap rejects before state mutation | `proof_v14_funding_rate_above_cap_rejects_before_mutation` |
| Dynamic trade-fee cap, conservation, and OI symmetry | dynamic fee tests/proofs and trade conservation proofs |
| Invalid/risk-increasing trade rejects before mutation | invalid trade, health, h-lock, and target/effective lag tests/proofs |
| Sign-flip trades preserve OI symmetry and senior totals | sign-flip trade tests/proofs |
| Released PnL conversion cannot mint beyond residual | released-PnL conversion tests/proofs |
| Permissionless refresh must return partial B progress | permissionless partial-B refresh tests/proofs |
| Public user-fund config must keep recovery/profile guarantees enabled | public config tests/proofs |
| Liquidation must strictly improve account risk and preserve residual durability | liquidation progress, partial liquidation, bankrupt residual, and fee-exclusion tests/proofs |
| Resolved close payout/progress behavior | resolved flat, positive, fee-current, partial-B, and active-position tests/proofs |

## Static Strength Scan

Strength indicators:

| Check | Result |
|---|---:|
| Harnesses over v14 production transitions | 110 |
| Harnesses over shared production arithmetic helpers | 7 |
| Harnesses with `kani::cover!` reachability checks | 110 |
| Explicit `kani::assume(false)` / `assume(false)` findings | 0 |
| Confirmed vacuous harnesses | 0 |
| Confirmed weak harnesses | 0 |

Current classification:

| Classification | Status |
|---|---|
| Non-vacuity | No confirmed vacuous harnesses found. Cover checks exercise h-min/h-max, stale set/clear, stale/B-stale deposit lock preservation, hidden-leg rejection, persisted provenance/bitmap smuggling rejection, B-chunk progress paths, B-stale trade rollback, malformed fee-credit states, invalid config branches, aggregate deposit branches, arithmetic floor/ceil branches, positive/negative K-diff branches, bankrupt residual recovery, zero/partial insurance paths, non-deficit insurance-boundary public paths, favorable-action lock composition, permissionless partial-B refresh, released-PnL zero/positive conversion paths, resolved partial-B close progress, and rebalance reduction paths. |
| Weak proofs | No confirmed weak proofs in the v14 inventory. Concrete-branch harnesses are intentional regression proofs over production methods, and symbolic arithmetic/transition harnesses cover the remaining branch families. |
| Inductive strength | The stale-counter and arithmetic helper proofs are closest to local inductive transition proofs. The overall suite is a strong production-code safety/liveness harness set, not a complete arbitrary-state inductive proof of the whole engine. |
| Practical proof boundary | The suite proves key v14 account-local invariants over real production methods: h-lock selection, provenance/hidden-leg fail-closed behavior, persisted wire provenance/bitmap fail-closed behavior, stale counter idempotence and refresh clearing, stale/B-stale deposit lock preservation, malformed signed state rejection, deposit/withdraw accounting, aggregate senior accounting, close-account local-state gating, risk-notional monotonicity, position-bound fail-before-mutation, B-chunk progress/fail-closed behavior, B-stale trade preflight rollback through the public staged API, bounded repeated B-chunk completion for small residuals, multi-asset full-refresh settlement/scoring, non-deficit public-path insurance preservation, full-refresh gating, favorable-action lock fail-before-mutation behavior, monotonic liquidation-score rejection, loss-before-fee ordering, account-free equity-active accrual protective-progress gating, one-segment bounded catchup, funding-rate cap fail-before-mutation, dynamic trade-fee enforcement, trade conservation/OI symmetry, target/effective lag risk-increase rejection, h-lock risk-increase rejection, h-lock risk-reducing liveness under no-positive-credit margin, h-lock withdrawal no-positive-credit gating, released-PnL conversion bounded by residual, loss-stale nonflat withdrawal blocking, bankrupt liquidation insurance-before-social-loss ordering, bankrupt residual durability before exposure release, uncollectible liquidation-fee exclusion from residual loss, resolved close liveness and payout shape, durable B residual booking, prior-epoch reset clearing, quantity-ADL OI symmetry, rebalance strict risk-progress, price/funding settlement, invalid trade rollback, partial liquidation, and shared wide arithmetic semantics. |

## Rust Test Matrix

| Command | Result |
|---|---|
| `cargo test` | PASS on 2026-05-16 |
| `cargo test --features test` | PASS on 2026-05-16 |
| `cargo test --features fuzz` | PASS on 2026-05-16 |

## Audit Conclusion

No missing engine-side v14 spec §15 coverage item was identified. No confirmed
weak or vacuous proof was identified in the current static pass. Applicable v12
property families have been reviewed and either ported to v14 production-code
tests/proofs or retired as slab/wrapper/v12-queue-specific.

The only open audit-maintenance item is to rerun `scripts/run_kani_full_audit.sh`
against the current 120-proof inventory and replace the older 57-proof timing
artifacts.
