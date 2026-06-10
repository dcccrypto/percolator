# Kani Proof Strength Audit Results

Generated: 2026-06-10

Source prompt: `scripts/audit-proof-strength.md`.

## Current Inventory

Static inventory from the current `master` tree (post v16.8.9):

| File | Kani proofs |
|---|---:|
| `tests/proofs_v16.rs` | 182 |
| `tests/proofs_v16_arithmetic.rs` | 11 |
| Total | 193 |

## Full Audit

Commands:

```text
cargo test --tests --features fuzz
LOG_DIR=kani_full_audit_v3 BUDGET_S=900 bash scripts/isolated_runner.sh
```

Results:

| Check | Result |
|---|---:|
| Rust tests (8 suites incl. proptest fuzz) | 106 passed, 0 failed |
| Kani harnesses | 193 passed, 0 failed |
| Kani timeouts | 0 |
| Total per-harness wall time | 20499s |

Timing artifact: `kani_audit_final.tsv` (one row per harness, isolated runs,
900s budget, orphaned-cbmc cleanup between proofs).

## Static Strength Classification (per audit prompt criteria 1-6)

- **0 unit tests masquerading as proofs.** Every harness takes symbolic input
  except documented CONCRETE WITNESSES, each flagged in-code with the
  tractability rationale and a pointer to the randomized runtime coverage:
  `proof_v16_expired_backing_yields_zero_realizable_support_after_expiry`
  (any symbolic input exceeds the budget; the symbolic surface is fuzzed in
  `tests/backing_double_claim_fuzz.rs`).
- **0 vacuous proofs.** `kani::cover!` discipline is universal; every
  assume-constrained path carries a reachability cover. High-assume harnesses
  (up to 11 assumes) all prove their interesting branches reachable.
- **0 weak-invariant substitutions.** Conservation proofs assert either full
  `validate_shape` (which under cfg(kani) includes the full audit scan and
  aggregate reconciliation) or exact loop-free field deltas. Rejection proofs
  assert the specific error variant plus state-unchanged, and pair with
  accepting twins.
- **Known intractable region (documented in-code):** harnesses entering the
  realize/convert path (`realize_source_backed_claims` value semantics) and
  the full `close_resolved_account_not_atomic` path exceed the 900s budget
  even fully concrete (in-path account validation forces unwind 40; per-domain
  U256 credit math). Coverage decomposes into passing component proofs
  (consume-delta exactness, support caps, close-ledger partition equality,
  expiry-liveness primitive) plus 5 randomized end-to-end properties
  (300 cases each) in `tests/backing_double_claim_fuzz.rs` and 2 in
  `tests/resolved_insolvent_fuzz.rs`.

## Slowest Harnesses

| Harness | Time | Status |
|---|---:|---|
| `proof_v16_residual_excludes_recoverable_counterparty_backing_principal` | 672s | PASS |
| `proof_v16_public_resolved_close_flat_account_pays_only_capital_and_vault` | 605s | PASS |
| `proof_v16_public_counterparty_backing_deposit_refills_expired_receivable_bucket` | 511s | PASS |
| `proof_v16_expired_backing_yields_zero_realizable_support_after_expiry` | 496s | PASS |
| `proof_v16_public_insurance_lien_consume_debits_only_domain_insurance` | 471s | PASS |
| `proof_v16_public_counterparty_backing_deposit_moves_vault_and_scaled_source_state` | 460s | PASS |
| `proof_v16_public_insurance_lien_release_restores_reserved_credit_without_value_movement` | 452s | PASS |
| `proof_v16_public_account_backing_fee_split_preserves_senior_stock` | 424s | PASS |
| `proof_v16_public_counterparty_lien_release_restores_unliened_backing_without_value_movement` | 414s | PASS |
| `proof_v16_withdraw_settles_flat_negative_pnl_before_value_exit` | 404s | PASS |

## Method Notes

- One-at-a-time isolated runs with `pkill` cleanup between proofs remain
  mandatory: a concurrent second runner's cleanup killed an in-flight cbmc and
  produced a phantom 39s "TIMEOUT" (re-run in isolation: 42s PASS).
- Solver-variance fragility: a witness that passed once at 658s timed out on
  three re-runs at 900s. Harnesses sitting above ~70% of budget should be
  reduced or dropped; one such witness was dropped with its coverage
  re-documented (see the tractability note in `tests/proofs_v16.rs`).
