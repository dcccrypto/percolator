# Risk Engine Spec (Source of Truth) — v12.19.13

**Design:** protected principal + junior profit claims + lazy A/K/F side indices, native 128-bit persistent state.
**Status:** implementation source of truth. Normative terms are **MUST**, **MUST NOT**, **SHOULD**, **MAY**.
**Scope:** one perpetual DEX risk engine for one quote-token vault.

This revision supersedes v12.19.12. It is a consolidation and oracle-catchup hardening pass: it preserves the v12.19.12 economics, keeps the spec succinct, and makes two safety clarifications from first principles:

> The stress-scaled consumption threshold is **not** an anti-oracle-manipulation warmup. Public or permissionless wrappers using untrusted live oracle or execution-price PnL MUST use a nonzero live admission minimum (`admit_h_min > 0`) for positive PnL. `admit_h_min = 0` is only appropriate for trusted/private deployments or other non-public flows that explicitly accept immediate-release semantics.
>
> The engine's `oracle_price` input is the **effective engine price** that will be accrued against, not necessarily the raw external oracle target. A public wrapper whose raw normalized target jumps farther than the engine price cap MUST feed the engine a valid capped staircase price, keep the raw target separate from the last effective engine price, and restrict or conservatively shadow-check user value-moving/risk-increasing operations while the target and effective engine price differ.

The engine safety boundary is:

1. exact lazy A/K/F accounting for all mark, funding, and ADL effects;
2. exact positive-PnL junior-claim haircuts bounded by `Residual = V - (C_tot + I)`;
3. mandatory warmup/admission for live positive PnL;
4. exact candidate-trade positive-slippage neutralization;
5. an exact per-risk-notional solvency envelope checked at initialization; and
6. per-accrual price-move and funding envelopes checked before any K/F/price/slot mutation; and
7. wrapper-owned oracle-target catch-up that never feeds a cap-violating raw jump into live exposed accrual.

Every top-level instruction is atomic. Any failed precondition, checked arithmetic guard, missing authenticated account proof, context-capacity overflow, or conservative-failure condition MUST roll back every mutation performed by that instruction.

---

## 0. Core safety and liveness requirements

The engine MUST maintain the following properties.

1. Flat protected principal is senior. An account with effective position `0` MUST NOT have protected principal reduced by another account’s insolvency.
2. Open opposing positions MAY be subject to explicit deterministic ADL during bankrupt liquidation. ADL MUST be visible protocol state, never hidden execution.
3. Live positive PnL MUST pass admission. It MUST NOT be directly withdrawable, converted to principal, or counted as matured collateral unless admitted by the current instruction policy and the engine gates.
4. Public or permissionless wrappers with untrusted live oracle or execution-price PnL MUST use `admit_h_min > 0`; stress-threshold gating is additive and MUST NOT be treated as a substitute for warmup.
5. A candidate trade’s own positive execution-slippage PnL MUST be removed from that same trade’s risk-increasing approval metric.
6. Explicit protocol fees are collected into `I` immediately or tracked as account-local fee debt up to collectible headroom. Uncollectible fee tails are dropped, not socialized.
7. Losses are senior to engine-native fees on the same local capital state.
8. Synthetic liquidation close executes at oracle mark; liquidation penalties are explicit fees only.
9. Resolved positive payouts MUST wait for all stale accounts and all negative PnL to be reconciled, then use one shared payout snapshot.
10. Any arithmetic not proven unreachable by bounds MUST have checked, deterministic behavior. Silent wrap, unchecked panic, and undefined truncation are forbidden.
11. Account capacity is finite; empty fully-drained accounts MUST be reclaimable permissionlessly.
12. Keeper progress MUST be possible with off-chain candidate discovery and without a mandatory on-chain global scan.
13. The wrapper MUST NOT overload raw oracle target state and effective engine price state. Known lag between them MUST NOT become a public free-option: user risk-increasing and extraction-sensitive operations MUST be rejected or checked under a conservative target-price shadow policy while the lag exists.

---

## 1. Types, units, constants, configuration

### 1.1 Persistent and transient arithmetic

- Persistent unsigned economic quantities use `u128` unless otherwise stated.
- Persistent signed economic quantities use `i128` and MUST NOT equal `i128::MIN`.
- `wide_unsigned` / `wide_signed` mean exact transient domains at least 256 bits wide, or a formally equivalent comparison-preserving method.
- All products involving prices, positions, A/K/F indices, funding numerators, ADL deltas, fee products, haircut numerators, or warmup-release numerators MUST use checked arithmetic or exact multiply-divide helpers.

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
PRICE_MOVE_CONSUMPTION_SCALE  = 1_000_000_000
```

`MAX_ACTIVE_POSITIONS_PER_SIDE` MUST be finite and MUST NOT exceed `MAX_MATERIALIZED_ACCOUNTS`.

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
```

Live admission pairs MUST satisfy:

