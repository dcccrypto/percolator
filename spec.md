# Risk Engine Spec (Source of Truth) — v12.20.1

**Design:** protected principal + junior profit claims + lazy A/K/F/B side indices.
**Scope:** one perpetual DEX risk engine for one quote-token vault.
**Status:** normative source-of-truth draft. Terms **MUST**, **MUST NOT**, **SHOULD**, **MAY** are normative.

This revision supersedes v12.20.0. It keeps the B-index bankruptcy design, but fixes the pass-2 safety/liveness issues:

1. B booking is chunked and representable under a practical denominator, so dust-sized opposing exposure cannot force immediate terminal recovery for ordinary residual sizes.
2. B loss is settled together with K/F side effects before principal-loss settlement, so an account is not bankrupted by B before same-touch mark/funding gains are applied.
3. Per-account and global fractional B remainders are never silently dropped on clear/reset; they are moved into explicit scaled dust buckets and realized when they accumulate to a whole quote atom.
4. A started bankrupt close freezes conflicting state until its residual is booked, so new accounts cannot join, leave, or change the loss-bearing set between deficit computation and B booking.
5. The protected-principal rule is stated against authoritatively flat accounts (`basis_pos_q_i == 0`). A stored zero-effective basis may still settle already-booked side effects before canonical cleanup.
6. Continuation of an active bankrupt close uses frozen economic inputs, not caller-supplied replacement price/slot/fee state.
7. Any loss-weight change flushes the prior global B remainder into scaled dust first, so a remainder computed under one weight sum is never reused under another.

Every top-level instruction is atomic. Any failed precondition, checked arithmetic guard, missing authenticated proof, context-capacity overflow, or conservative-failure condition MUST roll back every mutation performed by that instruction. Before commit, every top-level instruction MUST leave all global invariants in §2 true.

---

## 0. Non-negotiable safety and liveness requirements

1. **Authoritatively flat protected principal is senior.** An account whose stored position has been canonically cleared (`basis_pos_q_i == 0`) MUST NOT lose protected principal because another account went bankrupt. A nonzero stored basis whose current effective quantity floors to zero is not yet authoritatively flat; it may settle already-booked A/K/F/B side effects and then must be canonically cleared.
2. Open opposing positions MAY absorb bankrupt losses, but only through explicit protocol state: A/K/F/B side indices, account-local snapshots, and exact remainder/dust buckets. Hidden full-book execution is forbidden.
3. Bankrupt residual socialization MUST be bounded. A public keeper crank MUST NOT need to inspect or touch all opposing accounts to close or advance one bankrupt account.
4. If a liquidated account has a post-principal deficit after its own value and insurance are exhausted, the residual MUST be durably booked before the account's negative PnL is cleared or the account is freed.
5. B-index booking MUST be exact in scaled arithmetic for each committed chunk:

```text
delta_B * W + rem_new - rem_old = chunk * SOCIAL_LOSS_DEN
```

where `W = loss_weight_sum_side` at the moment of the chunk booking.

6. No double charge: an account applies each B-index delta at most once by advancing `b_snap_i` to the target B index after settlement.
7. No positive-PnL escape: an account with unsettled B loss MUST NOT withdraw, close, convert/release positive PnL, auto-convert, use positive PnL for risk-increasing approval, detach/replace exposure, or receive terminal payout until B settlement and any resulting loss settlement have run.
8. B-induced insolvency is a bankruptcy signal. If B settlement exhausts protected principal and leaves `PNL_i < 0` in live mode, the instruction MUST start or restart bankruptcy h-max and MUST NOT allow user-directed position changes, payouts, or positive-credit approvals for that account. The account can progress only through liquidation/bankrupt close, loss-safe settlement, or terminal recovery.
9. Positive PnL is junior. Live positive PnL MUST pass admission and warmup. While stress, bankruptcy h-lock, B-staleness, or bounded-loss-stale catchup is active, public positive-PnL usability uses no-positive-credit lanes.
10. Losses are senior to fees on the same account. A fee drawn from capital MUST NOT precede unsettled mark/funding/ADL/B losses.
11. Keeper progress MUST be incremental. Dense markets with more liquidatable accounts than the candidate cap MUST clear under repeated honest cranks; no all-or-nothing uncovered-account predicate may freeze the market.
12. Account-free equity-active accrual is forbidden on exposed live markets. Price/funding movement that can change account equity MUST be composed with keeper/account-touch progress or explicit recovery.
13. Live time is clock-owned. Public wrappers MUST source `now_slot` from authenticated runtime state and MUST NOT allow caller-supplied future slots to move engine time.
14. Raw oracle target and effective engine price are distinct. If target/effective lag exists, public extraction and risk-increasing paths MUST reject or use a conservative dual-price/no-positive-credit policy.
15. All arithmetic that is not proven unreachable by bounds MUST be checked or exact-wide. Silent wrap, unchecked panic, and undefined truncation are forbidden.
16. Recovery must be public for public crank-forward markets. If bounded catchup, B booking, K/F headroom, price floors, oracle data, or counter ranges can block normal progress, the market MUST expose permissionless terminal recovery or prove the block impossible at initialization.
17. A global B remainder is tied to the exact `loss_weight_sum_side` that produced it. Before any operation changes that sum outside the active residual-booking continuation, the engine MUST transfer the old remainder into scaled dust and reset the side remainder to zero.
18. B residual booking MUST NOT charge an account whose current effective quantity is zero at the booking target. If the engine cannot prove that the maintained loss-weight sum excludes zero-effective current-epoch accounts, it MUST record the residual explicitly or route to recovery rather than applying B to that side.

---

## 1. Units, bounds, and configuration

### 1.1 Arithmetic domains

Persistent economic quantities use `u128` or `i128`. Persistent signed fields MUST NOT equal `i128::MIN`. Transient products involving prices, positions, side indices, weights, fees, haircuts, B booking, or remainders MUST use an exact domain at least 256 bits wide, or an equivalent comparison-preserving method.

### 1.2 Units

```text
POS_SCALE                = 1_000_000
ADL_ONE                  = 1_000_000_000_000_000
FUNDING_DEN              = 1_000_000_000
SOCIAL_WEIGHT_SCALE      = ADL_ONE
SOCIAL_LOSS_DEN          = 1_000_000_000_000_000_000_000  // 1e21
STRESS_CONSUMPTION_SCALE = 1_000_000_000
```

`SOCIAL_LOSS_DEN = 1e21` is load-bearing. With the required attach invariant `a_basis_i >= MIN_A_SIDE`, the maximum per-account loss weight is:

```text
MAX_LOSS_WEIGHT_PER_ACCOUNT = ceil(MAX_POSITION_ABS_Q * SOCIAL_WEIGHT_SCALE / MIN_A_SIDE) = 1e15
MAX_ACTIVE_POSITIONS_PER_SIDE <= 1e6
MAX_LOSS_WEIGHT_SUM <= 1e21 = SOCIAL_LOSS_DEN
```

