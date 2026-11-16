//! Differential tests for `src/band_rent.rs`. Tests only.
//!
//! Each pub fn is compared with an independent reference written from the doc
//! formulas (band_rent.rs:14-27) using u128 / i128 arithmetic and a separate
//! rational formulation (ceil/floor via exact division with remainder checks,
//! not the same expression shape as the product code). The rent reference uses
//! a tiny 3-limb schoolbook multiply so it does not share `wide_math`.
//!
//!   cargo test -j 4 --test band_rent_diff -- --nocapture
//! Each test prints `BAND_RENT_DIFF fn=.. cases=.. mismatches=..`.

use percolator::band_rent::*;
use percolator::{MAX_ORACLE_PRICE, POS_SCALE};

// ---- deterministic rng -------------------------------------------------
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        // splitmix64
        self.0 = self.0.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }
    fn range(&mut self, lo: u64, hi: u64) -> u64 {
        lo + self.next() % (hi - lo + 1)
    }
    fn u128(&mut self) -> u128 {
        ((self.next() as u128) << 64) | self.next() as u128
    }
}

fn report(f: &str, cases: u64, mism: u64) {
    println!("BAND_RENT_DIFF fn={f} cases={cases} mismatches={mism}");
    assert_eq!(mism, 0, "{f}: mismatches");
}

// ---- references --------------------------------------------------------
/// lo = ceil(A(10000-d)/10000), hi = min(floor(A(10000+d)/10000), MAX)
fn ref_bounds(a: u64, d: u64) -> Option<(u64, u64)> {
    if a == 0 || a > MAX_ORACLE_PRICE || d == 0 || d > MAX_BAND_BPS {
        return None;
    }
    // Independent shape: A - floor(A*d/1e4) is NOT ceil in general; use
    // A*(1e4-d) = 1e4*k - r. Compute lo via floor+remainder test.
    let num = a as u128 * (10_000 - d as u128);
    let mut lo = num / 10_000;
    if lo * 10_000 < num {
        lo += 1;
    }
    let mut hi = a as u128 * (10_000 + d as u128) / 10_000;
    if hi > MAX_ORACLE_PRICE as u128 {
        hi = MAX_ORACLE_PRICE as u128;
    }
    Some((lo as u64, hi as u64))
}
fn ref_width_ok(a: u64, d: u64) -> Option<bool> {
    ref_bounds(a, d).map(|(lo, hi)| hi - lo >= 32)
}
/// smallest a (>=1) such that a..=a+8 all width-ok, by exhaustive scan from 1.
fn ref_min_wide(d: u64) -> Option<u64> {
    if d == 0 || d > MAX_BAND_BPS {
        return None; // error
    }
    let mut a = 1u64;
    let mut run = 0u64;
    // widths are monotone-ish in a; threshold ~ 16e4/d <= 160_000/1 -> scan bound
    while a <= 400_000 {
        if ref_width_ok(a, d).unwrap() {
            run += 1;
            if run == 9 {
                return Some(a - 8);
            }
        } else {
            run = 0;
        }
        a += 1;
    }
    None
}
fn ref_worst_adverse(d: u64) -> u64 {
    let d = d as u128;
    let num = (10_000 + d) * (10_000 + d);
    let den = 10_000 - d;
    let mut q = num / den;
    if q * den < num {
        q += 1;
    }
    (q - 10_000) as u64
}

