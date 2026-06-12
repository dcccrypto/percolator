# The No-Steal Theorem — proof composition over certified artifacts

Status: composition document. Every lemma below names machine-checked
artifacts (suite proof / contract / closure proof / flow witness / fuzz
property / runtime check), all certified 250/250 + frame-wave additions
(scripts/proof-strength-audit-results.md, kani_audit_certified.tsv).

## Theorem (engine no-steal)

> For every reachable engine state and every public transition: quote value
> is conserved with an explicitly typed flow; the senior stock stays senior;
> the transition's effect on state is EXACTLY its declared delta set (frame);
> failures change nothing; and no account, domain, or asset becomes more
> withdrawable, more claimable, or less loss-bearing through a transition
> that does not name it.

The theorem is established as the conjunction of five machine-checked
lattices plus runtime enforcement, NOT as one monolithic contract (proven
infeasible in this toolchain generation — see the elimination table in
src/v16_proofs.rs).

## Lemma 1 — Typed value conservation (flow lattice)
Every public body that moves quote value constructs a TokenValueFlowProofV16
and must pass validate() AT RUNTIME on every execution (engine code). All 11
transit constructors are proven to produce exactly their typed moves and to
balance under their typed vault movement:
* contract layer: contract_check_flow_* (8 simple transits), the multi-leg
  skeletons contract_check_flow_close_cure_to_account_capital /
  support_to_account_capital / capital_and_resolved_payout_to_external_out,
  and contract_check_flow_proof_debit_modifies (&mut-self ledger primitive).

## Lemma 2 — Junior-pool conservation (value-delta lattice)
Every public op either cannot move the junior residual pool or moves it by
exactly the declared amount:
* suite: the pool-isolation/exact-delta asserts across every public-op proof
  (deposit/withdraw, fees, cranks, resolve, oracle ops, lifecycle marks,
  loss reservation, earnings credit/withdraw, resolved topup, terminal
  receipt-clear, insurance flows). See commits aff6598..a0ffb83 and the
  per-proof kani_residual() asserts.

## Lemma 3 — Encumbrance soundness for ALL reachable states (closure lattice)
No lien/backing/insurance atom can be double-used, and every lifecycle delta
preserves ledger validity, inductively (genesis + closure under all 12
deltas over arbitrary inv-satisfying states):
* closure layer: closure_ledger_inv_* (13), closure_bucket_status_machine_*
  (4); contracts: the exact-delta leaf family (consume/create/release/
  terminal/impair/withdraw/add + insurance family).
* fuzz: backing_double_claim suite (double-claim, full-backing-range
  conservation, order independence, idempotence, extraction bound).

## Lemma 4 — Exact frame (nothing else moves)
For each tractable public op, the ENTIRE post-state equals the pre-state
except the declared deltas — whole-struct equality over the full header
(39 fields), the engine slot (13 incl. nested ledgers), and the whole
account (21 incl. all legs/domains):
* suite: proof_v16_frame_* (deposit, withdraw, domain-insurance pair,
  earnings withdraw, resolve, drain-only, budget credit, ...).
* Cross-asset corollary: an op naming asset/domain S cannot change any field
  of any other asset's slot — the frame proofs compare the untouched slot
  byte-for-byte.

## Lemma 5 — Failure atomicity (Err => unchanged)
Failed transitions mutate nothing:
* suite: 23+ rejects-before-mutation gate proofs, upgraded by the frame-Err
  template (proof_v16_frame_overwithdraw_err_leaves_state_unchanged) to
  whole-struct equality; close-exclusion proofs (occupied-domain, active-
  close) reject pre-mutation.

## Lemma 6 — Identity, isolation, and gates
* provenance/market-group binding: suite rejection proofs + runtime
  validate_with_market on every account-touching op.
* per-account/per-domain close exclusion + monotone close identity: suite
  close-ownership proofs.
* senior stock: runtime validate_header_aggregate_totals
  (c_tot+I+earnings+cbp <= vault) on every shape validation; contract
  earnings senior-coverage gate.

## What this composition does NOT cover (explicit boundary)
1. The intractable bodies (trade fill, realize, cure, close monoliths) carry
   Lemmas 1/2/3/5/6 via their gates, value skeletons, components, fuzz, and
   runtime checks — but have no monolithic frame proof (elimination table,
   src/v16_proofs.rs). Their runtime flow-proof validation is the enforced
   backstop on every execution.
2. Wrapper/Solana concerns (account loading, signers, CPI, oracle auth,
   serialization round-trips) — outside this repo.
3. #25 ADL/finalization atomicity — integration-level.
4. Spec-ahead-of-engine items (#21 preemption tuple, #23 drift reserve,
   #31 fallback price) — mechanisms not implemented; implemented
   alternatives proven (scripts/spec-coverage.md).
