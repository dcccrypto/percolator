# Risk Engine Spec (Source of Truth) — v12.19.6

**Combined Single-Document Native 128-bit Revision  
(Wrapper-Owned Two-Point Warmup Admission / Touch-Time Reserve Re-Admission / Wrapper-Owned Account-Fee Policy / Per-Account Recurring-Fee Checkpoint / Wrapper-Supplied High-Precision Funding Side-Index Input / Simplified Scheduled-Plus-Pending Warmup / Exact Candidate-Trade Neutralization / Self-Synchronizing Terminal-K-Delta Resolved Settlement / Whole-Only Automatic Flat Conversion / Full-Local-PnL Maintenance / Immutable Configuration / Unencumbered-Flat Deposit Sweep / Mandatory Post-Partial Local Health Check / Explicit Resolution Mode / Self-Neutral Insurance-Siphon Resistance / Round-Robin Sweep Generation / Engine-Enforced Stress-Scaled Admission Edition)**

**Design:** Protected principal + junior profit claims + lazy A/K/F side indices (native 128-bit base-10 scaling)  
**Status:** implementation source of truth (normative language: MUST / MUST NOT / SHOULD / MAY)  
**Scope:** perpetual DEX risk engine for a single quote-token vault

This revision supersedes v12.19 rev5. It preserves all rev5 economics and safety properties, and tightens the remaining spec-surface edges around engine-vs-wrapper responsibility, deterministic counterpart touch ordering, and explicit inheritance of §9.0 validation in the per-instruction procedures. The safety boundary remains the same: the per-accrual price-move and funding envelopes prevent one-envelope self-neutral insurance siphons by construction. The sweep-generation and consumption-threshold machinery remain UX and stress signals layered on top of that safety boundary.

The main deltas carried into rev6 are:

1. preserve the wrapper-supplied two-point admission pair `(admit_h_min, admit_h_max)`,
2. preserve sticky `admit_h_max` within one instruction so fresh reserve cannot be under-admitted,
3. preserve touch-time outstanding-reserve re-admission,
4. preserve the explicit `resolve_mode ∈ {Ordinary, Degenerate}` selector for `resolve_market`; value-detected branch selection is forbidden,
5. preserve the funding envelope (`cfg_max_accrual_dt_slots`, `cfg_max_abs_funding_e9_per_slot`) and the privileged degenerate recovery resolution branch,
6. preserve `last_fee_slot_i` as a persistent per-account checkpoint for wrapper-owned recurring fees,
7. define a canonical fee-sync helper that charges exactly once over `[last_fee_slot_i, fee_slot_anchor]`, advances `last_fee_slot_i`, and uses explicit saturating-to-`MAX_PROTOCOL_FEE_ABS` overflow semantics,
8. require new accounts to anchor `last_fee_slot_i` at their materialization slot so they do not inherit pre-creation fees,
9. require resolved-market recurring fee sync to anchor at `resolved_slot`, never after it,
10. make the same-epoch phantom-dust rules explicit: basis-replacement orphan remainder and same-epoch decay-to-zero each increment the relevant bound by exactly `1` q-unit,
11. make the scheduled-bucket warmup release rule explicit when the bucket empties, so no stale `sched_release_q` cursor survives on a non-empty bucket,
12. preserve immutable `cfg_max_price_move_bps_per_slot`,
13. preserve exact init-time solvency-envelope validation: `price_budget_bps + funding_budget_bps + cfg_liquidation_fee_bps ≤ cfg_maintenance_bps`,
14. preserve exact per-accrual price-move rejection before any K/F/price/slot mutation,
15. preserve the accrual dt envelope whenever live exposure can lose equity through price movement, even if funding is zero; zero-OI idle markets remain fast-forwardable,
16. add persistent `rr_cursor_position`, `sweep_generation`, and `price_move_consumed_bps_this_generation`,
17. add a wrapper-sized but engine-enforced consumption-threshold gate to `admit_fresh_reserve_h_lock`,
18. require `keeper_crank` to run a mandatory two-phase structure: keeper-priority liquidation followed by a deterministic round-robin structural sweep,
19. reset generation-scoped price-move consumption only on cursor wraparound, and
20. preserve rev4 behavior exactly for trusted or private wrappers when `admit_h_max_consumption_threshold_bps_opt = None` and `rr_window_size = 0`,
21. replace the `0` sentinel with an explicit optional threshold (`None` disables the gate; `Some(0)` is invalid),
22. add the no-accrual envelope bound to permissionless `reclaim_empty_account`, and
23. change generation-scoped price-move consumption from ceil to floor so sub-bps jitter does not spuriously trip the stress gate,
24. clarify that the public-wrapper prohibition on `(admit_h_min == 0, admit_h_max_consumption_threshold_bps_opt = None)` is wrapper-layer rather than an engine-side validation,
25. make the common §9.0 input validation explicit in each live per-instruction procedure that consumes the admission pair or optional threshold,
26. fix `execute_trade` counterpart touching to deterministic ascending storage-index order, including the pre-open dust/reset flush that observes that touched state,
27. add test coverage for the disabled-threshold immediate-release behavior and deterministic trade touch ordering, and
28. clarify that wrappers intending to disable the stress gate SHOULD use `None` explicitly rather than a pathologically large `Some(threshold)` that behaves like a quiet de-facto disable.

The engine core still keeps only:

- one **scheduled** reserve bucket plus one **pending** reserve bucket per live account,
- `PNL_matured_pos_tot`,
- the global trade haircut `g`,
- the matured-profit haircut `h`,
- the exact trade-open counterfactual approval metric `Eq_trade_open_raw_i`,
- capital, fee-debt, insurance, and recurring-fee-checkpoint accounting,
- lazy A/K/F settlement,
- liquidation and reset mechanics,
- resolved-market local reconciliation, shared positive-payout snapshot capture, and terminal close,
- a round-robin structural-sweep cursor plus one generation-scoped price-move-consumption accumulator.

The following policy inputs remain wrapper-owned and are **not** derived by the engine core:

- the live accrued instruction admission pair `(admit_h_min, admit_h_max)`,
- the per-instruction optional `admit_h_max_consumption_threshold_bps_opt` parameter (`None` disables the new gate; `Some(0)` is invalid),
- any optional wrapper-owned recurring account-fee rate or equivalent fee function,
- the funding rate applied to the elapsed live interval,
- the `rr_window_size` passed to `keeper_crank`,
- any public execution-price admissibility policy,
- any mark-EWMA or premium-funding model,
- oracle-account selection, oracle normalization, and wrapper-level rate limiting before the engine call.

The engine validates bounds and exactness requirements where applicable, but it does not derive those policies.

---

## 0. Security goals

The engine MUST provide the following properties.

1. **Protected principal for flat accounts:** an account with effective position `0` MUST NOT have its protected principal directly reduced by another account’s insolvency.
2. **Explicit open-position ADL eligibility:** accounts with open positions MAY be subject to deterministic protocol ADL if they are on the eligible opposing side of a bankrupt liquidation. ADL MUST operate through explicit protocol state, not hidden execution.
3. **Oracle-manipulation safety for extraction:** profits created by short-lived oracle distortion MUST NOT immediately dilute the matured-profit haircut denominator `h`, immediately become withdrawable principal, or immediately satisfy withdrawal or principal-conversion approval checks.
4. **Bounded trade reuse of positive PnL:** fresh positive PnL MAY support the generating account’s own risk-increasing trades only through the global trade haircut `g`. Aggregate positive PnL admitted through `g` MUST NOT exceed current `Residual`.
5. **No same-trade bootstrap from positive slippage:** a candidate trade’s own positive execution-slippage PnL MUST NOT be allowed to make that same trade pass a risk-increasing initial-margin check.
6. **No retroactive maturity inheritance:** fresh positive reserve added at slot `t` MUST NOT inherit time already elapsed on an older scheduled reserve bucket.
7. **No restart of older scheduled reserve:** adding new positive reserve to an account MUST NOT reset the scheduled bucket’s `sched_start_slot`, `sched_horizon`, `sched_anchor_q`, or already accrued maturity progress.
8. **Bounded warmup state:** each live account MUST use at most one scheduled reserve bucket and at most one pending reserve bucket.
9. **Conservative pending semantics:** the pending bucket MAY be more conservative than exact per-increment aging, but it MUST NEVER mature faster than its own stored horizon, and it MUST NEVER accelerate release of the older scheduled bucket.
10. **Profit-first haircuts:** when the system is undercollateralized, haircuts MUST apply to junior profit claims before any protected principal of flat accounts is impacted.
11. **Conservation:** the engine MUST NOT create withdrawable claims exceeding vault tokens, except for explicitly bounded rounding slack.
12. **Live-operation liveness:** on live markets, the engine MUST NOT require `OI == 0`, a global scan, a canonical account-order prefix, or manual admin recovery before a user can safely settle, deposit, withdraw, trade, liquidate, repay fee debt, reclaim, or make keeper progress.
13. **Resolved-close liveness split:** after a resolved account is locally reconciled, an account with `PNL_i <= 0` MUST be closable immediately; an account with `PNL_i > 0` MAY wait for global terminal-readiness and shared snapshot capture before payout.
14. **No zombie poisoning of the matured-profit haircut:** non-interacting accounts MUST NOT indefinitely pin the matured-profit haircut denominator `h` with fresh unwarmed PnL. Touched accounts MUST make warmup progress.
15. **Funding, mark, and ADL exactness under laziness:** any quantity whose correct value depends on the position held over an interval MUST be represented through A/K/F side indices or a formally equivalent event-segmented method. Integer rounding at settlement MUST NOT mint positive aggregate claims.
16. **Economically negligible ADL truncation before `DrainOnly`:** under the configured `ADL_ONE` and `MIN_A_SIDE`, same-epoch A-decay dust deferred into `phantom_dust_bound_*_q` MUST remain economically negligible before a side can remain live in `DrainOnly`.
17. **No hidden protocol MM:** the protocol MUST NOT secretly internalize user flow against an undisclosed residual inventory.
18. **Defined recovery from precision stress:** the engine MUST define deterministic recovery when side precision is exhausted. It MUST NOT rely on assertion failure, silent overflow, or permanent `DrainOnly` states.
19. **No sequential quantity dependency:** same-epoch account settlement MUST be fully local. It MAY depend on the account’s own stored basis and current global side state, but MUST NOT require a canonical-order prefix or global carry cursor.
20. **Protocol-fee neutrality:** explicit protocol fees MUST either be collected into `I` immediately or tracked as account-local fee debt up to the account’s collectible capital-plus-fee-debt limit. Any explicit fee amount beyond that collectible limit MUST be dropped rather than socialized through `h`, through `g`, or inflated into bankruptcy deficit `D`.
21. **Strict risk-reducing neutrality uses actual fee impact:** any “fee-neutral” strict risk-reducing comparison MUST add back the account’s **actual applied fee-equity impact**, not the nominal requested fee amount.
22. **Synthetic liquidation price integrity:** a synthetic liquidation close MUST execute at the current oracle mark with zero execution-price slippage. Any liquidation penalty MUST be represented only by explicit fee state.
23. **Loss seniority over engine-native protocol fees:** when a trade or a non-bankruptcy liquidation realizes trading losses for an account, those losses are senior to engine-native trade and liquidation fee collection from that same local capital state.
24. **Deterministic overflow handling:** any arithmetic condition that is not proven unreachable by the numeric bounds MUST have a deterministic fail-safe or bounded fallback path. Silent wrap, unchecked panic, and undefined truncation are forbidden.
25. **Finite-capacity liveness:** because account capacity is finite, the engine MUST provide permissionless dead-account reclamation or equivalent slot reuse so abandoned empty accounts and flat dust accounts below the live-balance floor cannot permanently exhaust capacity.
26. **Permissionless off-chain keeper compatibility:** candidate discovery MAY be performed entirely off chain. The engine MUST expose exact current-state shortlist processing and targeted per-account settle, liquidate, reclaim, or resolved-close paths so any permissionless keeper can make liquidation and reset progress without any required on-chain phase-1 scan.
27. **No pure-capital insurance draw without accrual:** pure capital-flow instructions (`deposit`, `deposit_fee_credits`, `top_up_insurance_fund`, `charge_account_fee`) that do not call `accrue_market_to` MUST NOT decrement `I` or record uninsured protocol loss.
28. **Configuration immutability within a market instance:** warmup bounds, admission bounds, trade-fee, margin, liquidation, insurance-floor, funding envelope, price-move envelope, and live-balance-floor parameters MUST remain fixed for the lifetime of a market instance unless a future revision defines an explicit safe update procedure.
29. **Scheduled-bucket exactness:** the active scheduled reserve bucket MUST mature according to its stored `sched_horizon` up to the required integer flooring and reserve-loss caps.
30. **Resolved-market close exactness:** resolved-market close MUST be defined through canonical helpers. It MUST NOT rely on direct zero-writes that bypass `C_tot`, `PNL_pos_tot`, `PNL_matured_pos_tot`, reserve state, fee-checkpoint state, or reset counters.
31. **Path-independent touched-account finalization:** flat auto-conversion and fee-debt sweep on live touched accounts MUST depend only on the post-live touched state and the shared conversion snapshot, not on whether the instruction was single-touch or multi-touch.
32. **No resolved payout race:** resolved accounts with positive claims MUST NOT be terminally paid out until stale-account reconciliation is complete across both sides and the shared resolved-payout snapshot is locked.
33. **Path-independent resolved positive payouts:** once stale-account reconciliation is complete and terminal payout becomes unlocked, all positive resolved payouts MUST use one shared resolved-payout snapshot so caller order cannot improve the payout ratio.
34. **Bounded resolved settlement price on the ordinary resolution path:** when `resolve_market` uses its ordinary self-synchronizing live-sync branch, the resolved settlement price MUST remain within an immutable deviation band of the trusted live-sync price supplied for that instruction. The privileged degenerate recovery branch may bypass this band and rely entirely on trusted settlement inputs.
35. **No permissionless haircut realization of flat released profit:** automatic flat conversion in live instructions MUST occur only at a whole snapshot (`h = 1`). Any lossy conversion of released profit under `h < 1` MUST be an explicit user action.
36. **No retroactive funding erasure at ordinary resolution:** in the ordinary self-synchronizing `resolve_market` path, the zero-funding settlement shift MUST only operate on market state already accrued through the resolution slot, so the settlement transition cannot erase elapsed live funding. The privileged degenerate recovery branch may intentionally skip omitted live accrual after `slot_last` and therefore must rely entirely on trusted settlement policy.
37. **No silent touched-set or admission-state truncation:** every account touched by live local touch and every account recorded in instruction-local admission state MUST either be tracked in the instruction context or the instruction MUST fail conservatively.
38. **No valid-price sentinel overloading:** no strictly positive price value may be used as an “uninitialized” sentinel for `P_last`, `fund_px_last`, or any other economically meaningful stored price field.
39. **Self-synchronizing resolution with a privileged degenerate-recovery escape hatch:** `resolve_market` MUST ordinarily synchronize live accrual to its resolution slot inside the same top-level instruction before applying the final zero-funding settlement shift. The same privileged instruction MAY instead take the explicit degenerate recovery branch described in §9.8 when the deployment needs to avoid additional live-state shift — for example because the accrual envelope has already been exceeded or cumulative `K` or `F` headroom is tight.
40. **Bounded-cost exact arithmetic:** the specification MUST permit exact implementations of scheduled warmup release and funding accrual without runtime work proportional to elapsed slots and without relying on narrow intermediate products that can overflow before the exact quotient is taken.
41. **Runtime-aware deployment constraints:** on constrained runtimes, deployments MUST choose batch sizes, wrapper-side deposit minimums, funding envelopes, and wrapper composition so exact wide arithmetic, materialized-account capacity, and transaction-size limits do not create avoidable operational deadlocks.
42. **Resolution must not depend on cumulative-K absorption of the final settlement mark:** the final settlement price shift is carried as separate resolved terminal K deltas rather than added into persistent live `K_side`.
43. **Resolved reconciliation must not deadlock on live-only claim caps:** once the market is resolved, local reconciliation MAY exceed live-market positive-PnL caps so long as all persistent values remain representable and terminal payout remains snapshot-capped.
44. **No live positive-PnL bypass of admission:** every positive reserve-creating event on a live market MUST pass through the two-point admission rule; there is no unconditional live `ImmediateRelease` path.
45. **No same-instruction under-admission:** within one top-level instruction, once an account requires the slow admitted horizon `admit_h_max` for any fresh positive increment, all later fresh positive increments on that account in that instruction MUST also use `admit_h_max`. An earlier newest pending increment MAY be conservatively lifted to `admit_h_max` if it merges with a later slower-admitted increment; under-admission is forbidden.
46. **Touch-time reserve acceleration is monotone:** touching a live account may only accelerate existing reserve by removing buckets when the current state safely admits immediate release; it MUST never extend or re-lock reserve.
47. **No inherited recurring fees for new accounts:** a newly materialized account MUST anchor its recurring-fee checkpoint at its materialization slot and MUST NOT be charged for earlier time.
48. **Exact touched-account recurring-fee liveness:** if a deployment enables wrapper-owned recurring account fees, a touched account MUST be fee-syncable from `last_fee_slot_i` to the relevant slot anchor without a global scan.
49. **No post-resolution recurring-fee accrual:** recurring account fees, if enabled by the wrapper, accrue only over live time and MUST NOT be charged past `resolved_slot`.
50. **Resolved payout snapshot stability under late fee sync:** fee sync or fee forgiveness performed after the shared resolved payout snapshot is captured MUST NOT invalidate that snapshot’s correctness. The snapshot is over `Residual = V - (C_tot + I)` and pure `C -> I` reclassification must preserve it.
51. **No implicit degenerate-mode selection:** the ordinary vs degenerate `resolve_market` branch MUST be chosen only from an explicit trusted wrapper mode input. Equality of economic values such as `live_oracle_price == P_last` or `funding_rate_e9_per_slot == 0` MUST NOT by itself force the degenerate branch.
52. **No self-neutral insurance siphon via oracle moves:** between any two successive authoritative `accrue_market_to` calls, the adverse equity drain on any live exposed position that was maintenance-healthy at the earlier call MUST be strictly less than that position’s maintenance buffer, net of liquidation cost and worst-case funding drain over the same interval. This is enforced by construction: §1.4 requires `cfg_max_price_move_bps_per_slot * cfg_max_accrual_dt_slots + funding_drain_bps_per_envelope_at_max_rate + cfg_liquidation_fee_bps ≤ cfg_maintenance_bps`, and §5.5 rejects any price-moving live-exposure `accrue_market_to` whose `dt` exceeds the configured envelope or whose proposed `|ΔP| / P_last` exceeds the per-slot cap scaled by `dt`. A compromised oracle or adversarial price sequence therefore cannot drive a maintenance-healthy position through zero equity within a single accrual envelope; the account is either liquidatable on the next crank with nonnegative equity after liquidation cost, or the accrual itself is rejected and the market must progress through explicit recovery or `resolve_market(Degenerate)`.
53. **Forgery-resistant sweep-generation signal with engine-enforced stress-scaled admission.** The engine tracks a round-robin cursor that walks the materialized-account index space during every `keeper_crank` call. The cursor advances deterministically by a keeper-supplied window size, and `sweep_generation` increments exactly once per wraparound past `cfg_max_accounts`. The engine also tracks cumulative price-move consumption since the last generation advance, and `admit_fresh_reserve_h_lock` forces `admit_h_max` when consumption exceeds an optional wrapper-supplied threshold. This composes with the existing residual-scarcity check, which already forces `admit_h_max` when the post-impact matured haircut would fall below `1`: together they block fast-lane admission both when the market is already underwater and when recent price movement suggests reconciliation is incomplete. The per-envelope price-move cap of goal 52 remains the construction-level safety property; `sweep_generation` and consumption tracking are stress and UX signals. `sweep_generation` is tamper-resistant because the only way to advance it is to run `keeper_crank`, which always executes its mandatory round-robin phase and touches every materialized account found in the traversed window. The engine enforces the stress gate iff a threshold is supplied; the public-wrapper prohibition on `(admit_h_min == 0, admit_h_max_consumption_threshold_bps_opt = None)` is wrapper-layer (§12.21), not an engine-side validation. With the gate disabled the engine still preserves all invariants and the goal-52 safety boundary, but the immediate-release cascade behavior witnessed by §11 property 107 returns.

**Atomic execution model:** every top-level external instruction defined in §9 MUST be atomic. If any required precondition, checked-arithmetic guard, or conservative-failure condition fails, the instruction MUST roll back all state mutations performed since that instruction began.

---

## 1. Types, units, scaling, bounds, and exact arithmetic

### 1.1 Amounts

- `u128` unsigned amounts are denominated in quote-token atomic units, positive-PnL aggregates, open interest, fixed-point position magnitudes, and bounded fee amounts.
- `i128` signed amounts represent realized PnL, K-space liabilities, funding-index snapshots, and fee-credit balances.
- `wide_signed` means any transient exact signed intermediate domain wider than `i128` (for example `i256`) or an equivalent exact comparison-preserving construction.
- `wide_unsigned` means any transient exact unsigned intermediate domain wider than `u128` (for example `u256`) or an equivalent exact comparison-preserving construction.
- All persistent state MUST fit natively into 128-bit boundaries. Emulated wide integers are permitted only within transient intermediate math steps.

### 1.2 Prices and internal positions

- `POS_SCALE = 1_000_000`.
- `price: u64` is quote-token atomic units per `1` base.
- Every external price input, including `oracle_price`, `exec_price`, `live_oracle_price`, `resolved_price`, and any stored funding-price sample, MUST satisfy `0 < price <= MAX_ORACLE_PRICE`.
- The engine stores position bases as signed fixed-point base quantities:
  - `basis_pos_q_i: i128`, units `(base * POS_SCALE)`.
- Oracle notional:
  - `Notional_i = mul_div_floor_u128(abs(effective_pos_q(i)), oracle_price, POS_SCALE)`.
