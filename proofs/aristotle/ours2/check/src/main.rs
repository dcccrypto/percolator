//! Random search for counterexamples to RCs1/RCs2/RCs4/RCs5/RCs7 on the REAL slice crate (the
//! Rust that is uploaded). Not a proof. Run: `cargo run --release -- <iterations>`.
//! Also reports how often run 1's invariant (no ghost list of cleared receipts) fails after a
//! close that kept the new invariant, and prints the first such state.
use percolator_resolved_close_slice2 as s;
use s::engine_slice::harness::{close_all, close_one, topup_one};
use s::engine_slice::*;
use s::BOUND_SCALE as S;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 { self.0 ^= self.0 << 13; self.0 ^= self.0 >> 7; self.0 ^= self.0 << 17; self.0 }
    fn below(&mut self, n: u64) -> u64 { if n == 0 { 0 } else { self.next() % n } }
    fn chance(&mut self, pct: u64) -> bool { self.below(100) < pct }
}

type H = MarketGroupV16HeaderAccount;
type A = PortfolioAccountV16Account;
type Ghost = Vec<(u128, u128)>; // (face, paid) of receipts the engine cleared

fn u(v: u128) -> V16PodU128 { V16PodU128::new(v) }
fn u64p(v: u64) -> V16PodU64 { V16PodU64::new(v) }

fn gross(face: u128, l: &ResolvedPayoutLedgerV16) -> u128 {
    // values are small here, so plain u128 arithmetic is exact
    face * l.current_payout_rate_num / l.current_payout_rate_den
}

/// The receipts the sums range over: present receipts of the accounts, then the ghost list.
fn receipts(accs: &[A], ghost: &Ghost, with_ghost: bool) -> Option<Vec<(u128, u128)>> {
    let mut v = vec![];
    for a in accs {
        let r = a.resolved_payout_receipt.try_to_runtime().ok()?;
        if r.present { v.push((r.terminal_positive_claim_face, r.paid_effective)); }
    }
    if with_ghost { v.extend(ghost.iter().copied()); }
    Some(v)
}

fn valid_receipt(r: ResolvedPayoutReceiptV16) -> bool {
    if !r.present { return r.is_empty(); }
    match r.terminal_positive_claim_face.checked_mul(S) {
        None => false,
        Some(e) => !(e > r.prior_bound_contribution_num || r.paid_effective > r.terminal_positive_claim_face
            || r.finalized != (r.paid_effective == r.terminal_positive_claim_face)),
    }
}

fn rate_fix(l: &ResolvedPayoutLedgerV16) -> bool {
    let Some(t) = l.terminal_claim_exact_receipts_num.checked_add(l.terminal_claim_bound_unreceipted_num) else { return false };
    if t == 0 { return l.current_payout_rate_num == 1 && l.current_payout_rate_den == 1; }
    let Some(m) = l.snapshot_residual.checked_mul(S) else { return false };
    l.current_payout_rate_num == m.min(t) && l.current_payout_rate_den == t
}

/// Inv of aristotle/resolved_close_spec.md. `with_ghost = false` gives run 1's invariant
/// (sums over present receipts only), for the captured case.
fn inv(h: &H, accs: &[A], ghost: &Ghost, with_ghost: bool) -> bool {
    if h.payout_snapshot_captured > 1 { return false; }
    let Ok(l) = h.resolved_payout_ledger.try_to_runtime() else { return false };
    let mut csum = 0u128;
    for a in accs {
        let Ok(r) = a.resolved_payout_receipt.try_to_runtime() else { return false };
        if !valid_receipt(r) { return false; }
        csum += a.capital.get();
    }
    if h.c_tot.get() != csum { return false; }
    let Some(rs) = receipts(accs, ghost, with_ghost) else { return false };
    if h.payout_snapshot_captured == 0 {
        return rs.is_empty() && h.c_tot.get() + h.insurance.get() <= h.vault.get();
    }
    if !rate_fix(&l) || l.payout_halted { return false; }
    let cover: u128 = rs.iter().map(|r| r.0 * S).sum();
    if cover > l.terminal_claim_exact_receipts_num { return false; }
    if rs.iter().any(|r| r.1 > gross(r.0, &l)) { return false; }
    let p: u128 = rs.iter().map(|r| r.1).sum();
    h.c_tot.get() + h.insurance.get() + l.snapshot_residual.saturating_sub(p) <= h.vault.get()
}

