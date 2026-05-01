# Risk Engine Spec (Source of Truth) — v12.19.46

**Design:** protected principal + junior profit claims + lazy A/K/F side indices, native 128-bit persistent state.  
**Status:** implementation source of truth. Normative terms are **MUST**, **MUST NOT**, **SHOULD**, **MAY**.  
**Scope:** one perpetual DEX risk engine for one quote-token vault.

This revision supersedes v12.19.45. It preserves the v12.19.45 economics and fixes four remaining liveness/precision issues. CrankForward price-floor liveness now requires progress for both effective prices and authenticated raw targets, or permissionless recovery when a raw target can fall below the floor. Bounded stale-catchup now separates `authenticated_now_slot` from per-segment `segment_slot`: segment time drives K/F accrual, while generation rate limits, fee anchors, current-slot ownership, and h-max same-instruction accounting use the authenticated instruction slot. Same-slot zero-dt funding is no longer equity-active. Reserve release/acceleration wording now consistently uses the final `hmax_effective_active(ctx)` state. The normative safety and liveness requirements remain in §§0–10.

> The stress-scaled consumption threshold is **not** an anti-oracle-manipulation warmup. `admit_h_min` is the market's normal live h-lock floor. It MAY be zero if the market explicitly accepts immediate-release semantics and the lack of fair-exit guarantees among positive-PnL accounts while `Residual > 0` and the system is not h-max locked. Public wrappers MUST make this a stable market policy, not caller-controlled input. Any active threshold stress or bankruptcy h-lock overrides `admit_h_min` and admits fresh positive PnL at `admit_h_max`.
>
> The engine's `oracle_price` input is the **effective engine price** that will be accrued against, not necessarily the raw external oracle target. A public wrapper whose raw normalized target jumps farther than the engine price cap MUST feed the engine a valid capped staircase price, keep the raw target separate from the last effective engine price, and restrict or conservatively shadow-check user value-moving/risk-increasing operations while the target and effective engine price differ.

The detailed normative requirements are in §§0–10.

Every top-level instruction is atomic. Any failed precondition, checked arithmetic guard, missing authenticated account proof, context-capacity overflow, or conservative-failure condition MUST roll back every mutation performed by that instruction. Before committing, every top-level instruction MUST leave all applicable global invariants in §2.2 true, not only the local invariant most directly related to the endpoint.

---

## 0. Core safety and liveness requirements

The engine MUST maintain the following properties.

1. Flat protected principal is senior. An account with effective position `0` MUST NOT have protected principal reduced by another account's insolvency.
2. Open opposing positions MAY be subject to explicit deterministic ADL during bankrupt liquidation. ADL MUST be visible protocol state, never hidden execution.
3. Live positive PnL MUST pass admission. It MUST NOT be directly withdrawable, converted to principal, or counted as matured collateral unless admitted by the current instruction policy and the engine gates.
4. `admit_h_min` is the normal market h-lock floor. Public or permissionless wrappers MAY set `admit_h_min = 0` only as stable market policy that explicitly accepts immediate-release semantics and no fair-exit guarantee among positive-PnL accounts while `Residual > 0` and not h-max locked. Stress-threshold gating and bankruptcy h-lock are additive overrides and MUST NOT be treated as substitutes for sound wrapper policy.
5. A candidate trade's own positive execution-slippage PnL MUST be removed from that same trade's risk-increasing approval metric.
6. Explicit protocol fees are collected into `I` immediately or tracked as account-local fee debt up to collectible headroom. Uncollectible fee tails are dropped, not socialized.
7. Losses are senior to engine-native fees on the same local capital state.
8. Synthetic liquidation close executes at oracle mark; liquidation penalties are explicit fees only.
9. Resolved positive payouts MUST wait for all stale accounts and all negative PnL to be reconciled, then use one shared payout snapshot.
10. Any arithmetic not proven unreachable by bounds MUST have checked, deterministic behavior. Silent wrap, unchecked panic, and undefined truncation are forbidden.
11. Account capacity is finite; empty fully-drained accounts MUST be reclaimable permissionlessly.
12. Keeper progress MUST be possible with off-chain candidate discovery and without a mandatory on-chain global scan.
13. The wrapper MUST NOT overload raw oracle target state and effective engine price state. Known lag between them MUST NOT become a public free-option: user risk-increasing and extraction-sensitive operations MUST be rejected or checked under a conservative target-price shadow policy while the lag exists.
14. While the configured stress gate or bankruptcy h-max lock is active, positive PnL MUST NOT become more withdrawable: natural reserve release, pending-reserve promotion that starts a release clock, reserve acceleration, auto-conversion, manual conversion, and public risk-increasing approval based on positive-PnL equity MUST be paused or conservatively shadow-checked until the stress gate clears. Active stress MUST NOT be treated as a blanket trade, settlement, liquidation, or keeper-crank halt; those paths continue under h-max admission and the zero-positive-credit or stricter dual-price lanes.
15. Live time is clock-owned. Public instructions MUST source `now_slot` from authenticated runtime/clock state; a caller-supplied slot may be used only as a hint bounded by that authenticated clock and MUST NOT move engine time forward beyond it. Live instructions MUST NOT decrease `current_slot` or `slot_last`, and MUST validate authenticated `now_slot >= current_slot` before writing time, fee anchors, or accrual state.
16. A fee path that can reduce account capital MUST NOT be applied ahead of unsettled mark/funding/ADL losses on that account. For nonflat accounts, or accounts not proven authoritative at the current engine state, the instruction MUST first perform the live touch/loss-settlement path, then charge recurring or explicit engine-native fees, then perform health-sensitive checks or payouts.
17. A live local-touch context MUST be finalized at most once. If a value-moving endpoint needs finalization before payout or before freeing a touched account, that endpoint's finalization is the single required lifecycle finalization; the later standard lifecycle step MUST be skipped for that context. A finalized context MUST NOT accept additional live touches, and a context MUST NOT be finalized after any of its touched accounts have been freed.
18. Resolution MUST compute terminal K deltas before zeroing OI, changing side epochs, resetting A/K/F, or entering resolved mode. The terminal delta for a side with zero effective OI at the live-sync price, including a side already `ResetPending`, is exactly zero. A side already `ResetPending` MUST NOT be reset again; its stale accounts settle only against the preserved `K_epoch_start_s` / `F_epoch_start_s_num` state.
19. Every persistent mutation of `C_i` after materialization and before free MUST use `set_capital`, and every persistent mutation of `basis_pos_q_i` after materialization and before free MUST use `set_position_basis_q`, or an exactly equivalent aggregate-updating path. Direct account-local writes are allowed only during canonical materialization/free-slot reset after aggregate counts have already been initialized, updated, or proven zero.
20. A live operation that changes an account's nonzero effective position MUST use the canonical effective-position attachment helper, or an exactly equivalent path, to update `basis_pos_q_i`, `a_basis_i`, `k_snap_i`, `f_snap_i`, and `epoch_snap_i` atomically from the current side state after old A/K/F effects have been settled. Calling `set_position_basis_q` alone is sufficient for count-correct zeroing only; it is not a complete live nonzero position attach. The live attach/clear helpers MUST NOT mutate `OI_eff_long` or `OI_eff_short`; global OI mutation is caller-owned and, for liquidation close quantity, is performed exactly once by `enqueue_adl`.
21. A nonflat withdrawal MUST be checked against the candidate post-withdrawal account state. The health check MUST reduce local `C_i`, global `C_tot`, and `V` by the requested amount before computing `Eq_withdraw_raw_i` or, during active stress on public paths, `Eq_withdraw_no_pos_i`. Checking only the global totals while leaving local `C_i` at its pre-withdrawal value is non-compliant.
22. A/K/F settlement MUST use the exact signed-floor formula in §5.2 for live, reset-pending, and resolved settlement. Rounding negative deltas toward zero is forbidden because it systematically undercharges losses and overstates equity.
23. A resolved terminal close MUST be fee-current. If recurring fees are enabled, the account MUST be synced to `resolved_slot` after terminal side/loss settlement and before any payout, capital free, fee forgiveness, or `ProgressOnly` return.
24. End-of-instruction phantom/orphan cleanup MUST NOT be a hidden position wipe and MUST NOT become a keeper deadlock. Clearing residual OI as ordinary phantom dust requires a side-local certified dust proof, consumes the certified bound used for the proof, and if the side still has stored positions, the same finalization MUST schedule/begin a full drain reset for that side unless the side is already `ResetPending`. If one side is aggregate-flat with residual OI, that side's residual is certified orphan OI because no current-epoch stored account can represent it. If the opposite side still has stored current-epoch positions and its residual OI is not certified phantom dust, the finalization MUST perform the explicit orphan-exposure drain reset in §5.5 rather than failing indefinitely or allowing future mark/funding accrual against the orphan side. A side with stored positions and zero OI MUST NOT remain in `Normal` or `DrainOnly` current live epoch where future mark/funding accrual is disabled but account basis is still live; a side already in `ResetPending` is valid stale-settlement state.
25. Value inflows and aggregate reclassifications MUST NOT rely on informal overflow assumptions. Deposits, fee-credit deposits, insurance top-ups, fee-to-insurance reclassifications, PnL-to-capital conversions, withdrawals, terminal payouts, and any helper-equivalent path MUST compute candidate `V`, `I`, `C_tot`, and relevant account-local values in checked arithmetic and MUST reject if the candidate state would violate `C_tot <= V <= MAX_VAULT_TVL`, `I <= V`, or `V >= C_tot + I`.
26. Side modes are hard gates, not labels. Before any operation creates, replaces, or increases exposure on side `s`, the side MUST be `Normal` after any required ready-reset finalization. `DrainOnly` forbids fresh opens, flips into the side, same-side increases, and replacement of one account's exposure with another's exposure on that side; it allows only settlement, liquidation, and strictly reducing or clearing already-stored same-side exposure. `ResetPending` forbids all fresh/current-epoch exposure and allows only stale settlement, reset finalization, and explicitly specified recovery.
27. ADL quantity socialization MUST update the opposing side's potential phantom-dust bound with the formula in §5.4, or with a tighter exact account-scan result. Potential dust MUST NOT be used as clearance proof unless certified by exact account scan or another exact proof of the represented/phantom split. A single account-local floor-to-zero settlement is not such a proof; it may add potential dust only.
28. Sweep-generation advancement is slot-rate-limited. `sweep_generation` MUST NOT advance more than once per slot, and a cursor wrap in the same slot as a nonzero stress consumption or bankruptcy h-lock activation MUST NOT count as an eligible post-event generation advance. H-max lock may clear only after the full post-event envelope is covered and after at least one eligible generation advance strictly after the h-max-lock start generation and start slot.
29. Equity-active catchup MUST be composed with a protective lifecycle. For global reconciliation claims, including stress clearance, the lifecycle MUST include keeper Phase 2 envelope coverage. For liquidation claims, it MUST touch/revalidate the liquidation candidate set required by the liquidation policy. For user value-moving claims, it MUST touch the user account and apply the conservative lag/stress shadow policy. An arbitrary one-account settlement is not sufficient proof that global stale losses are reconciled.
30. ADL K-loss socialization MUST exclude certified phantom OI from the loss-bearing set and MUST be bounded by an exact worst-case rounded settlement-loss upper bound for the proposed K delta. If `phantom_dust_potential_side_q > phantom_dust_certified_side_q` and no exact account scan proves the represented/phantom split, the implementation MUST NOT apply a K-loss allocation using an unsafe denominator; it MUST route the uncertain socialized loss to uninsured loss while still performing deterministic quantity socialization. Even when an exact scan is supplied, the implementation MUST NOT use `sum effective_abs_pos_q`, a current-state before/after settlement comparison, or any other floor-rounded aggregate as a standalone K-loss proof unless it also proves that the proposed K delta's worst-case rounded settlement-loss upper bound is no greater than the socialized loss budget.
31. A bounded keeper crank MUST be allowed to make partial progress. It MUST NOT reject solely because more suspected liquidatable, unhealthy, or not-provably-solvent accounts exist than fit into `ordered_candidates[]`, account context capacity, or a wrapper wire-format cap. Such accounts remain lazy and must be handled by later cranks, by user/account touches, or by resolution.
32. Public trading during an h-max-lock sweep MUST be conservative but live. A risk-increasing trade while h-max lock is active MUST use `Eq_trade_open_no_pos_i` and any required effective-price/raw-target dual-price checks; if those checks and all ordinary side-mode, OI, position, fee-current, and margin checks pass, the trade MUST NOT be rejected solely because the h-max envelope has not yet cleared. Risk-reducing and flat-result trades likewise MUST NOT be rejected solely because h-max lock is active.
33. Same-instruction h-max activation is retroactive for the instruction's own positive-PnL usability mutations and positive-credit approvals. A permissionless keeper or caller MUST NOT be able to admit or mature a winning account at `admit_h_min`, release or accelerate an existing reserve, convert released PnL, auto-convert positive PnL, approve a risk increase with positive-PnL credit, and then trigger bankruptcy h-max later in the same atomic instruction. If h-max activates or restarts before commit, the instruction MUST either recompute those mutations/approvals under h-max/zero-positive-credit lanes, retroactively reclassify the instruction-local positive PnL back into h-max reserve and undo any same-instruction conversion, or fail before commit.
34. A public market with `cfg_public_liveness_profile == CrankForward` MUST be crank-forward by construction or by canonical permissionless recovery. Such a wrapper MUST NOT deploy a price scale/configuration where exposed live catchup can enter a permanent `max_delta == 0` dead zone unless it also exposes `permissionless_recovery_resolve` in §8.8, or a stricter public recovery path with the same oracle-authentication, h-max, payout-readiness, and anti-extraction rules. A recovery caller MUST NOT be able to force recovery by omitting/corrupting proofs, naming an unproven reason, or choosing invalid segment inputs; the recovery reason and recovery price must be proven from authenticated state and deterministic market policy. CrankForward profiles SHOULD instead enforce the progress-floor invariant in §1.7 so every exposed bounded catchup segment can move by at least one price unit whenever target and effective price differ.
35. A CrankForward public market with stale exposed state MUST have permissionless bounded catchup. If `now_slot - slot_last > cfg_max_accrual_dt_slots`, a public keeper path MUST be able to advance one bounded accrual segment and perform protective progress instead of requiring privileged recovery or a one-shot full catchup. Repeated bounded catchup cranks MUST be able to move `slot_last`, K/F, h-max state, and the sweep cursor forward until ordinary live operation resumes. If K/F future-headroom or representability prevents the next bounded segment, a CrankForward profile MUST route to `permissionless_recovery_resolve` in §8.8, or a stricter public recovery path; privileged-only recovery is not sufficient.
36. Candidate and account-data packing limits MUST produce bounded partial progress, not compute or wire-format deadlock. Public wrappers MUST bound candidate entries inspected as well as successful revalidations, MUST expose a cursor-priority or cursor-only crank composition, and MUST design authenticated account storage/proofs so an honest keeper can permissionlessly supply the current Phase 2 cursor account whenever that slot is materialized.

---

## 1. Types, units, constants, configuration

### 1.1 Persistent and transient arithmetic

- Persistent unsigned economic quantities use `u128` unless otherwise stated.
- Persistent signed economic quantities use `i128` and MUST NOT equal `i128::MIN`.
- `wide_unsigned` / `wide_signed` mean exact transient domains at least 256 bits wide, or a formally equivalent comparison-preserving method.
- All products involving prices, positions, A/K/F indices, funding numerators, ADL deltas, fee products, haircut numerators, stress-progress numerators, or warmup-release numerators MUST use checked arithmetic or exact multiply-divide helpers.
- All monotonic counters and epochs, including side epochs and `sweep_generation`, MUST use checked increments. Counter overflow is a conservative failure/recovery condition; silent wrap is forbidden.

### 1.2 Units

- `POS_SCALE = 1_000_000`.
- `price: u64` is quote atomic units per `1` base.
- Every price input and stored live/resolved price MUST satisfy `0 < price <= MAX_ORACLE_PRICE`.
- For live accrual, `oracle_price` means the wrapper-fed **effective engine price**. The raw external oracle target is wrapper-owned input state and is not stored or derived by the engine core.
- `basis_pos_q_i: i128` stores signed base position scaled by `POS_SCALE`.
- `RiskNotional_i = 0` if `effective_pos_q(i) == 0`, else:

```text
RiskNotional_i = ceil(abs(effective_pos_q(i)) * oracle_price / POS_SCALE)
```

This ceiling is load-bearing. A nonzero fractional quote-notional position has nonzero risk notional and cannot evade maintenance by floor rounding. Floor oracle notional MAY be displayed or used by wrapper policy, but MUST NOT be used for margin.

- Trade fees use executed floor notional:

```text
trade_notional = floor(size_q * exec_price / POS_SCALE)
```

### 1.3 A/K/F scales

```text
ADL_ONE    = 1_000_000_000_000_000
FUNDING_DEN = 1_000_000_000
```

`A_side` is dimensionless and scaled by `ADL_ONE`. `K_side` has units `ADL scale * quote/base`. `F_side_num` has units `ADL scale * quote/base * FUNDING_DEN`.

### 1.4 Hard bounds

```text
MAX_VAULT_TVL                 = 10_000_000_000_000_000
MAX_ORACLE_PRICE              = 1_000_000_000_000
MAX_POSITION_ABS_Q            = 100_000_000_000_000
MAX_TRADE_SIZE_Q              = MAX_POSITION_ABS_Q
MAX_OI_SIDE_Q                 = 100_000_000_000_000
MAX_ACCOUNT_NOTIONAL          = 100_000_000_000_000_000_000
MAX_PNL_POS_TOT_LIVE          = 170_141_183_460_469_231_731_687_303_715_884_105_727
MAX_PROTOCOL_FEE_ABS          = 1_000_000_000_000_000_000_000_000_000_000_000_000
GLOBAL_MAX_ABS_FUNDING_E9_PER_SLOT = 10_000
MAX_TRADING_FEE_BPS           = 10_000
MAX_INITIAL_BPS               = 10_000
MAX_MAINTENANCE_BPS           = 10_000
MAX_LIQUIDATION_FEE_BPS       = 10_000
MAX_MATERIALIZED_ACCOUNTS     = 1_000_000
MIN_A_SIDE                    = 100_000_000_000_000
MAX_WARMUP_SLOTS              = 18_446_744_073_709_551_615
MAX_RESOLVE_PRICE_DEVIATION_BPS = 10_000
STRESS_CONSUMPTION_SCALE      = 1_000_000_000
```

`MAX_ACTIVE_POSITIONS_PER_SIDE` MUST be finite and MUST NOT exceed `MAX_MATERIALIZED_ACCOUNTS`. `MAX_PNL_POS_TOT_LIVE` is the aggregate live-positive-PnL guard; implementations MAY choose a lower deployment-specific value, but it MUST be finite and no greater than the stated bound.

### 1.5 Immutable per-market configuration

The market stores immutable:

```text
cfg_h_min, cfg_h_max
cfg_maintenance_bps, cfg_initial_bps
cfg_trading_fee_bps
cfg_liquidation_fee_bps, cfg_liquidation_fee_cap, cfg_min_liquidation_abs
cfg_min_nonzero_mm_req, cfg_min_nonzero_im_req
cfg_resolve_price_deviation_bps
cfg_max_active_positions_per_side
cfg_max_accrual_dt_slots
cfg_max_abs_funding_e9_per_slot
cfg_max_price_move_bps_per_slot
cfg_min_funding_lifetime_slots
cfg_account_index_capacity
cfg_public_liveness_profile in {BestEffort, CrankForward}
cfg_permissionless_recovery_enabled
cfg_recovery_p_last_fallback_enabled
```

Initialization MUST require:

