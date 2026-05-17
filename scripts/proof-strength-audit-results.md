# Kani Proof Strength Audit Results

Generated: 2026-05-17

Source prompt: `scripts/audit-proof-strength.md`.

## Current Inventory

Static inventory from the current `v15` tree:

| Item | Count |
|---|---:|
| Rust spec/fuzz tests | 174 |
| Kani proofs | 169 |
| Kani cover checks | 263 |
| Kani assumptions | 131 |

Breakdown:

| File | Tests | Kani proofs | Cover checks |
|---|---:|---:|---:|
| `tests/v15_spec_tests.rs` | 173 | 0 | 0 |
| `tests/v15_fuzzing.rs` | 1 | 0 | 0 |
| `tests/proofs_v15.rs` | 0 | 162 | 255 |
| `tests/proofs_v15_arithmetic.rs` | 0 | 7 | 8 |

The v15 suite is over production engine code and shared production arithmetic
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
| Slowest harness | `proof_v15_bankrupt_liquidation_cannot_free_exposure_before_residual_durable` |
| Slowest harness time | 397s |

The current tree has 169 Kani proofs, so the timing artifacts must be regenerated
before using them as a current full-proof pass record.

Focused incremental proofs added after the last completed full sweep:

| Harness | Time | Status |
|---|---:|---|
| `proof_v15_market_wire_roundtrip_preserves_valid_runtime_state` | 183s | PASS |
| `proof_v15_portfolio_wire_roundtrip_preserves_valid_runtime_state` | 452s | PASS |
| `proof_v15_persisted_wire_rejects_i128_min_economic_fields` | 106s | PASS |
| `proof_v15_repeated_account_b_chunks_complete_bounded_small_residual` | 37s | PASS |
| `proof_v15_full_refresh_settles_and_scores_two_active_assets` | 33s | PASS |
| `proof_v15_non_deficit_public_paths_do_not_decrease_insurance` | 31s | PASS |
| `proof_v15_favorable_locks_block_released_pnl_conversion_before_mutation` | 42s | PASS |
| `proof_v15_persisted_wire_rejects_provenance_and_hidden_leg_smuggling` | 213s | PASS |
| `proof_v15_b_stale_trade_preflight_rolls_back_partial_side_effects` | 56s | PASS |
| `proof_v15_deposit_into_stale_or_b_stale_account_does_not_unlock_favorable_actions` | 7s | PASS |
| `proof_v15_quantity_adl_preserves_oi_symmetry_after_close` | 146s | PASS |
| `proof_v15_quantity_adl_monotonically_shrinks_opposing_a_or_resets` | 254s | PASS |
| `proof_v15_expired_close_progress_routes_recovery_before_durable_mutation` | 15s | PASS |
| `proof_v15_dead_leg_forfeit_does_not_credit_positive_kf_delta` | 22s | PASS |
| `proof_v15_dead_leg_forfeit_books_loss_to_opposing_domain_only` | 225s | PASS |
| `proof_v15_dead_leg_forfeit_haircuts_positive_support_when_junior_impaired` | 51s | PASS |
| `proof_v15_negative_kf_settlement_uses_haircut_support_not_face_netting` | 513s | PASS |
| `proof_v15_positive_kf_delta_cures_prior_loss_at_haircut_value` | 29s | PASS |
| `proof_v15_partial_liquidation_cannot_socialize_residual_while_open_risk_remains` | 29s | PASS |
| `proof_v15_pending_domain_loss_barrier_does_not_freeze_asset_accrual` | 15s | PASS |
| `proof_v15_pending_domain_barrier_blocks_side_reset_before_mutation` | 3s | PASS |
| `proof_v15_pending_domain_barrier_does_not_block_unrelated_side_reset` | 5s | PASS |
| `proof_v15_new_close_cannot_overwrite_active_finalized_close_ledger` | 60s | PASS |
| `proof_v15_account_shape_rejects_malformed_quantity_adl_close_progress` | 32s | PASS |
| `proof_v15_account_shape_rejects_close_progress_domain_mismatch_for_open_leg` | 77s | PASS |
| `proof_v15_stale_open_close_snapshot_routes_recovery_before_durable_mutation` | 19s | PASS |
| `proof_v15_permissionless_flat_refresh_is_not_protective_for_equity_active_accrual` | 13s | PASS |
| `proof_v15_permissionless_cross_asset_liquidation_is_not_protective_for_equity_active_accrual` | 78s | PASS |
| `proof_v15_permissionless_recovery_declares_reason_or_fails_closed` | 6s | PASS |
| `proof_v15_permissionless_crank_recovery_declaration_is_accounting_neutral` | 67s | PASS |
| `proof_v15_permissionless_recovery_enables_dead_leg_forfeit_without_value_escape` | 27s | PASS |
| `proof_v15_recovery_mode_blocks_value_escape_paths_before_mutation` | 13s | PASS |
| `proof_v15_recovery_mode_rejects_non_recovery_crank_before_account_mutation` | 14s | PASS |
| `proof_v15_terminal_recovery_reason_and_mode_are_immutable` | 4s | PASS |
| `proof_v15_recovery_mode_rejects_liquidation_and_rebalance_before_mutation` | 13s | PASS |
| `proof_v15_explicit_loss_audit_overflow_declares_recovery_without_value_mutation` | 8s | PASS |
| `proof_v15_public_config_rejects_invalid_user_fund_shapes` | 4s | PASS |
| `proof_v15_persisted_wire_rejects_noncanonical_bool_enum_and_option` | 256s | PASS |
| `proof_v15_close_lifetime_uses_configured_bound_and_is_not_refreshed` | 18s | PASS |
| `proof_v15_public_invariants_reject_hard_global_bounds` | 7.0330296s | PASS |
| `proof_v15_bankrupt_liquidation_consumes_insurance_before_social_loss` | 111s | PASS |
| `proof_v15_domain_insurance_budget_caps_bankruptcy_spend` | 594s | PASS |
| `proof_v15_long_liquidation_residual_charges_short_domain` | 126.45743s | PASS |
| `proof_v15_short_liquidation_residual_charges_long_domain` | 126.074s | PASS |
| `proof_v15_bad_asset_cannot_spend_unrelated_domain_insurance_budget` | 126.98753s | PASS |
| `proof_v15_bankrupt_liquidation_cannot_free_exposure_before_residual_durable` | 499s | PASS |
| `proof_v15_bankrupt_liquidation_excludes_fee_from_residual_and_spends_insurance_once` | 536s | PASS |
| `proof_v15_rebalance_reduce_position_preserves_senior_claims_and_reduces_risk` | 159s | PASS |
| `proof_v15_pnl_pos_bound_tot_prevents_lazy_positive_pnl_first_mover_overpay` | 18s | PASS |
| `proof_v15_scaled_junior_bound_remainder_ceil_controls_resolved_payout` | 47s | PASS |
| `proof_v15_public_invariants_reject_scaled_junior_bound_cache_mismatch` | 5s | PASS |
| `proof_v15_ordinary_positive_conversion_disabled_outside_live_payout_lane` | 13s | PASS |
| `proof_v15_resolved_receipt_tracks_paid_effective_and_bound_refinement_topup` | 125s | PASS |
| `proof_v15_health_certificate_bound_to_market_epochs_and_prices` | 64.82726s | PASS |
| `proof_v15_global_cross_margin_positive_leg_supports_other_leg_maintenance_without_b_domain` | 56.28634s | PASS |
| `proof_v15_per_asset_slot_last_prevents_cross_asset_accrual_aliasing` | 27s | PASS |
| `proof_v15_resolved_payout_readiness_uses_exact_counters_and_bounds` | 16.9s | PASS |
| `proof_v15_reset_pending_epoch_start_snapshots_prevent_prior_epoch_resurrection` | 32s | PASS |
| `proof_v15_same_asset_duplicate_leg_cannot_double_count_support` | 3.5s | PASS |
| `proof_v15_stale_profitable_leg_cannot_withdraw_using_pre_refresh_positive_pnl` | 22.521006s | PASS |
| `proof_v15_asset_lifecycle_blocks_attach_before_accounting_mutation` | 6.8100333s | PASS |
| `proof_v15_asset_lifecycle_blocks_accrual_for_non_accruable_states` | 3.117619s | PASS |
| `proof_v15_asset_activation_requires_empty_slot_and_bumps_epochs` | 6.9754386s | PASS |
| `proof_v15_asset_activation_cooldown_fails_before_lifecycle_mutation` | 4.3727317s | PASS |
| `proof_v15_pending_domain_barrier_allows_rebalance_reduction_with_weight_obligation_preserved` | 83.452866s | PASS |
| `proof_v15_pending_domain_barrier_allows_trade_reduction_with_weight_obligation_preserved` | 171.34674s | PASS |
| `proof_v15_pending_domain_barrier_allows_full_trade_exit_as_flat_weight_obligation` | 370.18396s | PASS |
| `proof_v15_pending_domain_barrier_allows_rebalance_full_exit_as_flat_weight_obligation` | 232.63057s | PASS |
| `proof_v15_pending_obligation_blocks_side_reset_until_clear` | 259.52087s | PASS |
| `proof_v15_flat_pending_obligation_cannot_clear_before_b_settlement` | 304.95313s | PASS |