```text
0 <= admit_h_min <= admit_h_max <= cfg_h_max
admit_h_max > 0
admit_h_max >= cfg_h_min
if admit_h_min > 0: admit_h_min >= cfg_h_min
```

For public or permissionless wrappers with untrusted live oracle or execution-price PnL, wrapper policy MUST additionally enforce `admit_h_min > 0`.

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

### 1.7 Wrapper-fed effective price and raw oracle target

Oracle normalization, source selection, target storage, and rate limiting are wrapper-owned. The engine only validates and accrues the effective `oracle_price` passed to it.

A compliant public wrapper SHOULD maintain distinct fields equivalent to:

```text
oracle_target_price      // latest validated normalized external target
oracle_target_publish_ts // target source timestamp or publish slot
last_effective_price     // last price actually fed into engine accrual, equal to engine P_last when synchronized
```

The wrapper MUST NOT overload `last_effective_price` as the raw target. If the external target jumps beyond the engine cap, the wrapper keeps the raw target and feeds a capped staircase of effective prices until caught up.

For an exposed live market (`OI_eff_long != 0 || OI_eff_short != 0`), the wrapper-fed next effective price SHOULD be computed by the deterministic clamp law:

```text
dt = now_slot - slot_last
if target == P_last or dt == 0:
    next_price = P_last
else:
    max_delta = floor(P_last * cfg_max_price_move_bps_per_slot * dt / 10_000)
    next_price = clamp_toward(P_last, target, max_delta)
```

The multiplication MUST use exact wide arithmetic; `max_delta` MAY be capped to the price type maximum after the exact quotient. `clamp_toward` moves toward `target` by at most `max_delta` and never overshoots. The result MUST satisfy the engine cap in §5.3.

Normative consequences:

- Same-slot exposed cranks (`dt == 0`) MUST pass `P_last`; price catch-up requires elapsed slots. They MAY still do Phase 1 liquidation checks and Phase 2 round-robin touches at the unchanged effective price.
- If exposed `target != P_last`, `dt > 0`, and the computed `max_delta == 0`, ordinary live catch-up cannot make progress at the deployed price scale/cap. The wrapper MUST treat this as `CatchupRequired` / recovery territory and MUST NOT advance `slot_last` by feeding the unchanged price merely to bypass the lag.
- If exposed `dt > cfg_max_accrual_dt_slots` and the target differs from `P_last`, ordinary one-step live catch-up is unavailable. The wrapper MUST use an explicit recovery path, privileged degenerate resolution, or a separately specified atomic multi-accrual procedure that preserves all §5.3 mutation-order and cap invariants.
- If both OI sides are zero, no live position can lose equity, so the wrapper MAY feed the raw target directly subject to ordinary price validity.
- Feeding a cap-violating raw target into exposed live accrual is non-compliant and should fail before engine state mutation.

While `oracle_target_price != P_last`, the market is intentionally using a lagged effective engine price. For public wrappers, keeper progress, liquidation attempts, settlement, and structural sweep MAY continue at the effective price, but user operations that are risk-increasing or extraction-sensitive MUST either be rejected or pass a conservative wrapper shadow policy using both the effective engine price and the raw target. At minimum, public wrappers MUST reject risk-increasing user trades during target/effective-price divergence unless they are priced and margin-checked under a stricter dual-price policy that removes the known-lag free option.

---

## 2. State

### 2.1 Account state

Each materialized account stores:

```text
C_i: u128                      protected principal
PNL_i: i128                    realized PnL claim
R_i: u128                      reserved positive PnL, 0 <= R_i <= max(PNL_i,0)
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
phantom_dust_bound_long_q, phantom_dust_bound_short_q: u128
materialized_account_count, neg_pnl_account_count: u64
rr_cursor_position, sweep_generation: u64
price_move_consumed_bps_e9_this_generation: u128
market_mode in {Live, Resolved}
resolved_price, resolved_live_price: u64
resolved_slot: u64
resolved_k_long_terminal_delta, resolved_k_short_terminal_delta: i128
resolved_payout_snapshot_ready: bool
resolved_payout_h_num, resolved_payout_h_den: u128
```

Global invariants:

```text
C_tot <= V <= MAX_VAULT_TVL
I <= V
V >= C_tot + I
0 <= neg_pnl_account_count <= materialized_account_count <= cfg_account_index_capacity <= MAX_MATERIALIZED_ACCOUNTS
0 <= rr_cursor_position < cfg_account_index_capacity
slot_last <= current_slot
F_long_num and F_short_num fit i128
if Live: PNL_matured_pos_tot <= PNL_pos_tot <= MAX_PNL_POS_TOT_LIVE and resolved fields are zero
if Resolved: resolved_price > 0, resolved_live_price > 0, PNL_matured_pos_tot <= PNL_pos_tot
if snapshot not ready: resolved_payout_h_num = resolved_payout_h_den = 0
if snapshot ready: resolved_payout_h_num <= resolved_payout_h_den
```

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

