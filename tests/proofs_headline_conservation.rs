//! Headline Kani proof #2 (kernel level): a fee charge conserves value exactly, or fails closed.
//!
//! Kernel. `fee_charge_capital_to_insurance` is the scalar body of
//! `V16Core::charge_account_fee_current_not_atomic`, the production step that moves a trade /
//! liquidation fee out of an account's capital (and the `c_tot` aggregate) into insurance. It is
//! reached through the `cfg(kani)` shim `kani_fee_charge_capital_to_insurance`.
//!
//! Statement A (`proof_headline_fee_charge_conserves_value_exactly_or_fails_closed`). For EVERY
//! `requested_fee, capital, c_tot, insurance: u128` and EVERY `pnl: i128` (full domains, no
//! assumptions), with `want := 0 if requested_fee == 0 || pnl < 0, else min(requested_fee, capital)`:
//!   * the kernel returns `Err` exactly when `want > 0` and (`c_tot < want` -> CounterUnderflow,
//!     else `insurance + want` overflows u128 -> ArithmeticOverflow); an `Err` carries no balances,
//!     so nothing can be written (fail closed);
//!   * otherwise it returns `charged == want` and
//!       capital - capital_after == c_tot - c_tot_after == insurance_after - insurance == charged
//!     with no wrap: exactly what leaves the account enters insurance, zero created, zero destroyed;
//!   * `charged <= requested_fee` and `charged <= capital` (never over-charges);
//!   * on well-formed books (`capital <= c_tot` and `c_tot + insurance` fits in u128) it never errs.
//!
//! Statement B (`proof_headline_taker_then_maker_fee_charges_conserve_value_and_collect_exact_total`).
//! Two charges threaded the way `charge_trade_fee_taker_only_not_atomic` threads them (taker for
//! `fee`, then maker for the shortfall `fee - taker_fee` if it is nonzero): for all inputs,
//! if both succeed then the taker's + maker's capital loss == `c_tot` drop == insurance gain
//! == `fee_taker + fee_maker <= fee`; and when neither pnl is negative the pair collects exactly
//! `min(fee, taker_capital + maker_capital)`. The taker/maker sequencing is MIRRORED in this
//! harness (three lines), not called from production; only the kernel is production code.
//!
//! How this differs from the existing harnesses in tests/proofs_v16.rs
//! (`proof_v16_taker_only_conserves_notional_unaffected`,
//! `proof_v16_taker_only_never_overcharges_and_maker_pays_only_shortfall`, and the
//! `kani_charge_account_fee_current_not_atomic` harnesses): those drive the view-level function
//! on a concrete two-account fixture with u8-sized capitals / fee, `c_tot` and `vault` set equal
//! to the capital sum and pnl in {-1, 0}. They cannot reach the fail-closed arms and say nothing
//! above 255 atoms. This file proves the exact three-way conservation equality and the exact
//! error condition over the full u128 / i128 domains, including inconsistent `c_tot` and
//! insurance overflow.
//!
//! NOT proven here: the vault (unchanged by this step; the production caller separately checks a
//! `TokenValueFlowProofV16`), that the caller writes the returned balances to the right fields,
//! the fee amount derivation, the downstream split of insurance to protocol / LP / creator, or
//! any other value-moving step (PnL settlement, K/F, liquidation, ADL, withdrawals).
#![cfg(kani)]

use percolator::{kani_fee_charge_capital_to_insurance, V16Error};