// 256-bit-ish: product of two u128 as (hi, lo) u128 pair, then divide by DEN via
// long division on 64-bit digits. Returns None if quotient exceeds u128.
fn mul_u128(a: u128, b: u128) -> [u64; 4] {
    let al = [a as u64, (a >> 64) as u64];
    let bl = [b as u64, (b >> 64) as u64];
    let mut r = [0u64; 4];
    for i in 0..2 {
        let mut carry = 0u128;
        for j in 0..2 {
            let cur = r[i + j] as u128 + al[i] as u128 * bl[j] as u128 + carry;
            r[i + j] = cur as u64;
            carry = cur >> 64;
        }
        let mut k = i + 2;
        while carry > 0 && k < 4 {
            let cur = r[k] as u128 + carry;
            r[k] = cur as u64;
            carry = cur >> 64;
            k += 1;
        }
    }
    r
}
/// (limbs + small) / den -> (quotient limbs(4), remainder)
fn divmod_limbs(mut n: [u64; 4], den: u128) -> ([u64; 4], u128) {
    // den < 2^96 (POS_SCALE*1e9 = 1e6*1e9 = 1e15 actually), so rem*2^64 fits u128 when den < 2^64.
    assert!(den < (1u128 << 63));
    let mut rem = 0u128;
    for i in (0..4).rev() {
        let cur = (rem << 64) | n[i] as u128;
        n[i] = (cur / den) as u64;
        rem = cur % den;
    }
    (n, rem)
}
fn ref_rent(abs_q: u128, index: u128, snap: u128, carry: u64) -> Result<(u128, u64), BandRentError> {
    let den = POS_SCALE as u128 * 1_000_000_000;
    if snap > index || carry as u128 >= den {
        return Err(BandRentError::InvalidInput);
    }
    let mut n = mul_u128(abs_q, index - snap);
    // add carry
    let mut c = carry as u128;
    for limb in n.iter_mut() {
        let cur = *limb as u128 + (c & 0xFFFF_FFFF_FFFF_FFFF);
        *limb = cur as u64;
        c = (c >> 64) + (cur >> 64);
    }
    if c != 0 {
        return Err(BandRentError::Overflow);
    }
    let (q, r) = divmod_limbs(n, den);
    // product code: carry_out always fits; quotient > u128 saturates.
    let due = if q[2] != 0 || q[3] != 0 {
        u128::MAX
    } else {
        q[0] as u128 | (q[1] as u128) << 64
    };
    Ok((due, r as u64))
}

// ---- tests -------------------------------------------------------------
fn price_edges() -> Vec<u64> {
    let mut v: Vec<u64> = (1..=300).collect();
    for k in [
        1_000u64, 9_999, 10_000, 10_001, 160_000, 1_000_000, 1_000_000_000,
        MAX_ORACLE_PRICE - 1, MAX_ORACLE_PRICE,
    ] {
        v.push(k);
    }
    // hi-cap region for each d: anchor around MAX*1e4/(1e4+d)
    for d in [1u64, 100, 130, 1_000, MAX_BAND_BPS] {
        let c = (MAX_ORACLE_PRICE as u128 * 10_000 / (10_000 + d as u128)) as u64;
        for x in c.saturating_sub(3)..=c.saturating_add(3).min(MAX_ORACLE_PRICE) {
            v.push(x);
        }
    }
    v
}

#[test]
fn band_bounds_diff() {
    let (mut cases, mut mism) = (0u64, 0u64);
    let mut check = |a: u64, d: u64| {
        cases += 1;
        let got = band_bounds(a, d).ok();
        let want = ref_bounds(a, d);
        if got != want {
            mism += 1;
            eprintln!("band_bounds a={a} d={d} got={got:?} want={want:?}");
        }
        if let Some((lo, hi)) = got {
            assert!(1 <= lo && lo <= a && a <= hi && hi <= MAX_ORACLE_PRICE);
        }
    };
    let ds: Vec<u64> = vec![0, 1, 2, 10, 100, 129, 130, 131, 1_000, 1_999, 2_000, 2_001];
    for &a in &price_edges() {
        for &d in &ds {
            check(a, d);
        }
    }
    check(0, 100);
    check(MAX_ORACLE_PRICE + 1, 100);
    check(u64::MAX, 100);
    // exhaustive d x small a
    for d in 1..=MAX_BAND_BPS {
        for a in 1..=40u64 {
            check(a, d);
        }
    }
    let mut r = Rng(1);
    for _ in 0..200_000 {
        let a = r.range(1, MAX_ORACLE_PRICE);
        let d = r.range(1, MAX_BAND_BPS);
        check(a, d);
    }
    report("band_bounds", cases, mism);
}