`begin_full_drain_reset(side)` requires `OI_eff_side == 0` and then snapshots `K_side`/`F_side_num` to epoch-start fields, zeros live `K_side`/`F_side_num`, increments `epoch_side`, sets `A_side = ADL_ONE`, sets `stale_account_count_side = stored_pos_count_side`, clears phantom dust for that side, and enters `ResetPending`.

`finalize_side_reset(side)` requires `ResetPending`, zero OI, zero stale count, and zero stored position count, then sets mode to `Normal`.

Before any OI-increasing operation rejects on `ResetPending`, it MUST call `maybe_finalize_ready_reset_sides_before_oi_increase`.

---

## 3. Claims, haircuts, and equity

Let:

```text
Residual = V - (C_tot + I)   // checked, and invariant guarantees nonnegative
PosPNL_i = max(PNL_i, 0)
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
```

Candidate trade approval MUST neutralize that trade’s own positive slippage:

```text
TradeGain_i_candidate = max(candidate_trade_pnl_i, 0)
PNL_trade_open_i      = PNL_i - TradeGain_i_candidate
PosPNL_trade_open_i   = max(PNL_trade_open_i, 0)
PNL_pos_tot_trade_open_i = PNL_pos_tot - PosPNL_i + PosPNL_trade_open_i
compute g_open from PNL_pos_tot_trade_open_i and Residual
Eq_trade_open_raw_i = C_i + min(PNL_trade_open_i,0) + floor(PosPNL_trade_open_i*g_open.num/g_open.den) - FeeDebt_i
```

`Eq_trade_open_raw_i` is the only compliant risk-increasing trade approval metric.

---

## 4. Reserve, PnL, fee, and insurance helpers

### 4.1 Capital and position setters

`set_capital(i, new_C)` updates `C_tot` by the exact signed delta, then writes `C_i`.

`set_position_basis_q(i, new_basis)` updates long/short stored position counts exactly once according to old/new sign flags, enforcing `cfg_max_active_positions_per_side` on any increment, then writes `basis_pos_q_i`. All position-zeroing settlement branches MUST use this helper or an exactly equivalent path.

### 4.2 Reserve bucket operations

`promote_pending_to_scheduled(i)` does nothing if scheduled exists or pending absent. Otherwise it creates a scheduled bucket from pending with `sched_start_slot = current_slot`, `sched_anchor_q = sched_remaining_q = pending_remaining_q`, `sched_horizon = pending_horizon`, `sched_release_q = 0`, and clears pending. It MUST NOT change `R_i`.

`append_new_reserve(i, reserve_add, admitted_h_eff)` requires positive amount and positive horizon. If no scheduled bucket exists but pending exists, first promote pending. Then:

1. if scheduled absent, create scheduled at `current_slot`;
2. else if pending absent and `sched_start_slot == current_slot`, `sched_horizon == admitted_h_eff`, and `sched_release_q == 0`, merge into scheduled;
3. else if pending absent, create pending;
4. else merge into pending and set `pending_horizon = max(pending_horizon, admitted_h_eff)`.

Finally increase `R_i` by `reserve_add`.

`apply_reserve_loss_newest_first(i, reserve_loss)` consumes pending before scheduled, decrements `R_i`, and clears empty buckets.

`advance_profit_warmup(i)` promotes pending if needed, computes:

```text
elapsed = current_slot - sched_start_slot
effective_elapsed = min(elapsed, sched_horizon)
sched_total = floor(sched_anchor_q * effective_elapsed / sched_horizon)
sched_increment = sched_total - sched_release_q
release = min(sched_remaining_q, sched_increment)
```

It releases `release` to `PNL_matured_pos_tot`. If the scheduled bucket empties, it is cleared completely including `sched_release_q = 0`, and pending is promoted if present. A non-empty bucket MUST NOT persist with an over-advanced release cursor.

### 4.3 Admission

`admit_fresh_reserve_h_lock(i, fresh_positive_pnl_i, ctx, admit_h_min, admit_h_max) -> admitted_h_eff` requires a live materialized account and valid admission pair. Let:

```text
Residual_now = V - (C_tot + I)
matured_plus_fresh = PNL_matured_pos_tot + fresh_positive_pnl_i
threshold_opt = ctx.admit_h_max_consumption_threshold_bps_opt_shared
```

Law:

1. if `i` is in `ctx.h_max_sticky_accounts`, return `admit_h_max`;
2. if `threshold_opt = Some(threshold_bps)`, compute `threshold_e9 = threshold_bps * PRICE_MOVE_CONSUMPTION_SCALE`; if `price_move_consumed_bps_e9_this_generation >= threshold_e9`, choose `admit_h_max`;
3. otherwise choose `admit_h_min` iff `matured_plus_fresh <= Residual_now`, else `admit_h_max`;
4. if `admit_h_max` was chosen, insert `i` into the sticky set.

`None` disables the stress gate. `Some(0)` is invalid. The engine enforces only the supplied policy; public-wrapper nonzero-warmup requirements are wrapper obligations.