## Slowest Harnesses From Last Completed Sweep

All per-harness timings are recorded in `kani_audit_final.tsv`.

| Harness | Time | Status |
|---|---:|---|
| `proof_v15_bankrupt_liquidation_cannot_free_exposure_before_residual_durable` | 397s | PASS |
| `proof_v15_k_pair_mul_div_floor_matches_small_reference` | 193s | PASS |
| `proof_v15_trade_fee_conservation_and_oi_symmetry` | 160s | PASS |
| `proof_v15_sign_flip_trade_preserves_oi_symmetry_and_senior_accounting` | 150s | PASS |
| `proof_v15_account_b_chunk_either_advances_or_fails_closed` | 125s | PASS |
| `proof_v15_rebalance_reduce_position_preserves_senior_claims_and_reduces_risk` | 115s | PASS |
| `proof_v15_hlock_allows_pure_risk_reducing_trade_with_principal_margin` | 109s | PASS |
| `proof_v15_resolved_close_partial_b_settlement_makes_progress_without_closing` | 96s | PASS |
| `proof_v15_risk_increasing_trade_requires_initial_health_before_mutation` | 82s | PASS |
| `proof_v15_resolved_profit_close_pays_snapshot_residual_and_clears_claim` | 81s | PASS |
| `proof_v15_bankrupt_liquidation_excludes_fee_from_residual_and_spends_insurance_once` | 70s | PASS |
| `proof_v15_partial_liquidation_can_reduce_risk_without_forcing_full_close` | 64s | PASS |
| `proof_v15_bankrupt_liquidation_consumes_insurance_before_social_loss` | 59s | PASS |
| `proof_v15_permissionless_refresh_returns_partial_b_progress_without_accrual` | 50s | PASS |
| `proof_v15_funding_accrual_refresh_matches_sign_and_floor` | 47s | PASS |
| `proof_v15_price_accrual_refresh_matches_eager_mark_pnl` | 47s | PASS |
| `proof_v15_wide_signed_mul_div_floor_matches_small_reference` | 47s | PASS |
| `proof_v15_attach_then_clear_leg_restores_account_local_counters_for_long` | 44s | PASS |
| `proof_v15_mul_div_ceil_u256_is_floor_plus_remainder_indicator` | 40s | PASS |
| `proof_v15_b_residual_booking_makes_durable_progress_or_fails_closed` | 35s | PASS |
| `proof_v15_public_invariants_reject_hard_global_bounds` | 6.886905s | PASS |
| `proof_v15_pending_domain_barrier_allows_full_trade_exit_as_flat_weight_obligation` | 370.18396s | PASS |
| `proof_v15_flat_pending_obligation_cannot_clear_before_b_settlement` | 304.95313s | PASS |
| `proof_v15_pending_obligation_blocks_side_reset_until_clear` | 259.52087s | PASS |
| `proof_v15_pending_domain_barrier_allows_rebalance_full_exit_as_flat_weight_obligation` | 232.63057s | PASS |
| `proof_v15_pending_domain_barrier_allows_trade_reduction_with_weight_obligation_preserved` | 171.34674s | PASS |
| `proof_v15_hlock_allows_risk_increasing_trade_with_principal_margin` | 113.4s | PASS |
| `proof_v15_hlock_risk_increasing_trade_rejects_positive_credit_dependency_without_mutation` | 101.2s | PASS |
| `proof_v15_loss_stale_blocks_risk_increasing_trade_before_mutation` | 18.6s | PASS |
| `proof_v15_pending_domain_barrier_does_not_freeze_unrelated_positive_credit` | 39.1s | PASS |

