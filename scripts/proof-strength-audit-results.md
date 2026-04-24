# Kani Proof Strength Audit Results

Generated: 2026-04-24

Source prompt: `scripts/audit-proof-strength.md`.

Execution note: `scripts/audit proof strength` is not an executable in this checkout. The audit below applies the prompt directly to the current proof files and uses `cargo kani list --format json` for the harness inventory.

Kani version: `0.66.0`. Kani-listed standard harnesses: `305`. Parsed proof harnesses: `305`.

This is a proof-strength audit, not a full CBMC verification run. It classifies harness shape, symbolic breadth, non-vacuity risk, and inductive strength.

## Final Tally

| Classification | Count | Audit meaning |
|---|---:|---|
| **INDUCTIVE** | 0 | Fully symbolic initial state plus assumed decomposed invariant and loop-free modular preservation proof. |
| **STRONG** | 161 | Symbolic proof harness with meaningful assertions and no observed vacuity risk, but not inductive. |
| **WEAK** | 0 | Symbolic harness with a proof-strength issue that should be tightened. |
| **UNIT TEST** | 144 | Concrete or deterministic scenario harness with no `kani::any()` input. |
| **VACUOUS** | 0 | Confirmed contradictory assumptions or unreachable assertions. |

## Key Findings

- **No harness is INDUCTIVE under the prompt definition.** There is no fully symbolic `RiskEngine` state and no `kani::assume(INV(engine))` setup. Engine proofs construct state with `RiskEngine::new(...)`, helper materialization, direct field mutation, or concrete scenario setup.
- **The prompt references `canonical_inv`, `valid_state`, and decomposed `inv_*` predicates, but this checkout has none in `tests/proofs_*.rs`.** The current suite uses targeted assertions and `check_conservation()`; 61 harnesses reference `check_conservation()`.
- **Constructed topology dominates.** 262 harnesses construct a `RiskEngine`; account materialization counts are 119 single-account, 71 two-account, 3 three-account, 1 four-account, and 111 pure/helper proofs with no materialized account.
- **Symbolic breadth is useful but bounded.** 161 harnesses use `kani::any()`, 153 include `kani::assume`, and most assumptions intentionally bound ranges to small-model values or protocol envelopes.
- **Concrete regressions are numerous.** 144 harnesses have no symbolic input, so under the prompt they are UNIT TEST / regression harnesses even when they check important scenarios.

## Weak Harnesses

No WEAK harnesses remain in this audit pass. The four prior Ok-gated harnesses were strengthened with valid spec preconditions, explicit success assertions, and reachability covers for the intended branches.

No confirmed VACUOUS harnesses were found.

## Inductive Criteria 6a-6f

| Criterion | Current status |
|---|---|
| 6a State construction | Engine harnesses use constructed states (`RiskEngine::new`, helper allocation, direct field setup). None quantify over all invariant-satisfying states. |
| 6b Topology coverage | Mostly 1-2 account topologies. This exercises key scenarios but does not prove arbitrary account topology or abstract rest-of-system properties. |
| 6c Invariant decomposition | No reusable decomposed invariant predicates are present in the proof files. Properties are asserted directly or via `check_conservation()`. |
| 6d Loop-free invariant specs | No loop-free inductive invariant spec suite is present. Some properties are local arithmetic/delta checks, but there is no general modular invariant framework. |
| 6e Cone of influence | Constructed engine state fixes many fields outside the function under test. Direct engine/account field setup appears in 164/75 harnesses respectively. |
| 6f Full domain vs bounded ranges | Bounded symbolic ranges are common. This is appropriate for tractability but prevents full-domain inductive classification. |

## Per-File Tally

| File | Total | STRONG | WEAK | UNIT TEST |
|---|---:|---:|---:|---:|
| `tests/proofs_admission.rs` | 32 | 25 | 0 | 7 |
| `tests/proofs_arithmetic.rs` | 19 | 19 | 0 | 0 |
| `tests/proofs_audit.rs` | 35 | 11 | 0 | 24 |
| `tests/proofs_checklist.rs` | 16 | 11 | 0 | 5 |
| `tests/proofs_instructions.rs` | 51 | 12 | 0 | 39 |
| `tests/proofs_invariants.rs` | 26 | 20 | 0 | 6 |
| `tests/proofs_lazy_ak.rs` | 15 | 13 | 0 | 2 |
| `tests/proofs_liveness.rs` | 11 | 0 | 0 | 11 |
| `tests/proofs_safety.rs` | 76 | 32 | 0 | 44 |
| `tests/proofs_v1131.rs` | 24 | 18 | 0 | 6 |

