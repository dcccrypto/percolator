    // ---- audit round 4: R10 / R9 attack methods. Never part of the engine.
    #[inline(never)]
    fn a4_pick_ctot(h: &mut MarketGroupV16HeaderAccount) -> &mut V16PodU128 {
        &mut h.c_tot
    }
    #[inline(never)]
    fn a4_pick_cfg(h: &mut MarketGroupV16HeaderAccount) -> &mut V16ConfigAccount {
        &mut h.config
    }
    #[inline(never)]
    fn a4_pick_mpc(h: &mut MarketGroupV16HeaderAccount) -> &mut V16PodU64 {
        &mut h.materialized_portfolio_count
    }
    #[inline(never)]
    fn a4_set64(p: &mut V16PodU64) {
        *p = V16PodU64::new(u64::MAX);
    }
    #[inline(never)]
    fn a4_set128(p: &mut V16PodU128) {
        *p = V16PodU128::new(u128::MAX);
    }
    #[inline(never)]
    fn a4_id64(p: &mut V16PodU64) -> &mut V16PodU64 {
        p
    }
    #[inline(never)]
    fn a4_two64(a: &mut V16PodU64, b: &mut V16PodU64) {
        *a = V16PodU64::new(1);
        *b = V16PodU64::new(u64::MAX);
    }
    #[inline(never)]
    fn a4_pick2<'x>(a: &'x mut V16PodU64, b: &'x mut V16PodU64, f: bool) -> &'x mut V16PodU64 {
        if f { a } else { b }
    }
    #[inline(never)]
    fn a4_slots(ms: &mut [Market<T>]) {
        if let Some(m) = ms.get_mut(0) {
            m.engine = bytemuck::Zeroable::zeroed();
        }
    }
    #[inline(never)]
    fn a4_fs(p: &mut V16PodU64, ms: &mut [Market<T>]) {
        *p = V16PodU64::new(3);
        if let Some(m) = ms.get_mut(0) {
            m.engine = bytemuck::Zeroable::zeroed();
        }
    }
    #[inline(never)]
    fn a4_wr_read(&mut self) -> V16Result<u8> {
        self.header.c_tot = V16PodU128::new(u128::MAX);
        Ok(0)
    }
    #[inline(never)]
    fn a4_wr_unread(&mut self) -> V16Result<u8> {
        self.header.stale_certificate_count = V16PodU64::new(9);
        Ok(0)
    }
    #[inline(never)]
    fn a4_wr_plain(&mut self) {
        self.header.slot_last = V16PodU64::new(u64::MAX);
    }

    // H1 family: a pointer returned by a call that was given the WHOLE header keeps region
    // "header"; a store `(*p).0` through it is then read as "header field 0".
    pub fn a4_h1(&mut self) -> V16Result<()> {
        let p = Self::a4_pick_ctot(self.header);
        p.bytes = [0xff; 16];
        Ok(())
    }
    pub fn a4_h1a(&mut self, account: &mut PortfolioV16ViewMut<'_>) -> V16Result<()> {
        let _ = &account;
        let p = Self::a4_pick_ctot(self.header);
        p.bytes = [0xff; 16];
        Ok(())
    }
    pub fn a4_h1ctl(&mut self) -> V16Result<()> {
        let p = Self::a4_pick_ctot(self.header);
        *p = V16PodU128::new(u128::MAX);
        Ok(())
    }
    pub fn a4_h1ok(&mut self) -> V16Result<()> {
        let p = Self::a4_pick_mpc(self.header);
        p.bytes = [0xff; 8];
        Ok(())
    }
    pub fn a4_h2(&mut self) -> V16Result<()> {
        let c = Self::a4_pick_cfg(self.header);
        c.max_portfolio_assets = V16PodU16::new(0);
        Ok(())
    }
    pub fn a4_h2a(&mut self, account: &mut PortfolioV16ViewMut<'_>) -> V16Result<()> {
        let _ = &account;
        let c = Self::a4_pick_cfg(self.header);
        c.h_max = V16PodU64::new(0);
        Ok(())
    }
    pub fn a4_h3(&mut self) -> V16Result<()> {
        // config field 19 through a pointer whose region is "header": read as header field 19
        let c = Self::a4_pick_cfg(self.header);
        Self::a4_set64(&mut c.asset_activation_cooldown_slots);
        Ok(())
    }
    pub fn a4_h3b(&mut self, account: &mut PortfolioV16ViewMut<'_>) -> V16Result<()> {
        // config field 5 (h_max) -> "header field 5" (c_tot), unread by the account check
        let _ = &account;
        let c = Self::a4_pick_cfg(self.header);
        Self::a4_set64(&mut c.h_max);
        Ok(())
    }
    pub fn a4_h3c(&mut self) -> V16Result<()> {
        // config field 0 via &mut c.max_portfolio_assets handed to core::mem::take (sets 0)
        let c = Self::a4_pick_cfg(self.header);
        let _ = core::mem::take(&mut c.max_portfolio_assets);
        Ok(())
    }
    pub fn a4_h4(&mut self) -> V16Result<()> {
        // same, with no crate helper: bytemuck only
        let b = bytemuck::bytes_of_mut(self.header);
        let off = core::mem::offset_of!(MarketGroupV16HeaderAccount, c_tot);
        let p: &mut V16PodU128 = bytemuck::from_bytes_mut(&mut b[off..off + 16]);
        p.bytes = [0xff; 16];
        Ok(())
    }
    pub fn a4_h5(&mut self) -> V16Result<()> {
        // FnOnce closure that owns the header pointer and returns a field pointer
        let h = &mut *self.header;
        let p: &mut V16PodU128 = a4_once(move || {
            let hh = h;
            &mut hh.c_tot
        });
        p.bytes = [0xff; 16];
        Ok(())
    }
    pub fn a4_h6(&mut self) -> V16Result<()> {
        // Option-wrapped whole header, mapped to a field pointer by core
        let o = Some(&mut *self.header);
        if let Some(p) = o.map(|h| &mut h.c_tot) {
            p.bytes = [0xff; 16];
        }
        Ok(())
    }

    // ---- other region / store / call attacks
    pub fn a4_b1(&mut self, flag: bool) -> V16Result<()> {
        let h = &mut *self.header;
        let o: Option<&mut V16PodU64> = if flag { Some(&mut h.materialized_portfolio_count) } else { Some(&mut h.slot_last) };
        if let Some(p) = o {
            *p = V16PodU64::new(u64::MAX);
        }
        Ok(())
    }
    pub fn a4_b1r(&mut self) -> V16Result<()> {
        let h = &mut *self.header;
        let o: Option<&mut V16PodU64> = Some(&mut h.slot_last);
        if let Some(p) = o {
            *p = V16PodU64::new(u64::MAX);
        }
        Ok(())
    }
    pub fn a4_b1ok(&mut self) -> V16Result<()> {
        let h = &mut *self.header;
        let o: Option<&mut V16PodU64> = Some(&mut h.materialized_portfolio_count);
        if let Some(p) = o {
            *p = V16PodU64::new(u64::MAX);
        }
        Ok(())
    }
    pub fn a4_b2(&mut self, other: &mut MarketGroupV16ViewMut<'a, T>) -> V16Result<()> {
        other.header.c_tot = V16PodU128::new(u128::MAX);
        Ok(())
    }
    pub fn a4_b2s(&mut self, other: &mut MarketGroupV16ViewMut<'a, T>) -> V16Result<()> {
        core::mem::swap(&mut self.header, &mut other.header);
        Ok(())
    }
    pub fn a4_b2m(&mut self, other: &mut MarketGroupV16ViewMut<'a, T>) -> V16Result<()> {
        core::mem::swap(&mut self.markets, &mut other.markets);
        Ok(())
    }
    pub fn a4_b2x(&mut self, other: &mut MarketGroupV16ViewMut<'a, T>) -> V16Result<()> {
        core::mem::swap(&mut self.header.materialized_portfolio_count, &mut other.header.slot_last);
        Ok(())
    }
    pub fn a4_b2y(&mut self, other: &mut MarketGroupV16ViewMut<'a, T>) -> V16Result<()> {
        core::mem::swap(&mut self.header.slot_last, &mut other.header.materialized_portfolio_count);
        Ok(())
    }
    pub fn a4_b2z(&mut self, other: &mut MarketGroupV16ViewMut<'a, T>) -> V16Result<()> {
        // the header pointer of self is replaced first; the later store goes to other's old header
        core::mem::swap(&mut self.header, &mut other.header);
        self.header.materialized_portfolio_count = V16PodU64::new(1);
        Ok(())
    }
    pub fn a4_b3(&mut self, flag: bool) -> V16Result<()> {
        let h = &mut *self.header;
        let mut p: &mut V16PodU64 = &mut h.materialized_portfolio_count;
        if flag {
            p = &mut h.slot_last;
        }
        *p = V16PodU64::new(u64::MAX);
        Ok(())
    }
    pub fn a4_b4(&mut self) -> V16Result<()> {
        let h = &mut *self.header;
        let s = A4Hold { p: &mut h.materialized_portfolio_count, q: &mut h.slot_last };
        *s.q = V16PodU64::new(u64::MAX);
        Ok(())
    }
    pub fn a4_b4ok(&mut self) -> V16Result<()> {
        let h = &mut *self.header;
        let s = A4Hold { p: &mut h.materialized_portfolio_count, q: &mut h.stale_certificate_count };
        *s.q = V16PodU64::new(u64::MAX);
        Ok(())
    }
    pub fn a4_b4h(&mut self) -> V16Result<()> {
        let s = A4HoldH { h: &mut *self.header, n: 19 };
        s.h.c_tot = V16PodU128::new(u128::MAX);
        Ok(())
    }
    pub fn a4_b4t(&mut self) -> V16Result<()> {
        let h = &mut *self.header;
        let t = (&mut h.materialized_portfolio_count, &mut h.slot_last);
        *t.1 = V16PodU64::new(u64::MAX);
        Ok(())
    }
    pub fn a4_b5(&mut self) -> V16Result<()> {
        let p = &mut self.header.c_tot;
        let mut f = || {
            *p = V16PodU128::new(u128::MAX);
        };
        f();
        Ok(())
    }
    pub fn a4_b5m(&mut self) -> V16Result<()> {
        let p = &mut self.header.c_tot;
        let f = move || {
            let p = p;
            *p = V16PodU128::new(u128::MAX);
        };
        f();
        Ok(())
    }
    pub fn a4_b5ok(&mut self) -> V16Result<()> {
        let p = &mut self.header.materialized_portfolio_count;
        let f = move || {
            let p = p;
            *p = V16PodU64::new(5);
        };
        f();
        Ok(())
    }
    pub fn a4_b5h(&mut self) -> V16Result<()> {
        let h = &mut *self.header;
        let mut f = move || {
            h.materialized_portfolio_count = V16PodU64::new(5);
            h.c_tot = V16PodU128::new(u128::MAX);
        };
        f();
        Ok(())
    }
    pub fn a4_b5w(&mut self) -> V16Result<()> {
        // closure captures the whole header pointer by value (move), writes a read field
        let h: &mut MarketGroupV16HeaderAccount = self.header;
        let f = move |x: u64| {
            let hh = h;
            hh.slot_last = V16PodU64::new(x);
        };
        f(u64::MAX);
        Ok(())
    }
    pub fn a4_b6(&mut self) -> V16Result<u8> {
        let c = self.a4_wr_read()?;
        Ok(c)
    }
    pub fn a4_b6ok(&mut self) -> V16Result<u8> {
        let c = self.a4_wr_unread()?;
        Ok(c)
    }
    pub fn a4_b6p(&mut self) -> V16Result<()> {
        let f: fn(&mut Self) = Self::a4_wr_plain;
        f(self);
        Ok(())
    }
    pub fn a4_b6q(&mut self) -> V16Result<()> {
        Self::a4_wr_plain(&mut *self);
        Ok(())
    }
    pub fn a4_b6r(&mut self) -> V16Result<()> {
        let me: &mut Self = self;
        let me2 = &mut *me;
        me2.a4_wr_plain();
        Ok(())
    }
    pub fn a4_b7(&mut self, account: &mut PortfolioV16ViewMut<'_>) -> V16Result<()> {
        let _ = &account;
        self.header.current_slot = V16PodU64::new(u64::MAX);
        Ok(())
    }
    pub fn a4_b7g(&mut self, account: &mut PortfolioV16ViewMut<'_>) -> V16Result<()> {
        let _ = &account;
        self.header.market_group_id[3] ^= 1;
        Ok(())
    }
    pub fn a4_b7c(&mut self, account: &mut PortfolioV16ViewMut<'_>) -> V16Result<()> {
        let _ = &account;
        self.header.config.h_max = V16PodU64::new(0);
        Ok(())
    }
    pub fn a4_b7t(&mut self, account: &mut PortfolioV16ViewMut<'_>) -> V16Result<()> {
        let _ = &account;
        self.header.c_tot = V16PodU128::new(u128::MAX);
        Ok(())
    }
    pub fn a4_b7s(&mut self, account: &mut PortfolioV16ViewMut<'_>) -> V16Result<()> {
        let _ = &account;
        Self::a4_slots(self.markets);
        Ok(())
    }
    pub fn a4_b7i(&mut self, account: &mut PortfolioV16ViewMut<'_>) -> V16Result<()> {
        let _ = &account;
        for m in self.markets.iter_mut() {
            m.engine = bytemuck::Zeroable::zeroed();
        }
        Ok(())
    }
    pub fn a4_b7m(&mut self, account: &mut PortfolioV16ViewMut<'_>) -> V16Result<()> {
        let _ = &account;
        Self::a4_set64(&mut self.header.current_slot);
        Ok(())
    }
    pub fn a4_b7u(&mut self, account: &mut PortfolioV16ViewMut<'_>) -> V16Result<()> {
        let _ = &account;
        let m = &mut self.header.mode;
        *m = 77;
        Ok(())
    }
    pub fn a4_b7cp(&mut self, account: &mut PortfolioV16ViewMut<'_>) -> V16Result<()> {
        let _ = &account;
        self.header.market_group_id.copy_from_slice(&[1u8; 32]);
        Ok(())
    }
    pub fn a4_b8(&mut self) -> V16Result<()> {
        let h = &mut *self.header;
        Self::a4_two64(&mut h.materialized_portfolio_count, &mut h.slot_last);
        Ok(())
    }
    pub fn a4_b8ok(&mut self) -> V16Result<()> {
        let h = &mut *self.header;
        Self::a4_two64(&mut h.materialized_portfolio_count, &mut h.stale_certificate_count);
        Ok(())
    }
    pub fn a4_b9(&mut self, flag: bool) -> V16Result<()> {
        let h = &mut *self.header;
        let p = Self::a4_pick2(&mut h.materialized_portfolio_count, &mut h.slot_last, flag);
        *p = V16PodU64::new(u64::MAX);
        Ok(())
    }
    pub fn a4_b9i(&mut self) -> V16Result<()> {
        let p = Self::a4_id64(&mut self.header.slot_last);
        *p = V16PodU64::new(u64::MAX);
        Ok(())
    }
    pub fn a4_b9ok(&mut self) -> V16Result<()> {
        let p = Self::a4_id64(&mut self.header.materialized_portfolio_count);
        *p = V16PodU64::new(u64::MAX);
        Ok(())
    }
    pub fn a4_b10(&mut self) -> V16Result<()> {
        let h = &mut *self.header;
        let _old = core::mem::replace(&mut h.slot_last, V16PodU64::new(u64::MAX));
        Ok(())
    }
    pub fn a4_b11(&mut self) -> V16Result<()> {
        self.header.resolved_payout_ledger.snapshot_slot = V16PodU64::new(u64::MAX);
        self.header.resolved_payout_ledger.snapshot_residual = V16PodU128::new(1);
        Ok(())
    }
    pub fn a4_b11p(&mut self) -> V16Result<()> {
        let l = &mut self.header.resolved_payout_ledger;
        let q = &mut l.snapshot_residual;
        q.bytes[0] = 1;
        Ok(())
    }
    pub fn a4_b12(&mut self) -> V16Result<()> {
        let v: &mut Self = &mut *self;
        let h: &mut MarketGroupV16HeaderAccount = &mut *v.header;
        let h2 = &mut *h;
        let p = &mut h2.c_tot;
        let p2 = &mut *p;
        *p2 = V16PodU128::new(u128::MAX);
        Ok(())
    }
    pub fn a4_b12ok(&mut self) -> V16Result<()> {
        let h: &mut MarketGroupV16HeaderAccount = &mut *self.header;
        let h2 = &mut *h;
        let p = &mut h2.materialized_portfolio_count;
        let p2 = &mut *p;
        *p2 = V16PodU64::new(1);
        Ok(())
    }
    pub fn a4_b13(&mut self) -> V16Result<()> {
        let hh: &mut &mut MarketGroupV16HeaderAccount = &mut self.header;
        hh.c_tot = V16PodU128::new(u128::MAX);
        Ok(())
    }
    pub fn a4_b14(&mut self) -> V16Result<()> {
        *self.header = bytemuck::Zeroable::zeroed();
        Ok(())
    }
    pub fn a4_b14o(&mut self, other: &mut MarketGroupV16ViewMut<'a, T>) -> V16Result<()> {
        *self.header = *other.header;
        Ok(())
    }
    pub fn a4_b15(&mut self) -> V16Result<()> {
        Self::a4_fs(&mut self.header.materialized_portfolio_count, self.markets);
        Ok(())
    }
    pub fn a4_b15a(&mut self, account: &mut PortfolioV16ViewMut<'_>) -> V16Result<()> {
        let _ = &account;
        Self::a4_fs(&mut self.header.materialized_portfolio_count, self.markets);
        Ok(())
    }
    pub fn a4_b16(a: &mut MarketGroupV16ViewMut<'_, T>, b: &mut MarketGroupV16ViewMut<'_, T>) -> V16Result<()> {
        let _ = &b;
        a.header.c_tot = V16PodU128::new(u128::MAX);
        Ok(())
    }
    pub fn a4_b16s(a: &MarketGroupV16ViewMut<'_, T>, b: &mut MarketGroupV16ViewMut<'_, T>) -> V16Result<()> {
        let _ = &a;
        b.header.c_tot = V16PodU128::new(u128::MAX);
        Ok(())
    }
    pub fn a4_b17(&mut self) -> V16Result<()> {
        let ps: [&mut V16PodU64; 2] = {
            let h = &mut *self.header;
            [&mut h.materialized_portfolio_count, &mut h.stale_certificate_count]
        };
        for p in ps {
            *p = V16PodU64::new(u64::MAX);
        }
        Ok(())
    }
    pub fn a4_b18(&mut self) -> V16Result<()> {
        let h = &mut *self.header;
        let MarketGroupV16HeaderAccount { materialized_portfolio_count: a, slot_last: b, .. } = h;
        *a = V16PodU64::new(1);
        *b = V16PodU64::new(u64::MAX);
        Ok(())
    }
    pub fn a4_b19(&mut self, n: usize) -> V16Result<()> {
        let h = &mut *self.header;
        let mut i = 0;
        while i < n {
            let p = if i == 0 { &mut h.materialized_portfolio_count } else { &mut h.slot_last };
            *p = V16PodU64::new(u64::MAX);
            i += 1;
        }
        Ok(())
    }
    pub fn a4_b20(&mut self) -> V16Result<()> {
        // slot written through a pointer handed back by core
        if let Some((first, _rest)) = self.markets.split_first_mut() {
            first.engine = bytemuck::Zeroable::zeroed();
        }
        Ok(())
    }
    pub fn a4_b21(&mut self) -> V16Result<()> {
        // the store's destination is the result of a call
        self.header.slot_last = V16PodU64::new(u64::MAX);
        self.header.materialized_portfolio_count = V16PodU64::new(1);
        Ok(())
    }
    pub fn a4_b22(&mut self) -> V16Result<()> {
        let h = &mut *self.header;
        let a: &mut V16PodU64 = &mut h.materialized_portfolio_count;
        let b: &mut V16PodU64 = &mut h.slot_last;
        core::mem::swap(a, b);
        Ok(())
    }

    // ---- R9
    #[inline(never)]
    fn a4_e_callee(&mut self, f: bool) -> V16Result<A4Disc> {
        if f {
            self.header.c_tot = V16PodU128::new(u128::MAX);
            return Ok(A4Disc::Wrote(1));
        }
        Ok(A4Disc::Clean(0))
    }
    pub fn a4_e1(&mut self, f: bool) -> V16Result<()> {
        match self.a4_e_callee(f)? {
            A4Disc::Wrote(_) => Ok(()),
            A4Disc::Clean(_) => {
                self.validate_shape()?;
                Ok(())
            }
        }
    }
    #[inline(never)]
    fn a4_e_callee2(&mut self, f: bool) -> V16Result<A4Multi> {
        if f {
            self.header.c_tot = V16PodU128::new(u128::MAX);
            return Ok(A4Multi::Wrote(1));
        }
        Ok(A4Multi::Clean(0))
    }
    pub fn a4_e2(&mut self, f: bool) -> V16Result<()> {
        match self.a4_e_callee2(f)? {
            A4Multi::Wrote(_) => Ok(()),
            A4Multi::Clean(_) => {
                self.validate_shape()?;
                Ok(())
            }
        }
    }
    #[inline(never)]
    fn a4_e_callee3(&mut self, f: bool) -> V16Result<A4Decoy> {
        if f {
            self.header.c_tot = V16PodU128::new(u128::MAX);
            return Ok(A4Decoy::Wrote);
        }
        Ok(A4Decoy::Clean)
    }
    pub fn a4_e3(&mut self, f: bool) -> V16Result<()> {
        match self.a4_e_callee3(f)? {
            A4Decoy::Wrote => Ok(()),
            A4Decoy::Clean => {
                self.validate_shape()?;
                Ok(())
            }
        }
    }
    #[inline(never)]
    fn a4_e_callee4(&mut self, f: bool) -> V16Result<A4Plain> {
        if f {
            self.header.c_tot = V16PodU128::new(u128::MAX);
            return Ok(A4Plain::Wrote);
        }
        Ok(A4Plain::Clean)
    }
    pub fn a4_e4(&mut self, f: bool) -> V16Result<()> {
        // control: honest plain enum, WRONG arm validated (must be refused)
        match self.a4_e_callee4(f)? {
            A4Plain::Wrote => Ok(()),
            A4Plain::Clean => {
                self.validate_shape()?;
                Ok(())
            }
        }
    }
    pub fn a4_e4ok(&mut self, f: bool) -> V16Result<()> {
        // control: honest plain enum, right arm validated
        match self.a4_e_callee4(f)? {
            A4Plain::Clean => Ok(()),
            A4Plain::Wrote => {
                self.validate_shape()?;
                Ok(())
            }
        }
    }
    #[inline(never)]
    fn a4_e_callee5(&mut self, f: bool) -> V16Result<A4Cfg> {
        if f {
            self.header.c_tot = V16PodU128::new(u128::MAX);
            return Ok(A4Cfg::Wrote);
        }
        Ok(A4Cfg::Clean)
    }
    pub fn a4_e5(&mut self, f: bool) -> V16Result<()> {
        match self.a4_e_callee5(f)? {
            A4Cfg::Wrote => Ok(()),
            A4Cfg::Clean => {
                self.validate_shape()?;
                Ok(())
            }
        }
    }
