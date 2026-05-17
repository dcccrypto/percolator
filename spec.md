# Risk Engine Spec (Source of Truth) — v15.10.0 Full Shared Cross-Liquidity

**Design:** protected principal + junior profit claims + full shared cross-margin inside one market-group instance + mutable asset lifecycle + exact scaled decomposed junior-claim bounds + impairment-monotone positive credit + unified positive payout accounting + progressive resolved payout + aggregate pending-obligation drift netting + instance-level isolation + leg-attributed bankruptcy loss + domain-budgeted insurance + preemptible close ownership + durable close progress ledger + split pending-loss barriers + immutable close-drift anchor + market-side B domains.  
**Scope:** one Percolator market group for one quote-token vault, with up to `N` configured asset slots per `PortfolioAccount` and unbounded global account count. A UI MAY aggregate multiple market-group instances, but each instance is an independent vault, solvency, insurance, B, PnL, haircut, asset set, payout, and recovery domain.  
**Status:** normative source-of-truth draft. Terms **MUST**, **MUST NOT**, **SHOULD**, and **MAY** are normative.

This revision supersedes v15.9.0.

```text
Inside one trusted market-group instance:
    Hyperliquid-like full cross-margin UX;
    every Active asset is full Tier-4 / support weight 1.0.

Between market-group instances:
    no protocol netting;
    no shared health;
    no shared insurance;
    no shared B;
    no shared haircut;
    no shared bankruptcy socialization.
```

The safety boundary is the market-group instance. Riskier assets SHOULD be deployed in separate instances. A UI may merge display, but contracts MUST NOT merge solvency. Within one instance, current eligible PnL from any leg may support another leg. If an account bankrupts, residuals remain attributed to the asset-side domain that generated the loss.

Every top-level instruction is atomic. Any failed precondition, checked arithmetic guard, missing authenticated proof, context-capacity overflow, non-progressing lock, stale close snapshot, unreconciled pending loss, invalid asset-lifecycle transition, cross-instance netting attempt, invalid junior-bound bucket, invalid junior-bound scaled decrement, invalid resolved-payout receipt, pending-obligation mismatch, payout-lane conflict, or conservative-failure condition MUST roll back every mutation performed by that instruction. Before commit, every successful instruction MUST leave all global, asset, account, certificate, close-state, insurance, payout, obligation, junior-bound, and attribution invariants true.

-------------------------------------------------------------------------------
0. Non-negotiable requirements
-------------------------------------------------------------------------------

1. **Full shared instance solvency:** every Active asset inside one market-group instance has support weight `1.0`. Current conservative PnL from any eligible leg may support maintenance and risk approval for any other leg in the same account.
2. **Instance boundary is absolute:** no account health, PnL, insurance, B loss, recovery, haircut, collateral, or bankruptcy state may cross instances. UI aggregation is display only.
3. **Mutable asset lifecycle is fail-closed:** assets MAY be activated, drained, retired, or recovered, but activation requires full envelope proofs, bounded rate limits, support weight `1.0`, and certificate fail-closed handling.
4. **No global B pool:** bankruptcy residual is charged only to the asset-side domain whose exposure generated it.
5. **Protected principal is senior:** junior positive PnL MUST NOT outrank capital, insurance, or durable loss recognition.
6. **Junior impairment is fail-closed and monotone:** when `Residual < PNL_pos_bound_tot`, every positive-PnL use, including maintenance credit, withdrawal, trade approval, support, and resolved payout, is bounded by `g` and MUST NOT exceed the non-impaired leg-local credit.
7. **Junior claim bound has exact scaled arithmetic:** `PNL_pos_bound_tot` is computed from exact positive claims plus bounded, replaceable bucket terms in one shared `BOUND_SCALE` numerator domain. Aggregate bucket bounds and per-account receipt decrements MUST use the same scaled units, so rounding slack cannot be over-subtracted.
8. **One positive payout lane at a time:** ordinary live positive-PnL withdrawal/release is disabled once the market group leaves `Live` or a `ResolvedPayoutLedger` is initialized. Resolved/recovery positive payouts must go only through the unified resolved payout ledger.
9. **Resolved payout is order-invariant and fail-closed:** resolved accounts receive safe progressive payouts from conservative terminal claim bounds and later top-ups as receipts tighten bounds. A receipt whose exact claim exceeds its prior bound contribution MUST halt/recover before any payout uses it.
10. **No fake-profit extraction:** stale, lagged, B-stale, loss-stale, locked, partially refreshed, recovery-mode, target/effective-divergent, or pending-loss-exposed profitable legs provide zero or conservative credit.
11. **Loss-curing support must be durable:** open positive PnL may support maintenance, but it MUST NOT cure a residual unless the supporting claim is closed/finalized, its face junior claim is burned/locked, and matching source-domain loss exposure is recognized or reserved.
12. **Effective support burns face junior claims:** consuming haircut-valued support MUST burn or lock the corresponding face junior claim and update account/global junior aggregates in the same atomic step.
13. **No double-socialized close progress:** durable B booking, quantity ADL, insurance spend, support consumption, explicit loss assignment, pending-obligation credit, or drift consumption MUST durably advance close-local progress accounting in the same atomic step.
14. **No orphaned durable chunks:** durable B/ADL/insurance/support/obligation progress remains attributed to `(account, close_id, asset, domain)` or `(barrier_id, account_id)` until finalization or recovery reconciliation.
15. **Pending-loss barriers are split by purpose:** withdrawal/positive-credit gates use full-lifetime worst-case pending reserves; liquidation health uses only booked loss plus bounded near-term pending loss.
16. **Pending obligations are netted from the source residual exactly once:** when a participant exits a pending barrier, its backed/settled/pulled-forward share, including its frozen fractional share of future close drift, MUST reduce the originating close residual and MUST NOT also be socialized over remaining weight.
17. **Pending-obligation drift accounting is aggregate and bounded:** origin residual B-booking MUST apply due exited-participant drift credits in O(1) using aggregate barrier state. Individual obligation rebate/settlement is chunked and MUST NOT be a precondition to B-booking.
18. **Pending obligations survive participant finalization:** a participant reducing/clearing weight or finalizing while exposed to a pending residual MUST escrow, settle, or pull forward its pending obligation before weight is removed. A mere snapshot is not sufficient.
19. **No pending residual escape:** domain participants MUST NOT withdraw unrelated profits or reduce loss weight to escape before the pending loss is B-booked, backed, recovered, or escrowed.
20. **Risk-reducing exits remain possible:** pending-domain-loss barriers MUST allow risk-reducing position changes if the account preserves, escrows, settles, or pulls forward its pending loss-weight obligation.
21. **Immutable close lifecycle:** `close_id`, `gross_loss_at_close_start`, `drift_reference_slot`, and `max_close_slot` persist across preemption, unwind, restart, and recovery until finalized or safely canceled.
22. **Atomic cure-and-cancel:** an in-progress bankrupt close MUST be cancelable by an atomic owner deposit/refresh/cancel path if the account is healthy and no irreversible progress has occurred. Ordinary continuation MUST check this path before consuming new deposit capital.
23. **No close deadlock:** close ownership is deterministic and preemptible. A lower-priority close unwinds reversible staged state and releases conflicting domains before a higher-priority close proceeds. Hold-and-wait cycles are forbidden.
24. **Minimal current-step locking:** a close may reserve only domains currently required for the mutations it will perform. Speculative future domains MUST NOT be locked for the whole close lifetime.
25. **Accrual is non-exclusive but close-bounded:** domain locks MUST NOT freeze authenticated asset-wide price/funding accrual. Close snapshots MUST be recomputed or conservatively re-aged before use.
26. **Close drift is bounded:** post-start adverse price/funding/K/F drift is measured from immutable `drift_reference_slot`, covered by close-drift reserve, and bounded by immutable `max_close_slot`.
27. **Residual durability before clearing exposure:** basis, OI, PnL, and side weights for a bankrupt close MUST NOT be freed until residuals are durably booked or fully backed.
28. **No ADL/finalization split:** quantity ADL, closing-account exposure clear, and close-progress ledger advancement MUST be atomic or protected by a non-preemptible finalization barrier.
29. **No fee seniority:** uncollectible protocol/liquidation fees are dropped or forgiven, never paid from insurance or socialized through B.
30. **No caller-chosen loss domain:** liquidation order, support allocation, insurance allocation, and residual attribution MUST be deterministic.
31. **No arbitrary correlation trust:** hedge credit is allowed only under deterministic buckets and exact conservative envelopes.
32. **No double reset:** a side in `ResetPending` MUST NOT reset again until all prior-epoch stale accounts are settled, migrated, or recovered.
33. **Dead-leg exit:** public markets MUST expose a bounded owner-callable dead-leg forfeit/detach path for terminal/recovery assets.
34. **Hints are discovery only:** omitted or stale positions MUST NOT improve health.
35. **Full account refresh is bounded by `N`:** every user-favorable operation MUST refresh the full active portfolio first.
36. **No full-market atomic work:** public instructions MUST NOT scan all accounts or all opposing accounts.
37. **Crank-forward public markets:** any state that only a privileged actor can advance is non-compliant for public user-fund markets.
38. **Canonical per-asset leg:** each account has at most one canonical signed net leg per configured asset.
39. **Verified maker exemption is bounded:** maker/liquidator refresh exemption is allowed only with an engine-verified post-trade health certificate covering the candidate trade.

-------------------------------------------------------------------------------
1. Units, bounds, and configuration
-------------------------------------------------------------------------------

Persistent economic quantities use `u128` or `i128`. Persistent signed fields MUST NOT equal `i128::MIN`. Transient products involving price, position, A/K/F/B, weights, fees, haircuts, penalties, support allocation, residual attribution, insurance, re-aging, junior-bound deltas, pending obligations, resolved payout rates, and remainders MUST use an exact domain at least 256 bits wide. All divisions MUST round against the account. Checked arithmetic failure MUST revert.

