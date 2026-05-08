# Risk Engine Spec (Source of Truth) — v12.20.6

**Design:** protected principal + junior profit claims + lazy A/K/F/B side indices.  
**Scope:** one perpetual DEX risk engine for one quote-token vault.  
**Status:** normative source-of-truth draft. Terms **MUST**, **MUST NOT**, **SHOULD**, **MAY** are normative.

This revision supersedes v12.20.5. It keeps the B-index bankruptcy design and fixes the remaining worst-case liveness issue from the final pass: **a single stale account may itself have a huge B delta**. Public progress MUST NOT require applying that entire account-local B loss in one `i128` PnL mutation, and MUST NOT require scanning the market to decide what to do. v12.20.6 therefore adds **account-local B settlement chunks** plus a bounded recovery rule. Together with deterministic residual chunks, active-close chunk caps, and cursor-based keeper progress, no public instruction requires evaluating the entire market in one atomic instruction.

Every top-level instruction is atomic. Any failed precondition, checked arithmetic guard, missing authenticated proof, context-capacity overflow, or conservative-failure condition MUST roll back every mutation performed by that instruction. Before commit, every top-level instruction MUST leave all global invariants in §2 true. An active `BankruptCloseState` is permitted only as a bounded progress lock with staged economics; it MUST NOT require relaxed OI, stored-position, conservation, or loss-weight invariants.

---

## 0. Non-negotiable safety and liveness requirements

1. **Authoritatively flat protected principal is senior.** An account with `basis_pos_q_i == 0` MUST NOT lose protected principal because another account went bankrupt. A nonzero stored basis whose effective quantity floors to zero is not authoritatively flat until combined A/K/F/B settlement and canonical clear have run.
2. Bankrupt losses are visible state. Open opposing positions MAY absorb bankrupt residual only through B-side indices, account-local B snapshots/remainders, and explicit non-claim audit buckets. Hidden full-book execution is forbidden.
3. Bankrupt residual socialization is bounded. A close/crank MUST NOT inspect or touch the full opposing book to advance one bankrupt account.
4. A bankrupt account's post-principal residual MUST be durably booked to B, explicitly recorded, or carried into recovery before its negative PnL is cleared, basis is cleared, OI is decremented, fee debt is forgiven, or the slot is freed.
5. B-index booking is exact per committed residual chunk:

```text
delta_B * W + rem_new - rem_old = chunk * SOCIAL_LOSS_DEN
```

where `W = loss_weight_sum_side` at booking time.

6. A prior weight-set remainder MUST NOT be charged to a changed weight set. Before changing `loss_weight_sum_side`, quarantine `social_loss_remainder_side_num` into scaled dust and set it to zero.
7. No double charge: an account applies each B delta at most once by advancing `b_snap_i` to the settled B target or settled partial B target.
8. No positive-PnL escape: accounts with unsettled B loss, active close exposure, h-max/stress, or loss-stale catchup MUST NOT withdraw, close, convert/release PnL, auto-convert, use positive PnL for risk-increasing approval, detach/replace exposure, or receive resolved payout until the relevant settlement/lock clears.
9. B-induced insolvency is bankruptcy. If combined side-effect settlement exhausts `C_i` and leaves live `PNL_i < 0`, bankruptcy h-max MUST start/restart before commit.
10. Losses are senior to fees on the same account. Fees drawn from capital MUST NOT precede unsettled A/K/F/B losses. Uncollectible protocol/liquidation fees are dropped or forgiven; they MUST NOT be paid from insurance or socialized to users through B.
11. Dense markets clear incrementally. More liquidatable accounts than the candidate cap MUST produce bounded partial progress over repeated honest cranks, not rollback.
12. Account-free equity-active accrual is forbidden on exposed live markets. Price/funding movement that can change account equity requires keeper/account-touch progress or terminal recovery.
13. Public time is authenticated. Public wrappers MUST source `now_slot` from runtime/clock state and MUST NOT allow caller future slots to move engine time.
14. Raw oracle target and effective engine price are distinct. During target/effective lag, extraction and risk-increase paths MUST reject or use conservative dual-price/no-positive-credit checks.
15. Economic arithmetic MUST be checked or exact-wide. Non-claim audit counters MAY saturate only with explicit flags and MUST NOT affect payouts or liabilities.
16. Public user-fund markets MUST be crank-forward. A state that only a privileged actor can advance is non-compliant.
17. **No full-market atomic work.** Public instructions MUST NOT require scanning, recomputing, or validating all materialized accounts, all opposing accounts, or the whole account-index space in one atomic instruction. Full-market facts must be maintained incrementally, advanced through a durable cursor, proven by a bounded proof, or replaced by permissionless recovery.

---

## 1. Units, bounds, and configuration

### 1.1 Arithmetic and units

Persistent economic quantities use `u128` or `i128`; persistent signed fields MUST NOT equal `i128::MIN`. Transient products involving price, position, A/K/F/B, weights, fees, haircuts, active-close fields, and remainders MUST use an exact domain at least 256 bits wide.

```text
POS_SCALE                    = 1_000_000
ADL_ONE                      = 1_000_000_000_000_000
FUNDING_DEN                  = 1_000_000_000
SOCIAL_WEIGHT_SCALE          = ADL_ONE
SOCIAL_LOSS_DEN              = 1_000_000_000_000_000_000_000  // 1e21
STRESS_CONSUMPTION_SCALE     = 1_000_000_000
```

