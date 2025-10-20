# Percolator v0 - Minimal Viable Architecture

## Strategic Pivot (January 2025)

**Decision**: Simplify to prove the core thesis with minimal complexity.

## What v0 Proves

1. **Router coordination works** - separate component reads from multiple slabs, splits orders, CPIs atomically
2. **Portfolio netting works** - IM calculated on net exposure across all slabs (not per-slab)
3. **Capital efficiency is real** - long slab A + short slab B = ~0 IM requirement
4. **Oracle alignment prevents basis** - shared marks across slabs guarantee basis-free netting
5. **TOCTOU safety** - seqno matching ensures no race conditions

## Architecture

### Slab Program (Minimal Per-LP DEX)

**Single Account** (~4KB, not 10MB):
```rust
pub struct SlabState {
    pub header: SlabHeader,       // 256 bytes - metadata + seqno
    pub quote_cache: QuoteCache,  // 256 bytes - best 4 levels bid/ask
    pub book: BookArea,           // 3KB - price-time queues
}
```

**One Instruction**:
```rust
pub fn commit_fill(
    side: Side,
    qty: i64,           // desired quantity (1e6 scale)
    limit_px: i64,      // worst acceptable price
    receipt_account: &mut FillReceipt,
    signer: router_id,  // router-only access
) -> Result<()>
```

**No**:
- ❌ Per-user accounts
- ❌ Reservation/hold system
- ❌ Escrow/capability tokens
- ❌ Multi-phase commit
- ❌ On-slab settlement

### Router Program (Coordination + Portfolio)

**Responsibilities**:
1. Read QuoteCache directly from slab accounts (byte offsets)
2. Split user orders across N slabs (greedy on price × qty)
3. CPI to each slab's commit_fill in one transaction
4. Aggregate FillReceipts
5. Update user portfolio (net exposures across all slabs)
6. Check margin on **net exposure** (capital efficiency!)
7. Hold all tokens (no per-slab escrow)

**Key Accounts**:
```rust
pub struct UserPortfolio {
    pub exposures: Vec<(SlabId, InstrumentId, i64)>,  // net positions
    pub equity: i128,
    pub im_required: u128,  // computed on NET exposure!
}

pub struct Vault {
    pub mint: Pubkey,
    pub balance: u128,
    pub token_account: Pubkey,
}
```

## The Killer Demo

```
Initial State:
  User: 10,000 USDC, 0 positions

Atomic Transaction:
  1. Router reads QuoteCache from Slab A and Slab B
  2. Router splits: +1 BTC on Slab A @ $50,000
                    -1 BTC on Slab B @ $50,010
  3. Router CPIs to both slabs
  4. Both fill, receipts returned
  5. Router updates portfolio:
       Exposures: [(A, BTC, +1.0), (B, BTC, -1.0)]
       Net exposure: 0
       IM required: ~$0 (not $10,000!)
       Locked in: $10 arb
  6. Transaction succeeds

Result: Capital efficiency ∞ (zero capital for zero net exposure)
```

## What We Removed (vs. Complex Design)

| Component | Complex | v0 | Why Removed |
|-----------|---------|-----|-------------|
| Slab account size | 10MB multi-pool | 4KB single account | No per-user state needed |
| Instructions | 6 (reserve, commit, cancel, batch, init, add_instrument) | 1 (commit_fill) | Direct commit is sufficient |
| Settlement | Escrow + Cap tokens | Router vaults only | Centralizes complexity |
| Margin | Per-slab | Router net exposure | Proves capital efficiency |
| Liquidation | Multi-slab autonomous | Router reduce-only stub | Simplifies v0 |
| Funding | Periodic payments | None | Not needed for demo |
| Insurance | Fund + DLP | None | Not needed for demo |

## What We Kept (Still Valuable)

- ✅ Instruction data deserialization helpers (percolator-common)
- ✅ Error types
- ✅ PDA derivation patterns
- ✅ Test infrastructure
- ✅ Portfolio tracking
- ✅ Vault management

## Implementation Timeline

- **Week 1**: Minimal slab (SlabHeader + QuoteCache + BookArea + commit_fill)
- **Week 2**: Router coordination (read quotes, CPI, aggregate, margin check)
- **Week 3**: 7 critical tests (atomic split, TOCTOU, capital efficiency, etc.)
- **Week 4**: Integration, audit prep, documentation

vs. 8-12 weeks for complex design.

## The 7 Critical Tests

1. **Atomic router split** - Lock N slabs, read caches, CPI commits, aggregate in one tx
2. **TOCTOU safety** - seqno matching prevents races
3. **Price limit enforcement** - vwap ≤ limit
4. **Capital efficiency demo** - Long A + Short B = ~0 IM (THE PROOF!)
5. **Oracle alignment gate** - Reject if mark_px drift > ε
6. **Failure rollback** - All-or-nothing on insufficient qty
7. **Compute budget** - K=4 levels, M slabs fits in budget

## Migration to v1

When v0 proves the model, add:
- Per-slab settlement (escrow/caps)
- Reservation/hold for multi-tx workflows
- Funding rate mechanism
- Insurance layer
- Autonomous liquidations

But **none of that is needed to prove the thesis**.

## Status

- [x] Complex design implemented (research phase)
- [x] v0 design documented
- [ ] Slab simplification (remove pools, multi-account, etc.)
- [ ] Router simplification (remove escrow, cap, etc.)
- [ ] QuoteCache implementation
- [ ] commit_fill instruction
- [ ] Router CPI coordination
- [ ] 7 critical tests

---

**Core Insight**: The complex multi-pool design was solving problems we don't have in v0. This minimal design proves capital efficiency (the core value prop) in ~1000 LOC vs ~5000 LOC.
