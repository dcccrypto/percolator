//! GH#290 -- a Recovery leg whose ceil'd effective quantity exceeds its side's floor'd
//! aggregate by the rounding gap must still be forfeitable, and the owner's capital must
//! then be withdrawable.
//!
//! Finding and the four-site enumeration by @0x-SquidSol (dcccrypto/percolator#290, #291).
//! Their #291 widened the drain predicate; the drain refuses any `oi_eff_side != 0` with
//! `LockActive`, so forfeit stayed wedged. This fix instead absorbs the gap on the side's LAST
//! stored leg when the leg is detached (detach clamps the OI subtraction to the aggregate).
//!
//! The mirror direction (side OI exceeds its legs) is aeyakovenko/percolator#238 and is
//! deliberately untouched: `controls_*` pin that the clamp never fires there.

use percolator::{
    AssetLifecycleV16, AssetStateV16, EngineAssetSlotV16Account, Market,
    MarketGroupV16HeaderAccount, MarketGroupV16ViewMut, PortfolioAccountV16Account,
    PortfolioV16ViewMut, ProvenanceHeaderV16, ProvenanceHeaderV16Account, TradeRequestV16,
    V16Config, V16Error,
};
use percolator::ADL_ONE;

const PRICE: u64 = 1_000_000;
/// Long side scaled to ~1/3 by a prior one-sided ADL; with `LEG_BASIS` the ceil lands exactly
/// one atom above the floor aggregate (same numbers as the #291 unit fixture).
const A_LONG: u128 = 333_333_333_333_333;
const LEG_BASIS: u128 = 4_000_002;
const OI_EFF_LONG: u128 = 1_333_333;

fn account(seed: u8) -> PortfolioAccountV16Account {
    let header = ProvenanceHeaderV16Account::from_runtime(&ProvenanceHeaderV16::new(
        [1; 32],
        [seed; 32],
        [3; 32],
    ));
    let mut a = PortfolioAccountV16Account::default();
    a.init_empty_in_place(header).unwrap();
    a
}

struct World {
    header: MarketGroupV16HeaderAccount,
    markets: Vec<Market<u64>>,
    long: PortfolioAccountV16Account,
    short: PortfolioAccountV16Account,
}

impl World {
    fn asset(&self) -> AssetStateV16 {
        self.markets[0].engine.asset.try_to_runtime().unwrap()
    }
    fn set_asset(&mut self, asset: AssetStateV16) {
        self.markets[0].engine.asset =
            percolator::AssetStateV16Account::from_runtime(&asset);
    }
}

/// Real deposits + a real opening trade of `LEG_BASIS`, then the side is put in the
/// post-haircut state by writing the asset aggregate (a_long, oi_eff_long) exactly as the
/// `oi_eff == floor(a * sum_w / SCALE)` relation requires, and the asset is forced to Recovery.
fn world(oi_eff_long: u128) -> World {
    let cfg = V16Config::public_user_fund_with_market_slots(1, 1, 0, 10);
    let mut header = MarketGroupV16HeaderAccount::new_dynamic([1; 32], cfg, 1, 0).unwrap();
    let mut markets = vec![Market::new(0, EngineAssetSlotV16Account::default())];
    header
        .activate_empty_asset_slot_not_atomic(0, &mut markets[0].engine, PRICE, 1)
        .unwrap();
    let mut w = World {
        header,
        markets,
        long: account(10),
        short: account(11),
    };
    {
        let mut m = MarketGroupV16ViewMut::new(&mut w.header, &mut w.markets);
        for acct in [&mut w.long, &mut w.short] {
            let mut v = PortfolioV16ViewMut::new(acct);
            m.deposit_not_atomic(&mut v, 400_000_000).unwrap();
        }
        let mut l = PortfolioV16ViewMut::new(&mut w.long);
        let mut s = PortfolioV16ViewMut::new(&mut w.short);
        m.execute_trade_with_fee_loss_stale_scoped_not_atomic(
            &mut l,
            &mut s,
            TradeRequestV16 {
                asset_index: 0,
                size_q: LEG_BASIS as i128,
                exec_price: PRICE,
                fee_bps: 0,
            },
            true,
        )
        .unwrap();
    }
    let mut asset = w.asset();
    assert_eq!(asset.oi_eff_long_q, LEG_BASIS, "real trade opened the leg");
    assert_eq!(asset.stored_pos_count_long, 1);
    asset.a_long = A_LONG;
    asset.oi_eff_long_q = oi_eff_long;
    w.set_asset(asset);
    {
        let mut m = MarketGroupV16ViewMut::new(&mut w.header, &mut w.markets);
        m.force_asset_recovery_not_atomic(0, 4).unwrap();
    }
    assert_eq!(w.asset().lifecycle, AssetLifecycleV16::Recovery);
    w
}

