# Risk Engine Spec (Source of Truth) — v13.0.0

**Design:** protected principal + junior profit claims + lazy A/K/F/B side indices + lazy cross-margin portfolio accounts.
**Scope:** one cross-margin Percolator market group for one quote-token vault, with up to `N` configured assets per market group and per portfolio account.
**Status:** normative source-of-truth draft. Terms **MUST**, **MUST NOT**, **SHOULD**, **MAY** are normative.

This revision supersedes v12.20.6. It removes the finite global account slab and full-market cursor sweep as a safety dependency. Global account count is unbounded. Safety is preserved by bounded per-account verification, conservative health certificates, stale fail-closed rules, permissionless hinted recovery, and monotonic liquidation/rebalance progress.

The protocol no longer promises automatic discovery of every unhealthy account by scanning all accounts. It promises: if an honest crank supplies a valid account hint, the protocol can make bounded progress on that account, and no stale or omitted account can extract value or increase risk using optimistic health.

v13 is an architectural break from the slab-backed single-asset v12 engine. A compliant implementation stores each portfolio account as a distinct authenticated account bound to the MarketGroup; global aggregates are maintained incrementally and never proven by scanning all accounts.

Every top-level instruction is atomic. Any failed precondition, checked arithmetic guard, missing authenticated proof, context-capacity overflow, or conservative-failure condition MUST roll back every mutation performed by that instruction. Before commit, every top-level instruction MUST leave all global invariants true.

-------------------------------------------------------------------------------
0. Non-negotiable safety and liveness requirements
-------------------------------------------------------------------------------

1. Protected principal is senior. Positive PnL is junior and haircut-limited.
2. A hinted subset is never a correctness proof. Omitted or stale positions MUST NOT improve account health.
3. Every PortfolioAccount has at most `N` configured asset legs. Global account count is unbounded; per-account verification is bounded by `N`.
4. Any user-favorable operation MUST refresh the full active portfolio account first.
5. Stale certificates fail closed: stale accounts lose margin credit or gain risk penalties automatically.
6. Cranks use hints for discovery only. On-chain validation uses the account's actual active bitmap, conservative certificate, and touched state.
7. Rebalance MUST conserve senior claims and MUST NOT double-count collateral, positive PnL, insurance, provisional value, or bankruptcy-loss offsets.
8. Every successful liquidation/rebalance of an unhealthy account MUST strictly reduce a deterministic scalar risk score or liquidating deficit.
9. Bankruptcy losses are visible state and are absorbed only by insurance, explicit non-claim loss buckets, or B-side indices.
10. Account-local B settlement is chunked. No public instruction may require applying an unbounded B loss in one `i128` mutation.
11. No positive-PnL escape: B-stale, active-close, h-max/stress, loss-stale, stale-certificate, or partial-refresh accounts MUST NOT withdraw, close, convert/release PnL, use positive PnL for risk increase, or receive resolved payout until cleared.
12. Fees never outrank losses. Uncollectible protocol/liquidation fees are dropped or forgiven; they MUST NOT be paid from insurance or socialized through B.
13. Account-free equity-active accrual is forbidden on exposed live markets unless the operation also commits bounded protective progress.
14. Raw oracle targets and effective engine prices are distinct. During target/effective lag, extraction and risk-increase paths MUST reject or use conservative dual-price/no-positive-credit checks.
15. Public user-fund markets MUST be crank-forward. A state that only a privileged actor can advance is non-compliant.
16. No full-market atomic work. Public instructions MUST NOT scan, recompute, or validate all accounts or all opposing accounts in one atomic instruction.
17. No hidden legs. Active positions are defined only by a canonical active bitmap and deterministic leg array.
18. Markets MUST NOT rely on global aggregate health to prove a specific account is healthy.

-------------------------------------------------------------------------------
1. Units, bounds, and configuration
-------------------------------------------------------------------------------

Persistent economic quantities use `u128` or `i128`; persistent signed fields MUST NOT equal `i128::MIN`. Transient products involving price, position, A/K/F/B, weights, fees, haircuts, certificates, stale penalties, and remainders MUST use an exact domain at least 256 bits wide.

```text
POS_SCALE                    = 1_000_000
ADL_ONE                      = 1_000_000_000_000_000
FUNDING_DEN                  = 1_000_000_000
SOCIAL_WEIGHT_SCALE          = ADL_ONE
SOCIAL_LOSS_DEN              = 1_000_000_000_000_000_000_000
STRESS_CONSUMPTION_SCALE     = 1_000_000_000
MAX_BPS                      = 10_000
```

Every live, resolved, raw target, and effective engine price MUST satisfy `0 < price <= MAX_ORACLE_PRICE`.

```text
RiskNotional(asset, account) =
    0 if effective_pos_q == 0
    else ceil(abs(effective_pos_q) * effective_price / POS_SCALE)

trade_notional =
    floor(abs(size_q) * exec_price / POS_SCALE)
```