```text
POS_SCALE                    = 1_000_000
ADL_ONE                      = 1_000_000_000_000_000
FUNDING_DEN                  = 1_000_000_000
SOCIAL_WEIGHT_SCALE          = ADL_ONE
SOCIAL_LOSS_DEN              = 1_000_000_000_000_000_000_000
STRESS_CONSUMPTION_SCALE     = 1_000_000_000
MAX_BPS                      = 10_000
SUPPORT_WEIGHT_SCALE         = 1_000_000
FULL_SUPPORT_WEIGHT          = SUPPORT_WEIGHT_SCALE
BOUND_SCALE                  = 1_000_000_000_000
```

Every live, resolved, raw target, effective engine, recovery, and fallback price MUST satisfy:

```text
0 < price <= MAX_ORACLE_PRICE
```

```text
RiskNotional(asset, account) =
    0 if effective_pos_q == 0
    else ceil(abs(effective_pos_q) * conservative_effective_price / POS_SCALE)

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
MAX_PORTFOLIO_ASSETS_N                = implementation/config bounded
MAX_PROTOCOL_FEE_ABS                  = 1_000_000_000_000_000_000_000_000_000_000_000_000
GLOBAL_MAX_ABS_FUNDING_E9_PER_SLOT   = 10_000
MAX_WARMUP_SLOTS                      = u64::MAX
MAX_RESOLVE_PRICE_DEVIATION_BPS       = 10_000
MIN_A_SIDE                            = 100_000_000_000_000
MAX_JUNIOR_BOUND_BUCKETS_PER_SIDE     = implementation/config bounded
```

`N` and the bound-bucket count MUST be small enough that full account refresh, health computation, liquidation validation, asset activation checks, junior-bound recomputation, resolved receipt creation, close-vector re-aging, aggregate obligation-credit application, residual attribution, resolved close, and proof packing fit within runtime limits.

### 1.2 Public-market configuration

Initialization and every asset activation MUST validate:

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
0 < oracle_price(asset) <= MAX_ORACLE_PRICE
0 < cfg_max_portfolio_assets <= MAX_PORTFOLIO_ASSETS_N
0 < cfg_junior_bound_bucket_count <= MAX_JUNIOR_BOUND_BUCKETS_PER_SIDE
for every asset side: cfg_max_active_weight_per_side <= SOCIAL_LOSS_DEN
```

Public user-fund markets MUST satisfy:

```text
cfg_margin_mode == FullSharedCrossLiquidity
cfg_asset_support_weight(asset) == FULL_SUPPORT_WEIGHT for every Active asset
cfg_asset_set_lifecycle == MutableWithActivationProofs
cfg_bankruptcy_mode == LegAttributedMarketSideB
cfg_positive_support_mode == GlobalHaircutBoundedWhenImpaired
cfg_positive_payout_mode == UnifiedLiveOrResolvedLane
cfg_junior_bound_mode == ExactBucketFormulaDecomposedReplaceable
cfg_resolved_payout_mode == ProgressiveBoundedReceiptsWithTopups
cfg_resolved_receipt_underbound_action in {HaltPayoutsAndRecover, RevertBeforePayout}
cfg_pending_loss_mode == SplitReservesAndEscrowedObligations
cfg_pending_obligation_mode == AggregateSourceResidualAndDriftNetting
cfg_insurance_mode in {DomainBudgeted, GlobalProtocolFirstLossWithCaps}
cfg_instance_isolation == true
cfg_public_liveness_profile == CrankForward
cfg_permissionless_recovery_enabled == true
cfg_recovery_fallback_price_enabled == true
cfg_owner_dead_leg_forfeit_enabled == true
cfg_full_refresh_required_for_favorable_actions == true
cfg_stale_certificate_penalty_enabled == true
cfg_deterministic_portfolio_liquidation_enabled == true
cfg_close_state_scope == AccountLocalWithPreemptibleDomainLocks
cfg_close_conflict_policy == DeterministicPreemptivePriority
cfg_no_global_B_index == true
cfg_no_cross_instance_socialization == true
cfg_asset_activation_cooldown_slots >= cfg_min_public_refresh_grace_slots
cfg_public_b_chunk_atoms > 0
cfg_max_account_b_settlement_chunks > 0
cfg_max_bankrupt_close_chunks > 0
cfg_max_bankrupt_close_lifetime_slots > 0
cfg_pending_loss_maintenance_horizon_slots <= cfg_max_accrual_dt_slots
cfg_pending_obligation_settlement_chunks > 0
cfg_close_drift_reserve_enabled == true
cfg_close_drift_anchor_mode == ImmutableReferenceSlot
cfg_close_progress_after_drift_positive == true
```

Assets that should not fully share account solvency SHOULD be deployed in a separate instance. Wrappers MUST NOT use UI aggregation as health, collateral, margin, transfer, or payout proof.

If `cfg_insurance_mode == GlobalProtocolFirstLossWithCaps`, then:

```text
permitted_global_protocol_first_loss_for_domain =
    min(domain_global_cap - domain_global_spent,
        global_protocol_budget - global_protocol_spent)
```

If exhausted, residual routes to the domain's B or recovery.

### 1.3 Solvency, offsets, junior bounds, and close-progress envelopes

For each Active or activating asset, prove in exact wide arithmetic:

```text
ADL_ONE * MAX_ORACLE_PRICE * cfg_max_abs_funding_e9_per_slot * cfg_max_accrual_dt_slots <= i128::MAX
cfg_min_funding_lifetime_slots >= cfg_max_accrual_dt_slots
ADL_ONE * MAX_ORACLE_PRICE * cfg_max_abs_funding_e9_per_slot * cfg_min_funding_lifetime_slots <= i128::MAX
```

For every integer `1 <= X <= MAX_ACCOUNT_NOTIONAL_PER_ASSET`, prove:

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

Cross-margin offsets MUST be proven under deterministic buckets:

```text
SameUnderlyingExact:
    same canonical price source or deterministic 1:1 conversion with no independent depeg risk.

ExplicitFamilyWithGap:
    distinct configured assets; proof assumes simultaneous adverse movement
    plus configured basis/depeg gap against the account.
```

No production config may rely on empirical correlation unless reduced to deterministic adverse-gap caps.

Close-progress envelope MUST quantify over every allowed portfolio and close domain set, not merely per asset:

```text
max_close_drift_loss =
    worst adverse price/funding/K/F/stale/thin-market movement
    over cfg_max_bankrupt_close_lifetime_slots
    for any allowed portfolio and any close domain set

min_close_progress_per_continuation =
    minimum residual/insurance/B/recovery progress guaranteed by a valid crank
    net of chunking, representability, and domain budget constraints

require min_close_progress_per_continuation > max_adverse_drift_per_continuation
require cfg_max_bankrupt_close_chunks * min_close_progress_per_continuation
        covers max portfolio close residual plus max_close_drift_loss
```

-------------------------------------------------------------------------------
2. Exact scaled junior-bound bucket arithmetic
-------------------------------------------------------------------------------

`PNL_pos_bound_tot` is the source of truth for the junior solvency haircut `g`. Its construction MUST be formulaic, additive, and auditable. All live and resolved junior bounds are stored in the same scaled numerator domain.

```text
amount_from_bound_num(x_num) = ceil(x_num / BOUND_SCALE)

PNL_pos_bound_tot_num =
    PNL_pos_exact_tot * BOUND_SCALE
  + account_base_bound_sum_num
  + sum(bucket.current_upper_bound_num for all JuniorClaimBoundBucket)
  + unresolved_recovery_bound_num

PNL_pos_bound_tot = amount_from_bound_num(PNL_pos_bound_tot_num)
```

No resolved-payout receipt may decrement a ceil-of-sum aggregate by a sum-of-ceils contribution. If an aggregate is initialized from a bucket, the aggregate and every per-account decrement MUST be expressed in `BOUND_SCALE` numerator units and MUST be additive partitions of the same stored bucket terms.

### 2.1 Account base bound

For every account not fully exact-current, the engine stores:

```text
account_base_bound_num_i = max(PNL_i_snapshot, 0) * BOUND_SCALE
```

`R_i` is ignored for this bound. This `R_i = 0` assumption is intentionally conservative because previous positive releases can only lower the remaining junior claim. Account refresh may replace this with exact refreshed PnL and remove the stale base bound.

### 2.2 Bucket membership

Each nonzero leg that is not exact-current contributes to exactly one bucket:

```text
BucketKey = (asset_id, side, bucket_id)
```

A bucket stores:

```text
JuniorClaimBoundBucket {
    side
    sum_abs_pos_q
    sum_funding_weight
    basis_lo
    basis_hi
    k_snap_range
    f_snap_range
    oracle_uncertainty_bound
    stale_slot_bound

    unit_current_profit_bound_num
    unit_terminal_profit_bound_num
    unit_current_funding_bound_num
    unit_terminal_funding_bound_num

    current_upper_bound_num
    terminal_claim_upper_bound_num
}
```

For every member leg, the leg's effective basis and K/F snapshots MUST lie inside the stored ranges. If an accrual, funding update, price update, attach, clear, or side reset would make any member potentially leave the range, the engine MUST, before any favorable action using the resulting state, either:
- recompute/split/rebucket the affected bounded bucket set;
- expand the bucket range and recompute safe scaled upper bounds;
- replace the bucket bound with the hard maximum possible scaled bound for `sum_abs_pos_q`; or
- fail closed and route to recovery if representability or runtime limits prevent a safe bound.

No bucket may be used with an out-of-range member.

### 2.3 Per-unit positive upper bound

For each bucket, define a side-specific best-case price and basis for bounding positive PnL.

```text
For a long bucket:
    P_best_current  = effective_price + oracle_uncertainty_bound + max_price_move_bound
    P_best_terminal = resolved_or_recovery_price + terminal_uncertainty_bound
    basis_best      = basis_lo

For a short bucket:
    P_best_current  = max(1, effective_price - oracle_uncertainty_bound - max_price_move_bound)
    P_best_terminal = max(1, resolved_or_recovery_price - terminal_uncertainty_bound)
    basis_best      = basis_hi
```

Funding and K/F use the most favorable value inside the bucket ranges and configured funding envelope:

```text
favorable_kf_current =
    exact_wide_upper_AKF_delta(side, basis_best, P_best_current,
                               k_snap_range, f_snap_range,
                               cfg_max_abs_funding_e9_per_slot,
                               stale_slot_bound)