#[test]
fn band_bounds_hi_cap_hit() {
    // A where hi would exceed the cap must be clamped, and the first such A is exact.
    let mut hit = 0u64;
    for d in [1u64, 130, 2_000] {
        let a = MAX_ORACLE_PRICE;
        let (_, hi) = band_bounds(a, d).unwrap();
        assert_eq!(hi, MAX_ORACLE_PRICE);
        hit += 1;
        // first anchor whose uncapped hi > MAX
        let first = (MAX_ORACLE_PRICE as u128 * 10_000 / (10_000 + d as u128)) as u64;
        for x in first.saturating_sub(2)..=first + 2 {
            let (_, h) = band_bounds(x, d).unwrap();
            let uncapped = x as u128 * (10_000 + d as u128) / 10_000;
            assert_eq!(h as u128, uncapped.min(MAX_ORACLE_PRICE as u128));
            hit += 1;
        }
    }
    report("band_bounds_cap", hit, 0);
}

#[test]
fn band_width_ok_diff() {
    let (mut cases, mut mism) = (0u64, 0u64);
    let mut r = Rng(2);
    let mut check = |a: u64, d: u64| {
        cases += 1;
        let got = band_width_ok(a, d).ok();
        if got != ref_width_ok(a, d) {
            mism += 1;
            eprintln!("band_width_ok a={a} d={d} got={got:?}");
        }
    };
    for d in [1u64, 50, 130, 500, 2_000] {
        for a in 1..=200_000u64 {
            check(a, d);
        }
    }
    for &a in &price_edges() {
        for d in [0u64, 1, 130, 2_000, 2_001] {
            check(a, d);
        }
    }
    for _ in 0..100_000 {
        check(r.range(1, MAX_ORACLE_PRICE), r.range(1, MAX_BAND_BPS));
    }
    report("band_width_ok", cases, mism);
}

#[test]
fn degenerate_d130_price_le_76() {
    // Comment at :88: at d = 130 every price <= 76 has lo == hi.
    let mut n = 0;
    for a in 1..=76u64 {
        let (lo, hi) = band_bounds(a, 130).unwrap();
        assert_eq!(lo, hi, "a={a} lo={lo} hi={hi}");
        assert!(!band_width_ok(a, 130).unwrap());
        n += 1;
    }
    // and 77 is NOT degenerate (claim is tight on the "<= 76" side only if lo<hi)
    let (lo, hi) = band_bounds(77, 130).unwrap();
    assert!(lo < hi, "a=77 lo={lo} hi={hi}");
    report("d130_degenerate", n, 0);
}

