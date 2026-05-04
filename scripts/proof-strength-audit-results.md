# Kani Proof Audit Results

Generated: 2026-05-01

Source prompt: `scripts/audit-proof-strength.md`.

Timing sweep command: `scripts/run_kani_full_audit.sh`.

## Current Tree Addendum

Updated: 2026-05-04.

This report's **full timing sweep** is still the 2026-05-01 overnight run
below. It is now stale for the current tree: the parsed proof inventory is
`403` standard `#[kani::proof]` harnesses, while the recorded overnight sweep
covered `333`.

Targeted production-code proofs added after the overnight sweep and rerun on
2026-05-03 and 2026-05-04:

| Harness | Time | Status | Scope |
|---|---:|---|---|
| `proof_permissionless_progress_dispatcher_recovers_b_index_headroom_on_prod_code` | 17.23s | PASS | Dispatcher reaches P-last B-index recovery. |
| `proof_live_insurance_withdraw_fails_closed_when_exposed_or_reconciling_on_prod_code` | 40.89s | PASS | Live insurance withdrawal fails closed under exposure/reconciliation and remains available for empty current markets. |
| `proof_permissionless_progress_dispatcher_decreases_live_catchup_rank_on_prod_code` | 61.54s | PASS | Dispatcher ordinary-crank branch reduces live catchup rank. |
| `proof_permissionless_progress_dispatcher_decreases_active_close_rank_on_prod_code` | 36.22s | PASS | Dispatcher active-close branch reduces residual rank before ordinary crank. |
| `proof_permissionless_progress_dispatcher_reduces_live_catchup_rank_on_prod_code` | 70.65s | PASS | Dispatcher ordinary-crank branch strictly reduces the public progress rank. |
| `proof_permissionless_progress_dispatcher_recovers_b_headroom_blocker_on_prod_code` | 23.58s | PASS | Dispatcher routes exhausted B-index headroom through public recovery. |
| `proof_permissionless_progress_dispatcher_recovers_counter_or_epoch_overflow_on_prod_code` | 52.03s | PASS | Dispatcher routes global counter overflow through public P-last recovery without using caller raw target or moving vault, capital, or insurance funds. |
| `proof_permissionless_progress_dispatcher_reduces_resolved_blocker_rank_on_prod_code` | 21.82s | PASS | Dispatcher resolved-cursor branch strictly reduces the public progress rank. |
| `proof_permissionless_progress_resolved_progress_only_makes_account_fee_current_on_prod_code` | 25.13s | PASS | Dispatcher resolved ProgressOnly path syncs the touched account to resolved_slot before returning progress without terminal payout/free. |
| `proof_force_close_resolved_with_fee_progress_only_syncs_before_payout_on_prod_code` | 21.96s | PASS | Production fee-aware resolved close syncs and charges fees before returning ProgressOnly, leaving the positive account open and unpaid while terminal readiness is blocked. |
| `proof_force_close_resolved_rechecks_terminal_counters_despite_ready_flag_on_prod_code` | 17.99s | PASS | Production fee-aware resolved close rechecks terminal counters before honoring an already-set payout-ready flag, leaving positive winners unpaid while blockers remain. |
| `proof_active_close_recovery_records_residual_before_resolve_on_prod_code` | 8.28s | PASS | Active-close recovery records residual as non-claim loss without minting vault, capital, or insurance funds. |
| `proof_explicit_loss_recovery_resolves_at_p_last_without_minting_claims_on_prod_code` | 15.51s | PASS | Explicit unallocated-loss recovery resolves at P-last and preserves explicit non-claim loss without minting vault, capital, or insurance funds. |
| `proof_below_floor_recovery_rejects_when_bounded_step_can_progress_on_prod_code` | 7.51s | PASS | Below-progress-floor P-last recovery fails closed without moving vault, capital, or insurance funds while bounded catchup can still make a price step. |
| `proof_blocked_segment_recovery_rejects_when_bounded_accrual_can_progress_on_prod_code` | 351.55s | PASS | Blocked-segment P-last recovery fails closed without moving vault, capital, or insurance funds while the production accrual planner can still advance a bounded segment. |
| `proof_insurance_reward_credit_fails_closed_under_reconciliation_on_prod_code` | 33.08s | PASS | Insurance-funded account credit fails closed under h-max/loss-stale reconciliation and preserves accounting otherwise. |
| `proof_live_insurance_withdraw_blocks_active_close_or_negative_pnl_on_prod_code` | 15.37s | PASS | Live insurance withdrawal fails closed without moving vault or insurance during active-close reconciliation or while negative PnL remains. |
| `proof_insurance_reward_credit_blocks_active_close_or_negative_pnl_on_prod_code` | 20.03s | PASS | Insurance-funded account credit fails closed without moving insurance, recipient capital, or recipient PnL during active-close reconciliation or while negative PnL remains. |
| `proof_adl_pipeline_books_b_and_schedules_resets_on_prod_code` | 9.53s | PASS | Replaces stale K-residual ADL pipeline proof; production ADL books bankruptcy residual through B and schedules both side resets. |
| `proof_adl_b_loss_booking_bounded_by_rounded_settlement_effect` | 65.79s | PASS | Replaces the stale ADL K-loss timeout with production B-index residual booking and proves represented settlement loss is bounded by the deficit. |
| `proof_adl_uncertified_potential_dust_routes_deficit_without_b_or_k_write` | 34.95s | PASS | Production ADL routes deficits to non-claim audit loss when uncertified potential dust makes the B denominator unsafe. |
| `proof_production_b_residual_booking_or_recording_accounts_for_full_deficit` | 25.49s | PASS | Production B residual booking/recording accounts for every deficit atom while preserving vault, capital totals, insurance, and represented account capital; recorded atoms remain durable non-claim audit loss. |
| `v19_speculative_hmax_does_not_mask_prior_positive_pnl_use_on_prod_code` | 3.87s | PASS | Production bankruptcy-residual trigger fails closed when speculative Phase 2 h-max would otherwise mask earlier ordinary positive-PnL usability. |
| `v19_phase1_positive_pnl_use_forces_later_phase2_bankruptcy_fail_closed_on_prod_code` | 59.99s | PASS | Production Phase 1 candidate runner plus Phase 2 live touch fails closed when an ordinary positive-PnL release precedes a bankruptcy trigger in the same instruction. |
| `v19_phase2_replay_latent_bankruptcy_pauses_winner_release_on_prod_code` | 53.97s | PASS | Production live-touch replay proves a winner -> latent-bankrupt Phase 2 window progresses while keeping the winner reserve paused. |
| `v19_explicit_fee_rejects_nonflat_unsettled_side_effects_before_fee_draw` | 35.45s | PASS | Production explicit-fee path rejects nonflat stale K/F/A/B side effects before moving account capital into insurance, preserving capital, PnL, fee credits, insurance, and current_slot. |
| `v19_generation_first_wrap_advances_on_prod_code` | 13.81s | PASS | Production keeper crank advances sweep generation on a permitted cursor wrap. |
| `v19_same_slot_cursor_does_not_wrap_without_generation_advance` | 13.23s | PASS | Production keeper crank cannot cross the cursor wrap boundary again in the same authenticated slot after generation already advanced. |
| `proof_property_31_settle_rejects_missing_account_on_prod_code` | 6s | PASS | Production settle rejects a missing account without materializing it. |
| `proof_property_31_withdraw_rejects_missing_account_on_prod_code` | 7s | PASS | Production withdraw rejects a missing account without materializing it. |
| `proof_property_31_trade_rejects_missing_party_a_on_prod_code` | 7s | PASS | Production trade rejects a missing maker/taker party before materialization. |
| `proof_property_31_trade_rejects_missing_party_b_on_prod_code` | 8s | PASS | Production trade rejects a missing counterparty before materialization. |
| `proof_property_31_liquidate_rejects_missing_account_on_prod_code` | 7s | PASS | Production liquidation rejects a missing account without materializing it. |
| `proof_property_31_keeper_candidate_does_not_materialize_missing_account_on_prod_code` | 13s | PASS | Production keeper candidate scan ignores missing candidate slots without materializing them. |
| `proof_keeper_rejects_funding_rate_above_config_before_state_mutation_on_prod_code` | 9s | PASS | Production keeper crank rejects an out-of-config funding rate before market clock, price, F, K, or stress mutation. |
| `proof_keeper_crank_accepts_positive_boundary_funding_rate_on_prod_code` | 13s | PASS | Production keeper crank accepts the configured positive funding-rate boundary and advances the market slot. |
| `proof_keeper_crank_accepts_negative_boundary_funding_rate_on_prod_code` | 14s | PASS | Production keeper crank accepts the configured negative funding-rate boundary and advances the market slot. |
| `proof_property_56_raw_initial_margin_predicate_rejects_min_floor_shortfall_on_prod_code` | 2.78s | PASS | Production raw initial-margin predicate rejects a nonzero-position floor shortfall. |
| `proof_property_56_trade_margin_gate_rejects_raw_im_shortfall_on_prod_code` | 2.50s | PASS | Production post-trade margin gate rejects a risk-increasing raw IM floor shortfall. |
| `t11_53_keeper_phase1_stops_after_pending_reset_on_prod_code` | 4.14s | PASS | Production keeper Phase 1 candidate helper stops on a pending reset before mutating later candidates. |
| `proof_live_touch_decreases_account_b_rank_on_prod_code` | 37.48s | PASS | Production live touch strictly reduces account-local B settlement rank for a hinted blocker. |
| `proof_permissionless_account_b_progress_reduces_hinted_account_b_rank_on_prod_code` | 240.35s | PASS | Production permissionless account-B progress branch validates, touches, finalizes, strictly reduces the hinted account's B rank, preserves market clock/price, and does not spend insurance. |
| `proof_active_close_continuation_preserves_frozen_economics_on_prod_code` | 15.19s | PASS | Production active-close continuation books one bounded B residual chunk while preserving frozen close account/side/price/slot/quantity metadata. |
| `proof_permissionless_progress_dispatcher_recovers_exhausted_active_close_on_prod_code` | 14.87s | PASS | Production permissionless dispatcher routes exhausted active-close residuals to P-last recovery after recording the remainder as durable non-claim loss. |
| `proof_resolved_terminal_close_rejects_account_b_stale_position_on_prod_code` | 17.61s | PASS | Production resolved terminal close rejects a B-stale account before freeing or paying it. |