### 1.1 Hard bounds

```text
MAX_VAULT_TVL                         = 10_000_000_000_000_000
MAX_ORACLE_PRICE                      = 1_000_000_000_000
MAX_POSITION_ABS_Q_PER_ASSET          = 100_000_000_000_000
MAX_TRADE_SIZE_Q                      = MAX_POSITION_ABS_Q_PER_ASSET
MAX_OI_SIDE_Q_PER_ASSET               = 100_000_000_000_000
MAX_ACCOUNT_NOTIONAL_PER_ASSET        = 100_000_000_000_000_000_000
MAX_PORTFOLIO_ASSETS_N                = 16
MAX_PROTOCOL_FEE_ABS                  = 1_000_000_000_000_000_000_000_000_000_000_000_000
GLOBAL_MAX_ABS_FUNDING_E9_PER_SLOT   = 10_000
MAX_WARMUP_SLOTS                      = u64::MAX
MAX_RESOLVE_PRICE_DEVIATION_BPS       = 10_000
MIN_A_SIDE                            = 100_000_000_000_000
```

`N` MUST be small enough that full account refresh, liquidation validation, resolved close, and proof packing fit within the target runtime limits. Public user-fund initialization MUST reject `N` that cannot be refreshed in one bounded account-local instruction; it MUST NOT initialize and rely on later recovery for ordinary oversized portfolios.

With `a_basis >= MIN_A_SIDE`:

```text
MAX_LOSS_WEIGHT_PER_LEG =
    ceil(MAX_POSITION_ABS_Q_PER_ASSET * SOCIAL_WEIGHT_SCALE / MIN_A_SIDE)

For each asset side:
    loss_weight_sum_side <= SOCIAL_LOSS_DEN
```

### 1.2 Immutable configuration

Initialization MUST validate:

```text
0 < cfg_min_nonzero_mm_req < cfg_min_nonzero_im_req
0 <= cfg_maintenance_bps <= cfg_initial_bps <= MAX_BPS
0 <= cfg_max_trading_fee_bps <= MAX_BPS
0 <= cfg_liquidation_fee_bps <= MAX_BPS
0 <= cfg_min_liquidation_abs <= cfg_liquidation_fee_cap <= MAX_PROTOCOL_FEE_ABS
0 <= cfg_h_min <= cfg_h_max <= MAX_WARMUP_SLOTS
cfg_h_max > 0
0 <= cfg_resolve_price_deviation_bps <= MAX_RESOLVE_PRICE_DEVIATION_BPS
0 < cfg_max_accrual_dt_slots
0 <= cfg_max_abs_funding_e9_per_slot <= GLOBAL_MAX_ABS_FUNDING_E9_PER_SLOT
0 < cfg_max_price_move_bps_per_slot
0 < initial_oracle_price(asset) <= MAX_ORACLE_PRICE for every configured asset
0 < cfg_max_portfolio_assets <= MAX_PORTFOLIO_ASSETS_N
for every asset side:
    cfg_max_active_weight_per_side <= SOCIAL_LOSS_DEN
```

Public user-fund markets MUST satisfy:

```text
cfg_public_liveness_profile == CrankForward
cfg_permissionless_recovery_enabled == true
cfg_max_account_b_settlement_chunks > 0
cfg_max_bankrupt_close_chunks > 0
cfg_public_b_chunk_atoms >= MAX_VAULT_TVL
cfg_stale_certificate_penalty_enabled == true
cfg_full_refresh_required_for_favorable_actions == true
for every conservative-failure class, recovery is enabled or the class is proven impossible
```

`BestEffort` is allowed only for non-custodial tests, private simulation, or privileged/admin-operated deployments that do not custody public user funds.

### 1.3 Solvency and funding envelopes

For each asset, initialization MUST prove in exact wide arithmetic:

```text
ADL_ONE * MAX_ORACLE_PRICE * cfg_max_abs_funding_e9_per_slot * cfg_max_accrual_dt_slots <= i128::MAX
cfg_min_funding_lifetime_slots >= cfg_max_accrual_dt_slots
ADL_ONE * MAX_ORACLE_PRICE * cfg_max_abs_funding_e9_per_slot * cfg_min_funding_lifetime_slots <= i128::MAX
```

It MUST also prove for every integer `1 <= X <= MAX_ACCOUNT_NOTIONAL_PER_ASSET`:

```text
price_budget_bps      = cfg_max_price_move_bps_per_slot * cfg_max_accrual_dt_slots
funding_budget_num    = cfg_max_abs_funding_e9_per_slot * cfg_max_accrual_dt_slots * 10_000
loss_budget_num       = price_budget_bps * FUNDING_DEN + funding_budget_num
price_funding_loss_X  = ceil(X * loss_budget_num / (10_000 * FUNDING_DEN))
worst_liq_notional_X  = ceil(X * (10_000 + price_budget_bps) / 10_000)
liq_fee_raw_X         = ceil(worst_liq_notional_X * cfg_liquidation_fee_bps / 10_000)
liq_fee_X             = min(max(liq_fee_raw_X, cfg_min_liquidation_abs), cfg_liquidation_fee_cap)
mm_req_X              = max(floor(X * cfg_maintenance_bps / 10_000), cfg_min_nonzero_mm_req)
require price_funding_loss_X + liq_fee_X <= mm_req_X
```

For cross-margin offsets, initialization MUST additionally prove:

```text
maintenance_req(portfolio)
    >= sum(per_leg_price_funding_loss + per_leg_liquidation_fee)
```

for every allowed portfolio and every configured hedge-credit bucket. Hedge credit MAY reduce excess margin above this floor, but MUST NOT reduce the maintenance requirement below the sum of per-leg one-segment loss envelopes proven above.

-------------------------------------------------------------------------------
2. State
-------------------------------------------------------------------------------

### 2.1 MarketGroup

```text
MarketGroup {
    V                         // quote vault balance
    I                         // insurance bucket
    C_tot                     // total senior capital
    PNL_pos_tot
    PNL_matured_pos_tot
    materialized_portfolio_count
    stale_certificate_count
    b_stale_account_count
    negative_pnl_account_count

    risk_epoch
    oracle_epoch
    funding_epoch
    slot_last
    current_slot

    assets[0..N)
    global_stale_penalty_params
    optional active_bankrupt_close
    mode in {Live, Resolved, Recovery}
}
```

All counters are checked `u64` counters. Counter overflow fails closed: new account creation and the mutating operation that would overflow MUST reject or enter configured permissionless recovery before any value-moving mutation.

### 2.1.1 Account provenance and registry

The finite slab is removed, not account authentication. Every PortfolioAccount MUST be uniquely bound to exactly one MarketGroup by deterministic address/proof material containing:

```text
market_group_id
portfolio_account_id
owner authority
account version/layout discriminator
```

The engine/wrapper MUST reject any account whose provenance does not match the MarketGroup. Account creation increments `materialized_portfolio_count`; final close decrements it. Duplicate active accounts with the same `portfolio_account_id` are invalid by construction. Hidden or forged accounts MUST NOT affect aggregates, health, or terminal readiness.

### 2.2 Asset state

```text
Asset {
    raw_oracle_target_price
    effective_price
    fund_px_last

    A_long, A_short
    K_long, K_short
    F_long_num, F_short_num

    B_long_num, B_short_num
    B_epoch_start_long_num, B_epoch_start_short_num

    OI_eff_long, OI_eff_short
    stored_pos_count_long, stored_pos_count_short
    stale_account_count_long, stale_account_count_short

    loss_weight_sum_long, loss_weight_sum_short
    social_loss_remainder_long_num, social_loss_remainder_short_num
    social_loss_dust_long_num, social_loss_dust_short_num
    explicit_unallocated_loss_long, explicit_unallocated_loss_short

    epoch_long, epoch_short
    mode_long, mode_short in {Normal, DrainOnly, ResetPending}
}
```

### 2.3 PortfolioAccount

```text
PortfolioAccount {
    provenance_header          // market_group_id, portfolio_account_id, owner, version/layout
    owner

    C_i
    PNL_i
    R_i
    fee_credits_i <= 0 and != i128::MIN

    active_bitmap
    legs[0..N)

    health_cert
    stale_state
    b_stale_state
    rebalance_lock
    liquidation_lock
}
```

A flat inactive leg MUST be canonical:

```text
basis_pos_q = 0
a_basis = ADL_ONE
k_snap = f_snap = 0
epoch_snap = 0
loss_weight = b_snap = b_rem = b_epoch_snap = 0
```

A nonzero leg MUST satisfy:

```text
basis_pos_q != 0
a_basis >= MIN_A_SIDE
loss_weight = ceil(abs(basis_pos_q) * SOCIAL_WEIGHT_SCALE / a_basis) > 0
b_rem < SOCIAL_LOSS_DEN
b_epoch_snap == epoch_snap
b_epoch_snap == side_epoch or (side_mode == ResetPending and b_epoch_snap + 1 == side_epoch)
```

Any transition that makes an account stale, B-stale, or negative-PnL MUST update the corresponding MarketGroup counter exactly once. Clearing that condition MUST decrement exactly once. Favorable operations MUST validate the account-local flag and the relevant aggregate counter transition; they MUST NOT infer an individual account's freshness from an aggregate count alone.

### 2.4 HealthCert