fn psum(accs: &[A], ghost: &Ghost) -> u128 { receipts(accs, ghost, true).unwrap().iter().map(|r| r.1).sum() }

fn empty_receipt() -> ResolvedPayoutReceiptV16Account { ResolvedPayoutReceiptV16Account::from_runtime(&ResolvedPayoutReceiptV16::EMPTY) }

/// A random state satisfying Inv (captured or not), with a random ghost list.
fn gen(rng: &mut Rng, m: u64) -> (H, Vec<A>, Ghost) {
    let n = rng.below(5) as usize;
    let captured = rng.chance(75);
    let mut accs = vec![];
    let mut ghost: Ghost = vec![];
    let mut faces: Vec<Option<u128>> = vec![];
    for _ in 0..n {
        let has = captured && rng.chance(50);
        faces.push(if has { Some(rng.below(m) as u128) } else { None });
    }
    let nghost = if captured { rng.below(3) } else { 0 };
    let gfaces: Vec<u128> = (0..nghost).map(|_| rng.below(m) as u128).collect();
    let cover: u128 = faces.iter().flatten().map(|f| f * S).sum::<u128>() + gfaces.iter().map(|f| f * S).sum::<u128>();
    let exact = cover + if rng.chance(30) { rng.below(m) as u128 * S + rng.below(3) as u128 } else { 0 };
    let unrec = if rng.chance(35) { 0 } else { rng.below(3 * m) as u128 * S + if rng.chance(20) { rng.below(1000) as u128 } else { 0 } };
    let resid = rng.below(3 * m) as u128;
    let den = exact + unrec;
    let (num, den) = if den == 0 { (1, 1) } else { ((resid * S).min(den), den) };
    let l = ResolvedPayoutLedgerV16 { snapshot_residual: resid, terminal_claim_exact_receipts_num: exact,
        terminal_claim_bound_unreceipted_num: unrec, current_payout_rate_num: num, current_payout_rate_den: den,
        snapshot_slot: rng.below(100), payout_halted: false, finalized: rng.chance(10) };
    let mut p = 0u128;
    for f in &gfaces {
        let g = gross(*f, &l);
        let paid = if rng.chance(80) { g } else { rng.below(g as u64 + 1) as u128 };
        ghost.push((*f, paid)); p += paid;
    }
    let mut csum = 0u128;
    let mut pos = 0u128;
    for f in &faces {
        let capital = if rng.chance(30) { 0 } else { rng.below(m) as u128 };
        csum += capital;
        let (receipt, pnl) = match f {
            Some(face) => {
                let g = gross(*face, &l);
                let paid = match rng.below(3) { 0 => 0, 1 => g, _ => rng.below(g as u64 + 1) as u128 };
                p += paid;
                let r = ResolvedPayoutReceiptV16 { present: true,
                    prior_bound_contribution_num: face * S + if rng.chance(20) { rng.below(5) as u128 } else { 0 },
                    live_released_face_at_receipt: 0, terminal_positive_claim_face: *face, paid_effective: paid,
                    finalized: paid == *face };
                // the engine leaves pnl == 0 after a close; a receipt with pnl > 0 is also tried
                (ResolvedPayoutReceiptV16Account::from_runtime(&r), if rng.chance(70) { 0 } else { rng.below(m) as i128 })
            }
            None => (empty_receipt(), if rng.chance(40) { 0 } else { rng.below(m) as i128 }),
        };
        pos += pnl.max(0) as u128;
        accs.push(A { capital: u(capital), pnl: V16PodI128::new(pnl), reserved_pnl: u(rng.below(m) as u128),
            fee_credits: V16PodI128::new(rng.below(m) as i128 - (m / 2) as i128),
            health_cert: HealthCertV16Account { valid: rng.below(2) as u8 }, resolved_payout_receipt: receipt });
    }
    let insurance = rng.below(m) as u128;
    let earnings = if rng.chance(30) { rng.below(m) as u128 } else { 0 };
    let fresh = if rng.chance(30) { rng.below(m) as u128 * S } else { 0 };
    let extra = if rng.chance(50) { 0 } else { rng.below(m) as u128 };
    let vault = if captured { csum + insurance + resid.saturating_sub(p) + extra } else { csum + insurance + extra + rng.below(3 * m) as u128 };
    let pos_tot = if rng.chance(85) { pos + if rng.chance(20) { rng.below(m) as u128 } else { 0 } } else { rng.below(m) as u128 };
    let bound_num = if rng.chance(85) { pos_tot * S + if rng.chance(20) { rng.below(m) as u128 * S } else { 0 } } else { rng.below(m) as u128 * S };
    let h = H { vault: u(vault), insurance: u(insurance), c_tot: u(csum), pnl_pos_tot: u(pos_tot),
        pnl_pos_bound_tot_num: u(bound_num), pnl_pos_bound_tot: u(bound_num.div_ceil(S)),
        pnl_matured_pos_tot: u(rng.below(m) as u128), backing_provider_earnings_total: u(earnings),
        source_fresh_backing_total_num: u(fresh),
        resolved_payout_blocker_count: u64p(if rng.chance(10) { 1 } else { 0 }),
        stale_certificate_count: u64p(if rng.chance(5) { 1 } else { 0 }),
        b_stale_account_count: u64p(if rng.chance(5) { 1 } else { 0 }),
        negative_pnl_account_count: u64p(if rng.chance(5) { 1 } else { 0 }),
        risk_epoch: u64p(rng.below(9)), current_slot: u64p(rng.below(100)), mode: 1, resolved_slot: u64p(rng.below(100)),
        payout_snapshot: u(if captured { resid } else { 0 }), payout_snapshot_pnl_pos_tot: u(0),
        payout_snapshot_captured: captured as u8,
        resolved_payout_ledger: if captured { ResolvedPayoutLedgerV16Account::from_runtime(&l) } else { ResolvedPayoutLedgerV16Account::from_runtime(&ResolvedPayoutLedgerV16::EMPTY) } };
    (h, accs, ghost)
}