`admit_outstanding_reserve_on_touch(i, ctx)` accelerates all outstanding reserve only when all hold:

```text
reserve_total > 0
ctx.admit_h_min_shared == 0
stress threshold is absent or inactive
PNL_matured_pos_tot + reserve_total <= Residual_now
```

If so it moves the entire reserve into `PNL_matured_pos_tot`, clears both buckets, and sets `R_i = 0`. Otherwise it leaves reserve unchanged. It never extends or resets a horizon.

### 4.4 PnL mutation

Every persistent `PNL_i` mutation after materialization MUST use `set_pnl`, except `consume_released_pnl`.

`set_pnl(i, new_PNL, reserve_mode[, ctx])` where reserve mode is:

```text
UseAdmissionPair(admit_h_min, admit_h_max)
ImmediateReleaseResolvedOnly
NoPositiveIncreaseAllowed
```

It updates `PNL_pos_tot`, `PNL_matured_pos_tot`, `R_i`, reserve buckets, and `neg_pnl_account_count` atomically.

For positive increases:

- `NoPositiveIncreaseAllowed` fails;
- `ImmediateReleaseResolvedOnly` requires `Resolved`, increases `PNL_matured_pos_tot`, and does not reserve;
- `UseAdmissionPair` requires `Live`, obtains `admitted_h_eff`, immediately matures iff `admitted_h_eff == 0`, otherwise appends reserve.

For non-increases it consumes reserve loss newest-first, then matured loss, updates aggregates and sign count, and requires no reserve remains when live positive PnL becomes zero.

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

`charge_fee_to_insurance(i, fee_abs)` requires `fee_abs <= MAX_PROTOCOL_FEE_ABS`. It computes collectible headroom from capital plus fee-credit headroom, pays as much as possible from `C_i` into `I`, records any collectible shortfall as negative `fee_credits_i`, and drops the uncollectible tail. It MUST NOT mutate PnL, reserves, positive-PnL aggregates, or K/F indices.

`sync_account_fee_to_slot(i, anchor, rate)` charges recurring wrapper-owned fees exactly once over `[last_fee_slot_i, anchor]`, caps `rate * dt` at `MAX_PROTOCOL_FEE_ABS` without failing on raw-product overflow, routes the capped amount through `charge_fee_to_insurance`, and advances `last_fee_slot_i = anchor`. Live anchors must be `<= current_slot`; resolved anchors must be `<= resolved_slot`.

`fee_debt_sweep(i)` pays fee debt from available `C_i` into `I`. This preserves `Residual` because it is a pure `C -> I` reclassification.

### 4.6 Insurance loss

`use_insurance_buffer(loss_abs)` MUST spend exactly `pay = min(loss_abs, I)`, set `I -= pay`, and return `loss_abs - pay`. It MUST NOT drain the full insurance fund when the loss is smaller.

`record_uninsured_protocol_loss(loss_abs)` may record telemetry but MUST NOT inflate `D`, `C_tot`, `PNL_pos_tot`, `PNL_matured_pos_tot`, `V`, or `I`. The loss remains represented by junior haircuts.

`absorb_protocol_loss(loss_abs)` calls `use_insurance_buffer` and records only the returned nonzero remainder.

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

Live touch settlement:

1. if basis is zero, return;
2. require `a_basis_i > 0` and compute `den = a_basis_i * POS_SCALE` exactly;
3. if current epoch, compute effective quantity and `pnl_delta` with `wide_signed_mul_div_floor_from_kf_pair(abs_basis, k_snap, K_s, f_snap, F_s_num, den)`;
4. apply `set_pnl(..., UseAdmissionPair(ctx...))`;
5. if effective quantity floors to zero, increment the side phantom-dust bound by exactly one q-unit, clear basis through `set_position_basis_q(i,0)`, and reset snapshots; otherwise update snapshots.

Epoch-mismatch settlement requires `mode_s == ResetPending`, `epoch_snap_i + 1 == epoch_s`, and positive stale count. It settles against `K_epoch_start_s` / `F_epoch_start_s_num`, applies PnL through admission, clears basis through `set_position_basis_q(i,0)`, decrements stale count, and resets snapshots.

Resolved settlement first calls `prepare_account_for_resolved_touch`, then settles stale one-epoch-lag basis against:

```text
k_terminal_s_exact = K_epoch_start_s + resolved_k_terminal_delta_s
f_terminal_s_exact = F_epoch_start_s_num
```

using `ImmediateReleaseResolvedOnly`, then clears basis through `set_position_basis_q` and decrements stale count.

### 5.3 Accrual

`accrue_market_to(now_slot, oracle_price, funding_rate_e9_per_slot)` requires live mode, trusted `now_slot >= slot_last`, valid oracle price, and funding-rate magnitude within config.

Let:

```text
dt = now_slot - slot_last
funding_active = funding_rate != 0 && OI_eff_long != 0 && OI_eff_short != 0 && fund_px_last > 0
price_move_active = P_last > 0 && oracle_price != P_last && (OI_eff_long != 0 || OI_eff_short != 0)
```

