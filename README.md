# Percolator

**EDUCATIONAL RESEARCH PROJECT — NOT PRODUCTION READY. NOT AUDITED. Do NOT use with real funds.**

Current normative spec: [`spec.md`](spec.md), **v16.9.0**.

Percolator is a perpetual-futures risk-engine library for account-local,
permissionless risk progress. v16 keeps the slab-free account model and adds
source-domain realizable credit: positive PnL from one source domain is usable
only up to conservatively proven counterparty or insurance backing for that
domain.

The core promise is narrower and more realistic than global auto-discovery:
if an honest crank supplies a valid account hint, the engine can make bounded
progress on that account, while omitted or stale accounts cannot extract value
or increase risk using optimistic health.

## Three Invariants

1. **Realizable credit:** protected principal is senior, positive PnL is junior, and source-domain positive credit cannot exceed realizable backing reserved for that domain.
2. **Account-local safety:** every favorable action refreshes the account's full active portfolio first; hidden, stale, or B-stale legs fail closed.
3. **Bounded progress:** cranks and recovery paths are account-local and incremental; no public instruction needs to evaluate the whole market.

## Account-Local v16

Each `PortfolioAccountV16` carries provenance:

```text
market_group_id
portfolio_account_id
owner
version/layout discriminator
```

The engine rejects any account whose provenance does not match the
`MarketGroupV16`. Active positions are defined only by the canonical active
bitmap and bounded leg array. There is no hidden slab slot and no global account
table to scan.

The account-local bounded work unit is a full portfolio refresh over at most
`MAX_PORTFOLIO_ASSETS_N` configured legs. A fresh health certificate is required
for user-favorable actions. If an account is stale, B-stale, under h-max/stress,
or loss-stale, favorable paths must reject or use conservative no-positive-credit
lanes.

## H-Lock

Capital is senior. Profit is junior. `h_min` may be zero while the market is
healthy, but h-lock selection is state-derived and permissionless:

- `h_min` is used only when no h-max condition is active.
- `h_max` is used under threshold stress, bankruptcy h-lock, instruction-local
  bankruptcy candidates, loss-stale catchup, stale/B-stale account state, or
  active bankrupt close state.

Wrappers do not choose h-lock from an oracle. They supply authenticated market
inputs; the engine selects the lane from committed market/account state.

## Positive Payouts

Live positive PnL is an ordinary junior lane only while the market group is
`Live` and no resolved payout ledger exists. Once the market resolves, positive
claims move to a single resolved payout ledger: exact account receipts replace
scaled unreceipted bounds, payouts track `paid_effective`, and later bound
refinements can only increase claimable top-ups.

## A/K/F/B

v16 keeps the lazy index model but makes bankruptcy residuals explicit:

- **A** scales effective quantity for side-level quantity ADL.
- **K/F** represent mark and funding settlement.
- **B** books bankruptcy residual loss through account-local chunks.

Account-local B settlement is bounded. A public endpoint must either apply the
engine-determined positive chunk, leave the account B-stale, or route to
permissionless recovery if no positive chunk is representable. B-stale accounts
cannot withdraw, close favorably, convert/release PnL, use hedge credit, increase
risk, or receive resolved payout.

## Crank And Recovery

Public user-fund markets are `CrankForward`. An account-free equity-active crank
is forbidden unless it also commits bounded protective progress. Candidate lists
are hints, not proofs of completeness: missing accounts do not make a crank
unsafe, and hinted unhealthy accounts must either make bounded progress or route
to recovery.

If ordinary bounded progress cannot continue, the public recovery API records a
deterministic recovery reason. The caller does not choose a recovery price.

## Proofs

The current v16 proof suite is intentionally account-local and runs over the
production v16 methods:

```bash
cargo install --locked kani-verifier
cargo kani setup
scripts/run_kani_full_audit.sh
```

The latest checked timing sweep is in:

```text
kani_audit_full.tsv
kani_audit_final.tsv
scripts/proof-strength-audit-results.md
```

The old slab proof inventory was retired with the v16 cutover because it no
longer applies to the architecture.

### No-LoF / No-DoS: what is proven, and under what assumptions

The engine carries a decomposed, machine-checked argument for two safety
properties at the pure-engine boundary. It is a composed proof, not a single
all-transitions theorem (that one query is intractable for this prover
generation; see below).

No loss of funds (no-LoF):

- `GlobalValidState` — `validate_shape` plus per-touched-account
  `validate_with_market` — is preserved at every committed `Ok` exit of all 55
  public `*_not_atomic` entrypoints, checked transitively by
  `scripts/boundary_audit.py` (55/55). `Err` paths fully revert at the execution
  boundary, so they need no preservation.
- Every public entrypoint is mapped to a stronger no-LoF proof source by an
  enforced partition, `scripts/lof_transition_class_roster.py` (10 transition
  classes; build fails on any unclassified entrypoint or missing artifact):
  exact whole-state frames, whole-body frame+value composition (attach/clear),
  production kernel-contract value deltas, typed `TokenValueFlowProofV16`
  validation, and inductive encumbrance/lien closure proofs.
- Value-moving arithmetic is proven via the arithmetic-axiom recipe: the wide
  division helper is abstracted to an opaque spec value inside Kani, and its
  exact form (`ceil(abs * SCALE / a_basis)`) is discharged by differential fuzz
  against an independent reimplementation. Kani never executes wide arithmetic.

No denial of service (no-DoS / liveness):

- `ActionableState` is a 7-class disjunction; every class has a present, named
  machine-checked witness, classified by strength and enforced by
  `scripts/actionable_class_coverage.py`: 2 kernel-existential (a rank-decreasing
  call the proven kernel accepts), 3 public-body-route (drive the real
  production routing/preflight fn), 1 protective-segment, 1 terminal-suite.
- A well-founded lexicographic rank decreases on each continuation; the B-advance
  and close-advance rank steps are machine-proven production kernels.

Assumptions and named boundaries (the trusted base):

- `ArithmeticAxiom` + differential fuzz: the stubbed wide-division helper equals
  its spec; only this narrow, helper-specific arithmetic is assumed, never a
  global arithmetic operator.
- Execution-boundary atomicity: a rejected (`Err`) public call fully reverts.
- External scheduler / fairness: the engine proves a successful bounded
  continuation *exists* for every actionable state; it does not prove an external
  actor *submits* it. Permissionless cranks make every continuation callable by
  any actor.
- Tool-generation limits (not soundness gaps): a single Kani query over all
  public transitions at once, and whole-body value composition for large-interior
  bodies (resize / trade / batch — their value-exactness is proven at the kernel
  contracts), are intractable due to bit-precise wide arithmetic and large-struct
  symbolic state. The rosters above are the sound decomposition.

Full detail: `scripts/no-steal-theorem.md` (no-LoF), `scripts/no-dos-liveness.md`
(no-DoS), and `scripts/proof-frontier-closure.md` (the goal-by-goal index).

## Tests

```bash
cargo test
cargo test --features test
```

## Scope

This repository is a pure risk-engine library. It does not define an on-chain
program id, account decoder, persisted market registry, or deployment manifest.
Wrappers own authorization, account loading, oracle/funding authentication,
fee-schedule policy, and raw-state layout migration.

## License

Apache-2.0.