Thus every positive residual atom with nonzero weight sum can produce a positive B-index delta, while the maximum one-step residual chunk at minimum weight is about `u128::MAX / 1e21`, greater than `MAX_VAULT_TVL`. Residuals above one-step capacity are booked over bounded O(1) chunks.

Every live/resolved/effective price MUST satisfy:

```text
0 < price <= MAX_ORACLE_PRICE
```

Risk notional is ceiling-rounded:

```text
RiskNotional_i = 0 if effective_pos_q(i) == 0
else ceil(abs(effective_pos_q(i)) * price / POS_SCALE)
```

Trade fee notional is floor-rounded:

```text
trade_notional = floor(size_q * exec_price / POS_SCALE)
```

### 1.3 Hard bounds

```text
MAX_VAULT_TVL                   = 10_000_000_000_000_000
MAX_ORACLE_PRICE                = 1_000_000_000_000
MAX_POSITION_ABS_Q              = 100_000_000_000_000
MAX_TRADE_SIZE_Q                = MAX_POSITION_ABS_Q
MAX_OI_SIDE_Q                   = 100_000_000_000_000
MAX_ACCOUNT_NOTIONAL            = 100_000_000_000_000_000_000
MAX_PROTOCOL_FEE_ABS            = 1_000_000_000_000_000_000_000_000_000_000_000_000
GLOBAL_MAX_ABS_FUNDING_E9_PER_SLOT = 10_000
MAX_BPS                         = 10_000
MAX_MATERIALIZED_ACCOUNTS       = 1_000_000
MIN_A_SIDE                      = 100_000_000_000_000
MAX_WARMUP_SLOTS                = u64::MAX
MAX_RESOLVE_PRICE_DEVIATION_BPS = 10_000
```

Deployments MAY choose stricter bounds. Initialization MUST reject any config that cannot prove all configured per-market envelopes under these bounds.

### 1.4 Immutable per-market configuration

The market stores immutable:

```text
cfg_h_min, cfg_h_max
cfg_maintenance_bps, cfg_initial_bps
cfg_trading_fee_bps
cfg_liquidation_fee_bps, cfg_liquidation_fee_cap, cfg_min_liquidation_abs
cfg_min_nonzero_mm_req, cfg_min_nonzero_im_req
cfg_resolve_price_deviation_bps
cfg_account_index_capacity
cfg_max_active_positions_per_side
cfg_max_accrual_dt_slots
cfg_max_abs_funding_e9_per_slot
cfg_max_price_move_bps_per_slot
cfg_min_funding_lifetime_slots
cfg_public_liveness_profile in {BestEffort, CrankForward}
cfg_permissionless_recovery_enabled
cfg_recovery_p_last_fallback_enabled
cfg_non_catchupable_states_impossible_proof
```

Initialization MUST require:

```text
0 < cfg_min_nonzero_mm_req < cfg_min_nonzero_im_req
0 <= cfg_maintenance_bps <= cfg_initial_bps <= MAX_BPS
0 <= cfg_trading_fee_bps <= MAX_BPS
0 <= cfg_liquidation_fee_bps <= MAX_BPS
0 <= cfg_min_liquidation_abs <= cfg_liquidation_fee_cap <= MAX_PROTOCOL_FEE_ABS
0 <= cfg_h_min <= cfg_h_max <= MAX_WARMUP_SLOTS
cfg_h_max > 0
0 <= cfg_resolve_price_deviation_bps <= MAX_RESOLVE_PRICE_DEVIATION_BPS
0 < cfg_account_index_capacity <= MAX_MATERIALIZED_ACCOUNTS
0 < cfg_max_active_positions_per_side <= cfg_account_index_capacity
0 < cfg_max_accrual_dt_slots
0 <= cfg_max_abs_funding_e9_per_slot <= GLOBAL_MAX_ABS_FUNDING_E9_PER_SLOT
0 < cfg_max_price_move_bps_per_slot
0 < initial_oracle_price <= MAX_ORACLE_PRICE
cfg_max_active_positions_per_side * MAX_LOSS_WEIGHT_PER_ACCOUNT <= SOCIAL_LOSS_DEN
if cfg_public_liveness_profile == CrankForward:
    cfg_permissionless_recovery_enabled || cfg_non_catchupable_states_impossible_proof
```

Funding and K/F envelopes MUST be proven in exact wide arithmetic:

```text
ADL_ONE * MAX_ORACLE_PRICE * cfg_max_abs_funding_e9_per_slot * cfg_max_accrual_dt_slots <= i128::MAX
cfg_min_funding_lifetime_slots >= cfg_max_accrual_dt_slots
ADL_ONE * MAX_ORACLE_PRICE * cfg_max_abs_funding_e9_per_slot * cfg_min_funding_lifetime_slots <= i128::MAX
```

Initialization MUST also validate the exact per-risk-notional solvency envelope for every integer `N` with `1 <= N <= MAX_ACCOUNT_NOTIONAL` by an exact bounded proof or a stronger conservative proof:

```text
price_budget_bps      = cfg_max_price_move_bps_per_slot * cfg_max_accrual_dt_slots
funding_budget_num    = cfg_max_abs_funding_e9_per_slot * cfg_max_accrual_dt_slots * 10_000
loss_budget_num       = price_budget_bps * FUNDING_DEN + funding_budget_num
price_funding_loss_N  = ceil(N * loss_budget_num / (10_000 * FUNDING_DEN))
worst_liq_notional_N  = ceil(N * (10_000 + price_budget_bps) / 10_000)
liq_fee_raw_N         = ceil(worst_liq_notional_N * cfg_liquidation_fee_bps / 10_000)
liq_fee_N             = min(max(liq_fee_raw_N, cfg_min_liquidation_abs), cfg_liquidation_fee_cap)
mm_req_N              = max(floor(N * cfg_maintenance_bps / 10_000), cfg_min_nonzero_mm_req)
require price_funding_loss_N + liq_fee_N <= mm_req_N
```

### 1.5 B-index representability and chunking

A B booking step may book at most the representable chunk. Let `H = u128::MAX - B_side_num`, `W = loss_weight_sum_side`, and `R = social_loss_remainder_side_num`. The exact safe upper bound is:

```text
// computed in exact wide arithmetic; H + 1 is a wide integer, not u128
max_scaled = (H + 1) * W - 1
if R > max_scaled:
    max_chunk_by_B = 0
else:
    max_chunk_by_B = floor((max_scaled - R) / SOCIAL_LOSS_DEN)

chunk = min(residual_remaining, max_chunk_by_B, caller_or_engine_chunk_budget)
```

This formula is load-bearing. A looser expression such as `floor((H * W + R) / SOCIAL_LOSS_DEN)` is non-compliant because it can admit a chunk whose resulting `delta_B` exceeds `H` when `R` is large.

A successful B booking requires:

```text
W > 0
chunk > 0
B_side_num + delta_B <= u128::MAX
new_remainder < W <= SOCIAL_LOSS_DEN
```