Every live, resolved, and effective engine price MUST satisfy `0 < price <= MAX_ORACLE_PRICE`.

```text
RiskNotional_i = 0 if effective_pos_q(i) == 0
else ceil(abs(effective_pos_q(i)) * price / POS_SCALE)

trade_notional = floor(size_q * exec_price / POS_SCALE)
```

### 1.2 Hard bounds

```text
MAX_VAULT_TVL                         = 10_000_000_000_000_000
MAX_ORACLE_PRICE                      = 1_000_000_000_000
MAX_POSITION_ABS_Q                    = 100_000_000_000_000
MAX_TRADE_SIZE_Q                      = MAX_POSITION_ABS_Q
MAX_OI_SIDE_Q                         = 100_000_000_000_000
MAX_ACCOUNT_NOTIONAL                  = 100_000_000_000_000_000_000
MAX_PROTOCOL_FEE_ABS                  = 1_000_000_000_000_000_000_000_000_000_000_000_000
GLOBAL_MAX_ABS_FUNDING_E9_PER_SLOT   = 10_000
MAX_BPS                               = 10_000
MAX_MATERIALIZED_ACCOUNTS             = 1_000_000
MIN_A_SIDE                            = 100_000_000_000_000
MAX_WARMUP_SLOTS                      = u64::MAX
MAX_RESOLVE_PRICE_DEVIATION_BPS       = 10_000
```

With `a_basis_i >= MIN_A_SIDE`:

```text
MAX_LOSS_WEIGHT_PER_ACCOUNT = ceil(MAX_POSITION_ABS_Q * SOCIAL_WEIGHT_SCALE / MIN_A_SIDE) = 1e15
MAX_ACTIVE_POSITIONS_PER_SIDE <= 1e6
MAX_LOSS_WEIGHT_SUM <= SOCIAL_LOSS_DEN
```

### 1.3 Immutable configuration

Initialization MUST validate ordinary risk/fee/warmup/capacity bounds:

```text
0 < cfg_min_nonzero_mm_req < cfg_min_nonzero_im_req
0 <= cfg_maintenance_bps <= cfg_initial_bps <= MAX_BPS
0 <= cfg_max_trading_fee_bps <= MAX_BPS
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
```

Any market accepting public user funds MUST satisfy:

```text
cfg_public_liveness_profile == CrankForward
cfg_permissionless_recovery_enabled == true
cfg_max_bankrupt_close_chunks > 0
cfg_max_account_b_settlement_chunks > 0
cfg_public_b_chunk_atoms >= MAX_VAULT_TVL
for every conservative-failure class in §8.1, recovery is enabled or the class is proven impossible
if cfg_recovery_p_last_fallback_enabled == false, every valid authenticated recovery price is proven terminal-representable
```

`BestEffort` is allowed only for non-custodial tests, private simulation, or privileged/admin-operated deployments that do not custody public user funds.

### 1.4 Solvency and funding envelopes

Initialization MUST prove in exact wide arithmetic:

```text
ADL_ONE * MAX_ORACLE_PRICE * cfg_max_abs_funding_e9_per_slot * cfg_max_accrual_dt_slots <= i128::MAX
cfg_min_funding_lifetime_slots >= cfg_max_accrual_dt_slots
ADL_ONE * MAX_ORACLE_PRICE * cfg_max_abs_funding_e9_per_slot * cfg_min_funding_lifetime_slots <= i128::MAX
```

It MUST also prove for every integer `1 <= N <= MAX_ACCOUNT_NOTIONAL`:

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

### 1.5 B residual booking chunk

Let `H = u128::MAX - B_side_num`, `W = loss_weight_sum_side`, and `R = social_loss_remainder_side_num`.

```text
max_scaled = (H + 1) * W - 1        // exact wide; H+1 is not u128
if R > max_scaled: max_chunk_by_B = 0
else:              max_chunk_by_B = floor((max_scaled - R) / SOCIAL_LOSS_DEN)

engine_chunk = min(residual_remaining, max_chunk_by_B)
if engine_chunk > cfg_public_b_chunk_atoms:
    chunk = min(engine_chunk, cfg_public_b_chunk_atoms)
else:
    chunk = engine_chunk      // final or headroom-limited chunk
```

A successful B residual booking requires:

```text
W > 0
chunk > 0
delta_B = floor((chunk * SOCIAL_LOSS_DEN + R) / W) > 0
B_side_num + delta_B <= u128::MAX
new_remainder < W <= SOCIAL_LOSS_DEN
```

A caller MUST NOT choose a smaller chunk than the engine-determined `chunk`. If no positive chunk is representable while residual remains, the close MUST return `RecoveryRequired` or enter permissionless recovery.

### 1.6 Account-local B settlement chunk

A single account touch MUST NOT require one unbounded B realization. Let:

```text
B_remaining = B_target - b_snap_i
w = loss_weight_i
r = b_rem_i
L = per_touch_B_loss_limit    // <= i128::MAX and chosen so set_pnl candidate fits
```

The maximum B-index delta that realizes at most `L` quote atoms is:

```text
max_num = (L + 1) * SOCIAL_LOSS_DEN - 1      // exact wide
if r > max_num: max_delta_by_loss = 0
else:          max_delta_by_loss = floor((max_num - r) / w)

delta_B_settle = min(B_remaining, max_delta_by_loss, endpoint_or_engine_delta_budget)
```

