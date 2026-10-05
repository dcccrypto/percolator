# P2b bounded lock exits: Kani design (L1 hlock clear predicate, L2 ADL wind-down conservation)

Status: DESIGN ONLY. Nothing here has been run. Per the project rule, Kani runs once, locally,
on the final code, after design review (`feedback_kani_design_first_run_once`). Every harness
below must report its `kani::cover!` properties as SATISFIED; a SUCCESSFUL verdict with an
unsatisfied cover proves nothing (`kani_vacuity_detector_cover_properties`).

Code under proof (engine `feat/p2b-lock-exits`):

| item | function | file |
|---|---|---|
| wire encoding | `bankruptcy_hlock_mark_domain`, `bankruptcy_hlock_mark_unattributed`, `bankruptcy_hlock_domain_mask`, `validate_bankruptcy_hlock_wire` | `src/v16.rs` (free fns before `trade_preflight_risk_gate`) |
| attribution | `mark_bankruptcy_hlock_event`, `mark_bankruptcy_hlock_for_account`, `bankruptcy_hlock_asset_is_sole_ever_activated` | `src/v16.rs` (next to `try_clear_bankruptcy_hlock_if_healthy`) |
| clear predicate | `bankruptcy_hlock_claim_term_clear`, `try_clear_bankruptcy_hlock_if_healthy` | same |
| wind-down | `adl_wind_down_eligible`, `wind_down_adl_position_not_atomic` | `src/v16.rs` (after `rebalance_reduce_position_not_atomic`) |
| episode storage (wrapper) | `adl_episode_step`, `handle_adl_wind_down`, `handle_set_adl_wind_down_max_slots`, tag-93 preservation | percolator-prog `src/v16_program.rs` (Part C) |

All harnesses go in `tests/proofs_v16.rs` (engine `#[cfg(kani)]` test file) and are added to
`kani-list.json`. Fixtures reuse `one_market_view_fixture()` / `one_market_only_fixture()`.

---

## Part A. L1, the hlock clear predicate

### A1 `proof_p2b_hlock_wire_encoding_is_total_sticky_and_legacy_safe` (pure, cheap)

Symbolic `wire: u8`, `domain: usize` (assume `domain < 16`).

- `m = mark_domain(wire, domain)`: `m != 0` and `m & 1 == 1` (marked always active).
- Monotone: if `wire` is attributed-valid (`wire & 1 == 1`) then `mask(m) ⊇ mask(wire)` or
  `m == 1`.
- Legacy-safe: `mark_domain(1, d) == 1` for all `d`; `mark_unattributed(w) == 1` for all `w`.
- `domain >= 7 ⇒ m == 1`.
- `validate_bankruptcy_hlock_wire(m, D)` is `Ok` whenever `domain < D` (or `m == 1`).
- covers: from 0 to an attributed value; attributed → wider attributed; attributed → 1;
  domain 7 → 1.

### A2 `proof_p2b_hlock_cannot_clear_while_an_attributed_domain_has_claims` (THE safety theorem)

The property the task asks for: the lock cannot clear while the bankrupt domain still has
unresolved claims.

State: one-asset view fixture with `max_market_slots` symbolic in `{1, 14}` (the deployed shape)
and only asset 0 active. Symbolic:
`wire: u8` (assume `validate_bankruptcy_hlock_wire(wire, domains).is_ok()` and `wire != 0`);
`claims_long_bound, claims_long_exact, claims_short_bound, claims_short_exact: u128`
(assume `exact <= bound`), `pnl_pos_tot: u128` (assume consistent: `pnl_pos_tot == 0 ⇔ all four
are 0`, and `pnl_pos_tot >= max(bound)/BOUND_SCALE`), every other health counter symbolic,
`mode` symbolic in `{Live, Resolved, Recovery}`, barriers symbolic.

Call `try_clear_bankruptcy_hlock_if_healthy`. Assert:

```
cleared  ⇒  neg == 0 ∧ stale_cert == 0 ∧ b_stale == 0 ∧ recovery_reason == None
            ∧ barrier_long == 0 ∧ barrier_short == 0
            ∧ ( wire == 1 ∨ mode != Live  ⇒  pnl_pos_tot == 0 )
            ∧ ( ∀ d ∈ mask(wire), d < 2 :  bound_d == 0 ∧ exact_d == 0 )
¬cleared ⇒  header byte unchanged
```

and that the function never changes any field other than the hlock byte (compare a snapshot of
the header and slot with the byte masked).

Covers: cleared with `pnl_pos_tot > 0` (the narrowing is live, not vacuous); held with only an
attributed domain's claim nonzero; held with legacy `wire == 1` and the attributed domain empty;
held in Resolved with the attributed domain empty; cleared with `wire == 0b111` only when both
domains are empty.

Solver note: everything is header/slot scalar reads; no loops beyond the 7-bit mask
(`#[kani::unwind(9)]`). The u128 claim fields only feed `!= 0` tests, so no arithmetic lemmas
are needed.

### A3 `proof_p2b_attribution_is_written_only_for_a_sole_activated_live_asset`

Symbolic `max_market_slots ∈ {1, 2, 3}`, per-slot `lifecycle` and `market_id`, `mode`,
`asset_index`, `bankrupt_side`, prior `wire`. Call `mark_bankruptcy_hlock_event`.

```
attributed write (new wire has bit 1+d for d = 2·asset + side, d < 7, and wire != 1 before)
  ⇒ mode == Live ∧ ∀ j ≠ asset_index : lifecycle_j == Disabled ∧ market_id_j == 0
otherwise ⇒ new wire == 1
```

Covers: attributed in the 14-slot single-active shape (bound symbolic slots to 3 for cost, with
slots 1..2 Disabled); unattributed because slot 1 is Active; unattributed because slot 1 is
Retired (market_id != 0); unattributed in Resolved.

### A4 `proof_p2b_attributed_domain_is_the_claim_source_of_the_bankrupt_side` (wiring)

Links A3 to the claim model: for a leg on side `X`, the engine sources positive PnL from
`insurance_domain_index(asset, opposite_side(X))` (`src/v16.rs`, settlement at the
`source_domain = self.insurance_domain_index(asset_index, opposite_side(leg.side))` site). For a
bankrupt side `S`, the winners are on `opposite(S)` and source from
`index(asset, opposite(opposite(S))) = index(asset, S)`, which is the domain
`mark_bankruptcy_hlock_event(asset, S)` records. Harness: symbolic `S`; assert
`asset*2 + encode_side(S) == kani_insurance_domain_index(asset, opposite_side(opposite_side(S)))`.
Cheap; its value is that a refactor of either side of the mapping breaks the proof.

### A5 (stretch) `proof_p2b_liquidation_marks_the_bankrupt_leg_side`

Drive `liquidate_account_not_atomic` on a symbolic small one-leg bankrupt account (reuse the
existing liquidation proof fixture and its stubs) and assert the post-state wire is
`1 | (1 << (1 + side_index))` or 1. Expensive; only if A3+A4 leave a gap reviewers care about.

Mutation checks to run once with the proofs (each must make a named harness FAIL):
- restore `pnl_pos_tot == 0` as the claim term → A2's "cleared with pnl_pos_tot > 0" cover
  becomes UNSATISFIABLE (vacuity detector fires);
- drop the `return false` on a nonzero attributed domain → A2 assertion fails;
- attribute `opposite_side(bankrupt_side)` → A4 fails;
- drop the sole-activated check → A3 fails.

---

## Part B. L2, ADL wind-down conservation

The wind-down is `reduce_position(account, asset, close_q)` (the existing unilateral close, the
same one tag 44 RebalanceReduce uses) wrapped in eligibility and health gates. The proofs split
the claim into a kernel part (cheap, exhaustive) and a wiring part (one bounded harness).

### B1 `proof_p2b_wind_down_unilateral_close_kernel_conserves_effective_oi` (kernel)