```text
0 < cfg_min_nonzero_mm_req < cfg_min_nonzero_im_req
0 <= cfg_maintenance_bps <= MAX_MAINTENANCE_BPS
cfg_maintenance_bps <= cfg_initial_bps <= MAX_INITIAL_BPS
0 <= cfg_trading_fee_bps <= MAX_TRADING_FEE_BPS
0 <= cfg_liquidation_fee_bps <= MAX_LIQUIDATION_FEE_BPS
0 <= cfg_min_liquidation_abs <= cfg_liquidation_fee_cap <= MAX_PROTOCOL_FEE_ABS
0 <= cfg_h_min <= cfg_h_max <= MAX_WARMUP_SLOTS
cfg_h_max > 0
0 <= cfg_resolve_price_deviation_bps <= MAX_RESOLVE_PRICE_DEVIATION_BPS
0 < cfg_account_index_capacity <= MAX_MATERIALIZED_ACCOUNTS
0 < cfg_max_active_positions_per_side <= MAX_ACTIVE_POSITIONS_PER_SIDE
cfg_max_active_positions_per_side <= cfg_account_index_capacity
0 < cfg_max_accrual_dt_slots <= MAX_WARMUP_SLOTS
0 <= cfg_max_abs_funding_e9_per_slot <= GLOBAL_MAX_ABS_FUNDING_E9_PER_SLOT
0 < cfg_max_price_move_bps_per_slot
initial_oracle_price satisfies 0 < initial_oracle_price <= MAX_ORACLE_PRICE
cfg_public_liveness_profile is explicitly set; public wrappers MUST NOT claim CrankForward behavior unless it is CrankForward
if cfg_public_liveness_profile == CrankForward: cfg_permissionless_recovery_enabled == true, unless initialization proves ordinary bounded catchup cannot enter any below-floor effective-price, below-floor authenticated-raw-target, stale-dt, K/F-headroom, representability, oracle-unavailable, or counter-overflow deadlock state
if cfg_recovery_p_last_fallback_enabled: the market publicly discloses emergency `P_last` settlement semantics
P_last = fund_px_last = initial_oracle_price at initialization
bankruptcy_hmax_lock_active = false and stress/h-max envelope fields are zero/None at initialization
```

For a public market with `cfg_public_liveness_profile == CrankForward`, define:

```text
MIN_EXPOSED_PROGRESS_PRICE = ceil(10_000 / cfg_max_price_move_bps_per_slot)
```

A public CrankForward market MUST satisfy both liveness layers below before it may advertise or rely on permissionless crank-forward liveness:

```text
A. ordinary progress layer:
   either the deployed wrapper/oracle proves that, while live exposure exists and ordinary
   catchup is the advertised liveness path, every valid exposed live effective price and every
   authenticated raw target that must be caught up satisfies
   price >= MIN_EXPOSED_PROGRESS_PRICE, or any below-floor effective-price or raw-target state
   routes to §8.8 recovery; and

B. terminal recovery layer:
   §8.8 permissionless recovery-resolution, or a stricter public recovery path, is enabled for
   every non-catchupable state: below-floor effective price, below-floor authenticated raw target,
   stale segment blockage, K/F headroom or representability failure, authenticated
   oracle/target/funding unavailability, and declared counter/epoch overflow. If this layer is
   disabled, initialization MUST include a formal proof that none of those states can occur under
   the deployed wrapper, oracle, and engine bounds.
```

A public market using the price-scale invariant MUST either normalize price units and oracle policy so the authenticated raw target for the supported market cannot fall below the progress floor while live exposure remains open, or enable §8.8 permissionless recovery for that case. If the authenticated raw target falls below that floor anyway while the market is exposed, the wrapper MUST treat the market as `BelowProgressFloor`: ordinary user extraction/risk-increase stays rejected or on conservative dual-price/zero-positive-credit lanes, and §8.8 permissionless recovery-resolution MUST be callable once the effective price cannot strictly move toward the raw target without violating the floor. A floored-effective-price policy MAY keep keeper sweeps and liquidations live temporarily, but it MUST NOT be the only escape and MUST NOT be advertised as raw-target catchup.

If `cfg_public_liveness_profile == BestEffort`, the wrapper MUST disclose that worst-case permissionless crank-forward liveness is not guaranteed and MUST NOT describe the market as always-crankable. Safety rules still apply: account-free equity-active accrual is forbidden, user extraction/risk-increase stays on conservative lanes during lag/h-max, and privileged-only recovery remains a disclosed product/governance risk rather than a crank-forward guarantee.


Live admission pairs MUST satisfy:

```text
0 <= admit_h_min <= admit_h_max <= cfg_h_max
admit_h_max > 0
admit_h_max >= cfg_h_min
if admit_h_min > 0: admit_h_min >= cfg_h_min
```

For public or permissionless wrappers with untrusted live oracle, execution-price PnL, or live funding PnL, wrapper policy MUST treat `admit_h_min` as stable market configuration, not caller-controlled input. `admit_h_min = 0` is allowed only for markets that explicitly accept immediate-release semantics and the absence of ordinary fair-exit guarantees while `Residual > 0` and not h-max locked; active threshold stress or bankruptcy h-lock still forces `admit_h_max`.

### 1.6 Funding and solvency-envelope validation

Initialization MUST validate, in exact wide arithmetic:

```text
ADL_ONE * MAX_ORACLE_PRICE * cfg_max_abs_funding_e9_per_slot * cfg_max_accrual_dt_slots <= i128::MAX
cfg_min_funding_lifetime_slots >= cfg_max_accrual_dt_slots
ADL_ONE * MAX_ORACLE_PRICE * cfg_max_abs_funding_e9_per_slot * cfg_min_funding_lifetime_slots <= i128::MAX
```

Initialization MUST also validate the exact per-risk-notional envelope below for every integer risk notional `N` with `1 <= N <= MAX_ACCOUNT_NOTIONAL`, by an exact bounded breakpoint/interval proof or by a stronger conservative sufficient proof. Unbounded runtime loops over all `N` are forbidden on constrained runtimes.

Let:

```text
price_budget_bps  = cfg_max_price_move_bps_per_slot * cfg_max_accrual_dt_slots
funding_budget_num = cfg_max_abs_funding_e9_per_slot * cfg_max_accrual_dt_slots * 10_000
loss_budget_num   = price_budget_bps * FUNDING_DEN + funding_budget_num
```

For each `N`:

```text
price_funding_loss_N = ceil(N * loss_budget_num / (10_000 * FUNDING_DEN))
worst_liq_notional_N = ceil(N * (10_000 + price_budget_bps) / 10_000)
liq_fee_raw_N        = ceil(worst_liq_notional_N * cfg_liquidation_fee_bps / 10_000)
liq_fee_N            = min(max(liq_fee_raw_N, cfg_min_liquidation_abs), cfg_liquidation_fee_cap)
mm_req_N             = max(floor(N * cfg_maintenance_bps / 10_000), cfg_min_nonzero_mm_req)
require price_funding_loss_N + liq_fee_N <= mm_req_N
```

This law is the construction-level self-neutral-siphon boundary. It accounts for fractional funding, integer rounding, worst adverse post-move liquidation notional, bps fees, fee floors, and fee caps. Implementations MUST NOT substitute floor-funded bps budgeting, pre-move liquidation notional, floor risk notional, or a two-point small-notional shortcut unless accompanied by an exact proof covering every intervening and larger notional.

If a deployment defines `permissionless_resolve_stale_slots`, initialization MUST require:

```text
permissionless_resolve_stale_slots <= cfg_max_accrual_dt_slots
```

### 1.7 Wrapper-fed effective price, raw oracle target, and catchup composition

Oracle normalization, source selection, target storage, and rate limiting are wrapper-owned. The engine only validates and accrues the effective `oracle_price` passed to it.

A compliant public wrapper SHOULD maintain distinct fields equivalent to:

```text
oracle_target_price
oracle_target_publish_ts
last_effective_price
```

The wrapper MUST NOT overload `last_effective_price` as the raw target. If the external target jumps beyond the engine cap, the wrapper keeps the raw target and feeds a capped staircase of effective prices until caught up.

For an exposed live market (`OI_eff_long != 0 || OI_eff_short != 0`), a public wrapper's next effective price MUST be computed by the deterministic clamp law, unless it enters a `permissionless_recovery_resolve`/resolution procedure with at least the same safety guarantees:

```text
dt = now_slot - slot_last
if target == P_last or dt == 0:
    next_price = P_last
else:
    max_delta = floor(P_last * cfg_max_price_move_bps_per_slot * dt / 10_000)
    next_price = clamp_toward(P_last, target, max_delta)
```

The multiplication MUST use exact wide arithmetic; `max_delta` MAY be capped to the price type maximum after the exact quotient. `clamp_toward` moves toward `target` by at most `max_delta` and never overshoots. For public markets using the price-scale invariant in §1.5, exposed live `next_price` MUST remain at or above `MIN_EXPOSED_PROGRESS_PRICE`; a raw target below that floor is handled by `permissionless_recovery_resolve`/resolution or by a documented floored-effective-price policy with all target/effective divergence protections active. If the floored policy cannot strictly move toward the authenticated raw target, `permissionless_recovery_resolve` MUST be callable. The result MUST satisfy the engine cap in §5.3.

Normative consequences:

- Same-slot exposed cranks (`dt == 0`) MUST pass `P_last`; price catch-up requires elapsed slots. They MAY still do Phase 1 liquidation checks and Phase 2 round-robin touches at the unchanged effective price.
- If exposed `target != P_last`, `dt > 0`, and the computed `max_delta == 0`, this is a deployment/configuration failure for a public liveness profile unless §8.8 recovery is available. The wrapper MUST treat this as `BelowProgressFloor` / `CatchupRequired`, MUST NOT advance `slot_last` by feeding the unchanged price merely to bypass the lag, and MUST make permissionless recovery-resolution callable; rejecting user extraction/risk-increase alone is not sufficient if no public path can move or resolve the market.
- If exposed `target != P_last`, `dt > 0`, and the computed `max_delta > 0`, a public wrapper MUST feed the clamped moved price. It MUST NOT perform a no-op accrual by passing `P_last` merely to update liveness stamps or defer catch-up.
- If exposed `dt > cfg_max_accrual_dt_slots`, ordinary one-step live catch-up is unavailable. A public wrapper MUST provide a permissionless bounded catchup crank as specified in §8.7.1, or an explicit public recovery/resolution path with equivalent safety. Privileged-only recovery is not sufficient for a market that claims permissionless worst-case crank-forward liveness.
- If both OI sides are zero, no live position can lose equity, so the wrapper MAY feed the raw target directly subject to ordinary price validity.
- Feeding a cap-violating raw target into exposed live accrual is non-compliant and should fail before engine state mutation.

While `oracle_target_price != P_last`, the market is intentionally using a lagged effective engine price. For public wrappers, keeper progress, liquidation attempts, settlement, and structural sweep MAY continue at the effective price, but user operations that are risk-increasing or extraction-sensitive MUST either be rejected or pass a conservative wrapper shadow policy using both the effective engine price and the raw target. At minimum, public wrappers MUST subject risk-increasing user trades during target/effective-price divergence to a stricter dual-price policy that removes the known-lag free option. A public wrapper MUST NOT reject a trade solely because target/effective divergence exists if the trade passes that dual-price policy, the active-stress zero-positive-credit lane when applicable, side-mode gating, OI/position limits, and ordinary fee-current margin checks.

Account-free catchup is a wrapper composition boundary. A public wrapper instruction that has no candidate list, no authenticated index scan, no account touch set, and no liquidation/revalidation phase MUST NOT perform equity-active accrual while the market is exposed. Equity-active means either:

```text
price_move_active = (P_last > 0 && next_price != P_last && (OI_eff_long != 0 || OI_eff_short != 0))
funding_active    = (dt > 0 && funding_rate != 0 && OI_eff_long != 0 && OI_eff_short != 0 && fund_px_last > 0)
```

If price movement or active funding would move account equity, the wrapper MUST reject before accrual and route through a protective account-touching lifecycle. The required lifecycle depends on the safety claim being made:

```text
user value-moving or risk-increasing claim:
    touch the user's account, settle it authoritatively, and apply lag/stress shadow checks

liquidation claim:
    touch/revalidate the liquidation candidate set required by the liquidation policy,
    then execute liquidation only on fee-current authoritative state

global reconciliation or stress-clear claim:
    run keeper-style account touching with authenticated Phase 2 envelope coverage;
    a single arbitrary settle/touch is not sufficient

recovery claim:
    use an explicitly specified conservative recovery procedure whose touched set and
    invariants are at least as protective as the standard lifecycle it replaces
```

A path MAY touch fewer than all accounts only when it does not claim global reconciliation and all user-visible value movement/risk increase is checked under the appropriate conservative effective-price/raw-target and stress-shadow policy.

---

## 2. State

### 2.1 Account state

Each materialized account stores:

```text
C_i: u128
PNL_i: i128
R_i: u128
basis_pos_q_i: i128
a_basis_i: u128
k_snap_i: i128
f_snap_i: i128
epoch_snap_i: u64
fee_credits_i: i128            <= 0, never i128::MIN
last_fee_slot_i: u64
```

Live accounts additionally store at most one scheduled bucket and one pending bucket.

Scheduled bucket:

```text
sched_present_i: bool
sched_remaining_q_i: u128
sched_anchor_q_i: u128
sched_start_slot_i: u64
sched_horizon_i: u64
sched_release_q_i: u128
```

Pending bucket:

```text
pending_present_i: bool
pending_remaining_q_i: u128
pending_horizon_i: u64
```

Live reserve invariants:

```text
R_i = scheduled_remaining + pending_remaining
if sched_present: 0 < sched_remaining <= sched_anchor, cfg_h_min <= sched_horizon <= cfg_h_max, sched_release <= sched_anchor
if pending_present: 0 < pending_remaining, cfg_h_min <= pending_horizon <= cfg_h_max
if R_i == 0: both buckets absent
pending never matures while pending
```

If `basis_pos_q_i != 0`, then `a_basis_i > 0`. Any helper dividing by `a_basis_i` or `a_basis_i * POS_SCALE` MUST fail conservatively if the denominator is zero.

On resolved markets, reserve storage is inert and MUST be cleared by `prepare_account_for_resolved_touch` before mutating resolved PnL.

Wrapper-owned annotation fields MAY exist, but the engine MUST never read them to decide margin, liquidation, fee routing, admission, accrual, resolution, reset, reclamation, conservation, or authorization. They MUST be canonicalized on materialization and cleared on free-slot reset.

### 2.2 Global state

The engine stores:

```text
V, I, C_tot, PNL_pos_tot, PNL_matured_pos_tot: u128
current_slot, slot_last: u64
P_last, fund_px_last: u64
A_long, A_short: u128
K_long, K_short: i128
F_long_num, F_short_num: i128
epoch_long, epoch_short: u64
K_epoch_start_long, K_epoch_start_short: i128
F_epoch_start_long_num, F_epoch_start_short_num: i128
OI_eff_long, OI_eff_short: u128
mode_long, mode_short in {Normal, DrainOnly, ResetPending}
stored_pos_count_long, stored_pos_count_short: u64
stale_account_count_long, stale_account_count_short: u64
phantom_dust_certified_long_q, phantom_dust_certified_short_q: u128
phantom_dust_potential_long_q, phantom_dust_potential_short_q: u128
materialized_account_count, neg_pnl_account_count: u64
rr_cursor_position, sweep_generation: u64
last_sweep_generation_advance_slot: optional u64
bankruptcy_hmax_lock_active: bool
stress_consumed_bps_e9_since_envelope: u128
stress_envelope_remaining_indices: u64
stress_envelope_start_slot: optional u64
stress_envelope_start_generation: optional u64
market_mode in {Live, Resolved}
resolved_price, resolved_live_price: u64
resolved_slot: u64
resolved_k_long_terminal_delta, resolved_k_short_terminal_delta: i128
resolved_payout_snapshot_ready: bool
resolved_payout_h_num, resolved_payout_h_den: u128
```

`phantom_dust_certified_side_q` is the only dust quantity that may be used to clear residual OI as phantom. It is a lower-bound proof that at least that much global OI is not represented by any current-epoch account. `phantom_dust_potential_side_q` is a conservative upper-bound diagnostic/recovery quantity produced by per-account floor slack and ADL formulae; it MAY guide exact scans or `permissionless_recovery_resolve`, but it MUST NOT by itself justify clearing real current-epoch OI.

Global invariants:

```text
C_tot <= V <= MAX_VAULT_TVL
I <= V
V >= C_tot + I
0 <= neg_pnl_account_count <= materialized_account_count <= cfg_account_index_capacity <= MAX_MATERIALIZED_ACCOUNTS
0 <= stored_pos_count_long <= materialized_account_count
0 <= stored_pos_count_short <= materialized_account_count
0 <= stale_account_count_long <= stored_pos_count_long
0 <= stale_account_count_short <= stored_pos_count_short
0 <= OI_eff_long <= MAX_OI_SIDE_Q
0 <= OI_eff_short <= MAX_OI_SIDE_Q
0 < P_last <= MAX_ORACLE_PRICE
0 < fund_px_last <= MAX_ORACLE_PRICE
0 < A_long <= ADL_ONE
0 < A_short <= ADL_ONE
abs(K_long)  + A_long  * MAX_ORACLE_PRICE <= i128::MAX
abs(K_short) + A_short * MAX_ORACLE_PRICE <= i128::MAX
funding_headroom_long  = A_long  * MAX_ORACLE_PRICE * cfg_max_abs_funding_e9_per_slot * cfg_max_accrual_dt_slots
funding_headroom_short = A_short * MAX_ORACLE_PRICE * cfg_max_abs_funding_e9_per_slot * cfg_max_accrual_dt_slots
abs(F_long_num)  + funding_headroom_long  <= i128::MAX
abs(F_short_num) + funding_headroom_short <= i128::MAX
0 <= rr_cursor_position < cfg_account_index_capacity
if last_sweep_generation_advance_slot != None: last_sweep_generation_advance_slot <= current_slot
if stress_consumed_bps_e9_since_envelope == 0 && bankruptcy_hmax_lock_active == false:
    stress_envelope_remaining_indices = 0
    stress_envelope_start_slot = None
    stress_envelope_start_generation = None
if stress_consumed_bps_e9_since_envelope > 0 || bankruptcy_hmax_lock_active == true:
    stress_envelope_start_slot != None
    stress_envelope_start_generation != None
    stress_envelope_remaining_indices <= cfg_account_index_capacity
for each side s in Live: if mode_s == ResetPending then OI_eff_s == 0
for each side s in Live: if mode_s != ResetPending and stored_pos_count_s > 0 then OI_eff_s > 0
for each side s in Live: if stored_pos_count_s == 0 && OI_eff_s == 0 then phantom_dust_certified_s_q == 0 and phantom_dust_potential_s_q == 0
phantom_dust_certified_s_q <= phantom_dust_potential_s_q <= OI_eff_s unless OI_eff_s == 0, in which case both are 0
slot_last <= current_slot
F_long_num and F_short_num fit i128
if Live: PNL_matured_pos_tot <= PNL_pos_tot <= MAX_PNL_POS_TOT_LIVE and resolved fields are zero
if Resolved: resolved_price > 0, resolved_live_price > 0, PNL_matured_pos_tot <= PNL_pos_tot
if snapshot not ready: resolved_payout_h_num = resolved_payout_h_den = 0
if snapshot ready: resolved_payout_h_num <= resolved_payout_h_den
```

The stored live-price invariants ensure price-move detection cannot be bypassed by a zero `P_last` or `fund_px_last`. The K and F future-headroom invariants are checked with exact wide arithmetic after every K/F mutation, including mark accrual, funding accrual, ADL K loss, epoch reset, and resolved terminal-delta preparation. They are liveness guards: a state that can accept the current mutation but cannot represent the next valid bounded mark or bounded funding accrual is not a valid live state.

### 2.3 Account materialization and freeing

Every external index MUST satisfy `i < cfg_account_index_capacity`. Missing/materialized status MUST come from authenticated engine state; omitted account data is not proof of missingness.

Only `deposit(i, amount > 0, now_slot)` may materialize a missing account. `materialize_account(i, materialize_slot)` initializes all fields to zero/canonical defaults, sets `last_fee_slot_i = materialize_slot`, and increments `materialized_account_count`.

`free_empty_account_slot(i)` is the only canonical free path. Preconditions:

```text
account materialized
C_i = 0, PNL_i = 0, R_i = 0
both buckets absent
basis_pos_q_i = 0
fee_credits_i <= 0
```

Effects: forgive fee debt by setting `fee_credits_i = 0`, reset local fields to canonical zero-position defaults, clear reserves and wrapper annotations, set `last_fee_slot_i = 0`, mark the slot missing/reusable in authenticated state, and decrement `materialized_account_count`. `neg_pnl_account_count` is unchanged.