## Complete Classification

### `tests/proofs_admission.rs`

| Line | Proof | Classification | Basis |
|---:|---|---|---|
| 20 | `ah1_single_admission_range` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 65 | `ah2_sticky_is_absorbing` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 102 | `ah3_no_under_admission` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 148 | `ah4_hmin_zero_preserves_h_equals_one` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 196 | `ah5_cross_account_sticky_isolation` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 235 | `ah6_positive_hmin_floor` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 266 | `ac1_acceleration_all_or_nothing` | **STRONG** | Symbolic valid §4.9 reserve state with explicit Ok assertion, branch reachability covers, and direct acceleration/unchanged postconditions. |
| 326 | `ac2_acceleration_fires_iff_admits` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 388 | `ac4_acceleration_conservation` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 431 | `in1_no_live_immediate_release` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 466 | `ah7_sticky_bitmap_is_idempotent_and_never_capacity_bound` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 505 | `ah8_broken_conservation_fails` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 532 | `k9_admission_pair_rejects_zero_max` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 548 | `k1_accrue_rejects_dt_over_envelope` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 586 | `k2_resolve_degenerate_bypasses_dt_cap` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 613 | `k71_neg_pnl_count_tracks_actual` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 647 | `k201_keeper_crank_rejects_oversized_budget` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 669 | `k202_postcondition_detects_broken_conservation` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 693 | `ac5_admit_outstanding_atomic_on_err` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 743 | `rs1_validate_rejects_reserved_exceeding_pos_pnl` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 772 | `rs2_admit_outstanding_rejects_bucket_sum_mismatch` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 809 | `rs3_apply_reserve_loss_rejects_malformed_queue` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 841 | `rs4_warmup_rejects_malformed_pending_before_promotion` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 867 | `k104_oi_geq_sum_of_effective` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 898 | `v19_admit_gate_stress_lane_forces_h_max` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 938 | `v19_admit_gate_none_disables_step2` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 978 | `v19_admit_gate_some_zero_rejected` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 992 | `v19_admit_gate_sticky_early_return` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 1026 | `v19_consumption_monotone_within_generation` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 1076 | `v19_consumption_floor_below_one_bp` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 1114 | `v19_rr_window_zero_no_cursor_advance` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 1147 | `v19_accrual_consumption_only_commits_on_success` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |

### `tests/proofs_arithmetic.rs`

| Line | Proof | Classification | Basis |
|---:|---|---|---|
| 17 | `t0_1_floor_div_signed_conservative_is_floor` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 43 | `t0_1_sat_negative_with_remainder` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 67 | `t0_2_mul_div_floor_algebraic_identity` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 92 | `t0_2_mul_div_ceil_algebraic_identity` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 117 | `t0_2c_mul_div_floor_matches_reference` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 139 | `t0_2d_mul_div_ceil_matches_reference` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 165 | `t0_4_fee_debt_no_overflow` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 179 | `t0_4_saturating_mul_no_panic` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 196 | `t0_4_fee_debt_i128_min` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 220 | `proof_notional_flat_is_zero` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 234 | `proof_notional_scales_with_price` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 267 | `proof_warmup_release_bounded_by_reserved` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 292 | `t13_59_fused_delta_k_no_double_rounding` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 321 | `proof_ceil_div_positive_checked` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 344 | `proof_haircut_mul_div_conservative` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 379 | `proof_wide_signed_mul_div_floor_sign_and_rounding` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 429 | `proof_k_pair_variant_sign_and_rounding` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 466 | `proof_k_pair_variant_zero_diff` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 483 | `proof_wide_signed_mul_div_floor_zero_inputs` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |

### `tests/proofs_audit.rs`

