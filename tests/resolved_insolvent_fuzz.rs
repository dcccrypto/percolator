//! Fuzz/integration coverage for Finding D — the parts that are intractable for Kani.
//!
//! Kani proves the receipt-clearing PRIMITIVE (via claim_resolved_payout_topup) on a
//! concrete insolvent case, but the full `close_resolved_account_not_atomic` path
//! (payout-receipt machinery + validate_shape + validate_with_market) times out under
//! the model checker. These randomized integration tests drive the real end-to-end
//! close/topup of an insolvent (haircut) resolved market and assert the winner's
//! fully-diluted receipt is cleared so the portfolio can dematerialize — i.e. the market
//! is drainable, not permanently stranded.

use percolator::v16::{
    EngineAssetSlotV16Account, Market, MarketGroupV16HeaderAccount, MarketGroupV16ViewMut,
    PortfolioAccountV16Account, PortfolioV16ViewMut, ProvenanceHeaderV16,
    ProvenanceHeaderV16Account, ResolvedCloseOutcomeV16, ResolvedPayoutLedgerV16,
    ResolvedPayoutLedgerV16Account, ResolvedPayoutReceiptV16, ResolvedPayoutReceiptV16Account,
    V16Config, V16PodI128, V16PodU128, V16PodU64,
};
use percolator::BOUND_SCALE;
use proptest::prelude::*;

fn market_id() -> [u8; 32] {
    [1u8; 32]
}

fn empty_account() -> PortfolioAccountV16Account {
    let header = ProvenanceHeaderV16Account::from_runtime(&ProvenanceHeaderV16::new(
        market_id(),
        [2u8; 32],
        [2u8; 32],
    ));
    let mut account = PortfolioAccountV16Account::default();
    account.init_empty_in_place(header).unwrap();
    account
}