If `max_chunk_by_B == 0` while residual remains, the close MUST return `RecoveryRequired` or enter permissionless recovery; it MUST NOT clear the bankrupt account. A huge residual MAY require multiple successful calls. Each call is O(1), commits a positive residual chunk, and monotonically reduces `residual_remaining`.

For CrankForward markets, `RecoveryRequired` MUST be actionable through the permissionless recovery path unless initialization proved that state impossible. A public keeper path MUST NOT return a permanent non-progress error for an active bankrupt close.

---

## 2. State and invariants

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
fee_credits_i: i128 <= 0 and != i128::MIN
last_fee_slot_i: u64

// Bankruptcy social-loss state
loss_weight_i: u128
b_snap_i: u128
b_rem_i: u128
b_epoch_snap_i: u64

// Warmup reserve, at most one scheduled and one pending bucket
sched_present_i: bool
sched_remaining_q_i, sched_anchor_q_i: u128
sched_start_slot_i, sched_horizon_i: u64
sched_release_q_i: u128
pending_present_i: bool
pending_remaining_q_i: u128
pending_horizon_i: u64
```

If `basis_pos_q_i == 0`, then all A/K/F/B local position fields MUST be canonical zero/defaults:

```text
a_basis_i = ADL_ONE
k_snap_i = 0
f_snap_i = 0
epoch_snap_i = 0
loss_weight_i = 0
b_snap_i = 0
b_rem_i = 0
b_epoch_snap_i = 0
```

If `basis_pos_q_i != 0`, then:

```text
a_basis_i >= MIN_A_SIDE
loss_weight_i = ceil(abs(basis_pos_q_i) * SOCIAL_WEIGHT_SCALE / a_basis_i)
loss_weight_i > 0
b_rem_i < SOCIAL_LOSS_DEN
b_epoch_snap_i == epoch_side or side is ResetPending and b_epoch_snap_i + 1 == epoch_side
```

`loss_weight_i` is the deterministic bankruptcy-loss weight for this attachment. It is proportional to current effective exposure across a side because all accounts on the same current epoch share the same `A_side` factor. B equivalence is defined against these engine-owned weights, not against floor-rounded `OI_eff` or a full-book scan.

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

// Bankruptcy social-loss indices
B_long_num, B_short_num: u128
B_epoch_start_long_num, B_epoch_start_short_num: u128
loss_weight_sum_long, loss_weight_sum_short: u128
social_loss_remainder_long_num, social_loss_remainder_short_num: u128
explicit_unallocated_loss_long, explicit_unallocated_loss_short: u128
explicit_unallocated_protocol_loss: u128
social_loss_dust_long_num, social_loss_dust_short_num: u128
social_loss_dust_protocol_num: u128

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

bankrupt_close: optional BankruptCloseState

market_mode in {Live, Resolved}
resolved_price, resolved_live_price, resolved_slot
resolved_k_long_terminal_delta, resolved_k_short_terminal_delta: i128
resolved_payout_snapshot_ready: bool
resolved_payout_h_num, resolved_payout_h_den: u128
```

Global invariants:

```text
C_tot <= V <= MAX_VAULT_TVL
I <= V
V >= C_tot + I
0 <= materialized_account_count <= cfg_account_index_capacity
0 <= neg_pnl_account_count <= materialized_account_count
PNL_matured_pos_tot <= PNL_pos_tot
0 < P_last <= MAX_ORACLE_PRICE
0 < fund_px_last <= MAX_ORACLE_PRICE
slot_last <= current_slot
0 < A_long <= ADL_ONE
0 < A_short <= ADL_ONE
if a side is Normal and has current-epoch stored positions: A_side >= MIN_A_SIDE
0 <= OI_eff_long <= MAX_OI_SIDE_Q
0 <= OI_eff_short <= MAX_OI_SIDE_Q
if Live and both sides have ordinary exposure: OI_eff_long == OI_eff_short
0 <= stored_pos_count_side <= materialized_account_count
0 <= stale_account_count_side <= stored_pos_count_side
0 <= loss_weight_sum_side <= SOCIAL_LOSS_DEN
social_loss_remainder_side_num < SOCIAL_LOSS_DEN
social_loss_dust_*_num < SOCIAL_LOSS_DEN
rr_cursor_position < cfg_account_index_capacity
```

Live K/F future-headroom invariants:

```text
abs(K_side) + A_side * MAX_ORACLE_PRICE <= i128::MAX
abs(F_side_num) + A_side * MAX_ORACLE_PRICE * cfg_max_abs_funding_e9_per_slot * cfg_max_accrual_dt_slots <= i128::MAX
```

Live B invariants:

```text
if mode_side == Normal or DrainOnly:
    B_side_num is current-epoch side B index
    loss_weight_sum_side equals the incremental sum of loss_weight_i for current-epoch nonzero-basis accounts on that side

if mode_side == ResetPending:
    B_epoch_start_side_num is the terminal B target for stale accounts from the prior epoch
    current B_side_num and loss_weight_sum_side are for the new epoch and may be zero
```

### 2.3 Scaled dust realization

Scaled B dust is not user claim state. It is exact-accounting residue below one quote atom until it accumulates. Whenever global B remainder or account-local `b_rem_i` is discarded by account clear, side reset, terminal close, recovery, or a change to the side's loss-weight sum, it MUST first be transferred into a scaled dust accumulator for that side or protocol:

```text
social_loss_dust_side_num += b_rem_i or social_loss_remainder_side_num
while social_loss_dust_side_num >= SOCIAL_LOSS_DEN:
    explicit_unallocated_loss_side += 1
    social_loss_dust_side_num -= SOCIAL_LOSS_DEN
```

The same rule applies to protocol-level dust. If a dust realization creates a whole-atom live protocol loss, it MUST trigger/restart bankruptcy h-max before commit. Dropping fractional B remainder without this transfer is non-compliant, because repeated clears/resets could otherwise lose whole atoms over time.

Before any attach, clear, replacement, trade, liquidation quantity update, or reset changes `loss_weight_sum_side`, the engine MUST run:

```text
flush_side_b_remainder(side):
    transfer social_loss_remainder_side_num to social_loss_dust_side_num and realize whole atoms
    social_loss_remainder_side_num = 0
```

The only exception is the active bankrupt-close residual-booking continuation itself: during that continuation the loss-bearing set is frozen, so repeated chunks may use the same weight sum until `ResidualBooked`.

`explicit_unallocated_loss_*` fields are durable loss ledgers, not withdrawable funds, protected principal, insurance, or positive-PnL claims. They justify clearing an otherwise unassigned negative tail only after the exact loss amount has been recorded with checked arithmetic. They MUST NOT increase `Residual`, `C_tot`, `I`, `PNL_pos_tot`, or any payout capacity, and resolved payout haircuts still use actual vault residual claims.

### 2.4 Bankrupt close progress state

At most one global bankrupt close may be active unless an implementation proves independent progress states cannot conflict. Shape:

```text
BankruptCloseState {
  idx: u32
  generation: u64
  phase in {Touched, PositionClosed, DeficitComputed, InsuranceApplied, ResidualPartiallyBooked, ResidualBooked, AccountClosed}
  close_slot: u64
  close_price: u64
  liquidation_fee_obligation: u128
  liq_side
  opp_side
  liq_epoch_at_start: u64
  opp_epoch_at_deficit: u64
  opp_b_index_at_deficit: u128
  opp_loss_weight_sum_at_deficit: u128
  q_close_q: u128
  deficit_total: u128
  insurance_paid: u128
  residual_remaining: u128
  residual_booked_total: u128
}
```

A successful call that observes an active close MUST continue that close, complete it, or enter permissionless recovery. It MUST NOT start a different close or perform a conflicting operation. Continuation MUST use the frozen `close_slot`, `close_price`, side epochs, close quantity, and fee obligation stored in `BankruptCloseState`; caller-supplied price, slot, fee, or policy inputs are only authentication/routing inputs and MUST NOT replace the stored economics. A mismatched continuation input MUST reject before mutation or enter the specified recovery path.

While a close is active and not yet `ResidualBooked`, the engine MUST reject any operation that can change the loss-bearing set for `opp_side`, including attach, clear, trade, side reset, resolved transition, or another bankrupt close, unless that operation is the active close continuation or permissionless recovery. This residual-booking barrier prevents accounts from joining/leaving the opposing side between deficit computation and B booking.

Every successful close step MUST monotonically advance the phase, book a positive residual chunk, close the account, or advance a durable keeper cursor/touch state. A close state that cannot progress because B booking is unrepresentable MUST route to permissionless recovery; it MUST NOT become a permanent market lock.

---

## 3. Claims, equity, and admission

Definitions:

```text
Residual = V - (C_tot + I)
PosPNL_i = max(PNL_i, 0)
FeeDebt_i = max(-fee_credits_i, 0)
ReleasedPos_i = PosPNL_i - R_i on Live
ReleasedPos_i = PosPNL_i on Resolved
```

Haircuts:

```text
if PNL_matured_pos_tot == 0: h = (1,1)
else h = (min(Residual, PNL_matured_pos_tot), PNL_matured_pos_tot)

if PNL_pos_tot == 0: g = (1,1)
else g = (min(Residual, PNL_pos_tot), PNL_pos_tot)
```

Equity lanes:

```text
PNL_eff_matured_i = floor(ReleasedPos_i * h.num / h.den)
PNL_eff_trade_i   = floor(PosPNL_i     * g.num / g.den)

Eq_withdraw_raw_i = C_i + min(PNL_i,0) + PNL_eff_matured_i - FeeDebt_i
Eq_trade_raw_i    = C_i + min(PNL_i,0) + PNL_eff_trade_i   - FeeDebt_i
Eq_maint_raw_i    = C_i + PNL_i                            - FeeDebt_i
Eq_net_i          = max(0, Eq_maint_raw_i)

Eq_withdraw_no_pos_i = C_i + min(PNL_i,0) - FeeDebt_i
Eq_trade_no_pos_i    = C_i + min(PNL_i,0) - FeeDebt_i
```

Risk-increasing trade approval MUST remove the candidate trade's own positive slippage from the same account's approval metric.

Admission pair:

```text
0 <= admit_h_min <= admit_h_max <= cfg_h_max
admit_h_max > 0
admit_h_max >= cfg_h_min
if admit_h_min > 0: admit_h_min >= cfg_h_min
```

`hmax_effective_active(ctx)` is true if any of the following is true:

```text
threshold stress gate active
bankruptcy_hmax_lock_active
instruction-local bankruptcy candidate active
loss-stale bounded catchup active
account or endpoint has unsettled B loss and wants positive-PnL usability
```

While active, fresh positive PnL uses `admit_h_max`; reserve release, reserve acceleration, manual conversion, auto-conversion, and positive-credit approvals are paused or recomputed under no-positive-credit lanes.

Same-instruction h-max activation is retroactive. Any instruction that can trigger bankruptcy MUST stage or recompute positive-PnL usability before commit. It MUST NOT admit, mature, convert, auto-convert, or approve with positive credit and then later trigger h-max in the same atomic instruction unless those earlier effects are reversed or recomputed under h-max/no-positive-credit policy.

---

## 4. Canonical helpers

### 4.1 Capital and position helpers

Every persistent `C_i` mutation after materialization MUST use `set_capital(i,new_C)` or an equivalent aggregate-updating path that updates `C_tot` and proves:

```text
C_tot <= V <= MAX_VAULT_TVL
I <= V
V >= C_tot + I
```

Every persistent `basis_pos_q_i` mutation after materialization MUST use `set_position_basis_q` or an equivalent stored-count-updating path.

Nonzero live attach MUST use:

```text
attach_effective_position_q(i, new_eff)
```

It first requires the old side effects are settled, flushes the side B remainder if the side loss-weight sum will change, and checks that side mode permits a fresh/current-epoch attach. It then writes:

```text
basis_pos_q_i = new_eff
a_basis_i = A_side
require a_basis_i >= MIN_A_SIDE
k_snap_i = K_side
f_snap_i = F_side_num
epoch_snap_i = epoch_side
loss_weight_i = ceil(abs(new_eff) * SOCIAL_WEIGHT_SCALE / A_side)
require loss_weight_i > 0
require loss_weight_sum_side + loss_weight_i <= SOCIAL_LOSS_DEN
b_snap_i = B_side_num
b_rem_i = 0
b_epoch_snap_i = epoch_side
loss_weight_sum_side += loss_weight_i
```

It requires the resulting `effective_pos_q(i)` equals `new_eff`. Local attach/clear MUST NOT mutate global OI; trade writes bilateral OI after-values, and liquidation close quantity writes OI only through `enqueue_adl` / bankrupt close.

`clear_position_basis_q(i)` first requires B and A/K/F are settled to the correct target. It then transfers any nonzero `b_rem_i` into scaled dust, flushes the side B remainder if `loss_weight_sum_side` will change, subtracts `loss_weight_i` from `loss_weight_sum_side` if the account is current epoch, clears basis and all A/K/F/B account-local snapshots, and updates stored side counts. It MUST NOT mutate OI by itself.

### 4.2 Combined side-effect settlement

Authoritative touch MUST settle B and A/K/F as one combined side-effect mutation before principal loss settlement. The engine MUST NOT drain protected capital for B loss before same-touch K/F gains or losses have been applied.

`prepare_side_effect_delta(i) -> SideEffectDelta` computes candidates without writing account state:

```text
B_loss_i, b_snap_candidate, b_rem_candidate
KF_pnl_delta_i, k_snap_candidate, f_snap_candidate, epoch_candidate
net_pnl_delta_i = KF_pnl_delta_i - B_loss_i
```

B part:

```text
if basis_pos_q_i == 0:
    require loss_weight_i = b_snap_i = b_rem_i = b_epoch_snap_i = 0
    B_loss_i = 0
else if epoch_snap_i == epoch_side:
    B_target = B_side_num
else:
    require mode_side == ResetPending and epoch_snap_i + 1 == epoch_side
    B_target = B_epoch_start_side_num

require b_snap_i <= B_target
ΔB = B_target - b_snap_i
num = b_rem_i + loss_weight_i * ΔB
B_loss_i = floor(num / SOCIAL_LOSS_DEN)
b_rem_candidate = num % SOCIAL_LOSS_DEN
b_snap_candidate = B_target
```

A/K/F part uses the exact signed-floor formula in §5.1 with the matching current or epoch-start targets.

`apply_side_effect_delta(i, delta, ctx)` calls `set_pnl(i, PNL_i + net_pnl_delta_i, UseAdmissionPair(ctx...))` in live mode, or the resolved-mode equivalent. It then writes `b_snap_i = b_snap_candidate`, `b_rem_i = b_rem_candidate`, and K/F snapshots atomically with the PnL mutation. Only after this combined side-effect application may the caller call `settle_negative_pnl_from_principal(i, ctx)`.

If `B_loss_i > 0`, the account is B-stale until this combined settlement commits. Before commit, positive-PnL usability, withdrawal, close, conversion, risk-increasing approval, detach/replace, and terminal payout are forbidden or must be recomputed after settlement.

### 4.3 PnL and fee helpers

Every `PNL_i` mutation uses `set_pnl`. Positive increases in live mode go through admission. Negative cleanup with `NoPositiveIncreaseAllowed` is allowed only if it does not create a positive junior claim.

`settle_negative_pnl_from_principal(i, ctx)` pays losses from `C_i`. If live `PNL_i < 0` remains after `C_i` is zero, the instruction MUST trigger bankruptcy h-max before commit. User-directed position changes, payouts, and positive-credit approvals for that account MUST then reject until liquidation/bankrupt close or recovery handles the tail.

`charge_fee_to_insurance` pays as much as possible from capital into insurance, records collectible shortfall as fee debt, and drops uncollectible fee tails. It MUST be called only after losses senior to that fee have been settled.

`sync_account_fee_to_slot` charges half-open `[last_fee_slot_i, anchor)` and advances the anchor. During bounded catchup with `slot_last < current_slot`, fees drawn from nonflat or potentially stale accounts MUST be anchored no later than `slot_last` until the account is loss-current or proven flat/current.

---

## 5. A/K/F/B side mechanics

### 5.1 Effective position and A/K/F settlement

For nonzero basis on side `s`:

```text
if epoch_snap_i != epoch_s: effective_pos_q(i) = 0
else effective_abs = floor(abs(basis_pos_q_i) * A_s / a_basis_i)
effective_pos_q = sign(basis_pos_q_i) * effective_abs
```

A/K/F PnL settlement uses exact signed floor:

```text
abs_basis = abs(basis_pos_q_i)
den_q = a_basis_i * POS_SCALE
k_delta = K_target_s - k_snap_i
f_delta_num = F_target_s_num - f_snap_i
pnl_num = abs_basis * (k_delta * FUNDING_DEN + f_delta_num)
pnl_den = den_q * FUNDING_DEN
pnl_delta = signed_floor_div(pnl_num, pnl_den)
```

A live floor-to-zero account after combined settlement may clear basis and add only potential phantom dust unless an exact proof certifies dust. It MUST NOT mutate global OI and MUST NOT be used as a user-directed close or liquidation substitute during loss-stale catchup.

### 5.2 Accrual

`accrue_market_to(now_slot, oracle_price, funding_rate)` requires live mode, authenticated `now_slot >= current_slot`, `slot_last <= current_slot`, `now_slot >= slot_last`, valid price, and funding magnitude within config.

Let:

```text
dt = now_slot - slot_last
funding_active = dt > 0 && funding_rate != 0 && OI_eff_long != 0 && OI_eff_short != 0 && fund_px_last > 0
price_move_active = P_last > 0 && oracle_price != P_last && (OI_eff_long != 0 || OI_eff_short != 0)
```

If either active branch is true, require `dt <= cfg_max_accrual_dt_slots`. If `price_move_active`, require before mutation:

```text
abs(oracle_price - P_last) * 10_000 <= cfg_max_price_move_bps_per_slot * dt * P_last
```

Compute K/F/stress candidates in exact wide arithmetic. K update:

```text
ΔP = oracle_price - P_last
if OI_eff_long  > 0: K_long'  = K_long  + A_long  * ΔP
if OI_eff_short > 0: K_short' = K_short - A_short * ΔP
```

Funding update:

```text
fund_num_total = fund_px_last * funding_rate * dt
if funding_active:
  F_long_num'  = F_long_num  - A_long  * fund_num_total
  F_short_num' = F_short_num + A_short * fund_num_total
```

Before writing, require K/F future-headroom. Stress state is written only if all K/F candidates succeed. Then atomically set K/F/stress candidates and:

```text
slot_last = now_slot
current_slot = now_slot
P_last = oracle_price
fund_px_last = oracle_price
```

### 5.3 B residual booking

`book_bankruptcy_residual_chunk_to_side(ctx, side, residual_remaining, chunk_budget) -> booked_chunk` is O(1) in account count.

B booking is allowed only for an eligible current-epoch loss-bearing side:

```text
mode_side != ResetPending
OI_eff_side > 0
loss_weight_sum_side > 0
loss_weight_sum_side <= SOCIAL_LOSS_DEN
loss_weight_sum_side is certified for the current B epoch and excludes known zero-effective accounts
```

If any eligibility condition fails, no current-epoch account set can safely receive a normal B-index loss. The engine records the remaining residual explicitly with checked arithmetic or routes to terminal recovery:

```text
explicit_unallocated_loss_side = checked_add(explicit_unallocated_loss_side, residual_remaining)
residual_remaining = 0
```

and triggers/restarts bankruptcy h-max. This is durable loss state; the bankrupt account may be cleared only after this record exists. A zero-OI, reset-pending, or zero-weight side MUST NOT receive B loss through stale weights.

A stored nonzero basis whose effective quantity has already floored to zero may settle B that was booked before it became zero-effective, but it MUST NOT be included in any later B residual booking. If excluding such accounts would require an unavailable scan, the implementation must conservatively use explicit unallocated loss or recovery for that residual.

If the side is eligible, compute the maximum representable chunk exactly. Let `H = u128::MAX - B_side_num`, `W = loss_weight_sum_side`, and `R = social_loss_remainder_side_num`:

```text
// exact wide arithmetic; H + 1 is a wide integer, not u128
max_scaled = (H + 1) * W - 1
if R > max_scaled:
    max_chunk_by_B = 0
else:
    max_chunk_by_B = floor((max_scaled - R) / SOCIAL_LOSS_DEN)

chunk = min(residual_remaining, chunk_budget, max_chunk_by_B)
require chunk > 0 or return RecoveryRequired
scaled = chunk * SOCIAL_LOSS_DEN + R
delta_B = floor(scaled / W)
new_rem = scaled % W
require delta_B > 0
require B_side_num + delta_B <= u128::MAX
```