The old `proof_adl_pipeline_trade_liquidate_reopen` harness is no longer part
of the current tree. Its 2026-05-01 failure below is historical: it asserted the
pre-v12.20.6 K-residual behavior that the engine intentionally replaced with
B-index bankruptcy residual booking. The deterministic unit test
`adl_b_pipeline_drains_resets_and_reopens_balanced_oi` covers the full
stale-settle/reopen lifecycle that is too large for a useful finishing Kani
harness.

The old `proof_adl_k_loss_write_bounded_by_rounded_settlement_effect` timeout is
also superseded in the current tree. Bankruptcy residuals are no longer written
through K; `proof_adl_b_loss_booking_bounded_by_rounded_settlement_effect`
proves the current B-index residual path, and the deterministic unit regression
`adl_k_loss_must_not_overcharge_floor_rounded_opposing_accounts` covers the
full live-touch settlement path.

The old `v19_generation_advances_at_most_once_per_slot` timeout is no longer
part of the current tree. Its combined two-crank harness was split into the
finishing production-code proofs
`v19_generation_first_wrap_advances_on_prod_code` and
`v19_same_slot_cursor_does_not_wrap_without_generation_advance`, which together
cover the permitted wrap and same-slot no-second-wrap halves of the slot-rate
generation invariant.