### 2.4 Side reset lifecycle

For every materialized account with nonzero basis on side `s`, exactly one holds:

```text
epoch_snap_i == epoch_s
or mode_s == ResetPending and epoch_snap_i + 1 == epoch_s
```

`begin_full_drain_reset(side)` requires `OI_eff_side == 0` and `mode_side != ResetPending`, and then snapshots `K_side`/`F_side_num` to epoch-start fields, zeros live `K_side`/`F_side_num`, checked-increments `epoch_side`, sets `A_side = ADL_ONE`, sets `stale_account_count_side = stored_pos_count_side`, clears certified and potential phantom dust for that side, and enters `ResetPending`. Epoch overflow fails conservatively before any side-reset mutation is written. Calling `begin_full_drain_reset` on a side already in `ResetPending` is forbidden because it would overwrite the preserved epoch-start settlement state and move stale accounts more than one epoch behind.

`finalize_side_reset(side)` requires `ResetPending`, zero OI, zero stale count, zero stored position count, and zero certified/potential phantom dust, then sets mode to `Normal`.

Before any OI-increasing operation rejects on `ResetPending`, it MUST call `maybe_finalize_ready_reset_sides_before_oi_increase`.

---

## 3. Claims, haircuts, and equity

Let:

```text
Residual = V - (C_tot + I)
PosPNL_i = max(PNL_i, 0)
FeeDebt_i = max(-fee_credits_i, 0)
ReleasedPos_i = PosPNL_i - R_i on Live
ReleasedPos_i = PosPNL_i on Resolved
PendingWarmupTot = PNL_pos_tot - PNL_matured_pos_tot = sum R_i on Live
```

Canonical haircut pairs:

```text
if PNL_matured_pos_tot == 0: h = (1, 1)
else h = (min(Residual, PNL_matured_pos_tot), PNL_matured_pos_tot)

if PNL_pos_tot == 0: g = (1, 1)
else g = (min(Residual, PNL_pos_tot), PNL_pos_tot)
```

Then:

```text
PNL_eff_matured_i = floor(ReleasedPos_i * h.num / h.den)
PNL_eff_trade_i   = floor(PosPNL_i     * g.num / g.den)
```

Equity lanes, all exact wide signed:

```text
Eq_withdraw_raw_i = C_i + min(PNL_i,0) + PNL_eff_matured_i - FeeDebt_i
Eq_trade_raw_i    = C_i + min(PNL_i,0) + PNL_eff_trade_i   - FeeDebt_i
Eq_maint_raw_i    = C_i + PNL_i                            - FeeDebt_i
Eq_net_i          = max(0, Eq_maint_raw_i)

Eq_withdraw_no_pos_i = C_i + min(PNL_i,0) - FeeDebt_i
Eq_trade_no_pos_i    = C_i + min(PNL_i,0) - FeeDebt_i
```

The `*_no_pos` lanes are the canonical stress-shadow metrics for public paths while `hmax_effective_active(ctx)` is true. They ignore live positive-PnL credit because the residual may still be stale until the post-stress crank envelope clears.

Candidate trade approval MUST neutralize that trade's own positive slippage:

```text
TradeGain_i_candidate = max(candidate_trade_pnl_i, 0)
PNL_trade_open_i      = PNL_i - TradeGain_i_candidate
PosPNL_trade_open_i   = max(PNL_trade_open_i, 0)
PNL_pos_tot_trade_open_i = PNL_pos_tot - PosPNL_i + PosPNL_trade_open_i
compute g_open from PNL_pos_tot_trade_open_i and Residual
Eq_trade_open_raw_i = C_i + min(PNL_trade_open_i,0) + floor(PosPNL_trade_open_i*g_open.num/g_open.den) - FeeDebt_i
Eq_trade_open_no_pos_i = C_i + min(PNL_trade_open_i,0) - FeeDebt_i
```

`Eq_trade_open_raw_i` is the only compliant risk-increasing trade approval metric when the effective h-max lock is inactive. While `hmax_effective_active(ctx)` is true, public risk-increasing approval MUST use `Eq_trade_open_no_pos_i`; active stress, h-max admission, nonzero `stress_envelope_remaining_indices`, or incomplete sweep coverage MUST NOT be an additional rejection reason if the no-positive-credit approval metric and all other checks pass.

---

## 4. Reserve, PnL, fee, and insurance helpers

### 4.1 Capital and position setters

`set_capital(i, new_C)` computes the exact signed delta from the old `C_i`, applies it to a candidate `C_tot`, and writes `C_tot` and `C_i` atomically only after the enclosing instruction has also included any paired `V` or `I` mutation in the same candidate state. The candidate state MUST satisfy `C_tot <= V <= MAX_VAULT_TVL`, `I <= V`, and `V >= C_tot + I` before any write is committed. Every persistent mutation of `C_i` after materialization and before free MUST use `set_capital` or an exactly equivalent path that updates `C_tot` and proves those aggregate bounds in the same atomic step. Direct `C_i` writes are permitted only inside canonical materialization/free-slot reset when the account is entering or leaving authenticated materialized state and the aggregate count/capital effects are explicitly handled.

`set_position_basis_q(i, new_basis)` updates long/short stored position counts exactly once according to old/new sign flags, enforcing `cfg_max_active_positions_per_side` on any increment, then writes `basis_pos_q_i`. Every persistent mutation of `basis_pos_q_i` after materialization and before free MUST use this helper or an exactly equivalent path that updates stored side counts and side-capacity checks in the same atomic step. Direct `basis_pos_q_i` writes are permitted only inside canonical materialization/free-slot reset when the account is known to be missing or already aggregate-flat.

`clear_position_basis_q(i)` is the canonical live/resolved zero-position helper. It requires the caller has already settled any required A/K/F effects for the old basis, calls `set_position_basis_q(i, 0)`, and resets `a_basis_i`, `k_snap_i`, `f_snap_i`, and `epoch_snap_i` to canonical zero-position defaults. All settlement zeroing, resolved zeroing, liquidation full-close attach-flat, and cleanup branches that clear basis MUST use this helper or an exactly equivalent path.

`attach_effective_position_q(i, new_effective_pos_q)` is the canonical live nonzero-position attachment helper. It requires a live materialized account, `new_effective_pos_q != 0`, the account's old A/K/F effects already settled in the current instruction, and caller validation of the side-mode gating law in §5.6, position limits, OI after-values, and side-capacity constraints. Let `s = sign(new_effective_pos_q)`. The helper computes all local representation candidates before any write:

```text
new_basis_pos_q_i = new_effective_pos_q
new_a_basis_i     = A_s
new_k_snap_i      = K_s
new_f_snap_i      = F_s_num
new_epoch_snap_i  = epoch_s
```

It requires `A_s > 0`, current side state representability, and that `effective_pos_q(i)` computed from the candidates equals exactly `new_effective_pos_q`. It then atomically calls `set_position_basis_q(i, new_basis_pos_q_i)` and writes the candidate `a_basis_i`, `k_snap_i`, `f_snap_i`, and `epoch_snap_i`. A live attach that changes a nonzero effective position MUST use this helper or an exactly equivalent path. Calling `set_position_basis_q` alone is not a complete attach because it can leave fresh size anchored to stale ADL basis or stale K/F snapshots. `attach_effective_position_q` and `clear_position_basis_q` mutate only local account representation and stored-position counts; they MUST NOT mutate `OI_eff_long` or `OI_eff_short`. Trade writes global OI through the explicit bilateral OI after-values. Liquidation close quantity mutates global OI only through `enqueue_adl`, so local liquidation attach/clear cannot double-decrement OI.

### 4.2 Reserve bucket operations

`promote_pending_to_scheduled(i)` does nothing if scheduled exists or pending absent. Otherwise it creates a scheduled bucket from pending with `sched_start_slot = current_slot`, `sched_anchor_q = sched_remaining_q = pending_remaining_q`, `sched_horizon = pending_horizon`, `sched_release_q = 0`, and clears pending. It MUST NOT change `R_i`.

`append_new_reserve(i, reserve_add, admitted_h_eff[, ctx])` requires positive amount and positive horizon.

If called with an h-max-lock-active context, the new reserve MUST be placed in the pending bucket, regardless of whether a scheduled bucket already exists. It MUST NOT create a scheduled bucket, merge into an existing scheduled bucket, promote pending reserve, or otherwise start a release clock for the new reserve while the h-max lock is active. The h-max-lock-active rule is:

```text
if hmax_effective_active(ctx):
    require pending bucket capacity is available or pending already exists
    if pending absent: create pending with pending_remaining_q = reserve_add and pending_horizon = admitted_h_eff
    else: pending_remaining_q += reserve_add and pending_horizon = max(pending_horizon, admitted_h_eff)
    R_i += reserve_add
    return
```

If the h-max lock is inactive, normal reserve composition applies. If no scheduled bucket exists but pending exists, first promote pending. Then:

1. if scheduled absent, create scheduled at `current_slot`;
2. else if pending absent and `sched_start_slot == current_slot`, `sched_horizon == admitted_h_eff`, and `sched_release_q == 0`, merge into scheduled;
3. else if pending absent, create pending;
4. else merge into pending and set `pending_horizon = max(pending_horizon, admitted_h_eff)`.

Finally increase `R_i` by `reserve_add`.

`apply_reserve_loss_newest_first(i, reserve_loss)` consumes pending before scheduled, decrements `R_i`, and clears empty buckets.

`advance_profit_warmup(i, ctx)` first checks the final/effective h-max state. If `hmax_effective_active(ctx)` is true, it MUST NOT promote pending reserve, MUST NOT release any reserve, MUST NOT advance `sched_release_q`, and MUST NOT convert pending or scheduled reserve into matured PnL. This is an h-max-lock pause, not a horizon extension: elapsed slot time may continue to accrue, but pending reserve remains pending and scheduled release is withheld until the stress gate clears.

If `hmax_effective_active(ctx)` is false, it promotes pending if needed and then computes:

```text
elapsed = current_slot - sched_start_slot
effective_elapsed = min(elapsed, sched_horizon)
sched_total = floor(sched_anchor_q * effective_elapsed / sched_horizon)
sched_increment = sched_total - sched_release_q
release = min(sched_remaining_q, sched_increment)
```

It releases `release` to `PNL_matured_pos_tot`. If the scheduled bucket empties, it is cleared completely including `sched_release_q = 0`, and pending is promoted if present. A non-empty bucket MUST NOT persist with an over-advanced release cursor. An already-scheduled reserve paused by h-max lock MAY fully release on the first later touch after the h-max lock clears if its elapsed horizon has already completed; a pending reserve that was not promoted during h-max lock MUST NOT receive credit for a scheduled horizon that had not started.

`prepare_account_for_resolved_touch(i)` requires `Resolved`. If reserve storage is nonzero, it clears scheduled and pending buckets and sets `R_i = 0` without changing `PNL_i`, `PNL_pos_tot`, or `PNL_matured_pos_tot`. This is valid only because resolution sets `PNL_matured_pos_tot = PNL_pos_tot` globally before permissionless resolved closes can mutate account PnL.

### 4.3 Admission and h-lock state

`threshold_stress_gate_active(ctx)` is true iff all hold:

```text
ctx.admit_h_max_consumption_threshold_bps_opt_shared = Some(threshold_bps)
threshold_e9 = threshold_bps * STRESS_CONSUMPTION_SCALE
stress_consumed_bps_e9_since_envelope >= threshold_e9
```

The threshold input MUST already be validated so the multiplication cannot overflow. `None` disables only threshold-based stress activation; it does not disable bankruptcy h-lock. `Some(0)` is invalid.

`stress_gate_active(ctx)` / `hmax_lock_active(ctx)` is true iff either `threshold_stress_gate_active(ctx)` is true or `bankruptcy_hmax_lock_active == true`. `hmax_effective_active(ctx)` is true iff `stress_gate_active(ctx)` is true or the instruction-local h-max candidate described below is active. Endpoint-local positive-PnL usability and approval decisions MUST use `hmax_effective_active(ctx)` unless a rule explicitly refers only to persistent state. When the effective h-max lock is active, scheduled reserve release is paused and existing released positive PnL MUST NOT be auto-converted, explicitly converted, or used as positive credit for public withdrawal or risk-increasing approval until the reconciliation envelope has reset. The h-max lock selects h-max/zero-positive-credit lanes; it is not a blanket halt for fee-current settlements, liquidations, keeper cranks, or trades that pass those conservative lanes.

`trigger_bankruptcy_hmax_lock(ctx)` is called by any live bankruptcy event, including a liquidation account loss beyond principal (`D > 0`), a live post-principal loss absorbed through insurance/uninsured loss, or an equivalent protocol-loss path. It computes candidates before commit:

```text
bankruptcy_hmax_lock_active_candidate = true
stress_envelope_remaining_indices_candidate = cfg_account_index_capacity
stress_envelope_start_slot_candidate = Some(ctx.now_slot)       // or current_slot for no-caller-slot live recovery
stress_envelope_start_generation_candidate = Some(sweep_generation)
```

A bankruptcy event restarts the full authenticated reconciliation envelope even if `admit_h_min == 0`, even if threshold stress is disabled, and even if insurance fully covers the bankrupt account's immediate loss. If the instruction rolls back, the bankruptcy h-lock candidates roll back too.

Because a bankruptcy event can be discovered after earlier account touches in the same atomic instruction, every live context MUST maintain an instruction-local h-max candidate state. A compliant implementation MUST use one of these equivalent patterns:

```text
Pattern A: staged positive-PnL usability
    positive-claim increases, reserve releases/accelerations, conversions,
    auto-conversions, and positive-credit approvals are journaled but not
    committed until all endpoint bankruptcy/h-max candidates are known;
    then every journaled mutation is committed using the final instruction
    h-max state

Pattern B: conservative h-max for bankruptcy-capable instructions
    any instruction that can trigger bankruptcy admits all positive-claim
    increases at admit_h_max and disables reserve release/conversion for the
    whole instruction

Pattern C: retroactive reclassification
    if h-max activates or restarts after an earlier same-instruction h-min
    admission, reserve release, conversion, auto-conversion, or approval, the
    implementation recomputes the candidate from the pre-instruction state or
    reverses the same-instruction positive-PnL usability mutation before commit
```

If h-max is active or becomes active anywhere in the instruction, no same-instruction positive-PnL usability mutation may remain committed under the non-h-max policy. Fresh positive-claim increases MUST be placed into h-max reserve, preferably pending reserve so no release clock starts during the h-max lock. Existing reserve that was released or accelerated earlier in the same instruction MUST be restored to reserve with an h-max horizon, and any same-instruction conversion or auto-conversion of positive PnL MUST be undone before commit. Any health-sensitive approval already evaluated in the same instruction using positive-PnL credit MUST be recomputed under the zero-positive-credit lane, or the instruction MUST fail before commit. Public wrappers MUST NOT rely on account-touch ordering to avoid this rule.

`admit_fresh_reserve_h_lock(i, fresh_positive_pnl_i, ctx, admit_h_min, admit_h_max) -> admitted_h_eff` requires a live materialized account and valid admission pair. `admit_h_min` is the normal market h-lock floor and may be zero. Let:

```text
Residual_now = V - (C_tot + I)
```

Law:

1. if `i` is in `ctx.h_max_sticky_accounts`, return `admit_h_max`;
2. if `hmax_effective_active(ctx)` is true, choose `admit_h_max`;
3. otherwise choose `admit_h_min` iff `Residual_now > 0`, else choose `admit_h_max`;
4. if `admit_h_max` was chosen, insert `i` into the sticky set.

The engine enforces the supplied market h-lock policy. `admit_h_min = 0` means immediate release when `Residual_now > 0` and no h-max lock is active; it is a deliberate fair-exit policy tradeoff, not a safety failure. It does not require `PNL_matured_pos_tot + fresh_positive_pnl_i <= Residual_now`; the haircut pair remains the safety bound on payout/conversion. Active threshold stress, bankruptcy h-lock, any same-instruction h-max candidate, zero residual, and sticky h-max accounts override `admit_h_min` and choose `admit_h_max`.

`admit_outstanding_reserve_on_touch(i, ctx)` accelerates all outstanding reserve only when all hold. In a bankruptcy-capable live instruction this release MUST be staged or reversible under the final effective h-max rule above:

```text
reserve_total > 0
ctx.admit_h_min_shared == 0
!hmax_effective_active(ctx)
Residual_now > 0
```

If so it may move the entire reserve into `PNL_matured_pos_tot`, clear both buckets, and set `R_i = 0`, but the mutation is valid at commit only if the final instruction state still has `!hmax_effective_active(ctx)`. If h-max activates or restarts later in the same instruction, the release MUST be reversed/recomputed under h-max or the instruction MUST fail. This is the same immediate-release market-policy tradeoff as `admit_h_min = 0`; it does not require full residual coverage because haircut pairs bound extraction. Otherwise it leaves reserve unchanged. It never extends or resets a horizon.

### 4.4 PnL mutation

Every persistent `PNL_i` mutation after materialization MUST use `set_pnl`, except `consume_released_pnl`.

`set_pnl(i, new_PNL, reserve_mode[, ctx])` where reserve mode is:

```text
UseAdmissionPair(admit_h_min, admit_h_max)
ImmediateReleaseResolvedOnly
NoPositiveIncreaseAllowed
```

It updates `PNL_pos_tot`, `PNL_matured_pos_tot`, `R_i`, reserve buckets, and `neg_pnl_account_count` atomically. The full candidate update MUST be computed in exact arithmetic and validated before any local or aggregate field is written. After every candidate update, require:

```text
PNL_matured_pos_tot <= PNL_pos_tot
0 <= neg_pnl_account_count <= materialized_account_count
if Live: PNL_pos_tot <= MAX_PNL_POS_TOT_LIVE
```

Resolved-mode aggregate positive PnL MAY exceed the live guard and remains bounded by `u128` plus exact wide arithmetic.

Reserve modes govern **positive-claim increases**, not ordinary loss cleanup. In live mode, `UseAdmissionPair` MUST consult the instruction-local h-max candidate state described in §4.3. If a later mutation in the same instruction triggers or restarts h-max, every positive-claim increase already staged or tentatively admitted by that instruction MUST be recomputed or reclassified under `admit_h_max` before any aggregate write is committed. Reserve releases, reserve acceleration, conversion, auto-conversion, and positive-credit approvals in the same instruction are subject to the same recompute/reversal rule. This requirement applies to live touch settlement, trade execution PnL, funding/mark settlement, warmup release, outstanding-reserve acceleration, and any other live positive-PnL source or positive-PnL usability path.

Define:

```text
old_pos = max(old_PNL_i, 0)
new_pos = max(new_PNL, 0)
positive_claim_increase = new_pos > old_pos
```

If `positive_claim_increase` is true, the increase amount is `new_pos - old_pos` and:

- `NoPositiveIncreaseAllowed` fails;
- `ImmediateReleaseResolvedOnly` requires `Resolved`, increases `PNL_matured_pos_tot`, and does not reserve;
- `UseAdmissionPair` requires `Live`, obtains `admitted_h_eff`, immediately matures iff `admitted_h_eff == 0`, otherwise appends reserve, passing `ctx` so stress-active calls cannot promote pending reserve.

If `new_pos <= old_pos`, no admission is required. The positive-claim decrease, if any, consumes reserve loss newest-first, then matured positive PnL, updates aggregates and sign count, and requires no reserve remains when live positive PnL becomes zero. Movement inside the nonpositive region, including clearing negative PnL toward zero after the corresponding loss has been settled from capital or absorbed through insurance, is allowed under `NoPositiveIncreaseAllowed` because it does not create a positive junior claim.

`settle_negative_pnl_from_principal(i)` is the canonical loss-cleanup helper. If `PNL_i < 0`, it computes `pay = min(C_i, abs(PNL_i))`, reduces protected capital through `set_capital(i, C_i - pay)`, and calls `set_pnl(i, PNL_i + pay, NoPositiveIncreaseAllowed)`. Any remaining negative PnL may be cleared to zero with `set_pnl(..., NoPositiveIncreaseAllowed)` only after the same amount has been absorbed through `absorb_protocol_loss` or an exactly specified loss path.

