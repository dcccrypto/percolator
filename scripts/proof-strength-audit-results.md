# Kani Proof Strength Audit Results

Generated: 2026-06-12 (full certification re-run; supersedes the 2026-06-10
193/193 audit). Every artifact verified IN ISOLATION (one harness per kani
invocation, pkill-clean between runs, 900s suite budget / 1800s
compile-inclusive layer budget).

## Certified inventory: 266/266 PASS, zero failures

(250 base certified 2026-06-12 at 8da7343; +16 exact-frame suite proofs verified in isolation across waves F1-F6, commits 7ac57f6..bfd29a5. BRANCH NOTE (kernel-proofs): production src/v16.rs HAS changed since that base — 7 kernels extracted from the monolithic bodies with production now calling them — so this inventory is inherited evidence pending the fresh full-branch re-certification, which is being run; the kernel harnesses' own PASS results are recorded in scripts/kernel_results.tsv.)

| layer | artifacts | result | notes |
|---|---|---|---|
| suite (tests/proofs_v16.rs) | 215 | 215/215 PASS (199 base + 16 exact-frame) | constructed-state Kani proofs over the public surface: junior-pool conservation lattice (pool-isolated or exact-delta for every public op), gates/rejections-before-mutation, two-op sequence witnesses, close-ownership exclusion, spec #19/#24 witnesses, and the EXACT-FRAME lattice (16 ops: whole-state equality vs pre-state-except-declared-deltas, incl. the Err-atomicity template and the crank clock/cert-only frame) |
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
See scripts/spec-coverage.md (current): 35 STRONG, 1 PARTIAL-accepted (#37),
1 STRUCTURAL (#3), 1 N/A, 0 GAP rows at the engine boundary. Spec v16.9.0
reconciled all former spec-ahead-of-engine items. The boundary theorem
(scripts/boundary_audit.py: all 55 public entrypoints terminate Ok paths in
state validators) makes the global validity predicate hold at every committed
state under the Err-non-commit execution assumption.

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