If either active branch is true, require `dt <= cfg_max_accrual_dt_slots`.

If `price_move_active`, before mutating any K/F/price/slot/consumption state, require exactly:

```text
abs(oracle_price - P_last) * 10_000 <= cfg_max_price_move_bps_per_slot * dt * P_last
```

Then update stress consumption:

```text
consumed = floor(abs_delta_price * 10_000 * PRICE_MOVE_CONSUMPTION_SCALE / P_last)
price_move_consumed_bps_e9_this_generation = saturating_add(price_move_consumed_bps_e9_this_generation, consumed)
```

The accumulator is a stress signal, not a conservation quantity; overflow MUST saturate at `u128::MAX` and force slow-lane admission for finite thresholds until generation reset.

Mark-to-market once:

```text
ΔP = oracle_price - P_last
if OI_long_0  > 0: K_long  += A_long  * ΔP
if OI_short_0 > 0: K_short -= A_short * ΔP
```

Funding, if active, uses one exact total:

```text
fund_num_total = fund_px_last * funding_rate_e9_per_slot * dt
F_long_num  -= A_long  * fund_num_total
F_short_num += A_short * fund_num_total
```

Persistent K/F overflow fails conservatively. Finally set `slot_last = now_slot`, `P_last = oracle_price`, and `fund_px_last = oracle_price`.

### 5.4 ADL / bankrupt liquidation socialization

`enqueue_adl(ctx, liq_side, q_close_q, D)`:

1. decrements liquidated-side OI by `q_close_q`;
2. spends insurance exactly with `use_insurance_buffer(D)`;
3. if opposing OI is zero, records any remainder as uninsured and schedules reset if both sides zero;
4. if opposing stored position count is zero, reduces opposing OI by `q_close_q`, records remainder, and schedules reset if both sides zero;
5. otherwise computes opposing quantity decay and optional K loss.

For `D_rem > 0`, compute:

```text
delta_K_abs = ceil(D_rem * A_old * POS_SCALE / OI_before)
delta_K_exact = -delta_K_abs
```

If representability, `K_opp + delta_K_exact`, or future mark headroom `|K_candidate| + A_old * MAX_ORACLE_PRICE <= i128::MAX` fails, route `D_rem` to uninsured loss while still continuing quantity socialization.

Then:

```text
OI_post = OI_before - q_close_q
A_candidate = floor(A_old * OI_post / OI_before)
```

If `OI_post == 0`, zero opposing OI and schedule reset. If `A_candidate > 0`, set `A_opp`, set `OI_eff_opp`, add the exact ADL dust bound, and enter `DrainOnly` if `A_opp < MIN_A_SIDE`. If `A_candidate == 0` while `OI_post > 0`, zero both OI sides and schedule both resets.

### 5.5 End-of-instruction reset scheduling

At the end of every top-level instruction that can touch accounts, mutate side state, liquidate, or resolved-close, call `schedule_end_of_instruction_resets(ctx)` exactly once, except for the additional explicit pre-open dust/reset flush inside `execute_trade`.

If both stored side counts are zero, compute `clear_bound = checked_add(phantom_dust_bound_long_q, phantom_dust_bound_short_q)`. If residual OI or dust exists, require OI symmetry and clear both OI sides only if both are within `clear_bound`; otherwise fail conservatively.

If exactly one stored side is zero, require OI symmetry and clear both sides only if the empty side’s OI is within that side’s phantom-dust bound; otherwise fail conservatively.

If a side is `DrainOnly` and its OI is zero, set that side’s pending reset flag.

`finalize_end_of_instruction_resets(ctx)` begins pending resets and finalizes any ready `ResetPending` side.

---

## 6. Live local touch and finalization

`touch_account_live_local(i, ctx)`:

1. requires live materialized account;
2. adds `i` to `ctx.touched_accounts` or fails on capacity;
3. calls `admit_outstanding_reserve_on_touch(i, ctx)`;
4. advances warmup;
5. settles A/K/F side effects;
6. settles negative PnL from principal;
7. if now authoritative flat and still negative, calls `absorb_protocol_loss` and sets PnL to zero;
8. MUST NOT auto-convert or sweep fee debt.

`finalize_touched_accounts_post_live(ctx)` computes one shared whole-haircut snapshot after all live local work. It then iterates touched accounts in ascending storage-index order. If an account is flat, has released positive PnL, and the snapshot has `h = 1`, it uses `consume_released_pnl` followed by `set_capital(C_i + released)`. It then calls `fee_debt_sweep`.

---

## 7. Margin and liquidation

After authoritative live touch:

```text
RiskNotional_i = 0 if effective_pos_q(i) == 0
else ceil(abs(effective_pos_q(i)) * oracle_price / POS_SCALE)

MM_req_i = 0 if flat else max(floor(RiskNotional_i * cfg_maintenance_bps / 10_000), cfg_min_nonzero_mm_req)
IM_req_i = 0 if flat else max(floor(RiskNotional_i * cfg_initial_bps / 10_000), cfg_min_nonzero_im_req)
```