`consume_released_pnl(i, x)` requires live `0 < x <= ReleasedPos_i`, decreases `PNL_i`, `PNL_pos_tot`, and `PNL_matured_pos_tot` by `x`, and leaves reserve unchanged.

### 4.5 Fees

Trading fee:

```text
fee = 0 if cfg_trading_fee_bps == 0 or trade_notional == 0
else ceil(trade_notional * cfg_trading_fee_bps / 10_000)
```

Liquidation fee for `q_close_q`:

```text
if q_close_q == 0: liq_fee = 0
else:
  closed_notional = floor(q_close_q * oracle_price / POS_SCALE)
  liq_fee_raw = ceil(closed_notional * cfg_liquidation_fee_bps / 10_000)
  liq_fee = min(max(liq_fee_raw, cfg_min_liquidation_abs), cfg_liquidation_fee_cap)
```

Fee/loss ordering is normative. If a fee is paid from an account's existing `C_i`, and that account is nonflat or may have unsettled A/K/F side effects, the caller MUST first perform authoritative live touch and `settle_negative_pnl_from_principal` for that account in the same instruction. Fee charging before touch is allowed only for accounts proven authoritatively flat/current, or for external fee-credit deposits that increase `V` rather than drawing from `C_i`. This rule applies to recurring fees, trade fees, liquidation fees, explicit account fees, and terminal resolved fee sweeps whenever they draw from account capital.

`charge_fee_to_insurance(i, fee_abs)` requires `fee_abs <= MAX_PROTOCOL_FEE_ABS`. It computes collectible headroom from capital plus fee-credit headroom, pays as much as possible from `C_i` into `I`, records any collectible shortfall as negative `fee_credits_i`, and drops the uncollectible tail. Any capital payment MUST update `C_i` through `set_capital` or an exactly equivalent `C_tot` update, and the combined candidate `C -> I` reclassification MUST prove `I <= V`, `V >= C_tot + I`, and `V <= MAX_VAULT_TVL` before commit. It MUST NOT mutate PnL, reserves, positive-PnL aggregates, or K/F indices. Its caller is responsible for satisfying the fee/loss ordering rule before invoking it.

`sync_account_fee_to_slot(i, anchor, rate)` charges recurring wrapper-owned fees exactly once over the half-open elapsed interval `[last_fee_slot_i, anchor)`, with `dt = anchor - last_fee_slot_i`. It requires `anchor >= last_fee_slot_i`, caps `rate * dt` at `MAX_PROTOCOL_FEE_ABS` without failing on raw-product overflow, routes the capped amount through `charge_fee_to_insurance`, and advances `last_fee_slot_i = anchor`. Live anchors must be `<= current_slot`; resolved anchors must be `<= resolved_slot`. On live nonflat/current-state paths, recurring fee sync MUST occur after authoritative touch/loss settlement and before health-sensitive checks, approvals, conversions, liquidations, or payouts.

`fee_debt_sweep(i)` pays `pay = min(C_i, FeeDebt_i)` from available capital into insurance by a single checked candidate reclassification equivalent to `set_capital(i, C_i - pay)`, `I += pay`, and `fee_credits_i += pay`. It MUST NOT make `fee_credits_i` positive, and it MUST prove the post-sweep aggregate bounds `I <= V`, `V >= C_tot + I`, and `V <= MAX_VAULT_TVL` before commit. This preserves `Residual` because it is a pure `C -> I` reclassification.

### 4.6 Insurance loss

`use_insurance_buffer(loss_abs)` MUST spend exactly `pay = min(loss_abs, I)`, set `I -= pay`, and return `loss_abs - pay`. It MUST NOT drain the full insurance fund when the loss is smaller.

`record_uninsured_protocol_loss(loss_abs)` may record telemetry but MUST NOT inflate `D`, `C_tot`, `PNL_pos_tot`, `PNL_matured_pos_tot`, `V`, or `I`. The loss remains represented by junior haircuts. In live mode, any direct call with `loss_abs > 0` from an account bankruptcy or protocol loss path MUST also ensure `trigger_bankruptcy_hmax_lock(ctx)` has been called in the same atomic instruction.

`absorb_protocol_loss(loss_abs)` calls `use_insurance_buffer` and records only the returned nonzero remainder. In live mode, any call with `loss_abs > 0` after the account's principal has been exhausted is a bankruptcy event and MUST call `trigger_bankruptcy_hmax_lock(ctx)` in the same atomic instruction before commit. Resolved-mode loss absorption does not affect live h-lock state.

---

## 5. A/K/F, accrual, ADL, and resets

### 5.1 Effective position

For account `i` with nonzero basis on side `s`:

```text
if epoch_snap_i != epoch_s: effective_pos_q(i) = 0
else effective_abs_pos_q = floor(abs(basis_pos_q_i) * A_s / a_basis_i)
effective_pos_q = sign(basis_pos_q_i) * effective_abs_pos_q
```

The exact bilateral trade OI after-values are:

```text
OI_long_after  = OI_eff_long  - old_long_a  - old_long_b  + new_long_a  + new_long_b
OI_short_after = OI_eff_short - old_short_a - old_short_b + new_short_a + new_short_b
```

They MUST be used for both gating and writeback.

### 5.2 Settlement of side effects

Canonical A/K/F settlement delta for side `s` is computed with exact signed floor arithmetic. For any settlement target pair `(K_target_s, F_target_s_num)`:

```text
abs_basis = abs(basis_pos_q_i)
den_q = a_basis_i * POS_SCALE
k_delta = K_target_s - k_snap_i
f_delta_num = F_target_s_num - f_snap_i
pnl_num = abs_basis * (k_delta * FUNDING_DEN + f_delta_num)
pnl_den = den_q * FUNDING_DEN
pnl_delta = signed_floor_div(pnl_num, pnl_den)
```

All products and differences are computed in exact wide signed arithmetic. `signed_floor_div` is mathematical floor division, not truncation toward zero. The side sign is already encoded in the side-specific `K_s` and `F_s_num` update laws; implementations MUST NOT apply an additional long/short sign to `pnl_delta`. This formula is what `wide_signed_mul_div_floor_from_kf_pair(abs_basis, k_snap, K_target_s, f_snap, F_target_s_num, den_q)` denotes.

Live touch settlement:

1. if basis is zero, return;
2. require `a_basis_i > 0` and compute `den = a_basis_i * POS_SCALE` exactly;
3. if current epoch, compute effective quantity and `pnl_delta` with `wide_signed_mul_div_floor_from_kf_pair(abs_basis, k_snap, K_s, f_snap, F_s_num, den)`;
4. apply `set_pnl(..., UseAdmissionPair(ctx...))`;
5. if effective quantity floors to zero, clear basis through `clear_position_basis_q(i)` and add at most one q-unit to the side's **potential** phantom-dust bound, capped by `OI_eff_side`; it MUST NOT increase the certified phantom-dust bound unless the same instruction performs an exact side scan or another exact proof that certifies the represented/phantom split; otherwise update snapshots.

Epoch-mismatch settlement requires `mode_s == ResetPending`, `epoch_snap_i + 1 == epoch_s`, and positive stale count. It settles against `K_epoch_start_s` / `F_epoch_start_s_num`, applies PnL through admission, clears basis through `clear_position_basis_q(i)`, and decrements stale count.

Resolved settlement first calls `prepare_account_for_resolved_touch`, then settles stale one-epoch-lag basis against:

```text
k_terminal_s_exact = K_epoch_start_s + resolved_k_terminal_delta_s
f_terminal_s_exact = F_epoch_start_s_num
```

using `ImmediateReleaseResolvedOnly`, then clears basis through `clear_position_basis_q(i)` and decrements stale count.

### 5.3 Accrual

`accrue_market_to(now_slot, oracle_price, funding_rate_e9_per_slot)` requires live mode, authenticated clock-owned `now_slot` for public paths, trusted `now_slot >= current_slot`, trusted `slot_last <= current_slot`, trusted `now_slot >= slot_last`, valid stored live prices, valid oracle price, and funding-rate magnitude within config. A caller-provided slot hint MUST NOT be used to create a larger `dt` than the authenticated clock permits. It MUST NOT be callable in a way that decreases `current_slot` after the caller writes the returned time. Bounded stale-catchup uses §8.7.1: `segment_slot` is used for K/F/price/`slot_last` accrual math, while `authenticated_now_slot` remains the instruction-time owner for sweep generation, fee/current-slot bounds, and h-max/stress start-slot comparisons.

Let:

```text
dt = now_slot - slot_last
funding_active = dt > 0 && funding_rate != 0 && OI_eff_long != 0 && OI_eff_short != 0 && fund_px_last > 0
price_move_active = P_last > 0 && oracle_price != P_last && (OI_eff_long != 0 || OI_eff_short != 0)
OI_long_0 = OI_eff_long
OI_short_0 = OI_eff_short
```

If either active branch is true, require `dt <= cfg_max_accrual_dt_slots`.

If `price_move_active`, before mutating any K/F/price/slot/consumption state, require exactly:

```text
abs(oracle_price - P_last) * 10_000 <= cfg_max_price_move_bps_per_slot * dt * P_last
```

Then compute transient stress candidates from every active equity-moving source; do not write the persistent stress fields yet:

```text
price_consumed_bps_e9 = 0
if price_move_active:
    price_consumed_bps_e9 = floor(abs_delta_price * 10_000 * STRESS_CONSUMPTION_SCALE / P_last)

funding_consumed_bps_e9 = 0
if funding_active:
    funding_consumed_bps_e9 = abs(funding_rate_e9_per_slot) * dt * 10_000

consumed_bps_e9 = saturating_add(price_consumed_bps_e9, funding_consumed_bps_e9)
stress_consumed_candidate =
    saturating_add(stress_consumed_bps_e9_since_envelope, consumed_bps_e9)
stress_remaining_candidate = stress_envelope_remaining_indices
stress_start_slot_candidate = stress_envelope_start_slot
stress_start_generation_candidate = stress_envelope_start_generation

if consumed_bps_e9 > 0:
    stress_remaining_candidate = cfg_account_index_capacity
    stress_start_slot_candidate = Some(now_slot)
    stress_start_generation_candidate = Some(sweep_generation)
```

The funding term is scaled bps because `funding_rate_e9_per_slot / FUNDING_DEN` is the per-slot fractional transfer rate. Its product MUST be computed in exact wide arithmetic and capped to `u128::MAX` before the saturating addition if necessary.

The accumulator is a stress signal, not a conservation quantity; overflow MUST saturate at `u128::MAX` and force slow-lane admission and no-positive-credit public approvals for finite thresholds until the full post-stress envelope has completed and an eligible reset clears it. A new nonzero price-move or funding consumption before reset restarts the required full envelope and resets the stress-start generation to the current `sweep_generation`. Because stress state is part of the accrual mutation, it MUST be committed only after all K/F candidates and future-headroom checks for the same accrual have succeeded.

Mark-to-market once, using transient candidates before any persistent write:

```text
ΔP = oracle_price - P_last
K_long_candidate  = K_long
K_short_candidate = K_short
if OI_long_0  > 0: K_long_candidate  = K_long  + A_long  * ΔP
if OI_short_0 > 0: K_short_candidate = K_short - A_short * ΔP
```

Funding, if active, also uses transient candidates before any persistent write:

```text
fund_num_total = fund_px_last * funding_rate_e9_per_slot * dt
F_long_candidate_num  = F_long_num
F_short_candidate_num = F_short_num
if funding_active:
    F_long_candidate_num  = F_long_num  - A_long  * fund_num_total
    F_short_candidate_num = F_short_num + A_short * fund_num_total
```

Persistent K/F overflow fails conservatively. Before writing any K/F/price/slot field, require the candidate K and F future-headroom invariants:

```text
abs(K_long_candidate)  + A_long  * MAX_ORACLE_PRICE <= i128::MAX
abs(K_short_candidate) + A_short * MAX_ORACLE_PRICE <= i128::MAX

funding_headroom_long  = A_long  * MAX_ORACLE_PRICE * cfg_max_abs_funding_e9_per_slot * cfg_max_accrual_dt_slots
funding_headroom_short = A_short * MAX_ORACLE_PRICE * cfg_max_abs_funding_e9_per_slot * cfg_max_accrual_dt_slots
abs(F_long_candidate_num)  + funding_headroom_long  <= i128::MAX
abs(F_short_candidate_num) + funding_headroom_short <= i128::MAX
```

The funding-headroom products MUST be computed in exact wide arithmetic. Then write the stress candidates and K/F candidates atomically. Finally set `slot_last = now_slot`, `P_last = oracle_price`, and `fund_px_last = oracle_price`. If any candidate computation or validation fails, none of the stress, K/F, price, or slot fields may be persisted.

### 5.4 ADL / bankrupt liquidation socialization

`enqueue_adl(ctx, liq_side, q_close_q, D)` uses checked arithmetic for every OI subtraction. Let `opp_side` be the side opposite `liq_side`, and let `OI_before = OI_eff_opp_side` before any opposing-side mutation.

The procedure:

1. requires `q_close_q <= OI_eff_liq_side` and decrements the liquidated-side OI by `q_close_q` with checked arithmetic;
2. if `D > 0`, treats the liquidation as a bankruptcy event and calls `trigger_bankruptcy_hmax_lock(ctx)` using the current instruction slot;
3. spends insurance exactly with `use_insurance_buffer(D)`, yielding `D_rem`;
4. if `OI_before == 0`, records any `D_rem` as uninsured, performs no opposing-side subtraction, schedules reset if both sides are zero, and returns;
5. before any opposing-side OI reduction, A decay, or K loss, requires `q_close_q <= OI_before`; otherwise the instruction MUST fail conservatively unless a separately specified dust/reset branch proves the excess is phantom-only and performs no account-affecting socialization;
6. if opposing stored position count is zero, sets `OI_eff_opp_side = OI_before - q_close_q` with checked arithmetic, records any `D_rem` as uninsured because no account can receive a K loss, sets both opposing certified and potential phantom-dust bounds to the remaining opposing OI because no stored account can represent it, schedules reset/phantom-dust cleanup as applicable, and returns;
7. otherwise computes opposing quantity decay, phantom-adjusted K loss, and post-ADL dust candidates as below.

Before K-loss allocation, compute the represented/phantom split. Certified dust is guaranteed nonrepresented. Potential dust is only an uncertainty bound; it is not a safe K-loss denominator proof unless an exact account scan certifies the represented/phantom split.

```text
old_certified_phantom = min(phantom_dust_certified_opp_q, OI_before)
old_potential_phantom = min(max(phantom_dust_potential_opp_q, old_certified_phantom), OI_before)
uncertified_potential = old_potential_phantom - old_certified_phantom
```

For `D_rem > 0` with opposing stored positions present, phantom OI is not a valid loss-bearing denominator. The implementation MAY use a tighter exact account scan. If no exact scan is supplied and `uncertified_potential > 0`, the represented denominator is uncertain; the instruction MUST route all `D_rem` to uninsured loss and MUST leave `K_opp` unchanged while still continuing deterministic quantity socialization. This conservative fallback is required so insolvency loss cannot disappear into uncertified phantom OI and so represented accounts are not overcharged from an unsafe guessed denominator.

If an exact represented/phantom scan is supplied, use the exact represented OI and exact phantom quantity from that scan only to split the loss budget between phantom and represented quantities:

```text
represented_oi_exact = sum effective_abs_pos_q(j) over all stored current-epoch accounts on opp_side
require represented_oi_exact <= OI_before
phantom_oi_exact = OI_before - represented_oi_exact
D_phantom_uninsured = ceil(D_rem * phantom_oi_exact / OI_before)
D_social_budget = D_rem - D_phantom_uninsured
```

`represented_oi_exact` is floor-rounded OI. It MUST NOT be used by itself as proof that a proposed K delta charges represented accounts by at most `D_social_budget`, because account settlement uses the exact A-basis weights in §5.2 and may include fractional exposure above the floor-rounded effective quantity.

Otherwise, when `uncertified_potential == 0`, use the certified split only to compute the represented socialized-loss budget:

```text
loss_bearing_oi_certified = OI_before - old_certified_phantom
if loss_bearing_oi_certified == 0:
    D_phantom_uninsured = D_rem
    D_social_budget = 0
else:
    D_phantom_uninsured = ceil(D_rem * old_certified_phantom / OI_before)
    D_social_budget = D_rem - D_phantom_uninsured
```

`D_phantom_uninsured` MUST be recorded as uninsured. If `D_social_budget > 0`, the implementation MAY allocate some or all of that budget through a K-loss write only with an exact worst-case settlement-loss upper-bound proof over the loss-bearing current-epoch account set. For a proposed `delta_K_abs > 0`:

```text
delta_K_exact = -delta_K_abs
K_candidate = K_opp + delta_K_exact

for each scanned loss-bearing account j:
    require epoch_snap_j == epoch_opp and a_basis_j > 0
    abs_basis_j = abs(basis_pos_q_j)
    den_j = a_basis_j * POS_SCALE

    // Worst-case additional rounded settlement loss caused by this negative K delta.
    // This is ceil(abs_basis_j * delta_K_abs / den_j), computed with exact wide arithmetic.
    // It is an upper bound over all possible existing and future K/F fractional states.
    k_loss_upper_bound_j = ceil(abs_basis_j * delta_K_abs / den_j)

K_loss_upper_bound = sum_j k_loss_upper_bound_j
require K_loss_upper_bound <= D_social_budget
D_social_unallocated_min = D_social_budget - K_loss_upper_bound
```

The scan/proof MUST include every current-epoch stored account that can settle against this K delta on the opposing side, or it MUST use a stronger formal proof that gives the same worst-case aggregate-loss upper bound. This upper-bound calculation is load-bearing: it uses exact A-basis weights and bounds the maximum extra rounded loss that the K delta can impose even if the account settles after later K/F mutations. A current-state before/after settlement comparison is non-compliant as a standalone proof because future fractional K/F state can increase the rounded loss attributable to the same negative K delta by up to one unit per affected account. An implementation SHOULD choose a deterministic maximal `delta_K_abs` satisfying this upper bound and the headroom checks, but it MAY choose `delta_K_abs = 0`; in that case all of `D_social_budget` is recorded as uninsured. Implementations MUST NOT use `ceil(D_social_budget * A_old * POS_SCALE / represented_oi_exact)` or any equivalent floor-OI denominator as a K write unless the exact worst-case settlement-loss upper-bound proof above also passes.

If `D_social_budget == 0`, no K candidate is formed. If `D_social_budget > 0` and no positive `delta_K_abs` with the required proof is chosen, route all of `D_social_budget` to uninsured loss and leave `K_opp` unchanged while still continuing quantity socialization. If a positive K candidate is chosen, the implementation MUST require representability and the future mark headroom check below:

```text
abs(K_candidate) + A_old * MAX_ORACLE_PRICE <= i128::MAX
```

If representability fails, that mark-headroom check fails, or the side's F future-headroom invariant would fail after the ADL writeback, route all of `D_social_budget` to uninsured loss and leave `K_opp` unchanged while still continuing quantity socialization. Otherwise set `K_opp = K_candidate`, record `D_social_unallocated_min` as uninsured, and do not record `K_loss_upper_bound` as uninsured. `K_loss_upper_bound` is an overcharge bound, not a promise that the full amount will be realized immediately. If signed-floor rounding later causes accounts to realize less than the upper bound, the remaining deficit stays represented by junior haircuts because the corresponding positive PnL claims remain outstanding until account settlement. Thus no part of `D_rem` can disappear into phantom OI, and represented accounts cannot be overcharged by a floor-rounded denominator or by an unsafe current-state proof.

Then compute the quantity-decay candidates and the post-ADL dust candidates before any write:

```text
OI_post = OI_before - q_close_q
A_candidate = floor(A_old * OI_post / OI_before)

if A_candidate > 0:
    // Certified dust is a lower bound. The closed quantity may have removed phantom first,
    // so only old certified dust beyond q_close_q can remain certified without a scan.
    if q_close_q >= old_certified_phantom:
        post_adl_certified_phantom_bound_q = 0
    else:
        post_adl_certified_phantom_bound_q = old_certified_phantom - q_close_q

    // Potential dust is an upper bound. Use the smallest represented-source value
    // consistent with the old potential bound, then add floor slack.
    represented_source_lower_bound_q = OI_before - old_potential_phantom
    represented_after_lower_bound_q = floor(represented_source_lower_bound_q * A_candidate / A_old)
    aggregate_gap_q = OI_post - min(OI_post, represented_after_lower_bound_q)
    account_floor_bound_q = stored_pos_count_opp_side
    post_adl_potential_phantom_bound_q = min(
        OI_post,
        checked_add(aggregate_gap_q, account_floor_bound_q)
    )
    post_adl_certified_phantom_bound_q = min(
        post_adl_certified_phantom_bound_q,
        post_adl_potential_phantom_bound_q
    )
```

`post_adl_certified_phantom_bound_q` is the no-scan lower bound on phantom OI after quantity decay. It MUST NOT be computed by scaling aggregate represented OI by `A_candidate / A_old`, because per-account floors can leave represented positions unchanged even when aggregate A decays; such a formula can over-certify and later clear real OI. The only certified dust that can survive without an exact scan is the part of old certified phantom that could not have been consumed by the closed quantity. A tighter exact account-scan result MAY be used instead.

`post_adl_potential_phantom_bound_q` is a conservative potential/recovery upper bound. It treats `OI_before - old_potential_phantom` as a lower bound on represented source quantity, carries the pre-ADL potential phantom through the A-decay, accounts for aggregate floor, and adds at most one q-unit of per-account floor slack. Potential dust MUST NOT be used for OI clearance. If `checked_add` overflows before the `min(OI_post, ...)` cap is applied, the instruction fails conservatively unless an exact account-scan result is used.

If `OI_post == 0`, zero opposing OI, set both certified and potential phantom dust for the opposing side to zero, and schedule reset. If `A_candidate > 0`, set `A_opp`, set `OI_eff_opp = OI_post`, set `phantom_dust_certified_opp_q = post_adl_certified_phantom_bound_q`, set `phantom_dust_potential_opp_q = max(post_adl_potential_phantom_bound_q, phantom_dust_certified_opp_q)`, and enter `DrainOnly` if `A_opp < MIN_A_SIDE`. If `A_candidate == 0` while `OI_post > 0`, zero both OI sides, set certified and potential phantom dust for both sides to zero, and schedule both resets as explicit deterministic ADL quantity socialization. The K-loss decision and the quantity-socialization decision are independent: failure to represent the K loss MUST NOT skip the deterministic quantity decay.

### 5.5 End-of-instruction reset scheduling

At the end of every top-level instruction that can touch accounts, mutate side state, liquidate, or resolved-close, call `schedule_end_of_instruction_resets(ctx)` exactly once, except for the additional explicit pre-open dust/reset flush inside `execute_trade`.

Certified phantom-dust bounds are consumable safety bounds, not reusable allowances. Any branch that clears residual OI by relying on `phantom_dust_certified_side_q` MUST either decrement that certified bound by the amount consumed or, more conservatively, clear the entire certified bound for that side. A consumed certified bound MUST NOT remain available to justify clearing unrelated future OI. Potential phantom bounds MUST NOT be used for OI clearance. Independently of whether a dust branch ran, if a side is aggregate-flat at instruction boundary (`stored_pos_count_side == 0 && OI_eff_side == 0`), both its certified and potential phantom-dust bounds MUST be zeroed before commit.

If both stored side counts are zero, any residual OI on either side is certified phantom because no stored account can represent it. If residual OI exists, require OI symmetry, then clear both OI sides and clear both certified and potential dust bounds. This branch MUST NOT require a pre-existing certified dust counter to cover the residual, because aggregate-flatness itself is the certification proof.

If exactly one stored side count is zero, let `e` be the empty side and `n` be the side with stored positions. The empty side's residual OI, if any, is certified orphan OI by aggregate-flatness because no current-epoch stored account can represent it. If residual OI exists, require OI symmetry. Then exactly one of the following branches MUST run in the same finalization:

1. **Certified-dust branch.** If the non-empty side's residual OI is also within its own certified phantom-dust bound,

```text
OI_eff_n <= phantom_dust_certified_n_q
```

both OI sides may be cleared as dust. The finalization MUST consume the certified dust bound used for side `n`, clear both certified and potential dust bounds for side `e`, and set side `n`'s pending reset flag unless side `n` is already `ResetPending`.

2. **Orphan-exposure drain reset.** If `OI_eff_n > phantom_dust_certified_n_q`, the non-empty side has represented exposure matched only by orphan OI on the aggregate-flat side. The finalization MUST NOT fail indefinitely and MUST NOT leave future mark/funding accrual running against the orphan side. Instead it MUST perform an explicit deterministic ADL quantity-socialization reset in the same instruction:

```text
require mode_n != ResetPending
capture K_n and F_n_num as the epoch-start state for side n
set OI_eff_e = 0 and OI_eff_n = 0
clear certified and potential phantom dust for side e
set side n's pending reset flag so finalize_end_of_instruction_resets begins a full drain reset for n
```

When `finalize_end_of_instruction_resets` begins the reset for side `n`, the stored current-epoch positions on `n` become one-epoch-lag stale positions and settle only against the preserved epoch-start K/F state. This branch is not ordinary phantom-dust clearance: it is explicit ADL quantity socialization of orphan-matched residual exposure. It MUST be visible protocol state and MUST be covered by tests. It MUST NOT be used unless the opposite side is aggregate-flat, residual OI is symmetric, and the non-empty side is not already `ResetPending`.

The empty side's orphan proof alone is not sufficient to silently clear the non-empty side as phantom dust, and a potential bound alone is never sufficient proof. The only compliant non-certified path is the explicit orphan-exposure drain reset above.

If a side is `DrainOnly` and its OI is zero, set that side's pending reset flag. If any branch leaves `stored_pos_count_side > 0` and `OI_eff_side == 0` while `mode_side != ResetPending`, that side MUST either have a pending reset flag set by the same finalization or the instruction MUST fail conservatively; future live accrual MUST NOT proceed with stored current-epoch positions hidden behind zero global OI. If `mode_side == ResetPending`, the side is already in the explicit stale-settlement path and no new pending reset flag is required.

`finalize_end_of_instruction_resets(ctx)` begins pending resets and finalizes any ready `ResetPending` side.

### 5.6 Side-mode gating for OI-increasing and fresh-attach operations

Side mode gating is evaluated per side using exact candidate local position changes and exact candidate global OI after-values. Before any operation that could increase OI or attach nonzero current-epoch exposure on side `s`, the implementation MUST call `maybe_finalize_ready_reset_sides_before_oi_increase` for any ready `ResetPending` side. If side `s` remains `ResetPending`, the operation MUST reject unless it is only stale settlement, reset finalization, or an explicitly specified recovery path.

For side `s`:

```text
mode_s == Normal:
    fresh opens, flips into the side, same-side increases, and OI increases MAY proceed subject to all other checks

mode_s == DrainOnly:
    OI_eff_s_after MUST be <= OI_eff_s_before
    no account may newly attach current-epoch exposure on side s
    no account may increase its absolute current-epoch exposure on side s
    only settlement, liquidation/ADL, exact close, partial close, and strictly reducing same-side changes are allowed

mode_s == ResetPending:
    no current-epoch attach, open, flip, increase, or OI increase is allowed
    only stale-account settlement, reset finalization, and explicitly specified recovery are allowed
```

A bilateral operation MUST apply these gates to the per-account candidate changes, not merely to net global OI. Replacing one account's closed `DrainOnly` exposure with another account's newly opened or increased exposure on the same side is non-compliant even if `OI_eff_s_after <= OI_eff_s_before`. `attach_effective_position_q` is only a local representation helper; its caller is responsible for proving this side-mode law before attachment. Liquidation close quantity remains OI-owned by `enqueue_adl` and does not authorize a fresh attach on a non-`Normal` side.

---

## 6. Live local touch and finalization

`touch_account_live_local(i, ctx)`:

1. requires live materialized account;
2. adds `i` to `ctx.touched_accounts` or fails on capacity;
3. calls `admit_outstanding_reserve_on_touch(i, ctx)`;
4. advances warmup with `advance_profit_warmup(i, ctx)`, which pauses reserve release while the h-max lock is active;
5. settles A/K/F side effects;
6. settles negative PnL from principal;
7. if now authoritative flat and still negative, calls `absorb_protocol_loss` and sets PnL to zero;
8. MUST NOT auto-convert or sweep fee debt.

`finalize_touched_accounts_post_live(ctx)` is an exactly-once finalization for a live local-touch context. It requires `ctx` not already finalized, requires every touched account still materialized and readable, computes one shared whole-haircut snapshot after all live local work, and marks the context finalized. After this call, no additional live local touches may be added to the same context.

It then iterates touched accounts in ascending storage-index order. If `hmax_effective_active(ctx)` is false and an account is flat, has released positive PnL, and the snapshot has `h = 1`, it uses `consume_released_pnl` followed by `set_capital(C_i + released)`. If `hmax_effective_active(ctx)` is true, it MUST skip this positive-PnL auto-conversion for every account. It then calls `fee_debt_sweep`. A touched account MUST NOT be paid out, closed, reclaimed, or freed until this finalization has either completed or been deliberately skipped because the context had no live local touches.

---

## 7. Margin and liquidation

After authoritative live touch:

```text
RiskNotional_i = 0 if effective_pos_q(i) == 0
else ceil(abs(effective_pos_q(i)) * oracle_price / POS_SCALE)

MM_req_i = 0 if flat else max(floor(RiskNotional_i * cfg_maintenance_bps / 10_000), cfg_min_nonzero_mm_req)
IM_req_i = 0 if flat else max(floor(RiskNotional_i * cfg_initial_bps / 10_000), cfg_min_nonzero_im_req)
```

Maintenance healthy iff `Eq_net_i > MM_req_i`. Withdrawal healthy iff `Eq_withdraw_raw_i >= IM_req_i` when the effective h-max lock is inactive, and iff `Eq_withdraw_no_pos_i >= IM_req_i` when `hmax_effective_active(ctx)` is true for a public path. For an actual nonflat withdrawal, these withdrawal lanes MUST be evaluated on the candidate post-withdrawal state with:

```text
C_i'      = C_i - amount
C_tot'    = C_tot - amount
V'        = V - amount
Residual' = V' - (C_tot' + I)
```

The local account equity term MUST use `C_i'`, not pre-withdrawal `C_i`. Risk-increasing trade approval healthy iff `Eq_trade_open_raw_i >= IM_req_post_i` when the effective h-max lock is inactive, and iff `Eq_trade_open_no_pos_i >= IM_req_post_i` when `hmax_effective_active(ctx)` is true for a public path. Active stress, h-max admission, or incomplete post-event sweep coverage MUST NOT be an additional rejection reason once the applicable lane and all ordinary checks pass.

A trade is risk-increasing if it increases absolute effective position, flips sign, or opens from flat. It is strictly risk-reducing if same sign, nonzero before/after, and absolute position decreases.

An account is liquidatable iff after full authoritative live touch it has nonzero effective position and `Eq_net_i <= MM_req_i`. If recurring fees are enabled, the account MUST be fee-current first.

Partial liquidation requires `0 < q_close_q < abs(old_eff_pos_q_i)`. It closes synthetically at oracle price, attaches the remaining position through `attach_effective_position_q`, settles losses from principal, charges liquidation fee, invokes `enqueue_adl(ctx, liq_side, q_close_q, 0)`, and requires the remaining nonzero position to be maintenance healthy after the step. The local attach MUST NOT mutate global OI; `enqueue_adl` is the sole OI decrement/socialization path for `q_close_q`.

Full-close liquidation closes the whole effective position at oracle price, attaches flat through `clear_position_basis_q`, settles losses from principal, charges liquidation fee, sets `D = max(-PNL_i, 0)`, invokes `enqueue_adl` if `q_close_q > 0 || D > 0`, then sets negative PnL to zero with `NoPositiveIncreaseAllowed` if `D > 0`. The local clear MUST NOT mutate global OI; `enqueue_adl` is the sole OI decrement/socialization path for `q_close_q`.

---

## 8. External operations

### 8.1 Standard live lifecycle

Live instructions that depend on current market state execute:

1. validate authenticated clock-owned `now_slot` (`now_slot >= current_slot`, `slot_last <= current_slot`, and `now_slot >= slot_last`), effective oracle price, funding-rate bound, admission pair, optional threshold (`None` disables; `Some(t)` requires `0 < t <= floor(u128::MAX / STRESS_CONSUMPTION_SCALE)`), and endpoint inputs;
2. initialize fresh `ctx`;
3. call `accrue_market_to` exactly once;
4. set `current_slot = now_slot`, preserving monotonic time;
5. endpoint preparation MUST make each health-sensitive or value-moving account authoritative before charging fees from its capital: touch/settle A/K/F and negative PnL first, then sync recurring fees for that account before health-sensitive checks, approvals, conversions, liquidations, or payouts;
6. run endpoint logic, using staged or recomputable positive-PnL admissions, reserve releases, conversions, and health approvals whenever the endpoint can trigger bankruptcy h-max;
7. call `finalize_touched_accounts_post_live(ctx)` exactly once under the final `hmax_effective_active(ctx)` state if live local touches were used and the endpoint has not already performed the required pre-payout/pre-free finalization; if the endpoint finalized early, that endpoint-local finalization MUST have used the final effective h-max state or be recomputed/reversible under it, and this step MUST observe `ctx` as already finalized and MUST NOT call finalization again;
8. schedule and finalize resets exactly once;
9. assert OI symmetry for side-mutating/live-exposure instructions;
10. require all applicable global invariants in §2.2, including `C_tot <= V <= MAX_VAULT_TVL`, `I <= V`, and `V >= C_tot + I`.

If `hmax_effective_active(ctx)` is true, endpoint logic MUST use the zero-positive-credit approval lanes defined in §3 for public withdrawal and risk-increasing approval, and MUST NOT release, auto-convert, or explicitly convert live positive PnL. The lifecycle MUST continue to allow partial keeper progress, liquidation revalidation, settlements, and trades that satisfy the applicable zero-positive-credit/dual-price lanes; the standard lifecycle MUST NOT require stress clearance before endpoint logic runs.

Any early no-op return after state mutation or fee sync MUST still perform the final applicable invariant checks.

Endpoint-local finalization is permitted only when the endpoint must evaluate a post-finalization condition before moving value or freeing an account, such as `withdraw` or `close_account`. That endpoint-local finalization consumes the standard lifecycle finalization for the context and MUST use the final `hmax_effective_active(ctx)` state. If h-max starts or restarts later in the same atomic instruction, any endpoint-local finalization that used non-h-max positive-PnL credit, auto-conversion, or released-PnL capital must be recomputed, reversed, or the instruction MUST fail before payout/free. Implementations MUST NOT finalize a context after a touched account has been paid out and freed, and MUST NOT finalize the same context twice.

### 8.2 No-accrual public path guard

Pure public live paths that advance `current_slot` without calling `accrue_market_to` MUST call:

```text
require_no_accrual_public_path_within_envelope(now_slot):
  require market_mode == Live
  require now_slot is the authenticated runtime/clock slot, not an unbounded caller value
  require now_slot >= current_slot
  require slot_last <= current_slot
  if OI_eff_long == 0 && OI_eff_short == 0: return
  dt = now_slot - slot_last
  require dt <= cfg_max_accrual_dt_slots
```

This avoids overflow-prone `slot_last + cfg_max_accrual_dt_slots` arithmetic and permits zero-OI idle fast-forward. Any no-accrual live path that accepts `now_slot`, syncs fees, materializes accounts, or writes `last_fee_slot_i` MUST call this guard before setting `current_slot = now_slot`; it MUST NOT create `last_fee_slot_i > current_slot`. A public exposed no-op liveness path that advances `current_slot` while no equity-active accrual is needed MUST either advance `slot_last` through a no-op `accrue_market_to` call or preserve a state for which the permissionless bounded catchup segment in §8.7.1 can later advance `slot_last` without decreasing `current_slot`. A wrapper MUST NOT let repeated no-accrual public paths create avoidable `slot_last` lag that can only be repaired by privileged intervention.

Public account-free catchup paths that do call `accrue_market_to` MUST first prove the call is not equity-active under §1.7. If the call would have `price_move_active == true` or `funding_active == true`, the path MUST reject before accrual and route through a protective account-touching lifecycle as defined in §1.7.

### 8.3 Pure capital / fee operations

All operations in this subsection are live-only no-accrual paths. If they accept `now_slot` and advance time, they MUST first call `require_no_accrual_public_path_within_envelope(now_slot)`, then set `current_slot = now_slot` before fee sync, materialization, or any write to `last_fee_slot_i`. They MUST also perform checked candidate accounting for every `V`, `I`, `C_tot`, `C_i`, and `fee_credits_i` mutation and reject before commit if the resulting state would violate any global invariant in §2.2.

`deposit(i, amount, now_slot)` may materialize missing `i` only if `amount > 0`. It computes the candidate vault inflow with checked arithmetic and requires `V + amount <= MAX_VAULT_TVL` before any materialization or capital write. It then increases `V`, increases protected capital through `set_capital(i, C_i + amount)` using the same candidate state, settles already-realized losses from principal, MUST NOT absorb flat negative loss through insurance, and sweeps fee debt only if the account is flat and nonnegative.

`deposit_fee_credits(i, amount, now_slot)` pays `x = min(amount, FeeDebt_i)` into `V` and `I`, increases `fee_credits_i` by `x`, and never makes fee credits positive. If `x > 0`, it MUST require checked candidates `V + x <= MAX_VAULT_TVL`, `I + x <= V + x`, and `V + x >= C_tot + I + x` before commit. Any `amount - x` excess MUST be rejected, ignored with no state mutation, or returned by the wrapper; it MUST NOT silently become vault capital or insurance top-up.

`top_up_insurance_fund(amount, now_slot)` increases `V` and `I` by `amount` only after checked candidate arithmetic proves `V + amount <= MAX_VAULT_TVL`, `I + amount <= V + amount`, and `V + amount >= C_tot + I + amount`.

`charge_account_fee(i, fee_abs, now_slot)` is a no-accrual explicit-fee path. It may charge from existing `C_i` only if the account is authoritatively flat/current. If `PNL_i < 0`, it MUST first settle the realized loss from principal and MUST reject rather than charging a fee if negative PnL remains. If the account has nonzero basis or possible unsettled side effects, explicit fee charging MUST use a standard live lifecycle that touches the account first. The operation performs no margin check by itself.

`settle_flat_negative_pnl(i, now_slot[, fee_rate])` is no-accrual, requires an authoritatively flat account with `basis_pos_q_i = 0` and no reserve, settles losses from principal first, absorbs any remaining negative PnL through insurance/uninsured loss and sets PnL to zero, and only then syncs fees if enabled.

`reclaim_empty_account(i, now_slot[, fee_rate])` is live-only, no-accrual, syncs fees if enabled, then requires the §2.3 free-slot preconditions and calls `free_empty_account_slot`.

### 8.4 User value-moving current-state operations

`settle_account` runs the standard live lifecycle, touches one account, syncs recurring fees after touch/loss settlement if enabled, and relies on the standard lifecycle finalization exactly once.

`withdraw` touches the account, syncs recurring fees after touch/loss settlement if enabled, and performs the single required live-context finalization under the final `hmax_effective_active(ctx)` state before evaluating or paying the withdrawal. This endpoint-local finalization consumes the standard lifecycle finalization; the lifecycle MUST NOT finalize the same context again after the payout. It then requires `amount <= C_i`. If the account is nonflat, it requires withdrawal health under the candidate post-withdrawal state where local `C_i`, global `C_tot`, and `V` all decrease by `amount`; the withdrawal equity lane MUST be recomputed with `C_i - amount`, not the pre-withdrawal `C_i`. During active stress, public wrappers MUST reject nonflat withdrawals unless the same post-withdrawal candidate check passes under `Eq_withdraw_no_pos_i`, ignoring positive-PnL equity. The payout decreases protected capital through `set_capital(i, C_i - amount)` and decreases `V` by the same amount.