```text
HealthCert {
    certified_equity
    certified_initial_req
    certified_maintenance_req
    certified_liq_deficit
    certified_worst_case_loss

    cert_oracle_epoch
    cert_funding_epoch
    cert_risk_epoch
    cert_effective_price_vector_hash
    active_bitmap_at_cert
    stale_penalty_accumulator
}
```

Certificate invariants:

```text
certified_equity          <= exact_conservative_equity(account)
certified_initial_req     >= exact_initial_requirement(account)
certified_maintenance_req >= exact_maintenance_requirement(account)
certified_liq_deficit     >= exact_liquidation_deficit(account)
```

If exactness is uncertain, the engine MUST round against the account.

-------------------------------------------------------------------------------
3. Global invariants
-------------------------------------------------------------------------------

```text
C_tot <= V <= MAX_VAULT_TVL
I <= V
V >= C_tot + I
PNL_matured_pos_tot <= PNL_pos_tot
0 < effective_price(asset) <= MAX_ORACLE_PRICE
0 < fund_px_last(asset) <= MAX_ORACLE_PRICE
slot_last <= current_slot
```

For every asset side:

```text
0 < A_side <= ADL_ONE
if side is Normal and has current-epoch stored positions: A_side >= MIN_A_SIDE
0 <= OI_eff_side <= MAX_OI_SIDE_Q_PER_ASSET
if Live: OI_eff_long == OI_eff_short for each asset
0 <= loss_weight_sum_side <= SOCIAL_LOSS_DEN
social_loss_remainder_side_num < SOCIAL_LOSS_DEN
social_loss_dust_side_num < SOCIAL_LOSS_DEN
```

K/F headroom:

```text
abs(K_side) + A_side * MAX_ORACLE_PRICE <= i128::MAX
abs(F_side_num) + A_side * MAX_ORACLE_PRICE * cfg_max_abs_funding_e9_per_slot * cfg_max_accrual_dt_slots <= i128::MAX
```

B epoch rules:

```text
Normal/DrainOnly side:
    B_side_num is current-epoch B and loss_weight_sum_side is the sum of current-epoch nonzero-basis weights.

ResetPending side:
    B_epoch_start_side_num is the B target for stale prior-epoch accounts.
    Current B_side_num/loss_weight_sum_side are new-epoch state and may be zero.
```

Explicit/unallocated loss buckets are non-redeemable audit/reconciliation state. They may trigger h-max while live, but are not user liabilities and MUST NOT block terminal market close after all accounts are closed.

-------------------------------------------------------------------------------
4. Claims, equity, and haircuts
-------------------------------------------------------------------------------

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

Eq_no_positive_credit_i = C_i + min(PNL_i,0) - FeeDebt_i
```

Risk-increasing trade approval MUST remove the candidate trade's own positive slippage from that same account.

`hmax_effective_active(ctx)` is true if threshold stress, bankruptcy h-lock, instruction-local bankruptcy candidate, loss-stale catchup, stale certificate, active bankrupt close, unresolved B loss, partial account-B settlement, or positive-PnL-using endpoint under B-stale is active. While true, fresh positive PnL uses `admit_h_max`; reserve release, reserve acceleration, conversion, auto-conversion, and positive-credit approvals are paused or recomputed under no-positive-credit lanes.

Same-instruction h-max activation is retroactive: earlier positive-PnL usability in the same atomic instruction MUST be staged, recomputed, reversed, or the instruction fails before commit.

-------------------------------------------------------------------------------
5. Portfolio health and cross-margin
-------------------------------------------------------------------------------

A full portfolio refresh MUST compute exact conservative health from all active legs.

```text
gross_mm = sum(asset_mm_leg)
gross_im = sum(asset_im_leg)

maintenance_req =
    gross_mm
    - hedge_credit
    + stale_penalty
    + concentration_penalty
    + unsettled_loss_penalty
```

Hedge credit is optional and conservative:

```text
hedge_credit <= min(offset_leg_risks) * cfg_max_offset_bps / 10_000
```

Hedge credit is allowed only for configured buckets:
- same underlying or explicitly configured oracle family,
- current oracle/funding/risk epochs,
- no unsettled B loss on either leg,
- no stale certificate,
- no recovery mode,
- no active close barrier.

No arbitrary covariance matrix is allowed unless it is represented by bounded deterministic buckets and exact conservative caps. Cross-margin MUST never reduce maintenance below the envelope proven in §1.3.

The account is:
- initial-healthy if `certified_equity >= certified_initial_req`;
- maintenance-healthy if `certified_equity >= certified_maintenance_req`;
- liquidatable if `certified_liq_deficit > 0` after full refresh.

-------------------------------------------------------------------------------
6. Stale certificate decay
-------------------------------------------------------------------------------

A certificate is fresh only if its epochs and price vector are valid under the configured envelope.

When stale, compute a conservative penalty:

```text
stale_loss_bound =
    sum_abs_notional_at_cert * max_price_move_since_cert
    + max_funding_move_since_cert
    + fee_bound
    + configured_oracle_uncertainty_bound