The old `proof_property_31_missing_account_safety` timeout is no longer part of
the current tree. It was a broad multi-entrypoint harness; the current tree
replaces it with six endpoint-specific production-code proofs covering settle,
withdraw, both trade parties, liquidation, and keeper-candidate scanning.

The old `proof_funding_rate_validated_before_storage` and
`proof_keeper_crank_r_last_stores_supplied_rate` timeouts are no longer part of
the current tree. They were broad keeper harnesses; the current tree replaces
them with three endpoint-specific production-code proofs covering invalid-rate
fail-closed behavior before mutation and both signed configured boundary rates.

The old `proof_property_56_exact_raw_im_approval` timeout is no longer part of
the current tree. It was split into two finishing production-code proofs: one
for the exact raw initial-margin predicate and one for the post-trade margin
gate. Both require the rejection path to be reachable through `kani::cover!`.

The old `t11_53_keeper_crank_quiesces_after_pending_reset` timeout is no longer
part of the current tree. Its broad full-crank harness was split into a
finishing production-code proof over the keeper Phase 1 helper used by the real
keeper entrypoint, plus the deterministic full-keeper regression
`keeper_phase1_stops_after_liquidation_schedules_pending_reset`.

These targeted passes do **not** replace a full proof-strength certification.
The next authoritative update should rerun `scripts/run_kani_full_audit.sh`
against the current 403-harness inventory, then rerun the static strength /
non-vacuity audit over the same inventory.