`convert_released_pnl` touches the account, syncs recurring fees after touch/loss settlement if enabled, and requires `!hmax_effective_active(ctx)` after touch/loss settlement and before conversion. It requires `0 < x_req <= ReleasedPos_i`, computes current `h`, and for flat accounts requires `x_req <= max_safe_flat_conversion_released`, where the maximum is any exact value that preserves `V >= C_tot + I`, `PNL_matured_pos_tot <= PNL_pos_tot`, and all aggregate bounds after reducing the junior claim by `x_req` and increasing capital by `floor(x_req * h.num / h.den)`. It consumes released PnL, adds `floor(x_req * h.num / h.den)` to protected capital through `set_capital`, sweeps fee debt, and if still nonflat requires maintenance health.

`close_account` touches the account, syncs recurring fees after touch/loss settlement if enabled, and performs the single required live-context finalization under the final `hmax_effective_active(ctx)` state before checking close preconditions. This endpoint-local finalization consumes the standard lifecycle finalization; the lifecycle MUST NOT finalize the same context again after the account is paid out and freed. It requires flat, zero PnL, no reserve, and no fee debt, pays out all capital by setting capital to zero through `set_capital(i, 0)` and decreasing `V` by the same paid amount, then calls `free_empty_account_slot`.

### 8.5 Trade

`execute_trade(a,b, ..., size_q, exec_price)` requires distinct materialized accounts, valid execution price, positive size, computed `trade_notional <= MAX_ACCOUNT_NOTIONAL`, and standard live lifecycle.

It touches both accounts in deterministic ascending storage-index order, settling A/K/F effects and negative PnL from principal before any fee draw from account capital. It then syncs recurring fees if enabled, runs a pre-open dust/reset flush using a separate reset-only context, and finalizes ready reset sides before any OI-increasing decision. It then captures pre-trade positions and maintenance state, computes candidate positions and exact bilateral OI after-values, enforces position/OI bounds and the §5.6 side-mode gating law, applies execution-slippage PnL before trade fees, attaches nonzero positions through `attach_effective_position_q` and flat results through `clear_position_basis_q`, writes OI after-values, settles losses caused by the trade before charging trade fees, charges trade fees from the loss-settled accounts, computes post-trade risk notional and approval metrics on the fee-current state, and approves each account independently:

- flat result: fee-neutral negative-shortfall comparison must not worsen;
- risk-increasing: require `Eq_trade_open_raw_i >= IM_req_post_i` when the effective h-max lock is inactive; while `hmax_effective_active(ctx)` is true, require `Eq_trade_open_no_pos_i >= IM_req_post_i`; active stress or incomplete sweep coverage alone is not a rejection reason if this no-positive-credit metric and all other checks pass;
- already maintenance healthy: allow;
- strictly risk-reducing while unhealthy: allow only if fee-neutral maintenance shortfall strictly improves and fee-neutral negative equity does not worsen;
- otherwise reject.

### 8.6 Liquidate

`liquidate(i, ..., policy)` runs the standard live lifecycle. Its endpoint logic touches the account first, settles A/K/F effects and negative PnL from principal, syncs recurring fees if enabled, requires liquidation eligibility on the fee-current authoritative state, and executes `FullClose` or `ExactPartial(q_close_q)`. The standard lifecycle performs the single required finalization, reset scheduling/finalization, and conservation check; implementations MUST NOT accidentally run those final steps twice.

### 8.7 Keeper crank: incremental model

`keeper_crank(authenticated_now_slot, oracle_price, funding_rate, admit_h_min, admit_h_max, threshold_opt, ordered_candidates[], max_candidate_inspections, max_revalidations, rr_touch_limit, rr_scan_limit[, fee_fn])` is live-only. It accrues exactly once before both phases in ordinary mode, or exactly one bounded segment in stale-catchup mode as specified in §8.7.1. In §8.7, `authenticated_now_slot` is the runtime/clock slot for the top-level instruction. If §8.7.1 bounded stale-catchup uses a smaller `segment_slot`, `segment_slot` does not control Phase 2 wrap eligibility, `last_sweep_generation_advance_slot`, fee anchors, current-slot ownership, or h-max/stress start-slot comparisons.

A keeper crank is an incremental progress instruction. It MUST NOT require all exposed accounts, all unhealthy accounts, all suspect accounts, or all not-provably-solvent accounts to fit into the current instruction. If the supplied candidate list is full and more off-chain candidates are known, the instruction still processes the supplied valid candidates and leaves the rest for later cranks. Rejecting an otherwise valid crank solely because `known_bad_count > max_revalidations`, because a wrapper bitmap cannot cover every account, or because the wire format is capped is non-compliant. While the h-max envelope is incomplete, new positive PnL touched by the crank is admitted at h-max when the gate is active, but the crank continues to accrue and perform Phase 1/Phase 2 partial progress; h-max mode is not a reason to roll back a valid crank.

If the accrual in this crank is equity-active on an exposed public market, the instruction MUST perform at least one protective progress unit before it can commit, unless it resolves the market or zeroes all OI through an explicit reset/recovery path in the same instruction. A protective progress unit is one of: a materialized Phase 1 candidate touch/revalidation, a liquidation execution, an authenticated Phase 2 missing-slot inspection, or a materialized Phase 2 touch_account_live_local. If no such unit can be performed because of context capacity, cursor boundary, zero limits, or omitted account data, the equity-active crank MUST reject before committing accrual. This is not an all-or-nothing account coverage rule; it is only the minimum proof that the instruction is not account-free. Public crank-forward liveness therefore depends on the wrapper exposing the cursor-priority composition in this section and on honest keepers supplying the required authenticated cursor account data when the cursor slot is materialized.

Phase 1 processes keeper-supplied candidates in supplied order until `max_candidate_inspections` is exhausted, `max_revalidations` is exhausted, the candidate list ends, or a pending reset is scheduled. Every candidate entry inspected counts against `max_candidate_inspections`, including authenticated missing-account skips, duplicates, already-flat/nonliquidatable candidates, and malformed/out-of-range entries that are rejected before account touch. Authenticated missing-account skips do not count against `max_revalidations`. If a candidate slot is materialized, its account state MUST be available; omission/unreadability fails conservatively for that entry/instruction. For liquidation revalidation, the candidate MUST be touched first, then recurring fees are synced if enabled, and liquidation eligibility is evaluated on the authoritative fee-current state. Liquidation is Phase 1 only. Hitting `max_candidate_inspections` or `max_revalidations` is a normal partial-progress outcome, not a failed coverage proof. Public wrappers MUST set `max_candidate_inspections` to a finite compute-bounded value and MUST NOT allow an unbounded missing-account or duplicate-candidate scan to consume the instruction.

Phase 2 always runs, even if Phase 1 stopped on pending reset. It does not count against `max_revalidations`, does not liquidate, and does not stop merely because Phase 1 found too many candidates. If Phase 2 syncs recurring fees, it MUST do so only after `touch_account_live_local` has settled the account's A/K/F effects and negative PnL. It advances through authenticated index space in deterministic order and touches up to `rr_touch_limit` materialized accounts while inspecting at most `rr_scan_limit` indices. `rr_scan_limit` MUST NOT exceed `cfg_account_index_capacity` unless the implementation first clamps it to that value. Public wrappers that claim permissionless crank-forward liveness MUST expose a cursor-priority or cursor-only crank form that reserves enough account context for the current `rr_cursor_position` when that slot is materialized. Candidate-list accounts MUST NOT be allowed to crowd out the current cursor account on the cursor-priority path.

A full post-hmax-lock crank envelope means `cfg_account_index_capacity` authenticated index advances after the most recent nonzero stress consumption from price movement or active funding, or after the most recent bankruptcy h-max activation/restart. Missing slots count as covered only when authenticated engine state proves they are missing. A materialized slot counts as covered only if its account data is present and `touch_account_live_local` is called. Stopping because `rr_touch_limit`, `rr_scan_limit`, context capacity, or compute budget is reached leaves all uninspected later indices uncovered. Full-envelope coverage alone is not enough to clear h-max lock; h-max lock also requires an eligible slot-rate-limited `sweep_generation` advance after the stress-start generation and after the stress-start slot.

`sweep_generation` advancement is slot-rate-limited by `authenticated_now_slot`, not by any stale-catchup `segment_slot`. A cursor wrap may increment `sweep_generation` at most once per authenticated instruction slot. The market stores `last_sweep_generation_advance_slot`. Cursor movement itself MAY wrap without incrementing `sweep_generation`; the generation increment may occur only when:

```text
last_sweep_generation_advance_slot == None || authenticated_now_slot > last_sweep_generation_advance_slot
```

A wrap that occurs in the same authenticated instruction slot as `stress_envelope_start_slot` is not an eligible post-event generation advance and does not help clear h-max lock, even if it increments `sweep_generation` for ordinary cursor accounting. Same-instruction Phase 2 inspections after nonzero stress consumption or bankruptcy h-lock activation also do not reduce `stress_envelope_remaining_indices`. If accrual, a stale-catchup segment, Phase 1, or Phase 2 starts or restarts the reconciliation envelope during the current instruction, all Phase 2 progress from that same instruction is ineligible for the newly started envelope and any previously accumulated `stress_counted_inspected` candidate MUST be discarded before commit.

Phase 2 pseudocode:

```text
sweep_limit = cfg_account_index_capacity
i = rr_cursor_position
inspected = 0
stress_counted_inspected = 0
touched = 0
wrapped = false
scan_cap = min(rr_scan_limit, sweep_limit)
instruction_slot = authenticated_now_slot
wrap_allowed = last_sweep_generation_advance_slot == None ||
               instruction_slot > last_sweep_generation_advance_slot
reconciliation_envelope_started_this_instruction =
    reconciliation_envelope_started_or_restarted_by_accrual_or_phase1_candidate
    // true for any same-instruction nonzero stress consumption, threshold h-max,
    // bankruptcy h-lock, or bounded stale-catchup segment that starts/restarts
    // stress_envelope_remaining_indices

while inspected < scan_cap && touched < rr_touch_limit:
    if authenticated engine state proves missing at i:
        i += 1
        inspected += 1
    else:
        require account data for i
        touch_account_live_local(i)   // may trigger or restart h-max
        if touch_started_or_restarted_reconciliation_envelope(i):
            reconciliation_envelope_started_this_instruction = true
        i += 1
        inspected += 1
        touched += 1

    hmax_reconciliation_envelope_active_now =
        (stress_consumed_bps_e9_since_envelope_candidate > 0) ||
        bankruptcy_hmax_lock_active_candidate ||
        instruction_hmax_candidate_active
    same_slot_as_hmax_start_now =
        (stress_envelope_start_slot_candidate == Some(instruction_slot))

    if hmax_reconciliation_envelope_active_now:
        if reconciliation_envelope_started_this_instruction || same_slot_as_hmax_start_now:
            // No inspection in the same atomic instruction that starts/restarts
            // the stress/h-max envelope may count, including stale-catchup
            // segments where segment_slot < authenticated_now_slot.
            stress_counted_inspected = 0
            mark_reconciliation_progress_ineligible_for_this_instruction()
        else:
            stress_counted_inspected += 1

    if i == sweep_limit:
        i = 0
        wrapped = true
        break

rr_cursor_candidate = i
sweep_generation_candidate = sweep_generation
last_sweep_generation_advance_slot_candidate = last_sweep_generation_advance_slot

if wrapped && wrap_allowed:
    sweep_generation_candidate = checked_increment(sweep_generation)
    last_sweep_generation_advance_slot_candidate = Some(instruction_slot)

rr_cursor_position = rr_cursor_candidate
sweep_generation = sweep_generation_candidate
last_sweep_generation_advance_slot = last_sweep_generation_advance_slot_candidate
```

If `checked_increment(sweep_generation)` would overflow, the crank fails conservatively or enters a `permissionless_recovery_resolve` procedure before writing cursor, generation, last-advance-slot, or stress-reset state. The cursor, generation, last-advance-slot, and stress-reset candidates are written atomically only after all Phase 2 touch and reconciliation-progress checks succeed.

After cursor advancement, stress/h-max envelope reset progress is updated using only `stress_counted_inspected`:

```text
hmax_reconciliation_envelope_active_after =
    (stress_consumed_bps_e9_since_envelope_candidate > 0) ||
    bankruptcy_hmax_lock_active_candidate

if hmax_reconciliation_envelope_active_after &&
   stress_counted_inspected > 0 &&
   !reconciliation_progress_ineligible_for_this_instruction:
    if stress_envelope_remaining_indices_candidate > 0:
        stress_envelope_remaining_indices_candidate =
            stress_envelope_remaining_indices_candidate - min(stress_envelope_remaining_indices_candidate, stress_counted_inspected)

    generation_has_advanced_after_hmax =
        stress_envelope_start_generation_candidate != None &&
        sweep_generation_candidate > stress_envelope_start_generation_candidate &&
        last_sweep_generation_advance_slot_candidate != None &&
        last_sweep_generation_advance_slot_candidate > stress_envelope_start_slot_candidate

    if stress_envelope_remaining_indices_candidate == 0
       && stress_envelope_start_slot_candidate != Some(instruction_slot)
       && generation_has_advanced_after_hmax:
        stress_consumed_bps_e9_since_envelope_candidate = 0
        bankruptcy_hmax_lock_active_candidate = false
        stress_envelope_remaining_indices_candidate = 0
        stress_envelope_start_slot_candidate = None
        stress_envelope_start_generation_candidate = None
```

Consequences:

- K/F/price accrual may advance before every exposed account is touched, because lazy accounting plus junior positive-PnL haircuts, h-max admission, and stress shadowing provide safety. A wrapper MUST NOT impose an all-or-nothing exposed-account coverage predicate or h-max-clear predicate as a precondition to accrual.
- Suspect or liquidatable accounts that do not fit in the current candidate list remain lazy and can be supplied in later cranks; they cannot withdraw, convert, close, or increase risk without their own authoritative touch.
- H-max lock cannot clear until at least one full post-stress crank envelope has covered the authenticated account-index space with slot-rate-valid Phase 2 progress.
- H-max lock also cannot clear until `sweep_generation` has advanced at least once after `stress_envelope_start_generation` and in a later slot than `stress_envelope_start_slot`.
- A crank that consumes price movement, funding, threshold h-max, or bankruptcy h-lock activation in a slot MUST NOT use same-instruction Phase 2 progress to reduce the newly started envelope. A same-slot wrap, if otherwise allowed, does not qualify as the post-event generation advance. If the reconciliation envelope is started or restarted during Phase 2, any Phase 2 inspections already accumulated in that same instruction are discarded for h-max/stress clearing.
- Repeated same-slot cranks cannot advance `sweep_generation` more than once and cannot clear stress by repeated same-slot cursor wraps; they MAY still move `rr_cursor_position` through authenticated missing slots or materialized touches subject to `rr_scan_limit`, `rr_touch_limit`, and context capacity.
- If another nonzero price-move, funding consumption, or bankruptcy event occurs before reset, the full-envelope and generation-advance requirements restart from that later consumption and the then-current `sweep_generation`.
- `rr_touch_limit = 0` or `rr_scan_limit = 0` cannot make Phase 2 stress-reset progress.
- The h-max lock is both an admission-lane selector and a positive-PnL usability lock; it is not a blanket halt and it is not a substitute for choosing an explicit market h-lock policy.

### 8.7.1 Permissionless bounded stale-catchup segments

A public exposed market MUST NOT rely on privileged-only recovery when ordinary one-step live accrual is unavailable because `authenticated_now_slot - slot_last > cfg_max_accrual_dt_slots`. The public keeper path MUST include a bounded stale-catchup mode or an explicit public recovery/resolution path.

In bounded stale-catchup mode, the keeper computes one accrual segment before Phase 1/Phase 2:

```text
authenticated_now_slot >= current_slot
slot_last <= current_slot <= authenticated_now_slot
remaining_dt = authenticated_now_slot - slot_last        // checked subtraction
segment_dt = min(remaining_dt, cfg_max_accrual_dt_slots)
require segment_dt > 0
segment_slot = slot_last + segment_dt                    // checked, since segment_dt <= remaining_dt
```

This subtraction-first construction is normative. Implementations MUST NOT compute `slot_last + cfg_max_accrual_dt_slots` before capping by `authenticated_now_slot`; overflow of that unbounded sum is not a valid reason to stall permissionless catchup.

Time ownership is explicit:

```text
authenticated_now_slot:
    owns public slot validation, current_slot advancement, fee-anchor bounds,
    Phase 2 cursor/wrap eligibility, last_sweep_generation_advance_slot,
    protective-progress accounting, and h-max/stress start-slot comparisons

segment_slot:
    owns only this segment's K/F/price/fund_px_last/slot_last candidate and
    numeric segment_dt used for price/funding envelopes
```

If a bounded segment creates nonzero stress consumption or starts/restarts bankruptcy h-max, the reconciliation envelope MUST record `stress_envelope_start_slot_candidate = Some(authenticated_now_slot)`, not `Some(segment_slot)`, and the instruction-local `reconciliation_envelope_started_this_instruction` flag MUST be true. A segment may have `segment_slot < current_slot` if prior no-op time advancement left `current_slot` ahead of `slot_last`; that never permits a fee anchor, sweep-generation timestamp, or public-time field to move backward.

The wrapper computes the effective segment price from the raw target and `P_last` using the §1.7 clamp law with `dt = segment_dt`. If the public price-scale invariant is used, the segment price MUST remain at or above `MIN_EXPOSED_PROGRESS_PRICE`; if it cannot strictly move toward the authenticated raw target without violating that floor, the path MUST enter §8.8 permissionless recovery-resolution instead of no-op advancing `slot_last`.

The engine then applies a segment accrual equivalent to §5.3 with `accrual_slot = segment_slot` for K/F/price/`slot_last` mutation ordering and `authenticated_now_slot` for instruction-time ownership. The segment path uses `segment_dt = segment_slot - slot_last` for price/funding envelopes. After a successful segment it sets:

```text
slot_last = segment_slot
current_slot = authenticated_now_slot
```

If `authenticated_now_slot` remains greater than `slot_last` after the segment, later permissionless catchup cranks repeat the same bounded process. The ordinary public keeper path executes at most one equity-active bounded segment per instruction. A public instruction MAY execute multiple bounded segments only as a separately specified multi-segment routine with a finite configured segment cap; every equity-active segment MUST independently satisfy §5.3 candidate validation, future-headroom checks, h-max mutation ordering, and at least one distinct protective progress unit after that segment. Unbounded loops are forbidden.

If the segment's K/F candidate computation, representability check, or future-headroom check fails, the instruction MUST roll back the segment and route to `permissionless_recovery_resolve` for public liveness; it MUST NOT silently leave the market in a state that only privileged actors can advance.

Bounded stale-catchup is a liveness path, not a payout shortcut: while `slot_last < authenticated_now_slot`, target/effective divergence, h-max, reserve-release, positive-PnL, and zero-positive-credit restrictions remain in force for user value-moving and risk-increasing operations.

### 8.8 Permissionless recovery, resolution, and resolved close

#### 8.8.1 Permissionless terminal recovery-resolution

`permissionless_recovery_resolve(recovery_reason, now_slot[, authenticated_settlement_price, recovery_price_proof])` is the canonical public recovery path for a liveness profile that cannot complete ordinary bounded catchup. It is permissionless, but not caller-priced. It is a terminal liveness stop, not ordinary settlement pricing and not a user payout shortcut.

Preconditions:

```text
market_mode == Live
now_slot is the authenticated runtime/clock slot, not an unbounded caller value
now_slot >= current_slot
slot_last <= current_slot
now_slot >= slot_last
dt = now_slot - slot_last    // checked subtraction
0 < P_last <= MAX_ORACLE_PRICE
```

At least one recovery reason MUST be proven before any state mutation. Recovery reasons are deterministic state predicates, not caller labels. Caller omission, malformed price/account proofs, caller-chosen invalid segment inputs, or failure to supply data that an honest keeper can supply permissionlessly are not recovery predicates. All oracle, target, funding, recovery-price, and oracle-unavailability proofs required for these predicates MUST be permissionlessly obtainable or referenceable under fixed account/proof limits from authenticated wrapper or oracle state. A public liveness profile MUST NOT depend on a recovery proof that can be withheld by a caller, privileged actor, or unbounded account packing.

