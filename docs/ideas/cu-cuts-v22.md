# CU cuts across v2.2 — engine pointer

The plan lives in the wrapper repo: `dcccrypto/percolator-prog`, branch `agent/J-20261121-CU-CUTS-PROG`,
`docs/ideas/cu-cuts-v22.md` (commit b7ffd7de). Engine items from it, base `feat/v22-engine-rc23-2026-10-22` @ `3e5970f3`:

- C1: `loss_weight_for_basis` (`src/v16.rs:29458`) computes a ceil(a*b/d) through `checked_mul_div_ceil_u256` (U512
  multiply + U256 divide, 1,782 CU per call) on every active-leg decode (`validate_active_leg`, `:29390`). The product
  fits u128 for every validated basis (max 1e14 * 1e15). u128 fast path, same value and errors; wide path kept for the
  rest. Prototype measured on top of #315/#319/#320: largest tx 1,028,762 -> 828,518 CU.
- C2: source-domain table walks copy each 196-byte slot by value (`validate_source_credit_shape_with_market` `:6517`,
  `source_claim_bound_sum_num` `:6637`, `is_occupied(self)` `:28094`); by-reference / all-zero-tail memcmp.
- C3: open PRs #319, #320, #315 (13 Nov, reviewed, never gated) — gate as they are.
- C5/C7: repeated `AssetStateV16Account::try_to_runtime`, remaining software u128/u256 arithmetic — after re-profile.

Hard rule: no change to risk-engine semantics or safety checks. Each cut ships with a differential test against the old
code, the engine suite, Kani where it applies, and a before/after CU table. Not overlapping #382's hunks.