favorable_kf_terminal =
    exact_wide_upper_AKF_delta(side, basis_best, P_best_terminal,
                               k_snap_range, f_snap_range,
                               terminal_funding_bound,
                               terminal_slot_bound)
```

`exact_wide_upper_AKF_delta` MUST round upward for possible positive PnL and MUST never subtract B loss for an upper bound. B loss is nonnegative and excluding it is conservative.

```text
unit_current_profit_bound_num =
    ceil((max(0, favorable_kf_current) + per_unit_thin_oracle_uncertainty)
         * BOUND_SCALE / POS_SCALE)

unit_terminal_profit_bound_num =
    ceil((max(0, favorable_kf_terminal) + per_unit_terminal_uncertainty)
         * BOUND_SCALE / POS_SCALE)

unit_current_funding_bound_num =
    ceil(favorable_funding_unit_bound * BOUND_SCALE / FUNDING_DEN)

unit_terminal_funding_bound_num =
    ceil(terminal_funding_unit_bound * BOUND_SCALE / FUNDING_DEN)
```

The per-unit scaled values are intentionally rounded upward once, then multiplied by position or funding weight. This makes bucket aggregates and account-level contributions additive in the same numerator units.

### 2.4 Bucket upper bounds

```text
bucket.current_upper_bound_num =
    sum_abs_pos_q * unit_current_profit_bound_num
  + sum_funding_weight * unit_current_funding_bound_num
  + stale_uncertainty_bound * BOUND_SCALE

bucket.terminal_claim_upper_bound_num =
    sum_abs_pos_q * unit_terminal_profit_bound_num
  + sum_funding_weight * unit_terminal_funding_bound_num
  + terminal_stale_or_recovery_uncertainty_bound * BOUND_SCALE
```

The amount represented by a bucket is:

```text
bucket.current_upper_bound_amount =
    amount_from_bound_num(bucket.current_upper_bound_num)

bucket.terminal_claim_upper_bound_amount =
    amount_from_bound_num(bucket.terminal_claim_upper_bound_num)
```

The bucket bound is a sum of per-leg nonnegative scaled upper contributions. Therefore:

```text
sum_i max(account_potential_PNL_i, 0) * BOUND_SCALE
    <= account_base_bound_sum_num + sum(bucket.current_upper_bound_num)
```

because `max(x_0 + x_1 + ... + x_n, 0) <= max(x_0,0) + sum(max(x_j,0))` and every per-unit term is rounded upward before multiplication.

### 2.5 Per-account prior bound contribution

Every account stores the bucket key and the parameters needed to compute its own scaled contribution at receipt or refresh:

```text
AccountJuniorBoundContribution {
    account_base_bound_num_at_bucket
    leg_terms[0..N] {
        bucket_key
        abs_pos_q
        funding_weight
        basis_snapshot
        k_snap
        f_snap
    }
}
```

For an account receipt or exact refresh:

```text
prior_bound_contribution_num_i =
    account_base_bound_num_at_bucket
  + sum(abs_pos_q_leg * bucket.unit_terminal_profit_bound_num
        + funding_weight_leg * bucket.unit_terminal_funding_bound_num
        + leg_uncertainty_share_num)
```

`prior_bound_contribution_num_i` is the exact scaled portion of `terminal_claim_bound_unreceipted_num` being replaced by that account's exact terminal claim. Receipt creation MUST require:

```text
terminal_positive_claim_face_i * BOUND_SCALE
    <= prior_bound_contribution_num_i
```

If this fails, the bound understated a claim. The receipt is rejected, payouts halt, and the market routes to recovery or bound repair.

When a receipt is accepted:

```text
terminal_claim_bound_unreceipted_num -= prior_bound_contribution_num_i
terminal_claim_exact_receipts_num    += terminal_positive_claim_face_i * BOUND_SCALE
```

Because both fields are in the same numerator domain, the decrement cannot exceed the stored contribution unless the ledger is corrupt. A receipt MUST NOT decrement `terminal_claim_bound_unreceipted_num` by an integer amount, a per-leg ceil sum in quote atoms, or any value not equal to the stored scaled contribution.

### 2.6 Bucket lowering and exact refresh

Account refresh atomically:
1. removes the account's previous base and leg contribution numerators from the relevant aggregates;
2. settles exact A/K/F/B and PnL;
3. adds exact positive PnL to `PNL_pos_exact_tot` or places any still-open nonzero leg into a current valid bucket;
4. recomputes `PNL_pos_bound_tot_num` and `PNL_pos_bound_tot`.

A bucket MAY decrease when price, funding, oracle uncertainty, or member ranges improve. Decrease is allowed only by formula recomputation in exact wide arithmetic. It MUST NOT depend on caller-selected values or optimistic correlation.
-------------------------------------------------------------------------------
3. Asset lifecycle
-------------------------------------------------------------------------------

Asset slots are bounded by `N` and have lifecycle:

```text
Disabled -> PendingActivation -> Active -> DrainOnly -> Retired
                                      \-> Recovery -> Retired
```

Activation requirements:
- slot is Disabled or Retired;
- no remaining OI, weights, B, K/F, pending barriers, pending obligations, close ledgers, stale accounts, or unresolved claims in that slot;
- oracle, price, funding, B-headroom, junior-bound, close-progress, and portfolio-envelope proofs pass for the whole instance;
- support weight is exactly `FULL_SUPPORT_WEIGHT`;
- activation respects `cfg_asset_activation_cooldown_slots`;
- activation increments `config_hash`, `risk_epoch`, and `asset_set_epoch`;
- certificates that touch or attempt to attach/trade the new asset fail closed until refresh;
- certificates for accounts with no exposure to the new asset may remain valid only if their active-bitmap proof and certificate schema explicitly exclude the new asset from credit and risk;
- accounts without the new asset treat its leg as canonical inactive on refresh.

Activation MUST NOT retroactively make any stale certificate favorable. A newly Active asset cannot be traded by an account until that account refreshes under the new asset set.

Drain/retire/recovery exit:
- `DrainOnly` blocks risk increase and new attaches;
- `Recovery -> Retired` requires all accounts with the asset closed, settled, forfeited/detached, or recovered;
- `Retired` requires zero OI, zero stored position count, no pending barriers, all close ledgers finalized/canceled, all pending obligations settled/rebated, all prior-epoch accounts settled/migrated/recovered;
- a side in `ResetPending` cannot reset again until all prior-epoch stale accounts are settled, migrated, or recovered.

-------------------------------------------------------------------------------
4. State
-------------------------------------------------------------------------------

### 4.1 MarketGroup

```text
MarketGroup {
    instance_id
    V, I, C_tot
    PNL_pos_exact_tot
    PNL_pos_bound_tot              // derived or cached from JuniorClaimBoundLedger
    PNL_matured_pos_tot            // legacy maturity counter, never used without g
    materialized_portfolio_count_unbounded_counter

    risk_epoch
    oracle_epoch
    funding_epoch
    asset_set_epoch
    current_slot

    assets[0..N)
    junior_claim_bound_ledger
    resolved_payout_ledger optional
    domain_locks[(asset, side)]
    insurance_ledger
    close_progress_ledger
    pending_domain_loss_barriers[(asset, side)]
    pending_obligation_aggregates[(barrier_id)]
    pending_obligation_ledger
    global_stale_penalty_params
    mode in {Live, Resolved, Recovery}
}
```

No state in another instance is part of this `MarketGroup`.

### 4.2 ResolvedPayoutLedger and unified payout lane

```text
ResolvedPayoutLedger {
    snapshot_residual
    terminal_claim_exact_receipts_num
    terminal_claim_bound_unreceipted_num
    current_payout_rate
    snapshot_slot
    snapshot_price_vector_hash
    payout_halted
    finalized

    account_receipts[account_id] {
        prior_bound_contribution_num
        live_released_face_at_receipt   // R_i at receipt creation
        terminal_positive_claim_face
        paid_effective
        receipt_finalized
    }
}
```

Before any positive resolved payout, capture `snapshot_residual` after terminal losses, insurance, pending barriers, pending obligations, and recovery states are durably settled or reserved. Then:

```text
terminal_claim_total_bound_num =
    terminal_claim_exact_receipts_num + terminal_claim_bound_unreceipted_num

if terminal_claim_total_bound_num == 0:
    current_payout_rate = (1,1)
else:
    current_payout_rate =
        (min(snapshot_residual * BOUND_SCALE, terminal_claim_total_bound_num),
         terminal_claim_total_bound_num)
```

`terminal_claim_bound_unreceipted_num` is conservative and MUST be monotone non-increasing. It is initialized from the sum of `account_base_bound_num` and bucket `terminal_claim_upper_bound_num` terms in §2. It MUST NOT be initialized from a quote-atom ceil-of-sum that is later decremented by per-account sum-of-ceil values.

Receipt creation:

```text
require MarketGroup.mode != Live or ResolvedPayoutLedger is initialized
require ordinary positive withdraw/release lane is disabled for this account
live_released_face_at_receipt = R_i
terminal_positive_claim_face = max(terminal_PNL_i - live_released_face_at_receipt, 0)
prior_bound_contribution_num = AccountJuniorBoundContribution evaluated under §2.5
require terminal_positive_claim_face * BOUND_SCALE <= prior_bound_contribution_num
```

If the final requirement is violated, the receipt MUST NOT be accepted for payout. Positive payouts halt and route to recovery or bound repair according to `cfg_resolved_receipt_underbound_action`.

Accepted receipt mutation:

```text
terminal_claim_bound_unreceipted_num -= prior_bound_contribution_num
terminal_claim_exact_receipts_num    += terminal_positive_claim_face * BOUND_SCALE
```

As receipts/refinements lower the bound, `current_payout_rate` may increase but MUST NOT decrease. A receipted account may claim top-up:

```text
claimable_now =
    floor(terminal_positive_claim_face * current_payout_rate.num
          / current_payout_rate.den)
    - paid_effective