## Spec Section 16 Traceability

The current v15.10 source-of-truth spec requires the following proof/TDD coverage.
Each item below maps to production-code tests, Kani proofs, or both.

| Spec §16 item | Coverage |
|---|---|
| `mutable_asset_activation_requires_full_envelope_proofs` | `v15_asset_retire_and_activation_require_empty_asset_state_and_invalidate_certs`; `proof_v15_asset_activation_requires_empty_slot_and_bumps_epochs`; activation calls production config/envelope validation, requires empty asset lifecycle state, and bumps risk/asset-set epochs |
| `asset_activation_invalidates_or_scopes_certs_fail_closed_without_full_scan` | `v15_asset_retire_and_activation_require_empty_asset_state_and_invalidate_certs`; risk epoch bump makes pre-activation certificates stale without market scan |
| `asset_cannot_activate_with_nonzero_or_unreconciled_state` | `v15_asset_retire_and_activation_require_empty_asset_state_and_invalidate_certs`; `proof_v15_asset_activation_requires_empty_slot_and_bumps_epochs` |
| `activation_rate_limit_prevents_staleness_lock_spam` | `v15_asset_activation_cooldown_rate_limits_asset_set_churn`; `proof_v15_asset_activation_cooldown_fails_before_lifecycle_mutation` |
| `drain_retire_recovery_exit_requires_no_oi_no_pending_barriers_no_unsettled_epochs` | `v15_asset_lifecycle_blocks_new_risk_unless_active`; `v15_asset_lifecycle_drain_only_allows_reduction_but_not_increase`; asset lifecycle public invariants reject inactive slots with open accounting; `proof_v15_asset_lifecycle_blocks_attach_before_accounting_mutation`; `proof_v15_asset_lifecycle_blocks_accrual_for_non_accruable_states` |
| `global_cross_margin_all_legs_support_maintenance` | `v15_global_cross_margin_positive_leg_supports_other_leg_maintenance_without_b_domain`; `proof_v15_global_cross_margin_positive_leg_supports_other_leg_maintenance_without_b_domain`; existing full-refresh and cross-margin envelope tests/proofs |
| `global_cross_margin_does_not_create_global_B_domain` | `v15_global_cross_margin_positive_leg_supports_other_leg_maintenance_without_b_domain`; `proof_v15_global_cross_margin_positive_leg_supports_other_leg_maintenance_without_b_domain`; B-domain counters and pending barriers remain unchanged while cross-leg support satisfies maintenance |
| `unbounded_global_accounts_no_full_market_scan_required` | `v15_permissionless_crank_does_not_require_full_market_scan`; `proof_v15_permissionless_crank_does_not_require_full_market_scan` |
| `full_account_refresh_is_O_N_and_required_for_favorable_actions` | `v15_favorable_action_requires_current_full_account_refresh`; `proof_v15_favorable_action_requires_current_full_refresh`; `proof_v15_full_refresh_settles_and_scores_two_active_assets`; bounded `PortfolioLegV15` array coverage |
| `certificate_bound_to_market_config_asset_slots_and_prices` | `v15_health_certificate_is_bound_to_market_epochs_and_prices`; `proof_v15_health_certificate_bound_to_market_epochs_and_prices`; health certs bind to oracle, funding, risk, asset-set epoch, active bitmap, and favorable-action stale rejection |
| `per_asset_slot_last_prevents_cross_asset_accrual_aliasing` | `v15_per_asset_slot_last_prevents_cross_asset_accrual_aliasing`; strengthened `proof_v15_per_asset_slot_last_prevents_cross_asset_accrual_aliasing` checks full non-accrued asset state isolation |
| `reset_pending_epoch_start_snapshots_prevent_prior_epoch_resurrection` | `v15_side_reset_snapshots_epoch_start_for_prior_epoch_accounts`; `proof_v15_reset_pending_epoch_start_snapshots_prevent_prior_epoch_resurrection`; side-reset finalize prior-epoch tests/proofs |
| `hinted_subset_cannot_hide_toxic_leg` | `v15_trade_hint_cannot_hide_toxic_portfolio_leg_on_other_asset`; `proof_v15_trade_hint_cannot_hide_toxic_portfolio_leg_on_other_asset` |
| `stale_certificate_loses_margin_credit` | `v15_full_refresh_clears_stale_certificate_but_not_b_stale_loss`; `proof_v15_full_refresh_clears_stale_certificate`; stale counter proofs |
| `stale_profitable_leg_cannot_support_risk_increase` | stale certificate and full-refresh gating tests/proofs; target/effective lag and h-lock no-positive-credit trade proofs |
| `stale_profitable_leg_zero_or_penalty_credit_for_withdraw` | `v15_stale_profitable_leg_cannot_withdraw_using_pre_refresh_positive_pnl`; `proof_v15_stale_profitable_leg_cannot_withdraw_using_pre_refresh_positive_pnl`; stale profitable support is refreshed and hidden losses are settled before withdrawal can extract vault value |
| `rebalance_conserves_senior_claims` | `v15_rebalance_reduce_position_requires_strict_risk_progress_and_preserves_senior_claims`; `proof_v15_rebalance_reduce_position_preserves_senior_claims_and_reduces_risk` |
| `rebalance_cannot_double_count_collateral` | `v15_cross_margin_collateral_counted_once_and_not_below_loss_envelope`; `proof_v15_cross_margin_equity_counts_collateral_once_and_score_uses_full_envelope` |
| `cross_margin_offset_cap_never_below_loss_envelope` | `v15_cross_margin_collateral_counted_once_and_not_below_loss_envelope`; public config envelope proofs |
| `unhealthy_rebalance_requires_strict_risk_progress` | `v15_rebalance_rejects_missing_or_zero_progress`; `proof_v15_liquidation_progress_rejects_non_reducing_scores`; rebalance risk-progress proof |
| `cyclic_rescue_without_progress_reverts` | `v15_cyclic_rescue_without_scalar_progress_reverts`; non-progress liquidation/rebalance proofs |
| `B_stale_blocks_withdraw_convert_close_and_risk_increase` | `v15_b_stale_blocks_refresh_and_favorable_actions_without_scanning_market`; `proof_v15_b_stale_blocks_refresh_and_favorable_actions`; `proof_v15_favorable_locks_block_released_pnl_conversion_before_mutation`; `proof_v15_b_stale_trade_preflight_rolls_back_partial_side_effects`; `proof_v15_deposit_into_stale_or_b_stale_account_does_not_unlock_favorable_actions` |
| `account_B_settlement_chunks_huge_delta_without_market_scan` | `v15_account_b_chunk_makes_strict_account_local_progress_or_requires_recovery`; `proof_v15_account_b_chunk_either_advances_or_fails_closed` |
| `B_booking_exact_remainder_conservation` | `v15_b_residual_booking_is_bounded_and_remainder_conserving`; `proof_v15_b_residual_booking_makes_durable_progress_or_fails_closed` |
| `bankrupt_close_books_residual_without_opposing_scan` | bankrupt liquidation residual-durability tests/proofs; residual booking tests/proofs; no full-market scan crank proof |
| `bankrupt_close_cannot_clear_basis_before_residual_durable` | `v15_bankrupt_liquidation_requires_residual_durable_before_freeing_exposure`; `proof_v15_bankrupt_liquidation_cannot_free_exposure_before_residual_durable` |
| `bad_asset_residual_charged_only_to_asset_side_domain` | `v15_liquidation_residual_domain_is_opposite_side_for_long_and_short`; `proof_v15_long_liquidation_residual_charges_short_domain`; `proof_v15_short_liquidation_residual_charges_long_domain` |
| `domain_budgeted_insurance_prevents_bad_asset_global_insurance_drain` | `v15_bad_asset_cannot_spend_unrelated_domain_insurance_budget`; `proof_v15_bad_asset_cannot_spend_unrelated_domain_insurance_budget`; domain-budget liquidation proofs |
| `liquidation_order_cannot_choose_residual_domain` | `v15_liquidation_residual_domain_is_opposite_side_for_long_and_short`; both liquidation residual proofs exercise production `liquidate_account_not_atomic` with caller-independent request fields and assert only the opposite-side domain is spent |
| `portfolio_insurance_allocation_is_caller_independent` | Domain-budget liquidation tests/proofs; unrelated-budget proof shows global insurance cannot be drained outside the selected domain; long/short residual-domain proofs show insurance spend is derived from bankrupt exposure side, not caller liquidation ordering |
| `pending_domain_loss_barrier_blocks_weight_exit_until_residual_durable` | `v15_pending_domain_loss_barrier_blocks_other_participants_until_residual_done`; `v15_pending_domain_loss_barrier_blocks_side_reset_before_residual_done`; `v15_pending_obligation_blocks_side_reset_until_obligation_account_clears`; `v15_pending_domain_loss_barrier_does_not_freeze_unrelated_positive_credit`; `proof_v15_pending_domain_barrier_blocks_participants_until_residual_finalized`; `proof_v15_pending_domain_barrier_blocks_side_reset_before_mutation`; `proof_v15_pending_obligation_blocks_side_reset_until_clear`; `proof_v15_pending_domain_barrier_does_not_freeze_unrelated_positive_credit`; full exits are covered as zero-basis obligation exits rather than hard rejects, and side reset is blocked until the obligation account clears |
| `pending_barrier_allows_risk_reduction_with_weight_obligation_preserved` | `v15_pending_domain_loss_barrier_allows_partial_risk_reduction_with_weight_obligation_preserved`; `v15_pending_domain_loss_barrier_allows_full_trade_exit_as_flat_weight_obligation`; `v15_pending_domain_loss_barrier_allows_rebalance_reduction_with_weight_obligation_preserved`; `v15_pending_domain_loss_barrier_allows_rebalance_full_exit_as_flat_weight_obligation`; `v15_flat_pending_obligation_must_settle_b_loss_before_clear`; `proof_v15_pending_domain_barrier_allows_rebalance_reduction_with_weight_obligation_preserved`; `proof_v15_pending_domain_barrier_allows_trade_reduction_with_weight_obligation_preserved`; `proof_v15_pending_domain_barrier_allows_full_trade_exit_as_flat_weight_obligation`; `proof_v15_pending_domain_barrier_allows_rebalance_full_exit_as_flat_weight_obligation`; `proof_v15_flat_pending_obligation_cannot_clear_before_b_settlement`; same-side reductions, including full exits, preserve the account's pre-barrier loss weight as a flat obligation until the barrier clears, and a flat obligation cannot clear before K/F/B settlement catches up |
| `oi_positive_requires_loss_weight_or_recovery` | `v15_public_invariants_reject_oi_loss_weight_shape_mismatch`; `proof_v15_public_invariants_reject_hard_global_bounds`; attach/clear and quantity-ADL OI symmetry tests/proofs |
| `live_oi_symmetric_in_live_mode` | `v15_public_invariants_reject_live_oi_imbalance`; `proof_v15_public_invariants_reject_hard_global_bounds`; trade, liquidation, rebalance, and quantity-ADL OI symmetry tests/proofs |
| `staged_insurance_not_double_spent` | `v15_bankrupt_liquidation_consumes_insurance_before_social_loss`; `v15_bankrupt_liquidation_drops_uncollectible_fee_and_spends_insurance_once`; matching bankrupt-liquidation proofs |
| `bankruptcy_residual_excludes_protocol_fees` | `v15_bankrupt_liquidation_drops_uncollectible_fee_and_spends_insurance_once`; `proof_v15_bankrupt_liquidation_excludes_fee_from_residual_and_spends_insurance_once` |
| `uncollectible_fees_forgiven_not_socialized` | fee loss-seniority tests/proofs; wide fee sync test/proof; bankrupt liquidation fee-exclusion test/proof |
| `insurance_boundary_non_deficit_paths` | `proof_v15_non_deficit_public_paths_do_not_decrease_insurance`; bankrupt liquidation insurance-spend proofs |
| `positive_pnl_support_not_withdrawable_without_gates` | h-lock/no-positive-credit withdraw proofs; favorable-action lock conversion proofs; stale-profitable withdraw test/proof shows pre-refresh positive PnL cannot become withdrawable value |
| `account_free_equity_active_accrual_requires_protective_progress` | `v15_account_free_equity_active_accrual_requires_protective_progress`; `v15_permissionless_crank_flat_refresh_is_not_protective_for_equity_active_accrual`; `v15_permissionless_crank_cross_asset_liquidation_is_not_protective_for_accrued_asset`; `proof_v15_equity_active_accrual_requires_protective_progress`; `proof_v15_permissionless_flat_refresh_is_not_protective_for_equity_active_accrual`; `proof_v15_permissionless_cross_asset_liquidation_is_not_protective_for_equity_active_accrual` |
| `effective_price_raw_target_lag_no_free_option` | target/effective lag trade, withdraw, and conversion tests; `proof_v15_target_effective_lag_rejects_risk_increasing_trade_before_mutation`; `proof_v15_target_effective_lag_blocks_pnl_conversion_before_mutation`; `proof_v15_favorable_locks_block_released_pnl_conversion_before_mutation` |
| `loss_stale_catchup_blocks_risk_increase_until_current` | `v15_loss_stale_blocks_nonflat_withdrawal_even_if_no_positive_credit_suffices`; `v15_loss_stale_blocks_risk_increasing_trade_even_with_no_positive_credit_margin`; `v15_loss_stale_allows_pure_risk_reducing_trade_path`; `proof_v15_loss_stale_blocks_nonflat_withdrawal`; `proof_v15_loss_stale_blocks_risk_increasing_trade_before_mutation` |
| `domain_locks_do_not_freeze_asset_accrual` | `v15_pending_domain_loss_barrier_does_not_freeze_asset_accrual`; `proof_v15_pending_domain_loss_barrier_does_not_freeze_asset_accrual` |
| `current_step_locking_does_not_reintroduce_maximal_serialization` / `side_lock_does_not_freeze_unrelated_side_accrual` | `v15_pending_domain_loss_barrier_does_not_block_unrelated_side_reset`; `proof_v15_pending_domain_barrier_does_not_block_unrelated_side_reset`; unrelated positive-credit and asset-accrual barrier tests/proofs |
| `close_id_reused_across_preemption_restart_until_finalized` / `new_close_id_for_unfinalized_account_reverts` | `v15_new_close_cannot_overwrite_active_finalized_close_ledger`; `proof_v15_new_close_cannot_overwrite_active_finalized_close_ledger` |
| `drift_reference_slot_immutable_across_preemption_restart` / `max_close_slot_immutable_across_recompute` / `repeated_preemption_cannot_extend_close_lifetime` | `v15_close_progress_uses_configured_lifetime_and_does_not_refresh_on_continuation`; `proof_v15_close_lifetime_uses_configured_bound_and_is_not_refreshed`; stale/expired close recovery tests/proofs |
| `durable_quantity_adl_requires_matching_close_progress_ledger_advance` | `v15_account_shape_rejects_malformed_quantity_adl_close_progress`; `proof_v15_account_shape_rejects_malformed_quantity_adl_close_progress`; quantity-ADL finalization tests/proofs |
| `resolved_close_one_account_bounded` | resolved flat/profit/active-position/partial-B tests; resolved close proofs |
| `permissionless_recovery_no_caller_chosen_price` | `v15_permissionless_recovery_is_declared_by_reason_not_caller_price`; `v15_permissionless_recovery_cannot_override_resolved_mode`; `v15_recovery_reason_is_terminal_and_idempotent`; `v15_recovery_mode_cannot_be_overridden_by_resolve`; `v15_recovery_mode_blocks_value_escape_and_fee_sync_before_mutation`; `v15_recovery_mode_rejects_non_recovery_crank_before_account_mutation`; `v15_recovery_mode_rejects_liquidation_and_rebalance_before_account_mutation`; `proof_v15_permissionless_recovery_declares_reason_or_fails_closed`; `proof_v15_terminal_recovery_reason_and_mode_are_immutable`; `proof_v15_recovery_mode_blocks_value_escape_paths_before_mutation`; `proof_v15_recovery_mode_rejects_non_recovery_crank_before_account_mutation`; `proof_v15_recovery_mode_rejects_liquidation_and_rebalance_before_mutation`; recovery crank proof |
| `recovery_fallback_price_required_for_public_markets` | `v15_public_init_rejects_disabled_recovery_fallback_price_policy`; `v15_persisted_account_wire_rejects_invalid_bool_enum_and_option_encoding`; `proof_v15_public_config_rejects_invalid_user_fund_shapes`; `proof_v15_persisted_wire_rejects_noncanonical_bool_enum_and_option` |
| `explicit_loss_audit_overflow_does_not_trap_funds` | `v15_explicit_loss_audit_overflow_declares_recovery`; `proof_v15_explicit_loss_audit_overflow_declares_recovery_without_mutation` |
| `owner_dead_leg_forfeit_does_not_hostage_unrelated_collateral` | `v15_permissionless_recovery_enters_terminal_mode_and_enables_dead_leg_forfeit`; `v15_dead_leg_forfeit_is_unavailable_for_normal_live_leg`; `v15_dead_leg_forfeit_detaches_without_crediting_positive_pnl`; `v15_dead_leg_forfeit_books_negative_residual_to_opposing_domain_only`; `proof_v15_permissionless_recovery_enables_dead_leg_forfeit_without_value_escape`; `proof_v15_dead_leg_forfeit_does_not_credit_positive_kf_delta`; `proof_v15_dead_leg_forfeit_books_loss_to_opposing_domain_only` |
| `effective_support_consumption_burns_required_face_junior_claim` / `support_consumed_cannot_exceed_g_value_of_face_claim_burned` | `v15_dead_leg_forfeit_haircuts_positive_support_when_junior_impaired`; `proof_v15_dead_leg_forfeit_haircuts_positive_support_when_junior_impaired`; `v15_full_refresh_uses_haircut_bounded_support_for_negative_kf_delta_when_impaired`; `proof_v15_negative_kf_settlement_uses_haircut_support_not_face_netting`; `v15_full_refresh_uses_haircut_bounded_new_positive_kf_to_cure_prior_loss`; `proof_v15_positive_kf_delta_cures_prior_loss_at_haircut_value` |
| `partial_liquidation_cannot_socialize_while_account_support_remains` | `v15_partial_liquidation_cannot_b_book_residual_while_open_risk_remains`; `proof_v15_partial_liquidation_cannot_socialize_residual_while_open_risk_remains` |
| `authoritatively_flat_account_never_receives_B_loss` | `v15_authoritatively_flat_account_never_receives_b_loss`; `proof_v15_authoritatively_flat_account_never_receives_b_loss` |
| `no_single_instruction_full_market_requirement` | no-slab v15 architecture; no full-market scan crank test/proof; account-local crank and refresh tests/proofs |
| `worst_case_hinted_progress_totality` | `v15_worst_case_hinted_progress_actions_are_total_and_bounded`; `proof_v15_worst_case_hinted_progress_actions_are_total_and_bounded` |
| `global_accumulator_not_account_health_proof` | `v15_global_residual_is_not_account_health_proof`; `proof_v15_global_residual_is_not_account_health_proof` |
| `active_bitmap_canonical_no_hidden_legs` | `v15_active_bitmap_is_the_only_active_leg_authority`; `proof_v15_hidden_leg_rejected_by_bitmap_authority` |
| `canonical_single_leg_per_asset_no_same_asset_double_support` | `v15_same_asset_duplicate_leg_cannot_double_count_support`; `proof_v15_same_asset_duplicate_leg_cannot_double_count_support`; production `attach_leg` duplicate guard |
| `N_too_large_rejected_at_public_user_fund_init` / `cfg_max_bankrupt_close_lifetime_slots_positive` | `v15_public_init_rejects_unbounded_portfolio_width`; `v15_public_init_requires_crankforward_recovery_and_chunk_caps`; `proof_v15_configured_portfolio_width_rejects_out_of_range_leg`; public config proof |
| `PNL_pos_bound_tot_prevents_lazy_positive_pnl_first_mover_overpay` | `v15_pnl_pos_bound_tot_prevents_lazy_positive_pnl_first_mover_overpay`; `proof_v15_pnl_pos_bound_tot_prevents_lazy_positive_pnl_first_mover_overpay`; resolved positive-payout bound-denominator test/proof |
| `resolved_payout_readiness_uses_exact_counters_and_bounds` | `v15_resolved_payout_readiness_uses_exact_counters_and_bounds`; `v15_resolved_positive_payout_waits_for_pending_domain_loss_barrier`; `proof_v15_resolved_payout_readiness_uses_exact_counters_and_bounds`; existing resolved close partial-B/active-position/payout proofs |