A successful partial account-B settlement requires `delta_B_settle > 0`, computes:

```text
num = r + w * delta_B_settle
B_loss_chunk = floor(num / SOCIAL_LOSS_DEN)
b_rem_new = num % SOCIAL_LOSS_DEN
b_snap_i += delta_B_settle
b_rem_i = b_rem_new
```

If `B_remaining > 0` after the chunk, the account remains B-stale and no user value/risk-increase action may proceed. If no positive `delta_B_settle` is possible, the endpoint MUST return progress-required or enter permissionless recovery. No account-local B settlement may inspect other accounts.

---

## 2. State and invariants

### 2.1 Account state

Each materialized account stores:

```text
C_i, PNL_i, R_i
basis_pos_q_i, a_basis_i, k_snap_i, f_snap_i, epoch_snap_i
fee_credits_i <= 0 and != i128::MIN
last_fee_slot_i
loss_weight_i, b_snap_i, b_rem_i, b_epoch_snap_i
optional scheduled reserve bucket
optional pending reserve bucket
```

If `basis_pos_q_i == 0`, local A/K/F/B fields MUST be canonical:

```text
a_basis_i = ADL_ONE
k_snap_i = f_snap_i = 0
epoch_snap_i = 0
loss_weight_i = b_snap_i = b_rem_i = b_epoch_snap_i = 0
```

If `basis_pos_q_i != 0`:

```text
a_basis_i >= MIN_A_SIDE
loss_weight_i = ceil(abs(basis_pos_q_i) * SOCIAL_WEIGHT_SCALE / a_basis_i) > 0
b_rem_i < SOCIAL_LOSS_DEN
b_epoch_snap_i == epoch_snap_i
b_epoch_snap_i == epoch_side or (mode_side == ResetPending and b_epoch_snap_i + 1 == epoch_side)
```

### 2.2 Global invariants

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
0 < A_side <= ADL_ONE
if side is Normal and has current-epoch stored positions: A_side >= MIN_A_SIDE
0 <= OI_eff_side <= MAX_OI_SIDE_Q
if Live: OI_eff_long == OI_eff_short
0 <= stored_pos_count_side <= materialized_account_count
0 <= stale_account_count_side <= stored_pos_count_side
0 <= loss_weight_sum_side <= SOCIAL_LOSS_DEN
social_loss_remainder_side_num < SOCIAL_LOSS_DEN
social_loss_dust_side_num < SOCIAL_LOSS_DEN
rr_cursor_position < cfg_account_index_capacity
```

K/F headroom:

```text
abs(K_side) + A_side * MAX_ORACLE_PRICE <= i128::MAX
abs(F_side_num) + A_side * MAX_ORACLE_PRICE * cfg_max_abs_funding_e9_per_slot * cfg_max_accrual_dt_slots <= i128::MAX
```

B epoch rules:

```text
Normal/DrainOnly side: B_side_num is current-epoch B and loss_weight_sum_side is the sum of current-epoch nonzero-basis weights.
ResetPending side: B_epoch_start_side_num is the B target for stale accounts from the prior epoch; current B_side_num/loss_weight_sum_side are new-epoch state and may be zero.
```

Explicit/unallocated loss buckets are non-redeemable audit/reconciliation state. They can trigger h-max while live, but are not user liabilities, MUST NOT make conservation fail, and MUST NOT block resolved close or terminal market close after all accounts are closed.

### 2.3 Scaled dust and audit buckets

Whenever global B remainder or account-local `b_rem_i` is discarded by weight-set change, account clear, side reset, terminal close, or recovery, it MUST be transferred into scaled dust:

```text
total = social_loss_dust_side_num + rem_to_transfer
atoms = floor(total / SOCIAL_LOSS_DEN)
social_loss_dust_side_num = total % SOCIAL_LOSS_DEN
if atoms > 0: add atoms to explicit_unallocated_loss_side
```

Audit bucket overflow MUST NOT trap funds. Implementations MUST either use checked exact audit buckets and enter permissionless recovery that safely archives/resets non-claim audit state on overflow, or saturate the audit bucket at `u128::MAX` and set an explicit saturation flag. Saturation MUST NOT alter `V`, `C_tot`, `I`, `PNL_pos_tot`, payouts, or liabilities.

### 2.4 Side remainder quarantine

Before any operation changes `loss_weight_sum_side`, including attach, clear, replacement, reset, or recovery reclassification:

```text
if social_loss_remainder_side_num != 0:
    transfer it to social_loss_dust_side_num under §2.3
    social_loss_remainder_side_num = 0
