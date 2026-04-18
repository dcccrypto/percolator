# Percolator Threat Model

**Status:** Experimental — NOT AUDITED, NOT PRODUCTION READY  
**Last updated:** 2026-04-16  
**Spec version:** v12.17.0  

---

## Overview

Percolator is a permissionless perpetual futures risk engine on Solana. This document describes the trust model, known deferred findings, deployment checklist, and security audit status for external auditors and security researchers.

---

## Trust Assumptions

### Admin

The admin key `7JVQvr...` is an EOA key that currently controls program upgrades and market configuration. It MUST be migrated to a Squads multisig before the mainnet production launch.

The admin key is authorized to:

- Update market config knobs (`UpdateConfig`, `SetMaintenanceFee`)
- Resolve markets (`ResolveMarket`)
- Set and rotate oracle authority (`SetOracleAuthority`, `SetOraclePriceCap`)
- Burn admin permanently by setting to all-zeros (`UpdateAdmin` to `[0;32]`)
- Force-close abandoned accounts after resolution (`AdminForceCloseAccount`)
- Pause or unpause the market (via admin crank path)
- Withdraw insurance post-resolution (`WithdrawInsuranceLimited`)
- Set insurance withdrawal policy (`SetInsuranceWithdrawPolicy`)
- Rotate the admin key itself (`UpdateAdmin`)

The admin key cannot: redirect user fund withdrawals to arbitrary accounts, close the slab while funds remain, mutate config after resolution, or perform admin operations after the key is burned to all-zeros. Each of these constraints is covered by dedicated test cases. See the Admin Key Threat Model section of the percolator-prog README for the full enumeration.

The planned mitigation is Squads multisig governance, which requires M-of-N approval for any admin operation. This eliminates the single-EOA risk surface.

### Oracle

Pyth price feeds are used for non-Hyperp markets. A staleness filter (`max_crank_staleness_slots`) bounds the maximum age of any accepted price. A confidence filter (`conf_filter_bps`) rejects prices with excessive uncertainty.

For Hyperp markets, prices are authority-pushed via `PushOraclePrice`. The oracle authority is set by admin via `SetOracleAuthority`. A per-slot circuit breaker (`SetOraclePriceCap`) caps price movement, preventing instant mark manipulation.

Oracle manipulation resistance is reinforced at the engine level: unrealized profit accrued from a price spike sits in the per-account reserve `R_i` and does not enter the matured haircut denominator until the warmup window passes. An attacker who spikes a price cannot immediately withdraw the accrued gain.

### Keeper

The keeper crank (`KeeperCrank`) is permissionless — any party may submit it. However, a funded keeper is required for liveness guarantees:

- **Funding drift:** If the keeper is offline for more than `MAX_FUNDING_DT` (approximately 7 hours at typical slot rates), the funding accumulation caps out. Positions are not overfunded, but the cap means accumulated funding is undercharged during the outage window.
- **Market freeze:** If the keeper is offline indefinitely, no liquidations execute, no funding settles, and no accounts are garbage-collected. Markets remain frozen but all funds remain in the vault — no funds are at risk due to keeper absence, only liveness.

The keeper must remain funded in SOL for transaction fees. Keeper balance below minimum fee threshold is a monitoring alert. The deployment uses Railway with auto-restart on failure.

### Matcher

The matcher program is registered by each LP via `InitLP`. During `TradeCpi`, Percolator performs a CPI to the LP's chosen matcher program. The binding is enforced as follows:

- **LP identity binding:** The matcher program address and context account must equal exactly what the LP registered at `InitLP` time. Substitution is rejected.
- **LP PDA signature:** The LP PDA is derived on-chain from seeds `["lp", slab_pubkey, lp_idx_le]`. The user cannot supply a counterfeit PDA — it is derived, not accepted as input.
- **Return data echo-validation:** The matcher's response must echo the oracle price, nonce (`req_id`), LP account ID, and size constraints back to Percolator. Any mismatch causes hard rejection. Reserved fields must be zero.
- **Execution size discipline:** Percolator always uses the matcher's `exec_size`, never the user's requested size. The matcher may partially fill or reject.