#[test]
fn min_wide_anchor_and_genesis_diff() {
    let (mut cases, mut mism) = (0u64, 0u64);
    // band_min_wide_anchor vs exhaustive reference: every d in 1..=MAX plus invalid.
    for d in 0..=MAX_BAND_BPS + 1 {
        cases += 1;
        let got = band_min_wide_anchor(d).ok();
        let want = if d == 0 || d > MAX_BAND_BPS {
            None
        } else {
            Some(ref_min_wide(d))
        };
        let got_cmp = if d == 0 || d > MAX_BAND_BPS {
            assert!(band_min_wide_anchor(d).is_err());
            None
        } else {
            Some(got.unwrap())
        };
        if got_cmp != want {
            mism += 1;
            if mism < 20 {
                eprintln!("min_wide d={d} got={got_cmp:?} want={want:?}");
            }
        }
    }
    report("band_min_wide_anchor", cases, mism);

    let (mut gcases, mut gm) = (0u64, 0u64);
    let mut r = Rng(3);
    for d in [1u64, 2, 7, 100, 129, 130, 131, 1_000, 2_000] {
        let m = ref_min_wide(d).unwrap();
        let thr = m as u128 * 100;
        let mut ps: Vec<u64> = vec![0, 1, MAX_ORACLE_PRICE];
        for off in [-2i128, -1, 0, 1, 2] {
            let p = thr as i128 + off;
            if p >= 0 && p as u128 <= u64::MAX as u128 {
                ps.push(p as u64);
            }
        }
        for _ in 0..500 {
            ps.push(r.range(0, thr as u64 * 2));
        }
        for p in ps {
            gcases += 1;
            let got = band_genesis_price_ok(p, d).unwrap();
            let want = p as u128 >= thr;
            if got != want {
                gm += 1;
                eprintln!("genesis d={d} p={p} got={got} want={want}");
            }
        }
    }
    assert!(band_genesis_price_ok(1, 0).is_err());
    assert!(band_genesis_price_ok(1, MAX_BAND_BPS + 1).is_err());
    report("band_genesis_price_ok", gcases, gm);
}

#[test]
fn price_in_band_clamp_diff() {
    let (mut cases, mut mism) = (0u64, 0u64);
    let mut r = Rng(4);
    let mut check = |p: u64, a: u64, d: u64| {
        cases += 1;
        let want = ref_bounds(a, d);
        let pin = price_in_band(p, a, d).ok();
        let cl = clamp_to_band(p, a, d).ok();
        let (wpin, wcl) = match want {
            None => (None, None),
            Some((lo, hi)) => (Some(lo <= p && p <= hi), Some(if p < lo { lo } else if p > hi { hi } else { p })),
        };
        if pin != wpin || cl != wcl {
            mism += 1;
            eprintln!("p={p} a={a} d={d} pin={pin:?}/{wpin:?} clamp={cl:?}/{wcl:?}");
        }
    };
    for &a in &price_edges() {
        for d in [1u64, 130, 2_000] {
            if let Some((lo, hi)) = ref_bounds(a, d) {
                for p in [0, lo.saturating_sub(1), lo, a, hi, hi.saturating_add(1), u64::MAX] {
                    check(p, a, d);
                }
            }
        }
    }
    for _ in 0..100_000 {
        let a = r.range(1, MAX_ORACLE_PRICE);
        let d = r.range(1, MAX_BAND_BPS);
        check(r.range(0, MAX_ORACLE_PRICE * 2), a, d);
    }
    report("price_in_band+clamp_to_band", cases, mism);
}

#[test]
fn worst_adverse_and_d1_and_pin_diff() {
    let (mut cases, mut mism) = (0u64, 0u64);
    for d in 1..=MAX_BAND_BPS {
        cases += 1;
        if band_worst_adverse_bps(d) != Ok(ref_worst_adverse(d)) {
            mism += 1;
        }
    }
    assert!(band_worst_adverse_bps(0).is_err());
    assert!(band_worst_adverse_bps(MAX_BAND_BPS + 1).is_err());
    report("band_worst_adverse_bps", cases, mism);

    let (mut c2, mut m2) = (0u64, 0u64);
    let mut r = Rng(5);
    for _ in 0..50_000 {
        let seg = r.next() >> r.range(0, 60);
        let anc = r.next() >> r.range(0, 60);
        let e = r.next() >> r.range(0, 60);
        c2 += 1;
        let want = (seg as i128 - anc as i128) > e as i128;
        if band_duration_pinned(seg, anc, e) != want {
            m2 += 1;
        }
    }
    report("band_duration_pinned", c2, m2);

    let (mut c3, mut m3) = (0u64, 0u64);
    for _ in 0..100_000 {
        let a = r.range(1, MAX_ORACLE_PRICE);
        let d = r.range(1, MAX_BAND_BPS);
        let cur = r.range(1, MAX_ORACLE_PRICE);
        let step = r.range(1, MAX_ORACLE_PRICE);
        let dp = r.next() & 1 == 0;
        c3 += 1;
        let (lo, hi) = ref_bounds(a, d).unwrap();
        let want = if dp {
            (cur, step != cur)
        } else {
            let c = step.clamp(lo, hi);
            (c, c != step)
        };
        if band_d1_pinned_step(cur, step, a, d, dp) != Ok(want) {
            m3 += 1;
        }
    }
    report("band_d1_pinned_step", c3, m3);
}