Maintenance healthy iff `Eq_net_i > MM_req_i`. Withdrawal healthy iff `Eq_withdraw_raw_i >= IM_req_i`. Risk-increasing trade approval healthy iff `Eq_trade_open_raw_i >= IM_req_post_i`.

A trade is risk-increasing if it increases absolute effective position, flips sign, or opens from flat. It is strictly risk-reducing if same sign, nonzero before/after, and absolute position decreases.

An account is liquidatable iff after full authoritative live touch it has nonzero effective position and `Eq_net_i <= MM_req_i`. If recurring fees are enabled, the account MUST be fee-current first.

Partial liquidation requires `0 < q_close_q < abs(old_eff_pos_q_i)`. It closes synthetically at oracle price, attaches the remaining position, settles losses from principal, charges liquidation fee, invokes `enqueue_adl(ctx, liq_side, q_close_q, 0)`, and requires the remaining nonzero position to be maintenance healthy after the step.

Full-close liquidation closes the whole effective position at oracle price, attaches flat, settles losses from principal, charges liquidation fee, sets `D = max(-PNL_i, 0)`, invokes `enqueue_adl` if `q_close_q > 0 || D > 0`, then sets negative PnL to zero with `NoPositiveIncreaseAllowed` if `D > 0`.

---

## 8. External operations

### 8.1 Standard live lifecycle

Live instructions that depend on current market state execute:

1. validate slots, effective oracle price, funding-rate bound, admission pair, optional threshold (`None` disables; `Some(t)` requires `0 < t <= floor(u128::MAX / PRICE_MOVE_CONSUMPTION_SCALE)`), and endpoint inputs;
2. initialize fresh `ctx`;
3. call `accrue_market_to` exactly once;
4. set `current_slot = now_slot`;
5. sync recurring fees for touched accounts before health-sensitive checks;
6. run endpoint logic;
7. call `finalize_touched_accounts_post_live(ctx)` exactly once if live local touches were used;
8. schedule and finalize resets exactly once;
9. assert OI symmetry for side-mutating/live-exposure instructions;
10. require `V >= C_tot + I`.

Any early no-op return after state mutation or fee sync MUST still perform the final applicable invariant checks.

### 8.2 No-accrual public path guard

Pure public live paths that advance `current_slot` without calling `accrue_market_to` MUST call:

```text
require_no_accrual_public_path_within_envelope(now_slot):
  require market_mode == Live
  require now_slot >= current_slot
  require slot_last <= current_slot
  if OI_eff_long == 0 && OI_eff_short == 0: return
  dt = now_slot - slot_last    // checked subtraction
  require dt <= cfg_max_accrual_dt_slots
```

This avoids overflow-prone `slot_last + cfg_max_accrual_dt_slots` arithmetic and permits zero-OI idle fast-forward.

### 8.3 Pure capital / fee operations

`deposit(i, amount, now_slot)` is live-only, no-accrual, and may materialize missing `i` only if `amount > 0`. It increases `V`, increases `C_i`, settles realized losses from principal, MUST NOT absorb flat negative loss through insurance, and sweeps fee debt only if the account is flat and nonnegative.

`deposit_fee_credits(i, amount, now_slot)` pays `min(amount, FeeDebt_i)` into `V` and `I`, increases `fee_credits_i` by that amount, and never makes fee credits positive.

`top_up_insurance_fund(amount, now_slot)` increases `V` and `I` by `amount`.

`charge_account_fee(i, fee_abs, now_slot)` routes `fee_abs` through `charge_fee_to_insurance` and performs no margin check by itself.

`settle_flat_negative_pnl(i, now_slot[, fee_rate])` is live-only, no-accrual, requires flat account with no reserve, syncs fee if enabled, settles losses from principal, then absorbs any remaining negative PnL through insurance/uninsured loss and sets PnL to zero.

`reclaim_empty_account(i, now_slot[, fee_rate])` is live-only, no-accrual, syncs fees if enabled, then requires the §2.3 free-slot preconditions and calls `free_empty_account_slot`.

### 8.4 User value-moving current-state operations

`settle_account` runs the standard live lifecycle, touches one account, and finalizes.

`withdraw` touches and finalizes first. It then requires `amount <= C_i`; if the account is nonflat, it requires withdrawal health under the hypothetical state where both `V` and `C_tot` decrease by `amount`; then it pays out by decreasing `C_i` and `V`.

`convert_released_pnl` touches first, requires `0 < x_req <= ReleasedPos_i`, computes current `h`, and for flat accounts requires `x_req <= max_safe_flat_conversion_released`. It consumes released PnL, adds `floor(x_req * h.num / h.den)` to capital, sweeps fee debt, and if still nonflat requires maintenance health.

`close_account` touches and finalizes first. It requires flat, zero PnL, no reserve, and no fee debt, pays out all capital by decreasing `C_i` and `V`, then calls `free_empty_account_slot`.