A looser formula such as `floor((H * W + R) / SOCIAL_LOSS_DEN)` is non-compliant because it can admit a chunk whose `delta_B` exceeds available B headroom when `R` is large.

Then atomically write:

```text
B_side_num += delta_B
social_loss_remainder_side_num = new_rem
residual_remaining -= chunk
```

Exact chunk conservation:

```text
delta_B * W + new_rem - R = chunk * SOCIAL_LOSS_DEN
```

If `residual_remaining > 0`, the bankrupt close remains active in `ResidualPartiallyBooked` and any keeper can continue it.

### 5.4 ADL quantity socialization and B residual socialization

Bankruptcy residual is socialized by B, not by a dense K scan. K is for mark-price PnL. B is for bankruptcy residual loss.

Full-close liquidation order:

1. Combined-settle B and A/K/F for the liquidated account.
2. Settle negative PnL from principal.
3. Clear/close local exposure through canonical helpers without mutating OI locally.
4. Charge liquidation fee only after losses are settled.
5. Compute the exact remaining deficit.
6. Spend insurance first.
7. Book the remaining residual to B on the opposing side in one or more chunks, or record explicit unallocated loss when no side weight exists.
8. Only after residual is fully booked/recorded may the negative PnL be cleared and the account freed.
9. Mutate global OI exactly once through the liquidation/ADL path.

If quantity ADL changes `A_opp`, the implementation MUST either prove that `loss_weight_sum_opp` remains a certified current effective-loss-bearing sum for future B booking, or mark the side uncertified for B booking until keeper touch/cleanup or an exact proof restores certification. An account that later floors to zero must touch-settle already-booked side effects, clear basis, flush B remainders as required, and remove its weight. Uncertified sides MAY continue settlement and cleanup, but MUST NOT receive new B residual loss.

### 5.5 Side reset and B epochs

`begin_full_drain_reset(side)` requires `OI_eff_side == 0` and side not already `ResetPending`. It snapshots and resets all side indices:

```text
K_epoch_start_side = K_side
F_epoch_start_side_num = F_side_num
B_epoch_start_side_num = B_side_num
transfer social_loss_remainder_side_num to social_loss_dust_side_num and realize whole atoms
K_side = 0
F_side_num = 0
B_side_num = 0
social_loss_remainder_side_num = 0
loss_weight_sum_side = 0
epoch_side += 1
A_side = ADL_ONE
stale_account_count_side = stored_pos_count_side
mode_side = ResetPending
```

Stale accounts settle against epoch-start K/F/B, then clear basis and transfer any `b_rem_i` into scaled dust. New-epoch accounts snapshot the reset side indices. A side already `ResetPending` MUST NOT be reset again. Resolved mode preserves existing reset-pending epoch-start K/F/B state.

---

## 6. Bankrupt close primitive

The engine exposes or internally uses:

```text
begin_or_continue_bankrupt_close(idx, price, now_slot, budget) -> CloseProgress
```

A public keeper may call this through Phase 1 liquidation when full close produces a deficit. Each successful call performs at most `budget` bounded substeps and MUST commit monotonic progress. Work is independent of total active accounts. `budget == 0` MUST reject before mutation.

Minimum phases:

```text
Touched:
    validate account; combined-settle B and A/K/F; settle losses from principal; sync loss-safe fees

PositionClosed:
    atomically remove q_close_q from global OI, clear or mark closed local basis through canonical helpers, update counts/weights, compute q_close_q; do not free account

DeficitComputed:
    compute exact remaining negative PnL plus required close fees/obligations minus available value

InsuranceApplied:
    spend min(deficit, I)

ResidualPartiallyBooked:
    book one or more residual chunks to B_opp or explicit unallocated bucket; trigger/restart h-max

ResidualBooked:
    residual_remaining == 0 and all loss is durable

AccountClosed:
    clear negative PnL through NoPositiveIncreaseAllowed, zero/finalize local state, free if eligible
```

The engine MUST persist the active close state before returning `ProgressOnly`. Any caller can continue it using the frozen economics. A close cannot skip `ResidualBooked`. A failed residual booking rolls back or routes to recovery; it MUST NOT leave the account cleared without a durable loss record.

No committed `ProgressOnly` phase may leave local exposure cleared while the matching global OI/count/weight deltas are uncommitted. Intermediate phases may keep a pre-close snapshot in `BankruptCloseState`, but public invariants must remain true after each committed progress step.

`CloseProgress` has disjoint outcomes:

```text
ProgressOnly { phase, residual_remaining }
ResidualChunkBooked { chunk, residual_remaining, side }
ResidualBooked { residual_total, side }
Closed { idx }
NoopNotLiquidatable
RecoveryRequired
```

A zero return value MUST NOT be ambiguous between progress and closed-with-zero-payout.

---

## 7. Operations

### 7.1 Live touch lifecycle

Every live account-touching endpoint that can move value or risk uses:

```text
1. validate clock, oracle/effective price, funding, admission, endpoint inputs
2. if an active bankrupt close exists, continue it or fail before mutation
3. initialize ctx
4. accrue or bounded-catchup exactly as allowed
5. touch account(s): combined-settle B and A/K/F, apply reserves, settle negative PnL
6. sync recurring fees after loss settlement when needed
7. run endpoint checks under final hmax_effective_active(ctx)
8. finalize touched accounts exactly once
9. schedule/finalize resets
10. assert all invariants
```

If h-max activates after any staged positive-PnL usability, recompute/reverse before commit.

### 7.2 Deposit, withdraw, convert, close

Deposits are pure capital paths and may materialize only with `amount > 0`. They MUST not accrue. They MUST reject if value inflow violates `MAX_VAULT_TVL` or conservation. If deposit loss settlement leaves live `PNL_i < 0`, it MUST trigger bankruptcy h-max or reject/reroute.

Withdraw uses candidate post-withdraw local `C_i - amount`, `C_tot - amount`, and `V - amount`. Nonflat withdrawals reject while loss-stale catchup remains. During h-max/stress/B-stale conditions, nonflat withdrawal approval uses `Eq_withdraw_no_pos_i`.

Conversion requires no h-max/B-stale/loss-stale positive-PnL lock. It consumes released PnL, converts only haircutted amount to capital, sweeps fees, and rechecks health.

Live close requires authoritatively flat (`basis_pos_q_i == 0`), zero PnL, no reserve, no fee debt, fee-current state, and no active loss-stale interval. It pays capital, zeroes through `set_capital`, reduces `V`, transfers any scaled B dust, and frees the slot.

### 7.3 Trade