- Trade fees use executed size:
  - `trade_notional = mul_div_floor_u128(size_q, exec_price, POS_SCALE)`.

### 1.3 A/K/F scales

- `ADL_ONE = 1_000_000_000_000_000`.
- `A_side` is dimensionless and scaled by `ADL_ONE`.
- `K_side` has units `(ADL scale) * (quote atomic units per 1 base)`.
- `FUNDING_DEN = 1_000_000_000`.
- `F_side_num` has units `(ADL scale) * (quote atomic units per 1 base) * FUNDING_DEN`.

### 1.4 Normative bounds and configuration

Global hard bounds:

- `MAX_VAULT_TVL = 10_000_000_000_000_000`
- `MAX_ORACLE_PRICE = 1_000_000_000_000`
- `MAX_POSITION_ABS_Q = 100_000_000_000_000`
- `MAX_TRADE_SIZE_Q = MAX_POSITION_ABS_Q`
- `MAX_OI_SIDE_Q = 100_000_000_000_000`
- `MAX_ACCOUNT_NOTIONAL = 100_000_000_000_000_000_000`
- `MAX_PROTOCOL_FEE_ABS = 1_000_000_000_000_000_000_000_000_000_000_000_000`
- `GLOBAL_MAX_ABS_FUNDING_E9_PER_SLOT = 10_000`
- `MAX_TRADING_FEE_BPS = 10_000`
- `MAX_INITIAL_BPS = 10_000`
- `MAX_MAINTENANCE_BPS = 10_000`
- `MAX_LIQUIDATION_FEE_BPS = 10_000`
- `cfg_max_accounts` is per-market runtime configuration (see §1.4)
- `MAX_ACTIVE_POSITIONS_PER_SIDE` MUST be finite and MUST NOT exceed `cfg_max_accounts`
- `MAX_ACCOUNT_POSITIVE_PNL_LIVE = 100_000_000_000_000_000_000_000_000_000_000`
- `MAX_PNL_POS_TOT_LIVE = 100_000_000_000_000_000_000_000_000_000_000_000_000`
- `MIN_A_SIDE = 100_000_000_000_000`
- `MAX_WARMUP_SLOTS = 18_446_744_073_709_551_615`
- `MAX_RESOLVE_PRICE_DEVIATION_BPS = 10_000`

Immutable per-market configuration:

- `cfg_h_min`
- `cfg_h_max`
- `cfg_maintenance_bps`
- `cfg_initial_bps`
- `cfg_trading_fee_bps`
- `cfg_liquidation_fee_bps`
- `cfg_liquidation_fee_cap`
- `cfg_min_liquidation_abs`
- `cfg_min_nonzero_mm_req`
- `cfg_min_nonzero_im_req`
- `cfg_resolve_price_deviation_bps`
- `cfg_max_active_positions_per_side`
- `cfg_max_accrual_dt_slots`
- `cfg_max_abs_funding_e9_per_slot`
- `cfg_max_price_move_bps_per_slot`
- `cfg_min_funding_lifetime_slots`

Configured values MUST satisfy:

- `0 < cfg_min_nonzero_mm_req < cfg_min_nonzero_im_req` (upper bound on `cfg_min_nonzero_im_req` is wrapper policy — the engine no longer tracks a minimum deposit)
- `0 <= cfg_maintenance_bps <= cfg_initial_bps <= MAX_INITIAL_BPS`
- `0 <= cfg_h_min <= cfg_h_max <= MAX_WARMUP_SLOTS`
- live instruction admission pairs MUST satisfy `0 <= admit_h_min <= admit_h_max <= cfg_h_max`
- if `admit_h_min > 0`, then `admit_h_min >= cfg_h_min`
- for live instructions that may create fresh reserve, `admit_h_max > 0` and `admit_h_max >= cfg_h_min`
- `0 <= cfg_resolve_price_deviation_bps <= MAX_RESOLVE_PRICE_DEVIATION_BPS`
- `0 <= cfg_min_liquidation_abs <= cfg_liquidation_fee_cap <= MAX_PROTOCOL_FEE_ABS`
- `0 < cfg_max_active_positions_per_side <= MAX_ACTIVE_POSITIONS_PER_SIDE`
- `0 < cfg_max_accrual_dt_slots <= MAX_WARMUP_SLOTS`
- `0 <= cfg_max_abs_funding_e9_per_slot <= GLOBAL_MAX_ABS_FUNDING_E9_PER_SLOT`
- `0 < cfg_max_price_move_bps_per_slot`
- exact init-time funding-envelope validation:
  - `ADL_ONE * MAX_ORACLE_PRICE * cfg_max_abs_funding_e9_per_slot * cfg_max_accrual_dt_slots <= i128::MAX` (per-call)
  - `cfg_min_funding_lifetime_slots >= cfg_max_accrual_dt_slots`
  - `ADL_ONE * MAX_ORACLE_PRICE * cfg_max_abs_funding_e9_per_slot * cfg_min_funding_lifetime_slots <= i128::MAX` (cumulative lifetime floor)
  - both validations MUST be performed in an exact wide signed domain of at least 256 bits, or a formally equivalent exact method
- exact init-time price/funding/liquidation solvency-envelope validation:
  - `price_budget_bps = cfg_max_price_move_bps_per_slot * cfg_max_accrual_dt_slots`
  - `funding_budget_bps = floor(cfg_max_abs_funding_e9_per_slot * cfg_max_accrual_dt_slots * 10_000 / FUNDING_DEN)`
  - require `price_budget_bps + funding_budget_bps + cfg_liquidation_fee_bps <= cfg_maintenance_bps`
  - this validation MUST be performed in an exact wide unsigned or signed domain of at least 256 bits, or a formally equivalent exact method
  - this inequality is the construction-level invariant backing §0 goal 52: a maintenance-healthy position cannot be driven through zero equity within one accrual envelope at any adversarially chosen price path, funding rate, and subsequent liquidation. Deployments that want looser price caps MUST raise `cfg_maintenance_bps`, tighten `cfg_max_accrual_dt_slots`, reduce `cfg_max_abs_funding_e9_per_slot`, or lower `cfg_liquidation_fee_bps` correspondingly.

Operational guidance and horizon examples for `cfg_min_funding_lifetime_slots` are in §13.

If the deployment also defines a stale-market resolution delay `permissionless_resolve_stale_slots` and expects permissionless resolution to remain callable after that delay, then initialization MUST additionally require:

- `permissionless_resolve_stale_slots <= cfg_max_accrual_dt_slots`

Deployments that rely only on privileged degenerate recovery resolution MAY omit `permissionless_resolve_stale_slots` entirely.

### 1.5 Trusted time and oracle requirements

- `now_slot` in every top-level instruction MUST come from trusted runtime slot metadata or an equivalent trusted source.
- `oracle_price` inputs MUST come from validated configured oracle feeds or trusted privileged settlement sources, depending on the instruction’s trust boundary.
- Any helper or instruction that accepts `now_slot` MUST require `now_slot >= current_slot`.
- Any call to `accrue_market_to(now_slot, oracle_price, funding_rate_e9_per_slot)` MUST require `now_slot >= slot_last`.
- `current_slot` and `slot_last` MUST be monotonically nondecreasing.
- The engine MUST NOT overload any strictly positive price value as an uninitialized sentinel for `P_last`, `fund_px_last`, or any equivalent stored price field.
- Any recurring-fee sync anchor `fee_slot_anchor` MUST satisfy:
  - on live markets: `last_fee_slot_i <= fee_slot_anchor <= current_slot`
  - on resolved markets: `last_fee_slot_i <= fee_slot_anchor <= resolved_slot`

The accrual envelope applies to any live interval that can create live equity drain:

- Define `funding_active = funding_rate_e9_per_slot != 0 && OI_eff_long != 0 && OI_eff_short != 0 && fund_px_last > 0`.
- Define `price_move_active = P_last > 0 && oracle_price != P_last && (OI_eff_long != 0 || OI_eff_short != 0)`.
- Every live accrual with `funding_active || price_move_active` MUST require `dt = now_slot - slot_last <= cfg_max_accrual_dt_slots`.
- When both branches are inactive — for example zero-OI idle markets, or open-interest markets with zero funding and no price movement — no K/F equity-drain delta is applied, so `dt` is unbounded and the market can fast-forward safely.

This refinement is load-bearing for §0 goal 52. If open interest exists and the oracle price moves, zero funding is not enough to bypass the envelope: price movement alone can drain equity, so the max-accrual dt bound MUST apply.

### 1.6 Required exact helpers

Implementations MUST provide exact checked helpers for at least:

- checked `add`, `sub`, and `mul` on `u64`, `u128`, and `i128`,
- checked cast helpers,
- exact conservative signed floor division,
- exact floor and ceil multiply-divide helpers,
- `fee_debt_u128_checked(fee_credits_i)`,
- `fee_credit_headroom_u128_checked(fee_credits_i)`,
- `wide_signed_mul_div_floor_from_kf_pair(abs_basis, k_then, k_now_exact, f_then, f_now_exact, den)`, where `k_then` and `f_then` are persistent i128 snapshots and `k_now_exact` and `f_now_exact` may be either persistent i128 values or exact wide signed values,
- exact comparison helper for the price-move cap: `abs_delta_price * 10_000 <= cfg_max_price_move_bps_per_slot * dt * P_last` without intermediate overflow.

The canonical law for `wide_signed_mul_div_floor_from_kf_pair` is:

`wide_signed_mul_div_floor_from_kf_pair(abs_basis, k_then, k_now_exact, f_then, f_now_exact, den)`  
`= floor( abs_basis * ( ((k_now_exact - k_then) * FUNDING_DEN) + (f_now_exact - f_then) ) / (den * FUNDING_DEN) )`

with floor toward negative infinity in the exact widened signed domain. The helper MUST use at least exact 256-bit signed intermediates, or a formally equivalent exact method. Implementations MUST NOT add `ΔK` and `ΔF` directly without this `FUNDING_DEN` un-scaling.

### 1.7 Arithmetic requirements

The engine MUST satisfy all of the following.

1. Every product involving `A_side`, `K_side`, `F_side_num`, `k_snap_i`, `f_snap_i`, `basis_pos_q_i`, `effective_pos_q(i)`, `price`, the raw funding numerator `fund_px_0 * funding_rate_e9_per_slot * dt`, trade-haircut numerators, trade-open counterfactual positive-aggregate numerators, scheduled-bucket release numerators, or ADL deltas MUST use checked arithmetic or an exact checked multiply-divide helper that is mathematically equivalent to the full-width product.
2. `accrue_market_to` MUST apply the exact total funding delta over the full interval `dt`. Implementations MAY use internal chunking only if it is exactly equivalent to the total-delta law and does not require an unbounded runtime loop proportional to `dt`.
3. The conservation check `V >= C_tot + I` and any `Residual` computation MUST use checked addition for `C_tot + I`.
4. Signed division with positive denominator MUST use exact conservative floor division.
5. Exact multiply-divide helpers MUST return the exact quotient even when the exact product exceeds native `u128`, provided the final quotient fits.
6. `PendingWarmupTot = PNL_pos_tot - PNL_matured_pos_tot` MUST use checked subtraction.
7. Haircut paths `floor(ReleasedPos_i * h_num / h_den)`, `floor(PosPNL_i * g_num / g_den)`, and the exact candidate-open trade-haircut path of §3.5 MUST use exact multiply-divide helpers.
8. Funding transfer MUST use the same exact total `fund_num_total = fund_px_0 * funding_rate_e9_per_slot * dt` value for both sides’ `F_side_num` deltas, with opposite signs. The engine MUST NOT introduce per-step or per-chunk rounding inside `accrue_market_to`.
9. `fund_num_total`, each `A_side * fund_num_total` product, and each live mark-to-market `A_side * (oracle_price - P_last)` product MUST be computed in an exact wide signed domain of at least 256 bits, or a formally equivalent exact method. `K_side` and `F_side_num` are cumulative across epochs. Implementations MUST use checked arithmetic and fail conservatively on persistent `i128` overflow.
10. Same-epoch or epoch-mismatch settlement MUST combine `K_side` and `F_side_num` through the exact helper `wide_signed_mul_div_floor_from_kf_pair`. The helper MUST accept exact wide signed terminal values such as `K_epoch_start_side + resolved_k_terminal_delta_side`, even when that terminal sum is not itself persisted as a live `K_side`.
11. The ADL quote-deficit path MUST compute `delta_K_abs = ceil(D_rem * A_old * POS_SCALE / OI_before)` using exact wide arithmetic.
12. If a K-index delta magnitude is representable but `K_opp + delta_K_exact` overflows `i128`, the engine MUST route `D_rem` through `record_uninsured_protocol_loss` while still continuing quantity socialization.
13. `PNL_i` MUST be maintained in `[i128::MIN + 1, i128::MAX]`, and `fee_credits_i` in `[i128::MIN + 1, 0]`.
14. Every decrement of `stored_pos_count_*`, `stale_account_count_*`, or `phantom_dust_bound_*_q` MUST use checked subtraction.
15. Every increment of `stored_pos_count_*`, `phantom_dust_bound_*_q`, `epoch_side`, `materialized_account_count`, `neg_pnl_account_count`, `C_tot`, `PNL_pos_tot`, `PNL_matured_pos_tot`, `V`, or `I` MUST use checked addition and MUST enforce the relevant bound.
16. `trade_notional <= MAX_ACCOUNT_NOTIONAL` MUST be enforced before charging trade fees.
17. Any out-of-range price input, invalid oracle read, invalid live admission pair, invalid `funding_rate_e9_per_slot`, invalid degenerate-resolution inputs, invalid recurring-fee anchor, or non-monotonic slot input MUST fail conservatively before state mutation.
18. `charge_fee_to_insurance` MUST cap its applied fee at the account’s exact collectible capital-plus-fee-debt headroom. It MUST never set `fee_credits_i < -(i128::MAX)`.
19. Any direct fee-credit repayment path MUST cap its applied amount at the exact current `FeeDebt_i`. It MUST never set `fee_credits_i > 0`.
20. Any direct insurance top-up or direct fee-credit repayment path that increases `V` or `I` MUST use checked addition and MUST enforce `MAX_VAULT_TVL`.
21. Scheduled- and pending-bucket mutations MUST preserve the invariants of §2.1 and MUST use checked arithmetic.
22. The exact counterfactual trade-open computation MUST recompute the account’s positive-PnL contribution and the global positive-PnL aggregate with the candidate trade’s own positive slippage gain removed.
23. Any wrapper-owned fee amount routed through the canonical helper MUST satisfy `fee_abs <= MAX_PROTOCOL_FEE_ABS`.
24. Fresh reserve MUST NOT be merged into an older scheduled bucket unless that bucket was itself created in the current slot, has the same admitted horizon, and has `sched_release_q == 0`.
25. Pending-bucket horizon updates MUST be monotone nondecreasing with `pending_horizon_i = max(pending_horizon_i, admitted_h_eff)` whenever new reserve is merged into an existing pending bucket. This monotone re-horizoning is intentionally conservative for the newest pending bucket and MUST NEVER affect the scheduled bucket.
26. If a live positive increase occurs, the engine MUST admit it through `admit_fresh_reserve_h_lock`; the only path that may immediately release positive PnL without live admission is `ImmediateReleaseResolvedOnly` on resolved markets.
27. Funding exactness MUST NOT depend on a bare global remainder with no per-account snapshot. Any retained fractional precision across calls MUST be represented through `F_side_num` and `f_snap_i`.
28. Any strict risk-reducing fee-neutral comparison MUST add back `fee_equity_impact_i`, not nominal fee.
29. `max_safe_flat_conversion_released` MUST use at least 256-bit exact intermediates, or a formally equivalent exact wide comparison, whenever `E_before * h_den` would exceed native `u128`.
30. Any helper that computes bucket maturity from `elapsed / sched_horizon` MUST clamp `elapsed` at `sched_horizon` before invoking an exact multiply-divide helper whose unclamped final quotient could exceed `u128` even though the clamped economic answer is `sched_anchor_q`.
31. Any helper precondition reachable from a top-level instruction MUST fail conservatively rather than panic or assert on caller-controlled inputs or mutable market state.
32. `phantom_dust_bound_long_q` and `phantom_dust_bound_short_q` are bounded by `u128` representability; any attempted overflow is a conservative failure.
33. Even after `market_mode == Resolved`, aggregate persistent quantities stored as `u128` — including `PNL_pos_tot` and `PNL_matured_pos_tot` — MUST remain representable in `u128`; any reconciliation or terminal-close path that would overflow them MUST fail conservatively rather than wrap.
34. All touched-account and instruction-local admission-state structures in `ctx` MUST be provisioned to hold the maximum number of distinct accounts any top-level instruction in this revision can touch or admit; if capacity would be exceeded, the instruction MUST fail conservatively.
35. `last_fee_slot_i` MUST be initialized, advanced, and reset only through canonical helper paths. A new account MUST start at its materialization slot, and a freed slot MUST return to `0`.
36. Recurring-fee sync to a resolved account MUST use `fee_slot_anchor = resolved_slot`, never `current_slot` if `current_slot > resolved_slot`.
37. A late recurring-fee sync after the resolved payout snapshot is captured MUST preserve `Residual = V - (C_tot + I)` except for intentionally dropped uncollectible fee tails, which are conservatively ignored rather than socialized.
38. `sync_account_fee_to_slot` MUST interpret `fee_rate_per_slot * dt` with explicit saturating-to-`MAX_PROTOCOL_FEE_ABS` semantics. It MUST either compute the product in an exact widened domain of at least 256 bits and then cap, or use an exactly equivalent branch on `fee_rate_per_slot > floor(MAX_PROTOCOL_FEE_ABS / dt)` for `dt > 0`. The helper MUST NOT fail solely because the uncapped raw fee product exceeds native `u128`.
39. `accrue_market_to` MUST enforce the per-accrual price-move cap exactly. For any call with `P_last > 0`, `dt > 0`, and live exposure (`OI_eff_long != 0 || OI_eff_short != 0`), it MUST require `dt <= cfg_max_accrual_dt_slots` if `oracle_price != P_last`, and it MUST require `abs(oracle_price - P_last) * 10_000 <= cfg_max_price_move_bps_per_slot * dt * P_last`, using checked wide arithmetic. The product on the right-hand side can exceed native `u128` at bounds; implementations MUST use at least 256-bit signed or unsigned intermediates, or a formally equivalent exact method. The check MUST fire before any `K_side`, `F_side_num`, `P_last`, `fund_px_last`, or `slot_last` mutation and MUST be reachable on every live instruction that advances `slot_last`.

---

## 2. State model

### 2.1 Account state

For each materialized account `i`, the engine stores at least:

- `C_i: u128` — protected principal
- `PNL_i: i128` — realized PnL claim
- `R_i: u128` — total reserved positive PnL, with `0 <= R_i <= max(PNL_i, 0)`
- `basis_pos_q_i: i128`
- `a_basis_i: u128`
- `k_snap_i: i128`
- `f_snap_i: i128`
- `epoch_snap_i: u64`
- `fee_credits_i: i128`
- `last_fee_slot_i: u64` — per-account recurring-fee checkpoint

Each live account additionally stores at most two reserve segments.

**Scheduled reserve bucket** (older bucket, matures linearly):

- `sched_present_i: bool`
- `sched_remaining_q_i: u128`
- `sched_anchor_q_i: u128`
- `sched_start_slot_i: u64`
- `sched_horizon_i: u64`
- `sched_release_q_i: u128`

**Pending reserve bucket** (newest bucket, does not mature while pending):

- `pending_present_i: bool`
- `pending_remaining_q_i: u128`
- `pending_horizon_i: u64`

Derived local quantities on a touched state:

- `PosPNL_i = max(PNL_i, 0)`
- if `market_mode == Live`, `ReleasedPos_i = PosPNL_i - R_i`
- if `market_mode == Resolved`, `ReleasedPos_i = PosPNL_i`
- `FeeDebt_i = fee_debt_u128_checked(fee_credits_i)`

Reserve invariants on live markets:

- `R_i = (sched_remaining_q_i if sched_present_i else 0) + (pending_remaining_q_i if pending_present_i else 0)`
- if `sched_present_i`:
  - `0 < sched_anchor_q_i`
  - `0 < sched_remaining_q_i <= sched_anchor_q_i`
  - `cfg_h_min <= sched_horizon_i <= cfg_h_max`
  - `0 <= sched_release_q_i <= sched_anchor_q_i`
- if `pending_present_i`:
  - `0 < pending_remaining_q_i`
  - `cfg_h_min <= pending_horizon_i <= cfg_h_max`
- the pending bucket is always economically newer than the scheduled bucket
- if `R_i == 0`, both buckets MUST be absent
- if `sched_present_i == false`, the pending bucket MAY still be present
- the pending bucket MUST NEVER auto-mature while pending
- when promoted, the pending bucket becomes the scheduled bucket with:
  - `sched_remaining_q = pending_remaining_q`
  - `sched_anchor_q = pending_remaining_q`
  - `sched_start_slot = current_slot`
  - `sched_horizon = pending_horizon`
  - `sched_release_q = 0`
- if `market_mode == Resolved`, reserve storage is economically inert and MUST be cleared by `prepare_account_for_resolved_touch(i)` before any resolved-account touch mutates `PNL_i`

Fee-credit and fee-slot bounds:

- `fee_credits_i` MUST be initialized to `0`
- the engine MUST maintain `-(i128::MAX) <= fee_credits_i <= 0`
- `fee_credits_i == i128::MIN` is forbidden
- if `market_mode == Live`, `last_fee_slot_i <= current_slot`
- if `market_mode == Resolved`, `last_fee_slot_i <= resolved_slot`
- `last_fee_slot_i` MUST be set to the account’s materialization slot on creation
- on free-slot reset, `last_fee_slot_i` MUST be cleared to `0`

#### 2.1.1 Wrapper-owned annotation fields (non-normative)