/// The ghost list after a call on account i: if a receipt was there (or was created) and is gone,
/// the engine cleared it; record (face, paid at the moment of clearing).
fn ghost_after(before: &A, after: &A, l_after: &ResolvedPayoutLedgerV16, ghost: &Ghost) -> Ghost {
    let rb = before.resolved_payout_receipt.try_to_runtime().unwrap();
    let ra = after.resolved_payout_receipt.try_to_runtime().unwrap();
    let mut g = ghost.clone();
    if !ra.present {
        if rb.present {
            g.push((rb.terminal_positive_claim_face, gross(rb.terminal_positive_claim_face, l_after)));
        } else if before.pnl.get() > 0 {
            let face = before.pnl.get() as u128;
            g.push((face, gross(face, l_after)));
        }
    }
    g
}

#[derive(Default)]
struct Stats { states: u64, close_ok: u64, close_err: u64, topup_ok: u64, topup_err: u64, all_ok: u64, all_err: u64,
    cleared: u64, old_inv_broken: u64, uncaptured_ok: u64, rc1: u64, rc3_checked: u64, rc6_checked: u64, rc6_receipts_differ: u64, rc6_accounts_equal: u64 }

fn fail(what: &str, h: &H, accs: &[A], ghost: &Ghost, i: usize) -> ! {
    eprintln!("COUNTEREXAMPLE {what}\n i={i}\n header={h:?}\n accounts={accs:?}\n ghost={ghost:?}");
    std::process::exit(1)
}