### 8.5 Trade

`execute_trade(a,b, ..., size_q, exec_price)` requires distinct materialized accounts, valid execution price, positive size, computed `trade_notional <= MAX_ACCOUNT_NOTIONAL`, and standard live lifecycle.

It syncs fees if enabled, touches both accounts in deterministic ascending storage-index order, then runs a pre-open dust/reset flush using a separate reset-only context. It captures pre-trade positions and maintenance state, finalizes ready reset sides, computes candidate positions and exact bilateral OI after-values, enforces position/OI bounds and side-mode gating, applies execution-slippage PnL before fees, attaches positions, writes OI after-values, settles losses, charges trade fees, computes post-trade risk notional and approval metrics, and approves each account independently:

- flat result: fee-neutral negative-shortfall comparison must not worsen;
- risk-increasing: require `Eq_trade_open_raw_i >= IM_req_post_i`;
- already maintenance healthy: allow;
- strictly risk-reducing while unhealthy: allow only if fee-neutral maintenance shortfall strictly improves and fee-neutral negative equity does not worsen;
- otherwise reject.

### 8.6 Liquidate

`liquidate(i, ..., policy)` runs standard live lifecycle, syncs fees if enabled, touches the account, requires liquidation eligibility, executes `FullClose` or `ExactPartial(q_close_q)`, finalizes, schedules/finalizes resets, and checks conservation.

### 8.7 Keeper crank

`keeper_crank(now_slot, oracle_price, funding_rate, admit_h_min, admit_h_max, threshold_opt, ordered_candidates[], max_revalidations, rr_window_size[, fee_fn])` is live-only and accrues exactly once before both phases.

Phase 1 processes keeper-supplied candidates in supplied order until `max_revalidations` is exhausted or a pending reset is scheduled. Authenticated missing-account skips do not count. If a candidate slot is materialized, its account state MUST be available; omission/unreadability fails conservatively. Liquidation is Phase 1 only.

Phase 2 always runs, even if Phase 1 stopped on pending reset. It does not count against `max_revalidations`, does not liquidate, and does not stop on pending reset. Let:

```text
sweep_limit = cfg_account_index_capacity
remaining = sweep_limit - rr_cursor_position
rr_advance = min(rr_window_size, remaining)
sweep_end = rr_cursor_position + rr_advance
```

For each index in `[rr_cursor_position, sweep_end)`, skip only if authenticated engine state proves missing; otherwise require account data and call `touch_account_live_local`. Then set `rr_cursor_position = sweep_end`. If it reaches `sweep_limit`, wrap to `0`, increment `sweep_generation`, and reset `price_move_consumed_bps_e9_this_generation = 0` atomically.

### 8.8 Resolution and resolved close

`resolve_market(resolve_mode, resolved_price, live_oracle_price, now_slot, funding_rate)` is privileged. Branch selection is explicit; value-detected branch selection is forbidden.

Ordinary branch calls `accrue_market_to(now_slot, live_oracle_price, funding_rate)`, sets `current_slot`, and requires the resolved price to be inside the configured deviation band around the trusted live-sync price. On this branch, `live_oracle_price` is the effective live-sync price supplied to the engine; if the raw external target is beyond the live cap, feeding it directly will fail and the wrapper must first catch up through valid capped accruals or choose an explicit recovery path.

Degenerate branch requires `live_oracle_price == P_last` and `funding_rate == 0`, sets `current_slot = slot_last = now_slot`, uses `P_last` as the resolved live price, and skips the ordinary band. It is a privileged recovery path only.

Both branches compute terminal K deltas exactly, store them separately from live K, enter `Resolved`, set `resolved_slot`, clear payout snapshot state, set `PNL_matured_pos_tot = PNL_pos_tot`, zero both OI sides, begin/finalize side resets as applicable, and require conservation.

`force_close_resolved(i)` is permissionless and takes no caller slot. It requires `current_slot == resolved_slot`, prepares the account for resolved touch, settles resolved side effects, settles/absorbs losses, finalizes ready reset sides, then:

- if `PNL_i == 0`, fee-sweeps, forgives remaining fee debt, pays out capital, and frees the slot;
- if `PNL_i > 0` and the market is not positive-payout ready, returns `ProgressOnly`;
- if positive-payout ready, captures the shared payout snapshot if needed, pays `floor(PNL_i * snapshot_num / snapshot_den)`, fee-sweeps, pays out capital, and frees the slot.

A zero payout MUST NOT be the only encoding of progress-only.

---

## 9. Wrapper obligations

