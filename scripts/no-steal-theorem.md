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

## Composition-proof experiment (whole-body frame lift) — SUCCEEDED with the two-lever recipe

Whole-body frames CAN be reached by composition. The working recipe, proven on
attach_leg_at_slot (composition_attach_body_frame_division_stubbed, PASS 117s):

```
#[kani::stub_verified(V16Core::kernel_attach_leg)]      // kernel -> its verified contract (no body)
#[kani::stub(loss_weight_for_basis, kani_any_loss_weight)]  // the body's division -> arbitrary nonzero
```

The whole-body frame VERIFIES: attach_leg_at_slot touches ONLY leg[0], the
active bitmap, and the health cert — every other leg and account field frozen.

The path to it (each step empirically established this session):
- direct body frame: TIMEOUT — the body computes loss_weight_for_basis (division);
- stub_verified(kernel) ALONE: TIMEOUT — the body STILL computes the division
  BEFORE the kernel call, so stubbing the kernel alone leaves it in;
- stub(division) ALONE: the contracted kernel called inside a plain proof has
  its own ensures checked at the call and the interaction fails;
- BOTH together: PASS. Stub the verified kernel AND the division primitive the
  kernel was built to exclude. Soundness: the division stub returns an
  arbitrary value, which is sound for a FRAME property (the frame asserts WHERE
  the weight lands, not its value); the value-exact part is the separately
  verified kernel contract. (NOT sound for a value/conservation claim — those
  keep the real division or stay in the documented intractable tier.)

Scope: this is one whole-body frame (attach). The recipe generalizes to any
body that is gates + a contracted kernel + a division-bearing weight input;
each target needs its own composition proof. Bodies with MULTIPLE interacting
division sites or no clean kernel seam remain in the intractable tier. But the
"monolithic-body frames are unreachable" claim is RETRACTED: they are reachable
by stub_verified + division-stub composition where a kernel seam exists.

## Division-axiom route (the review's proposal) — sound, with a precise limit

The route: replace the wide-division helper with an EXACT SPECIFICATION AXIOM
(kani::any() result + assume(ceil relation) — no division circuit), prove the
engine composition under it, and discharge `production == axiom` by fuzz.

WHAT WORKS (sound, machine-checked):
- DISCHARGE: loss_weight_helper_matches_division_axiom (tests/rounding_residue_
  fuzz.rs, 20k cases + rounding/denominator edges) proves the production helper
  loss_weight_for_basis EQUALS ceil(abs*SOCIAL_WEIGHT_SCALE / a_basis) over the
  full real input ranges. The narrow empirical obligation the review specifies
  is real and green.
- FRAME composition under the route (attach, clear): machine-checked.

THE PRECISE LIMIT (why the VALUE composition is not a single Kani query):
- Under the spec-EXACT axiom, the ceil relation is a wide MULTIPLICATION
  (q*a_basis with both ~2^50 -> ~2^100 product). Even the axiom's own
  self-consistency at engine ranges ran 28 min (effectively intractable), and
  a bounded-width version with a symbolic u128 division ran 8+ min — because
  CBMC's u128 division/multiplication circuits are structurally 128-bit and do
  NOT collapse under operand-magnitude bounds.
- DEEPEST FINDING: the intractable wall is bit-precise WIDE ARITHMETIC (division
  AND multiplication) at 2^50+ operand widths, essentially independent of value
  bounds — not division alone, and not account-state size.

THE SOUND REALIZATION: the trusted base DOES move to "DivisionAxiom +
differential fuzz" exactly as the review specifies. The "engine under the
axiom" step is the LOGICAL composition of the separately-proven per-stage
kernel contracts (in the 273 cert) with the fuzz-discharged axiom — a sound
transitive argument, NOT one mechanized query. No single Kani query over wide
symbolic arithmetic fits; that is a prover limit, not a soundness gap. The
boundary is materially better than before (small, named, fuzz-validated
arithmetic assumption) and is the most this prover generation supports.

## Division contracts and reduced-leg profiles — both conclusively negative

Two levers tested this session to crack division-bearing intractable bodies;
both fail, and the reasons are now precise.

DIVISION FUNCTION CONTRACTS — cannot verify the leaf at all. CBMC has no axiom
for division; it only has the bit-level long-division circuit. Verifying ANY
contract about a wide-integer division forces a one-time symbolic execution of
that circuit. Tested loss_weight_for_basis = ceil(abs*SOCIAL_WEIGHT_SCALE /
a_basis) with a multiplication-form ensures (no division in the spec) and
operands bounded to their REAL ranges (a_basis in [1e14, 1e15], abs <= 1e14):
  - unwind 140: TIMEOUT 1800s
  - unwind 40 (anti-inflation per the div_rem unwind lesson): TIMEOUT 1800s
Plus the earlier full-width and U256-nested attempts. Conclusion: the one-time
bit-blast of bit-precise wide division is SAT-hard even isolated and bounded —
not a spec-expressibility problem (the mult-form bounded contract is the exact
right artifact; it just doesn't fit the solver). So "verify the division leaf
once, stub_verified everywhere" is NOT available. The rate/weight cores stay
covered by differential fuzz (engine == independent reimplementation) and
exact suite proofs with concrete operands.

REDUCED-LEG PROFILES — don't help division-bearing bodies. cfg(kani)
V16_MAX_PORTFOLIO_ASSETS_N = 2 makes the account-state cost tiny, but the
direct attach+clear body frame still TIMEOUT 1800s at 2 legs, because the wall
is the loss_weight_for_basis DIVISION in the attach path, which is independent
of leg count. Reduced legs would only help an account-state-bound, DIVISION-
FREE body — for which the 16-leg frames already pass (deposit/withdraw etc.).
So the leg profile buys nothing for the intractable tier and was reverted.

NET: the intractable tier is intractable because of WIDE SYMBOLIC DIVISION in
the body, not account-state size and not contract coverage. The composition
recipe (stub_verified kernel + stub the frame-irrelevant division) is the only
lever that reaches these whole-body FRAMES, and only because a frame doesn't
depend on the division's value. Value/conservation theorems over division-
bearing bodies remain out, with differential fuzz as the documented substitute.

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