#[test]
fn rent_diff() {
    let den = POS_SCALE as u128 * 1_000_000_000;
    assert_eq!(RENT_INDEX_DEN, den);
    let mut r = Rng(6);

    // rent_index_delta: price*rate*dt, checked
    let (mut c1, mut m1) = (0u64, 0u64);
    let mut chk = |p: u64, rt: u64, dt: u64| {
        c1 += 1;
        let want = (p as u128)
            .checked_mul(rt as u128)
            .and_then(|v| v.checked_mul(dt as u128))
            .ok_or(BandRentError::Overflow);
        // independent overflow oracle via wide product
        let w = mul_u128(p as u128 * rt as u128, dt as u128);
        let fits = w[2] == 0 && w[3] == 0;
        if want.is_ok() != fits || rent_index_delta(p, rt, dt) != want {
            m1 += 1;
            eprintln!("delta {p} {rt} {dt}");
        }
    };
    let edges = [0u64, 1, 2, MAX_RENT_E9_PER_SLOT, MAX_ORACLE_PRICE, u64::MAX, u64::MAX - 1, 1 << 32];
    for &p in &edges {
        for &rt in &edges {
            for &dt in &edges {
                chk(p, rt, dt);
            }
        }
    }
    for _ in 0..100_000 {
        let sh = |r: &mut Rng| r.next() >> r.range(0, 63);
        let (a, b, c) = (sh(&mut r), sh(&mut r), sh(&mut r));
        chk(a, b, c);
    }
    report("rent_index_delta", c1, m1);

    // rent_due_with_carry / rent_due_atoms vs 4-limb reference
    let (mut c2, mut m2) = (0u64, 0u64);
    let mut chk2 = |q: u128, idx: u128, snap: u128, carry: u64| {
        c2 += 1;
        let got = rent_due_with_carry(q, idx, snap, carry);
        let want = ref_rent(q, idx, snap, carry);
        let ok = match (&got, &want) {
            (Ok(a), Ok(b)) => a == b,
            (Err(a), Err(b)) => a == b,
            _ => false,
        };
        let atoms_ok = if carry == 0 {
            rent_due_atoms(q, idx, snap).ok() == want.ok().map(|x| x.0)
        } else {
            true
        };
        if !ok || !atoms_ok {
            m2 += 1;
            if m2 < 20 {
                eprintln!("rent q={q} idx={idx} snap={snap} carry={carry} got={got:?} want={want:?}");
            }
        }
    };
    let dm = den as u64;
    let carries = [0u64, 1, dm - 1, dm / 2, dm, dm + 1, u64::MAX];
    let u = [0u128, 1, 2, den - 1, den, den + 1, u64::MAX as u128, u128::MAX / 2, u128::MAX - 1, u128::MAX, 1 << 64, (1 << 64) - 1];
    for &q in &u {
        for &idx in &u {
            for &snap in &u {
                for &c in &carries {
                    chk2(q, idx, snap, c);
                }
            }
        }
    }
    for _ in 0..300_000 {
        let sh = |r: &mut Rng| r.u128() >> r.range(0, 127);
        let (q, snap) = (sh(&mut r), sh(&mut r));
        let idx = snap.saturating_add(sh(&mut r));
        let carry = if r.next() & 1 == 0 { 0 } else { r.range(0, dm - 1) };
        chk2(q, idx, snap, carry);
    }
    report("rent_due_with_carry", c2, m2);

    // split-settle exactness: sum of due over pieces == single settle (carry-chained)
    let (mut c3, mut m3) = (0u64, 0u64);
    for _ in 0..50_000 {
        let q = r.u128() >> r.range(60, 127);
        let total = r.next() as u128 >> r.range(0, 40);
        let cut = if total == 0 { 0 } else { r.u128() % (total + 1) };
        let (d1, c1c) = rent_due_with_carry(q, cut, 0, 0).unwrap();
        let (d2, c2c) = rent_due_with_carry(q, total, cut, c1c).unwrap();
        let (d0, c0c) = rent_due_with_carry(q, total, 0, 0).unwrap();
        c3 += 1;
        if d1.saturating_add(d2) != d0 || c2c != c0c {
            m3 += 1;
        }
    }
    report("rent_split_settle", c3, m3);
}