No missing engine-side spec §16 coverage item was identified in this pass.

Additional Anchor v2 zero-copy persistence coverage:

| Property | Coverage |
|---|---|
| Persisted account/wire structs are `bytemuck::Pod` and `Zeroable` | `v15_persisted_account_wire_structs_are_bytemuck_pod` |
| Persisted account/wire structs are byte-aligned and bytemuck-readable | `v15_persisted_account_wire_structs_are_bytemuck_pod`; `v15_persisted_account_wire_roundtrips_runtime_state` |
| Persisted bool/enum/Option encodings fail closed, including public recovery fallback config | `v15_persisted_account_wire_rejects_invalid_bool_enum_and_option_encoding`; `proof_v15_persisted_wire_rejects_noncanonical_bool_enum_and_option` |
| Persisted signed economic fields reject `i128::MIN` | `proof_v15_persisted_wire_rejects_i128_min_economic_fields` |
| Persisted provenance, active bitmap, and hidden-leg smuggling fails closed | `proof_v15_persisted_wire_rejects_provenance_and_hidden_leg_smuggling` |
| Runtime/persisted conversion preserves validated state | `v15_persisted_account_wire_roundtrips_runtime_state`; `proof_v15_market_wire_roundtrip_preserves_valid_runtime_state`; `proof_v15_portfolio_wire_roundtrip_preserves_valid_runtime_state` |