| Line | Proof | Classification | Basis |
|---:|---|---|---|
| 23 | `proof_epoch_snap_zero_on_position_zeroout` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 64 | `proof_epoch_snap_correct_on_nonzero_attach` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 101 | `proof_add_user_count_rollback_on_alloc_failure` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 123 | `proof_add_lp_count_rollback_on_alloc_failure` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 149 | `proof_flat_account_maintenance_healthy` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 175 | `proof_flat_account_initial_margin_healthy` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 199 | `proof_flat_zero_equity_not_maintenance_healthy` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 233 | `proof_fee_debt_sweep_checked_arithmetic` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 282 | `proof_keeper_crank_invalid_partial_no_action` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 317 | `proof_liquidate_missing_account_no_market_mutation` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 342 | `proof_config_rejects_oversized_max_accounts` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 353 | `proof_config_rejects_zero_max_accounts` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 364 | `proof_config_rejects_invalid_bps` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 390 | `proof_close_account_pnl_check_before_fee_forgive` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 428 | `proof_settle_epoch_snap_zero_on_truncation` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 477 | `proof_keeper_hint_none_returns_none` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 500 | `proof_keeper_hint_fullclose_passthrough` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 530 | `proof_gc_cursor_advances_by_scanned` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 555 | `proof_gc_cursor_with_drained_accounts` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 590 | `proof_config_rejects_liq_fee_inversion` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 603 | `proof_config_rejects_fee_cap_exceeds_max` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 618 | `proof_touch_unused_returns_error` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 633 | `proof_touch_oob_returns_error` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 652 | `proof_withdraw_no_crank_gate` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 669 | `proof_trade_no_crank_gate` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 693 | `proof_gc_skips_negative_pnl` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 732 | `proof_validate_hint_preflight_conservative` | **STRONG** | Symbolic harness with explicit reachability cover or branch/property assertions. |
| 788 | `proof_validate_hint_preflight_oracle_shift` | **STRONG** | Symbolic harness with explicit reachability cover or branch/property assertions. |
| 848 | `proof_set_owner_rejects_claimed` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 874 | `proof_force_close_resolved_with_position_conserves` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 896 | `proof_force_close_resolved_with_profit_conserves` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 926 | `proof_force_close_resolved_flat_returns_capital` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 948 | `proof_force_close_resolved_position_conservation` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 978 | `proof_force_close_resolved_pos_count_decrements` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 1004 | `proof_force_close_resolved_fee_sweep_conservation` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |

### `tests/proofs_checklist.rs`

| Line | Proof | Classification | Basis |
|---:|---|---|---|
| 17 | `proof_a2_reserve_bounds_after_set_pnl` | **STRONG** | Symbolic harness with explicit reachability cover or branch/property assertions. |
| 53 | `proof_a7_fee_credits_bounds_after_trade` | **STRONG** | Symbolic harness with explicit reachability cover or branch/property assertions. |
| 90 | `proof_f8_loss_seniority_in_touch` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 125 | `proof_b7_oi_balance_after_trade` | **STRONG** | Symbolic harness with explicit reachability cover or branch/property assertions. |
| 152 | `proof_b1_conservation_after_trade_with_fees` | **STRONG** | Symbolic harness with explicit reachability cover or branch/property assertions. |
| 180 | `proof_e8_position_bound_enforcement` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 202 | `proof_b5_matured_leq_pos_tot` | **STRONG** | Symbolic harness with explicit reachability cover or branch/property assertions. |
| 234 | `proof_g4_drain_only_blocks_oi_increase` | **STRONG** | Symbolic harness with explicit reachability cover or branch/property assertions. |
| 271 | `proof_goal5_no_same_trade_bootstrap` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 322 | `proof_goal7_pending_merge_max_horizon` | **STRONG** | Symbolic harness with explicit reachability cover or branch/property assertions. |
| 362 | `proof_goal23_deposit_no_insurance_draw` | **STRONG** | Symbolic harness with explicit reachability cover or branch/property assertions. |
| 393 | `proof_goal27_finalize_path_independent` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 441 | `proof_two_bucket_reserve_sum_after_append` | **STRONG** | Symbolic harness with explicit reachability cover or branch/property assertions. |
| 477 | `proof_two_bucket_loss_newest_first` | **STRONG** | Symbolic harness with explicit reachability cover or branch/property assertions. |
| 507 | `proof_two_bucket_scheduled_timing` | **STRONG** | Symbolic harness with explicit reachability cover or branch/property assertions. |
| 541 | `proof_two_bucket_pending_non_maturity` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |

### `tests/proofs_instructions.rs`

| Line | Proof | Classification | Basis |
|---:|---|---|---|
| 18 | `t3_16_reset_pending_counter_invariant` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 57 | `t3_16b_reset_counter_with_nonzero_k_diff` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 97 | `t3_17_clean_empty_engine_no_retrigger` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 118 | `t3_18_dust_bound_reset_in_begin_full_drain` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 133 | `t3_19_finalize_side_reset_requires_all_stale_touched` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 156 | `t6_26b_full_drain_reset_nonzero_k_diff` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 198 | `t9_35_warmup_release_monotone_in_time` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 231 | `t9_36_fee_seniority_after_restart` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 265 | `t10_37_accrue_mark_matches_eager` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 302 | `t10_38_accrue_funding_payer_driven` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 359 | `t11_39_same_epoch_settle_idempotent_real_engine` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 392 | `t11_40_non_compounding_quantity_basis_two_touches` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 423 | `t11_41_attach_effective_position_remainder_accounting` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 458 | `t11_42_dynamic_dust_bound_inductive` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 491 | `t11_50_execute_trade_atomic_oi_update_sign_flip` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 518 | `t11_51_execute_trade_slippage_zero_sum` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 543 | `t11_52_touch_account_full_restart_fee_seniority` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 594 | `t11_54_worked_example_regression` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 630 | `t5_24_dynamic_dust_bound_sufficient` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 668 | `proof_begin_full_drain_reset` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 688 | `proof_finalize_side_reset_requires_conditions` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 718 | `t13_55_empty_opposing_side_deficit_fallback` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 746 | `t13_56_unilateral_empty_orphan_resolution` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 769 | `t13_57_unilateral_empty_corruption_guard` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 787 | `t13_58_unilateral_empty_short_side` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 810 | `t13_60_unconditional_dust_bound_on_any_a_decay` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 837 | `t12_53_adl_truncation_dust_must_not_deadlock` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 910 | `t14_61_dust_bound_adl_a_truncation_sufficient` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 952 | `t14_62_dust_bound_same_epoch_zeroing` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 986 | `t14_63_dust_bound_position_reattach_remainder` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 1018 | `t14_64_dust_bound_full_drain_reset_zeroes` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 1035 | `t14_65_dust_bound_end_to_end_clearance` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 1128 | `proof_fee_shortfall_routes_to_fee_credits` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 1178 | `proof_organic_close_bankruptcy_guard` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 1207 | `proof_solvent_flat_close_succeeds` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 1241 | `proof_property_23_deposit_materialization_threshold` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 1276 | `proof_property_51_withdraw_any_partial_ok` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 1305 | `proof_property_31_missing_account_safety` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 1355 | `proof_property_44_deposit_true_flat_guard` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 1406 | `proof_property_49_profit_conversion_reserve_preservation` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 1464 | `proof_property_50_flat_only_auto_conversion` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 1515 | `proof_property_52_convert_released_pnl_instruction` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 1583 | `proof_audit2_deposit_materializes_missing_account` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 1617 | `proof_audit2_deposit_rejects_zero_amount_for_missing` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 1632 | `proof_audit2_deposit_existing_accepts_small_topup` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 1657 | `proof_audit4_add_user_atomic_on_failure` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 1691 | `proof_audit4_add_user_atomic_on_tvl_failure` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 1721 | `proof_audit4_deposit_fee_credits_max_tvl` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 1746 | `v19_reclaim_envelope_rejection_is_pre_mutation` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 1790 | `v19_reclaim_envelope_accept_within_bound` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 1822 | `v19_accrue_market_envelope_enforces_goal52_bound` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |

### `tests/proofs_invariants.rs`

