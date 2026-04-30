# Kani Proof Strength Audit Results

Generated: 2026-04-30

Source prompt: `scripts/audit-proof-strength.md`.

Execution note: `scripts/audit proof strength` is not an executable in this checkout. This audit applies the prompt directly to the current `tests/proofs_*.rs` harnesses and uses `cargo kani list --format json` for the harness inventory.

Kani version: `0.66.0`. Kani-listed standard harnesses: `310`. Parsed proof harnesses: `310`.

This is a proof-strength audit, not the overnight full CBMC verification run. It classifies harness shape, symbolic breadth, non-vacuity risk, and inductive strength.

## Final Tally

| Classification | Count | Audit meaning |
|---|---:|---|
| **INDUCTIVE** | 0 | Fully symbolic initial state plus assumed decomposed invariant and loop-free modular preservation proof. |
| **STRONG** | 175 | Symbolic proof harness with meaningful assertions and no observed vacuity risk, but not inductive. |
| **WEAK** | 0 | Symbolic harness with a proof-strength issue that should be tightened. |
| **UNIT TEST** | 135 | Concrete or deterministic scenario harness with no `kani::any()` input. |
| **VACUOUS** | 0 | Confirmed contradictory assumptions or unreachable assertions. |

## Key Findings

- No WEAK harnesses remain after this pass.
- No confirmed VACUOUS harnesses were found.
- No Ok-gated assertion patterns remain. The prior `if result.is_ok() { assert!(...) }` harnesses were strengthened into explicit success proofs with unconditional postconditions.
- No proof harness lacks a checked outcome. Six harnesses have no direct `assert!`, and all six are intentional `#[kani::should_panic]` negative checks.
- No trivially false `kani::assume(false)` or `assert!(true)` proof patterns were found.
- No harness is INDUCTIVE under the prompt definition. The suite still uses constructed engine states rather than a fully symbolic `RiskEngine` with decomposed invariant assumptions.
- Concrete regression harnesses are retained as UNIT TEST by the audit rubric. They are useful scenario coverage, but they are not counted as symbolic proofs.
- The new sparse-sweep/stress-envelope harnesses are not weak implementation snapshots: they check spec-level properties for zero touch limits, greedy touch-budget bounds, funding and price stress accounting, no same-slot stress clearing, stress-envelope clearing only after a later eligible wrap, and at-most-once-per-slot generation advancement.

## Strengthened Harnesses

These harnesses previously gated assertions behind an Ok path or accepted an impossible Err path. They now assert the valid call must succeed and then check the spec postcondition unconditionally:

- `proof_goal23_deposit_no_insurance_draw`
- `inductive_withdraw_preserves_accounting`
- `bounded_withdraw_conservation`
- `proof_audit3_compute_trade_pnl_no_panic_at_boundary`

## Targeted Kani Verification

The four strengthened harnesses were re-run one by one with exact Kani harness selection:

```text
cargo kani --tests --exact --harness proof_goal23_deposit_no_insurance_draw --output-format terse
cargo kani --tests --exact --harness inductive_withdraw_preserves_accounting --output-format terse
cargo kani --tests --exact --harness bounded_withdraw_conservation --output-format terse
cargo kani --tests --exact --harness proof_audit3_compute_trade_pnl_no_panic_at_boundary --output-format terse
```

All four completed successfully.

The new/changed sparse-sweep and stress-envelope harnesses were also run one by one:

```text
cargo kani --tests --exact --harness v19_consumption_monotone_within_generation --output-format terse
cargo kani --tests --exact --harness v19_consumption_floor_below_one_bp --output-format terse
cargo kani --tests --exact --harness v19_funding_consumption_accumulates_scaled_bps --output-format terse
cargo kani --tests --exact --harness v19_rr_touch_zero_no_cursor_advance --output-format terse
cargo kani --tests --exact --harness v19_greedy_phase2_model_respects_touch_budget_and_bounds --output-format terse
cargo kani --tests --exact --harness v19_same_slot_stress_wrap_defers_generation_reset --output-format terse
cargo kani --tests --exact --harness v19_stress_envelope_clear_requires_later_wrap --output-format terse
cargo kani --tests --exact --harness v19_generation_advances_at_most_once_per_slot --output-format terse
cargo kani --tests --exact --harness v19_accrual_consumption_only_commits_on_success --output-format terse
```

All nine completed successfully. Reported verification times:

| Harness | Time |
|---|---:|
| `v19_consumption_monotone_within_generation` | 35.914470s |
| `v19_consumption_floor_below_one_bp` | 18.993318s |
| `v19_funding_consumption_accumulates_scaled_bps` | 15.079335s |
| `v19_rr_touch_zero_no_cursor_advance` | 10.603538s |
| `v19_greedy_phase2_model_respects_touch_budget_and_bounds` | 2.405437s |
| `v19_same_slot_stress_wrap_defers_generation_reset` | 11.800297s |
| `v19_stress_envelope_clear_requires_later_wrap` | 28.692291s |
| `v19_generation_advances_at_most_once_per_slot` | 209.424547s |
| `v19_accrual_consumption_only_commits_on_success` | 4.627249s |

## Validation Commands

The audit update and strengthened harnesses were validated with:

```text
cargo fmt --all -- --check
git diff --check
cargo test --features test
cargo test --no-default-features
cargo test --no-default-features --features small
cargo test --no-default-features --features medium
```

All commands completed successfully.

## Inductive Criteria 6a-6f

| Criterion | Current status |
|---|---|
| 6a State construction | Engine harnesses use constructed states (`RiskEngine::new`, helper allocation, direct field setup). None quantify over all invariant-satisfying states. |
| 6b Topology coverage | Mostly 1-2 account topologies. This exercises key scenarios but does not prove arbitrary account topology or abstract rest-of-system properties. |
| 6c Invariant decomposition | No reusable decomposed invariant predicates are present in the proof files. Properties are asserted directly or via `check_conservation()`. |
| 6d Loop-free invariant specs | No loop-free inductive invariant spec suite is present. Some properties are local arithmetic/delta checks, but there is no general modular invariant framework. |
| 6e Cone of influence | Constructed engine state fixes many fields outside the function under test. This limits generality compared with symbolic state plus minimal assumptions. |
| 6f Full domain vs bounded ranges | Bounded symbolic ranges are common. This is appropriate for tractability but prevents full-domain inductive classification. |

## Per-File Tally

| File | Total | STRONG | WEAK | UNIT TEST |
|---|---:|---:|---:|---:|
| `tests/proofs_admission.rs` | 39 | 32 | 0 | 7 |
| `tests/proofs_arithmetic.rs` | 19 | 19 | 0 | 0 |
| `tests/proofs_audit.rs` | 33 | 11 | 0 | 22 |
| `tests/proofs_checklist.rs` | 16 | 12 | 0 | 4 |
| `tests/proofs_instructions.rs` | 51 | 16 | 0 | 35 |
| `tests/proofs_invariants.rs` | 26 | 20 | 0 | 6 |
| `tests/proofs_lazy_ak.rs` | 15 | 13 | 0 | 2 |
| `tests/proofs_liveness.rs` | 11 | 0 | 0 | 11 |
| `tests/proofs_safety.rs` | 76 | 35 | 0 | 41 |
| `tests/proofs_v1131.rs` | 24 | 17 | 0 | 7 |

## Remaining Audit Boundary

This audit verifies harness strength and checks the strengthened harnesses with Kani. It does not replace the full overnight `scripts/run_kani_full_audit.sh` run across all 310 harnesses.