An engine implementation MAY carry additional per-account fields used by the deployment wrapper for its own bookkeeping — typical examples include an owner pubkey, an account-kind tag (user vs LP), a matching-engine program id, and a matching-engine context id. These fields are **wrapper-owned opaque annotation**. The engine MUST:

- store and canonicalize them through its normal materialization / reset / init paths so they do not leak stale data across slot reuse;
- **never** read them to decide any spec-normative behavior (margin health, liquidation eligibility, fee routing, reserve admission, accrual, resolution, reset lifecycle, conservation, authorization, or any other property enumerated in §0);
- treat them as inert payload on every engine-level path.

Authorization is a **wrapper responsibility**, not an engine invariant. Because these fields carry no engine-level semantics, they are outside the normative scope of this document.

### 2.2 Global engine state

The engine stores at least:

- `V: u128`
- `I: u128`
- `current_slot: u64`
- `P_last: u64`
- `slot_last: u64`
- `fund_px_last: u64`
- `A_long: u128`
- `A_short: u128`
- `K_long: i128`
- `K_short: i128`
- `F_long_num: i128`
- `F_short_num: i128`
- `epoch_long: u64`
- `epoch_short: u64`
- `K_epoch_start_long: i128`
- `K_epoch_start_short: i128`
- `F_epoch_start_long_num: i128`
- `F_epoch_start_short_num: i128`
- `OI_eff_long: u128`
- `OI_eff_short: u128`
- `mode_long ∈ {Normal, DrainOnly, ResetPending}`
- `mode_short ∈ {Normal, DrainOnly, ResetPending}`
- `stored_pos_count_long: u64`
- `stored_pos_count_short: u64`
- `stale_account_count_long: u64`
- `stale_account_count_short: u64`
- `phantom_dust_bound_long_q: u128`
- `phantom_dust_bound_short_q: u128`
- `materialized_account_count: u64`
- `neg_pnl_account_count: u64`
- `rr_cursor_position: u64`
- `sweep_generation: u64`
- `price_move_consumed_bps_this_generation: u128`
- `C_tot: u128`
- `PNL_pos_tot: u128`
- `PNL_matured_pos_tot: u128`

Immutable per-market configuration fields from §1.4 are stored in engine state and are part of the market instance.

Resolved-market state:

- `market_mode ∈ {Live, Resolved}`
- `resolved_price: u64`
- `resolved_live_price: u64`
- `resolved_slot: u64`
- `resolved_k_long_terminal_delta: i128`
- `resolved_k_short_terminal_delta: i128`
- `resolved_payout_snapshot_ready: bool`
- `resolved_payout_h_num: u128`
- `resolved_payout_h_den: u128`

Derived global quantity:

- `PendingWarmupTot = PNL_pos_tot - PNL_matured_pos_tot`

Global invariants:

- `C_tot <= V <= MAX_VAULT_TVL`
- `I <= V`
- `0 <= neg_pnl_account_count <= materialized_account_count <= cfg_max_accounts`
- `0 <= rr_cursor_position < cfg_max_accounts`
- `F_long_num` and `F_short_num` MUST remain representable as `i128`
- if `market_mode == Live`:
  - `PNL_matured_pos_tot <= PNL_pos_tot <= MAX_PNL_POS_TOT_LIVE`
  - `resolved_price == 0`
  - `resolved_live_price == 0`
  - `resolved_k_long_terminal_delta == 0`
  - `resolved_k_short_terminal_delta == 0`
- if `market_mode == Resolved`:
  - `resolved_price > 0`
  - `resolved_live_price > 0`
  - `PNL_matured_pos_tot <= PNL_pos_tot`
  - `resolved_k_long_terminal_delta` and `resolved_k_short_terminal_delta` are representable as `i128`
- if `resolved_payout_snapshot_ready == false`, then `resolved_payout_h_num == 0` and `resolved_payout_h_den == 0`
- if `resolved_payout_snapshot_ready == true`, then `resolved_payout_h_num <= resolved_payout_h_den`

### 2.3 Instruction context

Every top-level live instruction that uses the standard lifecycle MUST initialize a fresh ephemeral context `ctx` with at least:

- `pending_reset_long: bool`
- `pending_reset_short: bool`
- `admit_h_min_shared: u64`
- `admit_h_max_shared: u64`
- `admit_h_max_consumption_threshold_bps_opt_shared: Option<u128>`
- `touched_accounts[]` — deduplicated touched storage indices
- `h_max_sticky_accounts[]` — per-instruction set of storage indices for which `admit_h_max` has already been required in the current instruction

Capacity rules:

- `ctx.touched_accounts[]` capacity MUST be at least the deployment’s maximum allowed number of distinct touches in any single top-level instruction.
- For `keeper_crank`, the wrapper / runtime configuration MUST ensure `max_revalidations + rr_window_size` does not exceed that touched-account capacity.
- `ctx.h_max_sticky_accounts[]` capacity MUST be at least the deployment’s maximum allowed number of distinct accounts any single top-level instruction can both touch and create fresh reserve for.
- Implementations on constrained runtimes MAY choose capacities far smaller than `cfg_max_accounts`; in that case any instruction whose touched set would exceed capacity MUST fail conservatively before partial mutation.
- Implementations MAY choose to size both structures equally if that is operationally convenient.

### 2.4 Configuration immutability

No external instruction in this revision may change:

- `cfg_h_min`
- `cfg_h_max`
- `cfg_maintenance_bps`
- `cfg_initial_bps`
- `cfg_trading_fee_bps`
- `cfg_liquidation_fee_bps`
- `cfg_liquidation_fee_cap`
- `cfg_min_liquidation_abs`
- `cfg_min_nonzero_mm_req`
- `cfg_min_nonzero_im_req`
- `cfg_resolve_price_deviation_bps`
- `cfg_max_active_positions_per_side`
- `cfg_max_accrual_dt_slots`
- `cfg_max_abs_funding_e9_per_slot`
- `cfg_max_price_move_bps_per_slot`

### 2.5 Materialized-account capacity

The engine MUST track the number of currently materialized account slots. That count MUST NOT exceed `cfg_max_accounts`.

A missing account is one whose slot is not currently materialized. Missing accounts MUST NOT be auto-materialized by `settle_account`, `withdraw`, `execute_trade`, `close_account`, `liquidate`, `resolve_market`, `force_close_resolved`, or `keeper_crank`.

Only the following path MAY materialize a missing account:

- `deposit(i, amount, now_slot)` with `amount > 0`. The engine does not enforce a deposit minimum beyond "non-zero" — any higher floor is wrapper policy.

### 2.6 Canonical zero-position defaults

The canonical zero-position account defaults are:

- `basis_pos_q_i = 0`
- `a_basis_i = ADL_ONE`
- `k_snap_i = 0`
- `f_snap_i = 0`
- `epoch_snap_i = 0`

### 2.7 Account materialization

`materialize_account(i, materialize_slot)` MAY succeed only if the account is currently missing and materialized-account capacity remains below `cfg_max_accounts`.

On success, it MUST:

- increment `materialized_account_count`
- leave `neg_pnl_account_count` unchanged because the new account starts with `PNL_i = 0`
- set `C_i = 0`
- set `PNL_i = 0`
- set `R_i = 0`
- set canonical zero-position defaults
- set `fee_credits_i = 0`
- set `last_fee_slot_i = materialize_slot`
- leave both reserve buckets absent

### 2.8 Permissionless empty- or flat-dust-account reclamation

The engine MUST provide a permissionless reclamation path `reclaim_empty_account(i, now_slot)`.

It MAY succeed only if all of the following hold:

- account `i` is materialized
- trusted `now_slot >= current_slot`
- `C_i == 0`
- `PNL_i == 0`
- `R_i == 0`
- both reserve buckets are absent
- `basis_pos_q_i == 0`
- `fee_credits_i <= 0`

Wrappers that want to recycle accounts with residual dust capital MUST drain that capital first before calling reclaim. Dust-threshold policy is wrapper-owned; the engine only reclaims fully-drained slots.

On success, it MUST:

- forgive any negative `fee_credits_i`
- reset local fields to canonical zero
- set `last_fee_slot_i = 0`
- mark the slot missing or reusable
- decrement `materialized_account_count`
- require `neg_pnl_account_count` is unchanged (the reclaim precondition already requires `PNL_i == 0`)

### 2.9 Initial market state

At market initialization, the engine MUST set:

- `V = 0`
- `I = 0`
- `C_tot = 0`
- `PNL_pos_tot = 0`
- `PNL_matured_pos_tot = 0`
- `current_slot = init_slot`
- `slot_last = init_slot`
- `P_last = init_oracle_price`
- `fund_px_last = init_oracle_price`
- `A_long = ADL_ONE`, `A_short = ADL_ONE`
- `K_long = 0`, `K_short = 0`
- `F_long_num = 0`, `F_short_num = 0`
- `epoch_long = 0`, `epoch_short = 0`
- `K_epoch_start_long = 0`, `K_epoch_start_short = 0`
- `F_epoch_start_long_num = 0`, `F_epoch_start_short_num = 0`
- `OI_eff_long = 0`, `OI_eff_short = 0`
- `mode_long = Normal`, `mode_short = Normal`
- `stored_pos_count_long = 0`, `stored_pos_count_short = 0`
- `stale_account_count_long = 0`, `stale_account_count_short = 0`
- `phantom_dust_bound_long_q = 0`, `phantom_dust_bound_short_q = 0`
- `materialized_account_count = 0`
- `neg_pnl_account_count = 0`
- `rr_cursor_position = 0`
- `sweep_generation = 0`
- `price_move_consumed_bps_this_generation = 0`
- `market_mode = Live`
- `resolved_price = 0`
- `resolved_live_price = 0`
- `resolved_slot = init_slot`
- `resolved_k_long_terminal_delta = 0`
- `resolved_k_short_terminal_delta = 0`
- `resolved_payout_snapshot_ready = false`
- `resolved_payout_h_num = 0`
- `resolved_payout_h_den = 0`

### 2.10 Side modes and reset lifecycle

A side may be in one of:

- `Normal`
- `DrainOnly`
- `ResetPending`

`begin_full_drain_reset(side)` MAY succeed only if `OI_eff_side == 0`. It MUST:

1. set `K_epoch_start_side = K_side`
2. set `F_epoch_start_side_num = F_side_num`
3. set `K_side = 0` and `F_side_num = 0` (new-epoch numerical baseline)
4. require `epoch_side != u64::MAX`, then increment `epoch_side` by exactly `1` using checked arithmetic
5. set `A_side = ADL_ONE`
6. set `stale_account_count_side = stored_pos_count_side`
7. set `phantom_dust_bound_side_q = 0`
8. set `mode_side = ResetPending`

Step 3 is required for liveness and is economically sound: stale accounts settle against the `K_epoch_start_side` / `F_epoch_start_side_num` snapshots taken in steps 1–2, not against the live indices.

`finalize_side_reset(side)` MAY succeed only if:

- `mode_side == ResetPending`
- `OI_eff_side == 0`
- `stale_account_count_side == 0`
- `stored_pos_count_side == 0`

On success, it MUST set `mode_side = Normal`.

`maybe_finalize_ready_reset_sides_before_oi_increase()` MUST finalize any already-ready reset side before any OI-increasing operation checks side modes.

### 2.10.1 Epoch-gap invariant

For every materialized account with `basis_pos_q_i != 0` on side `s`, the engine MUST maintain exactly one of:

- `epoch_snap_i == epoch_s`, or
- `mode_s == ResetPending` and `epoch_snap_i + 1 == epoch_s`

Epoch gaps larger than `1` are forbidden.

---

## 3. Solvency, haircuts, and live equity

### 3.1 Residual backing

Define:

- `senior_sum = checked_add_u128(C_tot, I)`
- `Residual = max(0, V - senior_sum)`

Invariant: the engine MUST maintain `V >= senior_sum`.

### 3.2 Positive-PnL aggregates

Define:

- `PosPNL_i = max(PNL_i, 0)`
- if `market_mode == Live`, `ReleasedPos_i = PosPNL_i - R_i`
- if `market_mode == Resolved`, `ReleasedPos_i = PosPNL_i`
- on live markets, `PendingWarmupTot = PNL_pos_tot - PNL_matured_pos_tot = Σ R_i`

Reserved fresh positive PnL increases `PNL_pos_tot` immediately but MUST NOT increase `PNL_matured_pos_tot` until warmup release or explicit touch-time acceleration.

### 3.3 Matured withdrawal and conversion haircut `h`

Let:

- if `PNL_matured_pos_tot == 0`, define `h = 1`
- else:
  - `h_num = min(Residual, PNL_matured_pos_tot)`
  - `h_den = PNL_matured_pos_tot`

For account `i`:

- if `PNL_matured_pos_tot == 0`, `PNL_eff_matured_i = ReleasedPos_i`
- else `PNL_eff_matured_i = mul_div_floor_u128(ReleasedPos_i, h_num, h_den)`

### 3.4 Trade-collateral haircut `g`

Let:

- if `PNL_pos_tot == 0`, define `g = 1`
- else:
  - `g_num = min(Residual, PNL_pos_tot)`
  - `g_den = PNL_pos_tot`

For account `i`:

- if `PNL_pos_tot == 0`, `PNL_eff_trade_i = PosPNL_i`
- else `PNL_eff_trade_i = mul_div_floor_u128(PosPNL_i, g_num, g_den)`

Aggregate bound:

- `Σ PNL_eff_trade_i <= g_num <= Residual`

### 3.5 Live equity lanes

All raw equity comparisons in this section MUST use an exact widened signed domain.

For account `i` on a touched state:

- `Eq_withdraw_raw_i = (C_i as wide_signed) + min(PNL_i, 0) + (PNL_eff_matured_i as wide_signed) - (FeeDebt_i as wide_signed)`
- `Eq_trade_raw_i = (C_i as wide_signed) + min(PNL_i, 0) + (PNL_eff_trade_i as wide_signed) - (FeeDebt_i as wide_signed)`
- `Eq_maint_raw_i = (C_i as wide_signed) + (PNL_i as wide_signed) - (FeeDebt_i as wide_signed)`

Derived clamped quantity:

- `Eq_net_i = max(0, Eq_maint_raw_i)`

For candidate trade approval only, define:

- `candidate_trade_pnl_i` = signed execution-slippage PnL created by the candidate trade
- `TradeGain_i_candidate = max(candidate_trade_pnl_i, 0) as u128`
- `PNL_trade_open_i = PNL_i - (TradeGain_i_candidate as i128)`
- `PosPNL_trade_open_i = max(PNL_trade_open_i, 0)`

Counterfactual positive aggregate:

- `PNL_pos_tot_trade_open_i = checked_add_u128(checked_sub_u128(PNL_pos_tot, PosPNL_i), PosPNL_trade_open_i)`

Counterfactual trade haircut:

- if `PNL_pos_tot_trade_open_i == 0`, `PNL_eff_trade_open_i = PosPNL_trade_open_i`
- else:
  - `g_open_num_i = min(Residual, PNL_pos_tot_trade_open_i)`
  - `g_open_den_i = PNL_pos_tot_trade_open_i`
  - `PNL_eff_trade_open_i = mul_div_floor_u128(PosPNL_trade_open_i, g_open_num_i, g_open_den_i)`

Then:

- `Eq_trade_open_raw_i = (C_i as wide_signed) + min(PNL_trade_open_i, 0) + (PNL_eff_trade_open_i as wide_signed) - (FeeDebt_i as wide_signed)`

Interpretation:

- `Eq_withdraw_raw_i` is the extraction lane
- `Eq_trade_open_raw_i` is the only compliant risk-increasing trade approval metric
- `Eq_maint_raw_i` is the maintenance lane
- `Eq_trade_raw_i` is informational only in this revision
- strict risk-reducing comparisons MUST use exact widened `Eq_maint_raw_i`, never a clamped net quantity

---

## 4. Canonical helpers

### 4.1 `set_capital(i, new_C)`

When changing `C_i`, the engine MUST update `C_tot` by the exact signed delta and then set `C_i = new_C`.

### 4.2 `set_position_basis_q(i, new_basis_pos_q)`

When changing stored `basis_pos_q_i` from `old` to `new`, the engine MUST update `stored_pos_count_long` and `stored_pos_count_short` exactly once using the sign flags of `old` and `new`, then write `basis_pos_q_i = new`.

Any transition that increments a side-count — including `0 -> nonzero` and sign flips — MUST enforce `cfg_max_active_positions_per_side`.

### 4.3 `promote_pending_to_scheduled(i)`

Preconditions:

- `market_mode == Live`
- `current_slot` is already the trusted slot anchor for the current instruction state

Effects:

1. if `sched_present_i == true`, return
2. if `pending_present_i == false`, return
3. create the scheduled bucket:
   - `sched_present_i = true`
   - `sched_remaining_q_i = pending_remaining_q_i`
   - `sched_anchor_q_i = pending_remaining_q_i`
   - `sched_start_slot_i = current_slot`
   - `sched_horizon_i = pending_horizon_i`
   - `sched_release_q_i = 0`
4. clear the pending bucket

This helper MUST NOT change `R_i`.

### 4.4 `append_new_reserve(i, reserve_add, admitted_h_eff)`

Preconditions:

- `reserve_add > 0`
- `market_mode == Live`
- `admitted_h_eff > 0`
- `cfg_h_min <= admitted_h_eff <= cfg_h_max`
- `current_slot` is already the trusted slot anchor for the current instruction state

Effects:

1. if the scheduled bucket is absent and the pending bucket is present, call `promote_pending_to_scheduled(i)`
2. if the scheduled bucket is absent:
   - create a scheduled bucket with:
     - `sched_remaining_q = reserve_add`
     - `sched_anchor_q = reserve_add`
     - `sched_start_slot = current_slot`
     - `sched_horizon = admitted_h_eff`
     - `sched_release_q = 0`
3. else if the scheduled bucket is present, the pending bucket is absent, and all of the following hold:
   - `sched_start_slot == current_slot`
   - `sched_horizon == admitted_h_eff`
   then exact same-slot merge into the scheduled bucket is permitted:
   - `sched_remaining_q += reserve_add`
   - `sched_anchor_q += reserve_add`
4. else if the pending bucket is absent:
   - create a pending bucket with:
     - `pending_remaining_q = reserve_add`
     - `pending_horizon = admitted_h_eff`
5. else:
   - `pending_remaining_q += reserve_add`
   - `pending_horizon = max(pending_horizon, admitted_h_eff)`
6. set `R_i += reserve_add`

### 4.5 `apply_reserve_loss_newest_first(i, reserve_loss)`

Preconditions:

- `reserve_loss > 0`
- `reserve_loss <= R_i`
- `market_mode == Live`

Effects:

1. consume reserve from the pending bucket first, if present
2. then consume reserve from the scheduled bucket
3. require full consumption of `reserve_loss`
4. decrement `R_i` by the exact consumed amount
5. clear any now-empty bucket

### 4.6 `prepare_account_for_resolved_touch(i)`

Preconditions:

- `market_mode == Resolved`

Effects:

1. clear the scheduled bucket
2. clear the pending bucket
3. set `R_i = 0`
4. do **not** mutate `PNL_matured_pos_tot`

### 4.6.1 `sync_account_fee_to_slot(i, fee_slot_anchor, fee_rate_per_slot)`

This helper supports exact wrapper-owned recurring fee realization without global scans.

Preconditions:

- account `i` is materialized
- `fee_rate_per_slot >= 0`
- `fee_slot_anchor >= last_fee_slot_i`
- if `market_mode == Live`, `fee_slot_anchor <= current_slot`
- if `market_mode == Resolved`, `fee_slot_anchor <= resolved_slot`

Procedure:

1. `dt = fee_slot_anchor - last_fee_slot_i`
2. if `dt == 0`, return
3. define `fee_abs` by the exact capped-product law:
   - if `fee_rate_per_slot == 0`, set `fee_abs = 0`
   - else if the implementation computes in a widened domain, compute `fee_abs_raw = fee_rate_per_slot * dt` exactly and set `fee_abs = min(fee_abs_raw, MAX_PROTOCOL_FEE_ABS)`
   - else it MUST use the exactly equivalent branch law:
     - if `fee_rate_per_slot > floor(MAX_PROTOCOL_FEE_ABS / dt)`, set `fee_abs = MAX_PROTOCOL_FEE_ABS`
     - else set `fee_abs = fee_rate_per_slot * dt`
4. route `fee_abs` through `charge_fee_to_insurance(i, fee_abs)`
5. set `last_fee_slot_i = fee_slot_anchor`

Normative consequences:

- recurring fees are charged exactly once over `[old_last_fee_slot_i, fee_slot_anchor]`
- double-sync at the same anchor is a no-op
- zero-fee sync still advances the checkpoint to `fee_slot_anchor`
- a newly materialized account starts with `last_fee_slot_i = materialize_slot`, so it never inherits earlier recurring fees
- on resolved markets this helper syncs at most through `resolved_slot`; no recurring fee accrues after resolution
- any tail above `MAX_PROTOCOL_FEE_ABS` is intentionally dropped for liveness rather than blocking progress
- this helper MUST NOT fail solely because the uncapped raw product would exceed native `u128`

### 4.7 `admit_fresh_reserve_h_lock(i, fresh_positive_pnl_i, ctx, admit_h_min, admit_h_max) -> admitted_h_eff`

Preconditions:

- `market_mode == Live`
- account `i` is materialized
- `fresh_positive_pnl_i > 0`
- `0 <= admit_h_min <= admit_h_max <= cfg_h_max`
- `admit_h_max > 0`
- if `admit_h_min > 0`, then `admit_h_min >= cfg_h_min`
- `admit_h_max >= cfg_h_min`

Definitions:

- `senior_sum = checked_add_u128(C_tot, I)`
- `Residual_now = max(0, V - senior_sum)`
- `matured_plus_fresh = checked_add_u128(PNL_matured_pos_tot, fresh_positive_pnl_i)`
- `threshold_opt = ctx.admit_h_max_consumption_threshold_bps_opt_shared`