1. Public wrappers MUST NOT expose arbitrary caller-controlled `admit_h_min`, `admit_h_max`, threshold, or funding-rate inputs.
2. Public or permissionless wrappers with untrusted live oracle or execution-price PnL MUST use `admit_h_min > 0` for instructions that can create or accelerate live positive PnL. `admit_h_min = 0` is reserved for trusted/private immediate-release deployments.
3. Stress threshold gating is optional engine machinery. It is a reconciliation/UX stress signal, not a substitute for warmup.
4. Resolution is privileged. Wrappers MUST source trusted live and settlement prices, funding rate, and explicit `resolve_mode`.
5. Wrappers MUST monitor accrual envelopes and K/F headroom, and crank or resolve before exposed markets exceed live envelopes.
6. Public wrappers MUST separate raw oracle target state from effective engine price state and MUST feed capped staircase prices, not cap-violating raw jumps, into exposed live accrual. Same-slot exposed cranks MUST pass the unchanged engine price. If exposed catch-up would have `target != P_last`, `dt > 0`, and `max_delta == 0`, the wrapper MUST enter recovery or wait for enough elapsed slots; it MUST NOT advance `slot_last` with the unchanged price as a silent bypass.
7. While raw target and effective engine price differ, public wrappers MUST reject or conservatively shadow-check extraction-sensitive user actions (`withdraw`, `convert_released_pnl`, user-triggered settlement/finalization that can release or convert positive PnL, and any close path whose payout depends on lagged PnL) and MUST reject risk-increasing user trades unless a stricter dual-price policy prices and margin-checks the trade against the lag.
8. Public wrappers using the sweep-generation stress gate MUST pass nonzero `rr_window_size` on normal keeper cranks and ensure `max_revalidations + rr_window_size` fits touched-account capacity and compute budget. `rr_window_size = 0` is reserved for trusted/private compatibility or explicit recovery flows.
9. Public wrappers SHOULD enforce execution-price admissibility, e.g. bounded deviation from effective engine price and, during oracle catch-up lag, from the raw target as well.
10. User value-moving operations must be account-authorized. Intended permissionless paths are settlement, liquidation, reclaim, flat-negative cleanup, resolved close, and keeper crank.
11. If recurring fees are enabled, wrappers MUST sync fee-current state before health-sensitive checks, reclaim checks, and resolved terminal close, and MUST use `resolved_slot` on resolved markets.
12. Wrappers own account-materialization anti-spam economics: minimum deposit, recurring fees, and reclaim incentives.
13. Runtime configuration MUST bound `max_revalidations + rr_window_size` to fit actual context capacity and compute budget.
---

## 10. Required test coverage

Implementations and public wrappers MUST test at least:

1. conservation `V >= C_tot + I` across all paths;
2. PnL aggregate and `neg_pnl_account_count` consistency;
3. reserve admission, sticky `admit_h_max`, pending/scheduled behavior, reserve loss ordering, and no stale release cursor;
4. public-wrapper policy tests that `admit_h_min = 0` is not used for untrusted public live PnL;
5. outstanding reserve acceleration blocked by nonzero `admit_h_min` or active threshold;
6. exact candidate-trade positive-slippage neutralization;
7. fee-debt sweep residual neutrality and actual-fee-impact comparisons;
8. `RiskNotional` ceil margin including fractional-notional dust;
9. exact per-risk-notional init envelope including funding fractions, post-move liquidation notional, fee floor, fee cap, and rounded notionals;
10. price-move cap rejection before any K/F/price/slot/consumption mutation;
11. wrapper oracle catch-up clamp: raw target is stored separately, next effective price moves toward target by at most `floor(P_last * cap * dt / 10_000)`, and same-slot exposed cranks pass `P_last`;
12. target/effective-price divergence policy: public risk-increasing trades and extraction-sensitive actions are rejected or pass a stricter dual-price shadow check;
13. zero-OI no-accrual fast-forward and exposed-market no-accrual envelope rejection using checked subtraction near `u64::MAX`;
14. exact insurance spending `min(loss_abs, I)`;
15. stress accumulator floor-at-scaled-bps precision, saturating addition, threshold activation, and reset only on generation advance;
16. deterministic Phase 2 cursor arithmetic over `cfg_account_index_capacity`, authenticated missing-slot skips, and failure on omitted materialized account data;
17. public keeper wrappers using the stress gate pass nonzero `rr_window_size` on normal cranks and enforce touched-account budget;
18. deterministic ascending trade touch order and pre-open dust/reset flush;
19. all position zeroing through `set_position_basis_q` and all frees through `free_empty_account_slot`;
20. resolved payout readiness, shared snapshot stability, and explicit progress-vs-close outcome;
21. degenerate resolution requires explicit mode and exact degenerate inputs; ordinary resolution never value-detects into degenerate mode;
22. ADL exact K deficit computation, overflow fallback to uninsured loss while quantity socialization continues, and phantom-dust clearance bounds;
23. self-neutral insurance/oracle-siphon scenarios across multiple valid accrual envelopes;
24. exposed `target != P_last`, `dt > 0`, `max_delta == 0` cannot advance `slot_last` by feeding `P_last`; it must wait, reject as catch-up-required, or enter explicit recovery;
25. raw target jumps beyond the cap are never fed directly to exposed live engine accrual except in an explicit recovery/resolution test that confirms conservative failure or privileged recovery semantics.