```text
BelowProgressFloor:
    exposed market, authenticated wrapper target != P_last, dt > 0, and the deterministic
    clamp law gives max_delta == 0 for the next permitted segment, or the next valid
    exposed effective price would fall below MIN_EXPOSED_PROGRESS_PRICE

SegmentBlocked:
    the next §8.7.1 bounded stale-catchup segment is computed from authenticated target,
    funding, slot, and market state exactly as an ordinary keeper segment would be,
    and that deterministic checked candidate fails representability, K/F future-headroom,
    or engine price validity for reasons independent of caller omission or malformed proof

GenerationOrEpochRecovery:
    a checked monotonic counter needed for permissionless crank/reset progress would overflow
    and the deployment has declared recovery resolution, not silent wrap, as the recovery procedure

OracleUnavailableRecovery:
    a stable authenticated wrapper predicate proves that the oracle/target/funding data
    required to compute the next bounded catchup segment or authenticated recovery price
    is unavailable, invalid, or stale under market policy; caller omission, malformed
    proof, or failure to include data that an honest keeper can supply is not this predicate
```

The recovery instruction MUST reject user value movement, positive-PnL conversion/release, and risk-increasing approvals in the same instruction. It MUST apply h-max/zero-positive-credit treatment to any local touches before terminal transition.

The recovery price is chosen by this deterministic fallback law:

```text
recovery_live_price = P_last

if wrapper state contains an authenticated recovery price under the public recovery oracle policy:
    require the caller supplies or references that bounded permissionless proof
    require 0 < authenticated_recovery_price <= MAX_ORACLE_PRICE
    require the price is not caller-selected
    if exact terminal-delta and signed-settlement representability checks pass:
        recovery_resolved_price = authenticated_recovery_price
    else:
        authenticated_recovery_price_unusable = true
else:
    authenticated_recovery_price_unusable = false

if recovery_resolved_price is unset:
    if cfg_recovery_p_last_fallback_enabled
       && (the authenticated `OracleUnavailableRecovery` recovery reason is true
           || authenticated_recovery_price_unusable is proven from the checks above
              independently of caller omission or malformed proof):
        recovery_resolved_price = P_last
    else:
        fail without mutation
```

A caller MUST NOT force `P_last` recovery by omitting account/proof data, supplying a stale or invalid proof, or choosing an unsafe optional branch while a representable authenticated recovery price exists. If a public CrankForward deployment does not enable the `P_last` fallback, initialization MUST prove that every valid authenticated recovery price under the public recovery oracle policy is terminal-representable; otherwise the deployment is `BestEffort`, not CrankForward. The `P_last` fallback is an emergency market-failure settlement mode fixed by immutable market policy, not a normal economic settlement and not a caller option. It is available only when `cfg_recovery_p_last_fallback_enabled` is true and the authenticated fallback predicate is proven from bounded, permissionlessly includable state. When it is used, the engine does not accrue additional mark movement or funding after the last committed effective accrual; this omission is explicit recovery semantics, not hidden accounting. Wrappers MUST disclose this emergency behavior and MUST keep extraction-sensitive actions on conservative lanes until resolved closes finish.

Effects are the same as terminal resolution below, except that no further live price or funding accrual is attempted. The path stores:

```text
resolved_price = recovery_resolved_price
resolved_live_price = recovery_live_price
resolved_slot = max(current_slot, now_slot)
current_slot = resolved_slot
slot_last = resolved_slot
P_last = recovery_live_price
fund_px_last = recovery_live_price
```

It then captures the pre-resolution side state, computes terminal K deltas from `resolved_price - resolved_live_price`, enters `Resolved`, clears live h-max/stress state, zeros both OI sides, and resets current-epoch stored positions into stale positions exactly as specified for the privileged branches below. If the authenticated recovery-price branch is unusable by exact representability checks, fallback to `recovery_resolved_price = P_last` is allowed only when the fallback is enabled and the branch failure is proven from authenticated state, not caused by caller omission or malformed proof. A CrankForward market MUST NOT fail into a privileged-only state merely because the authenticated terminal price is unrepresentable: it must either enable this fallback or prove at initialization that the unrepresentable-price case cannot occur. It also MUST NOT let callers downgrade a valid representable terminal price to `P_last`.

The path MUST NOT capture the positive-payout snapshot. Resolved payouts still require ordinary readiness, fee-current resolved closes, stale-account settlement, and negative-PnL reconciliation. A stricter public recovery path MAY exist, but it MUST either use the valid authenticated recovery price or prove an equivalent permissionless terminal state that cannot be blocked by price scale, stale time, K/F headroom, representability, candidate capacity, oracle-proof availability, or account-proof packing. Caller omission of required account or price-proof data is not a recovery predicate; account/proof packing liveness is handled by the cursor-priority crank requirements in §8.7 and the resolved-close proof-availability rule below.


#### 8.8.2 Privileged resolution

`resolve_market(resolve_mode, resolved_price, live_oracle_price, now_slot, funding_rate)` is privileged unless it is the permissionless recovery path above. Branch selection is explicit; value-detected branch selection is forbidden.

Ordinary branch first requires `now_slot >= current_slot` and `slot_last <= current_slot`, then calls `accrue_market_to(now_slot, live_oracle_price, funding_rate)`, sets `current_slot = now_slot`, and requires the resolved price to be inside the configured deviation band around the trusted live-sync price. On this branch, `live_oracle_price` is the effective live-sync price supplied to the engine; if the raw external target is beyond the live cap, feeding it directly will fail and the wrapper must first catch up through valid capped accruals or choose `permissionless_recovery_resolve`.

Degenerate branch requires `now_slot >= current_slot`, `slot_last <= current_slot`, `now_slot >= slot_last`, `live_oracle_price == P_last`, and `funding_rate == 0`, sets `current_slot = slot_last = now_slot`, uses `P_last` as the resolved live price, and skips the ordinary band. It is a privileged recovery path only. Permissionless below-floor or stale-dt recovery uses `permissionless_recovery_resolve` in §8.8, or a stricter public recovery path with equivalent oracle-authentication, h-max, terminal-delta, payout-readiness, and anti-extraction rules.

Both branches compute terminal K deltas from the pre-resolution, post-live-sync side state before any OI zeroing, side reset, A/K/F reset, epoch increment, or resolved-mode write. They also store `resolved_price` and `resolved_live_price` explicitly before entering `Resolved`:

```text
resolved_price = input resolved_price
resolved_live_price = live_oracle_price        // ordinary branch, equal to P_last after live accrual
resolved_live_price = P_last                   // degenerate branch
resolved_slot = now_slot
```

Capture the pre-resolution side state:

```text
pre_resolve_mode_long  = mode_long
pre_resolve_mode_short = mode_short
pre_resolve_A_long     = A_long
pre_resolve_A_short    = A_short
pre_resolve_OI_long    = OI_eff_long
pre_resolve_OI_short   = OI_eff_short
pre_resolve_stored_long  = stored_pos_count_long
pre_resolve_stored_short = stored_pos_count_short
resolve_delta_price    = resolved_price - resolved_live_price
```

Then the terminal deltas are exactly:

```text
resolved_k_long_terminal_delta =
    0 if pre_resolve_mode_long == ResetPending or pre_resolve_OI_long == 0
    else pre_resolve_A_long * resolve_delta_price

resolved_k_short_terminal_delta =
    0 if pre_resolve_mode_short == ResetPending or pre_resolve_OI_short == 0
    else -pre_resolve_A_short * resolve_delta_price
```

The products and signs MUST be computed in exact wide signed arithmetic and must fit `i128`. A side with zero effective OI, including a side already in `ResetPending`, MUST have zero terminal K delta; its stale accounts settle only against that side's stored `K_epoch_start_s` / `F_epoch_start_s_num`. A side with positive effective OI and not already `ResetPending`, including a `DrainOnly` side that still has positive effective OI, MUST receive the terminal settlement-price delta exactly once. Implementations MUST NOT compute terminal deltas after resetting `A_side`, zeroing OI, or incrementing the side epoch.

After storing the terminal deltas separately from live K, both branches enter `Resolved`, clear live stress state (`bankruptcy_hmax_lock_active = false`, `stress_consumed_bps_e9_since_envelope = 0`, `stress_envelope_remaining_indices = 0`, `stress_envelope_start_slot = None`, `stress_envelope_start_generation = None`), clear payout snapshot state, set `PNL_matured_pos_tot = PNL_pos_tot`, and zero both OI sides. For each side, if `pre_resolve_mode_s == ResetPending`, the branch MUST preserve that side's existing `epoch_s`, `K_epoch_start_s`, `F_epoch_start_s_num`, `stale_account_count_s`, and stored-position state and MUST NOT call `begin_full_drain_reset` again. If `pre_resolve_mode_s != ResetPending` and `pre_resolve_stored_s > 0`, the branch MUST begin exactly one drain reset for that side after OI is zeroed, using the captured pre-resolution live K/F state as the epoch-start state; this is what makes current-epoch stored positions become the one-epoch-lag stale positions consumed by resolved settlement. If `pre_resolve_mode_s != ResetPending` and `pre_resolve_stored_s == 0`, the branch MAY skip the reset or perform a zero-stale reset/finalization, but it MUST NOT leave nonzero stored positions in the current epoch. It then finalizes any ready reset sides as applicable and requires conservation.

Resolved positive-payout readiness is true only when all hold:

```text
market_mode == Resolved
stale_account_count_long == 0
stale_account_count_short == 0
stored_pos_count_long == 0
stored_pos_count_short == 0
neg_pnl_account_count == 0
PNL_matured_pos_tot == PNL_pos_tot
```

The shared positive-payout snapshot MUST be captured at most once, only after readiness is true, using the canonical haircut pair from the then-current `Residual` and `PNL_pos_tot`. If `PNL_pos_tot > 0`, the snapshot is:

```text
resolved_payout_h_num = min(Residual, PNL_pos_tot)
resolved_payout_h_den = PNL_pos_tot
```

If `PNL_pos_tot == 0`, no positive-payout snapshot is needed. Once captured, the snapshot MUST remain stable for all later positive resolved closes even as individual positive claims are closed and removed.

`force_close_resolved(i[, fee_rate])` is permissionless and takes no caller slot. Resolved account data and authenticated materialized/missing proofs needed to close stale, negative, zero, and positive accounts MUST be permissionlessly obtainable or includable under fixed account/proof limits. A public wrapper MUST expose a cursor-priority or account-priority resolved-close composition so an honest keeper can close any remaining stale or negative account over bounded calls; omission of a resolved account proof by one caller MUST NOT indefinitely block payout readiness for all winners. The operation requires `current_slot == resolved_slot`, prepares the account for resolved touch, settles resolved side effects, calls `settle_negative_pnl_from_principal(i)`, and finalizes ready reset sides. If `PNL_i < 0` after principal loss settlement, it MUST call `absorb_protocol_loss(abs(PNL_i))` and then clear the same negative amount with `set_pnl(..., NoPositiveIncreaseAllowed)` before any payout branch.

If recurring fees are enabled, `force_close_resolved` MUST then call `sync_account_fee_to_slot(i, resolved_slot, fee_rate)` before any `ProgressOnly` return, fee forgiveness, payout, capital free, or slot free. The resolved fee sync is after terminal side/loss settlement, so fees do not outrank same-account resolved losses, and before terminal payout/free, so accrued fee debt cannot be bypassed.

Then:

- if `PNL_i == 0`, fee-sweeps, forgives remaining fee debt, pays out capital by setting capital to zero through `set_capital(i, 0)` and decreasing `V` by the same paid amount, and frees the slot;
- if `PNL_i > 0` and the market is not positive-payout ready, returns `ProgressOnly` after the required resolved fee sync has been applied;
- if positive-payout ready, captures the shared payout snapshot if needed, computes `payout = floor(PNL_i * snapshot_num / snapshot_den)`, requires `payout <= Residual` before converting the claim to capital, writes `PNL_i = 0` through the canonical positive-claim decrease path so `PNL_pos_tot` and `PNL_matured_pos_tot` both decrease by the full positive claim, adds `payout` to protected capital via `set_capital(C_i + payout)` without changing `V`, fee-sweeps from the combined capital so collectible fee debt is not bypassed by direct payout, then pays out remaining capital by setting capital to zero through `set_capital(i, 0)` and decreasing `V` by the same paid amount, and frees the slot. Any unpaid haircut tail is extinguished with the PnL claim; it MUST NOT remain as account-local PnL.

A zero payout MUST NOT be the only encoding of progress-only.

---

## 9. Wrapper obligations

1. Public wrappers MUST NOT expose arbitrary caller-controlled `admit_h_min`, `admit_h_max`, stress-threshold, or funding-rate inputs.
2. Public or permissionless wrappers with untrusted live oracle, execution-price PnL, or live funding PnL MUST make `admit_h_min` a stable market h-lock policy, not arbitrary caller input. `admit_h_min = 0` is permitted only for markets that explicitly accept immediate-release semantics and no ordinary fair-exit guarantee among positive-PnL accounts while `Residual > 0` and not h-max locked. Active threshold stress or bankruptcy h-lock overrides `admit_h_min` and forces `admit_h_max`.
3. Stress threshold gating is optional engine machinery. It is a reconciliation/UX stress signal, not a substitute for warmup. If enabled for a public market, the threshold MUST be treated as stable market policy and passed consistently to every live instruction that can create, release, convert, withdraw against, or use live positive PnL.
4. Resolution is privileged. Wrappers MUST source trusted live and settlement prices, funding rate, and explicit `resolve_mode`.
5. Wrappers MUST monitor accrual envelopes and K/F future headroom, and crank, reset, or resolve before exposed markets approach a state where the next valid bounded live accrual could fail. Public wrappers MUST source `now_slot` from authenticated runtime/clock state, reject caller slot hints that would move engine time backward or forward beyond that authenticated clock, and enforce `now_slot >= current_slot` before writing `current_slot`, `slot_last`, or `last_fee_slot_i`.
6. Public wrappers MUST separate raw oracle target state from effective engine price state and MUST feed capped staircase prices, not cap-violating raw jumps, into exposed live accrual. Same-slot exposed cranks MUST pass the unchanged engine price. If exposed catch-up would have `target != P_last`, `dt > 0`, and `max_delta == 0`, a public liveness wrapper MUST make `permissionless_recovery_resolve` callable or use a stricter public recovery path; rejecting extraction/risk-increase alone is not sufficient. It MUST NOT advance `slot_last` with the unchanged price as a silent bypass and MUST NOT rely on privileged-only intervention.
7. While raw target and effective engine price differ, public wrappers MUST reject or conservatively shadow-check extraction-sensitive user actions (`withdraw`, `convert_released_pnl`, user-triggered settlement/finalization that can release or convert positive PnL, and any close path whose payout depends on lagged PnL) and MUST apply a stricter dual-price policy to risk-increasing user trades. A risk-increasing trade that passes the stricter dual-price policy, active-stress no-positive-credit checks when applicable, side-mode gating, OI/position limits, and fee-current margin checks MUST NOT be rejected solely because the target and effective engine price still differ.
8. Public wrappers MUST NOT provide an account-free catchup path that performs equity-active accrual on exposed markets. Any price-moving or funding-active catchup MUST be composed with a protective account-touching lifecycle as defined in §1.7.
9. Public wrappers using the sweep stress gate MUST pass nonzero `rr_touch_limit` and nonzero `rr_scan_limit` on normal keeper cranks. `rr_touch_limit = 0` or `rr_scan_limit = 0` is reserved for trusted/private compatibility or `permissionless_recovery_resolve` flows and cannot clear stress.
10. While `hmax_effective_active(ctx)` / `hmax_lock_active(ctx)` is true, public wrappers MUST reject manual positive-PnL conversion, MUST rely on the engine's skipped auto-conversion and paused warmup release, and MUST use the zero-positive-credit lanes (`Eq_withdraw_no_pos_i`, `Eq_trade_open_no_pos_i`) for extraction-sensitive actions and risk-increasing trades. They MUST NOT reject a fee-current trade, liquidation attempt, settlement, or keeper crank solely because h-max lock is active or the post-event envelope has not yet cleared, provided the applicable zero-positive-credit/dual-price lane and all ordinary safety checks pass.
11. Public wrappers MUST calibrate `admit_h_min`, `admit_h_max`, the stress threshold, `rr_touch_limit`, `rr_scan_limit`, and keeper incentives together: below-threshold movement must be safe under ordinary warmup, and above-threshold movement must remain non-extractable until a full post-stress envelope can complete.
12. Public wrappers SHOULD enforce execution-price admissibility, e.g. bounded deviation from effective engine price and, during oracle catch-up lag, from the raw target as well.
13. User value-moving operations must be account-authorized. Intended permissionless paths are settlement, liquidation, reclaim, flat-negative cleanup, resolved close, and keeper crank.
14. If recurring fees are enabled, wrappers MUST sync fee-current state after authoritative touch/loss settlement and before health-sensitive checks, reclaim checks, and resolved terminal close, and MUST use `resolved_slot` on resolved markets. A wrapper MUST NOT charge fees from a stale/nonflat account's capital before settling that account's current losses, and MUST NOT allow `force_close_resolved` to return `ProgressOnly`, forgive fee debt, pay capital, or free a slot before the resolved fee sync has run.
15. Wrappers own account-materialization anti-spam economics: minimum deposit, recurring fees, and reclaim incentives.
16. Runtime configuration MUST bound `max_revalidations + rr_touch_limit` to fit actual account context capacity, and MUST bound `rr_scan_limit` to fit compute budget while preserving eventual full-envelope coverage.
17. Public wrappers MUST reject deposits, fee-credit deposits, insurance top-ups, and any other external value inflow that would make the candidate vault state exceed `MAX_VAULT_TVL` or violate `C_tot <= V`, `I <= V`, or `V >= C_tot + I`.
18. Public wrappers and engine integrations MUST enforce side-mode gating per side. `DrainOnly` MUST NOT be treated as permission to recycle exposure by closing one account while opening or increasing another account on the same side, and `ResetPending` MUST NOT accept fresh/current-epoch exposure until finalized back to `Normal`.
19. Public wrappers using stress gates MUST treat `sweep_generation` as slot-rate-limited. They MUST NOT rely on repeated same-slot cursor wraps to clear stress or prove global reconciliation.
20. Public wrappers MUST NOT implement all-or-nothing suspect-account coverage checks that make a crank fail solely because more accounts are suspected or liquidatable than fit in one instruction. Candidate-set capacity limits must produce partial progress, not market freeze.
21. Public wrappers MUST expose and disclose the immutable `cfg_public_liveness_profile`. If it is `BestEffort`, the wrapper MUST NOT claim worst-case crank-forward liveness. If it is `CrankForward`, the wrapper MUST enforce the §1.5 ordinary progress layer and the terminal recovery layer: non-catchupable states must either be impossible by formal deployed-bound proof or must route to `permissionless_recovery_resolve` in §8.8, or a stricter public recovery path with equivalent safety. It MUST NOT deploy a public exposed market whose price scale, stale time, oracle proof, K/F headroom, representability, or counter state can deadlock while the only escape is privileged intervention.
22. Public `CrankForward` wrappers MUST expose bounded stale-catchup composition for `dt > cfg_max_accrual_dt_slots`, and MUST expose `permissionless_recovery_resolve` for states where bounded catchup cannot proceed because of below-floor price scale, authenticated oracle/target/funding unavailability, K/F headroom, representability, counter overflow, or separately specified recovery predicates, unless initialization formally proves those states impossible. Privileged degenerate resolution alone is not sufficient for public crank-forward liveness.
23. Public recovery-price or oracle-unavailable proofs MUST be permissionlessly obtainable/referenceable under fixed proof limits. Wrappers MUST prevent callers from forcing or blocking `P_last` recovery by omitting, corrupting, or withholding an otherwise valid authenticated recovery-price proof. If `cfg_recovery_p_last_fallback_enabled` is true, wrappers MUST disclose that this is emergency market-failure settlement that skips unaccrued post-`P_last` mark/funding by design. If it is false for a CrankForward market, wrappers MUST prove valid recovery prices are always terminal-representable under the public recovery oracle policy.
24. Public keeper ABIs MUST bound candidate entries inspected (`max_candidate_inspections`) independently from successful revalidations, and MUST expose a cursor-priority or cursor-only Phase 2 crank form that reserves account context for the current cursor slot when materialized. Account storage/proof design MUST allow an honest keeper to fetch and supply that cursor account data permissionlessly.
25. Public wrappers MUST expose resolved-close proof/account packing that lets honest keepers permissionlessly close remaining stale, negative, zero, and positive accounts over bounded calls. One unresolved account MUST NOT stay unreachable because only a privileged actor can obtain or include its resolved account proof.
26. Public wrappers MUST NOT implement stress-sweep blanket trade halts. During active stress or incomplete post-event sweep coverage, they MUST keep at least one public trade path live for orders that pass no-positive-credit margin, target/effective dual-price checks when relevant, side-mode gating, OI/position limits, fee-current checks, and authorization.