The matcher is treated as adversarial input. All ABI fields from the matcher response are validated before any state changes occur. The nonce (`req_id`) is a monotonic `u64` derived from the slab nonce. It increments on every accepted trade and is unchanged on reject, preventing replay.

---

## Known Deferred Findings

The following findings were identified during internal audit and deferred. Each has a documented rationale and is tracked for resolution before full mainnet production.

### F-6 — LP Identity Inconsistency ✅ RESOLVED (2026-04-17)

**Description:** `TradeNoCpi` uses a generation-table lookup for LP identity, while `TradeCpi` uses an FNV hash of the LP public key. These are two different identity-binding mechanisms for the same logical concept.
**Impact:** No current fund or position risk. The `NoOpMatcher` used by `TradeNoCpi` ignores the identity field. There is no path where this inconsistency allows an LP to impersonate another LP or bypass controls.
**Resolution:** Pre-audit hygiene review confirmed the two schemes intentionally serve different contexts — `gen_table` (mat_counter) is strictly monotonic and used for internal lifecycle tracking, while FNV hash is used for matcher CPI binding and does not need to distinguish instance generations. Documented inline at `percolator-prog/src/percolator.rs` near the FNV compute site. Unifying would either break matcher binaries or lose slot-reuse detection. Not a bug; documented as intentional.

### C-7 — NFT Account ID Guard Inoperative on v12.17

**Description:** The NFT program's `account_id` guard does not execute correctly under the v12.17 slab layout.
**Impact:** The guard failure allows a PDA to be created with an incorrect account ID, costing approximately 0.002 SOL in PDA rent. No position ownership bypass, no fund risk.
**Status:** ACCEPTED RISK. Bounded blast radius (~0.002 SOL rent per occurrence), no fund or position access. Fix requires a layout-aware `account_id` read, scheduled for the next NFT program upgrade. Monitoring: count of NFT PDA creations per slab (alert if rate exceeds expected new-user rate).

### C-12 — NFT Transfer Hook Stale Oracle

**Description:** The NFT transfer hook does not re-fetch the oracle price at transfer time. It uses the price cached at last crank.
**Impact:** In markets where the keeper cranks infrequently, the health check performed at NFT transfer time may use a stale price. This could allow a transfer when a freshly-priced health check would block it.
**Status:** ACCEPTED RISK. Bounded by keeper crank frequency (`max_crank_staleness_slots`, configured to 200 slots). The fix requires an `ExtraAccountMeta` change (adding the oracle feed to the transfer hook accounts), which is a non-trivial Token-2022 interface change affecting wallet UX. Scheduled for next NFT program upgrade. Monitoring: staleness gap between last crank slot and NFT transfer signature slot.

### SP-2 — `_reserved[8..16]` Market Start Slot Dead Write ✅ FIXED (2026-04-17)

**Description:** The `_reserved[8..16]` field in the market header received a write of `market_start_slot` that was never subsequently read by any instruction.
**Impact upgraded during review:** This was NOT purely cosmetic. The bytes `_reserved[8..16]` are also used by `mat_counter` (PERC-623 per-instance LP identity counter). The `market_start_slot` write at InitMarket was corrupting `mat_counter` with the slot value, causing the first `InitUser/InitLP` to receive `mat_counter = (creation_slot + 1)` instead of `1`.
**Resolution:** Removed `write_market_start_slot` function and its call site at InitMarket. The slot value was never consumed as "market_start_slot" anywhere, so removal is safe. `mat_counter` now correctly starts at 0 and increments from 1 on the first account creation.

### SP-5 — Admin Can Disable HWM Without Timelock

**Description:** The admin can disable the high-watermark (HWM) policy for insurance withdrawals without any timelock or delay. An operator could in principle reduce withdrawal protections immediately.
**Impact:** Governance policy risk, not a code vulnerability. The HWM mechanism provides withdrawal rate-limiting above and beyond the cooldown.
**Status:** GOVERNANCE POLICY ITEM. There is no on-chain setter for `hwm_floor_bps` in current code; the field is set once at `CreateLpVault` and cannot be changed. The finding becomes relevant only if a setter is added in the future. Mitigation: with Squads multisig as admin, any future HWM-disable setter would require M-of-N signers. Tracked for post-launch governance hardening. No code change needed in current release.

