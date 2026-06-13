# The No-Steal Theorem — proof composition over certified artifacts

Status: composition document. Every lemma below names machine-checked
artifacts (suite proof / contract / closure proof / flow witness / fuzz
property / runtime check), all certified 250/250 + frame-wave additions
(scripts/proof-strength-audit-results.md, kani_audit_certified.tsv).

## GlobalValidState — the named predicate

`GlobalValidState(market, account) :=`
  `market.validate_shape()` (senior cover
  `c_tot + insurance + earnings + counterparty_backing_principal <= vault`,
  exact O(1) aggregate totals == per-domain sums, per-domain ledger closure,
  per-status bucket shapes) `AND`, for every account a transition touches,
  `a.validate_with_market(market)` (provenance/identity binding, active-bitmap
  /leg canonicality, per-account shape). The validators' SEMANTICS are
  Kani-proven (aggregate-scan proofs, ledger-parts closure layer, senior-cover
  contracts); identity-independence of the binding is machine-checked
  (scripts/identity_independence_audit.py).

## Lemma 0 — The committed-state invariant (boundary theorem)

ASSUMPTION (execution boundary, named and required): a failed call commits
nothing — the caller aborts on Err and the runtime discards all mutations.
This is the actual semantics of the intended execution environment.

Under that assumption: **every committed engine state satisfies the global
validity predicate** — `validate_shape`'s content (senior cover
`c_tot + insurance + earnings + counterparty_backing_principal <= vault`,
exact O(1) aggregate totals == per-domain sums, per-domain ledger closure,
per-status bucket shapes) plus, on value-moving paths, a balanced typed
`TokenValueFlowProofV16`.

Machine check: `scripts/boundary_audit.py` verifies that ALL 55 public
`*_not_atomic` entrypoints terminate their Ok path in (or transitively
delegate to) one of the engine's state validators — so an Ok return cannot
exist without the validators having passed, and an Err return commits
nothing. `GlobalValidState` is therefore not a per-op proof obligation: it
holds at every commit by construction, and the validators' SEMANTICS are
themselves Kani-proven (aggregate-scan proofs, ledger-parts closure layer,
senior-cover gate contracts).

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
* suite: proof_v16_frame_* — 12 ops verified: deposit, withdraw, fee charge,
  domain-insurance deposit/withdraw, budget credit, provider-earnings
  withdraw, counterparty-backing deposit/withdraw (incl. the discovered
  risk_epoch/credit_epoch recompute deltas, now part of the declared
  contract), resolve_market, mark-drain-only, plus the Err-frame template.
  Remaining tractable ops (oracle updates, side reset, restart, crank,
  insurance->account credit) extend by the same mechanical template.
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
   Lemmas 0/1/2/3/5/6 via their Ok-exit validators, gates, value skeletons,
   components, and fuzz — but have no monolithic frame proof (elimination
   table, src/v16_proofs.rs). The residual unproven risk class is a
   cross-account frame violation INSIDE a transition that still satisfies
   every validator and flow proof — bounded by the order-independence /
   extraction-bound / double-claim fuzz, not frame-proven.
2. #37 maker exemption: gates + components proven; full path intractable.
3. No-DoS as a universal constructive theorem (every actionable state has a
   SUCCESSFUL continuation): the cheap classes are witnessed (stale →
   refresh succeeds; same-slot crank frame; empty-market clock progress) and
   close lifetime is bounded; the success theorems THROUGH liquidation/close
   bodies are intractable-tier. The engine's liveness claim is
   class-conditional, NOW WITH MACHINE-PROVEN RANK COMPONENTS (the
   kernel-proofs branch):

   | progress class | rank artifact (production kernel, full-domain proven) |
   |---|---|
   | B settlement | kernel_advance_leg_b_snap: b_snap advances by exactly delta_b — distance-to-target strictly decreases per successful chunk |
   | close progress | kernel_advance_close_ledger: residual_remaining decreases by exactly the booked total; finalization == residual exhaustion; immutables frozen |
   | trade finalization | kernel_initial_margin_gate (EXACT total decision: Ok <=> valid cert + equity >= IM) and kernel_locked_margin_gate (positive credit can never satisfy IM under h-lock) |
   | leg mutations | kernel_resize_leg_same_side / kernel_attach_leg / kernel_clear_leg: the complete leg stage family of trade/liquidation/rebalance, exact deltas + complete frames |

   Each rank kernel is real production code (the monoliths call them), so a
   successful body execution NECESSARILY decreases its class's rank by the
   proven amount — the composed liveness argument needs only the (gate-proven)
   reachability of a successful call per class.
4. Anything outside the pure engine (out of scope by project decision).

## Companion documents (same branch, same boundary)
- scripts/kernel-branch-certification.md — 273/273 fresh branch certification.
- scripts/no-dos-liveness.md — ActionableState -> bounded successful
  continuation, with the machine-proven rank steps and the named scheduler
  assumption.
- scripts/spec-coverage.md — 37 STRONG / 1 N/A; the two static audits
  (boundary_audit.py 55/55, identity_independence_audit.py) cited inline.
- scripts/boundary_audit.py, scripts/identity_independence_audit.py —
  executable static checks for Lemma 0 (Ok-exit GlobalValidState) and #3
  (identity independence).

All four describe one boundary: machine-proven safety lemmas + GlobalValidState
at every committed state (under Err-full-revert) + the leg/B/close/margin
production kernels; the genuinely-open frontier is identical everywhere —
the single composed transition theorem and exact frames over the intractable
monolithic bodies, which are seven-way-eliminated and validator+fuzz
backstopped, not pretended closed.