Kani version: `0.66.0`. The sweep script parsed `333` unique `#[kani::proof]` harnesses from `tests/proofs_*.rs` and ran each one with exact harness selection and a `600s` timeout.

The checked-in `kani-list.json` inventory was refreshed during the audit and now reports `333` standard harnesses.

Timing artifacts:

```text
kani_audit_full.tsv
kani_audit_final.tsv
```

`kani_audit_full.tsv` contains one row per proof with raw timing and status. `kani_audit_final.tsv` contains the same rows plus the sweep note `overnight-2026-05-01`.

## Latest Full Timing Sweep

```text
SUMMARY: 325 passed, 8 failed/timeout (7 timeout) out of 333
```

This sweep completed and recorded timings for every parsed proof harness. It did not complete cleanly: one proof failed and seven proofs hit the `600s` cap.

## Non-Passing Harnesses

| Harness | Time | Status | Note |
|---|---:|---|---|
| `proof_adl_k_loss_write_bounded_by_rounded_settlement_effect` | 600s | TIMEOUT | Hit per-harness cap. |
| `proof_adl_pipeline_trade_liquidate_reopen` | 68s | FAIL | Failed assertion: `deficit must be socialized to the opposing short side K` in `tests/proofs_liveness.rs:527`. |
| `proof_funding_rate_validated_before_storage` | 600s | TIMEOUT | Hit per-harness cap. |
| `proof_keeper_crank_r_last_stores_supplied_rate` | 600s | TIMEOUT | Hit per-harness cap. |
| `proof_property_31_missing_account_safety` | 600s | TIMEOUT | Hit per-harness cap. |
| `proof_property_56_exact_raw_im_approval` | 600s | TIMEOUT | Hit per-harness cap. |
| `t11_53_keeper_crank_quiesces_after_pending_reset` | 600s | TIMEOUT | Hit per-harness cap. |
| `v19_generation_advances_at_most_once_per_slot` | 600s | TIMEOUT | Hit per-harness cap. |

The one FAIL was rerun exactly:

```text
timeout 600 cargo kani --tests --exact --harness proof_adl_pipeline_trade_liquidate_reopen --output-format terse
```

It failed again in about `67s` with the same assertion. The rerun log was captured at `/tmp/kani_proof_adl_pipeline_trade_liquidate_reopen.log` during this audit session.

## Slowest Passing Harnesses

| Harness | Time |
|---|---:|
| `proof_validate_hint_preflight_oracle_shift` | 226s |
| `t3_16b_reset_counter_with_nonzero_k_diff` | 161s |
| `t3_16_reset_pending_counter_invariant` | 146s |
| `v19_cascade_safety_gate_disabled_preserves_invariants` | 142s |
| `t14_63_dust_bound_position_reattach_remainder` | 136s |
| `t0_2_mul_div_ceil_algebraic_identity` | 134s |
| `t11_54_worked_example_regression` | 125s |
| `t6_26_full_drain_reset_regression` | 121s |
| `t2_12_floor_shift_lemma` | 120s |
| `proof_force_close_resolved_with_profit_conserves` | 109s |
| `t3_14_epoch_mismatch_forces_terminal_close` | 109s |
| `proof_wide_signed_mul_div_floor_sign_and_rounding` | 83s |
| `bounded_withdraw_conservation` | 70s |
| `t3_14b_epoch_mismatch_with_nonzero_k_diff` | 70s |
| `proof_property_51_withdraw_any_partial_ok` | 64s |