---

## Deployment Checklist

The following items must be verified before removing the mainnet beta designation.

### Admin Key Migration

Verify the admin key has been transferred to the Squads multisig:

```bash
solana program show ESa89R5Es3rJ5mnwGybVRG1GrNt9etP11Z5V2QWD4edv
```

The upgrade authority in the output must be the Squads multisig address, not an EOA key.

### Keeper Liveness

- Verify the keeper is funded and cranking on Railway
- Check last successful crank slot is within `max_crank_staleness_slots`
- Verify keeper SOL balance is above minimum fee threshold
- Confirm Railway auto-restart is active

### Oracle Authority

- Verify oracle authority is set to the keeper bot's public key via `SetOracleAuthority`
- For Pyth markets: verify Pyth feed IDs are correct and staleness bounds are appropriate
- Verify `SetOraclePriceCap` is configured for Hyperp markets

### Insurance Fund

- Verify the insurance fund has been seeded with initial capital via `TopUpInsurance`
- Verify insurance floor is set appropriately via `SetRiskThreshold`
- Verify insurance withdrawal policy is configured with conservative cooldown

### Stake Pool

- Verify the stake pool `percolator_program` field matches the mainnet program ID `ESa89R5Es3rJ5mnwGybVRG1GrNt9etP11Z5V2QWD4edv`
- Verify `admin_transferred` is 1 (pool PDA is admin, not EOA)
- Verify `TransferAdmin` was called before accepting deposits

---

## Program IDs

| Program | Mainnet Address |
|---------|-----------------|
| Core (percolator-prog) | `ESa89R5Es3rJ5mnwGybVRG1GrNt9etP11Z5V2QWD4edv` |
| Matcher (percolator-match) | `DHP6DtwXP1yJsz8YzfoeigRFPB979gzmumkmCxDLSkUX` |
| Stake (percolator-stake) | `DC5fovFQD5SZYsetwvEqd4Wi4PFY1Yfnc669VMe6oa7F` |
| NFT (percolator-nft) | `FqhKJT9gtScjrmfUuRMjeg7cXNpif1fqsy5Jh65tJmTS` |

Devnet program IDs differ. See the SDK README for the full address table across networks.

---

## Security Audit Status

### Internal Audit

- **Total findings:** 25
- **Fixed:** 12
- **Deferred:** 5 (documented above)
- **Already fixed at time of audit:** 1
- **Remaining open:** 7 (tracked in internal issue tracker, none are critical-severity)

### Test Coverage

- **Total tests:** 1,265
- **Failures:** 0
- **Composition:**
  - percolator-prog: 707 tests (462 integration on LiteSVM, 28 unit, 8 alignment, 1 CU benchmark)
  - percolator (core): remainder of 1,265
  - percolator-nft: 65 tests
  - percolator-match: 34 tests
  - percolator-stake: 270 tests

### Formal Verification

- **Kani proof harnesses:** 471 total
  - percolator (core): ~386 harnesses
  - percolator-prog: 113 harnesses
  - percolator-stake: 85 harnesses (35 harnesses in kani-proofs/)
  - percolator-nft: additional harnesses
- **All proofs passing:** yes

### Upstream Contributions

6 pull requests have been submitted to the upstream `aeyakovenko` repositories as part of the audit process.

---

## References

- [Risk Engine Spec v12.17.0](spec.md) — normative specification
- [percolator-prog README](../percolator-prog/README.md) — Admin Key Threat Model, instruction reference
- [percolator-stake docs/AUDIT.md](../percolator-stake/docs/AUDIT.md) — 4-round stake program audit report
- [percolator-stake docs/KANI-DEEP-ANALYSIS.md](../percolator-stake/docs/KANI-DEEP-ANALYSIS.md) — Proof-by-proof Kani analysis