Trade requires loss-current market state (`slot_last == current_slot`), current B/K/F settlement for both counterparties, side-mode gating, OI bounds, position bounds, and fee-current state. It touches both accounts in deterministic ascending index order, settles losses before fees, removes the candidate trade's own positive slippage from approval, and uses no-positive-credit lanes while h-max/stress is active. It MUST NOT execute while bounded catchup remains incomplete.

### 7.4 Liquidation

Liquidation requires loss-current market state before any OI-changing liquidation/ADL execution. During bounded catchup, keepers may touch and revalidate but MUST NOT execute OI-changing liquidation until catchup completes or recovery resolves.

Full-close liquidation that discovers post-principal deficit MUST use the bankrupt close primitive. It MUST NOT scan the opposing book.

Partial liquidation is allowed only if it leaves the account maintenance healthy after fees and ADL quantity update. If partial liquidation would create a post-principal deficit, it MUST be treated as bankrupt close or rejected.

### 7.5 Keeper crank

Keeper crank is live-only, bounded, and incremental. It has Phase 1 candidate processing and Phase 2 round-robin sweep. It MUST bound both candidate entries inspected and successful revalidations. Missing, duplicate, malformed, already-flat, and nonliquidatable entries count against inspection budget.

If a crank performs equity-active accrual on an exposed market, it MUST also commit at least one protective progress unit: materialized candidate touch/revalidation, bankrupt-close phase progress, residual B chunk booking, liquidation execution, authenticated Phase 2 missing-slot inspection, or materialized Phase 2 touch. It MUST NOT require all suspect accounts to fit in the current instruction.

Phase 2 advances `rr_cursor_position` over authenticated index space, touches up to `rr_touch_limit`, inspects up to `rr_scan_limit`, and advances `sweep_generation` at most once per authenticated slot. Same-instruction stress/h-max starts or restarts do not count same-instruction Phase 2 progress toward clearing the new envelope.

### 7.6 Bounded stale catchup

If `authenticated_now_slot - slot_last > cfg_max_accrual_dt_slots`, public crank-forward markets use one bounded segment:

```text
remaining_dt = authenticated_now_slot - slot_last
segment_dt = min(remaining_dt, cfg_max_accrual_dt_slots)
segment_slot = slot_last + segment_dt
```

The segment accrues K/F/price/`slot_last` to `segment_slot`, while `current_slot` and generation-rate limits use `authenticated_now_slot`. If `slot_last < current_slot` after the segment, the market is loss-stale:

- positive PnL uses h-max/no-positive-credit lanes;
- reserves do not release;
- conversion/auto-conversion is disabled;
- trades, nonflat withdraw/close, and OI-changing liquidation/ADL are blocked;
- keeper touch/revalidation and canonical floor-to-zero cleanup may continue;
- nonflat fee anchors are capped by loss-accrued `slot_last`.

If the next segment cannot be represented because of price floor, oracle/target unavailability, K/F/B headroom, or counter overflow, public CrankForward markets MUST route to permissionless recovery.

---

## 8. Resolution and recovery

### 8.1 Permissionless recovery resolve

A CrankForward market MUST expose permissionless terminal recovery for any state where ordinary bounded progress cannot continue and the state is not proven impossible at initialization.

Permitted recovery reasons include:

```text
BelowProgressFloor
BlockedSegmentHeadroomOrRepresentability
BIndexHeadroomExhausted
OracleOrTargetUnavailableByAuthenticatedPolicy
CounterOrEpochOverflowDeclaredRecovery
```

The caller cannot choose the price. Recovery uses a deterministic authenticated recovery price if available and representable. If unavailable/unusable and immutable policy enables fallback, it settles at `P_last`. Caller omission/corruption of proof MUST NOT force fallback.

Recovery enters `Resolved`, computes terminal K deltas from pre-resolution state, snapshots K/F/B epoch-start state for reset sides, zeros OI, transfers scaled B remainders/dust according to §2.3, and does not capture payout snapshot. Positive payouts still require resolved readiness.

### 8.2 Privileged resolution

Ordinary resolution live-syncs first, checks deviation band, computes terminal K deltas before zeroing OI or resetting sides, and enters `Resolved`. Degenerate resolution requires explicit mode, `live_oracle_price == P_last`, and funding rate zero.

Terminal K deltas:

```text
resolved_k_long_terminal_delta = 0 if long side ResetPending or OI_long == 0
else A_long * (resolved_price - resolved_live_price)

resolved_k_short_terminal_delta = 0 if short side ResetPending or OI_short == 0
else -A_short * (resolved_price - resolved_live_price)
```

Resolved mode preserves/snapshots B:

```text
if side already ResetPending: preserve B_epoch_start_side_num
else if side has stored positions: begin_full_drain_reset(side), including B snapshot and dust transfer
```

### 8.3 Resolved close

Resolved close is permissionless and bounded. It combined-settles B and terminal K/F, clears reserve metadata, settles negative PnL from principal then insurance/unallocated loss, syncs recurring fees to `resolved_slot`, and only then may return `ProgressOnly`, pay, forgive fee debt, or free.

Positive payout readiness requires:

```text
stale_account_count_long == 0
stale_account_count_short == 0
stored_pos_count_long == 0
stored_pos_count_short == 0
neg_pnl_account_count == 0
PNL_matured_pos_tot == PNL_pos_tot
```

The positive payout snapshot is captured once after readiness and remains stable. Positive close consumes the full PnL claim and pays only the snapshotted haircutted amount.

---

## 9. Wrapper obligations

1. Wrappers own authorization, oracle normalization, raw-target storage, effective-price staircase policy, account proof packing, and anti-spam economics.
2. Public wrappers MUST not expose caller-controlled admission, funding, threshold, or future-slot inputs.
3. Public CrankForward wrappers MUST expose bounded catchup, cursor-priority Phase 2, bounded candidate inspection, and permissionless recovery for non-catchupable states unless initialization proves every non-catchupable state impossible.
4. Candidate list capacity MUST produce partial progress, not rollback because more candidates exist.
5. Cursor account/proof packing MUST allow honest keepers to supply the current Phase 2 cursor account even when the candidate list is full.
6. If recurring fees are enabled, wrappers MUST sync them after authoritative loss settlement and before health-sensitive checks or payouts; during bounded catchup, nonflat/stale fee anchors are capped at `slot_last`.
7. Target/effective lag MUST not give users a free option. Extraction-sensitive actions reject or shadow-check; risk-increasing trades use dual-price/no-positive-credit checks.
8. Wrappers MUST not advertise BestEffort markets as worst-case crank-forward live.
9. Wrappers MUST expose resolved-close account/proof packing so one unreachable resolved account cannot indefinitely block payout readiness.
10. Wrappers MUST disclose emergency `P_last` recovery semantics when enabled.
11. Wrappers MUST ensure an active bankrupt close cannot be bypassed by routing a different endpoint; callers must continue the active close, complete it, or recover.

---

## 10. Required TDD / proof coverage

Implementations MUST include tests or proofs for at least the following.

### 10.1 B-index bankruptcy socialization

