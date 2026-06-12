# Kani Proof Strength Audit Results

Generated: 2026-06-12 (full certification re-run; supersedes the 2026-06-10
193/193 audit). Every artifact verified IN ISOLATION (one harness per kani
invocation, pkill-clean between runs, 900s suite budget / 1800s
compile-inclusive layer budget).

## Certified inventory: 250/250 PASS, zero failures

| layer | artifacts | result | notes |
|---|---|---|---|
| suite (tests/proofs_v16.rs) | 199 | 199/199 PASS | constructed-state Kani proofs over the public surface: junior-pool conservation lattice (pool-isolated or exact-delta for every public op), gates/rejections-before-mutation, two-op sequence witnesses, close-ownership exclusion, spec #19/#24 witnesses |
| contracts (src/v16_proofs.rs, -Z function-contracts) | 34 | 34/34 PASS | full-input-domain leaf contracts: complete counterparty lien lifecycle, insurance lien family, domain-insurance moves, aggregate maintenance, flow-typing transit witnesses (all 11 incl. the cure/support/resolved-exit skeletons of the intractable bodies), &mut-self debit (modifies/old) |
| closure (src/v16_proofs.rs, plain) | 17 | 17/17 PASS | inductive: genesis + encumbrance-ledger closure under all 12 deltas (any state satisfying inv), bucket status-machine closure (4 delta-level) |

Suite solver-time stats: median 54s, max 785s
(budget 900s), 170/199 within the 300s ideal.

Complementary non-Kani layers (all green at certification):
- 12 proptest properties (backing double-claim, close order-independence,
  re-close idempotence, random-sequence extraction bound, rounding-residue
  direction suite) at 300-2000 cases each;
- 8 runtime test suites;
- runtime fail-closed validation (validate_shape / flow-proof validate() on
  every execution of every intractable body).

## Spec coverage
See scripts/spec-coverage.md: 26 STRONG, 5 PARTIAL (named accept-reasons),
1 permanent GAP (#25 ADL atomicity -- integration-level), 3 SPEC-AHEAD-OF-ENGINE
findings awaiting user decision (#21 ClosePriority preemption, #23 drift
reserve, #31 recovery fallback price -- mechanisms specified but not
implemented; the engine's implemented alternatives are proven).

## Boundary (proven, not assumed)
The intractable tier (trade/realize/cure/close monolithic bodies) was
eliminated under seven reduction strategies (concrete, stubbed validators,
solver swap, scale shrink, combinations, reduced-leg profile, function-
contract composition) -- documented in src/v16_proofs.rs. Public-op contracts
over arbitrary symbolic states closed by the deposit probe. Division- and
multiplication-bearing leaves are not contract-checkable in this kani
generation; their semantics are covered by exact suite proofs with concrete
operands plus the direction fuzz.

## Reproduction
- suite:    LOG_DIR=<dir> BUDGET_S=900 bash scripts/isolated_runner.sh  (roster: grep proof_v16_ tests/proofs_v16.rs)
- contracts: FEATURES=fuzz,contracts bash scripts/contracts_runner.sh
- closure:  LOG_DIR=kani_closure FEATURES=fuzz,closure KANI_Z= bash scripts/contracts_runner.sh