---

## 10. Required test coverage

Implementations and public wrappers MUST test at least:

1. conservation `V >= C_tot + I` across all paths;
2. PnL aggregate and `neg_pnl_account_count` consistency;
3. reserve admission, sticky `admit_h_max`, pending/scheduled behavior, reserve loss ordering, resolved reserve cleanup, and no stale release cursor;
4. public-wrapper policy tests that `admit_h_min` is stable market configuration, not caller-controlled input; tests cover markets with `admit_h_min = 0` immediate-release policy while `Residual > 0`, including cases where matured-plus-fresh positive PnL exceeds residual, and confirm bankruptcy/threshold h-lock or zero residual still forces `admit_h_max`;
5. outstanding reserve acceleration, pending-reserve promotion, natural reserve release, manual conversion, and flat auto-conversion follow the configured `admit_h_min` policy and are blocked by active threshold stress or bankruptcy h-max lock; stress-active fresh positive PnL is placed into pending reserve, never scheduled reserve, even when no scheduled bucket exists or when a same-slot scheduled bucket could otherwise accept a merge;
6. exact candidate-trade positive-slippage neutralization;
7. fee-debt definition, half-open fee-slot charging, no stale fee anchors on no-accrual paths, loss-senior fee ordering on stale/nonflat accounts, fee-debt sweep residual neutrality, and actual-fee-impact comparisons;
8. `RiskNotional` ceil margin including fractional-notional dust;
9. exact per-risk-notional init envelope including funding fractions, post-move liquidation notional, fee floor, fee cap, and rounded notionals;
10. price-move cap rejection before any K/F/price/slot/consumption mutation;
11. wrapper oracle catch-up clamp: raw target is stored separately, next effective price moves toward target by at most `floor(P_last * cap * dt / 10_000)`, and same-slot exposed cranks pass `P_last`;
12. target/effective-price divergence policy: public risk-increasing trades and extraction-sensitive actions are rejected or pass a stricter dual-price shadow check;
13. account-free catchup rejects exposed price movement and active funding, while still allowing flat/no-op catchup;
14. equity-active catchup cannot claim global reconciliation through a single arbitrary settlement; h-max clear requires authenticated Phase 2 envelope coverage and an eligible slot-rate-limited generation advance; public equity-active keeper cranks reject if they would commit with zero protective progress units;
15. account-limited cascade liveness: with more simultaneously liquidatable/suspect accounts than fit in one candidate list, the first valid keeper crank accrues and processes its bounded subset, later cranks process additional subsets, and no all-or-nothing uncovered-account predicate freezes K/F advancement;
16. zero-OI no-accrual fast-forward and exposed-market no-accrual envelope rejection using checked subtraction near `u64::MAX`;
17. exact insurance spending `min(loss_abs, I)`;
18. stress accumulator floor-at-scaled-bps precision, saturating addition, threshold activation, bankruptcy h-lock activation independent of threshold configuration, restart on new nonzero price/funding consumption or bankruptcy event, paused reserve release/conversion during h-max lock, no reset before a full post-event crank envelope, no same-slot h-lock clear, and no h-lock clear without an eligible `sweep_generation` advance after `stress_envelope_start_generation` and `stress_envelope_start_slot`;
19. deterministic Phase 2 cursor arithmetic over `cfg_account_index_capacity`, authenticated missing-slot skips, materialized-slot touch requirements, `rr_touch_limit`, `rr_scan_limit`, cursor wrap handling, failure on omitted materialized account data, and `sweep_generation` advancing at most once per slot;
20. public keeper wrappers using the stress gate pass nonzero `rr_touch_limit` and nonzero `rr_scan_limit` on normal cranks and enforce touched-account/context and scan/compute budgets;
21. deterministic ascending trade touch order, pre-open dust/reset flush before pre-trade capture, explicit trade-fee charging after trade-induced loss settlement, fee-current post-trade approval metrics, and active-stress rejection or stress-shadowing of risk-increasing approvals;
22. all protected-capital mutations through `set_capital`, all position-basis mutations through `set_position_basis_q`, aggregate consistency for `C_tot` and stored side counts after every deposit/withdraw/convert/trade/liquidation/settlement/resolved close, canonical nonzero position attachment through `attach_effective_position_q`, canonical zeroing through `clear_position_basis_q`, no OI mutation inside local attach/clear helpers, liquidation OI decrement exactly once through `enqueue_adl`, and all frees through `free_empty_account_slot`;
23. resolved payout readiness, shared snapshot stability, and explicit progress-vs-close outcome;
24. degenerate resolution requires explicit mode and exact degenerate inputs; ordinary resolution never value-detects into degenerate mode;
25. ADL K-loss accounting, phantom-adjusted loss-budget splitting, exact worst-case settlement-loss upper-bound K writes, uninsured recording for phantom and deterministic unallocated loss shares, overflow fallback to uninsured loss while quantity socialization continues, and certified-versus-potential phantom-dust distinction, including that certified ADL dust is computed from an upper bound on represented OI and cannot be `min(old_certified, OI_post)` when represented OI remains;
26. self-neutral insurance/oracle-siphon scenarios across multiple valid accrual envelopes;
27. exposed `target != P_last`, `dt > 0`, `max_delta == 0` cannot advance `slot_last` by feeding `P_last`; it must wait, reject as catch-up-required, or enter `permissionless_recovery_resolve`;
28. raw target jumps beyond the cap are never fed directly to exposed live engine accrual except in a `permissionless_recovery_resolve`/resolution test that confirms conservative failure, privileged recovery semantics, or the separately specified permissionless recovery-resolution semantics;
29. h-max mode remains active if `rr_touch_limit = 0`, `rr_scan_limit = 0`, if the cursor only wraps over a suffix after the stress consumption, or if repeated same-slot wraps occur without an eligible generation advance;
30. public wrappers cannot no-op accrue exposed catch-up by feeding `P_last` when `target != P_last`, `dt > 0`, and `max_delta > 0`;
31. resolved positive-payout snapshot cannot be captured until both stale counts, both stored position counts, and `neg_pnl_account_count` are zero, and remains stable after capture;
32. ADL/OI subtractions fail conservatively on underflow and fee/insurance reclassifications update `C_tot`, `I`, and `V` consistently;
33. resolved negative close settles protected capital before insurance, resolved terminal close syncs recurring fees to `resolved_slot` before `ProgressOnly`, payout, fee forgiveness, or free, and fee charging, fee-debt sweeping, and resolved positive payout update `C_tot`, `I`, `fee_credits_i`, PnL aggregates, and `V` exactly;
34. stress-shadow withdrawal and risk-increasing trade tests use `Eq_withdraw_no_pos_i` and `Eq_trade_open_no_pos_i`, and nonflat withdrawal tests recompute the withdrawal lane with post-withdrawal local `C_i - amount`;
35. every K mutation preserves future mark headroom, every F mutation preserves future funding headroom, and candidate K/F updates are computed and validated before persistent writes;
36. standard live lifecycle, account-free catchup, no-accrual paths, bounded catchup, permissionless recovery, and both privileged resolution branches use authenticated clock-owned `now_slot`, reject unbounded caller future slots, reject slots that would decrease `current_slot`, and never create `last_fee_slot_i > current_slot`;
37. stored live prices (`P_last`, `fund_px_last`) are always nonzero after initialization and after every accrual/resolution path, so price-move and funding detection cannot be bypassed by zero sentinels;
38. side epoch and `sweep_generation` increments fail conservatively or enter `permissionless_recovery_resolve` on overflow;
39. accrual stress candidates are not persisted if K/F candidate validation fails, and ADL opposing-side branches fail conservatively on checked-subtraction underflow before any account-affecting socialization;
40. `set_pnl(..., NoPositiveIncreaseAllowed)` permits settled negative-loss cleanup but rejects any increase in positive junior claim;
41. live touched-account finalization is exactly once: `withdraw` and `close_account` consume the standard lifecycle finalization when they finalize before payout/free, the lifecycle skips any second finalization, and no finalization can run over a freed touched account;
42. resolved terminal K deltas are computed before OI zeroing and side reset: exposed long and short sides receive exactly the signed `A_side * (resolved_price - resolved_live_price)` terminal mark delta, zero-OI or already reset-pending sides receive zero terminal delta, already `ResetPending` sides are not reset again, and non-reset-pending sides with stored positions are reset exactly once so current-epoch positions become resolved stale positions;
43. direct writes to `C_i` or `basis_pos_q_i` outside canonical materialization/free-slot reset are rejected in code review/tests, and helper-equivalent paths prove the same aggregate deltas and capacity checks as `set_capital` and `set_position_basis_q`;
44. nonzero live position changes cannot inherit stale representation state: opening, increasing, decreasing, flipping, partial liquidation attach, and post-trade attach set `a_basis_i = A_s`, `k_snap_i = K_s`, `f_snap_i = F_s_num`, and `epoch_snap_i = epoch_s` atomically with the basis/count update, and the resulting `effective_pos_q(i)` equals the intended attached effective position exactly;
45. nonflat withdrawal approval fails in tests when the account is healthy under pre-withdrawal `C_i` but unhealthy after replacing `C_i` with `C_i - amount`, including the active-stress shadow lane;
46. A/K/F settlement uses the exact signed-floor formula in §5.2 for live, reset-pending, and resolved settlement, including negative PnL deltas, funding deltas, and terminal K deltas; truncation toward zero, omitted `FUNDING_DEN`, or double-applying long/short signs is rejected;
47. phantom/orphan OI cleanup consumes the certified dust bounds it uses, aggregate-flat sides certify their own residual orphan OI and clear certified/potential dust bounds, potential dust alone cannot clear OI, account-local floor-to-zero settlement adds potential rather than certified dust unless an exact side proof is supplied, ordinary dust cleanup cannot zero a non-empty side's OI unless that side's own residual OI is within its own certified phantom-dust bound, and the one-empty-side/non-certified case performs the explicit orphan-exposure drain reset rather than failing indefinitely or silently wiping positions;
48. deposits, fee-credit deposits, insurance top-ups, PnL-to-capital conversions, fee sweeps, withdrawals, resolved payouts, and helper-equivalent aggregate mutations reject before commit if candidate checked arithmetic would exceed `MAX_VAULT_TVL` or violate `C_tot <= V`, `I <= V`, or `V >= C_tot + I`; tests include boundary cases at `MAX_VAULT_TVL`, `I == V`, and `Residual == 0`;
49. side-mode gating is enforced per side and per account: `DrainOnly` allows strictly reducing or clearing already-stored same-side exposure but rejects fresh opens, flips into the side, same-side increases, and exposure replacement where one account closes while another opens/increases on the same side; `ResetPending` rejects all fresh/current-epoch exposure until finalized back to `Normal`; `Normal` remains subject to all ordinary margin/OI/capacity checks;
50. ADL quantity-socialization dust accounting updates the opposing side's post-ADL certified survivor bound and potential dust bound by the §5.4 formula; tests reject implementations that leave the old bound unchanged, blindly add `stored_pos_count`, over-certify old dust with `min(old_certified, OI_post)`, certify aggregate A-decay gaps that can be explained by per-account floor stickiness, under-bound potential dust and strand residual OI, promote account-floor potential dust into certified OI-clearance authority without an exact scan, or reach a one-sided aggregate-flat residual state that neither clears by certified dust nor performs the explicit orphan-exposure drain reset;
51. ADL K-loss allocation excludes certified phantom OI from the loss-bearing set, refuses unsafe K allocation when uncertified potential dust remains, and bounds any exact-scan K write by worst-case aggregate settlement-loss upper bound: tests cover nonzero certified phantom share, uncertified potential dust fallback routing all uncertain loss to uninsured without K mutation, all-phantom OI, exact account-scan override, uninsured phantom loss recording, K-overflow fallback recording the socialized share as uninsured while still performing quantity socialization, and floor-rounded represented OI cases where `ceil(D * A * POS_SCALE / sum_effective_abs)` would overcharge accounts and is rejected unless the exact `K_loss_upper_bound <= D_social_budget` proof passes; tests also reject current-state before/after settlement proofs that understate future rounded loss from the same K delta;
52. same-slot repeated keeper cranks cannot clear h-max lock by repeatedly wrapping the cursor; `sweep_generation` advances at most once per slot, same-slot h-max wraps do not count as eligible generation advances, and h-max clear requires both full envelope coverage and a generation advance after `stress_envelope_start_generation` and after `stress_envelope_start_slot`;
53. more-than-cap liquidations are live: with 129, 1,000, or any number of liquidatable accounts above the candidate cap, a sequence of bounded keeper cranks can advance K once, liquidate bounded subsets over successive cranks, continue Phase 2 coverage, and never requires every remaining account to be in the same instruction;
54. a wrapper-level bitmap or candidate-cap predicate cannot reject a crank only because some uncovered exposed account is not provably solvent. Uncovered accounts remain lazy and must be handled by future touches, but their existence alone is not a reason to roll back an otherwise valid incremental crank; and
55. per-account floor-to-zero settlement increases potential dust but not certified dust unless accompanied by an exact lower-bound proof; tests include two or more zero-effective accounts sharing less than their count in residual OI, proving that one-q-per-account floor slack cannot be used to clear real OI; tests also cover ADL partial quantity decay where old certified dust is reduced by `q_close_q`, not carried forward unchanged or certified by aggregate A-ratio scaling; tests cover the orphan-exposure drain reset where all accounts on one side floor to zero while the opposite side still has stored positions, proving keeper/stress progress continues without funding or mark accrual against an orphan counterparty; and
56. h-max-lock sweep nonblocking liveness: while `hmax_effective_active(ctx)` / `hmax_lock_active(ctx)` remains true for multiple cranks and new positive PnL is h-locked at `admit_h_max`, keeper cranks continue to accrue and make bounded Phase 1/Phase 2 progress, risk-reducing trades proceed, and risk-increasing trades proceed when they pass `Eq_trade_open_no_pos_i`, dual-price checks when target/effective prices diverge, and all ordinary side-mode/OI/position/fee-current margin checks. Tests reject wrappers that halt all trading or roll back otherwise valid cranks solely because the post-event envelope is incomplete.
57. `admit_h_min = 0` immediate-release policy: when no h-max lock is active and `Residual > 0`, fresh positive PnL and outstanding reserves may mature immediately even when `PNL_matured_pos_tot + fresh_or_reserved_positive_pnl > Residual`; payouts/conversions remain haircut-bounded and fair exits are not guaranteed. If `Residual == 0` or after any live bankruptcy event, the same market forces `admit_h_max`, pauses release/conversion, uses zero-positive-credit approval lanes, and clears only after the authenticated post-event envelope and eligible generation advance.
58. same-instruction bankruptcy ordering and effective h-max endpoint checks: a permissionless keeper cannot touch or settle a winning account at `admit_h_min`/immediate release, release or accelerate existing reserve, convert or auto-convert positive PnL, finalize a touched account into capital, perform endpoint-local pre-payout/pre-free finalization using positive-PnL credit, or approve with positive-PnL credit and then trigger bankruptcy h-max later in the same atomic instruction; all same-instruction positive-PnL usability and finalization mutations are staged, recomputed, reversed, or retroactively reclassified into h-max reserve before commit, every endpoint-local check uses the final `hmax_effective_active(ctx)` state, and any already evaluated positive-credit approvals are recomputed under zero-positive-credit lanes;
59. dynamic reconciliation-envelope counting: if accrual, a stale-catchup segment, Phase 1, or Phase 2 starts/restarts the stress/h-max envelope, same-instruction `stress_counted_inspected` is reset to zero and no later inspection from that atomic instruction reduces the new envelope; tests cover nonzero below-threshold stress consumption, h-max activation before Phase 2, during Phase 2 after prior counted inspections, after a same-slot cursor wrap, and with stale-catchup `segment_slot < authenticated_now_slot`;
60. price-scale progress-floor and below-floor recovery: exposed CrankForward public catchup with `target != P_last` and `dt > 0` always has `max_delta >= 1` under the configured progress-floor invariant, the authenticated raw target as well as the effective price is proven above the floor while ordinary catchup is relied on, and any below-floor effective-price or raw-target state enters `permissionless_recovery_resolve` or a stricter public recovery path rather than advancing `slot_last` with a no-op price;
61. stale-dt bounded catchup: if `now_slot - slot_last > cfg_max_accrual_dt_slots`, repeated permissionless bounded catchup cranks construct each segment with checked subtraction-first arithmetic, never by overflowing `slot_last + cfg_max_accrual_dt_slots`, and advance `slot_last`, K/F, h-max state, and Phase 2 progress in finite segments without privileged recovery; tests include `slot_last` near `u64::MAX` and `cfg_max_accrual_dt_slots == u64::MAX`; if future-headroom/representability prevents a segment, `permissionless_recovery_resolve` or a stricter public recovery path is available and privileged-only recovery is rejected;
62. Phase 1 compute DoS resistance: missing, duplicate, malformed, already-flat, and nonliquidatable candidate entries count against `max_candidate_inspections`, while materialized revalidation count remains bounded by `max_revalidations`; hitting either limit produces partial progress, not rollback;
63. cursor-priority liveness: candidate-list packing cannot crowd out the current Phase 2 cursor account on the cursor-priority path, a materialized cursor slot with supplied account data can be touched even when the Phase 1 candidate list is full, and honest keepers can permissionlessly obtain/supply the authenticated cursor account proof;
64. no-accrual public paths do not create avoidable exposed `slot_last` lag: repeated no-op liveness/fee/materialization paths either advance `slot_last` through non-equity-active accrual or preserve a state from which bounded catchup remains permissionless; and
65. `permissionless_recovery_resolve`: below-floor, blocked-segment, oracle-unavailable, and declared counter-overflow recovery reasons can enter `Resolved` permissionlessly with an authenticated deterministic recovery price or authenticated oracle-unavailable `P_last` fallback, compute terminal K deltas from `P_last` before OI zeroing/reset, do not capture a payout snapshot early, cannot be invoked with a caller-chosen price, caller-chosen invalid segment input, omitted/malformed ordinary keeper proof, unproven recovery reason, or while ordinary bounded catchup can still make progress, and cannot fall back to `P_last` merely because the caller omitted or corrupted valid recovery-price proof, uses a representable authenticated recovery price before considering `P_last`, and only falls back on authenticated unrepresentability/oracle-unavailability when the immutable fallback is enabled;
66. public liveness-profile declaration: `BestEffort` deployments cannot advertise crank-forward guarantees, and `CrankForward` deployments enforce either the price-scale floor or permissionless recovery with bounded, always-includable recovery-price/oracle-unavailable proofs;
67. multi-segment catchup and segment/instruction time: the ordinary public keeper path executes at most one equity-active bounded segment per instruction; any separately specified multi-segment routine provides at least one protective progress unit per equity-active segment and rejects attempts to reuse a single touch as proof for multiple equity-moving segments; tests distinguish `segment_slot` from `authenticated_now_slot`, proving that generation rate limits, fee anchors, current-slot ownership, and h-max same-instruction progress counting use the authenticated instruction slot, not a stale segment slot;
68. same-slot no-op funding: `funding_active` is false when `dt == 0`, so same-slot no-op funding inputs cannot unnecessarily block public no-op progress or account-free non-equity-active liveness paths;
69. emergency `P_last` recovery fallback: fallback is available only when the immutable market policy enables it and an authenticated oracle-unavailable/unusable-price predicate is proven from bounded permissionless state; tests reject caller omission/corruption as a reason to select `P_last`; and
70. resolved-close liveness: all account/proof data needed to close stale or negative resolved accounts is permissionlessly obtainable/includable under fixed limits, a cursor/account-priority resolved-close path can clear every blocking account over bounded calls, and one unreachable account cannot indefinitely block positive-payout readiness.