fn check_close(h0: &H, a0: &[A], g0: &Ghost, i: usize, st: &mut Stats) -> Option<(H, Vec<A>, Ghost, u128)> {
    let mut h = *h0; let mut a = a0.to_vec();
    let r = close_one(&mut h, &mut a[i]);
    let Ok(payout) = r else { st.close_err += 1; return None };
    st.close_ok += 1;
    if h0.payout_snapshot_captured == 0 { st.uncaptured_ok += 1; }
    let l1 = h.resolved_payout_ledger.try_to_runtime().unwrap();
    let g1 = ghost_after(&a0[i], &a[i], &l1, g0);
    if g1.len() != g0.len() { st.cleared += 1; }
    let cap = a0[i].capital.get();
    let p0 = psum(a0, g0); let p1 = psum(&a, &g1);
    if !inv(&h, &a, &g1, true) { fail("RCs2 Inv after close", h0, a0, g0, i) }
    if h.vault.get() + payout != h0.vault.get() { fail("RCs2 vault", h0, a0, g0, i) }
    if payout != cap + (p1 - p0) { fail("RCs2 payout", h0, a0, g0, i) }
    if h.c_tot.get() + cap != h0.c_tot.get() { fail("RCs2 c_tot", h0, a0, g0, i) }
    if a[i].capital.get() != 0 || a[i].pnl.get() != 0 { fail("RCs2 zeroed", h0, a0, g0, i) }
    let ra = a[i].resolved_payout_receipt.try_to_runtime().unwrap();
    if ra.present && ra.paid_effective != gross(ra.terminal_positive_claim_face, &l1) { fail("RCs2 paid==gross", h0, a0, g0, i) }
    for j in 0..a.len() { if j != i && a[j] != a0[j] { fail("RCs2 frame", h0, a0, g0, i) } }
    if h.insurance != h0.insurance { fail("RCs2 insurance", h0, a0, g0, i) }
    if h0.payout_snapshot_captured == 1 {
        let l0 = h0.resolved_payout_ledger.try_to_runtime().unwrap();
        if (l1.snapshot_residual, l1.current_payout_rate_num, l1.current_payout_rate_den) != (l0.snapshot_residual, l0.current_payout_rate_num, l0.current_payout_rate_den) { fail("RCs2 rate frame", h0, a0, g0, i) }
    }
    if h.payout_snapshot_captured == 1 && inv(h0, a0, g0, false) && g0.is_empty() && !inv(&h, &a, &vec![], false) {
        if st.old_inv_broken == 0 {
            let l0 = h0.resolved_payout_ledger.try_to_runtime().unwrap();
            let r0 = a0[i].resolved_payout_receipt.try_to_runtime().unwrap();
            println!("first state where run 1's Inv (sums over present receipts only) holds before and fails after one Ok close (account {i} of {}):", a0.len());
            println!(" before: vault={} c_tot={} insurance={} snapshot_residual={} exact_num={} unreceipted_num={} rate={}/{} fresh_backing_num={}",
                h0.vault.get(), h0.c_tot.get(), h0.insurance.get(), l0.snapshot_residual, l0.terminal_claim_exact_receipts_num,
                l0.terminal_claim_bound_unreceipted_num, l0.current_payout_rate_num, l0.current_payout_rate_den, h0.source_fresh_backing_total_num.get());
            println!("   account: capital={} pnl={} receipt present={} face={} paid={}; P(present receipts)={}", a0[i].capital.get(), a0[i].pnl.get(), r0.present,
                r0.terminal_positive_claim_face, r0.paid_effective, psum(a0, &vec![]));
            println!(" after:  payout={payout} vault={} c_tot={} insurance={} receipt present={}; P(present receipts)={}; backed needs c_tot+insurance+max(snapshot_residual-P,0)={} <= vault={}",
                h.vault.get(), h.c_tot.get(), h.insurance.get(), a[i].resolved_payout_receipt.present, psum(&a, &vec![]),
                h.c_tot.get() + h.insurance.get() + l1.snapshot_residual.saturating_sub(psum(&a, &vec![])), h.vault.get());
        }
        st.old_inv_broken += 1;
    }
    Some((h, a, g1, payout))
}