| Line | Proof | Classification | Basis |
|---:|---|---|---|
| 17 | `t0_3_set_pnl_aggregate_exact` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 37 | `t0_3_sat_all_sign_transitions` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 70 | `t0_4_conservation_check_handles_overflow` | **STRONG** | Symbolic harness with explicit reachability cover or branch/property assertions. |
| 114 | `inductive_top_up_insurance_preserves_accounting` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 132 | `inductive_set_capital_decrease_preserves_accounting` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 150 | `inductive_set_pnl_preserves_pnl_pos_tot_delta` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 171 | `inductive_deposit_preserves_accounting` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 184 | `inductive_withdraw_preserves_accounting` | **STRONG** | Symbolic harness with explicit reachability cover or branch/property assertions. |
| 204 | `inductive_settle_loss_preserves_accounting` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 236 | `prop_pnl_pos_tot_agrees_with_recompute` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 260 | `prop_conservation_holds_after_all_ops` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 299 | `proof_set_pnl_rejects_i128_min` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 309 | `proof_set_pnl_maintains_pnl_pos_tot` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 331 | `proof_set_pnl_underflow_safety` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 357 | `proof_set_pnl_clamps_reserved_pnl` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 383 | `proof_set_capital_maintains_c_tot` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 407 | `proof_check_conservation_basic` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 426 | `proof_haircut_ratio_no_division_by_zero` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 450 | `proof_absorb_protocol_loss_drains_to_zero` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 475 | `proof_set_position_basis_q_count_tracking` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 514 | `proof_side_mode_gating` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 545 | `proof_account_equity_net_nonnegative` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 582 | `proof_effective_pos_q_epoch_mismatch_returns_zero` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 605 | `proof_effective_pos_q_flat_is_zero` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 626 | `proof_attach_effective_position_updates_side_counts` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 659 | `proof_fee_credits_never_i128_min` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |

### `tests/proofs_lazy_ak.rs`

| Line | Proof | Classification | Basis |
|---:|---|---|---|
| 17 | `t1_7_adl_quantity_only_lazy_conservative` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 42 | `t1_8_adl_deficit_only_lazy_equals_eager` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 72 | `t1_9_adl_quantity_plus_deficit_lazy_conservative` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 111 | `t1_8b_adl_deficit_lazy_conservative_symbolic_a_basis` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 143 | `t2_12_floor_shift_lemma` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 164 | `t2_12_fold_step_case` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 197 | `t2_14_compose_mark_adl_mark` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 264 | `t3_14_epoch_mismatch_forces_terminal_close` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 310 | `t3_14b_epoch_mismatch_with_nonzero_k_diff` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 362 | `t7_28a_noncompounding_floor_inequality_correct_direction` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 398 | `t7_28b_noncompounding_exact_additivity_divisible_increments` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 442 | `t6_24_worked_example_regression` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 487 | `t6_25_pure_pnl_bankruptcy_regression` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 514 | `t6_26_full_drain_reset_regression` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 575 | `proof_property_43_k_pair_chronology_correctness` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |

### `tests/proofs_liveness.rs`

| Line | Proof | Classification | Basis |
|---:|---|---|---|
| 17 | `t11_43_end_instruction_auto_finalizes_ready_side` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 45 | `t11_44_trade_path_reopens_ready_reset_side` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 86 | `t11_46_enqueue_adl_k_add_overflow_still_routes_quantity` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 125 | `t11_47_precision_exhaustion_terminal_drain` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 153 | `t11_48_bankruptcy_liquidation_routes_q_when_D_zero` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 183 | `t11_49_pure_pnl_bankruptcy_path` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 213 | `t11_53_keeper_crank_quiesces_after_pending_reset` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 272 | `proof_drain_only_to_reset_progress` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 300 | `proof_keeper_reset_lifecycle_last_stale_triggers_finalize` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 351 | `proof_unilateral_empty_orphan_dust_clearance` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 392 | `proof_adl_pipeline_trade_liquidate_reopen` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |

### `tests/proofs_safety.rs`