#[test]
fn the_fixture_has_a_one_atom_ceil_over_floor_gap() {
    let mut w = world(OI_EFF_LONG);
    let asset = w.asset();
    let leg = w.long.legs[0].try_to_runtime().unwrap();
    assert_eq!(leg.basis_pos_q as u128, LEG_BASIS);
    assert_eq!(leg.a_basis, ADL_ONE);
    // floor aggregate vs ceil leg: oi_eff == floor(a * leg_basis / ONE), leg ceil = oi_eff + 1.
    let exact = A_LONG * LEG_BASIS;
    assert_eq!(exact / ADL_ONE, OI_EFF_LONG);
    assert_ne!(exact % ADL_ONE, 0);
    assert_eq!(asset.oi_eff_long_q, OI_EFF_LONG);
    let view = MarketGroupV16ViewMut::new(&mut w.header, &mut w.markets);
    view.validate_shape().expect("fixture must be a valid shape");
}

/// The #290 state: the opposite side holds only a zero-basis obligation (its owner forfeited), so the long's leg is
/// the side's last stored position and its owner's exit is a clean detach.
fn forfeit_short(w: &mut World) {
    let mut m = MarketGroupV16ViewMut::new(&mut w.header, &mut w.markets);
    let mut short = PortfolioV16ViewMut::new(&mut w.short);
    m.forfeit_recovery_leg_not_atomic(&mut short, 0, u128::MAX)
        .expect("the short's leg detaches (its side OI covers it)");
}

/// RED on base: forfeit returns CounterUnderflow and the capital is stranded.
#[test]
fn forfeit_then_withdraw_succeed_on_a_one_atom_gap() {
    let mut w = world(OI_EFF_LONG);
    forfeit_short(&mut w);
    // retained as a zero-basis pending obligation: no real position left on the short side
    assert_eq!(w.asset().pending_obligation_count_short, w.asset().stored_pos_count_short);
    let mut m = MarketGroupV16ViewMut::new(&mut w.header, &mut w.markets);
    let mut long = PortfolioV16ViewMut::new(&mut w.long);

    // The wedge: the leg is attached, so withdraw refuses before the forfeit.
    assert!(m.withdraw_not_atomic(&mut long, 1).is_err());

    let outcome = m
        .forfeit_recovery_leg_not_atomic(&mut long, 0, u128::MAX)
        .expect("forfeit must absorb the rounding gap, not underflow");
    assert!(outcome.detached, "the leg must actually detach: {outcome:?}");
    m.validate_shape().unwrap();

    m.withdraw_not_atomic(&mut long, 1)
        .expect("capital must be withdrawable once the leg is gone");
    drop(m);
    let asset = w.asset();
    assert_eq!(asset.stored_pos_count_long, 0);
    assert_eq!(asset.oi_eff_long_q, 0, "the gap was absorbed, not left behind");
    assert_eq!(asset.loss_weight_sum_long, 0);
}

/// Control: a gap larger than the side's leg count is NOT absorbed (fail loud, not silent).
#[test]
fn controls_a_gap_wider_than_the_leg_count_still_fails_closed() {
    let mut w = world(OI_EFF_LONG - 5);
    forfeit_short(&mut w);
    let mut m = MarketGroupV16ViewMut::new(&mut w.header, &mut w.markets);
    let mut long = PortfolioV16ViewMut::new(&mut w.long);
    let err = m
        .forfeit_recovery_leg_not_atomic(&mut long, 0, u128::MAX)
        .err();
    assert_eq!(err, Some(V16Error::CounterUnderflow));
}

/// Control (upstream #238 direction): aggregate >= leg => nothing is clamped, the exact
/// quantity is subtracted and the residue stays visible for #238's owner to handle.
#[test]
fn controls_oi_exceeding_the_leg_is_not_touched() {
    let mut w = world(OI_EFF_LONG + 1_000);
    forfeit_short(&mut w);
    let before = w.asset().oi_eff_long_q;
    let mut m = MarketGroupV16ViewMut::new(&mut w.header, &mut w.markets);
    let mut long = PortfolioV16ViewMut::new(&mut w.long);
    m.forfeit_recovery_leg_not_atomic(&mut long, 0, u128::MAX)
        .expect("aggregate covers the leg: ordinary detach");
    drop(m);
    // detach subtracted exactly ceil(leg) = OI_EFF_LONG + 1, nothing more: the residue remains.
    assert_eq!(w.asset().oi_eff_long_q, before - (OI_EFF_LONG + 1));
    assert_eq!(w.asset().oi_eff_long_q, 999);
}