```

Resolved payouts update only `paid_effective`; they MUST NOT re-enable the ordinary positive withdrawal lane. Once `ResolvedPayoutLedger` exists or `MarketGroup.mode != Live`, the ordinary Eq_withdraw positive-PnL component is zero for every account. Recovery direct positive settlement MUST create/update the corresponding resolved receipt and `paid_effective`, or pay no positive junior value.
### 4.3 Asset

```text
Asset {
    lifecycle in {Disabled, PendingActivation, Active, DrainOnly, Retired, Recovery}
    raw_oracle_target_price
    effective_price
    fund_px_last
    slot_last

    A_long, A_short
    K_long, K_short
    F_long_num, F_short_num

    B_long_num, B_short_num
    B_epoch_start_long_num, B_epoch_start_short_num
    K_epoch_start_long, K_epoch_start_short
    F_epoch_start_long_num, F_epoch_start_short_num
    A_epoch_start_long, A_epoch_start_short

    OI_eff_long, OI_eff_short
    stored_pos_count_long, stored_pos_count_short
    stale_account_count_long, stale_account_count_short

    loss_weight_sum_long, loss_weight_sum_short
    social_loss_remainder_long_num, social_loss_remainder_short_num
    social_loss_dust_long_num, social_loss_dust_short_num
    explicit_unallocated_loss_long, explicit_unallocated_loss_short

    support_weight = FULL_SUPPORT_WEIGHT when Active
    epoch_long, epoch_short
    mode_long, mode_short in {Normal, DrainOnly, ResetPending}
}
```

B state is per `(asset, side)`. There is no global B accumulator.

### 4.4 InsuranceLedger

```text
InsuranceLedger {
    total_available
    domain_budget[(asset, side)]
    domain_spent[(asset, side)]
    domain_global_cap[(asset, side)]
    domain_global_spent[(asset, side)]
    staged_by_close_id[(asset, side)] optional
    global_protocol_budget optional
    global_protocol_spent
}
```

```text
total_available <= I
domain_spent <= domain_budget
domain_global_spent <= domain_global_cap
sum(domain_budget - domain_spent - staged_domain_debits)
  + (global_protocol_budget - global_protocol_spent) <= total_available
staged insurance is reserved exactly once by close_id
uncollectible fees are never insurance-eligible
```

### 4.5 CloseProgressLedger, PendingDomainLossBarrier, and pending obligation ledgers

```text
CloseProgressLedger {
    entries[(account_id, close_id, asset_id, domain)] {
        gross_loss_at_close_start
        drift_reference_slot
        max_close_slot
        support_consumed
        junior_face_burned
        insurance_spent
        b_loss_booked
        explicit_loss_assigned
        pending_obligation_credits          // base + drift shares removed from this residual
        quantity_adl_applied_q
        drift_consumed
        residual_remaining
        finalized
        canceled
    }
}
```

```text
residual_remaining =
    gross_loss_at_close_start
  + drift_consumed
  - support_consumed
  - insurance_spent
  - b_loss_booked
  - explicit_loss_assigned
  - pending_obligation_credits

close_id, gross_loss_at_close_start, drift_reference_slot, max_close_slot are immutable
drift_consumed is the monotone maximum conservative adverse drift from drift_reference_slot to now
support_consumed <= floor(junior_face_burned * g.num / g.den) at consumption time
durable B booking, support consumption, insurance spend, pending-obligation credit, quantity ADL, and explicit loss assignment have exactly one matching ledger advance
```

A finalized or canceled entry may be archived only with an authenticated digest preserving reconciliation totals.

Pending-domain-loss barrier tracks two separate terms:

```text
withdraw_pending_loss_reserve =
    participant's worst-case share of
    residual_remaining + unbooked_adverse_drift_bound + uncertainty
    through max_close_slot, rounded against the participant

maintenance_pending_loss_penalty =
    already booked but unsettled B share
    + participant's share of the next bounded bookable residual chunk
    + near-term adverse drift over cfg_pending_loss_maintenance_horizon_slots
    rounded against the participant
```

The withdrawal reserve is used for withdrawals, transfers, releases, conversion, and positive-credit actions. The maintenance penalty is used in `portfolio_maintenance_req` and liquidation tests. The maintenance penalty MUST NOT exceed the withdrawal reserve and MUST be large enough to cover the next bounded loss-booking step.

```text
PendingObligationAggregate {
    barrier_id
    origin_close_key               // (source account, close_id, asset, domain)
    reference_weight               // domain loss_weight_sum at barrier formation
    remaining_reference_weight
    exited_weight_sum
    weighted_exit_drift_sum        // sum(obligation_weight * drift_consumed_at_exit)
    aggregate_base_credit
    aggregate_drift_credit
    aggregate_backing_available
    drift_credit_remainder
}
```

```text
PendingObligationLedger {
    entries[(barrier_id, account_id)] {
        domain
        obligation_weight
        drift_consumed_at_exit

        base_source_residual_credit
        drift_source_residual_credit
        source_residual_credit_total

        max_obligation
        escrowed_amount
        pulled_forward_loss
        applied_to_origin
        settled_amount
        rebate_due
        settled
    }
}
```

Rules:
- any weight reduction, leg clear, account finalization, or domain exit touching a pending barrier MUST first create or update an obligation entry;
- a barrier's `reference_weight` is fixed at barrier formation and MUST NOT shrink;
- current residual share at exit uses `remaining_reference_weight`, not current live `loss_weight_sum_side`;
- future drift share uses fixed denominator `reference_weight`, so each original participant bears `obligation_weight / reference_weight` of post-exit drift;
- individual obligation entry creation is bounded to one account and updates aggregate state in O(1);
- origin residual B-booking applies aggregate due drift credit in O(1), not by iterating obligation entries.

Exit step:

```text
apply_due_aggregate_drift_credit(barrier_id, now) first

base_source_residual_credit =
    floor(origin.residual_remaining * obligation_weight / remaining_reference_weight)
    with deterministic dust/remainder against the exiting participant

remaining_reference_weight -= obligation_weight
exited_weight_sum += obligation_weight
weighted_exit_drift_sum += obligation_weight * origin.drift_consumed

max_obligation =
    base_source_residual_credit
  + worst future drift share through max_close_slot
  + uncertainty
```

In the same atomic step:
1. increment origin `pending_obligation_credits` by `base_source_residual_credit`;
2. reduce origin `residual_remaining` by the same amount;
3. reserve escrow, settle, or pull forward value backing `max_obligation`;
4. ensure future B booking uses the reduced residual over the reduced live weight set.

Aggregate drift credit before origin B-booking:

```text
gross_due_drift =
    floor((origin.drift_consumed * exited_weight_sum - weighted_exit_drift_sum)
          / reference_weight)

due_drift_credit =
    gross_due_drift - aggregate_drift_credit
```

If positive, the same atomic step MUST:
1. increase aggregate and origin `pending_obligation_credits` by `due_drift_credit`;
2. reduce origin `residual_remaining` by `due_drift_credit`;
3. consume aggregate backing or increase pulled-forward backed loss;
4. record deterministic dust/remainder.

If due drift credit cannot be backed, the origin close MUST NOT socialize that drift over remaining weight and must route to recovery. Individual obligation entries may receive their detailed drift allocations and rebates later through bounded `settle_pending_obligation` cranks; individual settlement MUST NOT block origin B-booking once aggregate credit is backed.

### 4.6 DomainLock

```text
DomainLock {
    locked_by_close_id optional
    close_priority
    staged_residual
    staged_insurance_debit
    staged_b_booking
    phase
    last_progress_slot
    progress_nonce
}
```

A domain lock blocks only operations that mutate or depend on that side's B residual booking, quantity ADL, A-side scaling, OI, weights, staged residual, staged insurance, exposure clear, or positive-credit eligibility. It MUST NOT block unrelated accounts, unrelated domains, or authenticated asset-wide K/F/price/funding accrual.

```text
ClosePriority =
    (
        higher liquidating deficit first,
        then higher total_abs_risk_notional,
        then older snapshot_slot,
        then deterministic close_id
    )