#[test]
fn rent_chargeable_diff() {
    let (mut cases, mut mism) = (0u64, 0u64);
    let mut r = Rng(7);
    let e: [i128; 7] = [0, 1, -1, i128::MIN, i128::MAX, -(1 << 100), 1 << 100];
    let ue: [u128; 6] = [0, 1, u128::MAX, 1 << 100, u64::MAX as u128, 12345];
    let mut chk = |due: u128, cap: u128, pnl: i128| {
        cases += 1;
        let lost = if pnl < 0 { pnl.unsigned_abs() } else { 0 };
        let free = if cap > lost { cap - lost } else { 0 };
        let want = if due < free { due } else { free };
        if rent_chargeable_atoms(due, cap, pnl) != want {
            mism += 1;
        }
    };
    for &d in &ue {
        for &c in &ue {
            for &p in &e {
                chk(d, c, p);
            }
        }
    }
    for _ in 0..100_000 {
        let sh = |r: &mut Rng| r.u128() >> r.range(0, 127);
        let p = (sh(&mut r) as i128) * if r.next() & 1 == 0 { 1 } else { -1 };
        chk(sh(&mut r), sh(&mut r), p);
    }
    report("rent_chargeable_atoms", cases, mism);
}

#[test]
fn pinned_boundaries_and_known_values() {
    // independent hard-coded anchors
    assert_eq!(band_worst_adverse_bps(2_000), Ok(8_000));
    assert_eq!(band_bounds(77, 130), Ok((76, 78)));
    assert_eq!(band_bounds(1, 1), Ok((1, 1)));
    assert_eq!(band_bounds(10_000, 100), Ok((9_900, 10_100)));
    assert_eq!(band_bounds(MAX_ORACLE_PRICE, 2_000), Ok((MAX_ORACLE_PRICE / 5 * 4, MAX_ORACLE_PRICE)));
    // duration pin boundary: pinned iff seg - anc > e
    let (anc, e) = (1_000u64, 150u64);
    assert!(!band_duration_pinned(anc + e - 1, anc, e));
    assert!(!band_duration_pinned(anc + e, anc, e));
    assert!(band_duration_pinned(anc + e + 1, anc, e));
    assert!(!band_duration_pinned(anc - 1, anc, e));
    assert!(!band_duration_pinned(0, anc, 0));
    assert!(band_duration_pinned(1, 0, 0));
    // d1: duration pinned with step == current is not a pin
    assert_eq!(band_d1_pinned_step(500, 500, 1_000, 100, true), Ok((500, false)));
    assert_eq!(band_d1_pinned_step(500, 501, 1_000, 100, true), Ok((500, true)));
    // d1 error path
    assert!(band_d1_pinned_step(5, 5, 0, 100, false).is_err());
    assert!(band_d1_pinned_step(5, 5, 100, 0, false).is_err());
    assert!(band_d1_pinned_step(5, 5, MAX_ORACLE_PRICE + 1, 100, false).is_err());
    report("pinned_boundaries", 17, 0);
}