#[kani::proof]
fn proof_headline_fee_charge_conserves_value_exactly_or_fails_closed() {
    let requested_fee: u128 = kani::any();
    let pnl: i128 = kani::any();
    let capital: u128 = kani::any();
    let c_tot: u128 = kani::any();
    let insurance: u128 = kani::any();

    let want = if requested_fee == 0 || pnl < 0 {
        0
    } else if requested_fee < capital {
        requested_fee
    } else {
        capital
    };
    let c_tot_short = want > 0 && c_tot < want;
    let insurance_overflows = want > 0 && insurance > u128::MAX - want;

    // The real production kernel.
    let result =
        kani_fee_charge_capital_to_insurance(requested_fee, pnl, capital, c_tot, insurance);

    match result {
        Ok((charged, capital_after, c_tot_after, insurance_after)) => {
            // Succeeds only when it can move the value exactly.
            assert!(!c_tot_short && !insurance_overflows);
            assert!(charged == want);
            assert!(charged <= requested_fee && charged <= capital);
            // Exact three-way conservation, no wrap in any direction.
            assert!(capital_after <= capital && capital - capital_after == charged);
            assert!(c_tot_after <= c_tot && c_tot - c_tot_after == charged);
            assert!(insurance_after >= insurance && insurance_after - insurance == charged);
            kani::cover!(charged > 0 && charged == requested_fee, "full fee charged");
            kani::cover!(
                charged > 0 && charged < requested_fee && capital_after == 0,
                "fee capped by capital"
            );
            kani::cover!(
                charged == 0 && requested_fee > 0 && capital > 0 && pnl < 0,
                "pnl<0 waiver: nothing moves"
            );
            kani::cover!(
                charged > (1u128 << 100),
                "conservation holds far above the u8 range of the fixture harnesses"
            );
        }
        Err(e) => {
            // Fail closed: an error only when the books cannot absorb the movement.
            assert!(c_tot_short || insurance_overflows);
            if c_tot_short {
                assert!(e == V16Error::CounterUnderflow);
            } else {
                assert!(e == V16Error::ArithmeticOverflow);
            }
            kani::cover!(c_tot_short, "fail closed: c_tot smaller than the charge");
            kani::cover!(
                !c_tot_short && insurance_overflows,
                "fail closed: insurance would overflow"
            );
        }
    }

    // Well-formed books never hit the error arms.
    if capital <= c_tot && c_tot <= u128::MAX - insurance {
        assert!(result.is_ok());
    }
}

#[kani::proof]
fn proof_headline_taker_then_maker_fee_charges_conserve_value_and_collect_exact_total() {
    let fee: u128 = kani::any();
    let taker_pnl: i128 = kani::any();
    let maker_pnl: i128 = kani::any();
    let taker_capital: u128 = kani::any();
    let maker_capital: u128 = kani::any();
    let c_tot: u128 = kani::any();
    let insurance: u128 = kani::any();

    // Mirror of charge_trade_fee_taker_only_not_atomic: taker first, maker asked for the shortfall.
    let Ok((fee_taker, taker_after, c_tot_1, insurance_1)) =
        kani_fee_charge_capital_to_insurance(fee, taker_pnl, taker_capital, c_tot, insurance)
    else {
        return;
    };
    let shortfall = fee.saturating_sub(fee_taker);
    let (fee_maker, maker_after, c_tot_2, insurance_2) = if shortfall != 0 {
        let Ok(r) = kani_fee_charge_capital_to_insurance(
            shortfall,
            maker_pnl,
            maker_capital,
            c_tot_1,
            insurance_1,
        ) else {
            return;
        };
        r
    } else {
        (0, maker_capital, c_tot_1, insurance_1)
    };

    // Never over-collects, and the sum cannot wrap.
    assert!(fee_taker <= fee && fee_maker <= fee - fee_taker);
    let collected = fee_taker + fee_maker;
    // Pairwise conservation: both capital debits == c_tot drop == insurance gain == collected.
    assert!(taker_after <= taker_capital && maker_after <= maker_capital);
    assert!((taker_capital - taker_after) == fee_taker);
    assert!((maker_capital - maker_after) == fee_maker);
    assert!(c_tot_2 <= c_tot && c_tot - c_tot_2 == collected);
    assert!(insurance_2 >= insurance && insurance_2 - insurance == collected);

    // With neither side waived, the pair pays exactly min(fee, combined capital).
    if taker_pnl >= 0 && maker_pnl >= 0 {
        match taker_capital.checked_add(maker_capital) {
            Some(both) if both < fee => assert!(collected == both),
            _ => assert!(collected == fee),
        }
    }

    kani::cover!(fee_taker > 0 && fee_maker > 0, "taker partial, maker pays the remainder");
    kani::cover!(
        fee > 0 && fee_taker == 0 && fee_maker == fee,
        "taker waived or empty, maker pays the whole fee"
    );
    kani::cover!(
        fee > 0 && collected < fee && taker_pnl >= 0 && maker_pnl >= 0,
        "combined capital short of the fee"
    );
    kani::cover!(fee > 0 && collected == 0, "both waived or empty: nothing moves");
}