```

Then, in exact signed/wide arithmetic:

```text
current_certified_equity =
    old_certified_equity - stale_loss_bound

current_certified_maintenance =
    old_certified_maintenance + stale_risk_penalty
```

If either computation cannot be represented exactly, the certificate becomes unusable for favorable operations and liquidation/recovery must use a full refresh or fail closed. Implementations MUST NOT wrap, saturate toward the account, or treat overflow as healthy.

Stale accounts MUST NOT:
- withdraw,
- close favorably,
- convert/release PnL,
- increase risk,
- use hedge credit,
- use positive PnL for approval,
- receive resolved payout.

A stale account MAY be refreshed, rebalanced defensively, liquidated, or moved toward recovery through bounded permissionless actions. Any value-transferring liquidation based only on stale data requires a certificate whose deficit is a strict conservative upper bound on the refreshed liquidation deficit; otherwise liquidation MUST first refresh the full account.

-------------------------------------------------------------------------------
7. Canonical helpers
-------------------------------------------------------------------------------

Every `C_i`, `PNL_i`, position, B, and fee mutation MUST use aggregate-updating helpers or equivalent proofs.

### 7.1 Attach and clear leg

`attach_leg(account, asset, side, new_eff)` requires old side effects settled, side mode permits current-epoch attach, full account refresh context, and no active close barrier. It quarantines side remainder, then writes side snapshots and weight.

`clear_leg(account, asset)` requires A/K/F/B settled to the required target. It quarantines side remainder, transfers local `b_rem` to scaled dust, subtracts current-epoch weight, clears local fields, and MUST NOT mutate OI unless called by the OI-changing transition that proves the matching OI change.

### 7.2 Side remainder quarantine

Before changing `loss_weight_sum_side`:

```text
if social_loss_remainder_side_num != 0:
    transfer it to social_loss_dust_side_num
    social_loss_remainder_side_num = 0
```

While an active bankrupt close has staged residual against a side, weight-set changes on that side are forbidden.

### 7.3 Combined side-effect settlement

Authoritative touch prepares A/K/F and B together before principal loss settlement. The engine MUST NOT drain capital for B before same-touch K/F gains/losses are included.

For a nonzero leg, B target is current `B_side_num` if in the current epoch, else `B_epoch_start_side_num` under `ResetPending`.

A full settlement computes:

```text
ΔB = B_target - b_snap
num = b_rem + loss_weight * ΔB
B_loss = floor(num / SOCIAL_LOSS_DEN)
b_rem_new = num % SOCIAL_LOSS_DEN
KF_pnl_delta = exact signed-floor A/K/F settlement
net_pnl_delta = KF_pnl_delta - B_loss
```

If full B settlement is too large or not representable, partial B settlement under §7.4 is allowed. A user-favorable endpoint MUST stop after partial B settlement and return progress-required.

### 7.4 Account-local B settlement chunk

Let:

```text
B_remaining = B_target - b_snap
w = loss_weight
r = b_rem
L = per_touch_B_loss_limit
```

```text
max_num = (L + 1) * SOCIAL_LOSS_DEN - 1
if r > max_num: max_delta_by_loss = 0
else:           max_delta_by_loss = floor((max_num - r) / w)

delta_B_settle = min(B_remaining, max_delta_by_loss, endpoint_or_engine_delta_budget)
```

A successful chunk requires `w > 0`, `delta_B_settle > 0`, and `b_snap + delta_B_settle <= B_target`. The caller MUST NOT choose a smaller chunk than the engine-determined maximum for the supplied endpoint budget. It writes:

```text
num = r + w * delta_B_settle
B_loss_chunk = floor(num / SOCIAL_LOSS_DEN)
b_rem = num % SOCIAL_LOSS_DEN
b_snap += delta_B_settle
```

If `B_remaining > 0`, the account remains B-stale and no user-favorable action may continue.

If `B_remaining > 0` and no positive chunk is representable under the configured per-touch limit and exact arithmetic bounds, the account must be routed to permissionless recovery rather than looping forever.

### 7.5 PnL and fees

Every `PNL_i` mutation uses `set_pnl`. Positive increases in live mode go through admission. Negative cleanup with `NoPositiveIncreaseAllowed` is allowed only if it does not create a positive junior claim.

`settle_negative_pnl_from_principal` pays negative PnL from `C_i`. If live `PNL_i < 0` remains after `C_i` reaches zero, bankruptcy h-max MUST start/restart before commit.

Fees are charged only after senior losses are settled. For a bankrupt account, unpaid protocol/liquidation fees are dropped or forgiven and MUST NOT be included in bankruptcy loss, paid from insurance, or booked to B.

-------------------------------------------------------------------------------
8. A/K/F/B mechanics
-------------------------------------------------------------------------------

### 8.1 Accrual

`accrue_asset_to(asset, now_slot, effective_price, funding_rate)` requires live mode, no active bankrupt close, authenticated time, valid price, and bounded funding rate.

```text
dt = now_slot - slot_last
funding_active = dt > 0 && funding_rate != 0 && OI_eff_long != 0 && OI_eff_short != 0 && fund_px_last > 0
price_move_active = effective_price != previous_effective_price && (OI_eff_long != 0 || OI_eff_short != 0)
```

If active, require `dt <= cfg_max_accrual_dt_slots`. If price moves:

```text
abs(effective_price - previous_effective_price) * 10_000
    <= cfg_max_price_move_bps_per_slot * dt * previous_effective_price
