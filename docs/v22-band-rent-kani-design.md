# v2.2 band + rent: Kani design (DESIGN ONLY, not run)

Scope: Phase 4 items 1 (per-epoch price band) and 2 (holding-fee rent), engine branch
`feat/v22-band-rent`, wrapper branch `feat/v22-wave-b`. Design source:
`~/percolator-ops/ledger/phase4-design-2026-10-05.md` §0.3, §1.4, §2.4.

Method (unchanged from §0.3, and the standing rule "design + review first, run ONCE on the
final code"):

- Every harness calls the **real production function**, never a re-implementation.
- u128 multiply/divide does not verify in Kani. Each target is proved on **u8/u16/u32
  symbolic inputs cast up** (bounded), with small-domain exhaustive twins where the bound is
  the whole claim. The same function is proptested **at full width** in
  `tests/v22_band_rent.rs` (already green). Neither alone is the claim.
- Every harness has `kani::cover!` properties for each branch it is meant to reach. A
  SUCCESSFUL run with an unsatisfied cover is a failure (vacuity rule).
- Each harness lists its mutants. Mutants are applied by file copy (never `git stash`); the
  mutant runs already performed against the tests are recorded at the end.

## 1. Pure arithmetic (`src/band_rent.rs`)

| harness | real function | bounds | asserts | covers |
|---|---|---|---|---|
| `kani_band_bounds_inward_contain_anchor` | `band_bounds` | `anchor: u32 as u64` (>=1), `d: u16 <= 2000` (>=1) | `1 <= lo <= anchor <= hi <= MAX_ORACLE_PRICE`; `lo*1e4 >= anchor*(1e4-d)`; `hi*1e4 <= anchor*(1e4+d)`; `Err` iff out-of-domain | `lo == anchor` (tiny anchor), `lo < anchor`, `hi == anchor`, `hi > anchor` |
| `kani_band_clamp_in_range` (design §1.4) | `clamp_to_band` | `anchor, price: u32`, `d: u16 <= 2000` | result in `[lo, hi]`; idempotent; `price` in band => identity; `price > hi` => `hi`; `price < lo` => `lo`; never overshoots toward `price` | each of the three regions |
| `kani_band_G_upper_bound` (I-B6) | `band_worst_adverse_bps`, `band_bounds` | **exhaustive** `d in [1, 300]`; `A, A', p1, p3: u16` with `A' in band(A)`, `p1 in band(A)`, `p3 in band(A')` | `|p3 - p1| * 1e4 <= G * p1` (both directions) | upward worst case, downward worst case, `A' == A` |
| `kani_band_d1_step` | `band_d1_pinned_step` | `current, step, anchor: u32`, `d: u16`, `pinned: bool` | pinned => `(current, step != current)`; else `(clamp(step), clamp(step) != step)` | pinned+moved, pinned+unchanged, edge-clamped up, edge-clamped down, pass-through |
| `kani_band_duration_pinned` | `band_duration_pinned` | `u32` slots | `true` iff `end - anchor > E` (saturating) | true, false, `end < anchor` |
| `kani_rent_due_with_carry_exact` | `rent_due_with_carry` | `abs_q: u16`, `index, snap: u32` (snap <= index), `carry < 1e15` as u64 | `due * D + carry' == abs_q*(index-snap) + carry`; `carry' < D`; `snap > index` => `Err` | due 0, due > 0, carry' 0, carry' > 0 |
| `kani_rent_split_never_overcharges` (I-R4) | `rent_due_with_carry` twice vs once | `abs_q: u8`, deltas `u16` | `due(r0->r1) + due(r1->r2)` with carried remainder `== due(r0->r2)` (exact with carry) | both splits nonzero |
| `kani_rent_chargeable_junior` (I-R7) | `rent_chargeable_atoms` | `due, capital: u32`, `pnl: i32` | `<= due`, `<= capital`; `pnl < 0` => `capital - charged >= min(|pnl|, capital)` | pnl < 0 with charge > 0, pnl < 0 fully owned (charge 0), pnl >= 0 |
| `kani_rent_index_delta_failclosed` (I-R2) | `rent_index_delta` | `u32` inputs (exact) and a `u64::MAX` symbolic twin | `Ok(v)` => `v == p*r*dt`; overflow => `Err` | Ok, Err |

## 2. Engine kernels (`src/v16.rs`, `V16Core`)

| harness | real function | asserts | covers |
|---|---|---|---|
| `kani_band_attach_detach_roundtrip` | `kernel_band_attach`, `kernel_band_detach` | attach then detach of the same (uncertified) leg is identity on the counters; `band_epoch == 0` leaves counters untouched (I-B7); detach of a leg with `snap > e` is `Err` | band on / off, liq_pending leg detach |
| `kani_band_certify_leg` | `kernel_band_certify_leg` | healthy: `snap' == e`, uncertified decremented once iff `snap < e`, liq_pending cleared once; unhealthy: liq_pending set once (idempotent); `band_epoch == 0` => identity | every arm, incl. healthy-and-already-certified (no double decrement) |
| `kani_band_reanchor_iff_certified` (I-B3) | `kernel_band_reanchor_ready`, `kernel_band_reanchor` | ready iff all four counters 0 and both barriers 0; reanchor => `A' == effective_price`, `e' == e+1`, uncertified' == stored counts, pin clock 0; `kernel_band_reanchor` on a not-ready asset is `Err` (defense in depth) | ready, each of the six not-ready causes |
| `kani_band_census_inductive` (I-B2, design §1.4) | the real kernels above + `kernel_attach_leg`, `kernel_clear_leg` | **inductive**: a 3-leg model (each leg: active, side, snap <= e, liq flag) and an `AssetStateV16` whose counters equal the census; apply ONE nondeterministic transition from {attach, clear, certify healthy, certify unhealthy, reanchor-if-ready, liq-partial (certify healthy or unhealthy), trade-grow (certify healthy)}; assert the census still equals the counters and `snap <= e` | each transition taken; reanchor reachable |
| `kani_band_accrue_rejects_before_mutation` (I-B1, spec test 10) | `accrue_asset_to_with_rent_not_atomic` on a 1-asset fixture | out-of-band price => `Err(BandOutOfRange)` and the asset slot / header bytes unchanged; duration-pinned moving accrual => `Err(BandPinned)`, bytes unchanged | both refusals; an accepted in-band accrual |
| `kani_band_unarmed_fails_closed` | same | `band_bps != 0 && band_epoch == 0` => `Err(InvalidConfig)` | — |
| `kani_rent_route_conserves` (I-R3) | `rent_route_delta` | `dI == -x`, `dC == +x`, `dCap == +x`, `x <= rent_unrouted`, `x <= insurance - reserved`; `next_claim <= rent_unrouted - x` and `<= next_insurance` | full route, partial route (claim kept), zero route |
| `kani_rent_settle_idempotent_and_attach_snap` (I-R4, I-R5) | `settle_leg_rent_not_atomic` via a kani shim on a 1-leg fixture | second settle at the same index charges 0; a freshly attached leg (`kernel_attach_leg`) owes 0 at the attach index | waiver branch (pnl < 0, capital fully loss-owned), cap-at-capital branch |
| `kani_bsl_validator_matches_pointwise` | `validate_band_safety_law` vs `solvency_envelope_holds_for_notional` | for `N: u16`, config fields bounded (u8/u16): validator `Ok` => pointwise law holds at `N` | validator Ok and Err both reachable |

## 3. The inductive "loss <= capital" harness (I-B5)

This is the claim the band exists for. It is too wide to verify end to end on the engine, so
it is split, per §0.3, into a **bounded inductive lemma on real primitives** plus the
**full-width adversarial proptest** (`band_malicious_mark_never_creates_bad_debt`, green:
240 runs, 1,485 liquidations, 4,509 re-anchors, 0 bad debt).

### 3.1 Abstraction (one leg, one account)

State `S = (e, A, A_prev, p_cert, e_cert, eq_cert, N_cert, P_last, t_used, liq_pending)`:
the band epoch and anchors, the price / epoch / equity / notional at the leg's last
C-event, the current price, the loss-accruing slots used since the C-event, and the flag.

Transitions (each mirrors a real engine path; the harness calls the real arithmetic):

1. `accrue(p, dt, f, r)`: requires `p in band(A)` (the real `price_in_band`) and, if the
   window elapsed (the real `band_duration_pinned`), `p == P_last && f == 0 && r == 0`.
   Equity moves by `q * (p - P_last)` minus funding and rent over `dt`.
2. `reanchor`: only if the real `kernel_band_reanchor_ready` holds for the abstract
   counters (this leg certified in `e` or detached, not liq_pending): `A_prev := A`,
   `A := P_last`, `e += 1`, leg becomes uncertified.
3. `certify`: requires `eq >= MM(N)` at `P_last` (the real `margin_requirement`): resets
   `(p_cert, e_cert, eq_cert, N_cert, t_used)`.
4. `mark_liq_pending`: requires `eq < MM(N)`.
5. `liquidate`: closes the leg at `P_last`.

### 3.2 Invariant (inductive)

```text
INV:  e_cert in {e, e-1}                                          (two-epoch window)
   && p_cert in band(A_prev) U band(A)   (A_prev = the anchor of e_cert's epoch)
   && every price since the C-event in band(A_prev) U band(A)
   && t_used <= 2E
   && eq_cert >= MM(N_cert)
```

`INV => eq_now >= liq_fee(N_now)` is the lemma `kani_bsl_no_bankruptcy_lemma` (design
§1.4): with `|P_last - p_cert| * 1e4 <= G * p_cert` (I-B6, proved above) and funding + rent
`<= (f_max + r_max) * 2E` per notional, the BSL inequality validated at InitMarket gives
`eq_now >= liq_fee`. Hence the liquidation's `D = max(-PNL, 0) == 0`.

Induction step obligations (one harness each, symbolic `S` satisfying `INV`, one
transition, assert `INV'`):

- `kani_inv_accrue`: band membership and `t_used' = t_used + dt` with
  `t_used' <= 2E` because an epoch's moving accruals end before `anchor_slot + E` and the
  leg's window spans at most the rest of `e_cert`'s epoch plus one more (option B: the
  re-anchoring accrual is governed by the old window, so no interval is double counted).
- `kani_inv_reanchor`: needs `e_cert == e` (the leg is certified in the closing epoch or
  it blocks), so after `e += 1` we have `e_cert == e-1`; `A' = P_last in band(A)`.
- `kani_inv_certify`: trivially re-establishes `INV` at the new C-event.
- `kani_inv_no_third_epoch`: from `e_cert == e-1`, `reanchor` is impossible unless the leg
  certifies (then `e_cert == e`) or detaches. This is exactly the uncertified-cohort rule;
  the liquidation-pending cohort is not needed for it (see §5, mutant M1).

Bounds: prices `u16`, `q: u8`, `d in [1, 300]`, `E in [1, 64]`, MMR / fee / funding / rent
in `u8`; the BSL instance is generated by the real validator on the same bounded config, so
the harness only explores configs the program would accept. Covers: a liquidation with the
equity exactly at `liq_fee` (tightness), a re-anchor, a duration pin, an edge clamp.

### 3.3 What is NOT claimed

- Multi-asset portfolios: band markets are single-asset by construction
  (`max_market_slots == 1 && max_portfolio_assets == 1`, validated); the law does not extend.
- Oracle correctness: a malicious mark can move value between traders and the LP at up to
  `d` per certified epoch; it cannot create bad debt (design §1.5).
- Equity reductions that carry no health check are budgeted: funding and rent (in the BSL
  rate term) and the wrapper maintenance fee (design §1.1; every other debit either
  re-certifies or requires IM). Trades always end in a fresh certificate (C-event or
  liq-pending), so trading fees on a reduce are covered by the next C-event.

## 4. Wrapper (`percolator-prog/src/growth_v19.rs`)

| harness | real function | bounds | asserts | covers |
|---|---|---|---|---|
| `kani_rent_rate_kink_cap_monotone` (I-R1) | `rent_rate_e9` | `users, n_cap: u32`, `kink: u16 <= 1e4`, `max: u16` | `u <= kink` => 0; `<= max`; monotone in `users`; `users >= n_cap` => `max`; `n_cap == 0` => `None` | each region |
| `kani_band_lambda_max` | `band_lambda_max_bps` | `mmr, g: u16` | `<= MAX_LAMBDA_BPS`; `lambda_max * (mmr + G) <= 1e4 * 9500` | capped, uncapped |
| `kani_graduation_requires_band_and_depth` | `graduation_allowed` | all inputs | true iff `band != 0 && tier >= 1` | — |
| `kani_c_launch_floor` (I-1) | `c_launch_atoms_for` | `u128` symbolic (linear) | `>= MIN_C_LAUNCH_ATOMS`, `>= c_m` | both arms |

## 5. Mutants (negative controls), applied by file copy; results against the tests

| id | mutant | red tests |
|---|---|---|
| M1 | re-anchor without the `liq_pending == 0` check | `band_liq_pending_blocks_advance_even_after_the_leg_was_certified` |
| M2 | re-anchor without the `uncertified == 0` check | re-anchor, malicious-mark (bad debt), pin, pin-expiry tests |
| M3 | `G` computed as `2d` | G exhaustive + full-width, design-table test |
| M4 | no duration pin | `band_duration_pin_forces_no_move_accruals` |
| M5 | C-event on an unhealthy refresh | `band_liq_pending_blocks_advance_until_liquidated_then_no_bad_debt` |
| M6 | detach without the counter decrement | malicious-mark census |
| R1 | rent snap not reset at attach | `rent_new_leg_pays_nothing_for_prior_time` |
| R2 | routing without updating `rent_unrouted` | `rent_accrues_settles_floor_exact_routes_and_conserves` |
| R3 | ceil instead of floor on due | floor-exact, split proptest, lock-out |
| R4 | drop the sub-atom carry | `rent_hedged_lockout_pays_at_least_the_floor_bound` |
| C1-C3 | remove a certification hook (refresh / attach / post-fill delta) | structural + dynamic census, re-anchor, pin tests |

Finding recorded with M1: the liquidation-pending cohort is NOT load-bearing for loss <=
capital (the uncertified cohort already forbids a third epoch for an unhealthy leg). It is
the design's stricter timing rule ("a leg found liquidatable blocks the advance in the same
epoch") and is kept as defense in depth. The Kani induction in §3 deliberately does not rely
on it.

## 6. Run plan (once, on the final code)

`cargo kani --harness 'kani_band_.*|kani_rent_.*|kani_inv_.*|kani_bsl_.*'` in the engine and
`--harness 'kani_rent_rate.*|kani_band_lambda.*|kani_graduation.*|kani_c_launch.*'` in the
wrapper, after review of this design, on the commit that goes to the v2.2 security review.
Every harness must be SUCCESSFUL with every cover satisfied.