Admission law:

1. if account `i` is present in `ctx.h_max_sticky_accounts[]`, return `admit_h_max`
2. **Consumption-threshold gate (stress-scaled):**
   - if `threshold_opt = Some(threshold)` and `price_move_consumed_bps_this_generation >= threshold`, set `admitted_h_eff = admit_h_max`
3. else **residual-scarcity gate (post-impact `h` check):**
   - if `matured_plus_fresh <= Residual_now`, set `admitted_h_eff = admit_h_min`
   - else set `admitted_h_eff = admit_h_max`
4. if `admitted_h_eff == admit_h_max`, insert account `i` into `ctx.h_max_sticky_accounts[]`
5. return `admitted_h_eff`

Normative consequences:

- live positive PnL cannot bypass admission
- if `admit_h_min == 0`, immediate release is allowed only when both gates pass
- if `admit_h_min > 0`, the fastest admitted live path is that positive minimum horizon
- once an account requires `admit_h_max` in one instruction for any reason — sticky carry, consumption threshold, or residual scarcity — later fresh positive increments on that same account in that instruction MUST also use `admit_h_max`
- an earlier newest pending increment that was admitted at `admit_h_min` MAY later be conservatively lifted to `admit_h_max` if a later same-instruction increment on the same account requires `admit_h_max` and both share one pending bucket
- this conservative lift may only affect the newest pending bucket; it MUST never rewrite an already-scheduled bucket
- the two gates compose: step 2 catches “recent volatility means reconciliation may still be incomplete” (predictive), and step 3 catches “admission would break `h = 1` right now” (reactive); either trigger forces `admit_h_max`
- `threshold_opt = None` disables step 2 entirely and recovers pre-threshold admission behavior
- `Some(0)` is invalid at input validation time; the optional threshold uses `None`, not `0`, as the disable form
- engine enforcement of the stress gate is conditional on `threshold_opt = Some(threshold)`; the public-wrapper prohibition on `(admit_h_min == 0, threshold_opt = None)` lives in §12.21 and is not an engine-side validation
- wrappers that intend to disable the gate SHOULD pass `None` explicitly rather than a pathologically large `Some(threshold)` that is merely de-facto disabled over any practical sweep horizon
- step 2 auto-relaxes on `sweep_generation` advance in §9.7 Phase 2, which atomically resets `price_move_consumed_bps_this_generation` to `0`

### 4.8 `set_pnl(i, new_PNL, reserve_mode[, ctx])`

`reserve_mode ∈ {UseAdmissionPair(admit_h_min, admit_h_max), ImmediateReleaseResolvedOnly, NoPositiveIncreaseAllowed}`.

Every persistent mutation of `PNL_i` after materialization that may change its sign across zero MUST go through this helper. The optional `ctx` argument is required only when `reserve_mode == UseAdmissionPair(...)`; it is ignored or may be omitted on other modes. The sole direct-mutation exception in this revision is `consume_released_pnl(i, x)` in §4.10, whose preconditions guarantee that `PNL_i` remains non-negative and `neg_pnl_account_count` is unchanged.

Let:

- `old_pos = max(PNL_i, 0)`
- if `market_mode == Resolved`, require `R_i == 0`
- `new_pos = max(new_PNL, 0)`
- `old_neg = (PNL_i < 0)`
- `new_neg = (new_PNL < 0)`

Procedure:

All steps of this helper are part of one atomic top-level instruction effect under §0. If any later checked step fails, all earlier writes performed by this helper — including any mutation to `PNL_i`, `PNL_pos_tot`, `PNL_matured_pos_tot`, `neg_pnl_account_count`, `R_i`, the scheduled bucket, or the pending bucket — MUST roll back atomically with the enclosing instruction.

1. require `new_PNL != i128::MIN`
2. if `market_mode == Live`, require `new_pos <= MAX_ACCOUNT_POSITIVE_PNL_LIVE`
3. if `market_mode == Resolved`, require `new_pos <= i128::MAX as u128`
4. compute `PNL_pos_tot_after` by applying the exact delta from `old_pos` to `new_pos` in checked arithmetic
5. if `market_mode == Live`, require `PNL_pos_tot_after <= MAX_PNL_POS_TOT_LIVE`

If `new_pos > old_pos`:

6. `reserve_add = new_pos - old_pos`
7. if `reserve_mode == NoPositiveIncreaseAllowed`, fail conservatively before any persistent mutation
8. if `reserve_mode == ImmediateReleaseResolvedOnly` and `market_mode == Live`, fail conservatively before any persistent mutation
9. if `reserve_mode == ImmediateReleaseResolvedOnly`:
   - require `market_mode == Resolved`
   - set `PNL_pos_tot = PNL_pos_tot_after`
   - set `PNL_i = new_PNL` and update `neg_pnl_account_count` exactly once if sign crosses zero
   - add `reserve_add` to `PNL_matured_pos_tot`
   - require `PNL_matured_pos_tot <= PNL_pos_tot`
   - return
10. if `reserve_mode == UseAdmissionPair(admit_h_min, admit_h_max)`:
   - require `market_mode == Live`
   - `admitted_h_eff = admit_fresh_reserve_h_lock(i, reserve_add, ctx, admit_h_min, admit_h_max)`
   - set `PNL_pos_tot = PNL_pos_tot_after`
   - set `PNL_i = new_PNL` and update `neg_pnl_account_count` exactly once if sign crosses zero
   - if `admitted_h_eff == 0`:
     - add `reserve_add` to `PNL_matured_pos_tot`
   - else:
     - call `append_new_reserve(i, reserve_add, admitted_h_eff)`
   - require `R_i <= max(PNL_i, 0)` and `PNL_matured_pos_tot <= PNL_pos_tot`
   - return

If `new_pos <= old_pos`:

11. `pos_loss = old_pos - new_pos`
12. if `market_mode == Live`:
   - `reserve_loss = min(pos_loss, R_i)`
   - if `reserve_loss > 0`, call `apply_reserve_loss_newest_first(i, reserve_loss)`
   - `matured_loss = pos_loss - reserve_loss`
13. if `market_mode == Resolved`:
   - require `R_i == 0`
   - `matured_loss = pos_loss`
14. if `matured_loss > 0`, subtract `matured_loss` from `PNL_matured_pos_tot`
15. set `PNL_pos_tot = PNL_pos_tot_after`
16. set `PNL_i = new_PNL` and update `neg_pnl_account_count` exactly once if sign crosses zero
17. if `new_pos == 0` and `market_mode == Live`, require `R_i == 0` and both buckets absent
18. require `R_i <= max(PNL_i, 0)` and `PNL_matured_pos_tot <= PNL_pos_tot`

### 4.9 `admit_outstanding_reserve_on_touch(i)`

Preconditions:

- `market_mode == Live`
- account `i` is materialized

Definitions:

- `reserve_total = (sched_remaining_q_i if sched_present_i else 0) + (pending_remaining_q_i if pending_present_i else 0)`
- `senior_sum = checked_add_u128(C_tot, I)`
- `Residual_now = max(0, V - senior_sum)`
- `matured_plus_reserve = checked_add_u128(PNL_matured_pos_tot, reserve_total)`

Acceleration law:

1. if `reserve_total == 0`, return
2. if `matured_plus_reserve <= Residual_now`:
   - increase `PNL_matured_pos_tot` by `reserve_total`
   - clear both buckets
   - set `R_i = 0`
   - require `PNL_matured_pos_tot <= PNL_pos_tot`
   - require `R_i <= max(PNL_i, 0)`
   - return
3. else return

Normative consequences:

- acceleration never extends a horizon; it only removes reserve when current state safely admits immediate release
- acceleration is monotone: a bucket accelerated once cannot un-accelerate
- acceleration preserves goals 6 and 7: reserve is removed, not reset
- acceleration cannot be griefed: a third party cannot force non-acceleration, and acceleration is strictly more favorable to the user than non-acceleration

### 4.10 `consume_released_pnl(i, x)`

This helper removes only matured released positive PnL on a live account and MUST leave both reserve buckets unchanged.

Preconditions:

- `market_mode == Live`
- `0 < x <= ReleasedPos_i`

Effects:

1. decrease `PNL_i` by exactly `x`
2. decrease `PNL_pos_tot` by exactly `x`
3. decrease `PNL_matured_pos_tot` by exactly `x`
4. leave `neg_pnl_account_count` unchanged because the precondition guarantees the account remains non-negative after the write
5. leave `R_i`, the scheduled bucket, and the pending bucket unchanged
6. require `PNL_matured_pos_tot <= PNL_pos_tot`

### 4.11 `advance_profit_warmup(i)`

Preconditions:

- `market_mode == Live`

Procedure:

1. if `R_i == 0`, require both buckets absent and return
2. if the scheduled bucket is absent and the pending bucket is present, call `promote_pending_to_scheduled(i)`
3. if the scheduled bucket is still absent, return
4. let `elapsed = current_slot - sched_start_slot`
5. let `effective_elapsed = min(elapsed, sched_horizon)`
6. let `sched_total = mul_div_floor_u128(sched_anchor_q, effective_elapsed as u128, sched_horizon as u128)`
7. require `sched_total >= sched_release_q`
8. `sched_increment = sched_total - sched_release_q`
9. `release = min(sched_remaining_q, sched_increment)`
10. if `release > 0`:
   - `sched_remaining_q -= release`
   - `R_i -= release`
   - `PNL_matured_pos_tot += release`
11. if the scheduled bucket is now empty:
   - clear it completely, including `sched_release_q = 0`
   - if the pending bucket is present, call `promote_pending_to_scheduled(i)`
12. else:
   - set `sched_release_q = sched_total`
13. if `R_i == 0`, require both buckets absent
14. require `PNL_matured_pos_tot <= PNL_pos_tot`

This formulation makes explicit the intended law: if loss consumption made `release < sched_increment`, that can only happen because the scheduled bucket emptied in this call, so no persistent over-advanced `sched_release_q` remains on a non-empty bucket.

### 4.12 `attach_effective_position(i, new_eff_pos_q)`

This helper converts a current effective quantity into a new position basis at the current side state.

If discarding a same-epoch nonzero basis, it MUST first compute whether the old same-epoch effective quantity had a nonzero fractional orphan remainder. Concretely, let `old_basis = basis_pos_q_i`, `s = side(old_basis)`, `A_s_current = A_s`, and `a_basis_old = a_basis_i`. If `old_basis != 0`, `epoch_snap_i == epoch_s`, and `a_basis_old > 0`, compute `orphan_rem = (abs(old_basis) * A_s_current) mod a_basis_old` in exact wide arithmetic. If `orphan_rem != 0`, it MUST call `inc_phantom_dust_bound(s)`, i.e. increment the appropriate phantom-dust bound by exactly `1` q-unit, before overwriting the basis.

If `new_eff_pos_q == 0`, it MUST:

- zero the stored basis via `set_position_basis_q(i, 0)`
- reset snapshots to canonical zero-position defaults

If `new_eff_pos_q != 0`, it MUST:

- require `abs(new_eff_pos_q) <= MAX_POSITION_ABS_Q`
- write the new basis via `set_position_basis_q(i, new_eff_pos_q)`
- set `a_basis_i = A_side(new_eff_pos_q)`
- set `k_snap_i = K_side(new_eff_pos_q)`
- set `f_snap_i = F_side_num(new_eff_pos_q)`
- set `epoch_snap_i = epoch_side(new_eff_pos_q)`

### 4.13 Phantom-dust helpers

- `inc_phantom_dust_bound(side)` increments by exactly `1` q-unit.
- `inc_phantom_dust_bound_by(side, amount_q)` increments by exactly `amount_q`.

### 4.14 `max_safe_flat_conversion_released(i, x_cap, h_num, h_den)`

This helper returns the largest `x_safe <= x_cap` such that converting `x_safe` released profit on a live flat account cannot make the account’s exact post-conversion raw maintenance equity negative.

Implementation law:

1. if `x_cap == 0`, return `0`
2. let `E_before = Eq_maint_raw_i` on the current exact state
3. if `E_before <= 0`, return `0`
4. if `h_den == 0` or `h_num == h_den`, return `x_cap`
5. let `haircut_loss_num = h_den - h_num`
6. return `min(x_cap, floor(E_before * h_den / haircut_loss_num))` using an exact capped multiply-divide with at least 256-bit intermediates, or an equivalent exact wide comparison

### 4.15 `compute_trade_pnl(size_q, oracle_price, exec_price)`

For a bilateral trade where `size_q > 0` means account `a` buys base from account `b`, the execution-slippage PnL applied before fees MUST be:

- `trade_pnl_num = size_q * (oracle_price - exec_price)`
- `trade_pnl_a = floor_div_signed_conservative(trade_pnl_num, POS_SCALE)`
- `trade_pnl_b = -trade_pnl_a`

This helper MUST use checked signed arithmetic and exact conservative floor division.

### 4.16 `charge_fee_to_insurance(i, fee_abs) -> FeeChargeOutcome`

Preconditions:

- `fee_abs <= MAX_PROTOCOL_FEE_ABS`

Return value:

- `fee_paid_to_insurance_i`
- `fee_equity_impact_i`
- `fee_dropped_i`

Definitions:

- `fee_paid_to_insurance_i` = amount immediately paid out of capital into `I`
- `fee_equity_impact_i` = total actual reduction in the account’s raw equity from this fee application, equal to capital paid plus collectible fee debt added
- `fee_dropped_i = fee_abs - fee_equity_impact_i` = permanently uncollectible tail

Effects:

1. `debt_headroom = fee_credit_headroom_u128_checked(fee_credits_i)`
2. `collectible = checked_add_u128(C_i, debt_headroom)`
3. `fee_equity_impact_i = min(fee_abs, collectible)`
4. `fee_paid_to_insurance_i = min(fee_equity_impact_i, C_i)`
5. if `fee_paid_to_insurance_i > 0`:
   - `set_capital(i, C_i - fee_paid_to_insurance_i)`
   - `I = checked_add_u128(I, fee_paid_to_insurance_i)`
6. `fee_shortfall = fee_equity_impact_i - fee_paid_to_insurance_i`
7. if `fee_shortfall > 0`, subtract it from `fee_credits_i`
8. `fee_dropped_i = fee_abs - fee_equity_impact_i`

This helper MUST NOT mutate `PNL_i`, `PNL_pos_tot`, `PNL_matured_pos_tot`, reserve state, or any `K_side`.

### 4.17 Insurance-loss helpers

- `use_insurance_buffer(loss_abs)` spends the full insurance balance and returns the remainder.
- `record_uninsured_protocol_loss(loss_abs)` leaves the uncovered loss represented through `Residual` and junior haircuts.
- `absorb_protocol_loss(loss_abs)` = `use_insurance_buffer` then `record_uninsured_protocol_loss` if needed.

---

## 5. Unified A/K/F side-index mechanics

### 5.1 Eager-equivalent event law

For one side, a single eager global event on absolute fixed-point position `q_q >= 0` and realized PnL `p` has the form:

- `q_q' = α q_q`
- `p' = p + β * q_q / POS_SCALE`

The cumulative indices compose as:

- `A_new = A_old * α`
- `K_new = K_old + A_old * β`

### 5.2 `effective_pos_q(i)`

For an account with nonzero basis:

- let `s = side(basis_pos_q_i)`
- if `epoch_snap_i != epoch_s`, define `effective_pos_q(i) = 0`
- else `effective_abs_pos_q(i) = mul_div_floor_u128(abs(basis_pos_q_i), A_s, a_basis_i)`
- `effective_pos_q(i) = sign(basis_pos_q_i) * effective_abs_pos_q(i)`

### 5.2.1 Side-OI components

For any signed fixed-point position `q`:

- `OI_long_component(q) = max(q, 0) as u128`
- `OI_short_component(q) = max(-q, 0) as u128`

### 5.2.2 Exact bilateral trade side-OI after-values

For a bilateral trade with old and new effective positions for both counterparties:

- `OI_long_after_trade = (((OI_eff_long - old_long_a) - old_long_b) + new_long_a) + new_long_b`
- `OI_short_after_trade = (((OI_eff_short - old_short_a) - old_short_b) + new_short_a) + new_short_b`

These exact after-values MUST be used both for gating and for final writeback.

### 5.3 `settle_side_effects_live(i, ctx)`

When touching account `i` on a live market:

1. if `basis_pos_q_i == 0`, return
2. let `s = side(basis_pos_q_i)`
3. let `den = checked_mul_u128(a_basis_i, POS_SCALE)`
4. if `epoch_snap_i == epoch_s`:
   - `q_eff_new = mul_div_floor_u128(abs(basis_pos_q_i), A_s, a_basis_i)`
   - `pnl_delta = wide_signed_mul_div_floor_from_kf_pair(abs(basis_pos_q_i), k_snap_i, K_s, f_snap_i, F_s_num, den)`
   - `set_pnl(i, PNL_i + pnl_delta, UseAdmissionPair(ctx.admit_h_min_shared, ctx.admit_h_max_shared), ctx)`
   - if `q_eff_new == 0`:
     - call `inc_phantom_dust_bound(s)`, i.e. increment the appropriate phantom-dust bound by exactly `1` q-unit (the remaining same-epoch quantity is strictly between `0` and `1` q-unit)
     - zero the basis
     - reset snapshots to canonical zero-position defaults
   - else:
     - update `k_snap_i`
     - update `f_snap_i`
     - update `epoch_snap_i`
5. else:
   - require `mode_s == ResetPending`
   - require `epoch_snap_i + 1 == epoch_s`
   - require `stale_account_count_s > 0`
   - `pnl_delta = wide_signed_mul_div_floor_from_kf_pair(abs(basis_pos_q_i), k_snap_i, K_epoch_start_s, f_snap_i, F_epoch_start_s_num, den)`
   - `set_pnl(i, PNL_i + pnl_delta, UseAdmissionPair(ctx.admit_h_min_shared, ctx.admit_h_max_shared), ctx)`
   - zero the basis
   - decrement `stale_account_count_s`
   - reset snapshots

### 5.4 `settle_side_effects_resolved(i)`

When touching account `i` on a resolved market:

Preconditions:

- `market_mode == Resolved`
- `prepare_account_for_resolved_touch(i)` has already executed in the current top-level instruction, equivalently `R_i == 0` and both reserve buckets are absent

Procedure:

1. if `basis_pos_q_i == 0`, return
2. let `s = side(basis_pos_q_i)`
3. require stale one-epoch-lag conditions on its side
4. require `stale_account_count_s > 0`
5. let `den = checked_mul_u128(a_basis_i, POS_SCALE)`
6. let `resolved_k_terminal_delta_s` denote `resolved_k_long_terminal_delta` on the long side and `resolved_k_short_terminal_delta` on the short side
7. let `k_terminal_s_exact = (K_epoch_start_s as wide_signed) + (resolved_k_terminal_delta_s as wide_signed)`
8. let `f_terminal_s_exact = F_epoch_start_s_num`
9. compute `pnl_delta = wide_signed_mul_div_floor_from_kf_pair(abs(basis_pos_q_i), k_snap_i, k_terminal_s_exact, f_snap_i, f_terminal_s_exact, den)`
10. `set_pnl(i, PNL_i + pnl_delta, ImmediateReleaseResolvedOnly)`
11. zero the basis
12. decrement `stale_account_count_s`
13. reset snapshots

### 5.5 `accrue_market_to(now_slot, oracle_price, funding_rate_e9_per_slot)`

Before any live operation that depends on current market state, the engine MUST call `accrue_market_to(now_slot, oracle_price, funding_rate_e9_per_slot)`.

This helper MUST:

1. require `market_mode == Live`
2. require trusted `now_slot >= slot_last`
3. require validated `0 < oracle_price <= MAX_ORACLE_PRICE`
4. require `abs(funding_rate_e9_per_slot) <= cfg_max_abs_funding_e9_per_slot`
5. let `dt = now_slot - slot_last`
6. let `funding_active = funding_rate_e9_per_slot != 0 && OI_eff_long != 0 && OI_eff_short != 0 && fund_px_last > 0`
7. let `price_move_active = P_last > 0 && oracle_price != P_last && (OI_eff_long != 0 || OI_eff_short != 0)`
8. if `funding_active || price_move_active`, require `dt <= cfg_max_accrual_dt_slots`; otherwise `dt` is unbounded because no K/F equity-drain delta is applied
9. if `price_move_active`, require the per-slot price-move cap:
   - compute `abs_delta_price = abs(oracle_price - P_last)` in exact checked arithmetic
   - require `abs_delta_price * 10_000 <= cfg_max_price_move_bps_per_slot * dt * P_last`
   - compute the comparison in at least 256-bit signed or unsigned intermediates, or a formally equivalent exact method
   - fail conservatively before any state mutation if the check fails
9a. update generation-scoped consumption tracking:
   - if `price_move_active` and `abs_delta_price > 0`:
     - `consumed_this_step = mul_div_floor_u128(abs_delta_price, 10_000, P_last)`
     - `price_move_consumed_bps_this_generation = checked_add_u128(price_move_consumed_bps_this_generation, consumed_this_step)`
   - floor is intentional: this accumulator is a stress / UX signal, not the construction-level safety cap, so sub-bps jitter MUST NOT round up into whole-bps consumption
   - the accumulator resets to `0` only when `sweep_generation` advances (see §9.7 Phase 2)
   - this value is read-only exposed state and is consulted by the consumption-threshold gate in §4.7 step 2
10. snapshot `OI_long_0 = OI_eff_long`, `OI_short_0 = OI_eff_short`, and `fund_px_0 = fund_px_last`
11. mark-to-market once:
   - `ΔP = oracle_price - P_last`
   - if `OI_long_0 > 0`, compute `delta_k_long = A_long * ΔP` in an exact wide signed domain; if the resulting persistent `K_long` would overflow `i128`, fail conservatively; else apply it
   - if `OI_short_0 > 0`, compute `delta_k_short = -A_short * ΔP` in an exact wide signed domain; if the resulting persistent `K_short` would overflow `i128`, fail conservatively; else apply it