```

K/F/stress candidates are computed in exact wide arithmetic and validated before any persistent write. If validation fails, no K/F/stress/price/slot field is written.

### 8.2 B residual booking

`book_bankruptcy_residual_chunk(asset, side, residual_remaining)` is O(1). It requires eligible opposing weight or records explicit non-claim loss.

Let `H = u128::MAX - B_side_num`, `W = loss_weight_sum_side`, and `R = social_loss_remainder_side_num`.

```text
max_scaled = (H + 1) * W - 1
if R > max_scaled: max_chunk_by_B = 0
else:              max_chunk_by_B = floor((max_scaled - R) / SOCIAL_LOSS_DEN)

engine_chunk =
    min(residual_remaining, max_chunk_by_B, cfg_public_b_chunk_atoms)

delta_B = floor((engine_chunk * SOCIAL_LOSS_DEN + R) / W)
new_remainder = (engine_chunk * SOCIAL_LOSS_DEN + R) % W
```

A successful booking requires `W > 0`, `engine_chunk > 0`, `delta_B > 0`, and `B_side_num + delta_B <= u128::MAX`.

A caller MUST NOT choose a smaller chunk than the engine-determined chunk. If `residual_remaining > 0` and no positive chunk is representable, enter permissionless recovery. A final chunk may be smaller than `cfg_public_b_chunk_atoms` only because `residual_remaining` or `max_chunk_by_B` is smaller.

### 8.3 Quantity ADL after residual durability

Bankruptcy residual is B-indexed; K is not used for bankruptcy residual. Quantity ADL is staged and applied exactly once after residual durability.

For a full close of `q_close_q`:

```text
OI_eff_liq_side -= q_close_q
OI_eff_opp_side -= q_close_q
```

If opposing OI remains, compute:

```text
A_candidate = floor(A_opp * OI_opp_after / OI_opp_before)
```

If `A_candidate == 0`, zero both sides and schedule reset. Otherwise set `A_opp = A_candidate`, update OI, and enter `DrainOnly` if `A_opp < MIN_A_SIDE`.

This step MUST NOT change local loss weights. Failure rolls back finalization.

### 8.4 Side reset

`begin_full_drain_reset(asset, side)` requires zero OI and no active close barrier. It snapshots K/F/B epoch-start state, quarantines remainder, resets current K/F/B and weights, increments epoch, sets `A_side = ADL_ONE`, and enters `ResetPending`. Stale accounts settle against epoch-start snapshots.

-------------------------------------------------------------------------------
9. User operations
-------------------------------------------------------------------------------

A user-favorable operation MUST:

1. authenticate owner/authority;
2. validate clock, oracle target, effective price, admission, and inputs;
3. if active close exists, continue it, recover, or fail before unrelated mutation;
4. refresh the full active portfolio account;
5. settle losses before fees;
6. recompute HealthCert;
7. run candidate checks under final h-max/stale/B state;
8. commit only if all global and account invariants hold.

Deposits are pure capital paths. Deposits into B-stale/stale accounts are loss-curing only and MUST NOT enable favorable actions before refresh clears locks.

Withdrawals use post-withdraw candidate state and no-positive-credit lanes when any lock/stale condition is active.

Trades require:
- full portfolio refresh for both counterparties or verified maker/liquidator account,
- loss-current market state,
- current B/K/F settlement for touched legs,
- side-mode gating,
- OI/position bounds,
- candidate-slippage neutralization,
- no-positive-credit approval while h-max/stress/stale/B locks are active,
- exact charged fee supplied to and enforced by engine.

Trades MUST NOT execute while bounded catchup remains incomplete unless they are purely risk-reducing and pass no-positive-credit conservative checks.

-------------------------------------------------------------------------------
10. Rebalance
-------------------------------------------------------------------------------

Rebalance may occur on user touch, crank touch, or liquidation.

Allowed:
- move support equity across active legs within the same PortfolioAccount;
- reduce risk by closing, shrinking, or collateral-shifting among legs;
- refresh certificates.

Forbidden:
- double count collateral;
- treat positive PnL as senior capital;
- use stale profitable legs for credit;
- use B-stale legs for hedge credit;
- erase fee debt or bankruptcy loss;
- improve one account by worsening another unless explicit liquidation transfer rules apply.

Conservation rule:

```text
senior_claim_after
    <= senior_claim_before
       + realized_nonjunior_pnl
       - fees
       - realized_losses