fn fresh_activated_market() -> (MarketGroupV16HeaderAccount, [Market<u64>; 1]) {
    let cfg = V16Config::public_user_fund_with_market_slots(1, 1, 0, 10);
    let mut header = MarketGroupV16HeaderAccount::new_dynamic(market_id(), cfg, 1, 0).unwrap();
    let mut markets = [Market::new(0u64, EngineAssetSlotV16Account::default())];
    {
        let mut view = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        view.activate_empty_market_not_atomic(0, 100, 1).unwrap();
    }
    (header, markets)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(400))]

    /// END-TO-END close of a single-winner INSOLVENT resolved market (the path Kani
    /// times out on). The winner holds the entire junior bound (so its receipt makes the
    /// payout rate terminal) and the residual is strictly less than the bound (haircut),
    /// so the winner can never be paid its full face. close_resolved must still fully
    /// settle: pay the haircut entitlement, clear the fully-diluted receipt, and
    /// dematerialize the account, leaving the market drainable (vault and c_tot drop to 0).
    #[test]
    fn close_resolved_insolvent_winner_dematerializes(
        pnl in 2u128..=1_000_000u128,
        capital in 0u128..=1_000_000u128,
        residual_frac in 1u128..=999u128,
    ) {
        // residual strictly below the winner's bound => payout rate < 1 (insolvent).
        let residual = (pnl.saturating_mul(residual_frac) / 1000).max(1).min(pnl - 1);
        prop_assume!(residual < pnl);

        let (mut header, mut markets) = fresh_activated_market();
        // Pre-resolution senior/junior state: single winner with bound == pnl, vault only
        // covers the winner's capital plus the haircut residual.
        header.mode = 1; // Resolved
        header.resolved_slot = V16PodU64::new(1);
        header.current_slot = V16PodU64::new(1);
        header.vault = V16PodU128::new(capital + residual);
        header.c_tot = V16PodU128::new(capital);
        header.pnl_pos_tot = V16PodU128::new(pnl);
        header.pnl_matured_pos_tot = V16PodU128::new(pnl);
        header.pnl_pos_bound_tot = V16PodU128::new(pnl);
        header.pnl_pos_bound_tot_num = V16PodU128::new(pnl * BOUND_SCALE);

        let mut account_header = empty_account();
        account_header.capital = V16PodU128::new(capital);
        account_header.pnl = V16PodI128::new(pnl as i128);
        account_header.last_fee_slot = V16PodU64::new(1);

        let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        let mut account = PortfolioV16ViewMut::new(&mut account_header);

        // Start state is valid/reachable.
        prop_assert_eq!(market.validate_shape(), Ok(()));
        prop_assert_eq!(account.validate_with_market(&market.as_view()), Ok(()));

        let outcome = market
            .close_resolved_account_not_atomic(&mut account, 0)
            .expect("insolvent winner close must not revert");

        let receipt = account.header.resolved_payout_receipt.try_to_runtime().unwrap();
        // The winner is paid capital + the haircut residual...
        let is_closed = matches!(outcome, ResolvedCloseOutcomeV16::Closed { .. });
        prop_assert!(is_closed, "insolvent winner did not fully close");
        // ...and the fully-diluted receipt is cleared so the portfolio dematerializes.
        prop_assert!(!receipt.present, "receipt left present -> market would be stranded");
        prop_assert_eq!(account.header.pnl.get(), 0);
        prop_assert_eq!(account.header.capital.get(), 0);
        prop_assert_eq!(market.validate_shape(), Ok(()));
        prop_assert_eq!(account.validate_with_market(&market.as_view()), Ok(()));
    }

    /// topup must NOT clear a receipt while the rate is still non-terminal (unreceipted
    /// bound remains, so the rate could still rise): clearing early would forfeit a
    /// winner's future entitlement. Fully paid at the CURRENT rate but NOT terminal ->
    /// receipt stays present.
    #[test]
    fn topup_does_not_clear_while_rate_nonterminal(
        face in 2u128..=100_000u128,
        residual_frac in 1u128..=999u128,
        unreceipted_extra in 1u128..=100u128,
    ) {
        let residual = (face.saturating_mul(residual_frac) / 1000).max(1).min(face - 1);
        prop_assume!(residual < face);
        let total_bound_num = face * BOUND_SCALE;

        let (mut header, mut markets) = fresh_activated_market();
        header.mode = 1;
        header.vault = V16PodU128::new(residual);
        header.payout_snapshot_captured = 1;
        header.resolved_payout_ledger =
            ResolvedPayoutLedgerV16Account::from_runtime(&ResolvedPayoutLedgerV16 {
                snapshot_residual: residual,
                terminal_claim_exact_receipts_num: total_bound_num,
                // NON-terminal: unreceipted bound still outstanding.
                terminal_claim_bound_unreceipted_num: unreceipted_extra * BOUND_SCALE,
                current_payout_rate_num: residual * BOUND_SCALE,
                current_payout_rate_den: total_bound_num + unreceipted_extra * BOUND_SCALE,
                snapshot_slot: 1,
                payout_halted: false,
                finalized: false,
            });

        // gross at current rate = floor(face * residual*SCALE / (total+extra)); pay exactly
        // that so claimable == 0 but the rate is not terminal.
        let rate_den = total_bound_num + unreceipted_extra * BOUND_SCALE;
        let gross = ((face as u128) * (residual * BOUND_SCALE)) / rate_den;
        let mut account_header = empty_account();
        account_header.resolved_payout_receipt =
            ResolvedPayoutReceiptV16Account::from_runtime(&ResolvedPayoutReceiptV16 {
                present: true,
                prior_bound_contribution_num: total_bound_num,
                live_released_face_at_receipt: 0,
                terminal_positive_claim_face: face,
                paid_effective: gross,
                finalized: false,
            });

        let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        let mut account = PortfolioV16ViewMut::new(&mut account_header);
        prop_assume!(account.validate_with_market(&market.as_view()) == Ok(()));

        let paid = market.claim_resolved_payout_topup_not_atomic(&mut account).unwrap();
        let receipt = account.header.resolved_payout_receipt.try_to_runtime().unwrap();
        prop_assert_eq!(paid, 0);
        // Rate not terminal => must NOT clear (the winner may still be owed more later).
        prop_assert!(receipt.present, "cleared a receipt while the rate could still rise");
    }
}
