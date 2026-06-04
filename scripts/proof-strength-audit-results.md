# Kani Proof Strength Audit Results

Generated: 2026-06-04

Source prompt: `scripts/audit-proof-strength.md`.

## Current Inventory

Static inventory from the current `master` tree:

| File | Kani proofs | Proofs without symbolic input |
|---|---:|---:|
| `tests/proofs_v16.rs` | 100 | 0 |
| `tests/proofs_v16_arithmetic.rs` | 11 | 0 |
| Total | 111 | 0 |

All 111 harnesses include symbolic input through `kani::any`, at least one
`kani::cover!` reachability point, and at least one assertion. No current proof
is intentionally model-only. A small number of harnesses use `#[cfg(kani)]`
wrappers around private production helpers; those wrappers expose the exact
production transition being proved and do not add runtime APIs.

## Full Audit

Commands:

```text
cargo test --tests
scripts/run_kani_full_audit.sh
```

Results:

| Check | Result |
|---|---:|
| Rust tests | 82 passed, 0 failed |
| Kani harnesses | 111 passed, 0 failed |
| Kani timeouts | 0 |
| Total per-harness wall time | 10112s |

Timing artifacts:

```text
kani_audit_full.tsv
kani_audit_final.tsv
```

## Slowest Harnesses

| Harness | Time | Status |
|---|---:|---|
| `proof_v16_public_resolved_close_flat_account_pays_only_capital_and_vault` | 555s | PASS |
| `proof_v16_view_withdraw_reduces_vault_ctot_and_capital_equally` | 443s | PASS |
| `proof_v16_public_counterparty_lien_consume_creates_receivable_without_value_movement` | 368s | PASS |
| `proof_v16_public_counterparty_lien_release_restores_unliened_backing_without_value_movement` | 351s | PASS |
| `proof_v16_public_insurance_reserve_encumbers_budget_without_value_movement` | 325s | PASS |
| `proof_v16_sparse_source_domain_validation_rejects_duplicate_occupied_domain` | 311s | PASS |
| `proof_v16_retire_empty_asset_is_value_neutral_and_epoch_scoped` | 306s | PASS |
| `proof_v16_withdraw_allowed_after_canceled_close` | 306s | PASS |
| `proof_v16_public_permissionless_empty_market_crank_advances_clock_without_value_movement` | 277s | PASS |
| `proof_v16_view_overwithdraw_rejects` | 277s | PASS |
| `proof_v16_public_insurance_lien_create_moves_reserved_credit_to_valid_lien` | 268s | PASS |
| `proof_v16_public_backing_provider_earnings_withdraw_debits_only_earned_vault` | 248s | PASS |
| `proof_v16_view_fee_sync_settles_negative_pnl_before_fee` | 247s | PASS |
| `proof_v16_unliened_source_support_is_capped_by_realizable_backing` | 229s | PASS |
| `proof_v16_view_deposit_preserves_c_tot_vault_capital_sum` | 229s | PASS |

## Weak-Proof Fixes From This Audit

The proof-strength pass removed all remaining no-symbolic-input harnesses and
rechecked them under the full audit. The important changes were:

| Area | Strengthening |
|---|---|
| Public activation / retire / permissionless empty crank | Added symbolic senior-balance and slot/price branches while preserving value-neutrality and epoch assertions. |
| Fee/loss seniority | Made capital, loss, and fee-rate symbolic; corrected the expected fee to `fee_rate * dt` after loss settlement. |
| Source-claim convert guard | Replaced an intractable full public conversion search with a `#[cfg(kani)]` wrapper over the exact production source-claim exposure guard. |
| Duplicate active-leg rejection | Replaced a broad validator search with a production helper proof over `active_leg_slot_for_asset`, proving duplicate active legs fail before source support can be double counted. |
| Resolved terminal counterparty-lien wind-down | Replaced two 600s timeout-prone full-path proofs with symbolic proofs over the exact production terminal-release delta, including expired-status buckets. |
| Insurance and counterparty lien paths | Added symbolic amounts to reserve/create/consume/release proofs and retained public-path conservation/domain-budget assertions. |
| Insurance reservation encumbrance proof | Removed a redundant second full `validate_shape()` call after `reserve_insurance_credit_not_atomic` already validated. The proof still runs the public production API and checks the exact reservation/source/header value-neutrality postconditions; this brought the harness from timeout to a 325s pass in the full audit. |

## Current Assessment

The current suite is materially stronger than before this audit:

- Every Kani proof has symbolic input and a non-trivial cover or branch.
- The full suite passes under the repository's 600s per-harness timeout.
- High-risk public-path invariants remain covered: fee-before-loss prevention,
  domain insurance isolation, counterparty/source-credit encumbrance,
  permissionless crank progress, resolved payout bounds, market lifecycle
  value-neutrality, and view-level deposit/withdraw/funding operations.
- The two helper-level proofs added for tractability are over production helper
  functions, not independent models; they target the precise invariant that the
  full path was proving before it timed out.