12. funding transfer:
   - if `funding_active`:
     - compute `fund_num_total = fund_px_0 * funding_rate_e9_per_slot * dt` in an exact wide signed domain of at least 256 bits, or a formally equivalent exact method
     - compute each `A_side * fund_num_total` product in the same exact wide signed domain, or a formally equivalent exact method
     - if the resulting persistent `F_long_num` or `F_short_num` would overflow `i128`, fail conservatively
     - else apply both updates exactly:
       - `F_long_num -= A_long * fund_num_total`
       - `F_short_num += A_short * fund_num_total`
13. update `slot_last = now_slot`
14. update `P_last = oracle_price`
15. update `fund_px_last = oracle_price`

Because this helper is only defined as part of a top-level atomic instruction under §0, any overflow or conservative failure in a later leg of the helper or later instruction logic MUST roll back any earlier tentative `K_side`, `F_side_num`, `P_last`, `fund_px_last`, `slot_last`, or `price_move_consumed_bps_this_generation` writes from the same top-level call. The same top-level atomicity rule also applies to `rr_cursor_position` and `sweep_generation` when they are mutated later in §9.7.

### 5.6 `enqueue_adl(ctx, liq_side, q_close_q, D)`

Suppose a bankrupt liquidation from side `liq_side` leaves an uncovered deficit `D >= 0`. Let `opp = opposite(liq_side)`.

This helper MUST:

1. decrement `OI_eff_liq_side` by `q_close_q` if `q_close_q > 0`
2. spend insurance first: `D_rem = use_insurance_buffer(D)`
3. let `OI_before = OI_eff_opp`
4. if `OI_before == 0`:
   - if `D_rem > 0`, route it through `record_uninsured_protocol_loss`
   - if `OI_eff_long == 0` and `OI_eff_short == 0`, set both pending-reset flags true
   - return
5. if `OI_before > 0` and `stored_pos_count_opp == 0`:
   - require `q_close_q <= OI_before`
   - set `OI_eff_opp = OI_before - q_close_q`
   - if `D_rem > 0`, route it through `record_uninsured_protocol_loss`
   - if `OI_eff_long == 0` and `OI_eff_short == 0`, set both pending-reset flags true
   - return
6. otherwise:
   - require `q_close_q <= OI_before`
   - `A_old = A_opp`
   - `OI_post = OI_before - q_close_q`
7. if `D_rem > 0`:
   - compute `delta_K_abs = ceil(D_rem * A_old * POS_SCALE / OI_before)` using exact wide arithmetic
   - compute `K_candidate = K_opp + delta_K_exact` with `delta_K_exact = -delta_K_abs`
   - require future-mark headroom: `|K_candidate| + A_old * MAX_ORACLE_PRICE <= i128::MAX`. This ensures any subsequent `accrue_market_to` with a valid oracle move cannot overflow `K_opp` in the mark-to-market step. `A_opp` cannot grow post-ADL (`A_new <= A_old`), so using `A_old` here is conservative.
   - if the magnitude is non-representable, if the signed `K_opp + delta_K_exact` overflows, OR if the headroom requirement fails, route `D_rem` through `record_uninsured_protocol_loss`
   - else apply `K_opp += delta_K_exact`
8. if `OI_post == 0`:
   - set `OI_eff_opp = 0`
   - set both pending-reset flags true
   - return
9. compute `A_candidate = floor(A_old * OI_post / OI_before)`
10. if `A_candidate > 0`:
   - set `A_opp = A_candidate`
   - set `OI_eff_opp = OI_post`
   - if `OI_post < OI_before`:
     - `N_opp = stored_pos_count_opp as u128`
     - `global_a_dust_bound = N_opp + ceil((OI_before + N_opp) / A_old)`
     - increment the appropriate phantom-dust bound by `global_a_dust_bound`
   - if `A_opp < MIN_A_SIDE`, set `mode_opp = DrainOnly`
   - return
11. if `A_candidate == 0` while `OI_post > 0`:
   - set `OI_eff_long = 0`
   - set `OI_eff_short = 0`
   - set both pending-reset flags true

Insurance-first ordering in this helper is intentional. Bankruptcy deficit is senior to junior PnL and therefore hits available insurance before the engine determines whether any residual quote loss can also be represented through opposing-side `K` updates. Zero-OI and zero-stored-position-count branches may therefore consume insurance and still route the remaining deficit through `record_uninsured_protocol_loss`.

### 5.7 `schedule_end_of_instruction_resets(ctx)`

This helper MUST be called exactly once at the end of every top-level instruction that can touch accounts, mutate side state, liquidate, or resolved-close.

Procedure:

1. **Bilateral-empty dust clearance**  
   If `stored_pos_count_long == 0` and `stored_pos_count_short == 0`:
   - `clear_bound_q = phantom_dust_bound_long_q + phantom_dust_bound_short_q`
   - `has_residual_clear_work = (OI_eff_long > 0) or (OI_eff_short > 0) or (phantom_dust_bound_long_q > 0) or (phantom_dust_bound_short_q > 0)`
   - if `has_residual_clear_work`:
     - require `OI_eff_long == OI_eff_short`
     - if `OI_eff_long <= clear_bound_q` and `OI_eff_short <= clear_bound_q`:
       - set `OI_eff_long = 0`
       - set `OI_eff_short = 0`
       - set both pending-reset flags true
     - else fail conservatively

2. **Unilateral-empty dust clearance, long side empty**  
   Else if `stored_pos_count_long == 0` and `stored_pos_count_short > 0`:
   - `has_residual_clear_work = (OI_eff_long > 0) or (OI_eff_short > 0) or (phantom_dust_bound_long_q > 0)`
   - if `has_residual_clear_work`:
     - require `OI_eff_long == OI_eff_short`
     - if `OI_eff_long <= phantom_dust_bound_long_q`:
       - set `OI_eff_long = 0`
       - set `OI_eff_short = 0`
       - set both pending-reset flags true
     - else fail conservatively

3. **Unilateral-empty dust clearance, short side empty**  
   Else if `stored_pos_count_short == 0` and `stored_pos_count_long > 0`:
   - `has_residual_clear_work = (OI_eff_long > 0) or (OI_eff_short > 0) or (phantom_dust_bound_short_q > 0)`
   - if `has_residual_clear_work`:
     - require `OI_eff_long == OI_eff_short`
     - if `OI_eff_short <= phantom_dust_bound_short_q`:
       - set `OI_eff_long = 0`
       - set `OI_eff_short = 0`
       - set both pending-reset flags true
     - else fail conservatively

4. **DrainOnly zero-OI scheduling**
   - if `mode_long == DrainOnly` and `OI_eff_long == 0`, set `pending_reset_long = true`
   - if `mode_short == DrainOnly` and `OI_eff_short == 0`, set `pending_reset_short = true`

### 5.8 `finalize_end_of_instruction_resets(ctx)`

This helper MUST:

1. if `pending_reset_long` and `mode_long != ResetPending`, invoke `begin_full_drain_reset(long)`
2. if `pending_reset_short` and `mode_short != ResetPending`, invoke `begin_full_drain_reset(short)`
3. if `mode_long == ResetPending` and `OI_eff_long == 0` and `stale_account_count_long == 0` and `stored_pos_count_long == 0`, invoke `finalize_side_reset(long)`
4. if `mode_short == ResetPending` and `OI_eff_short == 0` and `stale_account_count_short == 0` and `stored_pos_count_short == 0`, invoke `finalize_side_reset(short)`

---

## 6. Loss settlement, live finalization, and resolved-close helpers

### 6.1 `settle_losses_from_principal(i)`

If `PNL_i < 0`, the engine MUST attempt to settle from principal immediately:

1. `need = (-PNL_i) as u128`
2. `pay = min(need, C_i)`
3. apply:
   - `set_capital(i, C_i - pay)`
   - `set_pnl(i, PNL_i + pay, NoPositiveIncreaseAllowed)`

### 6.2 Open-position negative remainder

If after §6.1:

- `PNL_i < 0`, and
- `effective_pos_q(i) != 0`

then the account MUST remain liquidatable.

### 6.3 Flat-account negative remainder

If after §6.1:

- `PNL_i < 0`, and
- `effective_pos_q(i) == 0`

then the engine MUST:

1. `absorb_protocol_loss((-PNL_i) as u128)`
2. `set_pnl(i, 0, NoPositiveIncreaseAllowed)`

This path is allowed only for already-authoritative flat accounts.

### 6.4 `fee_debt_sweep(i)`

After any operation that increases `C_i`, or after a full current-state authoritative touch where capital is no longer senior-encumbered by attached trading losses, the engine MUST pay down fee debt:

1. `debt = fee_debt_u128_checked(fee_credits_i)`
2. `pay = min(debt, C_i)`
3. if `pay > 0`:
   - `set_capital(i, C_i - pay)`
   - add `pay` to `fee_credits_i`
   - `I = I + pay`

Late fee realization from `C_i` to `I` does **not** change `Residual = V - (C_tot + I)` and therefore does not invalidate a previously captured resolved payout snapshot.

### 6.5 `touch_account_live_local(i, ctx)`

This is the canonical live local touch.

Procedure:

1. require `market_mode == Live`
2. require account `i` is materialized
3. add `i` to `ctx.touched_accounts[]` if not already present
4. `admit_outstanding_reserve_on_touch(i)`
5. `advance_profit_warmup(i)`
6. `settle_side_effects_live(i, ctx)`
7. `settle_losses_from_principal(i)`
8. if `effective_pos_q(i) == 0` and `PNL_i < 0`, resolve uncovered flat loss
9. MUST NOT auto-convert
10. MUST NOT call `fee_debt_sweep(i)`

If the deployment enables wrapper-owned recurring account fees, the wrapper MUST sync the account’s recurring fee to the relevant live slot anchor **before** relying on any health-sensitive result of this touched state.

### 6.6 `finalize_touched_accounts_post_live(ctx)`

This helper is mandatory for every live instruction that uses `touch_account_live_local`.

Procedure:

1. compute one shared post-live conversion snapshot:
   - `Residual_snapshot = max(0, V - (C_tot + I))`
   - `PNL_matured_pos_tot_snapshot = PNL_matured_pos_tot`
   - if `PNL_matured_pos_tot_snapshot == 0`, define `whole_snapshot = false`
   - else:
     - `h_snapshot_num = min(Residual_snapshot, PNL_matured_pos_tot_snapshot)`
     - `h_snapshot_den = PNL_matured_pos_tot_snapshot`
     - `whole_snapshot = (h_snapshot_num == h_snapshot_den)`
2. iterate `ctx.touched_accounts[]` in deterministic ascending storage-index order:
   - if `basis_pos_q_i == 0`, `ReleasedPos_i > 0`, and `whole_snapshot == true`:
     - `released = ReleasedPos_i`
     - `consume_released_pnl(i, released)`
     - `set_capital(i, C_i + released)`
   - call `fee_debt_sweep(i)`

### 6.7 Resolved positive-payout readiness

Positive resolved payouts MUST NOT begin until the market is terminal-ready for positive claims.

A market is **positive-payout ready** only when all of the following hold:

- `stale_account_count_long == 0`
- `stale_account_count_short == 0`
- `stored_pos_count_long == 0`
- `stored_pos_count_short == 0`
- `neg_pnl_account_count == 0`

`neg_pnl_account_count` is therefore the exact O(1) readiness aggregate for remaining negative claims.

### 6.8 `capture_resolved_payout_snapshot_if_needed()`

This helper MAY succeed only if:

- `market_mode == Resolved`
- `resolved_payout_snapshot_ready == false`
- the market is positive-payout ready per §6.7

On success:

1. `Residual_snapshot = max(0, V - (C_tot + I))`
2. if `PNL_matured_pos_tot == 0`:
   - `resolved_payout_h_num = 0`
   - `resolved_payout_h_den = 0`
3. else:
   - `resolved_payout_h_num = min(Residual_snapshot, PNL_matured_pos_tot)`
   - `resolved_payout_h_den = PNL_matured_pos_tot`
4. set `resolved_payout_snapshot_ready = true`

This snapshot is stable under later resolved fee sync because fee sync is a pure `C -> I` reclassification with `V` unchanged; it therefore preserves `V - (C_tot + I)`.

### 6.9 `force_close_resolved_terminal_nonpositive(i) -> payout`

This helper terminally closes a resolved account after the nonpositive branch has already normalized any negative flat remainder to zero, and returns its terminal payout.

Preconditions:

- `market_mode == Resolved`
- account `i` is materialized
- `basis_pos_q_i == 0`
- `PNL_i == 0`

Procedure:

1. call `fee_debt_sweep(i)` (recurring-fee ordering is a wrapper responsibility; see §9.9)
2. forgive any remaining negative `fee_credits_i`
3. let `payout = C_i`
4. if `payout > 0`:
   - `set_capital(i, 0)`
   - `V = V - payout`
5. require `PNL_i == 0`, `R_i == 0`, both reserve buckets absent, `basis_pos_q_i == 0`, and `last_fee_slot_i <= resolved_slot`
6. reset local fields and free the slot
7. require `V >= C_tot + I`
8. return `payout`

### 6.10 `force_close_resolved_terminal_positive(i) -> payout`

This helper terminally closes a resolved account with a positive claim and returns its terminal payout.

Preconditions:

- `market_mode == Resolved`
- account `i` is materialized
- `basis_pos_q_i == 0`
- `PNL_i > 0`
- `resolved_payout_snapshot_ready == true`
- `resolved_payout_h_den > 0`

Procedure:

1. let `x = max(PNL_i, 0)`
2. let `y = floor(x * resolved_payout_h_num / resolved_payout_h_den)`
3. `set_pnl(i, 0, NoPositiveIncreaseAllowed)`
4. `set_capital(i, C_i + y)`
5. call `fee_debt_sweep(i)` (recurring-fee ordering is a wrapper responsibility; see §9.9)
6. forgive any remaining negative `fee_credits_i`
7. let `payout = C_i`
8. if `payout > 0`:
   - `set_capital(i, 0)`
   - `V = V - payout`
9. require `PNL_i == 0`, `R_i == 0`, both reserve buckets absent, `basis_pos_q_i == 0`, and `last_fee_slot_i <= resolved_slot`
10. reset local fields and free the slot
11. require `V >= C_tot + I`
12. return `payout`

Impossible states — for example `resolved_payout_snapshot_ready == true` with `PNL_i > 0` but `resolved_payout_h_den == 0` — MUST fail conservatively rather than falling back to `y = x`.

---

## 7. Fees

This revision still has no engine-native recurring maintenance fee. The engine core defines native trading fees, native liquidation fees, and the canonical helpers for optional wrapper-owned account fees. The `last_fee_slot_i` checkpoint exists so wrapper-owned recurring fees can be realized exactly on touched accounts.

### 7.1 Trading fees

Define:

- `fee = mul_div_ceil_u128(trade_notional, cfg_trading_fee_bps, 10_000)`

Rules:

- if `cfg_trading_fee_bps == 0` or `trade_notional == 0`, then `fee = 0`
- if `cfg_trading_fee_bps > 0` and `trade_notional > 0`, then `fee >= 1`

### 7.2 Liquidation fees

For a liquidation that closes `q_close_q` at `oracle_price`:

- if `q_close_q == 0`, `liq_fee = 0`
- else:
  - `closed_notional = mul_div_floor_u128(q_close_q, oracle_price, POS_SCALE)`
  - `liq_fee_raw = mul_div_ceil_u128(closed_notional, cfg_liquidation_fee_bps, 10_000)`
  - `liq_fee = min(max(liq_fee_raw, cfg_min_liquidation_abs), cfg_liquidation_fee_cap)`

### 7.3 Optional wrapper-owned account fees

A wrapper MAY impose additional account fees by routing an amount `fee_abs` through `charge_fee_to_insurance(i, fee_abs)`, provided `fee_abs <= MAX_PROTOCOL_FEE_ABS`.

If the wrapper wants a recurring time-based fee, it SHOULD do so through `sync_account_fee_to_slot(i, fee_slot_anchor, fee_rate_per_slot)` rather than by attempting to reconstruct elapsed time externally without a per-account checkpoint.

---

## 8. Margin checks and liquidation

### 8.1 Margin requirements

After live touch reconciliation, define:

- `Notional_i = mul_div_floor_u128(abs(effective_pos_q(i)), oracle_price, POS_SCALE)`

If `effective_pos_q(i) == 0`:

- `MM_req_i = 0`
- `IM_req_i = 0`

Else:

- `MM_req_i = max(mul_div_floor_u128(Notional_i, cfg_maintenance_bps, 10_000), cfg_min_nonzero_mm_req)`
- `IM_req_i = max(mul_div_floor_u128(Notional_i, cfg_initial_bps, 10_000), cfg_min_nonzero_im_req)`

Healthy conditions:

- maintenance healthy if exact `Eq_net_i > MM_req_i`
- withdrawal healthy if exact `Eq_withdraw_raw_i >= IM_req_i`
- risk-increasing trade approval healthy if exact `Eq_trade_open_raw_i >= IM_req_post_i`

### 8.2 Risk-increasing and strictly risk-reducing trades

A trade for account `i` is risk-increasing when either:

1. `abs(new_eff_pos_q_i) > abs(old_eff_pos_q_i)`, or
2. the position sign flips across zero, or
3. `old_eff_pos_q_i == 0` and `new_eff_pos_q_i != 0`

A trade is strictly risk-reducing when:

- `old_eff_pos_q_i != 0`
- `new_eff_pos_q_i != 0`
- `sign(new_eff_pos_q_i) == sign(old_eff_pos_q_i)`
- `abs(new_eff_pos_q_i) < abs(old_eff_pos_q_i)`

### 8.3 Liquidation eligibility

An account is liquidatable when after a full current-state authoritative live touch:

- `effective_pos_q(i) != 0`, and
- `Eq_net_i <= MM_req_i`

If the deployment enables wrapper-owned recurring account fees, that touched state MUST be fee-current for the account before liquidatability is evaluated.

### 8.4 Partial liquidation

A liquidation MAY be partial only if:

- `0 < q_close_q < abs(old_eff_pos_q_i)`

A successful partial liquidation MUST:

1. use the current touched state
2. compute the nonzero remaining effective position
3. close `q_close_q` synthetically at `oracle_price`; this adds **no** additional execution-slippage PnL because the synthetic execution price equals the oracle price
4. apply the remaining position with `attach_effective_position`
5. settle realized losses from principal
6. charge the liquidation fee on the closed quantity
7. invoke `enqueue_adl(ctx, liq_side, q_close_q, 0)`
8. even if a pending reset is scheduled, still require the remaining nonzero position to be maintenance healthy on the current post-step state before returning

### 8.5 Full-close or bankruptcy liquidation

A deterministic full-close liquidation MUST:

1. use the current touched state
2. close the full remaining effective position synthetically at `oracle_price`; this adds **no** additional execution-slippage PnL because the synthetic execution price equals the oracle price
3. zero the basis with `attach_effective_position(i, 0)`
4. settle realized losses from principal
5. charge liquidation fee
6. define bankruptcy deficit `D = max(-PNL_i, 0)`
7. invoke `enqueue_adl(ctx, liq_side, q_close_q, D)` if `q_close_q > 0` or `D > 0`
8. if `D > 0`, set `PNL_i = 0` with `NoPositiveIncreaseAllowed`

### 8.6 Side-mode gating

Before any top-level instruction rejects an OI-increasing operation because a side is in `ResetPending`, it MUST first invoke `maybe_finalize_ready_reset_sides_before_oi_increase()`.

Any operation that would increase net side open interest on a side whose mode is `DrainOnly` or `ResetPending` MUST be rejected.

For `execute_trade`, this prospective check MUST use the exact bilateral candidate after-values of §5.2.2 on both sides.

---

## 9. External operations

### 9.0 Standard live instruction lifecycle

`(admit_h_min, admit_h_max)`, `admit_h_max_consumption_threshold_bps_opt`, and `funding_rate_e9_per_slot` are wrapper-owned logical inputs, not public caller-owned fields. Public or permissionless wrappers MUST derive them internally.

If the deployment enables wrapper-owned recurring account fees, any top-level instruction that depends on current account health or reclaimability MUST sync the relevant touched account(s) to the intended fee anchor before relying on health-sensitive or reclaim-sensitive results.

Unless explicitly noted otherwise, a live external state-mutating operation that depends on current market state executes in this order:

1. validate monotonic slot, oracle input, funding-rate bound, admission-pair bound, the optional consumption threshold, and any other instruction-specific price inputs required by the endpoint:
   - `admit_h_max_consumption_threshold_bps_opt = None` disables the consumption-threshold gate
   - `admit_h_max_consumption_threshold_bps_opt = Some(threshold)` requires `threshold > 0`
2. initialize fresh `ctx` with `admit_h_min_shared = admit_h_min`, `admit_h_max_shared = admit_h_max`, and `admit_h_max_consumption_threshold_bps_opt_shared = admit_h_max_consumption_threshold_bps_opt`
3. call `accrue_market_to(now_slot, oracle_price, funding_rate_e9_per_slot)` exactly once
4. set `current_slot = now_slot`
5. if recurring account fees are enabled, sync the operation’s touched account set to `current_slot` before any health-sensitive check for those accounts
6. perform the endpoint’s exact current-state inner execution
7. call `finalize_touched_accounts_post_live(ctx)` exactly once
8. call `schedule_end_of_instruction_resets(ctx)` exactly once
9. call `finalize_end_of_instruction_resets(ctx)` exactly once
10. assert `OI_eff_long == OI_eff_short` at the end of every live top-level instruction that can mutate side state or live exposure
11. require `V >= C_tot + I`

The per-instruction procedures in §§9.1, 9.3, 9.3.1, 9.4, 9.5, 9.6, and 9.7 explicitly inherit step 1 above as their first numbered step so specification and harness authors do not need to infer that validation from context.

