# Kani Proof Audit Results

Generated: 2026-05-01

Source prompt: `scripts/audit-proof-strength.md`.

Timing sweep command: `scripts/run_kani_full_audit.sh`.

## Current Tree Addendum

Updated: 2026-05-03.

This report's **full timing sweep** is still the 2026-05-01 overnight run
below. It is now stale for the current tree: the parsed proof inventory is
`375` standard `#[kani::proof]` harnesses, while the recorded overnight sweep
covered `333`.

Targeted production-code proofs added after the overnight sweep and rerun on
2026-05-03:

| Harness | Time | Status | Scope |
|---|---:|---|---|
| `proof_permissionless_progress_dispatcher_recovers_b_index_headroom_on_prod_code` | 17.23s | PASS | Dispatcher reaches P-last B-index recovery. |
| `proof_live_insurance_withdraw_fails_closed_when_exposed_or_reconciling_on_prod_code` | 40.89s | PASS | Live insurance withdrawal fails closed under exposure/reconciliation and remains available for empty current markets. |
| `proof_permissionless_progress_dispatcher_decreases_live_catchup_rank_on_prod_code` | 61.54s | PASS | Dispatcher ordinary-crank branch reduces live catchup rank. |
| `proof_permissionless_progress_dispatcher_decreases_active_close_rank_on_prod_code` | 36.22s | PASS | Dispatcher active-close branch reduces residual rank before ordinary crank. |

These targeted passes do **not** replace a full proof-strength certification.
The next authoritative update should rerun `scripts/run_kani_full_audit.sh`
against the current 375-harness inventory, then rerun the static strength /
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
| Slot-rate-limited stress reset | `v19_generation_advances_at_most_once_per_slot`, `v19_same_slot_stress_wrap_defers_generation_reset`, `v19_stress_envelope_clear_requires_later_wrap` |
| Sparse sweep budget semantics | `v19_rr_touch_zero_no_cursor_advance`, `v19_rr_scan_zero_no_stress_progress`, `v19_greedy_phase2_model_respects_touch_budget_and_bounds` |
| Stress accounting and admission hardening | `v19_accrual_consumption_only_commits_on_success`, `v19_consumption_monotone_within_generation`, `v19_funding_consumption_accumulates_scaled_bps`, `v19_admit_gate_stress_lane_forces_h_max`, `v19_admit_gate_some_zero_rejected` |
| Phantom-dust cleanup bounds | `proof_unilateral_empty_orphan_dust_clearance`, `t13_56_unilateral_empty_orphan_resolution`, `t13_57_unilateral_empty_corruption_guard`, `t13_58_unilateral_empty_short_side`, `t13_58b_unilateral_empty_short_requires_long_bound` |
| ADL phantom dust and K-loss safety | `proof_adl_k_loss_write_bounded_by_rounded_settlement_effect`, `t13_60_unconditional_dust_bound_on_any_a_decay`, `t14_61_dust_bound_adl_a_truncation_sufficient`, `t14_65_dust_bound_end_to_end_clearance`, `t4_22_k_overflow_routes_to_absorb` |
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
