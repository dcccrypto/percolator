# Percolator

**EDUCATIONAL RESEARCH PROJECT — NOT PRODUCTION READY. NOT AUDITED. Do NOT use with real funds.**

A predictable alternative to ADL.

If you want the `xy = k` of perpetual futures risk engines -- something you can reason about, audit, and run without human intervention -- the cleanest move is simple: stop treating profit like money. Treat it like what it really is in a stressed exchange: a junior claim on a shared balance sheet.

> No user can ever withdraw more value than actually exists on the exchange balance sheet.

## Two Problems, Two Mechanisms

A perp exchange has two fairness problems:

1. **Exit fairness:** when the vault is stressed, who gets paid and how much?
2. **Overhang clearing:** when positions go bankrupt, how does the opposing side absorb the residual without deadlocking the market?

Percolator solves them with two independent mechanisms that compose cleanly:

- **H** (the haircut ratio) keeps all exits fair.
- **A/K** (the lazy side indices) keeps all residual overhang clearing fair, and guarantees markets always return to healthy.

---

## H: Fair Exits

### One Vault. Two Claim Classes.

**Capital** (principal) is senior. It is withdrawable. Deposits create capital, and withdrawals only return capital.

**Profit** is junior. It is an IOU backed by system residual value. It must mature into capital through time-gated warmup before it can leave.

### The Haircut Ratio

```
Residual  = max(0, V - C_tot - I)

              min(Residual, PNL_pos_tot)
    h     =  --------------------------
                    PNL_pos_tot
```

`V` is the vault balance. `C_tot` is the sum of all capital. `I` is the insurance fund. `PNL_pos_tot` is the sum of all positive PnL.

If fully backed, `h = 1`. If stressed, `h < 1`. Every profitable account is backed by the same fraction `h`.

### Why H is fair

Every winner gets the same deal:

```
effective_pnl_i = floor(max(PNL_i, 0) * h)
```

No rankings, no queue priority, no first-come advantage. Pure proportional equity math. The floor rounding is conservative — the sum of all effective PnL never exceeds what actually exists in the vault.

Profit converts to withdrawable capital through warmup, bounded by `h`:

```
payout = floor(warmable_amount * h)
```

When the system is stressed, `h` falls and less converts. When losses settle or buffers recover, `h` rises. The mechanism self-heals. No manual intervention, no governance vote, no admin key.

### Flat accounts are protected

An account with no open position cannot have its principal reduced by another account's insolvency. `h` only gates profit extraction — it never touches deposited capital. This is the protected-principal guarantee.

---

## A/K: Fair Overhang Clearing

When a leveraged account goes bankrupt, two things need to happen:

1. The position quantity must be removed from the market's open interest.
2. Any uncovered quote deficit must be distributed across the opposing side.

Traditional ADL picks specific counterparties (usually ranked by profitability) and force-closes their positions. This is unfair: the selected traders lose their positions while identical traders on the same side keep theirs.

### Lazy Side Indices

Percolator replaces targeted ADL with two global coefficients per side:

- **A** (the position multiplier): a dimensionless fraction that scales everyone's effective position equally. When OI shrinks due to ADL, `A` decreases. Every account on that side shrinks by the same ratio.

- **K** (the PnL accumulator): a cumulative index that encodes all mark-to-market, funding, and deficit-socialization events. When a deficit is socialized, `K` shifts. Every account on that side absorbs the same per-unit loss.

The key property: **no account is singled out.** Everyone on the same side, with the same entry state, gets the same outcome. Settlement is O(1) per account — read global A and K, compute your delta against your stored snapshot.

```
effective_pos_q(i) = floor(basis_pos_q_i * A_side / a_basis_i)
pnl_delta(i)       = floor_div(|basis_i| * (K_side - k_snap_i), a_basis_i * POS_SCALE)
```

### Why A/K is fair

Three properties:

1. **Proportional quantity reduction.** When A decreases, every account's effective position shrinks by the same fraction. No targeting. The floor rounding means each account shrinks by at least their proportional share — the engine never over-allocates remaining OI.