```

Higher-priority closes preempt lower-priority conflicting closes by unwinding reversible staged state, preserving durable ledger entries and barriers, releasing held domains, and restarting under the same close id. Lower-priority incoming closes mutate nothing.

### 4.7 PortfolioAccount

```text
PortfolioAccount {
    owner
    instance_id
    market_group_id
    config_hash_at_open

    C_i
    PNL_i
    R_i                         // live released/withdrawn positive PnL face amount
    fee_credits_i <= 0 and != i128::MIN

    active_bitmap
    legs[0..N)

    account_junior_bound_contribution
    health_cert
    stale_state
    positive_credit_lock
    rebalance_lock
    liquidation_lock
    cancel_deposit_escrow
    portfolio_close_state optional {
        close_id
        required_domain_set
        snapshot_slot
        drift_reference_slot
        max_close_slot
        close_drift_reserve
        drift_consumed
        progress_measure
        close_progress_ledger_keys[0..bounded]
    }
}
```

Each configured asset has at most one canonical signed net leg. Same-asset opposite exposure MUST net into that leg.

### 4.8 Refresh records and certificates

```text
LegRefresh {
    asset_id
    side
    signed_pos_q
    conservative_pnl
    positive_pnl_current
    negative_pnl_current
    leg_local_positive_value
    positive_support_value
    maintenance_positive_value
    mm_req
    im_req
    stale_penalty
    thin_market_penalty
    b_stale
    loss_stale
    oracle_current
    funding_current
    domain_locked
    withdraw_pending_loss_reserve
    maintenance_pending_loss_penalty
    pending_obligation_exposure
    eligible_for_maintenance_positive_credit
    eligible_for_positive_credit
    eligible_for_withdraw_credit
    bankruptcy_domain = (asset_id, opposing_side)
}
```

```text
HealthCert {
    certified_equity_maint
    certified_equity_initial
    certified_equity_trade
    certified_equity_withdraw
    certified_initial_req
    certified_maintenance_req
    certified_liq_deficit
    certified_worst_case_loss

    cert_instance_id
    cert_market_group_id
    cert_config_hash
    cert_asset_set_epoch
    cert_oracle_epoch
    cert_funding_epoch
    cert_risk_epoch
    cert_asset_slot_vector_hash
    cert_effective_price_vector_hash
    active_bitmap_at_cert
    stale_penalty_accumulator
    positive_credit_mask
}
```

Certificates MUST round against the account and bind to instance id, market group, config hash, asset-set epoch, active bitmap, asset slots, and effective prices.

-------------------------------------------------------------------------------
5. Global invariants
-------------------------------------------------------------------------------

```text
C_tot <= V <= MAX_VAULT_TVL
I <= V
V >= C_tot + I
PNL_matured_pos_tot <= PNL_pos_exact_tot <= PNL_pos_bound_tot
0 < effective_price(asset) <= MAX_ORACLE_PRICE for Active/DrainOnly/Recovery assets
0 < fund_px_last(asset) <= MAX_ORACLE_PRICE for Active/DrainOnly/Recovery assets
asset.slot_last <= current_slot
insurance_ledger.total_available <= I
```

For every Active/DrainOnly/Recovery asset side:

```text
0 < A_side <= ADL_ONE
if side is Normal and has current-epoch stored positions: A_side >= MIN_A_SIDE
0 <= OI_eff_side <= MAX_OI_SIDE_Q_PER_ASSET
if Live: OI_eff_long == OI_eff_short
if OI_eff_side > 0 and side is not ResetPending: loss_weight_sum_side > 0
if loss_weight_sum_side == 0: residual may clear only via fully backed protocol-owned explicit loss
0 <= loss_weight_sum_side <= SOCIAL_LOSS_DEN
social_loss_remainder_side_num < SOCIAL_LOSS_DEN
social_loss_dust_side_num < SOCIAL_LOSS_DEN
```

```text
abs(K_side) + A_side * MAX_ORACLE_PRICE <= i128::MAX
abs(F_side_num) + A_side * MAX_ORACLE_PRICE * cfg_max_abs_funding_e9_per_slot * cfg_max_accrual_dt_slots <= i128::MAX
B_side_num <= u128::MAX
```

For every pending obligation aggregate:

```text
reference_weight is immutable after barrier formation
remaining_reference_weight + exited_weight_sum == reference_weight
origin close ledger pending_obligation_credits includes aggregate_base_credit + aggregate_drift_credit
aggregate credited amount is backed, settled, pulled-forward, or recoverable
credited amount is not included in future B booking of the origin residual
due aggregate drift credits through the current booking slot are applied before origin residual B booking
```

For every resolved payout receipt:

```text
terminal_positive_claim_face * BOUND_SCALE <= prior_bound_contribution_num
paid_effective <= floor(terminal_positive_claim_face * current_payout_rate.num
                        / current_payout_rate.den)
ordinary positive withdrawal lane is disabled for that account after receipt creation
```

For the resolved payout ledger:

```text
terminal_claim_total_bound_num =
    terminal_claim_exact_receipts_num + terminal_claim_bound_unreceipted_num

terminal_claim_bound_unreceipted_num never goes below the sum of
unreceipted accounts' stored scaled prior-bound contributions
```

For every junior-bound bucket:

```text
every member is inside basis/K/F range
current_upper_bound_num equals the formula in §2
terminal_claim_upper_bound_num equals the formula in §2
PNL_pos_bound_tot_num >= true positive junior claims * BOUND_SCALE
PNL_pos_bound_tot = ceil(PNL_pos_bound_tot_num / BOUND_SCALE)
```

ResetPending sides hold prior-epoch A/K/F/B targets for stale accounts. A side in ResetPending cannot reset again until all prior-epoch stale accounts are settled, migrated, or recovered.

-------------------------------------------------------------------------------
6. Claims, equity lanes, support, and resolved payout
-------------------------------------------------------------------------------

```text
Residual = V - (C_tot + I)
PosPNL_i = max(PNL_i, 0)
FeeDebt_i = max(-fee_credits_i, 0)
ReleasedPos_i = max(PosPNL_i - R_i, 0)     // Live ordinary lane only
```

The `R_i` offset records live positive PnL face already released. Once `MarketGroup.mode != Live` or `ResolvedPayoutLedger` is initialized, `R_i` is frozen except for audit rebasing that cannot increase payout.

```text
if PNL_matured_pos_tot == 0: h = (1,1)
else h = (min(Residual, PNL_matured_pos_tot), PNL_matured_pos_tot)

if PNL_pos_bound_tot == 0: g = (1,1)
else g = (min(Residual, PNL_pos_bound_tot), PNL_pos_bound_tot)

withdraw_haircut = min_fraction(h, g)
junior_impaired = PNL_pos_bound_tot > Residual
ordinary_positive_withdraw_enabled =
    MarketGroup.mode == Live && ResolvedPayoutLedger is not initialized
```

`PNL_matured_pos_tot` is retained only for maturity accounting and is never used without `g`.

### 6.1 Exact leg-local positive credit

For every refreshed leg:

```text
leg_local_factor =
    min(
        maturity_or_warmup_factor,
        oracle_confidence_factor,
        target_effective_dual_price_factor,
        thin_market_factor,
        domain_lock_factor,
        pending_loss_factor,
        recovery_factor,
        configured_leg_credit_cap
    )
```

Each factor is in `[0, SUPPORT_WEIGHT_SCALE]`, deterministic, exact, and rounded against the account. A disabled factor defaults to `SUPPORT_WEIGHT_SCALE`; an ineligible factor is `0`.

```text
leg_local_positive_value =
    floor(positive_pnl_current * leg_local_factor / SUPPORT_WEIGHT_SCALE)

g_positive_value =
    floor(positive_pnl_current * g.num / g.den)

leg_positive_support_value =
    0 if not eligible_for_positive_credit
    else min(leg_local_positive_value, g_positive_value)

leg_maintenance_positive_value =
    0 if not eligible_for_maintenance_positive_credit
    else if junior_impaired:
        min(leg_local_positive_value, g_positive_value)
    else:
        leg_local_positive_value
```

Impairment MUST never increase maintenance credit:

```text
if junior_impaired:
    leg_maintenance_positive_value <= non_impaired_leg_local_positive_value
```

### 6.2 Loss-curing support

Loss-curing positive support is durable only if it comes from:
- senior capital `C_i`;
- explicitly realized nonjunior gains;
- a leg being closed/finalized in the same close where matching source-domain loss exposure is recognized or reserved;
- settled or rebated pending obligation surplus; or
- a previously locked/burned junior claim whose source-domain obligation has been accounted for.

`durable_realized_nonjunior_gains` are quote-token gains already in `V` that are not backed by unresolved counterparty PnL, not junior claims, and not insurance. Mark PnL, open funding PnL, and open-leg gains are excluded unless their matching loss is durably collected or reserved.

Open positive PnL from a leg that remains open may support maintenance/trade approval, but MUST NOT reduce a bankruptcy residual or avoid B booking.

When support `S` is consumed:

```text
if g.num == 0: S must be 0
else junior_face_burn = ceil(S * g.den / g.num)
```

`PNL_i`, `PNL_pos_exact_tot`, `PNL_pos_bound_tot`, `PNL_matured_pos_tot` when applicable, and `junior_face_burned` MUST update atomically.

### 6.3 Equity lanes

```text
if ordinary_positive_withdraw_enabled:
    Eq_withdraw_i =
        C_i + floor(ReleasedPos_i * withdraw_haircut.num / withdraw_haircut.den)
            + min(PNL_i,0) - FeeDebt_i - penalties
else:
    Eq_withdraw_i =
        C_i + min(PNL_i,0) - FeeDebt_i - penalties

Eq_maint_i =
    C_i + conservative_negative_leg_pnl
        + sum(maintenance_eligible leg_maintenance_positive_value)
        - FeeDebt_i - penalties

Eq_initial_i =
    C_i + conservative_negative_leg_pnl
        + sum(initial_eligible leg_positive_support_value)
        - FeeDebt_i - penalties

Eq_trade_i =
    C_i + conservative_negative_leg_pnl
        + sum(trade_eligible leg_positive_support_value)
        - FeeDebt_i - penalties

Eq_no_positive_credit_i =
    C_i + conservative_sum_negative_leg_pnl - FeeDebt_i - penalties
```

PositiveCreditActions reject or use no-positive-credit lanes if any contributing leg is stale, B-stale, loss-stale, partial, locked, pending-loss-exposed, pending-obligation-exposed, recovery-mode, target/effective-lagged without dual-price pass, thin-market locked, or hmax/stress locked.

### 6.4 Resolved payout

```text
resolved_positive_payout_delta =
    floor(terminal_positive_claim_face * current_payout_rate.num
          / current_payout_rate.den)
    - paid_effective
```

No positive resolved payout is allowed before `ResolvedPayoutLedger` is initialized. Payout order MUST NOT change final entitlement; later top-ups are allowed as conservative unreceipted bounds tighten. Receipt underbound violations halt payouts or route to recovery before the invalid receipt can affect payout rate.

Total positive payout safety:

```text
ordinary live positive face released = R_i before resolved ledger
resolved terminal claim face = max(terminal_PNL_i - R_i_at_receipt, 0)
ordinary positive lane disabled after resolved ledger
resolved paid_effective tracked by paid_effective
```

A user MUST NOT receive positive PnL through both lanes for the same face claim.

Realizing positive PnL MUST NOT increase withdrawable senior claims unless matching portfolio/counterparty losses, fees, stale penalties, and support consumption are durably recognized in the same instruction or were already current.

-------------------------------------------------------------------------------
7. Portfolio health and staleness
-------------------------------------------------------------------------------

A full portfolio refresh computes:

```text
portfolio_maintenance_req =
    gross_mm
    - hedge_credit
    + stale_penalty
    + concentration_penalty
    + thin_market_penalty
    + unsettled_loss_penalty
    + target_effective_lag_penalty
    + domain_lock_penalty
    + sum(maintenance_pending_loss_penalty)
    + pending_obligation_exposure

portfolio_initial_req =
    gross_im - initial_hedge_credit + stricter_penalties
