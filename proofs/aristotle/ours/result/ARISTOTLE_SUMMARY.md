# Summary of changes for run 1222bad6-e17d-4d90-81e2-9a4ba0131dd8
All seven properties are **proved** about the code as built: release profile, overflow checks on. None was refuted. Inv was strong enough for every property as written, so no statement had to be weakened. Every result except RC1 assumes the contract of `wide_mul_div_floor_u128` as a hypothesis (see "Assumed"); for RC1 it is the premise of `rc1_spec`, stated in the formula. Each result holds for slices and orders of any length. Sums are mathematical integers. Each "does not panic" means no execution ends in a panic; termination is not claimed.

- **RC1 – proved.** This is a public Lean statement, `Spec.rc1_spec` in `Spec/RC1.lean`. It covers any pool and any slice of accounts whose fields fit their Rust types and which satisfy `Inv`:
  - `kernel_recompute_resolved_payout_rate(pool.ledger)` returns `Ok(pool.ledger)`;
  - for each account, `resolved_receipt_claimable_against_ledger` returns `Ok(floor(face*rate_num/rate_den) - paid_effective)` (or `Ok(0)` without a receipt);
  - the sum of these amounts plus `P` is at most `snapshot_residual`, `G <= snapshot_residual`, and `P <= G`.
- **RC2 – proved.** `close_resolved_payout_tail(pool, &mut accounts[i])` from an `Inv` state does not panic. On `Ok(payout)`, every conclusion in your RC2 text holds, including that `Inv` is kept and the payout is never cut by the vault cap. Other accounts are untouched because the call borrows only `accounts[i]`.
- **RC4 – proved.** This covers `close_all` on any slice and any `order`, including repeated or out-of-range indices (an out-of-range index gives `Err`). It does not panic, and on `Ok(total)` every conclusion in your RC4 text holds. The loop invariant is that the state after `k` steps equals the model's state after the first `k` entries of `order`, with RC2 as the step.
- **RC5 – proved.** When `order` covers every index, `c_tot' == 0`, `total == C + (P' - P) <= C + snapshot_residual - P`, and `insurance <= vault'`.
- **RC7 – proved.** `claim_resolved_payout_topup_core` from an `Inv` state does not panic. On `Ok(payout)`, `Inv` holds after, `vault' + payout == vault`, `P' == P + payout`, and the account's `capital`, `c_tot` and `insurance` are unchanged.
- **RC3 – proved**, under your condition (`Inv`, `pnl_i >= 0`, and either a present receipt or `pnl_i * BOUND_SCALE <= terminal_claim_bound_unreceipted_num`). The call never panics, and if it returns, it returns `Ok`. The no-panic part needs only that the values fit their types and that `rate_den > 0` and `rate_num <= rate_den`.
- **RC6 – proved.**
  - First, any call of `close_all` on `order1` that returns `Ok(total)` leaves exactly the result of the model of `close_all` (`closeAll` in `Spec/Model.lean`).
  - Second, when that model result is `Ok`, the call on any permutation `order2`, from the same start, does not panic and, if it returns, returns `Ok(total)` with the same pool and accounts.
  - Order independence uses only `Inv`.

RC2–RC7 are stated in Rust prose in the Properties table rather than as public Lean formulas. The public interface cannot name the struct types behind `&mut PoolV16`, `&mut ClaimantV16` and `&mut [ClaimantV16]`. Their proofs are about the generated code, like RC1's.

**Verified:** the five functions above and every crate function they call:
- in `src/resolved_payout.rs`: receipt validation, `bound_num_from_amount`, `apply_resolved_payout_receipt_payment`, and `create_resolved_payout_receipt_if_needed`;
- from `core`: the `checked_add`, `checked_sub`, `checked_mul`, `saturating_sub` and `min`/`max` calls, `Option::ok_or`, the `?` operator's `branch`/`from_residual`, slice `len`, and the drop calls.

No library function's behaviour is assumed in place of its body.

**Not verified:** the body of `wide_mul_div_floor_u128` and the rest of `src/wide_math.rs`. Other functions of the crate were out of scope.

**Assumed:**
1. The `wide_mul_div_floor_u128` contract: if `d > 0` and `floor(a*b/d)` fits in `u128`, it returns `floor(a*b/d)` without panicking. RC1 through RC7 all depend on it, since each reaches `resolved_receipt_claimable_against_ledger`. Without it, the claimed amounts and the absence of panics in those calls are not established.
2. A standard Rust environment: every object is smaller than `2^56` bytes. In the `close_all` results this appears as the slices `accounts` and `order` having byte sizes below `2^63`.

The public specification files `Spec/Model.lean` and `Spec/RC1.lean` are published.