2. **Proportional deficit distribution.** When K shifts, every account absorbs the same per-unit loss. The ceiling rounding on the K delta ensures the aggregate charge covers the deficit — no shortfall, no minting.

3. **No canonical order dependency.** Account settlement is fully local. One account's settlement does not depend on whether another account has been touched yet. No sequential prefix requirement, no global scan.

### Markets Always Return to Healthy

The A/K mechanism guarantees forward progress through a three-phase recovery:

**Phase 1: DrainOnly.** When A drops below a precision threshold (2^64), the side enters DrainOnly mode. Existing positions can close, but no new OI can be added. This prevents precision exhaustion from getting worse.

**Phase 2: ResetPending.** When all OI on a side reaches zero (either through closures or precision-exhaustion terminal drain), the engine begins a full-drain reset:
- Snapshots the current K as `K_epoch_start`
- Increments the epoch counter
- Resets A back to 1.0 (ADL_ONE = 2^96)
- Marks all remaining accounts as "stale"

Each stale account settles its residual PnL exactly once when next touched, using the K delta from their snapshot to the epoch start. No stale account is left behind — the stale counter tracks them.

**Phase 3: Normal.** Once all stale accounts have been touched and all OI is confirmed zero, the side transitions back to Normal. Fresh trading resumes with full precision.

This cycle is deterministic. No admin intervention. No governance. No "freeze the market and figure it out later." The state machine always makes progress: DrainOnly prevents expansion, positions close, OI reaches zero, the reset fires, stale accounts settle, the side reopens.

### Phantom Dust Tracking

Integer division creates sub-unit residuals ("phantom dust") — positions that floor to zero effective quantity but still occupy accounting state. The engine tracks these with precise bounds:

```
phantom_dust_bound_side_q
```

When all stored positions on both sides are closed, if remaining OI falls within the dust bound, the engine clears it bilaterally and triggers the reset. This prevents dust from accumulating indefinitely or deadlocking the system.

---

## How They Compose

H and A/K solve orthogonal problems:

| | H (haircut ratio) | A/K (side indices) |
|---|---|---|
| **Problem** | Exit fairness | Overhang clearing |
| **Scope** | All accounts (global) | One side at a time |
| **Mechanism** | Pro-rata profit scaling | Pro-rata position/deficit scaling |
| **Triggered by** | Any withdrawal or conversion | Bankrupt liquidation |
| **Recovery** | Automatic as Residual improves | Deterministic three-phase reset |
| **Order-dependent** | No | No |

Together they guarantee:

1. **No user can withdraw more than exists** (H bounds effective profit by Residual).
2. **No user is singled out for forced closure** (A/K distributes equally).
3. **Markets always recover** (DrainOnly -> ResetPending -> Normal is a deterministic cycle).
4. **Flat accounts keep their deposits** (H never touches capital; A/K only affects open positions).

## vs Traditional ADL

| | Traditional ADL | Percolator |
|---|---|---|
| **Mechanism** | Force-close selected profitable positions | Proportional haircut on profit + proportional position scaling |
| **When triggered** | Insurance depleted | Continuously via `h`; per-liquidation via A/K |
| **User experience** | Position deleted without consent | Withdrawable amount reduced; position shrinks proportionally |
| **Recovery** | Manual re-entry at worse price | Automatic as `h` recovers; side resets and reopens |
| **Fairness** | Ranked selection (winners punished first) | Equal treatment (same ratio for everyone) |
| **Liveness** | Can deadlock if counterparties unavailable | Deterministic forward progress guaranteed |

---

## Formal Verification

```bash
cargo install --locked kani-verifier
cargo kani setup
cargo kani
```

## Open Source

Fork it, test it, send bug reports. Percolator is open research under Apache-2.0.

## References

- Tarun Chitra, *Autodeleveraging: Impossibilities and Optimization*, arXiv:2512.01112, 2025. https://arxiv.org/abs/2512.01112
