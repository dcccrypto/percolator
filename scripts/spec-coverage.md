# Spec §0 requirement → verification-artifact coverage matrix

Generated 2026-06-11 (engine @ spec v16.8.11). Artifact classes:
- **suite** — tests/proofs_v16.rs Kani proof (constructed-state, isolated ≤900s)
- **contract** — verified function contract (contracts layer, scripts/contracts_runner.sh)
- **closure** — inductive-closure proof (closure layer: any-state + assume(inv) → op → assert inv)
- **flow** — verified TokenValueFlowProofV16 transit witness (+ runtime validate() on every execution)
- **fuzz** — proptest property (tests/backing_double_claim_fuzz.rs et al.)
- **runtime** — engine validate_* fail-closed enforcement at execution time
- **structural** — holds by construction/code shape; argued, not machine-proven

| # | Requirement (short) | Coverage | Primary artifacts | Gap action |
|---|---|---|---|---|
| 1 | Full shared solvency | STRONG | suite margin/health proofs; contract lag-penalty; support weight 1.0 pinned as a compile-time constant (support_weight_is_constant_one) — cross-leg support is unconditional by construction | — |
| 2 | Source-domain realizability cap | STRONG | contract availability-cap (exact formula); suite realize gates; full-range fuzz | — |
| 3 | No identity assumptions | STRONG | scripts/identity_independence_audit.py (machine check): all 6 `.owner` reads are self-consistency binding or serialization plumbing; NO cross-account identity comparison; no economic decision branches on owner | — |
| 4 | Instance boundary absolute | STRONG | suite provenance/market-group-id rejections; runtime validate_with_market | — |
| 5 | No global B pool | STRONG | STRUCTURALLY EXACT: the market header carries NO global B value field (B residual exists only as per-asset-slot b_long_num/b_short_num); suite residual-booking proofs (bounded+exact to the loss-bearing side); bankruptcy-capacity proof | — |
| 6 | Protected principal senior | STRONG | runtime aggregate-totals check (c_tot+I+earnings+cbp ≤ vault); junior-pool lattice (every public op); contract earnings senior-coverage gate | — |
| 7 | Fully-backed rate = 1.0 | STRONG | suite credit-rate proofs | — |
| 8 | Oracle containment (haircut/impair) | STRONG | suite impair/expiry/drain-only; closure impair | — |
| 9 | Credit liens for durable use | STRONG | suite lien-gate + grant-gate proofs | — |
| 10 | No double use of credit/insurance | STRONG | closure layer (all 12 deltas); double-claim fuzz; Finding-G regressions | — |
| 11 | Insurance lien lifecycle exactly-once | STRONG | contracts ins create/release/impair/terminal; closure ins family | — |
| 12 | Cures counted once | STRONG | flow support_to_account_capital (credit == exactly 3 sources); suite partition-equation + cure-count proofs | — |
| 13 | Flow-proof conservation mandatory | STRONG | all 11 flow transit witnesses + runtime validate() every execution | — |
| 14 | Rounding residue explicit sink | STRONG | direction fuzz (rounding_residue_fuzz: fee CEILS against user, margin exact-floor with ceiled-notional composition, notional floor≤exact≤ceil, ADL rounds toward zero, support floors against claimant); exact-split Kani proofs (fee_split, utilization exact-floor); end-to-end residue via close/sequence conservation fuzz | — |
| 15 | No open unbacked loss curing | STRONG | suite realize/consume gates (cure requires lien consume + face burn) | — |
| 16 | Stale backing fails closed | STRONG | suite expiry proofs + expiry-liveness regression | — |
| 17 | Claim bounds never understate | STRONG | suite bound-refine proofs; contract claim-bound grant | — |
| 18 | Deterministic credit rates | STRONG | DIFFERENTIAL fuzz (engine_rate_matches_spec_formula, 4000 cases: engine == independent reimplementation of min(floor(avail·S/bound), S)); rate-bound suite proofs; kink-schedule exactness proofs; recompute epoch proofs; rates are pure functions of state (no caller parameter — structural) | — |
| 19 | Pending obligations survive exit | STRONG | suite withdraw-rejects-while-close-active witness; close-ledger validation; cancel/cure gates | — |
| 20 | Single-sided penalty accounting | STRONG | BY SIGNATURE: requirement functions take only (notional, bps, floor, lag) — lien/obligation penalties CANNOT enter the requirement side and appear only as equity/support deductions (suite impair/support proofs); the one requirement-side penalty (lag) is contract-proven single-sided (uniform add, equity untouched) | — |
| 21 | Exclusive close ownership (v16.9.0) | STRONG | suite close-exclusion proofs: occupied-domain begin rejects pre-mutation, one close per account, monotone close_id identity, bounded lifetime. FINDING: the spec's ClosePriority preemption tuple is NOT implemented — the engine uses exclusive per-domain barriers + bounded lifetime, which forecloses hold-and-wait and livelock by construction (each close holds exactly one domain; contention rejects, never compares). Spec reconciled in v16.9.0 (req 21 rewritten to the proven exclusive-barrier mechanism). | — |
| 22 | Immutable close lifecycle | STRONG | suite residual-equation + ledger validation proofs | — |
| 23 | Bounded close lifetime (v16.9.0) | STRONG | drift_consumed is a validated partition category with NO writer — the drift-reserve mechanism is not implemented; close lifetime bounded via max_close_slot (proven); spec reconciled in v16.9.0 — drift_consumed is a reserved always-zero partition category | — |
| 24 | Residual durability before clear | STRONG | suite dropped-residual cancel-shape rejection; residual-equation proof; close gates; terminal realization proofs | — |
| 25 | ADL/finalization atomicity | STRONG (engine boundary) | the finalization barrier IS the proven close exclusivity (occupied-domain rejection, per-account exclusion); quantity_adl_applied_q is a validated close-ledger field (a canceled ledger with progress — incl. ADL — is rejected: dropped-residual proof); within one engine call the sequence is atomic by construction (no intermediate observable state) | — |
| 26 | No fee seniority | STRONG | suite inductive fee proof (never debits insurance); fee contracts | — |
| 27 | Deterministic residual attribution | STRONG | close_order_does_not_redistribute fuzz (full backing range); per-op determinism structural | — |
| 28 | No arbitrary correlation trust | N/A | hedge credit not implemented | — |
| 29 | Asset lifecycle fail-closed | STRONG | suite activation/retire/restart/reactivation proofs | — |
| 30 | Dead-leg exit | STRONG | suite forfeit proofs (typed flow, v16.8.10) | — |
| 31 | Recovery fallback pricing reserved (v16.9.0) | STRONG (as specified) | config knobs (deviation bps, enable flags) exist and are bound-validated, but NO fallback price computation uses them — spec reconciled in v16.9.0: mechanism RESERVED, knobs validated-unused, recovery accounting-neutrality proven; the envelope text is retained as the normative bar for any future implementation | — |
| 32 | Hints discovery-only | STRONG | suite account-validation proofs (full-bitmap equality) | — |
| 33 | Refresh bounded by N | STRONG | every Kani proof that executes a refresh path under #[kani::unwind(40)] IS a machine-checked termination bound (unwinding assertions fail otherwise); loop bounds are struct constants | — |
| 34 | No full-market atomic work | STRONG | the 16 exact-frame proofs prove public ops change ONLY the named header fields, the named asset slot, and the hint account — no other account or slot is touched, machine-checked byte-for-byte | — |
| 35 | Crank-forward markets | STRONG | suite permissionless-crank proofs | — |
| 36 | Canonical per-asset leg | STRONG | suite duplicate-asset/domain rejections | — |
| 37 | Maker exemption bounded | STRONG (decision core) | kernel_initial_margin_gate: EXACT total decision contract (Ok <=> valid cert + certified equity covers IM); kernel_locked_margin_gate: positive credit never satisfies IM under h-lock (full domain); both are production code called by trade finalization. The monolithic path composition stays with the documented elimination + flow/frame/validator backstops | — |

## Outstanding items (engine boundary)
Spec v16.9.0 reconciled all former spec-ahead-of-engine items; the matrix has no open
divergences and no GAP rows at the engine boundary.

The remaining floor, all documented-by-experiment rather than open:
- #37 maker exemption: gates + components proven; the full trade path is in the
  intractable tier (seven-way elimination, src/v16_proofs.rs).
- #3 no identity assumptions: absence-of-reads is a code-review fact (STRUCTURAL).
- Exact frames for the intractable bodies: backstopped by gates + value skeletons +
  runtime flow validation on every execution.

Bottom line: 37 STRONG (incl. #3 via identity-independence machine check and #37 decision core via the kernel-proofs gates), 1 N/A (#28). Machine-checked static audits: scripts/boundary_audit.py (55/55 Ok-exit validators) and scripts/identity_independence_audit.py.
Exact-frame lattice: 16 ops (scripts/no-steal-theorem.md Lemma 4). Differential rate
fuzz + constant-weight witness close #18/#1. no-DoS liveness: ActionableState ->
bounded-successful-continuation composition with machine-proven rank steps in
scripts/no-dos-liveness.md (rank kernels proven; gate-reachability validator+fuzz
backstopped; continuation submission is the named external scheduler assumption).