```

Withdraw/trade/release/conversion gates use `withdraw_pending_loss_reserve`; liquidation health uses `maintenance_pending_loss_penalty`. Pending obligations that are not fully escrowed add conservative account-local exposure.

Hedge credit is optional and deterministic:

```text
hedge_credit <= min(offset_leg_risks) * cfg_max_offset_bps / 10_000
```

Allowed only for configured buckets with current epochs and no unsettled B, stale cert, target/effective lag without dual-price pass, recovery, close barrier, pending-domain-loss barrier, or pending obligation.

```text
initial-healthy      if certified_equity_initial >= certified_initial_req
maintenance-healthy  if certified_equity_maint   >= certified_maintenance_req
liquidatable         if certified_liq_deficit > 0 after full refresh
```

A certificate is fresh only if instance id, market group, config hash, asset-set epoch, epochs, active bitmap, asset slot vector, and effective price vector remain valid.

When stale:

```text
stale_loss_bound =
    sum_abs_notional_at_cert * max_price_move_since_cert
    + max_funding_move_since_cert
    + fee_bound
    + configured_oracle_uncertainty_bound
    + thin_market_bound
    + domain_lock_bound
    + pending_domain_loss_bound
    + pending_obligation_bound
```

Stale accounts cannot perform favorable actions, use hedge credit, use positive PnL for approval/support, or receive resolved positive payout. They may refresh, rebalance defensively, liquidate, recover, or forfeit/detach a dead leg.

-------------------------------------------------------------------------------
8. Settlement helpers
-------------------------------------------------------------------------------

Every `C_i`, `PNL_i`, position, B, fee, close-state, insurance, support, junior-bound, pending obligation, and ledger mutation MUST use aggregate-updating helpers.

`attach_leg` requires old effects settled, side mode permitting attach, full account refresh, no active close/pending loss/pending obligation conflict, asset Active, and no same-asset opposite nonzero leg.

`clear_leg` requires A/K/F/B settled. It quarantines remainder, transfers local `b_rem` to dust, subtracts weight only after pending obligations are escrowed/settled/pulled forward with source-residual and aggregate drift credit, clears local fields, and mutates OI only through a transition proving matching OI change.

Before any `loss_weight_sum_side` change, quarantine `social_loss_remainder_side_num` to dust. Pending residual against a side forbids weight-set changes except by close/recovery or by preserving/escrowing/pulling-forward the old loss-weight obligation with source-residual and aggregate drift credit.

For a nonzero leg:

```text
B_target = current B_side_num if current epoch else B_epoch_start_side_num under ResetPending
ΔB = B_target - b_snap
num = b_rem + loss_weight * ΔB
B_loss = floor(num / SOCIAL_LOSS_DEN)
b_rem_new = num % SOCIAL_LOSS_DEN
KF_pnl_delta = exact signed-floor A/K/F settlement
net_pnl_delta = KF_pnl_delta - B_loss
```

If full B settlement is too large, partial settlement is allowed. While `B_remaining > 0`, no user-favorable action may continue.

Dust and remainders are audit state only and MUST round against the account.

-------------------------------------------------------------------------------
9. A/K/F/B mechanics
-------------------------------------------------------------------------------

`accrue_asset_to(asset, now_slot, effective_price, funding_rate)` requires Active/DrainOnly live mode, authenticated time, valid price, and bounded funding rate. Domain locks do not block K/F/price/time accrual. If accrual occurs while a close snapshot exists, that snapshot becomes stale until recomputed or re-aged. Accrual MUST NOT mutate B, A, OI, weights, staged residuals, staged insurance, ADL, pending barriers, pending obligations, or exposure-clear state for a locked domain unless held by the close/recovery path.

Before any accrual/effective-price/K/F write, affected JuniorClaimBoundLedger buckets MUST be recomputed conservatively for the new state using §2 formulas.

```text
dt = now_slot - asset.slot_last
funding_active = dt > 0 && funding_rate != 0 && OI_eff_long != 0 && OI_eff_short != 0 && fund_px_last > 0
price_move_active = effective_price != previous_effective_price && (OI_eff_long != 0 || OI_eff_short != 0)
```

If active, require `dt <= cfg_max_accrual_dt_slots`; price moves must satisfy configured per-slot bound. K/F/stress candidates are computed in exact wide arithmetic and validated before writes.

`apply_due_aggregate_drift_credit(barrier_id, now)` is O(1). It computes aggregate due drift from exited participants using `exited_weight_sum`, `weighted_exit_drift_sum`, and immutable `reference_weight`. It MUST run before any B booking of the origin residual at the same or later `origin.drift_consumed`.

`book_bankruptcy_residual_chunk(asset, side, residual_remaining)` is O(1):

```text
H = u128::MAX - B_side_num
W = loss_weight_sum_side
R = social_loss_remainder_side_num

max_scaled = (H + 1) * W - 1
if R > max_scaled: max_chunk_by_B = 0
else:              max_chunk_by_B = floor((max_scaled - R) / SOCIAL_LOSS_DEN)

engine_chunk = min(residual_remaining, max_chunk_by_B, cfg_public_b_chunk_atoms)
delta_B = floor((engine_chunk * SOCIAL_LOSS_DEN + R) / W)
new_remainder = (engine_chunk * SOCIAL_LOSS_DEN + R) % W
```

Successful B booking requires `W > 0`, positive chunk/delta, and B headroom. Before booking, aggregate pending obligation drift credits due through the booking slot for the origin close MUST be applied in O(1). The same atomic step increments `b_loss_booked`, reduces `residual_remaining`, consumes any matching aggregate pending obligation escrow, and updates/releases the pending barrier. B booking MUST use the source residual after subtracting `pending_obligation_credits`; it MUST NOT socialize credited shares over remaining weight.

If `W == 0`, residual may clear only by already-reserved eligible insurance or explicit protocol-owned backing preserving `V >= C_tot + I`; otherwise route to recovery.

Individual `PendingObligationLedger` entries are settled or rebated through bounded per-account cranks and MUST NOT block B booking once aggregate credits are backed.

Quantity ADL applies exactly once after residual durability and is atomic with closing exposure clear/finalization or protected by a non-preemptible finalization barrier.

`begin_full_drain_reset(asset, side)` requires zero OI, no close/pending barrier, no pending obligations, and side not already `ResetPending`. It snapshots A/K/F/B epoch-start state, quarantines remainder, resets current A/K/F/B and weights, increments epoch, sets `A_side = ADL_ONE`, and enters `ResetPending`.

-------------------------------------------------------------------------------
10. Liquidation and bankrupt close
-------------------------------------------------------------------------------

Liquidation is triggered by:

```text
certified_equity_maint < certified_maintenance_req
```

A liquidation instruction refreshes the full account and builds a deterministic plan from all active legs. Plan order is deterministic: highest risk contribution, largest deficit, asset id ascending, Long before Short. Hints cannot choose attribution.

Before mutation, reserve current-step domains. Lower-priority conflicts preempt/unwind; higher-priority conflicts mutate nothing. Initial close sets immutable:

```text
DriftReferenceSlot = snapshot_slot
MaxCloseSlot = DriftReferenceSlot + cfg_max_bankrupt_close_lifetime_slots
CloseDriftReserve >= max adverse close drift through MaxCloseSlot
```

Before any account close, finalization, or weight reduction, the plan MUST call:

```text
settle_pending_obligations(account):
    for every pending barrier where account is a domain participant:
        apply_due_aggregate_drift_credit(barrier_id, now)
        compute base_source_residual_credit from remaining_reference_weight
        compute max_obligation including future drift share through max_close_slot
        require one of:
            escrow max_obligation backing base and drift credits;
            settle current booked share and preserve remaining obligation weight;
            add unpaid max_obligation shortfall to this account's LossVector
                with Domain_j = original barrier domain and process it durably;
            route to recovery
        atomically credit the origin residual by base_source_residual_credit
```

`SupportPool` for residual curing may include only durable support:

```text
reserve_required_for_remaining_open_risk =
    conservative equity required so every remaining open leg
    not included in the current close candidate set still satisfies maintenance
    after worst bounded price/funding/stale/thin/lag/pending-maintenance-loss/liquidation costs

SupportPool =
    max(0,
        available senior C_i
      + durable_realized_nonjunior_gains
      + durable positive support from legs being closed/finalized
      + refundable pending-obligation surplus that is already finalized
      - fee debt
      - stale penalties
      - required locks
      - pending-withdrawal-loss reserves required for support use
      - reserve_required_for_remaining_open_risk)
```

Open non-candidate positive PnL is excluded from residual curing. If such support exists and would avoid residual, the engine MUST expand to deterministic terminal portfolio close, prove support is unavailable, or route to recovery. It MUST NOT B-book a residual from a partial liquidation while deterministic account-close support remains.

For each losing candidate:

```text
LegLoss_j = max(0, loss_to_close_leg_j + liquidation_cost_j + side_effect_loss_j)
Domain_j  = (asset_j, opposing_side_j)
```

Losses subtract durable progress already recorded for the same `(account, close_id, asset, domain)`. Pending-obligation shortfalls pulled forward from another barrier are included as their own `LegLoss_j` with `Domain_j = original barrier domain`.

Support allocation is deterministic, preferably pro-rata:

```text
TotalLoss = sum(LegLoss_j)
SupportToLeg_j = floor(SupportPool * LegLoss_j / TotalLoss)
UncuredLoss_j = LegLoss_j - SupportToLeg_j
```

Insurance allocation:

```text
InsuranceBudget_j =
    remaining_domain_budget[Domain_j]
  + min(domain_global_cap[Domain_j] - domain_global_spent[Domain_j],
        global_protocol_budget - global_protocol_spent)

InsuranceToLeg_j <= min(UncuredLoss_j, InsuranceBudget_j)
Residual_j = UncuredLoss_j - InsuranceToLeg_j
```

Residuals may only book to `Domain_j`, never to unrelated assets, all shorts, all profitable accounts, or a global B index.

A successful liquidation/rebalance of an unhealthy account must strictly reduce deterministic `RiskScore` or certified liquidating deficit. Equal-score churn reverts.

### 10.1 Bankrupt close

Minimum phases:

```text
Touched
PendingObligationsSettledOrPulledForward
FullPortfolioSideEffectsPartiallySettled
PortfolioLossVectorComputed
SupportPoolComputed
SupportAllocated
InsuranceAllocated
ResidualsPartiallyBooked
ResidualsBooked
QuantityADLApplied
AccountFinalized
CanceledIfCured
```

Durable progress includes support consumed, junior face claim burned/locked, insurance spent, B loss booked, explicit loss assigned, pending obligation escrow consumed, pending obligation source-residual credit, aggregate pending obligation drift credit, quantity ADL applied, and drift consumed. Each durable item updates `CloseProgressLedger`, `PendingObligationAggregate`, or `PendingObligationLedger` atomically.

Remaining residual:

```text
remaining_residual =
    gross_loss_at_close_start
  + total_adverse_drift_from(drift_reference_slot, now)
  - support_consumed
  - insurance_spent
  - b_loss_booked
  - explicit_loss_assigned
  - pending_obligation_credits