## V12 Property Migration

The old v12 proof inventory had 416 Kani harnesses. Many were intentionally not
ported because v15 removed the slab, fixed account capacity, full-market cursor
scan, v12 reserve queues, and wrapper-era entrypoints. The applicable properties
were migrated to v15 production-code tests/proofs.

Migrated property families covered in the v15 suite:

| v12 property family | v15 coverage |
|---|---|
| Deposit/withdraw accounting roundtrip | `proof_v15_deposit_then_withdraw_roundtrip_preserves_accounting`, `proof_v15_partial_withdraw_can_leave_small_remainder` |
| Multiple deposits aggregate into senior totals | `proof_v15_multiple_deposits_aggregate_c_tot_and_vault` |
| Account close/reclaim requires clean local state | `proof_v15_close_portfolio_account_requires_clean_local_state` |
| Malformed signed fee-credit and PnL state fails closed | malformed account-shape tests/proofs and fee-credit/PnL proofs |
| Conservative risk-notional arithmetic | `proof_v15_risk_notional_flat_zero_and_monotone_in_price` |
| Shared wide arithmetic floor/ceil/K-diff semantics | `tests/proofs_v15_arithmetic.rs` |
| Position bounds reject before OI mutation | `proof_v15_oversize_position_rejected_before_oi_mutation` |
| Price/funding accrual matches eager account settlement | price and funding refresh tests/proofs |
| Same-slot exposed price move cannot mutate state | `proof_v15_same_slot_exposed_price_move_rejects_before_mutation` |
| Funding cap rejects before state mutation | `proof_v15_funding_rate_above_cap_rejects_before_mutation` |
| Dynamic trade-fee cap, conservation, and OI symmetry | dynamic fee tests/proofs and trade conservation proofs |
| Invalid/risk-increasing trade rejects before mutation | invalid trade, health, h-lock positive-credit dependency, loss-stale, and target/effective lag tests/proofs |
| Sign-flip trades preserve OI symmetry and senior totals | sign-flip trade tests/proofs |
| Released PnL conversion cannot mint beyond residual | released-PnL conversion tests/proofs |
| Permissionless refresh must return partial B progress | permissionless partial-B refresh tests/proofs |
| Public user-fund config must keep recovery/fallback/profile guarantees enabled | public config tests/proofs |
| Liquidation must strictly improve account risk and preserve residual durability | liquidation progress, partial liquidation, bankrupt residual, and fee-exclusion tests/proofs |
| Resolved close payout/progress behavior | resolved flat, positive, fee-current, partial-B, and active-position tests/proofs |