Symbolic `oi_before_side, oi_before_opp, a_opp, account_eff_q, close_q` (u64-bounded for cost,
`close_q <= min(account_eff_q, oi_before_opp)`, `oi_before_side == oi_before_opp`, the matched
book). Model the two engine steps: side OI −= close_q;
`opp_oi_after = oi_before_opp − close_q`, `a_opp_after = floor(a_opp · opp_oi_after / oi_before_opp)`
(the `reduce_matching_open_interest_for_unilateral_close` arithmetic, via the existing
`wide_mul_div_floor_u128` lemma).

```
oi_side_after == oi_opp_after                      (matched book preserved)
a_opp_after <= a_opp                               (ADL only decays)
opp_oi_after == 0  ⇒  a_opp_after reset path taken (A := ADL_ONE, reset begins)
```

Covers: partial step; the last leg zeroes both sides; `a_opp_after < MIN_A_SIDE` (DrainOnly).
Reuse the existing `kernel_unilateral_close_capacity` proof as a lemma for the capacity bound.

### B2 `proof_p2b_wind_down_moves_no_pnl_and_charges_no_fee` (wiring, the conservation theorem)

Fixture: one asset, one ADL'd side, the target account with one leg already settled to the
current K/F/B (the wind-down refreshes first, so model the post-refresh state directly:
`k_snap == K_side`, `f_snap == F_side`, `b_snap == B_side`, not stale). Symbolic basis, `a_basis`,
`A_side`, `A_opp`, capital, pnl (assume `certified_liq_deficit == 0` via the certify helper, or
assume `capital + pnl >= mm_req`).

Call `wind_down_adl_position_not_atomic` with `EpisodeExpired`. On `Ok`:

```
vault_after == vault_before
insurance_after == insurance_before
capital_after + pnl_after == capital_before + pnl_before        (closing at the mark is value-neutral)
c_tot_after - c_tot_before == capital_after - capital_before
K_side, F_side, B_side and K/F/B of the opposite side unchanged  (A scales only future increments)
|position_after| < |position_before|  and  sign unchanged or flat (never attaches/enlarges/flips)
```

On `Err`: no requirement (the transaction reverts; the wrapper runs it atomically).

Covers: Ok with the leg fully closed; Ok with `adl_cleared == true`; Err(NonProgress) when
`A_long == A_short == ADL_ONE`; Err(LockActive) with a deficit.

Cost control: stub `refresh_account_and_certify_not_atomic` with a contract "returns
Certified and leaves K/F/B snapshots equal to the indices" (the account is constructed settled),
`#[kani::unwind]` sized to `V16_MAX_PORTFOLIO_ASSETS_N`; the u128 products go through the
existing mul-div lemmas, not raw multiplication. Follow `feedback_kani_design_first_run_once`:
lemmas and contracts for u128, one design lead, no trial reruns.

### B3 `proof_p2b_no_risk_increase_admitted_until_both_sides_reset` (no early open)

Symbolic `a_long, a_short, mode_long, mode_short, lifecycle`; call
`kani_require_asset_risk_change_allowed(0, true)`.

```
Ok  ⇒  a_long == a_short == ADL_ONE ∧ mode_long == mode_short == Normal ∧ lifecycle == Active
```

This is the existing `proof_v16_persisted_risk_gate_is_complete_for_all_lifecycles_and_side_modes`
(already updated for the E7 error variants); B3 only adds covers for the two post-wind-down
states: `A == ADL_ONE` on both sides but a side `ResetPending` (refused), and both `Normal`
(admitted). Together with B1 (the last leg sets `ResetPending`) this proves the wind-down can
never let anyone open into a drained side before its reset finalizes.

### B4 `proof_p2b_wind_down_eligibility_is_exactly_adl_plus_bound`

Symbolic `mode`, `lifecycle`, `a_long`, `a_short`, `oi_eff_long/short`, `effective_price`,
`bound` (both variants, symbolic `max_notional_atoms`). Assert `adl_wind_down_eligible` returns
`Ok(true)` iff `mode == Live ∧ lifecycle ∈ {Active, DrainOnly} ∧ (a_long != ADL_ONE ∨ a_short != ADL_ONE)
∧ (bound == EpisodeExpired ∨ floor(max(oi) · price / POS_SCALE) <= max_notional_atoms)`.
Covers both bounds true and false.