```

`drift_consumed` is the monotone maximum total adverse drift from immutable `drift_reference_slot`. Favorable post-start movement cannot increase support, payout, or withdrawable value.

Every continuation must first check whether the close is cancelable after any owner `cancel_deposit_escrow`. It MUST NOT consume newly deposited cancel-escrow capital as support before this check. Then it must strictly reduce:

```text
CloseProgressMeasure =
    residual_remaining
  + unbooked_adverse_drift_bound
  + unsettled_B_loss
  + unsettled_insurance_staging
  + unsettled_pending_obligations
```

after adding worst-case drift. If not, if now exceeds `MaxCloseSlot`, if reserve is exhausted, or if drift can outpace progress, route to recovery.

Close cancel is owner-callable and atomic:

```text
cure_and_cancel_close(account, optional_deposit):
    deposit to cancel_deposit_escrow
    full refresh under current state
    settle reversible side effects
    require account initial-healthy after pending-obligation reserves
    require no B booking, quantity ADL, insurance spend, explicit loss assignment,
            support consumption, pulled-forward pending obligation shortfall,
            pending_obligation_credit, or pending_obligation_drift_credit
    release staged reversible state and pending barriers from this close
    mark close ledger canceled
    release cancel escrow as ordinary capital
```

Before consuming a prior snapshot, recompute or re-age. Re-aging incorporates all K/F/price/slot accrual, computes adverse movement from immutable `drift_reference_slot`, recomputes `g`, junior impairment, eligibility, domain locks, pending barriers, pending obligations, B-stale/recovery state, and restages insurance if needed.

-------------------------------------------------------------------------------
11. User operations
-------------------------------------------------------------------------------

A user-favorable operation MUST:
1. authenticate owner/authority;
2. validate clock, oracle target, effective price, admission, and inputs;
3. continue conflicting close, recover, cure-and-cancel, detach/forfeit a dead leg, or fail before unrelated mutation;
4. refresh the full active portfolio;
5. settle A/K/F/B for touched legs;
6. settle losses before fees;
7. recompute `HealthCert`;
8. run candidate checks under final hmax/stale/B/loss-stale/domain-lock/pending-loss/pending-obligation/recovery state;
9. commit only if all invariants hold.

Deposits are pure capital. Deposits into accounts with cancelable closes MAY be placed in cancel escrow and MUST receive cancel consideration before being consumed as close support. Other deposits into stale/B-stale/locked accounts are loss-curing only until refresh clears locks.

Withdrawals use post-withdraw candidate state, `withdraw_haircut = min(h,g)`, and full `withdraw_pending_loss_reserve`. Ordinary positive-PnL withdrawals are disabled when `ordinary_positive_withdraw_enabled == false`. Any positive-credit action must pass gates; otherwise use `Eq_no_positive_credit_i`.

Trades require:
- full portfolio refresh for both counterparties; or an engine-verified, authenticated post-trade health certificate for a verified maker/liquidator account;
- the certificate covers candidate trade size, price envelope, active bitmap, current epochs, effective prices, locks, barriers, and all existing legs;
- loss-current market state;
- current B/K/F settlement for touched legs;
- side-mode gating;
- OI/position bounds;
- candidate-slippage neutralization;
- no-positive-credit approval under locks/stress/stale states;
- matched-side loss recognition before gain extractability;
- exact fee enforcement.

Trades MUST NOT execute while bounded catchup remains incomplete unless purely risk-reducing and conservative.

-------------------------------------------------------------------------------
12. Rebalance
-------------------------------------------------------------------------------

Allowed:
- move support equity across active legs within the same account and instance;
- reduce risk by closing, shrinking, or collateral shifting while preserving, escrowing, settling, or pulling forward pending loss-weight obligations with source-residual and aggregate drift credit;
- refresh certificates;
- consume durable haircut-bounded support against losses without converting it to capital;
- forfeit/detach a dead recovery-mode leg.

Forbidden:
- double count collateral;
- treat positive PnL as senior capital;
- use stale/B-stale/domain-locked/pending-loss/pending-obligation profitable legs for credit;
- consume open non-candidate positive PnL to cure residuals;
- erase fee debt or bankruptcy loss;
- improve one account by worsening another except explicit liquidation transfer rules;
- move bankruptcy residual across asset domains;
- use cross-instance equity or PnL.

```text
senior_claim_after
    <= senior_claim_before
       + realized_nonjunior_pnl
       - fees
       - realized_losses
```

For unhealthy accounts, accepted rebalance/liquidation requires strict `RiskScore` or deficit progress.

-------------------------------------------------------------------------------
13. Keeper cranks, recovery, and resolution
-------------------------------------------------------------------------------

Keeper cranks are bounded and incremental. Hints are never assumed complete. Missing global accounts do not cause rollback merely because more accounts exist. If equity-active accrual is performed on an exposed market, protective progress must also commit. Domain locks, pending-domain-loss barriers, and pending obligations must progress, settle, or route to recovery. Close snapshots must be recomputed or re-aged before consumption. Asset-wide K/F accrual is not blocked by domain locks.

If `authenticated_now_slot - asset.slot_last > cfg_max_accrual_dt_slots`, use bounded catchup segments. While incomplete, the market is loss-stale: positive PnL uses hmax/no-positive-credit lanes, reserve release/conversion are disabled, risk-increasing trades/nonflat withdrawals/OI-increasing actions are blocked, and risk-reducing actions may continue.

A public `CrankForward` market MUST expose permissionless terminal recovery for any state where bounded progress cannot continue, including headroom/representability failure, account B settlement failure, B-index exhaustion, active close failure, domain lock/barrier/obligation failure, insurance budget exhaustion, snapshot re-aging failure, close drift expiration, oracle/target unavailability, counter/epoch overflow, asset lifecycle failure, junior-bound refinement failure, resolved payout bound failure, payout-lane conflict, invalid junior-bound bucket, and `N` too large for bounded refresh.

Recovery price is deterministic: authenticated recovery price when available and representable; otherwise immutable configured fallback. Caller cannot choose recovery price. The fallback may be conservative against the owner and may overcharge the loss domain or underpay profitable legs; both transfers are configured recovery risk and MUST be bounded.

Recovery preserves and reconciles `CloseProgressLedger`, `PendingObligationAggregate`, `PendingObligationLedger`, `ResolvedPayoutLedger`, and pending barriers; it cannot erase ledgered B/ADL/support/obligation progress and recompute gross loss. Recovery direct positive payout MUST create/update a resolved receipt and `paid_effective`; it cannot bypass the unified payout lane. Recovery must complete, settle, or deterministically unwind close/lock/obligation state before clearing it. It must not orphan barriers, double-spend insurance, double-pay positive PnL, clear PnL without durable loss state, or leave booked B loss charged again.

Resolved payout is progressive:
1. initialize `ResolvedPayoutLedger` with `snapshot_residual` and conservative terminal claim bound after terminal losses are settled/reserved;
2. disable ordinary positive-PnL withdrawal/release for all accounts;
3. allow any account to create a bounded exact receipt by refreshing and settling terminal K/F/B;
4. require `exact receipt claim * BOUND_SCALE <= prior_bound_contribution_num`; otherwise halt payouts and route to recovery/bound repair before accepting it;
5. replace that account's unreceipted bound with exact `terminal_positive_claim_face`;
6. recompute non-decreasing `current_payout_rate`;
7. pay only `claimable_now` top-up.

No full account sweep is required before first safe positive payout. Final top-ups require receipts or exact bucket refinements. Receipt creation SHOULD be incentivized by wrapper bounties or claim-funded fees.

Dead-leg forfeit/detach is owner-callable and bounded for terminal/recovery/dead assets. It refreshes the full account, settles or over-reserves losses, values positive PnL at zero, values negative PnL at conservative fallback/recovery loss, burns/forfeits junior claim, books residual only to `(asset, opposing_side)`, clears only after residual durability, and leaves unrelated legs usable once otherwise healthy.

-------------------------------------------------------------------------------
14. Cross-instance transfers and UI aggregation
-------------------------------------------------------------------------------

A cross-instance transfer is not cross margin. It is two separate protocol actions:

```text
source instance:
    full refresh
    settle losses
    enforce no-positive-credit / pending-loss / B-stale / recovery gates
    withdraw value up to the senior claim, paid only as actual quote tokens

destination instance:
    deposit actual received quote tokens as new capital