### 9.1 `settle_account(i, oracle_price, now_slot, funding_rate_e9_per_slot, admit_h_min, admit_h_max, admit_h_max_consumption_threshold_bps_opt[, fee_rate_per_slot])`

1. validate inputs per §9.0 step 1
2. require `market_mode == Live`
3. require account `i` is materialized
4. initialize `ctx`
5. accrue market once
6. set `current_slot`
7. if recurring fees are enabled, `sync_account_fee_to_slot(i, current_slot, fee_rate_per_slot)`
8. `touch_account_live_local(i, ctx)`
9. `finalize_touched_accounts_post_live(ctx)`
10. schedule resets
11. finalize resets
12. assert `OI_eff_long == OI_eff_short`
13. require `V >= C_tot + I`

### 9.2 `deposit(i, amount, now_slot)`

`deposit` is pure capital transfer. It MUST NOT call `accrue_market_to`, MUST NOT mutate side state, and MUST NOT mutate reserve state.

Procedure:

1. require `market_mode == Live`
2. require `now_slot >= current_slot`
2a. require `now_slot <= slot_last + cfg_max_accrual_dt_slots`
3. set `current_slot = now_slot`
4. if account `i` is missing:
   - require `amount > 0` (the engine has no deposit minimum beyond non-zero; any higher floor is wrapper policy)
   - materialize the account with `materialize_account(i, now_slot)`
5. require `V + amount <= MAX_VAULT_TVL`
6. set `V = V + amount`
7. `set_capital(i, C_i + amount)`
8. `settle_losses_from_principal(i)`
9. MUST NOT invoke flat-loss insurance absorption
10. if `basis_pos_q_i == 0` and `PNL_i >= 0`, call `fee_debt_sweep(i)`
11. require `V >= C_tot + I`

> **Live accrual envelope for no-accrual public paths.**  
> Public Live-mode instructions that advance `current_slot` but do NOT call `accrue_market_to` (i.e., do not advance `slot_last`) MUST also require `now_slot <= slot_last + cfg_max_accrual_dt_slots`. Without this bound, a permissionless caller could pick any `now_slot` beyond the envelope, commit the `current_slot` advance, and force subsequent live accrual into a conservative failure. Callers wanting to advance time beyond the envelope MUST go through `accrue_market_to`, which also advances `slot_last`, or through the wrapper’s explicit recovery / resolution path.

### 9.2.1 `deposit_fee_credits(i, amount, now_slot)`

1. require `market_mode == Live`
2. require account `i` is materialized
3. require `now_slot >= current_slot`
3a. require `now_slot <= slot_last + cfg_max_accrual_dt_slots`
4. set `current_slot = now_slot`
5. `pay = min(amount, FeeDebt_i)`
6. if `pay == 0`, return
7. require `V + pay <= MAX_VAULT_TVL`
8. set `V = V + pay`
9. set `I = I + pay`
10. add `pay` to `fee_credits_i`
11. require `fee_credits_i <= 0`
12. require `V >= C_tot + I`

### 9.2.2 `top_up_insurance_fund(amount, now_slot)`

1. require `market_mode == Live`
2. require `now_slot >= current_slot`
2a. require `now_slot <= slot_last + cfg_max_accrual_dt_slots`
3. set `current_slot = now_slot`
4. require `V + amount <= MAX_VAULT_TVL`
5. set `V = V + amount`
6. set `I = I + amount`
7. require `V >= C_tot + I`

### 9.2.3 `charge_account_fee(i, fee_abs, now_slot)`

1. require `market_mode == Live`
2. require account `i` is materialized
3. require `now_slot >= current_slot`
3a. require `now_slot <= slot_last + cfg_max_accrual_dt_slots`
4. require `fee_abs <= MAX_PROTOCOL_FEE_ABS`
5. set `current_slot = now_slot`
6. `charge_fee_to_insurance(i, fee_abs)`
7. require `V >= C_tot + I`

### 9.2.4 `settle_flat_negative_pnl(i, now_slot[, fee_rate_per_slot])`

1. require `market_mode == Live`
2. require account `i` is materialized
3. require `now_slot >= current_slot`
3a. require `now_slot <= slot_last + cfg_max_accrual_dt_slots`
4. set `current_slot = now_slot`
5. if recurring fees are enabled, `sync_account_fee_to_slot(i, current_slot, fee_rate_per_slot)`
6. require `basis_pos_q_i == 0`
7. require `R_i == 0` and both reserve buckets absent
8. if `PNL_i >= 0`, return
9. settle losses from principal
10. if `PNL_i < 0`, absorb protocol loss and set `PNL_i = 0`
11. require `PNL_i == 0`
12. require `V >= C_tot + I`

### 9.3 `withdraw(i, amount, oracle_price, now_slot, funding_rate_e9_per_slot, admit_h_min, admit_h_max, admit_h_max_consumption_threshold_bps_opt[, fee_rate_per_slot])`

1. validate inputs per §9.0 step 1
2. require `market_mode == Live`
3. require account `i` is materialized
4. initialize `ctx`
5. accrue market
6. set `current_slot`
7. if recurring fees are enabled, `sync_account_fee_to_slot(i, current_slot, fee_rate_per_slot)`
8. `touch_account_live_local(i, ctx)`
9. `finalize_touched_accounts_post_live(ctx)`
10. require `amount <= C_i` (no engine-side post-withdraw dust floor; any such floor is wrapper policy)
11. if `effective_pos_q(i) != 0`, require withdrawal health on the hypothetical post-withdraw state where both `V` and `C_tot` decrease by `amount`
12. apply `set_capital(i, C_i - amount)` and `V = V - amount`
13. schedule resets
14. finalize resets
15. assert `OI_eff_long == OI_eff_short`
16. require `V >= C_tot + I`

### 9.3.1 `convert_released_pnl(i, x_req, oracle_price, now_slot, funding_rate_e9_per_slot, admit_h_min, admit_h_max, admit_h_max_consumption_threshold_bps_opt[, fee_rate_per_slot])`

1. validate inputs per §9.0 step 1
2. require `market_mode == Live`
3. require account `i` is materialized
4. initialize `ctx`
5. accrue market
6. set `current_slot`
7. if recurring fees are enabled, `sync_account_fee_to_slot(i, current_slot, fee_rate_per_slot)`
8. `touch_account_live_local(i, ctx)`
9. require `0 < x_req <= ReleasedPos_i`
10. compute current `h`
11. if `basis_pos_q_i == 0`, require `x_req <= max_safe_flat_conversion_released(i, x_req, h_num, h_den)`
12. `consume_released_pnl(i, x_req)`
13. `set_capital(i, C_i + floor(x_req * h_num / h_den))`
14. call `fee_debt_sweep(i)`
15. if `effective_pos_q(i) != 0`, require the post-conversion state is maintenance healthy
16. `finalize_touched_accounts_post_live(ctx)`
17. schedule resets
18. finalize resets
19. assert `OI_eff_long == OI_eff_short`
20. require `V >= C_tot + I`

### 9.4 `execute_trade(a, b, oracle_price, now_slot, funding_rate_e9_per_slot, admit_h_min, admit_h_max, admit_h_max_consumption_threshold_bps_opt, size_q, exec_price[, fee_rate_per_slot_a, fee_rate_per_slot_b])`

`size_q > 0` means account `a` buys base from account `b`.

Procedure:

1. validate inputs per §9.0 step 1
2. require `market_mode == Live`
3. require both accounts are materialized
4. require `a != b`
5. require validated `0 < exec_price <= MAX_ORACLE_PRICE`
6. require `0 < size_q <= MAX_TRADE_SIZE_Q`
7. require `trade_notional <= MAX_ACCOUNT_NOTIONAL`
8. initialize `ctx`
9. accrue market
10. set `current_slot`
11. if recurring fees are enabled, sync `a` and `b` to `current_slot`
12. touch both accounts locally in deterministic ascending storage-index order (`first = min(a, b)`, `second = max(a, b)`)
12a. **pre-open dust/reset flush.** Run `schedule_end_of_instruction_resets` and `finalize_end_of_instruction_resets` against a fresh local reset context (NOT the main instruction `ctx`). This clears any dust-only empty sides created by the live touches in step 12. The flush observes the deterministic touched state from step 12, so cross-client execution is reproducible. Using a separate reset context prevents the main-instruction end-of-instruction pass from re-resetting the freshly opened positions.
13. capture pre-trade effective positions, maintenance requirements, and exact widened raw maintenance buffers
14. finalize any already-ready reset sides before OI increase
15. compute candidate post-trade effective positions
16. require position bounds
17. compute exact bilateral candidate OI after-values
18. enforce `MAX_OI_SIDE_Q`
19. reject any trade that would increase OI on a blocked side
20. compute `trade_pnl_a` and `trade_pnl_b` via `compute_trade_pnl(size_q, oracle_price, exec_price)` and apply execution-slippage PnL before fees:
   - `set_pnl(a, PNL_a + trade_pnl_a, UseAdmissionPair(admit_h_min, admit_h_max), ctx)`
   - `set_pnl(b, PNL_b + trade_pnl_b, UseAdmissionPair(admit_h_min, admit_h_max), ctx)`
21. attach the resulting effective positions
22. write the exact candidate OI after-values
23. settle post-trade losses from principal for both accounts
24. if a resulting effective position is zero, require `PNL_i >= 0` before fees
25. compute and charge explicit trading fees, capturing `fee_equity_impact_a` and `fee_equity_impact_b`
26. compute post-trade `Notional_post_i`, `IM_req_post_i`, `MM_req_post_i`, and `Eq_trade_open_raw_i`
27. enforce post-trade approval independently for both accounts:
   - if resulting effective position is zero, require exact `min(Eq_maint_raw_post_i + fee_equity_impact_i, 0) >= min(Eq_maint_raw_pre_i, 0)`
   - else if risk-increasing, require exact `Eq_trade_open_raw_i >= IM_req_post_i`
   - else if exact maintenance health already holds, allow
   - else if strictly risk-reducing, allow only if both:
     - `((Eq_maint_raw_post_i + fee_equity_impact_i) - MM_req_post_i) > (Eq_maint_raw_pre_i - MM_req_pre_i)`
     - `min(Eq_maint_raw_post_i + fee_equity_impact_i, 0) >= min(Eq_maint_raw_pre_i, 0)`
   - else reject
28. `finalize_touched_accounts_post_live(ctx)`
29. schedule resets
30. finalize resets
31. assert `OI_eff_long == OI_eff_short`
32. require `V >= C_tot + I`

### 9.5 `close_account(i, oracle_price, now_slot, funding_rate_e9_per_slot, admit_h_min, admit_h_max, admit_h_max_consumption_threshold_bps_opt[, fee_rate_per_slot]) -> payout`

Owner-facing close path for a clean live account.

1. validate inputs per §9.0 step 1
2. require `market_mode == Live`
3. require account `i` is materialized
4. initialize `ctx`
5. accrue market
6. set `current_slot`
7. if recurring fees are enabled, `sync_account_fee_to_slot(i, current_slot, fee_rate_per_slot)`
8. `touch_account_live_local(i, ctx)`
9. `finalize_touched_accounts_post_live(ctx)`
10. require `basis_pos_q_i == 0`
11. require `PNL_i == 0`
12. require `R_i == 0` and both reserve buckets absent
13. require `FeeDebt_i == 0`
14. let `payout = C_i`
15. if `payout > 0`:
    - `set_capital(i, 0)`
    - `V = V - payout`
16. free the slot
17. schedule resets
18. finalize resets
19. assert `OI_eff_long == OI_eff_short`
20. require `V >= C_tot + I`
21. return `payout`

### 9.6 `liquidate(i, oracle_price, now_slot, funding_rate_e9_per_slot, admit_h_min, admit_h_max, admit_h_max_consumption_threshold_bps_opt, policy[, fee_rate_per_slot])`

`policy ∈ {FullClose, ExactPartial(q_close_q)}`.

1. validate inputs per §9.0 step 1
2. require `market_mode == Live`
3. require account `i` is materialized
4. initialize `ctx`
5. accrue market
6. set `current_slot`
7. if recurring fees are enabled, `sync_account_fee_to_slot(i, current_slot, fee_rate_per_slot)`
8. touch the account locally
9. require liquidation eligibility
10. execute either exact partial liquidation or full-close liquidation on the already-touched state
11. `finalize_touched_accounts_post_live(ctx)`
12. schedule resets
13. finalize resets
14. assert `OI_eff_long == OI_eff_short`
15. require `V >= C_tot + I`

### 9.7 `keeper_crank(now_slot, oracle_price, funding_rate_e9_per_slot, admit_h_min, admit_h_max, admit_h_max_consumption_threshold_bps_opt, ordered_candidates[], max_revalidations, rr_window_size[, fee_rate_per_slot_fn])`

`ordered_candidates[]` is keeper-supplied and untrusted. It MAY be empty. `rr_window_size` is keeper-supplied and bounds the mandatory Phase 2 round-robin sweep; it MAY be zero, in which case Phase 2 is a no-op. Phase 2 MUST still run conceptually even when `rr_window_size == 0`.

**Phase 1 (spot liquidation)** processes keeper-prioritized candidates.  
**Phase 2 (round-robin structural sweep)** always runs and walks the next `rr_window_size` indices from `rr_cursor_position`.

Procedure:

1. validate inputs per §9.0 step 1
2. require `market_mode == Live`
3. initialize `ctx` with the shared admission pair and `admit_h_max_consumption_threshold_bps_opt`
4. accrue market exactly once
5. set `current_slot = now_slot`

6. **Phase 1: spot liquidation from keeper shortlist.**  
   Iterate `ordered_candidates[]` in keeper-supplied order until `max_revalidations` budget is exhausted or a pending reset is scheduled:
   - stopping at the first scheduled reset is intentional
   - “a pending reset is scheduled” means `ctx.pending_reset_long || ctx.pending_reset_short`
   - missing-account skips do not count
   - touching a materialized account counts against `max_revalidations`
   - if recurring fees are enabled, sync the candidate to `current_slot`
   - `touch_account_live_local(candidate, ctx)`
   - if the account is liquidatable after touch and a current-state-valid liquidation-policy hint is present, execute liquidation on the already-touched state
   - if the account is flat, clean, empty, or dust after that touched state, the wrapper MAY invoke the separate reclaim path in a later instruction
   - after each candidate’s touch/liquidation attempt, if `ctx.pending_reset_long || ctx.pending_reset_short`, break before processing the next candidate

7. **Phase 2: mandatory round-robin structural sweep.**  
   Phase 2 runs unconditionally, including when Phase 1 exited early on a pending reset. Phase 2 does NOT count against `max_revalidations`, does NOT break on pending reset, and does NOT execute liquidations.

   Let `sweep_end = min(cfg_max_accounts, rr_cursor_position + rr_window_size)`, using checked or saturating arithmetic on the addition. For each storage index `i` in `rr_cursor_position .. sweep_end`:
   - if account `i` is missing, skip
   - else:
     - if recurring fees are enabled, sync account `i` to `current_slot`
     - `touch_account_live_local(i, ctx)`

   Set `rr_cursor_position = sweep_end`. If `rr_cursor_position >= cfg_max_accounts`:
   - set `rr_cursor_position = 0`
   - `sweep_generation = checked_add_u64(sweep_generation, 1)`
   - `price_move_consumed_bps_this_generation = 0`

   Phase 2 MUST always run after Phase 1. This ordering gives keeper-prioritized liquidation first claim on compute; Phase 2 fills whatever budget remains.

8. `finalize_touched_accounts_post_live(ctx)`
9. schedule resets
10. finalize resets
11. assert `OI_eff_long == OI_eff_short`
12. require `V >= C_tot + I`

Candidate order in Phase 1 is **keeper policy**. Phase 2 round-robin order is **fixed by the engine**. A malicious keeper supplying `rr_window_size = 0` gains nothing: the cursor does not advance. Compute exhaustion during Phase 2 fails the whole instruction conservatively with no cursor or generation advance persisted. Under §0 atomicity, any later failure in the same top-level instruction also rolls back any tentative `rr_cursor_position`, `sweep_generation`, or `price_move_consumed_bps_this_generation` writes from step 7.

On resolved markets, `keeper_crank` is unavailable; `rr_cursor_position`, `sweep_generation`, and `price_move_consumed_bps_this_generation` are frozen at resolution and are not consulted by resolved-close paths.

### 9.8 `resolve_market(resolve_mode, resolved_price, live_oracle_price, now_slot, funding_rate_e9_per_slot)`

Privileged deployment-owned transition.

`resolve_mode ∈ {Ordinary, Degenerate}` is a trusted wrapper-controlled selector. Value-detected branch selection is forbidden.

This instruction has two privileged branches:

- **ordinary self-synchronizing resolution**, which first accrues the live market state to `now_slot` using the trusted current live oracle price and the wrapper-owned current funding rate, then stores the final settlement mark as separate resolved terminal `K` deltas; and
- **degenerate recovery resolution**, which is available only when the wrapper explicitly selects it and explicitly supplies degenerate live-sync inputs (`live_oracle_price = P_last` and `funding_rate_e9_per_slot = 0`), in which case the instruction resolves directly from the last synchronized live mark and intentionally applies no additional live accrual after `slot_last`.

Procedure:

1. require `market_mode == Live`
2. require `now_slot >= current_slot` and `now_slot >= slot_last`
3. require validated `0 < live_oracle_price <= MAX_ORACLE_PRICE`
4. require validated `0 < resolved_price <= MAX_ORACLE_PRICE`
5. if `resolve_mode == Degenerate`:
   - require `live_oracle_price == P_last`
   - require `funding_rate_e9_per_slot == 0`
   - set `current_slot = now_slot`
   - set `slot_last = now_slot`
   - set `resolved_live_price_candidate = P_last`
   - set `used_degenerate_resolution_branch = true`
6. else if `resolve_mode == Ordinary`:
   - call `accrue_market_to(now_slot, live_oracle_price, funding_rate_e9_per_slot)`
   - set `current_slot = now_slot`
   - set `resolved_live_price_candidate = live_oracle_price`
   - set `used_degenerate_resolution_branch = false`
7. value-based ambiguity is forbidden: if the wrapper wants the ordinary branch, it MUST pass `resolve_mode = Ordinary`, even when `live_oracle_price == P_last` and `funding_rate_e9_per_slot == 0`
8. if `used_degenerate_resolution_branch == false`:
   - require exact settlement-band check:
     - `abs(resolved_price - resolved_live_price_candidate) * 10_000 <= cfg_resolve_price_deviation_bps * resolved_live_price_candidate`
     - both `resolved_live_price_candidate` and `resolved_price` are privileged wrapper-trusted inputs on this path; on the ordinary branch the band is an internal consistency guard, not an independent oracle-integrity proof
9. else:
   - skip the ordinary live-sync settlement band check
   - the degenerate branch relies entirely on trusted wrapper settlement inputs and must be used only when explicitly permitted by the deployment’s settlement policy
10. compute resolved terminal mark deltas in exact checked signed arithmetic:
   - if `mode_long == ResetPending`, set `resolved_k_long_terminal_delta = 0`
   - else compute `resolved_k_long_terminal_delta = A_long * (resolved_price - resolved_live_price_candidate)` and require representable as persistent `i128`
   - if `mode_short == ResetPending`, set `resolved_k_short_terminal_delta = 0`
   - else compute `resolved_k_short_terminal_delta = -A_short * (resolved_price - resolved_live_price_candidate)` and require representable as persistent `i128`
   - these terminal deltas MUST NOT be added into persistent live `K_side`
11. set `market_mode = Resolved`
12. set `resolved_price = resolved_price`
13. set `resolved_live_price = resolved_live_price_candidate`
14. set `resolved_slot = now_slot`
15. clear resolved payout snapshot state explicitly:
   - `resolved_payout_snapshot_ready = false`
   - `resolved_payout_h_num = 0`
   - `resolved_payout_h_den = 0`
16. set `PNL_matured_pos_tot = PNL_pos_tot`
17. set `OI_eff_long = 0` and `OI_eff_short = 0`
18. for each side:
   - if `mode_side != ResetPending`, invoke `begin_full_drain_reset(side)`
   - if the resulting side state is `ResetPending` and `stale_account_count_side == 0` and `stored_pos_count_side == 0`, invoke `finalize_side_reset(side)`
19. require both open-interest sides are zero
20. require `V >= C_tot + I`

Under §0, steps 5 through 20 are one atomic transition. If any check fails — including ordinary live-sync accrual, explicit degenerate-mode validation, terminal-delta representability, or reset-finalization checks — the market remains live and all intermediate writes roll back with the enclosing instruction.

The ordinary branch is the normative path. The degenerate branch exists only to preserve privileged resolution liveness when applying additional live accrual would be impossible or undesirable under the deployment’s explicit settlement policy — for example because the price/funding accrual envelope has already been exceeded or cumulative live `K_side` or `F_side_num` headroom is tight. It is entered only when the wrapper explicitly passes `resolve_mode = Degenerate`.

### 9.9 `force_close_resolved(i)`

Multi-stage resolved-market progress path. Takes only the account index; the engine uses its stored `resolved_slot` as the time anchor and does not accept a caller-supplied slot. Recurring-fee ordering is a wrapper responsibility: deployments with recurring fees enabled MUST call `sync_account_fee_to_slot(i, resolved_slot, fee_rate_per_slot)` before invoking this path, so that `last_fee_slot_i == resolved_slot`. The engine does NOT take a `fee_rate_per_slot` parameter and does NOT gate on `last_fee_slot_i` — the gate is wrapper-owned because the engine does not store the deployment's fee rate.

An implementation MUST expose an explicit outcome distinguishing:

- `ProgressOnly` — local reconciliation progressed but no terminal close occurred yet
- `Closed { payout }` — the account was terminally closed and paid out `payout`

A zero payout MUST NOT be the sole encoding of "not yet closeable."

