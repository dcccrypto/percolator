# Percolator

**EDUCATIONAL RESEARCH PROJECT — NOT PRODUCTION READY. NOT AUDITED. Do NOT use with real funds.**

A predictable alternative to ADL — the core risk engine crate for the [Percolator](https://github.com/dcccrypto/percolator-launch) perpetual futures protocol on Solana.

[![Crate](https://img.shields.io/badge/crate-percolator-orange)](https://github.com/dcccrypto/percolator)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue)](LICENSE)
[![Kani Proofs](https://img.shields.io/badge/Kani-157%20proofs-14F195)]()

---

## The Core Idea

If you want the `xy = k` of perpetual futures risk engines — something you can reason about, audit, and run without human intervention — the cleanest move is simple: stop treating profit like money. Treat it like what it really is in a stressed exchange: a junior claim on a shared balance sheet.

> No user can ever withdraw more value than actually exists on the exchange balance sheet.

- **Principal** (capital deposited) is a **senior claim** — always withdrawable.
- **Profits** are **junior IOUs** — backed by system residual value.
- A single global ratio `h` determines how much of all profits are actually backed.
- Profits convert into withdrawable capital through a bounded warmup process.

## Why This Is Different From ADL

Most perp venues use a waterfall: liquidate → insurance absorbs loss → if insufficient, ADL. ADL preserves solvency by forcibly reducing profitable positions. Percolator instead applies a **global pro-rata haircut on profit extraction**.

| | ADL | Percolator (Withdrawal-Window) |
|---|---|---|
| **Mechanism** | Forcibly closes profitable positions | Haircuts profit extraction |
| **When triggered** | Insurance depleted | Continuously via `h` |
| **User experience** | Position deleted without consent | Withdrawable amount reduced |
| **Recovery** | Manual re-entry | Automatic as `h` recovers |

## The Global Coverage Ratio `h`

```
Residual  = max(0, V - C_tot - I)

              min(Residual, PNL_pos_tot)
    h     =  --------------------------
                    PNL_pos_tot
```

If the system is fully backed, `h = 1`. If stressed, `h < 1`. Every profitable account is backed by the same fraction `h`. No rankings. No queue. Just proportional equity math.

---

## Architecture

This crate is a **pure Rust library** with zero dependencies (no `std`, no allocator, no Solana SDK). It implements the `RiskEngine` state machine:

```
┌─────────────────────────────────────────────────────────────┐
│                     percolator (this crate)                 │
│                                                             │
│  RiskEngine                                                 │
│  ├── Account state (capital, PnL, positions, warmup)        │
│  ├── Trade execution (open, close, flip positions)          │
│  ├── Margin checks (initial + maintenance margin)           │
│  ├── Funding rate accrual (anti-retroactive)                │
│  ├── Warmup/conversion (time-gated profit → capital)        │
│  ├── Liquidation logic (fee-debt aware)                     │
│  ├── Insurance fund accounting                              │
│  └── Global coverage ratio h (haircut computation)          │
│                                                             │
│  Properties:                                                │
│  • Pure accounting — no CPI, no I/O, no signatures          │
│  • Deterministic — same inputs always produce same outputs  │
│  • no_std compatible — runs on Solana BPF                   │
│  • Zero dependencies at runtime                             │
└─────────────────────────────────────────────────────────────┘
         │
         │  Used by:
         ▼
┌─────────────────────────┐    ┌──────────────────────┐
│  percolator-prog        │    │  percolator-stake     │
│  (Solana on-chain       │    │  (Insurance LP        │
│   program / wrapper)    │    │   staking program)    │
└─────────────────────────┘    └──────────────────────┘
```

### Source Layout

```
percolator/
├── src/
│   ├── percolator.rs   # RiskEngine implementation (~4000 lines)
│   └── i128.rs         # Safe i128 arithmetic helpers
├── tests/
│   ├── unit_tests.rs   # Unit tests
│   ├── amm_tests.rs    # AMM integration tests
│   ├── fuzzing.rs      # Proptest fuzz tests
│   └── kani.rs         # Kani formal verification proofs
├── spec.md             # Normative spec (v7) — source of truth
├── audit.md            # Security audit notes
├── Cargo.toml
├── LICENSE             # Apache 2.0
└── README.md
```

### Key Types

```rust
pub struct RiskEngine {
    // Global state
    pub risk_params: RiskParams,
    pub insurance_balance: U128,
    pub total_capital: U128,
    pub total_positive_pnl: U128,
    pub risk_reduction_threshold: U128,
    pub last_crank_slot: u64,
    // ...
    pub accounts: [Account; MAX_ACCOUNTS],
}

pub struct Account {
    pub owner: [u8; 32],
    pub capital: U128,
    pub pnl: I128,
    pub position_size: I128,
    pub entry_price_e6: u64,
    pub warmup_started_at_slot: u64,
    pub warmup_slope_per_step: U128,
    pub funding_index: I128,
    // ...
}
```

---

## Spec

The normative specification lives in [`spec.md`](spec.md) (v7). It defines:

1. **Security goals** — principal protection, oracle manipulation safety, conservation, liveness, no zombie poisoning
2. **Types and scaling** — all amounts in quote token atomic units, prices in `u64 × 1e6`
3. **Account state machine** — capital, PnL, positions, warmup, funding snapshots
4. **Trade execution** — initial/maintenance margin, fee computation, position flips
5. **Funding rate** — anti-retroactive accrual, slot-based intervals
6. **Warmup/conversion** — time-gated profit → capital with bounded conversion
7. **Liquidation** — fee-debt-aware, keeper permissionless
8. **Coverage ratio** — global `h` computation, haircut distribution
9. **Insurance fund** — top-up, flush, withdrawal policies

---

## Formal Verification

**157 Kani proofs** covering conservation, principal protection, isolation, and no-teleport properties:

| Category | Count | Description |
|----------|-------|-------------|
| Inductive | 11 | Multi-step invariant proofs (conservation across sequences) |
| Strong | 144 | Single-step property proofs (margin, liquidation, warmup, funding) |
| Unit test | 2 | Boundary condition checks via Kani |

### Running Kani Proofs

```bash
# Install Kani (one-time)
cargo install --locked kani-verifier
cargo kani setup

# Run all proofs
cargo kani

# Run a specific harness
cargo kani --harness conservation_deposit_withdraw
```

### Property Tests

The crate also includes **proptest** fuzzing for randomized exploration:

```bash
cargo test --features fuzz
```

---

## Build & Test

### Prerequisites

- **Rust** (stable, 2021 edition)
- For Kani: `cargo-kani` (see [Kani docs](https://model-checking.github.io/kani/))

### Commands

```bash
# Run all unit and integration tests
cargo test

# Run with specific MAX_ACCOUNTS size
cargo test --features test     # MAX_ACCOUNTS=64  (~0.17 SOL)
cargo test --features small    # MAX_ACCOUNTS=256 (~0.68 SOL)
cargo test --features medium   # MAX_ACCOUNTS=1024 (~2.7 SOL)
cargo test                     # MAX_ACCOUNTS=4096 (~6.9 SOL, default)

# Run proptest fuzz tests
cargo test --features fuzz

# Run Kani formal verification
cargo kani --tests

# Build for Solana BPF (no-entrypoint, library only)
cargo build --target bpfel-unknown-unknown --release
```

### Feature Flags

| Feature | Effect |
|---------|--------|
| `test` | `MAX_ACCOUNTS=64` — small slab for fast tests |
| `small` | `MAX_ACCOUNTS=256` — medium slab |
| `medium` | `MAX_ACCOUNTS=1024` — large slab |
| (default) | `MAX_ACCOUNTS=4096` — production slab |
| `fuzz` | Enable proptest fuzzing harnesses |

---

## Usage as a Dependency

Add to your `Cargo.toml`:

```toml
[dependencies]
percolator = { git = "https://github.com/dcccrypto/percolator.git", branch = "master" }
```

With a size feature:

```toml
percolator = { git = "https://github.com/dcccrypto/percolator.git", branch = "master", features = ["small"] }
```

---

## Concrete Example

**Fully solvent:** `Residual = 150`, `PNL_pos_tot = 120` → `h = 1` (fully backed)

**Stressed:** `Residual = 50`, `PNL_pos_tot = 200` → `h = 0.25` (each dollar of profit is backed by 25 cents)

## References

- Tarun Chitra, *Autodeleveraging: Impossibilities and Optimization*, arXiv:2512.01112, 2025. [arxiv.org](https://arxiv.org/abs/2512.01112)

---

## Related Repositories

| Repository | Description |
|-----------|-------------|
| [percolator-prog](https://github.com/dcccrypto/percolator-prog) | Solana on-chain program (wrapper around this crate) |
| [percolator-matcher](https://github.com/dcccrypto/percolator-matcher) | Reference matcher program for LP pricing |
| [percolator-stake](https://github.com/dcccrypto/percolator-stake) | Insurance LP staking program |
| [percolator-sdk](https://github.com/dcccrypto/percolator-sdk) | TypeScript SDK for client integration |
| [percolator-ops](https://github.com/dcccrypto/percolator-ops) | Operations dashboard |
| [percolator-mobile](https://github.com/dcccrypto/percolator-mobile) | Solana Seeker mobile trading app |
| [percolator-launch](https://github.com/dcccrypto/percolator-launch) | Full-stack launch platform (monorepo) |

## License

Apache 2.0 — see [LICENSE](LICENSE).