```

While an active bankrupt close is in `DeficitComputed`, `InsuranceStaged`, or `ResidualPartiallyBooked`, weight-set changes on the opposing side are forbidden, so quarantine cannot evade pending residual.

### 2.5 Active bankrupt close state

At most one active close may exist unless independent close states are proven non-conflicting.

```text
BankruptCloseState {
  idx, generation
  phase in {Touched, SideEffectsPartiallySettled, ExposureStaged, DeficitComputed, InsuranceStaged, ResidualPartiallyBooked, ResidualBooked, AccountFinalized}
  close_price, close_slot
  liq_side, opp_side
  q_close_q
  loss_weight_sum_opp_at_deficit
  deficit_total
  insurance_to_spend
  residual_remaining
  residual_booked_total
  b_chunks_booked
  account_b_chunks_booked
}
```

Before `ResidualBooked`, the active close is a lock plus staged economics only:

```text
basis_pos_q_idx remains nonzero unless it was zero before close start
loss_weight_i remains included in current-epoch weight sum
stored_pos_count_side is not decremented
OI_eff is not decremented
PNL_i is not cleared
account is not freed
I is not decremented under the default staged-insurance model
```

Insurance withdrawals and unrelated insurance-consuming paths are blocked while staged insurance exists. Implementations MAY pre-decrement `I` only if the close state stores an exact prepaid-loss credit consumed exactly once by finalization or recovery.

Continuations use stored `close_price` and `close_slot`. They MUST NOT reprice, reaccrue, change `P_last`, change `slot_last`, advance `current_slot`, or run unrelated live mutation. If `b_chunks_booked >= cfg_max_bankrupt_close_chunks` and residual remains, the next continuation MUST enter permissionless terminal recovery rather than continuing indefinitely. If `account_b_chunks_booked >= cfg_max_account_b_settlement_chunks` and the closing account still cannot reach the B-current state needed for deficit computation, the close MUST enter permissionless recovery.

---

## 3. Claims, equity, and h-lock admission

```text
Residual = V - (C_tot + I)
PosPNL_i = max(PNL_i, 0)
FeeDebt_i = max(-fee_credits_i, 0)
ReleasedPos_i = PosPNL_i - R_i on Live; PosPNL_i on Resolved
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

Risk-increasing trade approval MUST remove the candidate trade's own positive slippage from that same account.

Valid admission pair:

```text
0 <= admit_h_min <= admit_h_max <= cfg_h_max
admit_h_max > 0
admit_h_max >= cfg_h_min
if admit_h_min > 0: admit_h_min >= cfg_h_min
```

`hmax_effective_active(ctx)` is true if threshold stress, bankruptcy h-lock, instruction-local bankruptcy candidate, loss-stale catchup, active bankrupt close, unsettled B on a positive-PnL-using endpoint, or partial account-B settlement is active. While true, fresh positive PnL uses `admit_h_max`; reserve release, reserve acceleration, conversion, auto-conversion, and positive-credit approvals are paused or recomputed under no-positive-credit lanes.

Same-instruction h-max activation is retroactive: earlier positive-PnL usability in the same atomic instruction MUST be staged, recomputed, reversed, or the instruction fails before commit.

---

## 4. Canonical helpers

### 4.1 Capital and position helpers

Every `C_i` mutation after materialization MUST use `set_capital` or an equivalent aggregate-updating path proving conservation. Every `basis_pos_q_i` mutation after materialization MUST use `set_position_basis_q` or equivalent stored-count updates.

`attach_effective_position_q(i, new_eff)` requires old side effects settled, side mode permits current-epoch attach, and no active close barrier. It quarantines side remainder, then writes:

```text
basis_pos_q_i = new_eff
a_basis_i = A_side >= MIN_A_SIDE
k_snap_i = K_side
f_snap_i = F_side_num
epoch_snap_i = epoch_side
loss_weight_i = ceil(abs(new_eff) * SOCIAL_WEIGHT_SCALE / A_side) > 0
loss_weight_sum_side += loss_weight_i, requiring <= SOCIAL_LOSS_DEN
b_snap_i = B_side_num
b_rem_i = 0
b_epoch_snap_i = epoch_side
```

The resulting `effective_pos_q(i)` MUST equal `new_eff`. Local attach/clear MUST NOT mutate global OI.

`clear_position_basis_q(i)` requires account B and A/K/F are settled to the target required by the caller. It quarantines side remainder, transfers `b_rem_i` to scaled dust, subtracts current-epoch `loss_weight_i`, clears A/K/F/B local fields, updates stored side count, and MUST NOT mutate OI.

### 4.2 Combined side-effect settlement

Authoritative touch prepares A/K/F and B candidates together before principal loss settlement. The engine MUST NOT drain capital for B before same-touch K/F gains/losses have been included.

For nonzero basis, B target is current `B_side_num` if `epoch_snap_i == epoch_side`; otherwise require `ResetPending` and use `B_epoch_start_side_num`. Require `b_epoch_snap_i == epoch_snap_i` and `b_snap_i <= B_target`.

A full combined settlement computes:

```text
ΔB = B_target - b_snap_i
num = b_rem_i + loss_weight_i * ΔB
B_loss_i = floor(num / SOCIAL_LOSS_DEN)
b_rem_candidate = num % SOCIAL_LOSS_DEN
KF_pnl_delta_i = exact signed-floor A/K/F settlement
net_pnl_delta_i = KF_pnl_delta_i - B_loss_i
```

If `B_loss_i` and `net_pnl_delta_i` are representable and endpoint budget permits, apply `set_pnl(PNL_i + net_pnl_delta_i, UseAdmissionPair)` in live mode or resolved equivalent, then atomically write B and K/F snapshots.

If full B settlement is not representable or not within endpoint budget, the engine MAY perform **partial B settlement** under §1.6. In that case it may also settle K/F once to current target, but the account remains B-stale until `b_snap_i == B_target`. A user value-moving/risk-increasing endpoint MUST stop after a partial B settlement and return progress-required; it MUST NOT continue to payout, close, convert, detach, attach, or approve risk increase. Keeper, bankrupt-close, and resolved-close paths MAY continue partial chunks over repeated bounded calls.