```

For an unhealthy account, accepted rebalance/liquidation requires a deterministic lexicographic score:

```text
risk_score =
    (
      certified_liq_deficit,
      unsettled_B_loss_bound,
      stale_loss_bound,
      gross_risk_notional,
      active_leg_count
    )
```

All components are computed after full refresh or from conservative certificate bounds. Accepted rebalance/liquidation requires:

```text
risk_score_after < risk_score_before
```

or:

```text
certified_liq_deficit_after < certified_liq_deficit_before
```

The scalar must include all active legs and conservative stale penalties, not just hinted legs. A transition that only moves deficit between legs, fees, insurance, B, or explicit loss without reducing this score is non-progress and MUST reject or route to recovery.

-------------------------------------------------------------------------------
11. Liquidation and bankrupt close
-------------------------------------------------------------------------------

Liquidation requires a full account refresh. If below maintenance:

1. try permitted defensive rebalance;
2. if still below, allow partial liquidation;
3. if a close creates post-principal deficit, use bankrupt close;
4. if ordinary progress cannot continue, enter permissionless recovery.

`begin_or_continue_bankrupt_close(account, asset, price, now_slot, budget)` is bounded. Minimum phases:

```text
Touched
SideEffectsPartiallySettled
ExposureStaged
DeficitComputed
InsuranceStaged
ResidualPartiallyBooked
ResidualBooked
AccountFinalized
```

Before `ResidualBooked`, the account's basis, weight, OI, PnL, and slot are not freed or cleared except under staged, durable, exactly-once semantics.

Residual loss:
- first consumes staged insurance exactly once;
- remaining residual is B-booked to the opposing side or recorded as explicit non-claim loss;
- quantity ADL applies exactly once only after residual durability;
- close-blocking fee debt is forgiven, not socialized.

A close cannot stay live forever through successful continuations. If configured side-effect or residual chunk limits are reached, the next bounded action MUST enter permissionless terminal recovery.

-------------------------------------------------------------------------------
12. Keeper cranks and hints
-------------------------------------------------------------------------------

Keeper cranks are bounded and incremental.

Inputs:
- account hints,
- proposed rebalance/liquidation actions,
- oracle/funding proofs as required,
- optional recovery proof.

Rules:
- candidate padding/missing/duplicates count against inspection budget;
- missing global accounts do not cause rollback merely because more accounts exist;
- if a crank performs equity-active accrual on an exposed market, it MUST also commit protective progress;
- hints are never assumed complete;
- any hinted unhealthy account must be processable with bounded work or routed to recovery.

If `authenticated_now_slot - slot_last > cfg_max_accrual_dt_slots`, use subtraction-first bounded catchup segments. While catchup remains incomplete, the market is loss-stale:
- positive PnL uses h-max/no-positive-credit lanes;
- reserves do not release;
- conversion/auto-conversion is disabled;
- risk-increasing trades, nonflat withdrawals, and OI-increasing actions are blocked;
- keeper touch/revalidation and risk-reducing actions may continue.

-------------------------------------------------------------------------------
13. Resolution and recovery
-------------------------------------------------------------------------------

A public CrankForward market MUST expose permissionless terminal recovery for any state where ordinary bounded progress cannot continue, including:

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

The caller cannot choose the recovery price. Recovery uses deterministic authenticated recovery price when available and representable; otherwise only immutable configured fallback may use `P_last`.

If recovery occurs while active close exists, recovery MUST complete or durably settle the active close before clearing the close state. It MUST NOT drop residuals, double-spend insurance, clear PnL without durable loss state, or leave booked B loss to be charged again.

Resolved close is permissionless and bounded. It refreshes one PortfolioAccount, settles terminal K/F/B, clears reserve metadata, settles negative PnL from principal then insurance/unallocated audit loss, syncs fees to `resolved_slot`, and only then may pay, forgive fee debt, or free.

Positive payout readiness is tracked by aggregates and certificates, not by scanning all accounts in one instruction. Readiness requires:

```text
active_bankrupt_close absent
for every asset side: stored_pos_count == 0 and stale_account_count == 0
b_stale_account_count == 0
stale_certificate_count == 0
negative_pnl_account_count == 0
all resolved fee sync obligations for paid/freeing accounts are current
```

Payout snapshot is captured once after readiness and remains stable. `materialized_portfolio_count` MAY remain nonzero while winners are being paid one at a time.

-------------------------------------------------------------------------------
14. Wrapper obligations
-------------------------------------------------------------------------------

Wrappers own:
- authorization,
- oracle normalization,
- raw target storage,
- effective-price staircase policy,
- account proof packing,
- anti-spam economics,
- hint markets or off-chain discovery.

Public wrappers MUST NOT expose caller-controlled:
- admission,
- funding,
- threshold,
- future slot,
- B residual chunk size,
- account-B settlement chunk size,
- favorable stale-certificate interpretation.

Public user-fund wrappers MUST expose:
- full account refresh,
- hinted crank,
- bounded catchup,
- active-close continuation,
- account-B settlement continuation,
- permissionless recovery,
- rebalance-on-touch.

Target/effective lag MUST not give users a free option. Extraction-sensitive actions reject or shadow-check; risk-increasing trades use dual-price/no-positive-credit checks.

No wrapper may treat a global accumulator as proof that a specific account is healthy. Global aggregates may trigger stress mode, fees, stale penalties, or recovery, but account health is proven only by full account refresh or conservative certificate.

-------------------------------------------------------------------------------
15. Required proof and TDD coverage
-------------------------------------------------------------------------------

1. `unbounded_global_accounts_no_full_market_scan_required`.
2. `full_account_refresh_is_O_N_and_required_for_favorable_actions`.
3. `hinted_subset_cannot_hide_toxic_leg`.
4. `stale_certificate_loses_margin_credit`.
5. `stale_profitable_leg_cannot_support_risk_increase`.
6. `rebalance_conserves_senior_claims`.
7. `rebalance_cannot_double_count_collateral`.
8. `cross_margin_offset_cap_never_below_loss_envelope`.
9. `unhealthy_rebalance_requires_strict_risk_progress`.
10. `cyclic_rescue_without_progress_reverts`.
11. `B_stale_blocks_withdraw_convert_close_and_risk_increase`.
12. `account_B_settlement_chunks_huge_delta_without_market_scan`.
13. `B_booking_exact_remainder_conservation`.
14. `bankrupt_close_books_residual_without_opposing_scan`.
15. `bankrupt_close_cannot_clear_basis_before_residual_durable`.
16. `staged_insurance_not_double_spent`.
17. `bankruptcy_residual_excludes_protocol_fees`.
18. `uncollectible_fees_forgiven_not_socialized`.
19. `account_free_equity_active_accrual_requires_protective_progress`.
20. `effective_price_raw_target_lag_no_free_option`.
21. `loss_stale_catchup_blocks_risk_increase_until_current`.
22. `resolved_close_one_account_bounded`.
23. `permissionless_recovery_no_caller_chosen_price`.
24. `explicit_loss_audit_overflow_does_not_trap_funds`.
25. `authoritatively_flat_account_never_receives_B_loss`.
26. `no_single_instruction_full_market_requirement`.
27. `worst_case_hinted_progress_totality`.
28. `global_accumulator_not_account_health_proof`.
29. `active_bitmap_canonical_no_hidden_legs`.
30. `N_too_large_rejected_at_public_user_fund_init`.

-------------------------------------------------------------------------------
16. Audit summary: major issues fixed
-------------------------------------------------------------------------------

[FIXED] Full-market liveness dependency
    Removed global account slab; bounded progress is per hinted account.

[FIXED] Hidden-position hint attack
    Hints are discovery only; full active bitmap and stale certificate govern health.

[FIXED] Optimistic stale health
    Stale certificates fail closed and block favorable actions.

[FIXED] Collateral double counting in cross-margin
    Rebalance has conservation and no-double-use rules.

[FIXED] Cross-margin offset over-credit
    Hedge credit is bucketed, capped, current-only, and envelope-bounded.

[FIXED] Cyclic rescue / liquidation oscillation
    Unhealthy rebalances require strict scalar progress.

[FIXED] Single-account huge B delta
    Account-local B settlement is chunked and blocks favorable actions until current.

[FIXED] Bankruptcy residual loss escape
    Residual must be durably B-booked or explicit before clearing PnL/basis/OI.

[FIXED] Fee socialization bug
    Uncollectible fees are dropped/forgiven, not paid by insurance or B.

[FIXED] Market stuck by non-progressing active close
    Chunk limits route to permissionless recovery.

[FIXED] Global accumulator false safety
    Aggregates cannot prove individual account health.

-------------------------------------------------------------------------------
17. Honest remaining tradeoff
-------------------------------------------------------------------------------

v12.20.6 guarantee:
    one honest crank plus bounded cursor traversal can eventually inspect all materialized accounts.

v13.0.0 guarantee:
    one honest crank with a valid account hint can force bounded progress on that account,
    and stale or omitted accounts cannot use optimistic health to extract funds or increase risk.

This is the intended tradeoff enabling:
- unlimited global account count,
- lazy evaluation,
- cross-margin accounts,
- no full-market atomic evaluation,
- permissionless hinted recovery.