```

The same collateral, PnL, junior claim, certificate, or insurance value MUST NOT be counted in two instances at once. Wrappers MUST NOT create synthetic merged collateral or merged liquidation health.

-------------------------------------------------------------------------------
15. Mode transitions and wrapper obligations
-------------------------------------------------------------------------------

MarketGroup mode transitions:

```text
Live -> Recovery
Live -> Resolved
Recovery -> Resolved
Recovery -> Retired/Closed if no claims remain
Resolved -> Retired/Closed after all claims closed or archived
```

Recovery mode may settle accounts directly or transition to Resolved after recovery prices, ledgers, barriers, obligations, payout state, and terminal claim bounds are durable.

Wrappers own authorization, oracle normalization, raw target storage, effective-price staircase policy, account proof packing, anti-spam economics, hint markets/off-chain discovery, thin-market guardrails, resolved receipt incentives, pending-obligation settlement incentives, and MEV-aware cancel transaction routing.

Public wrappers MUST NOT expose caller-controlled:
- admission/funding/threshold/future slot;
- asset activation, drain, or recovery lifecycle changes;
- B residual chunk size or account-B settlement chunk size;
- junior-bound bucket membership, formula inputs, or bound-lowering interpretation;
- portfolio support/insurance allocation method;
- residual attribution;
- domain insurance budget override;
- domain lock order, required-domain set, preemption priority, pending barrier/obligation, or ledger interpretation;
- pending reserve split, aggregate drift credit, or obligation sizing;
- whether K/F accrual is blocked by locks;
- close snapshot validity, re-aging, drift reserve/reference slot, ledger, or max close slot;
- resolved payout bound/rate/receipt values;
- ordinary-vs-resolved payout lane selection;
- recovery fallback price;
- favorable stale-certificate interpretation;
- cross-instance netting or merged health.

Public wrappers MUST expose full account refresh, hinted crank, bounded catchup, active close continuation, account-B settlement continuation, domain-lock/pending-loss/pending-obligation continuation, permissionless recovery, owner cure-and-cancel, owner dead-leg forfeit/detach, resolved claim receipt, and rebalance-on-touch.

Target/effective lag MUST not give users a free option. Extraction-sensitive actions reject or shadow-check; risk-increasing trades use dual-price/no-positive-credit checks. No wrapper may treat a global accumulator or UI-aggregated cross-instance balance as proof a specific account is healthy.

-------------------------------------------------------------------------------
16. Required proof and TDD coverage
-------------------------------------------------------------------------------

1. `full_shared_cross_liquidity_all_active_assets_weight_one`.
2. `mutable_asset_activation_requires_full_envelope_proofs`.
3. `asset_activation_invalidates_or_scopes_certs_fail_closed_without_full_scan`.
4. `asset_cannot_activate_with_nonzero_or_unreconciled_state`.
5. `activation_rate_limit_prevents_staleness_lock_spam`.
6. `drain_retire_recovery_exit_requires_no_oi_no_pending_barriers_no_pending_obligations_no_unsettled_epochs`.
7. `cross_instance_ui_aggregation_not_health_or_collateral_proof`.
8. `cross_instance_transfer_requires_actual_quote_token_withdraw_and_deposit`.
9. `global_cross_margin_all_legs_support_maintenance`.
10. `global_cross_margin_does_not_create_global_B_domain`.
11. `bad_asset_residual_charged_only_to_asset_side_domain`.
12. `withdraw_positive_credit_bounded_by_min_h_g`.
13. `ordinary_positive_withdraw_disabled_after_resolved_ledger`.
14. `resolved_receipt_uses_R_i_at_receipt_and_cannot_double_pay_live_releases`.
15. `recovery_positive_payout_uses_resolved_payout_ledger`.
16. `total_positive_payout_across_live_and_resolved_lanes_never_exceeds_entitlement`.
17. `resolved_payout_progressive_receipts_never_overpay`.
18. `resolved_payout_order_invariant_with_topups`.
19. `resolved_receipt_exact_claim_scaled_must_not_exceed_prior_bound_num`.
20. `resolved_bound_understatement_halts_payout_or_recovers`.
21. `terminal_claim_bound_unreceipted_num_never_understates_after_receipt_decrements`.
22. `resolved_receipt_decrement_uses_same_scaled_units_as_bucket_aggregate`.
23. `junior_bound_bucket_current_formula_never_understates`.
24. `junior_bound_bucket_terminal_formula_never_understates`.
25. `prior_bound_contribution_num_equals_removed_scaled_bucket_share`.
26. `bucket_member_out_of_range_fails_closed_or_rebounds_to_hard_max`.
27. `accrual_cannot_use_stale_bucket_range_for_favorable_action`.
28. `PNL_pos_bound_tot_never_understates_true_junior_claims`.
29. `account_refresh_replaces_own_junior_bound_bucket_contribution`.
30. `impairment_never_increases_leg_maintenance_positive_credit`.
31. `leg_local_positive_value_formula_rounds_against_account`.
32. `pending_barrier_withdraw_reserve_covers_full_lifetime_worst_case`.
33. `pending_barrier_maintenance_penalty_uses_near_term_bound_not_full_lifetime`.
34. `pending_barrier_maintenance_penalty_does_not_cascade_on_full_lifetime_reserve`.
35. `pending_obligation_escrow_required_before_weight_removal`.
36. `pending_obligation_credit_decrements_origin_residual_once`.
37. `pending_obligation_uses_reference_weight_denominator`.
38. `aggregate_due_drift_credit_is_O_1_before_b_booking`.
39. `individual_pending_obligation_settlement_is_chunked_not_booking_blocking`.
40. `pending_obligation_drift_credit_tracks_frozen_exit_fraction`.
41. `origin_residual_booking_excludes_credited_pending_obligation_share_and_drift`.
42. `participant_finalization_pulls_forward_pending_obligation`.
43. `phantom_weight_without_backing_reverts`.
44. `pending_obligation_surplus_rebated_or_domain_credited`.
45. `support_pool_never_uses_face_value_positive_pnl_when_g_below_one`.
46. `open_noncandidate_positive_pnl_cannot_cure_residual`.
47. `durable_realized_nonjunior_gains_excludes_unfunded_mark_pnl`.
48. `effective_support_consumption_burns_required_face_junior_claim`.
49. `self_dealing_realization_forces_matching_loss_recognition`.
50. `resolved_close_does_not_double_pay_R_i`.
51. `close_cancel_after_recapitalization_before_irreversible_progress`.
52. `continuation_checks_cancel_before_consuming_cancel_escrow`.
53. `pending_barrier_allows_risk_reduction_with_weight_obligation_preserved`.
54. `begin_full_drain_reset_forbidden_while_reset_pending`.
55. `verified_maker_exemption_requires_engine_verified_post_trade_health_cert`.
56. `rebalance_conserves_senior_claims`.
57. `cross_margin_offset_cap_never_below_loss_envelope`.
58. `liquidation_order_support_and_insurance_are_caller_independent`.
59. `partial_liquidation_cannot_socialize_while_account_support_remains`.
60. `domain_budgeted_insurance_prevents_bad_asset_global_insurance_drain`.
61. `permitted_global_protocol_first_loss_capped_by_domain_and_global_budget`.
62. `zero_weight_domain_residual_cannot_clear_without_backing`.
63. `B_booking_exact_remainder_conservation`.
64. `B_stale_blocks_withdraw_convert_close_and_risk_increase`.
65. `bankrupt_portfolio_close_books_all_residuals_before_clear`.
66. `bankruptcy_residual_excludes_protocol_fees`.
67. `uncollectible_fees_forgiven_not_socialized`.
68. `preemptive_close_priority_prevents_hold_and_wait_deadlock`.
69. `preempted_close_restart_cannot_double_book_residual`.
70. `durable_b_booking_requires_matching_close_progress_ledger_advance`.
71. `durable_quantity_adl_requires_matching_close_progress_ledger_advance`.
72. `pending_domain_loss_barrier_blocks_weight_exit_until_residual_durable`.
73. `preempted_close_releases_locks_but_not_pending_loss_barrier`.
74. `close_id_reused_across_preemption_restart_until_finalized`.
75. `quantity_adl_and_account_finalization_atomic_or_barriered`.
76. `drift_reference_slot_and_max_close_slot_immutable`.
77. `drift_consumed_total_from_reference_slot_not_working_snapshot`.
78. `bankrupt_close_progress_decreases_net_of_close_drift`.
79. `expired_close_drift_routes_to_recovery`.
80. `domain_lock_does_not_block_asset_wide_kf_accrual`.
81. `effective_price_raw_target_lag_no_free_option`.
82. `loss_stale_catchup_blocks_risk_increase_until_current`.
83. `recovery_fallback_price_required_for_public_markets`.
84. `permissionless_recovery_no_caller_chosen_price`.
85. `dead_leg_forfeit_unfreezes_unrelated_collateral_without_value_escape`.
86. `forfeit_recovery_leg_books_to_bankruptcy_domain_not_same_side`.
87. `authoritatively_flat_account_never_receives_B_loss`.
88. `no_single_instruction_full_market_requirement`.
89. `global_accumulator_not_account_health_proof`.
90. `active_bitmap_canonical_no_hidden_legs`.
91. `canonical_single_leg_per_asset_no_same_asset_double_support`.
92. `N_too_large_rejects_public_initialization_or_activation`.
93. `certificate_bound_to_instance_config_asset_set_slots_and_prices`.
94. `market_mode_transitions_do_not_orphan_claims`.
-------------------------------------------------------------------------------
17. Audit summary and intended tradeoff
-------------------------------------------------------------------------------

[FIXED] Resolved payout rounding/decrement mismatch.
    v15.10 stores resolved terminal bounds and receipt decrements in the same `BOUND_SCALE` numerator domain. Bucket aggregates are not ceil-of-sum quote amounts decremented by sum-of-ceil account values, so receipts cannot over-decrement the unreceipted bound.

[FIXED] Junior-bound bucket arithmetic.
    v15.10 keeps exact bucket formulas, side-specific basis extremes, `R_i = 0` conservatism, account-level prior-bound contributions, and fail-closed re-bucketing rules, but makes all receipt-facing terms additive in scaled units.

[FIXED] Impairment maintenance fail-open.
    Impaired maintenance credit remains `min(local, g)`, so impairment can only reduce or preserve credit, never increase it.

[KEPT] Unified payout lane.
    Ordinary positive withdrawal is disabled after resolved payout begins or mode leaves Live; recovery payouts use the resolved ledger.

[KEPT] Aggregate pending-obligation drift netting.
    Due drift credit is applied in O(1) before B booking; individual obligation settlement/rebate is chunked separately.

[KEPT] Mutable asset set and full shared cross liquidity.
    Asset activation remains fail-closed, and all Active assets inside the instance are full support weight 1.0.

v15.10 guarantee:

```text
one honest crank with a valid account hint can force bounded progress on that account;
inside one market-group instance, all Active assets share full cross-margin solvency;
asset sets are mutable only through fail-closed lifecycle gates;
bankruptcy residuals remain market-side local;
separate instances are isolated even if a UI aggregates them;
junior impairment, exact scaled bucketed PnL bounds, pending losses, participant exits, close preemption, asset lifecycle changes, resolved payout ordering, payout-lane transitions, and pending-obligation drift accounting cannot let users extract unbacked value or get stuck indefinitely.
```