1. require `market_mode == Resolved`
2. require account `i` is materialized
3. require `current_slot == resolved_slot` (frozen market anchor)
4. `prepare_account_for_resolved_touch(i)`
5. `settle_side_effects_resolved(i)`
6. settle losses from principal if needed
7. resolve uncovered flat loss if needed
8. if `mode_long == ResetPending` and `OI_eff_long == 0` and `stale_account_count_long == 0` and `stored_pos_count_long == 0`, finalize the long side
9. if `mode_short == ResetPending` and `OI_eff_short == 0` and `stale_account_count_short == 0` and `stored_pos_count_short == 0`, finalize the short side
10. require `OI_eff_long == OI_eff_short`
11. if `PNL_i == 0`, return `Closed { payout }` from `force_close_resolved_terminal_nonpositive(i)`
12. if `PNL_i > 0`:
    - if the market is not positive-payout ready:
      - require `V >= C_tot + I`
      - return `ProgressOnly` after persisting the local reconciliation
    - if the shared resolved payout snapshot is not ready, capture it
    - return `Closed { payout }` from `force_close_resolved_terminal_positive(i)`

### 9.10 `reclaim_empty_account(i, now_slot[, fee_rate_per_slot])`

1. require `market_mode == Live`
2. require account `i` is materialized
3. require `now_slot >= current_slot`
3a. require `now_slot <= slot_last + cfg_max_accrual_dt_slots`
4. set `current_slot = now_slot`
5. if recurring fees are enabled, `sync_account_fee_to_slot(i, current_slot, fee_rate_per_slot)`
6. require the flat-clean reclaim preconditions of §2.8
7. require final reclaim eligibility of §2.8
8. execute the reclamation effects of §2.8
9. require `V >= C_tot + I`

---

## 10. Permissionless off-chain shortlist keeper mode

1. The engine does **not** require any on-chain phase-1 search, barrier classifier, or no-false-negative scan proof.
2. `ordered_candidates[]` is keeper-supplied and untrusted. It MAY be stale, incomplete, duplicated, adversarially ordered, or produced by approximate heuristics.
3. Optional liquidation-policy hints are untrusted. They MUST be ignored unless they encode one of the supported policies and pass the same exact current-state validity checks as the normal `liquidate` entrypoint.
4. The protocol MUST NOT require that a keeper discover all currently liquidatable accounts before it may process a useful subset.
5. Because `settle_account`, `liquidate`, `reclaim_empty_account`, and `force_close_resolved` are permissionless, reset progress and dead-account recycling MUST remain possible without any mandatory on-chain scan order.
6. `max_revalidations` caps Phase 1 keeper-priority revalidation on materialized accounts. Phase 2 structural sweep is independently bounded by `rr_window_size` and does not consume `max_revalidations`. Missing-account skips do not count against either budget.
7. Inside `keeper_crank`, both Phase 1 per-candidate touches and Phase 2 per-index touches MUST be economically equivalent to `touch_account_live_local(i, ctx)` on the already-accrued instruction state. Liquidation is Phase 1 only.
8. The only mandatory on-chain ordering constraints are:
   - a single initial accrual
   - Phase 1 candidate processing in keeper-supplied order, stopping on pending reset
   - Phase 2 round-robin processing from `rr_cursor_position`
   - `sweep_generation` advance exactly once per cursor wraparound
   - atomic reset of `price_move_consumed_bps_this_generation` to `0` on wraparound
9. If recurring account fees are enabled, keeper processing MAY exact-touch fee-current state one account at a time in both phases using `last_fee_slot_i`; this is intentional and does not require a global scan.

---

## 11. Required test properties

An implementation MUST include tests covering at least the following.

1. `V >= C_tot + I` always.
2. Positive `set_pnl` increases raise `R_i` by the same delta and do not immediately increase `PNL_matured_pos_tot` unless admitted at `h_eff = 0`.
3. Fresh unwarmed manipulated PnL cannot satisfy withdrawal checks or principal conversion.
4. Aggregate positive PnL admitted through `g` is bounded by `Residual`.
5. `Eq_trade_open_raw_i` exactly neutralizes the candidate trade’s own positive slippage.
6. A trade that only passes because of its own positive slippage is rejected.
7. Fee-debt sweep leaves `Eq_maint_raw_i` unchanged.
8. Pure warmup release does not reduce `Eq_maint_raw_i`.
9. Pure warmup release does not increase `Eq_trade_raw_i`.
10. Pure warmup release can increase `Eq_withdraw_raw_i`.
11. Fresh reserve never inherits elapsed time from an older scheduled bucket.
12. Adding new reserve does not reset or alter the older scheduled bucket’s `sched_start_slot`, `sched_horizon`, `sched_anchor_q`, or already accrued progress.
13. The pending bucket never matures while pending.
14. When promoted, the pending bucket starts fresh at `current_slot` with zero scheduled release.
15. Reserve-loss ordering is newest-first: pending bucket before scheduled bucket.
16. Repeated small reserve additions can only affect the newest pending bucket; they cannot relock the older scheduled bucket.
17. Whole-only automatic flat conversion works only at `h = 1`.
18. No permissionless lossy flat conversion occurs under `h < 1`.
19. `convert_released_pnl` consumes only `ReleasedPos_i` and leaves reserve state unchanged.
20. Flat explicit conversion rejects if the requested amount exceeds `max_safe_flat_conversion_released`.
21. Same-epoch local settlement is prefix-independent.
22. Repeated same-epoch touches without explicit position mutation do not compound quantity-flooring loss.
23. Phantom-dust bounds conservatively cover same-epoch zeroing, basis replacements, and ADL multiplier truncation.
24. Dust-clear scheduling and reset initiation happen only at end of top-level instructions.
25. Epoch gaps larger than one are rejected as corruption.
26. If `A_candidate == 0` with `OI_post > 0`, the engine force-drains both sides instead of reverting.
27. If ADL `delta_K_abs` is non-representable or `K_opp + delta_K_exact` overflows, quantity socialization still proceeds and the remainder routes through `record_uninsured_protocol_loss`.
28. `enqueue_adl` spends the full insurance balance before any remaining bankruptcy loss is socialized or left as junior undercollateralization.
29. The exact ADL dust-bound increment matches §5.6 step 10 and the unilateral and bilateral dust-clear conditions match §5.7 exactly.
30. Funding accrual uses exact 256-bit-or-equivalent intermediates for both `fund_num_total` and each `A_side * fund_num_total` product, with symmetry preserved.
31. A flat account with negative `PNL_i` resolves through `absorb_protocol_loss` only in the allowed already-authoritative flat-account paths.
32. Reset finalization reopens a side once `ResetPending` preconditions are fully satisfied.
33. `deposit` settles realized losses before fee sweep.
34. A missing account cannot be materialized by a deposit of amount `0`. (Any higher minimum-deposit floor is wrapper policy.)
35. The strict risk-reducing trade exemption uses exact widened raw maintenance buffers and exact widened raw maintenance shortfall.
36. The strict risk-reducing trade exemption adds back `fee_equity_impact_i`, not nominal fee.
37. Any side-count increment — including a sign flip — enforces `cfg_max_active_positions_per_side`.
38. A flat trade cannot bypass ADL by leaving negative `PNL_i` behind.
39. Live flat dust accounts can be reclaimed safely.
40. Missing-account safety: ordinary live and resolved paths do not auto-materialize missing accounts.
41. `keeper_crank` accrues the market exactly once per instruction, before both Phase 1 and Phase 2.
42. The Phase 1 per-candidate keeper touch is economically equivalent to `touch_account_live_local`.
43. `max_revalidations` counts only normal exact Phase 1 revalidation attempts on materialized accounts.
44. `deposit_fee_credits` applies only `min(amount, FeeDebt_i)` and never makes `fee_credits_i` positive.
45. `charge_account_fee` mutates only capital, fee debt, and insurance through canonical helpers.
46. Trade-opening health and withdrawal health are distinct lanes.
47. Once resolved, all remaining positive PnL is globally treated as matured.
48. `prepare_account_for_resolved_touch(i)` clears local reserve state without a second global aggregate change.
49. No positive resolved payout occurs until stale-account reconciliation is complete across both sides and the shared payout snapshot is locked.
50. A resolved account with `PNL_i <= 0` can close immediately after local reconciliation, even while unrelated positive claims are still waiting for the shared snapshot.
51. Every positive terminal resolved close uses the same captured resolved payout snapshot.
52. Live instructions reject invalid admission pairs and invalid `funding_rate_e9_per_slot`.
53. `deposit`, `deposit_fee_credits`, `top_up_insurance_fund`, and `charge_account_fee` do not draw insurance.
54. `settle_flat_negative_pnl` is a live-only permissionless cleanup path that does not mutate side state.
55. On its ordinary branch, `resolve_market(Ordinary, ...)` self-synchronizes live accrual to `now_slot` and stores the final settlement mark as separate resolved terminal deltas.
56. On its ordinary branch, `resolve_market(Ordinary, ...)` rejects settlement prices outside the immutable band around the trusted live-sync price used for that instruction; on its degenerate branch, that ordinary live-sync band check is intentionally bypassed.
57. Resolved local reconciliation applies the stored `resolved_k_*_terminal_delta` exactly on sides that were still live at resolution, and applies zero terminal delta on sides that were already `ResetPending`.
58. Under open-interest symmetry, end-of-instruction reset scheduling preserves `OI_eff_long == OI_eff_short`.
59. Positive resolved payouts do not begin until the market is positive-payout ready per §6.7.
60. `neg_pnl_account_count` exactly matches iteration over materialized accounts with `PNL_i < 0` after every path that mutates `PNL_i`.
61. The touched-account set and instruction-local `h_max` sticky state cannot silently drop an account; if capacity would be exceeded, the instruction fails conservatively.
62. Whole-only automatic flat conversion in §6.6 uses the exact helper sequence `consume_released_pnl` then `set_capital`.
63. `force_close_resolved` exposes an explicit progress-versus-close outcome; a zero payout is never the sole encoding of “not yet closeable.”
64. The positive resolved-close path fails conservatively, not permissively, if a snapshot is marked ready with a zero payout denominator while some account still has `PNL_i > 0`.
65. `advance_profit_warmup` clamps `elapsed` at `sched_horizon` and therefore does not fail merely because an unclamped quotient would exceed `u128`.
66. Live positive reserve creation cannot use `ImmediateReleaseResolvedOnly`.
67. Within one instruction, once an account requires `admit_h_max`, later fresh positive increases on that account also use `admit_h_max`; an earlier newest pending increment may be conservatively lifted, but under-admission is forbidden.
68. `admit_outstanding_reserve_on_touch` either accelerates all outstanding reserve or leaves it unchanged; it never extends or resets reserve horizons.
69. A live-accrual instruction with price-moving exposure or active funding and `dt > cfg_max_accrual_dt_slots` fails conservatively; privileged `resolve_market` may proceed only through its explicit degenerate branch.
70. Market initialization rejects any `(cfg_max_abs_funding_e9_per_slot, cfg_max_accrual_dt_slots)` pair that violates the exact funding-envelope inequality.
71. `resolve_market(Degenerate, ...)` requires `live_oracle_price = P_last` and `funding_rate_e9_per_slot = 0`; `resolve_market(Ordinary, ...)` MUST stay on the ordinary branch even when those values happen to coincide.
72. A voluntary trade that closes an account exactly to flat is not rejected solely because current-trade fees create or increase fee debt; the zero-position branch uses the same fee-neutral shortfall-comparison principle as strict risk reduction.
73. `max_safe_flat_conversion_released` uses 256-bit-or-equivalent arithmetic and does not silently overflow on `E_before * h_den`.
74. Candidate ordering in `keeper_crank` may affect warmup UX but not solvency, conservation, or correctness.
75. Resolved local reconciliation may exceed live-only caps while still failing conservatively on any `u128` aggregate overflow.
76. `close_account` cannot be used to forgive unpaid fee debt; unresolved debt must be repaid or reclaimed through the dust path.
77. After any `A_side` decay in ADL, any mismatch between authoritative `OI_eff_side` and summed per-account same-epoch floor quantities is bounded and resolved only through explicit phantom-dust rules.
78. Long-running markets with little matured-PnL extraction eventually see `Residual` become scarce relative to `PNL_matured_pos_tot`, causing fresh reserve admission to select slower horizons more often; this is operationally visible but must never break safety or correctness.
79. A newly materialized account sets `last_fee_slot_i = materialize_slot` and is never charged for earlier time.
80. `sync_account_fee_to_slot(i, t, r)` charges exactly once over `[last_fee_slot_i, t]`, advances `last_fee_slot_i` to `t`, and a second sync at the same `t` is a no-op.
81. `last_fee_slot_i <= resolved_slot` holds for all materialized accounts on resolved markets.
82. Resolved recurring-fee sync uses `resolved_slot`, not later wall-clock time.
83. Capturing the resolved payout snapshot before some accounts are fee-current does not invalidate later payouts because late fee sync is a pure `C -> I` reclassification.
84. If `advance_profit_warmup` empties the scheduled bucket in a frame where `sched_total > sched_release_q`, the bucket is cleared immediately; no non-empty bucket can persist with an over-advanced `sched_release_q`.
85. `resolve_market(Ordinary, ...)` does not silently fall into the degenerate branch when `live_oracle_price == P_last` and `funding_rate_e9_per_slot == 0`; explicit `resolve_mode` controls branch selection.
86. `sync_account_fee_to_slot(i, t, r)` caps to `MAX_PROTOCOL_FEE_ABS` and advances `last_fee_slot_i` even when the uncapped raw product `r * (t - last_fee_slot_i)` exceeds native `u128`.
87. Same-epoch basis replacement with nonzero orphan remainder increments the relevant `phantom_dust_bound_*_q` by exactly `1` q-unit.
88. Same-epoch live settlement with `q_eff_new == 0` increments the relevant `phantom_dust_bound_*_q` by exactly `1` q-unit before basis reset.
89. `accrue_market_to` rejects any call where live exposure exists and `abs(oracle_price - P_last) * 10_000 > cfg_max_price_move_bps_per_slot * dt * P_last`. The rejection fires before any `K_side`, `F_side_num`, `P_last`, `fund_px_last`, or `slot_last` mutation, and the market remains live and accruable at the previous state. The same property MUST cover the zero-funding/open-OI case: if live exposure exists, `oracle_price != P_last`, and `dt > cfg_max_accrual_dt_slots`, the call rejects even when `funding_rate_e9_per_slot == 0`. A separate witness MUST cover zero-OI fast-forward and show that arbitrary idle-gap price updates remain permitted when no live exposure exists.
90. Market initialization rejects any parameter set that violates `cfg_max_price_move_bps_per_slot * cfg_max_accrual_dt_slots + floor(cfg_max_abs_funding_e9_per_slot * cfg_max_accrual_dt_slots * 10_000 / FUNDING_DEN) + cfg_liquidation_fee_bps > cfg_maintenance_bps`.
91. Self-neutral insurance-siphon resistance: given any two materialized accounts with distinct owners and any bilateral-trade setup, and given any sequence of valid `accrue_market_to` calls that together advance `P_last` by cumulative fraction `Δ` over `N` slots, the sum of attacker-controlled `(C_i + PNL_i)` minus the sum of attacker deposits is bounded below by `-Σ liquidation_fees_i` and cannot be net-positive due to insurance loss. The test MUST witness this on the A1 setup with a staircase price path and confirm `attacker_delta <= 0` holds across multiple accrual envelopes with liquidations interleaved.
92. `keeper_crank` always executes Phase 2 after Phase 1, including when Phase 1 exited early on a pending reset. An empty `ordered_candidates[]` with `rr_window_size > 0` is a valid structural-sweep-only instruction.
93. Phase 2 advances `rr_cursor_position` by exactly `min(rr_window_size, cfg_max_accounts - rr_cursor_position)` per successful call.
94. When `rr_cursor_position` reaches `cfg_max_accounts`, it wraps to `0`, `sweep_generation` increments by exactly `1`, and `price_move_consumed_bps_this_generation` resets to `0` atomically with the wrap.
95. Phase 2 does NOT consume `max_revalidations` budget.
96. Phase 2 does NOT execute liquidations; it only calls `touch_account_live_local`. An account discovered as liquidatable during Phase 2 remains liquidatable for Phase 1 processing in the next instruction.
97. `price_move_consumed_bps_this_generation` is monotone nondecreasing within a generation, zeroed exactly on generation advance, and reflects `Σ consumed_this_step` from all accruals since the last wraparound.
98. A keeper supplying `rr_window_size = 0` does not advance the cursor or generation; the call is otherwise valid. A keeper supplying `rr_window_size` that exhausts compute fails the whole instruction conservatively with no cursor advance persisted.
99. When `admit_h_max_consumption_threshold_bps_opt = Some(threshold)` and `price_move_consumed_bps_this_generation >= threshold`, `admit_fresh_reserve_h_lock` returns `admit_h_max` regardless of `Residual_now` or `matured_plus_fresh`.
100. When `price_move_consumed_bps_this_generation` resets to `0` on `sweep_generation` advance, the next fresh admission returns `admit_h_min` if the residual-scarcity condition is satisfied, even if consumption had previously exceeded the threshold.
101. Passing `admit_h_max_consumption_threshold_bps_opt = None` disables the consumption-threshold gate entirely; passing `Some(0)` is invalid and the instruction rejects conservatively.
102. Phase 2 touches flow through `finalize_touched_accounts_post_live` with the same whole-snapshot check and fee-sweep pass as Phase 1 touches.
103. Phase 2 during pre-existing `mode_s == ResetPending` correctly reconciles stale accounts via the epoch-mismatch branch in §5.3, decrementing `stale_account_count_s` as expected.
104. `reclaim_empty_account` rejects if `now_slot > slot_last + cfg_max_accrual_dt_slots` and, on rejection, does not advance `current_slot`.
105. `price_move_consumed_bps_this_generation` uses floor rather than ceil: if `abs_delta_price * 10_000 < P_last`, the step contributes `0` bps to the accumulator rather than rounding up to `1`.
106. If Phase 2 touches a flat negative account that normalizes through §6.3, `neg_pnl_account_count` decrements exactly once and remains globally consistent with the materialized-account scan after the instruction.
107. When `admit_h_min == 0` and `admit_h_max_consumption_threshold_bps_opt = None`, Phase 2 may immediately mature fresh positive PnL across many touched accounts in one instruction, but all engine invariants still hold (`V >= C_tot + I`, `PNL_matured_pos_tot <= PNL_pos_tot`, and the goal-52 accrual-envelope safety property). Public or permissionless wrappers are non-compliant if they expose this combination per §12.21.
108. `execute_trade` touches its two counterparties, and runs the pre-open dust/reset flush over the resulting touched state, in deterministic ascending storage-index order; cross-client order differences are forbidden because one touch may change `PNL_matured_pos_tot` and therefore the second account’s admission outcome.

---

## 12. Wrapper obligations (deployment layer, not engine-checked)

The following are deployment-wrapper obligations.

1. **Do not expose caller-controlled live policy inputs.**  
   `(admit_h_min, admit_h_max)`, `admit_h_max_consumption_threshold_bps_opt`, and `funding_rate_e9_per_slot` are wrapper-owned internal inputs. Public or permissionless wrappers MUST derive them internally and MUST NOT accept arbitrary caller-chosen values.

2. **Authority-gate market resolution and supply trusted inputs for both ordinary and degenerate branches.**  
   `resolve_market` is a privileged deployment-owned transition. A compliant wrapper MUST source both `live_oracle_price` and `resolved_price` from the deployment’s trusted settlement sources or policy, MUST source the wrapper-owned current funding rate used for the ordinary live-sync leg inside `resolve_market`, and MUST pass an explicit trusted `resolve_mode ∈ {Ordinary, Degenerate}` selector. For normal resolution it MUST pass `resolve_mode = Ordinary`. If it intentionally uses the degenerate recovery branch, it MUST pass `resolve_mode = Degenerate`, `live_oracle_price = P_last`, and `funding_rate_e9_per_slot = 0`, and it MUST do so only when that behavior is explicitly permitted by the deployment’s settlement policy.

3. **Do not emulate resolution with a separate prior accrual transaction as the normal path.**  
   Because `resolve_market` is self-synchronizing in this revision, a compliant wrapper MUST invoke it directly with trusted live-sync inputs and `resolve_mode = Ordinary` for ordinary operation. A separate pre-accrual transaction is not required and MUST NOT be treated as the normative path, though a deployment MAY use an explicit pre-accrual or headroom-management flow as an operational recovery tool if it is trying to avoid cumulative `K` or `F` saturation before resolution. If live accrual would still be unsafe or impossible, the wrapper MAY instead use the privileged degenerate branch inside `resolve_market` by explicitly passing `resolve_mode = Degenerate`.

4. **Respect the funding and price-move envelopes operationally.**  
   A compliant deployment MUST monitor `slot_last`, `cfg_max_accrual_dt_slots`, `cfg_max_abs_funding_e9_per_slot`, and `cfg_max_price_move_bps_per_slot` so the market is actively cranked or ordinarily resolved before a live-exposure accrual exceeds the engine envelope. If the deployment enables permissionless stale resolution, it MUST choose `permissionless_resolve_stale_slots <= cfg_max_accrual_dt_slots`. If a price-moving or funding-active exposed market exceeds the envelope anyway, the wrapper is in recovery / resolution territory and the privileged degenerate branch may become the only safe path.

   The price-move envelope is part of the same safety boundary. A compliant wrapper MUST NOT rely on unbounded oracle updates; its oracle policy MUST produce per-slot moves within `cfg_max_price_move_bps_per_slot`. If the configured oracle can legitimately move faster than that under normal market conditions, the deployment has configured `cfg_max_price_move_bps_per_slot` too tightly, or `cfg_maintenance_bps` too low, for that oracle; the init-time inequality in §1.4 catches the parameter inconsistency. If the oracle produces a move exceeding the cap in production, `accrue_market_to` fails conservatively and the market enters a bricked state where explicit recovery or `resolve_market(Degenerate, ...)` is required. This is a feature: a move exceeding the cap is either an oracle compromise or a market event severe enough to warrant resolution; the brick prevents the self-neutral insurance-siphon class from exploiting the gap.

   The only exemption from the dt envelope is when both `funding_active` and `price_move_active` are false: zero-OI markets, or any market state with `funding_rate_e9_per_slot == 0` and `oracle_price == P_last`. Unilateral-OI is not separately exempt; it is only exempt from the funding branch, and remains dt-bounded whenever the oracle price changes.

