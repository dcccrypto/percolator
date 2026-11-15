# Haircut-order fix B: liquidation counts in-flight counterparty backing

Status: implemented on `fix/v22-haircut-order-b` (base `feat/v22-engine-rc23-2026-10-22`).
Decision: rule B (keep the backed-profit rule of spec.md:266 for equity; fix the liquidation
trigger so that a counterparty's pending settlement is treated as backing it will book).

## Upstream check

Checked before writing: `aeyakovenko/percolator` master `827a125f` and
`aeyakovenko/percolator-prog` main `8b10d3b2`. Neither has a fix for the refresh-order
liquidation (#346 / #350). Newer upstream branches (`5e3af071`, `826eb973`) touch unrelated
code. So this is our own fix, written to stay inside Toly's model: no new state, no layout
change, the backed-profit rule for equity untouched.

## The bug (#346, #350)

`account_haircut_equity` counts a positive PnL claim only as far as its source domain is
backed (`SourceCreditStateV16`: available backing / claim bound, at the stored credit rate).
K/F settlement credits winners first and losers only when *they* are refreshed. So for a
hedged account A (long asset 0 vs B, short asset 1 vs C) after a move:

* refresh A first: A's losing leg is booked in full, its winning leg is credited a claim
  whose backing (C's loss) has not been booked yet, so the claim is haircut to ~0;
* A's backed equity falls below maintenance and A is liquidated, although once C is
  refreshed A is solvent.

Crank order alone decided whether a solvent hedge was liquidated (#346: 98_198_198 /
17_647_058 lots closed in the account-first order, 0 in the counterparty-first order; #350:
fee-loss tables peaking at 210 / 425 quote atoms for fees 50 / 100).

## Approach

The engine already tracks the in-flight term exactly, per asset/side domain:
`kf_pending_credit` = claims credited to winners by K/F settlement minus loss realized by
losers, clamped to `[0, claims]` by `kf_pending_credit_num(domain)`. R1 loss netting already
uses it (`source_credit_netting_rate(state, booked, extra, pending) = min(1, max(stored,
(A + P) / C))`).

Fix B adds a *liquidation-only* equity:

* `V16Core::source_credit_state_in_flight_support_for_claim_num(state, claim, P)`:
  rate = `source_credit_netting_rate(state, 0, 0, P)`, support =
  `min(claim * rate, available + P)` (floored to atoms). With `P == 0` it is exactly
  `source_credit_state_realizable_support_for_claim_num`.
* `account_source_support_inner(account, face, in_flight)`: the existing per-domain walk;
  with `in_flight` the *unliened* claims use the kernel above. Liened and impaired claims
  are valued exactly as before.
* `account_liquidation_equity` (in-flight) next to the unchanged `account_haircut_equity`.
* `account_liquidation_deficit(account, mm)` computes `certified_liq_deficit` from the
  liquidation equity. All four cert builders use it
  (`compute_account_health_cert_with_price_override`,
  `refresh_account_and_certify_not_atomic`, `recertify_account_after_source_lien_change`,
  the position-delta cert). `certified_equity` stays the backed (haircut) equity.
* `liquidate_account_inner_not_atomic` sizes the close with a cert whose
  `certified_equity` is the liquidation equity, so a partial close stops where the
  in-flight-backed account is healthy, consistent with the trigger.
* `health_cert_after_capital_debit` is unchanged: it only returns Ok with deficit 0.

## Invariants

1. Only the liquidation trigger and liquidation sizing read the in-flight value.
   Withdrawals, initial-margin gates, payouts and `certified_equity` still use backed
   equity (spec.md:266 kept). A hedge in the window cannot withdraw or add risk against
   unbooked backing.
2. `in_flight_support >= realizable_support` per claim (rate >= stored rate, cap >=
   available), equal when nothing is in flight. So fix B can only *remove* liquidations
   that depended on settlement order, never add one.
3. `in_flight_support <= claim face` and `<= available + P`: a claim is never valued above
   what the domain will hold once every in-flight loser settles.
4. Liveness: refreshing a loser lowers `P` by its realized loss. A loser that is bankrupt
   realizes less than its K/F debit; the unrealized part leaves `P` (settled, not in flight)
   and is then haircut as before. So an account that is truly insolvent becomes
   liquidatable as soon as its counterparties are refreshed (refresh is permissionless).
   Test: `hedged_refresh_fix_b_bankrupt_counterparty_settles_then_hedge_is_liquidatable`.
5. No new state, no account/slab layout change, no new instruction.

Kani: `proof_v16_haircut_b_in_flight_support_bounds` (verified, kani 0.67, atoms <= 31) proves 2, 3 and the `P == 0`
equality for the kernel over symbolic claim / stock / backing / pending.

## Tests

* `tests/hedged_refresh_order_liquidation.rs`: the two #346 cases are un-ignored and now
  assert that the account-first order no longer liquidates the solvent hedge, and that an
  asset-1 target is refused too. Window tests assert backed equity < MM but no liquidation.
  New: in-flight gain is not certified equity, auto-crank does not liquidate the hedge,
  bankrupt counterparty settles then the hedge is liquidatable.
* `tests/hedged_refresh_fee_loss.rs` (#350): `TABLE_50` / `TABLE_100` are now all zero
  (account-first loss 0); the pre-fix tables are kept as `*_BEFORE_FIX` for reference.

## CU impact

Liquidation trigger now walks the account's source-claim domains twice per cert (once for
backed equity, once for liquidation equity) plus one `kf_pending_credit_num` read and one
`source_credit_netting_rate` per unliened domain. Accounts with no source claims pay
nothing extra. Same-order cost as the existing walk; expected a small single-digit percent
on refresh/liquidate paths. Not measured on SBF in this PR (the prog pins the engine at
`ENGINE_CI_SIBLING`); measure with `tests/v16_cu` when the prog pin is bumped.

## Wrapper / downstream

No percolator-prog change needed (no ABI, layout or instruction change). Follow-up after
merge: bump `ENGINE_CI_SIBLING` in percolator-prog `ci/deployed-refs.env` and rerun its
suite; keeper liquidation candidate selection may now see fewer candidates in the window.

## Open risks

* `kf_pending_credit` was documented as read only for loss netting; it now gates
  liquidation, so any drift in it (saturating flow balance, dust residue) shifts the
  trigger by that dust.
* In the window a hedge is "healthy" for liquidation while backed equity < MM. If the
  in-flight losers are never refreshed, the account stays unliquidated until they are;
  refresh is permissionless and the crank refreshes them.
* Existing Kani proofs about deficit/sizing assume deficit derives from
  `certified_equity`; reviewers should check none relies on that identity.