Only after the combined full or partial side-effect application may the caller settle negative PnL from principal.

### 4.3 PnL and fees

Every `PNL_i` mutation uses `set_pnl`. Positive increases in live mode go through admission. Negative cleanup with `NoPositiveIncreaseAllowed` is allowed only if it does not create a positive junior claim.

`settle_negative_pnl_from_principal(i, ctx)` pays negative PnL from `C_i`. If live `PNL_i < 0` remains after `C_i` reaches zero, bankruptcy h-max MUST start/restart before commit.

`charge_fee_to_insurance` pays from capital to insurance, records collectible fee shortfall as fee debt when appropriate, and drops uncollectible tails. It MUST be called only after senior losses are settled.

For a bankrupt-closing account:

```text
paid_fee = min(fee, remaining_positive_capital_after_loss_settlement)
unpaid_fee = fee - paid_fee
paid_fee may move C -> I
unpaid_fee is dropped or forgiven
unpaid_fee MUST NOT become persistent close-blocking debt
unpaid_fee MUST NOT be included in bankruptcy_loss, paid from insurance, or booked to B
```

At `AccountFinalized`, all remaining `fee_credits_i < 0` on the bankrupt account MUST be set to zero before free.

Recurring fee sync charges half-open `[last_fee_slot_i, anchor)`. During loss-stale catchup, nonflat/stale capital-drawn fee anchors are capped by loss-accrued `slot_last`.

---

## 5. A/K/F/B mechanics

### 5.1 Accrual

`accrue_market_to(now_slot, oracle_price, funding_rate)` requires live mode, no active bankrupt close, authenticated `now_slot >= current_slot`, `slot_last <= current_slot`, `now_slot >= slot_last`, valid price, and bounded funding rate.

```text
dt = now_slot - slot_last
funding_active = dt > 0 && funding_rate != 0 && OI_eff_long != 0 && OI_eff_short != 0 && fund_px_last > 0
price_move_active = P_last > 0 && oracle_price != P_last && (OI_eff_long != 0 || OI_eff_short != 0)
```

If either active branch is true, require `dt <= cfg_max_accrual_dt_slots`. If `price_move_active`, require before mutation:

```text
abs(oracle_price - P_last) * 10_000 <= cfg_max_price_move_bps_per_slot * dt * P_last
```

Compute K/F/stress candidates in exact wide arithmetic, validate K/F future headroom, then atomically set K/F/stress, `slot_last = current_slot = now_slot`, `P_last = oracle_price`, and `fund_px_last = oracle_price`. If candidate validation fails, no K/F/stress/price/slot field is written.

### 5.2 B residual booking

`book_bankruptcy_residual_chunk_to_side(ctx, side, residual_remaining, chunk_budget)` is O(1). It is allowed only when:

```text
mode_side != ResetPending
OI_eff_side > 0
loss_weight_sum_side > 0
loss_weight_sum_side <= SOCIAL_LOSS_DEN
active close barrier permits booking to this side
```

If no eligible weight exists, record remaining residual into explicit non-claim audit loss state and set `residual_remaining = 0`; this durable state triggers/restarts h-max.

If eligible, compute the engine-determined chunk under §1.5 and atomically write:

```text
B_side_num += delta_B
social_loss_remainder_side_num = new_rem
residual_remaining -= chunk
```

If residual remains, the active close stays in `ResidualPartiallyBooked`, increments `b_chunks_booked`, and forbids weight-set changes on that side.

### 5.3 Quantity ADL after residual durability

Bankruptcy residual is B-indexed; K is not used for bankruptcy residual. Quantity ADL is staged and applied exactly once at `AccountFinalized`, after residual durability.

For full close of `q_close_q` on `liq_side`:

```text
require q_close_q <= OI_eff_liq_side
OI_eff_liq_side -= q_close_q
opp = opposite(liq_side)
OI_opp_before = OI_eff_opp
require q_close_q <= OI_opp_before unless certified phantom/orphan recovery/reset proof applies
OI_opp_after = OI_opp_before - q_close_q
```

If `OI_opp_after == 0`, set opposing OI to zero, clear phantom dust for that side, and schedule/begin reset as needed. Otherwise compute:

```text
A_candidate = floor(A_opp * OI_opp_after / OI_opp_before)
```

If `A_candidate == 0`, zero both OI sides and schedule both resets as precision-exhaustion quantity ADL. If positive, set `A_opp = A_candidate`, `OI_eff_opp = OI_opp_after`, update phantom bounds conservatively, and enter `DrainOnly` if `A_opp < MIN_A_SIDE`.

This step MUST NOT change `loss_weight_i` or `loss_weight_sum_opp`. Any OI/A failure rolls back finalization. If finalization cannot represent the quantity-ADL result, enter permissionless recovery; do not leave residual booked with half-applied local close.

### 5.4 Side reset and B epochs

`begin_full_drain_reset(side)` requires zero OI, side not already `ResetPending`, and no active close barrier. It snapshots and resets:

```text
K_epoch_start_side = K_side
F_epoch_start_side_num = F_side_num
B_epoch_start_side_num = B_side_num
quarantine_social_remainder_before_weight_change(side)
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

Stale accounts settle against epoch-start K/F/B, clear basis, and transfer `b_rem_i` to scaled dust. A side already `ResetPending` MUST NOT be reset again.

---

## 6. Bankrupt close primitive

```text
begin_or_continue_bankrupt_close(idx, price, now_slot, budget) -> CloseProgress
```

`budget == 0` rejects before mutation. If no active close exists, starting a close requires loss-current market, full-close liquidation eligibility proven at `close_price`, account touched, combined side effects fully settled or a bounded `SideEffectsPartiallySettled` active-close phase recorded, and loss-safe fee state. If active close exists, the call continues that close, completes it, or recovers.

Minimum phases:

```text
Touched:
    validate account; store close_price/close_slot; begin combined side-effect settlement

SideEffectsPartiallySettled:
    apply bounded account-local B settlement chunks until closing account is B-current;
    K/F may be settled once; no deficit is computed until side effects are sufficiently current

ExposureStaged:
    compute q_close_q and side info; do not clear basis, decrement OI, remove weight, clear PnL, or free

DeficitComputed:
    bankruptcy_loss = max(-PNL_after_principal, 0), excluding protocol/liquidation fees

InsuranceStaged:
    insurance_to_spend = min(bankruptcy_loss, I) without decrementing I under default staged model

ResidualPartiallyBooked:
    book chunks to B_opp or explicit non-claim loss; restart h-max; preserve frozen price/slot and weight set

ResidualBooked:
    residual_remaining == 0 and all residual loss is durable

AccountFinalized:
    spend staged insurance exactly once; clear negative PnL through NoPositiveIncreaseAllowed;
    forgive close-blocking fee debt; clear basis and B weight; apply quantity ADL exactly once;
    free if eligible; clear active close
```

A close cannot stay live forever through successful continuations. If side-effect chunks or residual chunks hit their configured maximum and the close is still not finalizable, the next bounded action MUST enter permissionless terminal recovery.

Outcomes are disjoint:

```text
ProgressOnly { phase, residual_remaining }
AccountBChunkSettled { b_remaining }
ResidualChunkBooked { chunk, residual_remaining, side }
ResidualBooked { residual_total, side }
Closed { idx }
NoopNotLiquidatable
RecoveryRequired
```

---

## 7. Operations

### 7.1 Standard live lifecycle

A live endpoint that can move value or risk:

1. validates clock, effective price, funding, admission, and inputs;
2. if active close exists, continues/completes it, recovers, or fails before unrelated mutation;
3. accrues or bounded-catches-up only when allowed; ordinary accrual is forbidden while active close exists;
4. combined-settles B and A/K/F before principal-loss settlement;
5. syncs fees only after senior losses are settled;
6. runs endpoint checks under final `hmax_effective_active(ctx)`;
7. finalizes touched accounts exactly once;
8. schedules/finalizes resets;
9. asserts all invariants.

### 7.2 User operations

Deposits are pure capital paths and may materialize only with `amount > 0`. Deposits into an account with unsettled B are loss-curing only and MUST NOT enable withdrawal, close, conversion, or risk increase before combined settlement.

Withdrawals use candidate post-withdraw `C_i - amount`, `C_tot - amount`, and `V - amount`. Nonflat withdrawals reject while loss-stale catchup remains. During h-max/stress/B-stale/active-close conditions, approval uses no-positive-credit lanes after combined settlement.

Conversion requires no h-max/B-stale/loss-stale/active-close positive-PnL lock. Live close requires authoritatively flat, zero PnL, no reserve, no fee debt, fee-current state, no active close, and no loss-stale interval.

### 7.3 Trade and liquidation

Trade requires loss-current market state, no active close, current B/K/F settlement for both counterparties, side-mode gating, OI/position bounds, fee-current state, deterministic touch order, candidate-slippage neutralization, and no-positive-credit lanes while h-max/stress is active. It MUST NOT execute while bounded catchup remains incomplete. The wrapper supplies the current trade fee; the engine MUST reject `trade_fee_bps > cfg_max_trading_fee_bps`, charge the supplied fee atomically inside the trade transition, and include the resulting fee impact in margin/conservation checks.

Liquidation requires loss-current market state before OI-changing execution. During bounded catchup, keepers may touch/revalidate but MUST NOT execute OI-changing liquidation until catchup completes or recovery resolves. Full-close liquidation that discovers post-principal deficit MUST use the bankrupt close primitive and MUST NOT scan the opposing book. Partial liquidation that would create a post-principal deficit MUST be treated as bankrupt close or rejected.

### 7.4 Keeper crank and bounded catchup

Keeper crank is bounded and incremental. It has bounded candidate inspection, bounded revalidation, and Phase 2 cursor sweep. Candidate padding/missing/duplicates count against inspection budget, not revalidation budget. Candidate-list capacity produces partial progress, not rollback because more candidates exist.

If a crank performs equity-active accrual on an exposed market, it MUST also commit at least one protective progress unit: candidate touch/revalidation, active-close phase progress, account-B chunk, residual-B chunk, liquidation execution, authenticated Phase 2 missing-slot inspection, or materialized Phase 2 touch.

If `authenticated_now_slot - slot_last > cfg_max_accrual_dt_slots`, use subtraction-first bounded segments:

```text
remaining_dt = authenticated_now_slot - slot_last
segment_dt = min(remaining_dt, cfg_max_accrual_dt_slots)
segment_slot = slot_last + segment_dt
```

If `slot_last < current_slot` after the segment, the market is loss-stale: positive PnL uses h-max/no-positive-credit lanes; reserves do not release; conversion/auto-conversion is disabled; trades, nonflat withdraw/close, and OI-changing liquidation/ADL are blocked; keeper touch/revalidation and floor-to-zero cleanup may continue; nonflat fee anchors are capped by `slot_last`.

### 7.5 Resolved close cursor progress

Resolved markets MUST expose bounded permissionless account-close progress. A resolved close call may settle at most one account, or a bounded set of accounts if context permits; it MUST NOT require scanning all unresolved accounts to decide payout readiness. Readiness is tracked by aggregates: stale counts, stored counts, negative-PnL count, and PnL aggregates. Cursor/account-priority wrappers MUST let honest keepers supply each blocking account over bounded calls.

---

## 8. Resolution and recovery

### 8.1 Permissionless terminal recovery

A public CrankForward market MUST expose permissionless terminal recovery for any state where ordinary bounded progress cannot continue. Recovery reasons include:

```text
BelowProgressFloor
BlockedSegmentHeadroomOrRepresentability
AccountBSettlementCannotProgress
BIndexHeadroomExhausted
ActiveBankruptCloseCannotProgress
ExplicitLossOrDustAuditOverflow
OracleOrTargetUnavailableByAuthenticatedPolicy
CounterOrEpochOverflowDeclaredRecovery
```

The caller cannot choose the price. Recovery uses deterministic authenticated recovery price when available and representable; if unavailable/unusable and immutable fallback allows it, recovery settles at `P_last`. Caller omission/corruption MUST NOT force fallback.

If recovery occurs while active close exists, recovery MUST complete the active close under recovery semantics before clearing `BankruptCloseState`:

```text
use stored close_price/close_slot
spend staged insurance exactly once or consume exact prepaid-loss credit
record residual_remaining as durable explicit non-claim loss or recovery B/resolved-loss state
clear negative PnL only after durable record exists
forgive remaining fee debt
apply staged quantity ADL or equivalent resolved terminal close accounting exactly once
free/finalize the account if eligible
```

Recovery MUST NOT abort a partially booked close while leaving booked B loss and original negative PnL to be absorbed again. It MUST NOT drop `residual_remaining`, double-spend staged insurance, clear PnL without durable loss state, or discard booked B chunks.

Recovery enters `Resolved`, computes terminal K deltas from pre-resolution state, snapshots K/F/B epoch-start state for reset sides, zeros OI, transfers scaled B remainders/dust, and does not capture the positive-payout snapshot.

### 8.2 Privileged resolution

Ordinary resolution live-syncs first, checks deviation band, computes terminal K deltas before OI zeroing/reset, and enters `Resolved`. Degenerate resolution requires explicit mode, `live_oracle_price == P_last`, and funding rate zero. Privileged resolution MUST reject while active close exists unless it completes that close exactly as §8.1 requires.

Terminal K deltas:

```text
resolved_k_long_terminal_delta = 0 if long side ResetPending or OI_long == 0
else A_long * (resolved_price - resolved_live_price)

