# Specification: the resolved close and the payout top-up, kernels first

The crate is uploaded exactly as committed at engine revision `c38b6b1e` (`src/v16.rs` blob
`7d3edfe8`, `src/wide_math.rs` blob `97c6b16d`); no file of it was edited. Only `src/v16.rs`,
`src/wide_math.rs` and `src/lib.rs` matter; the other files are there so the tree is one consistent
revision. Do not verify anything under `tests/`.

Verify the deployed function bodies: no `cfg(kani)`, default features, this crate's release
profile (`overflow-checks = true`). `BOUND_SCALE` is `1_000_000_000_000` (`src/lib.rs`).

## How to read the properties

- `X.contract`: for all inputs that satisfy every hypothesis of the property, if the call returns,
  the stated conclusion holds. A parameter name in a conclusion denotes the value the caller
  passed, also where the body mutates or rebinds a by-value parameter.
- `X.nopanic`: for all inputs that satisfy every hypothesis of the property, no execution of the
  call panics, including by arithmetic overflow. It does not ask for a proof that the call
  returns; say whether the body has a loop and, if so, the bound you used.
- If a property is false as written, report it as refuted with a concrete counterexample. Do not
  weaken a property silently; say so if you had to add a hypothesis.

## The one arithmetic lemma that may be assumed

- F3. `wide_mul_div_floor_u128(a, b, d)` panics exactly when `d = 0` or `floor(a*b/d) >= 2^128`;
  otherwise it returns `floor(a*b/d)` (the product is exact, it does not have to fit in `u128`).

F3 is already proved for this exact function text on another project (a function-by-function
comparison found `wide_mul_div_floor_u128` unchanged, with no changed callee and no changed type).
Use it as a hypothesis about the callee wherever it is needed and say in each result that it was
used. Do not re-prove it.

## Part 1. Pure functions on the payout path

All four take and return small `Copy` records; none takes a market or an account view.

### PR1. `V16Core::kernel_recompute_resolved_payout_rate(ledger)`

Write `E = ledger.terminal_claim_exact_receipts_num`, `U = ledger.terminal_claim_bound_unreceipted_num`,
`R = ledger.snapshot_residual`.

- PR1.contract: it returns `Err(ArithmeticOverflow)` exactly when `E + U >= 2^128`, or
  `E + U != 0` and `R * BOUND_SCALE >= 2^128`; it never returns another error. If it returns
  `Ok(l)`: `1 <= l.current_payout_rate_den` and `l.current_payout_rate_num <= l.current_payout_rate_den`;
  if `E + U = 0` then `l.current_payout_rate_num = l.current_payout_rate_den = 1`, otherwise
  `l.current_payout_rate_den = E + U` and `l.current_payout_rate_num = min(R * BOUND_SCALE, E + U)`;
  every other field of `l` equals the same field of `ledger`.
- PR1.nopanic: no hypothesis.

### P15. `MarketGroupV16ViewMut::resolved_receipt_claimable_against_ledger(receipt, ledger)`

An associated function (no `self`); it reaches `wide_mul_div_floor_u128` and
`validate_resolved_payout_receipt_value` (through `PortfolioV16View::validate_resolved_payout_receipt_static`).
Write `face = receipt.terminal_positive_claim_face`, `paid = receipt.paid_effective`,
`num = ledger.current_payout_rate_num`, `den = ledger.current_payout_rate_den`.
The hypothesis **H_rate** is `1 <= den` and `num <= den` (the ledger-rate invariant; PR1 is the
recompute half of it).

- P15.nopanic: under H_rate, no execution panics; in particular the call to
  `wide_mul_div_floor_u128` meets neither panic condition of F3 (`d = den != 0`, and
  `floor(face*num/den) <= face < 2^128`). Also state whether it can panic without H_rate, with an
  input if it can.
- P15.contract: if it returns `Ok(c)`: if `receipt.present` is false then `c = 0`; otherwise
  `ledger.payout_halted` is false, `receipt` passes `validate_resolved_payout_receipt_value`, and
  `c = floor(face*num/den) - paid` (as integers, so `floor(face*num/den) >= paid`). Under H_rate
  moreover `c <= face - paid`. It returns `Err(RecoveryRequired)` exactly when the receipt is valid,
  present and `ledger.payout_halted` is true.

### OURS-K1. `apply_resolved_payout_receipt_payment(receipt, actual_resolved_paid)` (free function)

No ledger row of its own; it is a callee of both the close and the top-up. Write `p = actual_resolved_paid`.

- OURS-K1.contract: if it returns `Ok(r)`: `receipt` passes `validate_resolved_payout_receipt_value`,
  and so does `r`. If `p = 0` then `r = receipt`. If `p != 0` then `receipt.present`,
  `p <= face - paid` (with `face`, `paid` of `receipt`), `r.paid_effective = paid + p`,
  `r.finalized = (r.paid_effective = r.terminal_positive_claim_face)`, and every other field of
  `r` equals the same field of `receipt`.
- OURS-K1.nopanic: no hypothesis.

### OURS-K2. The token-flow check of the close and of the top-up

The close (`close_resolved_account_not_atomic`) ends with
`TokenValueFlowProofV16::capital_and_resolved_payout_to_external_out(capital_paid, resolved_paid, payout, vault_before, vault_after)?.validate()?`,
and the top-up core (`claim_resolved_payout_topup_core_not_atomic`) with the same call on
`(0, payout, payout, vault_before, vault_after)`. Write the arguments `(c, r, t, vb, va)`.

- OURS-K2.contract: `capital_and_resolved_payout_to_external_out(c, r, t, vb, va)` returns `Ok(p)`
  and `p.validate()` returns `Ok(())` if and only if `c + r < 2^128`, `c + r = t`, `va <= vb` and
  `vb - va = t`. (On the paths that reach it, the vault falls by exactly the amount paid out, and
  that amount is the capital paid plus the resolved payout paid.)
- OURS-K2.nopanic: neither call panics, for all inputs. `validate` has a loop of
  `V16_TOKEN_VALUE_CLASS_COUNT = 17` iterations.

## Part 2. One small entry point: the payout top-up

`MarketGroupV16ViewMut::<'_, ()>::claim_resolved_payout_topup_not_atomic(account)`. Its body is four
calls with `?` and the return:

    self.preflight_claim_resolved_payout_topup(&account.as_view())?;
    let payout = self.claim_resolved_payout_topup_core_not_atomic(account)?;
    self.validate_shape()?;
    account.validate_with_market(&self.as_view())?;
    Ok(payout)

Treat `preflight_claim_resolved_payout_topup` and `claim_resolved_payout_topup_core_not_atomic`
as opaque calls: the second may write any field of the market and of the account. Do not go into
their bodies. `validate_shape` and `validate_with_market` take `&self`; a default build has no
`audit-scan` feature.

- T14.header: if the call returns `Ok(_)`, then `validate_shape()` returns `Ok(())` on the market
  state the call leaves. (The header predicate of the ledger is `validate_shape` of a default
  build.) Prove it from the path structure: the return is preceded by an `Ok` return of
  `validate_shape()`, and the only call between them, `validate_with_market`, takes the market
  through a shared view and writes nothing of it; say what you need about `validate_shape` being a
  function of the market state alone.
- T14.account: if the call returns `Ok(_)`, then `account.validate_with_market(&market.as_view())`
  returns `Ok(())` for the account and the market state the call leaves.

These two are the whole of ledger rows T14.header and T14.account. T14.scan and T14.mirror are not
asked for here.
