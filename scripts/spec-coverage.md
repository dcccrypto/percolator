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
| 1 | Full shared solvency | PARTIAL | suite margin/health proofs; contract lag-penalty | cross-leg support weight 1.0 is structural |
| 2 | Source-domain realizability cap | STRONG | contract availability-cap (exact formula); suite realize gates; full-range fuzz | — |
| 3 | No identity assumptions | STRUCTURAL | no code path reads identity | code-review item |
| 4 | Instance boundary absolute | STRONG | suite provenance/market-group-id rejections; runtime validate_with_market | — |
| 5 | No global B pool | PARTIAL | suite bankruptcy-residual proofs; per-domain booking spec tests | — |
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
| 18 | Deterministic credit rates | PARTIAL | suite recompute proofs (epoch); rate core div-bearing (excluded) → concrete witnesses + fuzz | accept |
| 19 | Pending obligations survive exit | STRONG | suite withdraw-rejects-while-close-active witness; close-ledger validation; cancel/cure gates | — |
| 20 | Single-sided penalty accounting | PARTIAL | contract lag-penalty (uniform add); suite health proofs | per-check audit item |
| 21 | Preemptible close total order | STRONG (mechanism) + SPEC-DIVERGENCE | suite close-exclusion proofs: occupied-domain begin rejects pre-mutation, one close per account, monotone close_id identity, bounded lifetime. FINDING: the spec's ClosePriority preemption tuple is NOT implemented — the engine uses exclusive per-domain barriers + bounded lifetime, which forecloses hold-and-wait and livelock by construction (each close holds exactly one domain; contention rejects, never compares). Spec text needs reconciling. | spec edit (user decision) |
| 22 | Immutable close lifecycle | STRONG | suite residual-equation + ledger validation proofs | — |
| 23 | Bounded close drift | SPEC-AHEAD-OF-ENGINE | drift_consumed is a validated partition category with NO writer — the drift-reserve mechanism is not implemented; close lifetime is bounded via max_close_slot (proven in the close-identity proof) | spec/engine reconciliation (user decision) |
| 24 | Residual durability before clear | STRONG | suite dropped-residual cancel-shape rejection; residual-equation proof; close gates; terminal realization proofs | — |
| 25 | ADL/finalization atomicity | **GAP (Kani)** | structural single-instruction paths; runtime tests | integration-level only |
| 26 | No fee seniority | STRONG | suite inductive fee proof (never debits insurance); fee contracts | — |
| 27 | Deterministic residual attribution | STRONG | close_order_does_not_redistribute fuzz (full backing range); per-op determinism structural | — |
| 28 | No arbitrary correlation trust | N/A | hedge credit not implemented | — |
| 29 | Asset lifecycle fail-closed | STRONG | suite activation/retire/restart/reactivation proofs | — |
| 30 | Dead-leg exit | STRONG | suite forfeit proofs (typed flow, v16.8.10) | — |
| 31 | Recovery fallback numeric envelope | SPEC-AHEAD-OF-ENGINE | config knobs (deviation bps, enable flags) exist and are bound-validated, but NO fallback price computation uses them — the mechanism is not implemented; recovery-crank accounting-neutrality is proven | spec/engine reconciliation (user decision) |
| 32 | Hints discovery-only | STRONG | suite account-validation proofs (full-bitmap equality) | — |
| 33 | Refresh bounded by N | STRUCTURAL | loop bounds are struct constants | — |
| 34 | No full-market atomic work | STRUCTURAL | per-account instruction shape | code-review item |
| 35 | Crank-forward markets | STRONG | suite permissionless-crank proofs | — |
| 36 | Canonical per-asset leg | STRONG | suite duplicate-asset/domain rejections | — |
| 37 | Maker exemption bounded | PARTIAL | trade-cert component proofs; full path intractable tier | gates + runtime; accept with note |

## Outstanding items
- **#25 ADL/finalization atomicity** — not Kani-expressible; integration/runtime only (permanent).
- **Spec/engine reconciliation (user decision)**: #21 ClosePriority preemption, #23 drift
  reserve, #31 recovery fallback price — all three are spec-described mechanisms with no
  engine implementation; the engine's simpler designs (exclusive barriers + bounded
  lifetime; no drift writer; no fallback pricing) are proven where they exist.

Bottom line: 26 STRONG (incl. #21 mechanism-as-built), 5 PARTIAL (named accept-reasons),
1 GAP (#25, permanent integration-level), 3 SPEC-AHEAD-OF-ENGINE findings flagged,
3 STRUCTURAL, 1 N/A.