## Current Audit Boundary

This file records the latest full per-harness Kani timing sweep. It should not be read as an all-green proof-strength certification: the current run has one failing proof and seven timing out proofs.

The prior static strength pass found no confirmed weak or vacuous harnesses in the then-current proof inventory, but the proof inventory has since grown from `312` to `333` parsed harnesses. Any new claim that all current proofs are strong and non-vacuous requires a fresh static strength pass over the current 333-harness inventory, separate from this timing sweep.

## Previously Surfaced Spec Coverage

The proof suite continues to surface the main engine obligations by name through harness coverage:

| Spec obligation | Surfaced coverage |
|---|---|
| Slot-rate-limited stress reset | `v19_generation_first_wrap_advances_on_prod_code`, `v19_same_slot_cursor_does_not_wrap_without_generation_advance`, `v19_same_slot_stress_wrap_defers_generation_reset`, `v19_stress_envelope_clear_requires_later_wrap` |
| Sparse sweep budget semantics | `v19_rr_touch_zero_no_cursor_advance`, `v19_rr_scan_zero_no_stress_progress`, `v19_greedy_phase2_model_respects_touch_budget_and_bounds` |
| Stress accounting and admission hardening | `v19_accrual_consumption_only_commits_on_success`, `v19_consumption_monotone_within_generation`, `v19_funding_consumption_accumulates_scaled_bps`, `v19_admit_gate_stress_lane_forces_h_max`, `v19_admit_gate_some_zero_rejected` |
| Phantom-dust cleanup bounds | `proof_unilateral_empty_orphan_dust_clearance`, `t13_56_unilateral_empty_orphan_resolution`, `t13_57_unilateral_empty_corruption_guard`, `t13_58_unilateral_empty_short_side`, `t13_58b_unilateral_empty_short_requires_long_bound` |
| ADL phantom dust and residual-loss safety | `proof_adl_b_loss_booking_bounded_by_rounded_settlement_effect`, `proof_adl_uncertified_potential_dust_routes_deficit_without_b_or_k_write`, `t13_60_unconditional_dust_bound_on_any_a_decay`, `t14_61_dust_bound_adl_a_truncation_sufficient`, `t14_65_dust_bound_end_to_end_clearance`, `t4_22_k_overflow_routes_to_absorb` |
| Reset lifecycle and side-mode gates | `proof_drain_only_to_reset_progress`, `proof_keeper_reset_lifecycle_last_stale_triggers_finalize`, `t11_43_end_instruction_auto_finalizes_ready_side`, `t3_16_reset_pending_counter_invariant`, `proof_side_mode_gating` |
| Exact arithmetic and risk checks | `proof_funding_sign_and_floor`, `proof_symbolic_margin_enforcement_on_reduce`, `proof_notional_scales_with_price`, `proof_wide_signed_mul_div_floor_sign_and_rounding`, `t0_2_mul_div_ceil_algebraic_identity` |
| Resolved/terminal conservation | `proof_force_close_resolved_position_conservation`, `proof_force_close_resolved_with_profit_conserves`, `proof_force_close_resolved_pos_count_decrements`, `proof_force_close_resolved_fee_sweep_conservation` |

## Inductive Criteria Snapshot

No current claim is made that the suite is fully inductive. The prior audit classified proof style against these criteria:

| Criterion | Current status |
|---|---|
| 6a State construction | Harnesses generally use constructed states (`RiskEngine::new`, helper allocation, direct field setup), not arbitrary invariant-satisfying symbolic engine states. |
| 6b Topology coverage | Many proofs use 1-2 account topologies. This exercises key scenarios but does not prove arbitrary account topology or abstract rest-of-system properties. |
| 6c Invariant decomposition | No reusable decomposed invariant predicate suite is present in the proof files. Properties are asserted directly or via conservation helpers. |
| 6d Loop-free invariant specs | No general loop-free inductive invariant framework is present. |
| 6e Cone of influence | Constructed engine state fixes many fields outside the function under test, limiting generality compared with symbolic state plus minimal assumptions. |
| 6f Full domain vs bounded ranges | Bounded symbolic ranges are common and are used for tractability. |