## Static Strength Scan

Strength indicators:

| Check | Result |
|---|---:|
| Harnesses over v15 production engine/wire methods | 162 |
| Harnesses over shared production arithmetic helpers | 7 |
| Harnesses with `kani::cover!` reachability checks | 138 |
| Explicit `kani::assume(false)` / `assume(false)` findings | 0 |
| Confirmed vacuous harnesses | 0 |
| Confirmed weak harnesses | 0 |

Current classification:

| Classification | Status |
|---|---|
| Non-vacuity | No confirmed vacuous harnesses found. Cover checks exercise h-min/h-max, stale set/clear, stale/B-stale deposit lock preservation, hidden-leg rejection, persisted provenance/bitmap smuggling rejection, persisted recovery fallback bool rejection, B-chunk progress paths, B-stale trade rollback, malformed fee-credit states, invalid config branches including disabled recovery fallback policy and zero close lifetime, OI/loss-weight shape mismatch branches, aggregate deposit branches, arithmetic floor/ceil branches, positive/negative K-diff branches, bankrupt residual recovery, zero/partial insurance paths, unrelated-domain insurance budget exclusion, non-deficit insurance-boundary public paths, favorable-action lock composition, scoped pending-domain barrier accrual, close-id overwrite rejection, configured close-lifetime anchoring and continuation immutability, malformed quantity-ADL ledgers, close-progress domain mismatch rejection, stale open-close snapshot recovery before B/ADL mutation, pending-domain barrier trade/rebalance risk reductions and full-exit flat obligations, pending-obligation side-reset blocking, flat pending-obligation B-stale clear rejection, unrelated-domain positive-credit conversion under an active pending-domain barrier, same-asset protective-progress gating for permissionless crank accrual, terminal recovery mode and dead-leg forfeit enablement, terminal recovery reason/mode immutability, terminal recovery value-escape, crank-mutation, liquidation, and rebalance rejection, permissionless partial-B refresh, released-PnL zero/positive conversion paths, loss-stale risk-increase rejection, resolved readiness blockers including pending-domain-loss barriers, resolved partial-B close progress, partial-liquidation recovery, and rebalance reduction paths. |
| Weak proofs | No confirmed weak proofs in the v15 inventory. Concrete-branch harnesses are intentional regression proofs over production methods, and symbolic arithmetic/transition harnesses cover the remaining branch families. |
| Inductive strength | The stale-counter and arithmetic helper proofs are closest to local inductive transition proofs. The overall suite is a strong production-code safety/liveness harness set, not a complete arbitrary-state inductive proof of the whole engine. |
| Practical proof boundary | The suite proves key v15 account-local invariants over real production methods: h-lock selection, provenance/hidden-leg fail-closed behavior, persisted wire provenance/bitmap fail-closed behavior, public recovery fallback config fail-closed behavior, stale counter idempotence and refresh clearing, stale/B-stale deposit lock preservation, malformed signed state rejection, OI/loss-weight canonicality, deposit/withdraw accounting, aggregate senior accounting, close-account local-state gating, risk-notional monotonicity, position-bound fail-before-mutation, B-chunk progress/fail-closed behavior, close-id immutability against active-ledger overwrite, configured close lifetime and immutable close drift anchors across continuation, quantity-ADL close-progress shape, close-progress residual-domain shape, stale open-close snapshot recovery before B/ADL mutation, B-stale trade preflight rollback through the public staged API, pending-domain-loss barrier same-side risk reduction and full-exit liveness with preserved flat loss-weight obligation, pending-obligation reset blocking, flat pending-obligation B-stale clear rejection, without freezing unrelated positive-credit conversion or unrelated asset accrual, bounded repeated B-chunk completion for small residuals, multi-asset full-refresh settlement/scoring, non-deficit public-path insurance preservation, domain-budgeted insurance isolation, full-refresh gating, favorable-action lock fail-before-mutation behavior, monotonic liquidation-score rejection, loss-before-fee ordering, account-free equity-active accrual protective-progress gating, terminal recovery declaration and immutability, dead-leg forfeit value preservation, terminal recovery value-escape blocking, terminal recovery crank/liquidation/rebalance mutation blocking, one-segment bounded catchup, funding-rate cap fail-before-mutation, dynamic trade-fee enforcement, trade conservation/OI symmetry, target/effective lag risk-increase rejection, h-lock risk-increase no-positive-credit acceptance/rejection, h-lock risk-reducing liveness under no-positive-credit margin, h-lock withdrawal no-positive-credit gating, released-PnL conversion bounded by residual, loss-stale nonflat withdrawal and risk-increasing trade blocking, bankrupt liquidation insurance-before-social-loss ordering, bankrupt residual durability before exposure release, partial-liquidation residual recovery before socialization, uncollectible liquidation-fee exclusion from residual loss, resolved close liveness and payout readiness including pending-domain-loss barriers, durable B residual booking, prior-epoch reset clearing, quantity-ADL OI symmetry, rebalance strict risk-progress, price/funding settlement, invalid trade rollback, partial liquidation, and shared wide arithmetic semantics. |

## Rust Test Matrix

| Command | Result |
|---|---|
| `cargo test` | PASS on 2026-05-17 |
| `cargo test --features test` | PASS on 2026-05-17 |
| `cargo test --features fuzz` | PASS on 2026-05-17 |
| `cargo test --all-features` | PASS on 2026-05-17 |

## Audit Conclusion

No missing engine-side v15 spec §16 coverage item was identified. No confirmed
weak or vacuous proof was identified in the current static pass. Applicable v12
property families have been reviewed and either ported to v15 production-code
tests/proofs or retired as slab/wrapper/v12-queue-specific.

The only open audit-maintenance item is to rerun `scripts/run_kani_full_audit.sh`
against the current 169-proof inventory and replace the older 57-proof timing
artifacts.