fn main() {
    let iters: u64 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(200_000);
    let mut rng = Rng(0x9E3779B97F4A7C15);
    let mut st = Stats::default();
    for it in 0..iters {
        let m = [4u64, 12, 60, 1000][(it % 4) as usize];
        let (h0, a0, g0) = gen(&mut rng, m);
        if !inv(&h0, &a0, &g0, true) { fail("generator produced a non-Inv state", &h0, &a0, &g0, 0) }
        st.states += 1;
        // RCs1 on captured states
        if h0.payout_snapshot_captured == 1 {
            let l = h0.resolved_payout_ledger.try_to_runtime().unwrap();
            let rs = receipts(&a0, &g0, true).unwrap();
            let g: u128 = rs.iter().map(|r| gross(r.0, &l)).sum();
            let p: u128 = rs.iter().map(|r| r.1).sum();
            if !(p <= g && g <= l.snapshot_residual) { fail("RCs1", &h0, &a0, &g0, 0) }
            st.rc1 += 1;
        }
        if a0.is_empty() { continue; }
        let i = rng.below(a0.len() as u64) as usize;
        // RCs2
        let closed = check_close(&h0, &a0, &g0, i, &mut st);
        // RC3s: under the stated extra conditions the close does not return Err
        {
            const MAX: u128 = u128::MAX;
            let pnl = a0[i].pnl.get();
            let r = a0[i].resolved_payout_receipt.try_to_runtime().unwrap();
            let l = h0.resolved_payout_ledger.try_to_runtime().unwrap();
            let pos = pnl.max(0) as u128;
            let resid_now = h0.vault.get().saturating_sub(h0.c_tot.get().saturating_add(h0.insurance.get())
                .saturating_add(h0.backing_provider_earnings_total.get()).saturating_add(h0.source_fresh_backing_total_num.get() / S));
            let bound = h0.pnl_pos_bound_tot_num.get();
            let receipt_cond = r.present
                || pnl == 0
                || (h0.payout_snapshot_captured == 1 && pos.checked_mul(S).map_or(false, |x| x <= l.terminal_claim_bound_unreceipted_num))
                || (h0.payout_snapshot_captured == 0 && pos.checked_mul(S).map_or(false, |x| x <= bound) && resid_now.checked_mul(S).is_some());
            let agg_cond = pos <= h0.pnl_pos_tot.get() && h0.pnl_pos_tot.get() <= MAX / S;
            if pnl >= 0 && receipt_cond && agg_cond {
                st.rc3_checked += 1;
                if closed.is_none() { fail("RC3s: close returned Err under the stated conditions", &h0, &a0, &g0, i) }
            }
        }
        // RCs7
        {
            let mut h = h0; let mut a = a0.clone();
            match topup_one(&mut h, &mut a[i]) {
                Err(_) => st.topup_err += 1,
                Ok(payout) => {
                    st.topup_ok += 1;
                    // topup never creates a receipt, so only a receipt present before can be cleared
                    let mut g1 = g0.clone();
                    let rb = a0[i].resolved_payout_receipt.try_to_runtime().unwrap();
                    let ra = a[i].resolved_payout_receipt.try_to_runtime().unwrap();
                    if rb.present && !ra.present { g1.push((rb.terminal_positive_claim_face, rb.paid_effective)); }
                    if !inv(&h, &a, &g1, true) { fail("RCs7 Inv", &h0, &a0, &g0, i) }
                    if h.vault.get() + payout != h0.vault.get() { fail("RCs7 vault", &h0, &a0, &g0, i) }
                    if psum(&a, &g1) != psum(&a0, &g0) + payout { fail("RCs7 P", &h0, &a0, &g0, i) }
                    if a[i].capital != a0[i].capital || a[i].pnl != a0[i].pnl || h.c_tot != h0.c_tot || h.insurance != h0.insurance { fail("RCs7 frame", &h0, &a0, &g0, i) }
                    for j in 0..a.len() { if j != i && a[j] != a0[j] { fail("RCs7 other", &h0, &a0, &g0, i) } }
                }
            }
        }
        // RCs4 / RCs5: a random order, checked against stepwise closes
        let len = rng.below(2 * a0.len() as u64 + 2) as usize;
        let full = rng.chance(50);
        let mut order: Vec<usize> = (0..len).map(|_| rng.below(a0.len() as u64) as usize).collect();
        if full { for j in 0..a0.len() { let at = rng.below(order.len() as u64 + 1) as usize; order.insert(at, j); } }
        let mut h = h0; let mut a = a0.clone();
        match close_all(&mut h, &mut a, &order) {
            Err(_) => st.all_err += 1,
            Ok(total) => {
                st.all_ok += 1;
                let (mut hs, mut as_, mut gs) = (h0, a0.clone(), g0.clone());
                let mut sum = 0u128;
                for &k in &order {
                    let Some((h2, a2, g2, pay)) = check_close(&hs, &as_, &gs, k, &mut st) else { fail("RCs4 stepwise close failed where close_all returned Ok", &h0, &a0, &g0, k) };
                    hs = h2; as_ = a2; gs = g2; sum += pay;
                }
                if hs != h || as_ != a || sum != total { fail("RCs4 close_all differs from stepwise closes", &h0, &a0, &g0, 0) }
                let c0: u128 = a0.iter().map(|x| x.capital.get()).sum();
                let c1: u128 = a.iter().map(|x| x.capital.get()).sum();
                let (p0, p1) = (psum(&a0, &g0), psum(&a, &gs));
                if !inv(&h, &a, &gs, true) { fail("RCs4 Inv", &h0, &a0, &g0, 0) }
                if h.vault.get() + total != h0.vault.get() || total != (c0 - c1) + (p1 - p0) { fail("RCs4 accounting", &h0, &a0, &g0, 0) }
                if h.c_tot.get() + h.insurance.get() > h.vault.get() || h.insurance != h0.insurance { fail("RCs4 senior", &h0, &a0, &g0, 0) }
                if h.payout_snapshot_captured == 1 {
                    let l1 = h.resolved_payout_ledger.try_to_runtime().unwrap();
                    if p1 > l1.snapshot_residual { fail("RCs4 P' <= snapshot_residual", &h0, &a0, &g0, 0) }
                    if full && total > c0 + l1.snapshot_residual - p0 { fail("RCs5 bound", &h0, &a0, &g0, 0) }
                }
                for &k in &order { if a[k].capital.get() != 0 || a[k].pnl.get() != 0 { fail("RCs4 zeroed", &h0, &a0, &g0, k) } }
                // RC6s: a permutation of the order
                {
                    let mut o2 = order.clone();
                    for k in (1..o2.len()).rev() { let j = rng.below(k as u64 + 1) as usize; o2.swap(k, j); }
                    let mut hb = h0; let mut ab = a0.clone();
                    match close_all(&mut hb, &mut ab, &o2) {
                        Err(_) => fail("RC6s: permuted order returned Err", &h0, &a0, &g0, 0),
                        Ok(t2) => {
                            st.rc6_checked += 1;
                            if t2 != total || hb != h { fail("RC6s: total or market header differs", &h0, &a0, &g0, 0) }
                            let lf = h.resolved_payout_ledger.try_to_runtime().unwrap();
                            let mut differ = false;
                            for j in 0..a.len() {
                                let (x, y) = (a[j], ab[j]);
                                if (x.capital, x.pnl, x.reserved_pnl, x.fee_credits, x.health_cert) != (y.capital, y.pnl, y.reserved_pnl, y.fee_credits, y.health_cert) { fail("RC6s: account scalar fields differ", &h0, &a0, &g0, j) }
                                if x.resolved_payout_receipt != y.resolved_payout_receipt {
                                    differ = true;
                                    let rx = x.resolved_payout_receipt.try_to_runtime().unwrap();
                                    let ry = y.resolved_payout_receipt.try_to_runtime().unwrap();
                                    let (pr, em) = if rx.present { (rx, ry) } else { (ry, rx) };
                                    if !(pr.present && em == ResolvedPayoutReceiptV16::EMPTY && pr.paid_effective == gross(pr.terminal_positive_claim_face, &lf)) { fail("RC6s: receipts differ other than by clearing", &h0, &a0, &g0, j) }
                                }
                            }
                            if differ { st.rc6_receipts_differ += 1 } else { st.rc6_accounts_equal += 1 }
                        }
                    }
                }
                if full && (h.c_tot.get() != 0 || total != c0 + (p1 - p0) || h.insurance.get() > h.vault.get()) { fail("RCs5", &h0, &a0, &g0, 0) }
            }
        }
    }
    println!("states={} rc1_checked={} close ok/err={}/{} (uncaptured ok={}, receipts cleared={}) topup ok/err={}/{} close_all ok/err={}/{}",
        st.states, st.rc1, st.close_ok, st.close_err, st.uncaptured_ok, st.cleared, st.topup_ok, st.topup_err, st.all_ok, st.all_err);
    println!("run-1 Inv (no ghost list) broken after an Ok close that kept the new Inv: {} times", st.old_inv_broken);
    println!("RC3s checked on {} closes; RC6s checked on {} order pairs (accounts identical: {}, receipts differ only by clearing: {})", st.rc3_checked, st.rc6_checked, st.rc6_accounts_equal, st.rc6_receipts_differ);
    println!("no counterexample to RCs1, RCs2, RC3s, RCs4, RCs5, RC6s, RCs7");
}
