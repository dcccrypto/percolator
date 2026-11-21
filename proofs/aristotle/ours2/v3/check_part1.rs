// ours2/v3/check_part1.rs: a random TEST (not a proof) of the Part 1 statements of
// aristotle/resolved_close_kernels_spec.md. Not uploaded. It is appended to a scratch copy of
// src/v16.rs at c38b6b1e by v3/run_check.sh and run with `cargo test --release`.
// Exact floor(a*b/d) is computed here with a schoolbook 256-bit product independent of wide_math.
#[cfg(test)]
mod ours2_v3_part1_check {
    use super::*;

    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
        fn u128(&mut self) -> u128 {
            const EDGE: [u128; 9] = [0, 1, 2, 3, 999_999_999_999, 1_000_000_000_000, u128::MAX, u128::MAX - 1, 1u128 << 127];
            match self.next() % 6 {
                0 => EDGE[(self.next() % 9) as usize],
                1 => (self.next() % 1000) as u128,
                2 => ((self.next() as u128) << 64) | self.next() as u128,
                3 => (self.next() as u128) >> (self.next() % 64),
                4 => u128::MAX / 1_000_000_000_000 + (self.next() % 3) as u128 - 1,
                _ => self.next() as u128,
            }
        }
        fn b(&mut self) -> bool {
            self.next() & 1 == 1
        }
    }

    // exact floor(a*b/d) as Option<u128> (None if >= 2^128); d != 0
    fn floor_mul_div(a: u128, b: u128, d: u128) -> Option<u128> {
        let (a0, a1) = (a as u64 as u128, a >> 64);
        let (b0, b1) = (b as u64 as u128, b >> 64);
        // 256-bit product as four 64-bit limbs
        let mut l = [0u128; 4];
        let add = |l: &mut [u128; 4], i: usize, v: u128| {
            let mut i = i;
            let mut carry = v;
            while carry != 0 && i < 4 {
                let s = l[i] + (carry as u64 as u128);
                l[i] = s as u64 as u128;
                carry = (carry >> 64) + (s >> 64);
                i += 1;
            }
        };
        add(&mut l, 0, a0 * b0);
        add(&mut l, 1, a0 * b1);
        add(&mut l, 1, a1 * b0);
        add(&mut l, 2, a1 * b1);
        // long division by d, bit by bit
        let mut q: [u128; 4] = [0; 4];
        let mut r: u128 = 0;
        for limb in (0..4).rev() {
            for bit in (0..64).rev() {
                let top = r >> 127;
                r = (r << 1) | ((l[limb] >> bit) & 1);
                if top == 1 || r >= d {
                    r = r.wrapping_sub(d);
                    q[limb] |= 1u128 << bit;
                }
            }
        }
        if q[2] != 0 || q[3] != 0 {
            None
        } else {
            Some(q[0] | (q[1] << 64))
        }
    }

    fn receipt(rng: &mut Rng) -> ResolvedPayoutReceiptV16 {
        if rng.next() % 5 == 0 {
            return ResolvedPayoutReceiptV16::EMPTY;
        }
        let face = rng.u128();
        let paid = match rng.next() % 3 {
            0 => 0,
            1 => if face == 0 { 0 } else { rng.u128() % (face + (face < u128::MAX) as u128) },
            _ => rng.u128(),
        };
        let prior = match rng.next() % 3 {
            0 => face.saturating_mul(BOUND_SCALE),
            1 => u128::MAX,
            _ => rng.u128(),
        };
        let finalized = if rng.next() % 4 == 0 { rng.b() } else { paid == face };
        ResolvedPayoutReceiptV16 {
            present: rng.next() % 8 != 0,
            prior_bound_contribution_num: prior,
            live_released_face_at_receipt: rng.u128(),
            terminal_positive_claim_face: face,
            paid_effective: paid,
            finalized,
        }
    }

    fn ledger(rng: &mut Rng) -> ResolvedPayoutLedgerV16 {
        ResolvedPayoutLedgerV16 {
            snapshot_residual: rng.u128(),
            terminal_claim_exact_receipts_num: rng.u128(),
            terminal_claim_bound_unreceipted_num: rng.u128(),
            current_payout_rate_num: rng.u128(),
            current_payout_rate_den: rng.u128(),
            snapshot_slot: rng.next(),
            payout_halted: rng.next() % 6 == 0,
            finalized: rng.b(),
        }
    }

    #[test]
    fn pr1() {
        let mut rng = Rng(0x9e3779b97f4a7c15);
        for _ in 0..2_000_000 {
            let l0 = ledger(&mut rng);
            let (e, u, r) = (l0.terminal_claim_exact_receipts_num, l0.terminal_claim_bound_unreceipted_num, l0.snapshot_residual);
            let res = V16Core::kernel_recompute_resolved_payout_rate(l0);
            let sum = e.checked_add(u);
            let expect_err = sum.is_none() || (sum != Some(0) && r.checked_mul(BOUND_SCALE).is_none());
            match res {
                Err(err) => {
                    assert!(expect_err, "unexpected Err {:?} on {:?}", err, l0);
                    assert_eq!(err, V16Error::ArithmeticOverflow);
                }
                Ok(l) => {
                    assert!(!expect_err);
                    let s = sum.unwrap();
                    assert!(l.current_payout_rate_den >= 1 && l.current_payout_rate_num <= l.current_payout_rate_den);
                    if s == 0 {
                        assert_eq!((l.current_payout_rate_num, l.current_payout_rate_den), (1, 1));
                    } else {
                        assert_eq!(l.current_payout_rate_den, s);
                        assert_eq!(l.current_payout_rate_num, (r * BOUND_SCALE).min(s));
                    }
                    let mut back = l;
                    back.current_payout_rate_num = l0.current_payout_rate_num;
                    back.current_payout_rate_den = l0.current_payout_rate_den;
                    assert_eq!(back, l0);
                }
            }
        }
    }

    #[test]
    fn p15() {
        let mut rng = Rng(0x2545f4914f6cdd1d);
        let mut oks = 0u64;
        for _ in 0..2_000_000 {
            let rc = receipt(&mut rng);
            let mut lg = ledger(&mut rng);
            // H_rate
            lg.current_payout_rate_den = lg.current_payout_rate_den.max(1);
            if lg.current_payout_rate_num > lg.current_payout_rate_den {
                lg.current_payout_rate_num %= lg.current_payout_rate_den + (lg.current_payout_rate_den < u128::MAX) as u128;
            }
            let (num, den) = (lg.current_payout_rate_num, lg.current_payout_rate_den);
            let res = MarketGroupV16ViewMut::<()>::resolved_receipt_claimable_against_ledger(rc, lg);
            let valid = validate_resolved_payout_receipt_value(rc).is_ok();
            match res {
                Ok(c) => {
                    oks += 1;
                    assert!(valid);
                    if !rc.present {
                        assert_eq!(c, 0);
                    } else {
                        assert!(!lg.payout_halted);
                        let g = floor_mul_div(rc.terminal_positive_claim_face, num, den).unwrap();
                        assert_eq!(Some(c), g.checked_sub(rc.paid_effective));
                        assert!(c <= rc.terminal_positive_claim_face - rc.paid_effective);
                    }
                }
                Err(e) => {
                    let rr = valid && rc.present && lg.payout_halted;
                    assert_eq!(e == V16Error::RecoveryRequired, rr, "{:?} {:?} {:?}", e, rc, lg);
                }
            }
        }
        assert!(oks > 1000, "too few Ok cases: {}", oks);
    }

    #[test]
    fn ours_k1() {
        let mut rng = Rng(0x853c49e6748fea9b);
        let mut nz = 0u64;
        for _ in 0..2_000_000 {
            let rc = receipt(&mut rng);
            let p = match rng.next() % 4 {
                0 => 0,
                1 => rc.terminal_positive_claim_face.saturating_sub(rc.paid_effective),
                2 => rng.u128() % (rc.terminal_positive_claim_face.saturating_sub(rc.paid_effective).saturating_add(1)).max(1),
                _ => rng.u128(),
            };
            if let Ok(r) = apply_resolved_payout_receipt_payment(rc, p) {
                assert!(validate_resolved_payout_receipt_value(rc).is_ok());
                assert!(validate_resolved_payout_receipt_value(r).is_ok());
                if p == 0 {
                    assert_eq!(r, rc);
                } else {
                    nz += 1;
                    assert!(rc.present);
                    assert!(p <= rc.terminal_positive_claim_face - rc.paid_effective);
                    assert_eq!(r.paid_effective, rc.paid_effective + p);
                    assert_eq!(r.finalized, r.paid_effective == r.terminal_positive_claim_face);
                    let mut back = r;
                    back.paid_effective = rc.paid_effective;
                    back.finalized = rc.finalized;
                    assert_eq!(back, rc);
                }
            }
        }
        assert!(nz > 1000, "too few nonzero Ok cases: {}", nz);
    }

    #[test]
    fn ours_k2() {
        let mut rng = Rng(0xda942042e4dd58b5);
        let mut oks = 0u64;
        for _ in 0..2_000_000 {
            let c = rng.u128();
            let r = rng.u128();
            let t = match rng.next() % 3 { 0 => c.wrapping_add(r), 1 => rng.u128(), _ => c.saturating_add(r) };
            let vb = rng.u128();
            let va = match rng.next() % 3 { 0 => vb.wrapping_sub(t), 1 => rng.u128(), _ => vb };
            let ok = TokenValueFlowProofV16::capital_and_resolved_payout_to_external_out(c, r, t, vb, va)
                .and_then(|p| p.validate())
                .is_ok();
            let spec = c.checked_add(r) == Some(t) && va <= vb && vb - va == t;
            assert_eq!(ok, spec, "c={} r={} t={} vb={} va={}", c, r, t, vb, va);
            oks += ok as u64;
        }
        assert!(oks > 1000, "too few Ok cases: {}", oks);
    }
}