### B5 E7 error-variant proofs (already edited, run with the set)

`proof_v16_trade_preflight_risk_gate_blocks_only_unsafe_risk_increase`,
`proof_v16_persisted_risk_gate_is_complete_for_all_lifecycles_and_side_modes` and the
`proof_v16_adl_position_change_gate_is_route_complete_and_exit_live` now assert `LossStale` / `AdlReduceOnly` exactly where the
engine returns them (see the E7 hunks in `tests/proofs_v16.rs`). No new harness.

---

## Run plan (once, locally, after review)

```
cargo kani --tests --harness proof_p2b_ --output-format terse   # A1-A4, B1-B4
cargo kani --tests --harness proof_v16_trade_preflight_risk_gate_blocks_only_unsafe_risk_increase
cargo kani --tests --harness proof_v16_persisted_risk_gate_is_complete_for_all_lifecycles_and_side_modes
cargo kani --tests --harness proof_v16_adl_position_change_gate_is_route_complete_and_exit_live
```

Expected cost: A1, A3, A4, B3, B4 seconds each; A2 under a minute (scalar); B1 minutes (one
mul-div lemma); B2 is the expensive one, budget it on its own and stop at the first verdict.
Record per-harness checks, covers and seconds in `kani-list.json` and the ledger.

---

## Part C. Episode-expiry storage (wrapper `feat/p2b-lock-exits-wrapper`, tags 104/105)

The `EpisodeExpired` bound is no longer "attested by a trusted caller": the wrapper derives it
trustlessly from on-chain state. These harnesses go in the wrapper's Kani crate (next to the
growth-v19 set), run with the same once-locally rule.

Storage: `AssetRiskLimitsV17` bytes 44..64 (asset-slot bytes 652..672), carved from the former
`_reserved: [u8; 22]` with compile-time offset asserts: `adl_max_episode_slots: u32` (0 = default
9,000), `adl_episode_since_slot: u64` (0 = none), `adl_episode_epoch_long/short: u32`.

### C1 `proof_p2b_episode_step_never_expires_on_the_arming_call`
Symbolic stored limits, epochs, `now`. `adl_episode_step`:
- missing record (`since == 0`) or a key mismatch ⇒ returns `expired == false` and stores
  `since = max(now, 1)` and the current key;
- matching key ⇒ never moves `since`, and `expired ⇔ now - since >= N_eff`.
Covers: arm; re-call before N; expiry at exactly N; key mismatch re-arms.

### C2 `proof_p2b_episode_key_changes_only_through_a_reset` (engine-side lemma)
For every engine transition that can set `a_side := ADL_ONE` (`kernel_begin_full_drain_reset`,
the zero-OI branch of `reduce_matching_open_interest_for_unilateral_close`), `epoch_side`
strictly increases. Together with C1: an episode can never be "inherited" by a later one, so
a stored start slot always belongs to the current reduce-only stretch (no early expiry).
Covers: each writer reached.

### C3 `proof_p2b_tag105_is_tighten_only` and `proof_p2b_tag93_preserves_episode`
- Tag 105 accepts `n` iff `1 <= n <= N_eff(stored)`, and writes only `adl_max_episode_slots`.
- Tag 93 (non-growth form) writes the P1 fields and leaves bytes 44..64 bit-identical.
- `validate_asset_risk_limits` rejects `adl_max_episode_slots > 9,000` and a nonzero `_reserved`.

### C4 `proof_p2b_tag104_force_close_requires_bound` (wiring)
With the engine's `wind_down_adl_position_not_atomic` stubbed by contract (requires
`adl_wind_down_eligible(asset, bound)`), the handler only ever passes `EpisodeExpired` when C1
returned `expired`, and otherwise `DustNotional { ADL_WIND_DOWN_DUST_NOTIONAL_ATOMS }`.
Covers both branches, and the "armed, not eligible" early return that commits only the record.

Mutation checks: arm with `expired = true` (C1 fails); key on one epoch only (C2's composition
cover becomes unsatisfiable on a one-sided reset); tag 105 accepting `n > N_eff` (C3 fails);
tag 93 writing zeros over 44..64 (C3 fails).