resolved_k_short_terminal_delta = 0 if short side ResetPending or OI_short == 0
else -A_short * (resolved_price - resolved_live_price)
```

Resolved mode preserves/snapshots B. A side already `ResetPending` preserves `B_epoch_start_side_num`; a side with stored positions begins full drain reset, including B snapshot and dust transfer.

### 8.3 Resolved close

Resolved close is permissionless and bounded. It combined-settles B and terminal K/F, clears reserve metadata, settles negative PnL from principal then insurance/unallocated audit loss, syncs recurring fees to `resolved_slot`, and only then may return `ProgressOnly`, pay, forgive fee debt, or free.

Positive payout readiness requires zero stale counts, zero stored counts, zero negative-PnL count, and `PNL_matured_pos_tot == PNL_pos_tot`. The payout snapshot is captured once after readiness and remains stable. Positive close consumes the full PnL claim and pays only the snapshotted haircutted amount.

---

## 9. Wrapper obligations

1. Wrappers own authorization, oracle normalization, raw-target storage, effective-price staircase policy, account proof packing, and anti-spam economics.
2. Public wrappers MUST not expose caller-controlled admission, funding, threshold, future-slot, B residual chunk-size, or account-B settlement chunk-size inputs.
3. Public user-fund wrappers MUST be CrankForward and expose bounded catchup, cursor-priority Phase 2, bounded candidate inspection, active-close continuation, account-B settlement continuation, and permissionless recovery for all non-catchupable states.
4. Cursor account/proof packing MUST allow honest keepers to supply the current Phase 2 cursor account even when candidate list is full.
5. Candidate capacity MUST produce partial progress, not rollback because more candidates exist.
6. If recurring fees are enabled, wrappers MUST sync them after authoritative loss settlement and before health-sensitive checks/payouts; during bounded catchup, nonflat/stale fee anchors are capped at `slot_last`.
7. Target/effective lag MUST not give users a free option. Extraction-sensitive actions reject or shadow-check; risk-increasing trades use dual-price/no-positive-credit checks.
8. Trade wrappers may compute dynamic fees, but they MUST pass the exact charged `trade_fee_bps` to the engine and MUST NOT use a lower fee for risk/mark influence than the fee charged.
9. Wrappers MUST disclose emergency `P_last` recovery semantics when enabled.
10. Active bankrupt close cannot be bypassed by a different endpoint. Callers must continue it, complete it, or recover.
11. Protocol/liquidation fee shortfall MUST NOT be included in B residual. Fee shortfall is dropped or forgiven under engine fee policy.

---

## 10. Required TDD / proof coverage

### 10.1 B-index and account-local B settlement

1. `bankrupt_full_close_books_residual_without_opposing_scan`.
2. `b_booking_exact_remainder_conservation`.
3. `b_booking_chunks_large_residual`.
4. `b_booking_min_weight_does_not_force_recovery_for_vault_sized_loss`.
5. `lazy_b_settlement_matches_eager_weighted_loss_mod_remainders`.
6. `b_settlement_no_double_charge`.
7. `b_stale_blocks_positive_pnl_escape`.
8. `account_b_settlement_chunks_huge_delta_without_market_scan`.
9. `account_b_partial_settlement_blocks_user_value_until_current`.
10. `account_b_settlement_chunk_formula_respects_i128_limit`.
11. `b_settlement_combines_with_kf_before_principal_loss`.
12. `b_settlement_can_trigger_second_bankruptcy`.
13. `zero_weight_opposing_side_records_explicit_loss`.
14. `b_overflow_does_not_clear_account`.
15. `b_epoch_reset_stale_accounts_settle_against_epoch_start`.
16. `b_epoch_snap_matches_position_epoch`.
17. `b_weight_sum_incremental_consistency`.
18. `b_remainders_are_not_dropped`.
19. `side_remainder_quarantined_before_weight_change`.
20. `audit_bucket_overflow_saturates_or_recovers_without_blocking_close`.

### 10.2 Bankrupt close progress

21. `bankrupt_close_phase_monotonic`.
22. `cannot_clear_basis_or_free_before_residual_booked`.
23. `active_close_preserves_public_invariants`.
24. `active_close_price_slot_fixed`.
25. `staged_insurance_not_double_spent`.
26. `bankruptcy_residual_excludes_protocol_fees`.
27. `bankrupt_close_fee_debt_forgiven_not_socialized`.
28. `active_close_quantity_adl_applied_exactly_once`.
29. `active_close_recovery_completes_close_no_double_count`.
30. `active_close_max_chunks_routes_to_recovery`.
31. `active_close_account_b_chunks_route_to_recovery`.
32. `any_keeper_can_continue_active_close`.
33. `dense_more_than_candidate_cap_eventually_clears`.
34. `candidate_padding_counts_against_inspection_not_revalidation`.

### 10.3 Core safety and liveness

35. Conservation across all endpoints: `C_tot <= V`, `I <= V`, `V >= C_tot + I`, and `V <= MAX_VAULT_TVL`.
36. PnL aggregates and `neg_pnl_account_count` remain exact.
37. Explicit/unallocated buckets are non-claim state and do not block terminal close.
38. Risk notional uses ceil rounding.
39. A/K/F settlement uses exact signed floor with `FUNDING_DEN`.
40. K/F candidates are checked before persistent writes.
41. Price-move cap rejection happens before K/F/price/slot/stress mutation.
42. No account-free exposed equity-active accrual commits without protective progress.
43. Bounded stale catchup advances in subtraction-first segments and preserves loss-stale locks.
44. Same-instruction h-max activation retroactively blocks/reclassifies positive-PnL usability.
45. Fees never outrank unsettled losses; recurring fees anchor to `slot_last` during loss-stale catchup.
46. Nonflat withdrawal approval uses candidate post-withdraw local `C_i - amount` and correct no-positive-credit lane.
47. Side-mode gates prevent DrainOnly exposure replacement and ResetPending fresh exposure.
48. Phantom/potential dust cannot clear OI unless certified; orphan-exposure reset runs instead of deadlocking.
49. Same-slot repeated cursor wraps cannot clear h-max; sweep generation advances at most once per slot.
50. Resolved close syncs fees to `resolved_slot` before `ProgressOnly`, payout, fee forgiveness, or free.
51. Permissionless recovery cannot be invoked with caller-chosen price, omitted valid proof, malformed proof, or while ordinary bounded catchup, account-B settlement, or active-close continuation can still progress.
52. Resolved terminal K and B epoch snapshots are computed before OI zeroing/reset and applied exactly once to stale accounts.
53. Authoritatively flat accounts never receive B loss; stored zero-effective basis settles booked side effects once, then cleanup removes weight.
54. `public_user_fund_markets_are_crankforward`: BestEffort cannot custody public user funds.
55. `no_single_instruction_full_market_requirement`: every endpoint has work bounded by candidate/rr/account-B/B-residual budgets, independent of total active accounts.
56. `worst_case_progress_totality`: from every valid nonterminal public state, at least one bounded permissionless action succeeds or enters terminal recovery.

---

## 11. Summary of v12.20.6 changes

v12.20.5 bounded residual B chunks, but this final pass tightened the account-local side: a single account with a huge stale B delta can now be settled through bounded account-local B chunks or routed to permissionless recovery. Active bankrupt close also tracks account-B chunk progress, so it cannot freeze the market while trying to make one account B-current. The final result preserves worst-case permissionless progress with fixed candidate, touch, scan, account-B, and residual-B bounds, never a full-market atomic evaluation.