| Line | Proof | Classification | Basis |
|---:|---|---|---|
| 17 | `bounded_deposit_conservation` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 35 | `bounded_withdraw_conservation` | **STRONG** | Symbolic harness with explicit reachability cover or branch/property assertions. |
| 58 | `bounded_trade_conservation` | **STRONG** | Symbolic harness with explicit reachability cover or branch/property assertions. |
| 90 | `bounded_haircut_ratio_bounded` | **STRONG** | Symbolic harness with explicit reachability cover or branch/property assertions. |
| 126 | `bounded_equity_nonneg_flat` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 163 | `bounded_liquidation_conservation` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 195 | `bounded_margin_withdrawal` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 225 | `proof_top_up_insurance_preserves_conservation` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 244 | `proof_deposit_then_withdraw_roundtrip` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 264 | `proof_multiple_deposits_aggregate_correctly` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 288 | `proof_close_account_returns_capital` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 306 | `proof_trade_pnl_is_zero_sum_algebraic` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 328 | `proof_flat_negative_resolves_through_insurance` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 358 | `t4_17_enqueue_adl_preserves_oi_balance_qty_only` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 388 | `t4_18_precision_exhaustion_both_sides_reset` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 414 | `t4_19_full_drain_terminal_k_includes_deficit` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 437 | `t4_20_bankruptcy_qty_routes_when_d_zero` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 457 | `t4_21_precision_exhaustion_zeroes_both_sides` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 483 | `t4_22_k_overflow_routes_to_absorb` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 519 | `t4_23_d_zero_routes_quantity_only` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 554 | `t5_21_local_floor_quantity_error_bounded` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 572 | `t5_21_pnl_rounding_conservative` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 594 | `t5_22_phantom_dust_total_bound` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 618 | `t5_23_dust_clearance_guard_safe` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 638 | `t13_54_funding_no_mint_asymmetric_a` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 685 | `proof_junior_profit_backing` | **STRONG** | Symbolic harness with explicit reachability cover or branch/property assertions. |
| 727 | `proof_protected_principal` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 775 | `proof_withdraw_simulation_preserves_residual` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 818 | `proof_funding_rate_validated_before_storage` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 846 | `proof_gc_dust_preserves_fee_credits` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 897 | `proof_min_liq_abs_does_not_block_liquidation` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 931 | `proof_trading_loss_seniority` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 972 | `proof_risk_reducing_exemption_path` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 1029 | `proof_buffer_masking_blocked` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 1071 | `proof_phantom_dust_drain_no_revert` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 1113 | `proof_fee_debt_sweep_consumes_released_pnl` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 1170 | `proof_v1126_flat_close_uses_eq_maint_raw` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 1212 | `proof_v1126_risk_reducing_fee_neutral` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 1250 | `proof_v1126_min_nonzero_margin_floor` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 1283 | `proof_gc_reclaims_drained_accounts` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 1329 | `proof_property_3_oracle_manipulation_haircut_safety` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 1395 | `proof_property_26_maintenance_vs_im_dual_equity` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 1465 | `proof_property_56_exact_raw_im_approval` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 1496 | `proof_audit_fee_sweep_pnl_conservation` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 1553 | `proof_audit_im_uses_exact_raw_equity` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 1588 | `proof_audit_empty_lp_gc_reclaimable` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 1617 | `proof_audit_k_pair_chronology_not_inverted` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 1662 | `proof_audit2_close_account_structural_safety` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 1696 | `proof_audit2_funding_rate_clamped` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 1733 | `proof_audit2_positive_overflow_equity_conservative` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 1769 | `proof_audit2_positive_overflow_no_false_liquidation` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 1803 | `proof_audit3_checked_u128_mul_i128_no_panic_at_boundary` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 1828 | `proof_audit3_compute_trade_pnl_no_panic_at_boundary` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 1875 | `proof_audit4_init_in_place_canonical` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 1987 | `proof_audit4_materialize_at_freelist_integrity` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 2032 | `proof_audit4_top_up_insurance_no_panic` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 2054 | `proof_audit4_top_up_insurance_overflow` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 2069 | `proof_audit4_deposit_fee_credits_time_monotonicity` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 2109 | `proof_audit4_deposit_fee_credits_checked_arithmetic` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 2137 | `proof_audit5_deposit_fee_credits_no_positive` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 2162 | `proof_audit5_deposit_fee_credits_zero_debt_noop` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 2187 | `proof_audit5_reclaim_empty_account_basic` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 2211 | `proof_audit5_reclaim_requires_zero_capital` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 2239 | `proof_audit5_reclaim_rejects_open_position` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 2256 | `proof_audit5_reclaim_rejects_live_capital` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 2280 | `bounded_trade_conservation_with_fees` | **STRONG** | Symbolic harness with explicit reachability cover or branch/property assertions. |
| 2311 | `proof_partial_liquidation_can_succeed` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 2354 | `proof_sign_flip_trade_conserves` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 2394 | `proof_close_account_fee_forgiveness_bounded` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 2443 | `bounded_trade_conservation_symbolic_size` | **STRONG** | Symbolic harness with explicit reachability cover or branch/property assertions. |
| 2476 | `proof_convert_released_pnl_conservation` | **STRONG** | Symbolic harness with explicit reachability cover or branch/property assertions. |
| 2532 | `proof_symbolic_margin_enforcement_on_reduce` | **STRONG** | Symbolic harness with explicit reachability cover or branch/property assertions. |
| 2578 | `proof_execute_trade_full_margin_enforcement` | **STRONG** | Symbolic harness with explicit reachability cover or branch/property assertions. |
| 2695 | `proof_convert_released_pnl_exercises_conversion` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 2745 | `v19_cascade_safety_gate_disabled_preserves_invariants` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 2801 | `v19_trade_touch_order_is_ascending` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |

### `tests/proofs_v1131.rs`

| Line | Proof | Classification | Basis |
|---:|---|---|---|
| 20 | `proof_funding_rate_accepted_in_accrue` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 40 | `proof_funding_rate_bound_rejected` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 59 | `proof_funding_sign_and_floor` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 102 | `proof_funding_floor_not_truncation` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 137 | `proof_funding_skip_zero_oi_short` | **STRONG** | Symbolic valid zero-OI public state with bounded funding rate, explicit Ok assertion, idle fast-forward cover, and no K/F delta assertions. |
| 180 | `proof_funding_skip_zero_oi_long` | **STRONG** | Symbolic valid zero-OI public state with bounded funding rate, explicit Ok assertion, idle fast-forward cover, and no K/F delta assertions. |
| 222 | `proof_funding_skip_zero_oi_both` | **STRONG** | Symbolic valid zero-OI public state with bounded funding rate, explicit Ok assertion, idle fast-forward cover, and no K/F delta assertions. |
| 266 | `proof_funding_substep_large_dt` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 299 | `proof_funding_price_basis_timing` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 339 | `proof_accrue_no_funding_when_rate_zero` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 365 | `proof_accrue_mark_still_works` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 405 | `proof_deposit_no_insurance_draw` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 442 | `proof_deposit_sweep_pnl_guard` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 477 | `proof_deposit_sweep_when_pnl_nonneg` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 511 | `proof_top_up_insurance_now_slot` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 541 | `proof_top_up_insurance_rejects_stale_slot` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 560 | `proof_positive_conversion_denominator` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 593 | `proof_bilateral_oi_decomposition` | **STRONG** | Symbolic harness with explicit reachability cover or branch/property assertions. |
| 656 | `proof_partial_liquidation_remainder_nonzero` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 705 | `proof_liquidation_policy_validity` | **UNIT TEST** | No symbolic input (`kani::any()`); concrete scenario/negative regression under the prompt criteria. |
| 743 | `proof_deposit_fee_credits_cap` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 793 | `proof_partial_liq_health_check_mandatory` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 832 | `proof_keeper_crank_r_last_stores_supplied_rate` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |
| 860 | `proof_deposit_nonflat_no_sweep_no_resolve` | **STRONG** | Symbolic harness with bounded assumptions and direct property assertions. |

## Upgrade Recommendations

1. Introduce explicit invariant predicates if the goal is a canonical invariant proof suite: structural, aggregate, accounting, mode, and per-account components.
2. Add at least one true inductive harness pattern: construct a fully symbolic minimal state, assume only the relevant invariant component, call one mutator, and assert the same component post-state.
3. Prefer loop-free delta properties for aggregate mutators (`c_tot`, `pnl_pos_tot`, OI counts) so proofs can use full-domain symbolic values instead of small bounded ranges.
4. For multi-account operations, add modular proofs over one target account plus abstract aggregate/rest-of-system summaries rather than only fixed 1-2 account concrete topologies.
5. Convert concrete regression harnesses into symbolic spec-invariant proofs where they are intended to prove general invariants rather than fixed examples.
