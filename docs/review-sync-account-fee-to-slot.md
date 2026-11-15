# Behaviour review: `sync_account_fee_to_slot_not_atomic` (src/v16.rs)

Job J-20261115-ENG-CU-FEESYNC. Purpose: list what must stay identical for a CU-only cut.

## Order of effects today

1. `account.validate_with_market(market)` (read-only). Error: any validation error.
2. Mode `Recovery` -> `LockActive`. `now_slot < last_fee_slot` -> `Stale`.
3. Fee anchor: Live and non-flat -> `account_fee_anchor_for_loss_currentness` (read-only);
   Resolved -> `resolved_slot`; else `now_slot`. `anchor <= last_fee_slot` -> `Ok(0)`, no writes.
4. `requested_fee = fee_rate * dt` (U256, saturating to `u128::MAX` on u128 overflow).
5. Live and non-flat: `settle_account_side_effects_not_atomic` (validate again, K/F settle, B chunks
   per leg, negative-PnL principal settle, `health_cert.valid = 0`). `AccountBChunk` -> `BStale`.
6. `settle_negative_pnl_from_principal_core_not_atomic` again (not provably idempotent: the
   `paid == 0` branch can re-mark the bankruptcy hlock, so it stays).
7. `charge_account_fee_current_not_atomic`: fee = min(requested, capital), skipped if PnL < 0;
   capital -> insurance, `c_tot` down, vault unchanged, `health_cert.valid = 0`.
8. `last_fee_slot = anchor`; `validate_with_market` again; `validate_shape`.

## Must stay identical

- Capital, PnL, `c_tot`, vault, insurance, `fee_credits`, legs, B/K/F snapshots, `health_cert`
  validity, `last_fee_slot`, bankruptcy-hlock state, and the returned `charged` value.
- Error kind and error precedence (validate, Recovery, Stale, anchor errors, overflow, BStale, ...).
  Early errors leave the account untouched; later errors keep today's partial-write behaviour
  (these are `_not_atomic`; the wrapper rolls the account back on error).
- Liquidation outcome of the crank that follows (the refresh walk reads the state written here).

## Cost shape

The Live/non-flat path validates the same account twice before any write (step 1, then at the top of
`settle_account_side_effects_not_atomic`), and a third time at step 8. Step 1 and the step 5 entry
check run on identical state (the anchor lookup is `&self`/`&account`), so the second is redundant.

## The cut

`settle_account_side_effects_not_atomic` keeps its validate and delegates to a new
`..._prevalidated_not_atomic` body. The fee sync calls the prevalidated body. Other callers are
unchanged. Same state in, same checks passed, so no behaviour change.

## Pins

`tests/fee_sync_refresh_equivalence.rs` (golden digest of capital/PnL/fee slot/legs/insurance/vault/
`c_tot`/cert/liquidation outcome) must pass unchanged, plus `slot_gap_time_edge`, `fees_liens_edge`.