1. `bankrupt_full_close_books_residual_without_opposing_scan`: one bankrupt account and thousands of opposing accounts; bounded close books residual through B without scanning the opposing book.
2. `b_booking_exact_remainder_conservation`: for non-divisible `chunk * SOCIAL_LOSS_DEN`, assert `delta_B * W + rem_new - rem_old == chunk * SOCIAL_LOSS_DEN`.
3. `b_booking_chunks_large_residual`: residual larger than one-step representability books over multiple O(1) calls, monotonically reducing `residual_remaining`.
4. `b_booking_min_weight_does_not_force_recovery_for_vault_sized_loss`: with `W = 1`, a residual up to `MAX_VAULT_TVL` books normally under the configured denominator.
5. `lazy_b_settlement_matches_eager_weighted_loss_mod_remainders`: after B booking, touching all affected accounts realizes the same aggregate scaled loss as eager weighted allocation modulo global and per-account remainders.
6. `b_settlement_no_double_charge`: touching the same account twice after one B update realizes loss only once.
7. `b_stale_blocks_positive_pnl_escape`: stale `b_snap_i` blocks withdraw, close, convert, auto-convert, risk-increasing positive-credit approval, detach/replace, and resolved payout until B settlement.
8. `b_settlement_combines_with_kf_before_principal_loss`: B loss and K/F gain/loss are netted in one side-effect settlement before principal is drained.
9. `b_settlement_can_trigger_second_bankruptcy`: net side-effect settlement that exhausts principal starts/restarts bankruptcy h-max before commit.
10. `zero_weight_opposing_side_records_explicit_loss`: if `loss_weight_sum_opp == 0`, residual is recorded explicitly and the bankrupt account is not cleared before that record exists.
11. `b_overflow_does_not_clear_account`: if no positive B chunk is representable, the instruction rolls back or enters recovery; account negative PnL is not cleared.
12. `b_epoch_reset_stale_accounts_settle_against_epoch_start`: side reset snapshots B, resets current B, and stale accounts settle exactly once against `B_epoch_start`.
13. `b_weight_sum_incremental_consistency`: attach/clear/trade/liquidation/reset maintain `loss_weight_sum_side` equal to the sum of current-epoch account weights.
14. `b_remainders_are_not_dropped`: account clear and side reset transfer `b_rem_i` and global B remainder into scaled dust, and scaled dust realization creates whole-atom explicit loss when it crosses `SOCIAL_LOSS_DEN`.
15. `active_bankrupt_close_freezes_loss_bearing_set`: after deficit computation and before residual fully booked, attach/clear/trade/reset on the opposing side rejects unless continuing the active close or recovering.
16. `active_bankrupt_close_uses_frozen_economics`: continuation cannot replace close price, slot, fee obligation, side epoch, close quantity, or residual amount.
17. `weight_change_flushes_global_b_remainder`: attach/clear/replacement/trade/reset that changes `loss_weight_sum_side` first transfers the old global B remainder into scaled dust and sets the side remainder to zero.
18. `zero_effective_account_not_charged_by_future_b`: after an account is known to be zero-effective, later residual booking either excludes it from the certified weight sum or records/routes the residual without mutating B for that side.

### 10.2 Bankrupt close progress

19. `bankrupt_close_phase_monotonic`: every successful call advances phase, books residual, closes account, or advances durable cursor/touch state.
20. `cannot_free_before_residual_booked`: full close cannot clear negative PnL or free slot before insurance and B/explicit residual booking are durable.
21. `position_closed_phase_preserves_public_invariants`: no committed progress step clears local basis without the matching global OI/count/weight deltas.
22. `explicit_unallocated_loss_is_durable_before_clear`: if no B side is eligible, the negative tail is cleared only after the exact residual is recorded as explicit unallocated loss; the record does not increase payout capacity.
23. `any_keeper_can_continue_active_close`: active close state cannot be held hostage by the starter.
24. `dense_more_than_candidate_cap_eventually_clears`: 129, 1,000, and >candidate-cap liquidatable accounts clear through repeated bounded honest cranks.
25. `candidate_padding_counts_against_inspection_not_revalidation`: malformed/missing/duplicates cannot force unbounded compute and do not roll back valid partial progress.

### 10.3 Core accounting and safety

26. Conservation across all endpoints: `C_tot <= V`, `I <= V`, `V >= C_tot + I`, and `V <= MAX_VAULT_TVL`.
27. PnL aggregates and `neg_pnl_account_count` remain exact.
28. Risk notional uses ceil rounding; fractional positions cannot evade margin.
29. A/K/F settlement uses exact signed floor with `FUNDING_DEN`; truncation toward zero fails tests.
30. All K/F candidates are computed and future-headroom-checked before persistent writes.
31. Price-move cap rejection happens before K/F/price/slot/stress mutation.
32. No account-free exposed equity-active accrual commits without protective progress.
33. Bounded stale catchup advances in subtraction-first segments and preserves loss-stale locks.
34. Same-instruction h-max activation retroactively blocks/reclassifies positive-PnL usability.
35. Fees never outrank unsettled losses; recurring fees anchor to `slot_last` during loss-stale catchup.
36. Nonflat withdrawal approval uses candidate post-withdraw local `C_i - amount` and the correct no-positive-credit lane when stressed.
37. Side-mode gates prevent DrainOnly exposure replacement and ResetPending fresh exposure.
38. Phantom/potential dust cannot clear OI unless certified; orphan-exposure reset runs instead of deadlocking.
39. Same-slot repeated cursor wraps cannot clear h-max; sweep generation advances at most once per slot.
40. Resolved close syncs fees to `resolved_slot` before `ProgressOnly`, payout, fee forgiveness, or free.
41. Permissionless recovery cannot be invoked with caller-chosen price, omitted valid proof, malformed proof, or while ordinary bounded catchup can still progress.
42. CrankForward initialization rejects configurations without permissionless recovery unless non-catchupable states are proven impossible.
43. Resolved terminal K and B epoch snapshots are computed before OI zeroing/reset and are applied exactly once to stale accounts.
44. Authoritatively flat accounts (`basis_pos_q_i == 0`) never receive B loss; stored zero-effective basis settles already-booked side effects once, then canonical cleanup removes its weight.

---

## 11. Summary of v12.20.1 changes

v12.20.0 introduced B-index bankruptcy socialization but had four material edge cases: an overlarge denominator could make tiny opposing exposure force recovery, B loss could be settled before same-touch K/F gains, B remainders could be dropped during clear/reset, and active bankrupt closes did not explicitly freeze the loss-bearing set before residual booking.

v12.20.1 fixes those by using a denominator sized to the provable maximum side weight, chunking residual bookings, requiring combined B+K/F settlement before principal loss settlement, carrying every fractional B remainder into explicit scaled dust, adding an active-close residual-booking barrier, freezing active-close economics, requiring B-remainder flushes before weight-sum changes, and making CrankForward recovery requirements explicit.