4a. **Cumulative `F_side_num` is bounded by `cfg_min_funding_lifetime_slots`.**  
   The per-call envelope (§1.4) bounds *one* accrual's F delta to fit `i128`, but persisted `F_long_num` and `F_short_num` accumulate across calls. Initialization (§1.4) enforces a cumulative lifetime floor: at sustained worst-case rate `cfg_max_abs_funding_e9_per_slot` on both sides, F stays within `i128` for at least `cfg_min_funding_lifetime_slots` slots. Deployments MUST choose this parameter to cover their intended market horizon.

   - At realistic operating rates, the observed saturation horizon is usually longer than this floor — years to decades in many deployments. The floor is a worst-case guarantee, not an expected lifetime.
   - Deployments that intend to run at or near `cfg_max_abs_funding_e9_per_slot` as an operating rate MUST either accept that cumulative saturation will eventually require `resolve_market`, or implement a periodic market-rollover / settlement cycle shorter than the saturation horizon.
   - A future engine revision MAY widen persisted `F_side_num` to an exact 256-bit signed domain or introduce a lazy F-renormalization to eliminate this bound. Until then, `cfg_min_funding_lifetime_slots` is the init-enforced lower bound on market lifetime at the configured rate ceiling.

5. **Public wrappers SHOULD enforce execution-price admissibility.**  
   A sufficient rule is `abs(exec_price - oracle_price) * 10_000 <= max_trade_price_deviation_bps * oracle_price`, with `max_trade_price_deviation_bps <= 2 * cfg_trading_fee_bps`.

6. **Use oracle notional for wrapper-side exposure ranking.**

7. **Keep user-owned value-moving operations account-authorized.**  
   User-owned value-moving paths include `deposit`, `withdraw`, `execute_trade`, `close_account`, and `convert_released_pnl`. Intended permissionless progress paths are `settle_account`, `liquidate`, `reclaim_empty_account`, `settle_flat_negative_pnl`, `force_close_resolved`, and `keeper_crank`.

8. **Do not expose pure wrapper-owned account fees carelessly.**  
   `charge_account_fee` performs no maintenance gating of its own. A compliant public wrapper MUST either restrict it to already-safe contexts or pair it with a same-instruction live-touch health-check flow when used on accounts that may still carry live risk.

9. **If desired, tighten the dropped-fee policy above the engine.**  
   The core engine’s strict risk-reducing comparison is defined by actual `fee_equity_impact_i` only. A deployment that wishes to reject strict risk-reducing trades whenever `fee_dropped_i > 0` MAY impose that stricter wrapper rule above the engine.

10. **Provide a post-snapshot resolved-close progress path.**  
    Because `force_close_resolved` is intentionally multi-stage, a compliant deployment SHOULD provide either a self-service retry path or a permissionless batch or incentive path that sweeps positive resolved accounts after the shared payout snapshot is ready.

11. **Wrapper is responsible for anti-spam on account materialization.**  
    The engine only rejects `amount == 0` at materialization; any higher minimum-deposit floor is wrapper policy. A compliant deployment MUST enforce a minimum deposit large enough that exhausting the configured materialized-account capacity is economically prohibitive, paired with a recurring maintenance fee that erodes account capital over time. Together, the wrapper-owned minimum deposit plus recurring fees plus `reclaim_empty_account` (§9.10) give the deployment a complete anti-spam mechanism: materialization has a real capital cost, fees erode it, and the engine recycles fully-drained slots.

12. **Size runtime batches to actual compute limits.**  
    On constrained runtimes, a compliant deployment MUST choose `max_revalidations`, batch-close sizes, and any wrapper-side multi-account composition so one instruction fits the runtime’s per-instruction compute budget.

13. **Plan market lifecycle before K/F headroom exhaustion.**  
    A compliant deployment SHOULD monitor cumulative `K_side` and `F_side_num` headroom and resolve or migrate the market before approaching persistent `i128` saturation.

14. **If more throughput is required than one market state can provide, shard at the deployment layer.**  
    One market instance serializes writes by design. A deployment that requires higher throughput SHOULD shard across multiple market instances rather than assuming runtime-level parallelism inside one market.

15. **If deterministic keeper UX is desired, canonicalize candidate order.**  
    The engine intentionally treats keeper candidate order as policy. A deployment that wants deterministic warmup-admission or acceleration UX across keepers SHOULD canonicalize `ordered_candidates[]`, for example by ascending storage index after off-chain risk bucketing.

16. **Surface matured-pool saturation to users.**  
    In long-running markets where users do not convert or withdraw matured profit, `PNL_matured_pos_tot` can grow close to `Residual`, causing fresh reserve admission to select slower horizons more often. Deployments SHOULD surface this state in UI and MAY prompt users to settle or extract matured claims when appropriate.

17. **Provide an operator recovery path for impossible invariant-breach orphans if the deployment requires one.**  
    The core engine intentionally fails conservatively if resolved reconciliation encounters a state that violates the epoch-gap or reset invariants. A deployment that wants an explicit operational escape hatch for such impossible states SHOULD provide a privileged migration or recovery path above the engine rather than weakening the engine’s conservative-failure rules.

18. **If the deployment enables wrapper-owned recurring account fees, sync before health-sensitive checks.**  
    A compliant wrapper MUST sync recurring fees to the relevant anchor before using an account’s touched state for:
    - live maintenance checks,
    - live liquidation eligibility,
    - reclaim eligibility,
    - resolved terminal close,
    - any user-facing action whose correctness depends on up-to-date fee debt.

19. **Use `resolved_slot` as the recurring-fee anchor on resolved markets.**  
    A compliant wrapper MUST NOT accrue recurring account fees past `resolved_slot`.

20. **Anchor new accounts correctly.**  
    A compliant wrapper MUST materialize new accounts using their actual creation slot as `materialize_slot`, so `last_fee_slot_i` starts at the right point.

21. **Stress-scaled admission threshold is optional in the engine interface but mandatory for public immediate-release deployments.**  
    A compliant public or permissionless wrapper MUST NOT combine `admit_h_min == 0` with `admit_h_max_consumption_threshold_bps_opt = None`. It MUST either:
    - pass `Some(threshold)` with `threshold > 0`, or
    - choose `admit_h_min > 0`.

    `None` is the disable form; `Some(0)` is invalid and MUST be rejected conservatively. Wrappers that use `Some(threshold)` SHOULD usually size it below the per-envelope cap so the gate triggers during sustained volatility without waiting for the cap itself to trip and brick the market. Wrappers that intend to disable the gate SHOULD pass `None` explicitly rather than a pathologically large `Some(threshold)` that behaves like a quiet de-facto disable over any practical sweep horizon while obscuring intent.

22. **Threshold, sweep cadence, and deployment size MUST be sized together.**  
    A wrapper that opts into the consumption-threshold gate MUST choose `rr_window_size`, crank cadence, deployment size, and threshold so a full structural sweep completes within its intended fast-lane recovery horizon. Very large deployments can otherwise leave `price_move_consumed_bps_this_generation` above threshold for long periods, making `admit_h_min` effectively unavailable. If a deployment cannot sweep quickly enough, it SHOULD shard, increase sweep cadence or window size, raise the threshold, or disable the gate and rely on nonzero `admit_h_min`.

23. **Runtime configuration MUST fit touched-account capacity and compute budget.**  
    A compliant wrapper / runtime MUST bound `max_revalidations + rr_window_size` so the resulting touched-account set fits the implementation’s actual `ctx` capacity and per-instruction compute budget. The theoretical spec hard bound `cfg_max_accounts` is not a practical per-instruction context size on constrained runtimes; oversized instructions MUST fail conservatively before partial mutation.

---

## 13. Operational notes (non-normative)

1. **Wide exact arithmetic costs compute.** Exact 256-bit-or-equivalent multiply-divide and signed floor arithmetic are materially more expensive than native 128-bit operations. Keepers and wrappers should use bounded candidate sets and avoid oversized multi-account transactions.

2. **One market account serializes one market.** Because core instructions update shared market aggregates (`V`, `I`, `C_tot`, `PNL_pos_tot`, `A_side`, `K_side`, `F_side_num`, and so on), one market instance is throughput-serialized by design.

3. **Account-capacity griefing is economic, not mathematical.** Anti-spam on account materialization is a wrapper concern: the engine only rejects `amount == 0` at materialization. A compliant wrapper combines (a) a wrapper-enforced minimum deposit, (b) a wrapper-enforced recurring maintenance fee (§4.6.1, §7.3) that erodes account capital over time, and (c) the engine’s `reclaim_empty_account` (§9.10) which recycles fully-drained slots. Together, those three give the deployment a complete anti-spam mechanism. The engine provides the primitives; the wrapper chooses the economic parameters.

4. **Resolution paths should stay thin.** Even though `resolve_market` is self-synchronizing, wrappers should keep the resolution path small in transaction size and compute. Precompute external checks off chain where possible, avoid unnecessary CPI fanout in the same transaction, and remember that the settlement band checks consistency between wrapper-trusted prices rather than supplying an independent oracle guarantee.

5. **Multi-instruction keeper progress is normal.** Because `keeper_crank` intentionally stops further live-OI-dependent processing once a reset is pending, volatile periods may require multiple successive keeper instructions.

6. **Batch positive resolved closes are recommended when practical.** The engine defines exact single-account progress and terminal-close semantics. Deployments that expect many resolved accounts should strongly consider a batched wrapper or incentive path for post-snapshot sweeping to reduce transaction overhead.

7. **Funding and price-move envelopes are engine safety boundaries, not only wrapper preferences.** The engine-enforced tuple `(cfg_max_abs_funding_e9_per_slot, cfg_max_accrual_dt_slots, cfg_max_price_move_bps_per_slot)` prevents dormant-market funding accrual overflow and one-envelope price moves that can siphon insurance. Wrapper policy should stay comfortably inside those envelopes; if an envelope is exceeded anyway, only explicit recovery or the privileged degenerate branch of `resolve_market` remains live.

8. **The recurring-fee checkpoint is intentionally local.** `last_fee_slot_i` is the minimal extra state needed to make touched-account recurring fees exact. It avoids a global fee scan, but it means fee freshness is per account, not globally uniform.

9. **Late resolved fee sync is harmless to payout ratios.** Once the resolved payout snapshot is captured, late fee sync only moves value from `C_i` to `I`. That preserves `Residual = V - (C_tot + I)`. Any uncollectible tail that is dropped stays as conservative unused slack; it is not socialized through payouts.

10. **Monotone pending-bucket max-horizon merge is deliberate.** Coalescing into the newest pending bucket by `max(pending_horizon_i, admitted_h_eff)` is intentionally conservative. It can delay newer-bucket maturity but it never accelerates it and never contaminates the older scheduled bucket.

11. **Price-move cap as a safety circuit breaker.** The §1.4 inequality `cfg_max_price_move_bps_per_slot * cfg_max_accrual_dt_slots + funding_drain_bps_per_envelope + cfg_liquidation_fee_bps <= cfg_maintenance_bps` is what prevents the A1-class self-neutral insurance siphon. The cap applies per accrual, not per trade, so frequent crank activity does not "build up" price headroom — each `accrue_market_to` call is bounded by its own `dt` times the per-slot cap, and price-moving open-interest accruals cannot use `dt > cfg_max_accrual_dt_slots`. The new `sweep_generation` / consumption mechanism of note 14 does **not** replenish safety headroom; it only throttles fresh reserve admission under stress. The per-accrual price cap remains the sole construction-level safety boundary.

12. **Cumulative funding lifetime is a deployment budget.** The init bound `ADL_ONE * MAX_ORACLE_PRICE * cfg_max_abs_funding_e9_per_slot * cfg_min_funding_lifetime_slots <= i128::MAX` gives a worst-case floor on how long persisted `F_side_num` can accumulate at the configured rate ceiling. With `ADL_ONE = 1e15`, `MAX_ORACLE_PRICE = 1e12`, and `cfg_max_abs_funding_e9_per_slot = 10_000`, the floor is about `1.7e7` slots — roughly 2.6 months at 400ms slots. Lower ceilings stretch the floor linearly: `1_000` gives about 2.15 years, `100` about 21.5 years, and `10` about 215 years. Real operating funding is usually far below the configured ceiling, so observed horizons are generally much longer than this worst-case floor.

13. **No-accrual public paths need heartbeat accruals after long inactivity.** The live no-accrual endpoints in §9.2–§9.2.4 and §9.10 require `now_slot <= slot_last + cfg_max_accrual_dt_slots`. After inactivity longer than that, pure-capital or reclaim calls remain blocked until some accruing instruction advances `slot_last`. Wrappers SHOULD run a heartbeat `keeper_crank` (or equivalent accruing instruction) at roughly half the configured envelope so users do not encounter this gate in normal operation. On zero-OI markets the heartbeat can fast-forward immediately; on exposed markets it must stay within the ordinary price/funding envelope or the deployment must resolve.

14. **Sweep-generation stress-scaled admission is engine-enforced when opted in.** The engine tracks `sweep_generation` and `price_move_consumed_bps_this_generation` as persistent state. Wrappers pass `admit_h_max_consumption_threshold_bps_opt` to every admission-creating instruction. When `admit_h_max_consumption_threshold_bps_opt = Some(threshold)` and cumulative consumption since the last generation advance reaches or exceeds `threshold`, the engine’s `admit_fresh_reserve_h_lock` forces `admit_h_max` regardless of residual state. On cursor wraparound in §9.7 Phase 2, consumption resets and the gate auto-relaxes. `None` is the disable form; `Some(0)` is invalid. In public or permissionless deployments that also use `admit_h_min == 0`, `None` is wrapper-prohibited by §12.21. Wrappers that intend to disable the gate SHOULD use `None` rather than a pathologically large `Some(threshold)` that only behaves like a quiet de-facto disable. The per-envelope price-move cap of goal 52 still provides the construction-level safety guarantee in either case. See note 15 for the deployment-size caveat: on large deployments this auto-relaxation cadence can be much slower than one envelope.

A malicious keeper can advance `sweep_generation` themselves by running `keeper_crank` repeatedly, but they can do so only by paying to execute the protocol’s mandatory round-robin work. They do not forge the signal; they earn it by doing the sweep. The per-envelope cap is independent of `sweep_generation` and still rejects adversarial accruals regardless of refill state.

The consumption threshold and the existing residual-scarcity check in §4.7 compose cleanly. Residual scarcity catches “admission would break `h = 1` right now” — reactive. The consumption threshold catches “recent volatility means reconciliation may still be incomplete” — predictive. Either trigger forces `admit_h_max`.

15. **Large deployments stretch generation turnover.** Because `sweep_generation` advances only on full cursor wraparound past `cfg_max_accounts`, a very large deployment with a small `rr_window_size` can keep `price_move_consumed_bps_this_generation` above threshold for long periods, making the fast lane effectively unavailable even though safety remains intact. This is a deployment-sizing issue, not a safety bug. Wrappers that want fast auto-relaxation SHOULD shard, increase `rr_window_size`, increase crank cadence, or choose a higher threshold.

---

## 14. Parameter guidance (non-normative)

For a typical Solana deployment at 400ms slots with:

- `cfg_maintenance_bps = 500`
- `cfg_liquidation_fee_bps = 50`
- `cfg_max_accrual_dt_slots = 100` (about 40 seconds at 400ms slots)
- `cfg_max_abs_funding_e9_per_slot = 10` (tight funding ceiling)

the funding budget is:

```text
funding_drain_bps_per_envelope = floor(10 * 100 * 10_000 / 1_000_000_000) = 0 bps
```

The available price budget is then:

```text
price_budget_bps = 500 - 50 - 0 = 450 bps over 100 slots
```

So a conservative cap is:

```text
cfg_max_price_move_bps_per_slot = 4
4 * 100 = 400 bps <= 450 ✓
```

At 4 bps/slot:

- the market tolerates about 1% price movement over 10 seconds (25 slots at 400ms/slot), and
- about 4% over the full 40-second / 100-slot envelope.

A deployment that needs to tolerate a 10% move over 10 seconds at 400ms slots would need roughly 40 bps/slot, which in turn would require a much shorter envelope, a materially higher maintenance margin, a lower liquidation fee, or some combination of those. Operators should calibrate this parameter against empirical oracle behavior and market-design goals rather than copy the example blindly.

If the deployment instead runs at the funding ceiling for the same envelope:

```text
cfg_max_abs_funding_e9_per_slot = 10_000
funding_drain_bps_per_envelope = floor(10_000 * 100 * 10_000 / 1_000_000_000) = 10 bps
price_budget_bps = 500 - 50 - 10 = 440 bps over 100 slots
cfg_max_price_move_bps_per_slot = 4
4 * 100 = 400 bps <= 440 ✓
```

This makes the three-term inequality concrete: aggressive funding ceilings consume real price budget even when the resulting haircut is still operationally loose.

For a higher-leverage market with `cfg_maintenance_bps = 200` and `cfg_liquidation_fee_bps = 20`, the envelope budget tightens to roughly 180 bps per envelope before funding. At a 100-slot envelope this implies `cfg_max_price_move_bps_per_slot = 1` if the deployment wants a simple integer-bps cap with slack. This is the design tradeoff: aggressive leverage forces either tighter price caps or shorter accrual envelopes.

### Round-robin sweep sizing and threshold selection

A full round-robin sweep of `M` indices costs approximately:

```text
full_sweep_cu ≈ M * touch_cu
```

where `touch_cu` is the realized compute of one `touch_account_live_local` plus any optional fee sync. With per-touch compute around 5k-10k units, a compact 4096-slot deployment or shard costs roughly 20-40M CU for a literal full wraparound. Under a 1.4M per-instruction compute limit, if Phase 1 typically consumes 300k-800k CU, Phase 2 has room for roughly 60-220 touches per call.

That yields a rough wraparound time of:

```text
calls_per_generation ≈ ceil(M / rr_window_size)
generation_time ≈ calls_per_generation * crank_interval
```

So, for a **compact 4096-slot deployment or shard**, generation can advance on the order of every 10-60 seconds under active cranking. For a deployment that really uses the full spec hard bound `cfg_max_accounts = 1_000_000`, generation advances much more slowly unless keepers use very large `rr_window_size`. That is a UX consideration, not a safety issue: the per-envelope cap still enforces goal 52 even if the generation signal turns over slowly.

For `admit_h_max_consumption_threshold_bps_opt = Some(threshold_bps)`, a reasonable starting point is about 50% of the per-envelope cap:

```text
threshold_bps ≈ 0.5 * cfg_max_price_move_bps_per_slot * cfg_max_accrual_dt_slots
              ≈ 0.5 * 4 * 100
              ≈ 200
```

This means admission forces `admit_h_max` after about 2% of cumulative price movement since the last full cursor wrap. Lower thresholds are more conservative; higher thresholds prefer fast-lane availability. `None` disables the gate entirely and should be reserved for wrappers that rely on nonzero `admit_h_min` or otherwise accept trusted/private semantics.

Wrappers should calibrate the threshold against both keeper cadence and oracle volatility. In a market that moves about 1%/minute with cranks every 10 seconds, a 200-bps threshold triggers after roughly two minutes of sustained drift unless keepers sweep faster. That gives a reasonable stress response without waiting for the hard price cap itself to trip.

### What remains wrapper-owned

The wrapper still chooses:

- whether to pass `Some(threshold)` or `None`, subject to the public-wrapper restriction in §12.21,
- what threshold value reflects its stress tolerance,
- how to source and budget `rr_window_size`,
- whether to run heartbeat cranks in idle markets, and
- authority gating and any user-facing access policy.

The wrapper does **not** choose:

- when the consumption gate fires once a threshold is set,
- when consumption resets,
- whether Phase 2 runs, or
- what counts as a generation advance.

That is the intended split: policy inputs remain wrapper-owned, while the mechanism is engine-enforced.

### Cost and compatibility summary

- **State:** three new global fields (`rr_cursor_position`, `sweep_generation`, and `price_move_consumed_bps_this_generation`) plus one new `ctx` field (`admit_h_max_consumption_threshold_bps_opt_shared`).
- **Compute:** Phase 2 now exists on every successful `keeper_crank`; its dominant cost is the touched-account window. Keepers naturally size `rr_window_size` to fit budget.
- **Complexity:** the change is additive. It does not weaken the existing goal-52 construction.
- **Backward compatibility:** for trusted or private wrappers, passing `admit_h_max_consumption_threshold_bps_opt = None` and `rr_window_size = 0` recovers pre-threshold rev4 behavior exactly, except that the new persistent cursor and generation fields remain inert state. Public or permissionless wrappers that also use `admit_h_min == 0` are wrapper-prohibited from using that combination by §12.21.

### What this buys

- A1 self-neutral insurance siphon: eliminated by construction under valid accruals.
- A1 variants with many colluding accounts: eliminated under the same per-position invariant.
- Oracle-compromise insurance drain: bounded to one rejected envelope before the market bricks; the attacker cannot cascade valid accruals through the insurance fund.
- Flash-loan-style levered extraction: no special effect, because the cap is on move magnitude per accrual envelope, not position size.

### What it does not change

Legitimate insurance draws from funding shortfalls, partial-liquidation dust, ADL K-overflow routing, and intentionally configured degenerate resolution remain governed by their existing rules. Those are orthogonal to the price-move cap, sweep-generation signal, and admission-threshold gate.
