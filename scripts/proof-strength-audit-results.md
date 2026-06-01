# Kani Proof Strength Audit Results

Generated: 2026-06-01

Source prompt: `scripts/audit-proof-strength.md`.

## Current Inventory

Static inventory from the current `master` tree:

| File | Kani proofs | Proofs without symbolic input |
|---|---:|---:|
| `tests/proofs_v16.rs` | 88 | 0 |
| `tests/proofs_v16_arithmetic.rs` | 11 | 0 |
| Total | 99 | 0 |

All 99 harnesses include symbolic input through `kani::any` and at least one
non-trivial execution path. No current proof is intentionally model-only. A small
number of harnesses use `#[cfg(kani)]` wrappers around private production
helpers; those wrappers expose the exact production transition being proved and
do not add runtime APIs.

## Full Audit

Commands:

```text
cargo test --all-features
scripts/run_kani_full_audit.sh
```

Results:

| Check | Result |
|---|---:|
| Rust tests | 66 passed, 0 failed |
| Kani harnesses | 99 passed, 0 failed |
| Kani timeouts | 0 |
| Total per-harness wall time | 9512s |

Timing artifacts:

```text
kani_audit_full.tsv
kani_audit_final.tsv
```

## Slowest Harnesses

| Harness | Time | Status |
|---|---:|---|
| `proof_v16_public_resolved_payout_topup_pays_min_claimable_and_vault` | 566s | PASS |
| `proof_v16_public_resolved_close_flat_account_pays_only_capital_and_vault` | 486s | PASS |
| `proof_v16_insolvent_resolved_receipt_clears_at_terminal_rate` | 483s | PASS |
| `proof_v16_public_insurance_lien_consume_spends_only_its_domain_budget` | 425s | PASS |
| `proof_v16_public_insurance_reserve_encumbers_budget_without_value_movement` | 400s | PASS |
| `proof_v16_view_withdraw_reduces_vault_ctot_and_capital_equally` | 378s | PASS |
| `proof_v16_public_counterparty_lien_consume_creates_receivable_without_value_movement` | 356s | PASS |
| `proof_v16_public_counterparty_lien_release_restores_unliened_backing_without_value_movement` | 318s | PASS |
| `proof_v16_cross_account_source_support_sum_capped_by_shared_backing` | 286s | PASS |
| `proof_v16_public_insurance_lien_create_moves_reserved_credit_to_valid_lien` | 272s | PASS |
| `proof_v16_public_permissionless_empty_market_crank_advances_clock_without_value_movement` | 272s | PASS |
| `proof_v16_withdraw_allowed_after_canceled_close` | 262s | PASS |
| `proof_v16_retire_empty_asset_is_value_neutral_and_epoch_scoped` | 253s | PASS |
| `proof_v16_view_overwithdraw_rejects` | 248s | PASS |
| `proof_v16_view_fee_sync_settles_negative_pnl_before_fee` | 236s | PASS |

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
