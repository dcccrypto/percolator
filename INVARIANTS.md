# Percolator Safety Invariants

This document provides detailed explanations of the 6 core safety invariants that have been formally verified using Kani model checking.

## Overview

Percolator's safety model is built on 6 fundamental invariants that ensure user funds are protected, accounting is correct, and the system cannot be exploited. These invariants have been proven using formal verification across **34 different test scenarios** covering concrete cases, parameterized symbolic tests, and edge cases.

---

## I1: Principal Inviolability

**Statement**: User principals never decrease during loss socialization.

### Formal Definition

```
∀ state, state', deficit:
  state' = socialize_losses(state, deficit)
  ⟹ ∀ user_id: state'.users[user_id].principal == state.users[user_id].principal
```

### Why It Matters

- **User Protection**: Users' deposited collateral (principal) is sacrosanct
- **Haircuts are Limited**: Losses can only reduce unrealized profits (PnL), never principal
- **Prevents Insolvency**: Ensures users can always withdraw their original deposits (minus their own withdrawals)

### What Can Reduce Principal

✅ **Allowed**:
- User-initiated withdrawals (`withdraw_principal`)
- Never: Socialization, haircuts, or liquidations

❌ **Never Allowed**:
- Loss socialization
- Market haircuts
- Insurance fund depletion
- Matcher operations

### Verification Coverage

- ✅ **Concrete**: Single socialization with fixed deficit (minimal.rs)
- ✅ **Bounded Symbolic**: Symbolic deficit 0-255 (minimal.rs)
- ✅ **Multi-Operation**: Sequential socializations (medium.rs)
- ✅ **Total Wipeout**: Massive deficit exceeding all PnL (edge.rs)
- ✅ **3-User**: Multiple users with different PnL levels (edge.rs)

### Example

```rust
// User deposits 1000 USDC
user.principal = 1000

// User makes profitable trade
user.pnl_ledger = +500  // Now has 1500 total value

// Market crash causes 300 USDC loss to socialize
socialize_losses(state, 300)

// After socialization:
user.principal = 1000     // ✅ UNCHANGED (I1 protected)
user.pnl_ledger = +200    // Reduced by 300 (took the haircut)
// Total value: 1200 (lost 300 from PnL, not principal)
```

---

## I2: Conservation

**Statement**: Vault accounting always balances across all state transitions.

### Formal Definition

```
∀ state, operation:
  Let change_in_vault = state'.vault - state.vault
  Let change_in_balances = Σ(state'.balances) - Σ(state.balances)

  ⟹ change_in_vault == change_in_balances (within rounding tolerance)
```

Where balances include principal, positive PnL, and insurance fund.

### Why It Matters

- **No Lost Funds**: Every token is accounted for
- **No Double Spend**: Can't create value from nothing
- **Audit Trail**: Vault changes must match user balance changes
- **System Integrity**: Prevents accounting bugs that could drain the protocol

### Conservation Equation

```
vault = Σ(user.principal) + Σ(max(0, user.pnl_ledger)) + insurance_fund - fees_outstanding
```

With saturation for overflow protection.

### Verified Operations

- ✅ **Deposit**: `vault += amount`, `principal += amount`
- ✅ **Withdraw**: `vault -= amount`, `principal -= amount`
- ✅ **Socialization**: `vault` unchanged, PnL redistributed
- ✅ **Sequences**: Deposit → Withdraw, Deposit → Socialize → Withdraw

### Verification Coverage

- ✅ **2-User Deposit+Withdraw**: Vault change matches operations (medium.rs)
- ✅ **3-Step Sequence**: Deposit → Socialize → Withdraw (medium.rs)
- ✅ **3-User Sequential**: Interleaved deposits and withdrawals (edge.rs)
- ✅ **Overflow Protection**: No vault overflow (medium.rs)

### Example

```rust
// Initial state
vault = 5000
user1.principal = 2000, user1.pnl = +500
user2.principal = 2000, user2.pnl = +500
// Check: 2000+2000+500+500 = 5000 ✅

// User1 deposits 1000
vault = 6000
user1.principal = 3000
// Check: 3000+2000+500+500 = 6000 ✅ (I2 preserved)
```

---

## I3: Authorization

**Statement**: Only the authorized router can mutate user balances.

### Formal Definition

```
∀ state, operation:
  state.authorized_router == false
  ⟹ balances_unchanged(state, operation(state))
```

### Why It Matters

- **Access Control**: Prevents unauthorized funds movement
- **Router Isolation**: Matcher programs cannot access user funds
- **Sybil Resistance**: Only the designated router program can execute trades
- **Attack Prevention**: Unauthorized programs can't drain accounts

### Protected Operations

When `authorized_router = false`, these operations are no-ops:

- ❌ `deposit()` - Cannot add to user principal
- ❌ `withdraw_principal()` - Cannot remove principal
- ❌ `withdraw_pnl()` - Cannot withdraw profits
- ❌ `socialize_losses()` - Cannot apply haircuts
- ✅ `matcher_noise()` - Always allowed (but can't move funds per I6)

### Verification Coverage

- ✅ **Single Operation**: Unauthorized deposit/withdrawal fails (minimal.rs)
- ✅ **Multi-User**: 2 users, unauthorized operations on each (medium.rs)
- ✅ **Mixed Sequence**: Unauthorized op → authorize → authorized op (edge.rs)

### Example

```rust
// Attacker tries to deposit to their account
state.authorized_router = false
before = state.clone()

deposit(state, attacker_id, 1_000_000)

// After operation:
state.users[attacker_id].principal == before.users[attacker_id].principal
// ✅ Deposit rejected (I3 protected)
```

---

## I4: Bounded Socialization

**Statement**: Haircuts only affect users with positive PnL, and total haircut is bounded by `min(deficit, sum_effective_winners)`.

### Formal Definition

```
∀ state, state', deficit:
  state' = socialize_losses(state, deficit)
  ⟹
    (1) ∀ user where pnl ≤ 0: pnl' == pnl  (losers untouched)
    (2) ∀ user where pnl > 0: pnl' ≤ pnl   (winners haircutted)
    (3) total_haircut ≤ min(deficit, sum_effective_winners)
```

### Why It Matters

- **Fair Loss Distribution**: Only profitable users share losses
- **No Unfair Punishment**: Losing traders don't get double-punished
- **Bounded Impact**: Users can't lose more than their unrealized profits
- **Deficit Coverage**: System can always cover bad debt up to available PnL

### Haircut Calculation

```rust
effective_pnl = max(0, pnl_ledger - reserved_pnl)
sum_effective_winners = Σ(effective_pnl for all users)

haircut_fraction = min(1.0, deficit / sum_effective_winners)

for each winner:
    new_pnl = pnl * haircut_fraction
```

### Verification Coverage

- ✅ **Winner+Loser**: Symbolic deficit, only winner haircutted (medium.rs)
- ✅ **Both Winners**: Proportional distribution (medium.rs)
- ✅ **All Losers**: Zero haircut when no winners (edge.rs)
- ✅ **Total Wipeout**: Deficit >> PnL, all wiped to zero (edge.rs)
- ✅ **Exact Balance**: Deficit == total PnL (edge.rs)
- ✅ **3 Winners**: Proportional across 3 users (edge.rs)
- ✅ **3 Mixed**: 2 winners, 1 loser correctly handled (edge.rs)

### Example

```rust
// Before socialization
user1: principal=1000, pnl=+500  // Winner
user2: principal=1000, pnl=-200  // Loser
deficit = 300

// After socialization
user1: principal=1000, pnl=+200  // Haircut: 500 → 200 (took 300 loss)
user2: principal=1000, pnl=-200  // ✅ UNCHANGED (losers protected by I4)

// Verify I4:
// (1) Loser untouched: ✅
// (2) Winner haircutted: ✅ (500 → 200)
// (3) Total haircut = 300 ≤ min(300, 500) ✅
```

---

## I5: Throttle Safety

**Statement**: PnL withdrawals respect warmup period limits; total withdrawn never exceeds `step * slope_per_step`.

### Formal Definition

```
∀ state, state', user_id, amount, step:
  state' = withdraw_pnl(state, user_id, amount, step)

  Let withdrawn = max(0, state.pnl - state'.pnl)
  Let max_allowed = step * state.users[user_id].warmup_state.slope_per_step

  ⟹ withdrawn ≤ max_allowed + ε  (where ε accounts for rounding)
```

### Why It Matters

- **Anti-Flash Attack**: Prevents instant withdrawal of unrealized profits
- **Stability**: Gradual unlock prevents bank runs
- **Haircut Protection**: Gives time for PnL to vest before it can be removed
- **Sybil Resistance**: Can't game the system with many accounts

### Warmup Mechanics

```rust
// Linear vesting schedule
withdrawable_at_step_n = min(
    n * slope_per_step,
    total_positive_pnl
)

// Example: slope_per_step = 100
// Step 0: 0 withdrawable
// Step 1: 100 withdrawable
// Step 2: 200 withdrawable
// Step 10: 1000 withdrawable (if pnl >= 1000)
```

### Edge Cases Verified

- ✅ **Zero Slope**: `slope=0` → no withdrawals ever (edge.rs)
- ✅ **Symbolic Step+Amount**: Step 0-15, amount 0-255 (medium.rs)
- ✅ **Larger Steps**: Step 0-31, higher slope (medium.rs)
- ✅ **Exact Cap**: Withdrawal exactly at limit (works correctly)
- ✅ **Vault Monotonicity**: Vault always decreases on withdrawal (medium.rs)

### Verification Coverage

- ✅ **Basic Throttle**: Symbolic step/amount, slope=10 (medium.rs)
- ✅ **Higher Slope**: slope=20, larger bounds (medium.rs)
- ✅ **Zero Slope**: No withdrawals allowed (edge.rs)
- ✅ **With Reserved**: Reserved PnL doesn't block throttle (edge.rs)

### Example

```rust
user.pnl_ledger = 1000
user.warmup_state.slope_per_step = 50

// At step 5: max_withdrawable = 5 * 50 = 250
withdraw_pnl(state, user_id, 300, step=5)
// Actually withdrawn: 250 (capped by throttle) ✅ I5

// At step 30: max_withdrawable = 30 * 50 = 1500, but pnl=1000
withdraw_pnl(state, user_id, 2000, step=30)
// Actually withdrawn: 1000 (capped by available PnL) ✅ I5
```

---

## I6: Matcher Isolation

**Statement**: Matcher operations cannot move funds; balances remain unchanged.

### Formal Definition

```
∀ state, state':
  state' = matcher_noise(state)
  ⟹ balances_unchanged(state, state')
```

### Why It Matters

- **Slab Isolation**: Matchers can only provide quotes, not move funds
- **No Direct Access**: Order book engines never touch collateral
- **Router Mediation**: Only router can execute fund movements (via I3)
- **Attack Surface**: Limits damage from compromised matcher programs

### What Matcher Can Do

✅ **Allowed** (no balance changes):
- Update order book state
- Calculate match prices (VWAP)
- Generate fill receipts
- Emit events

❌ **Never Allowed**:
- Change `user.principal`
- Change `user.pnl_ledger`
- Change `vault`
- Change `insurance_fund`

### Verification Coverage

- ✅ **Concrete Single-User**: Matcher noise on 1-user state (minimal.rs)
- ✅ **2-User Symbolic**: Matcher on 2-user state (medium.rs)
- ✅ **3-User**: Matcher isolation with 3 users (edge.rs)
- ✅ **Combined with I3**: Matcher+unauthorized operations (edge.rs)

### Example

```rust
// Before matcher operation
user1.principal = 1000, user1.pnl = +500
user2.principal = 1000, user2.pnl = -200
vault = 2300

matcher_noise(state)  // Matcher updates internal state

// After matcher operation
user1.principal = 1000  // ✅ UNCHANGED
user1.pnl = +500        // ✅ UNCHANGED
user2.principal = 1000  // ✅ UNCHANGED
user2.pnl = -200        // ✅ UNCHANGED
vault = 2300            // ✅ UNCHANGED (I6 protected)
```

---

## Verification Matrix

| Invariant | Minimal | Medium | Edge | Total Proofs |
|-----------|---------|--------|------|--------------|
| **I1** | 2 | 1 | 3 | 6 |
| **I2** | 0 | 2 | 2 | 4 |
| **I3** | 1 | 1 | 1 | 3 |
| **I4** | 0 | 2 | 7 | 9 |
| **I5** | 0 | 2 | 3 | 5 |
| **I6** | 1 | 1 | 1 | 3 |
| **Operations** | 4 | 2 | 4 | 10 |
| **TOTAL** | **7** | **11** | **16** | **34** |

---

## Proof Performance

| Category | Proofs | Runtime | Avg per Proof |
|----------|--------|---------|---------------|
| Minimal (concrete) | 7 | <10s | ~1s |
| Medium (parameterized) | 11 | <40s | ~3s |
| Edge (boundary cases) | 16 | ~60s | ~4s |
| **TOTAL** | **34** | **~110s** | **~3s** |

---

## How to Run Proofs

```bash
# All minimal proofs (7 proofs, <10s)
./run_fast_proofs.sh

# All medium proofs (11 proofs, ~40s)
./run_medium_proofs.sh

# All edge case proofs (16 proofs, ~60s)
./run_edge_proofs.sh

# Specific proof
cargo kani -p proofs-kani --harness i4_socialization_2users_symbolic_deficit

# All tractable proofs (34 total, ~110s)
./run_fast_proofs.sh && ./run_medium_proofs.sh && ./run_edge_proofs.sh

# All proofs including intractable ones (NOT RECOMMENDED)
cargo kani -p proofs-kani --lib
```

---

## Related Files

- **Proof Code**: `crates/proofs/kani/src/{minimal,medium,edge}.rs`
- **Safety Model**: `crates/model_safety/src/`
- **Verification Status**: `FORMAL_VERIFICATION_STATUS.md`
- **Scripts**: `run_fast_proofs.sh`, `run_medium_proofs.sh`

---

## References

- [Kani Rust Verifier](https://model-checking.github.io/kani/)
- [Percolator V0 Design](V0_DESIGN.md)
- [Safety Module Implementation](programs/router/src/state/)
