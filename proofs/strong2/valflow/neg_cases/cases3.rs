    // Functions written to attack rule R10 (read-set frame). Never part of the engine.
    #[inline(never)]
    fn s3_set64(p: &mut V16PodU64) {
        *p = V16PodU64::new(7);
    }
    #[inline(never)]
    fn s3_set128(p: &mut V16PodU128) {
        *p = V16PodU128::new(u128::MAX);
    }
    #[inline(never)]
    fn s3_hdr(h: &mut MarketGroupV16HeaderAccount) {
        h.c_tot = V16PodU128::new(u128::MAX);
    }
    #[inline(never)]
    fn s3_callee_unread(&mut self) {
        self.header.stale_certificate_count = V16PodU64::new(9);
    }
    #[inline(never)]
    fn s3_callee_read(&mut self) {
        self.header.c_tot = V16PodU128::new(u128::MAX);
    }
    pub fn s3_f1(&mut self) -> V16Result<()> {
        self.header.materialized_portfolio_count = V16PodU64::new(7);
        Ok(())
    }
    pub fn s3_f2(&mut self) -> V16Result<()> {
        self.header.c_tot = V16PodU128::new(u128::MAX);
        Ok(())
    }
    pub fn s3_f3(&mut self) -> V16Result<()> {
        Self::s3_set128(&mut self.header.vault);
        Ok(())
    }
    pub fn s3_f4(&mut self) -> V16Result<()> {
        Self::s3_set64(&mut self.header.materialized_portfolio_count);
        Ok(())
    }
    pub fn s3_f5(&mut self) -> V16Result<()> {
        Self::s3_hdr(self.header);
        Ok(())
    }
    pub fn s3_f6(&mut self, flag: bool) -> V16Result<()> {
        let p = if flag {
            &mut self.header.materialized_portfolio_count
        } else {
            &mut self.header.slot_last
        };
        *p = V16PodU64::new(u64::MAX);
        Ok(())
    }
    pub fn s3_f7(&mut self) -> V16Result<()> {
        if let Some(m) = self.markets.get_mut(0) {
            m.engine = bytemuck::Zeroable::zeroed();
        }
        Ok(())
    }
    pub fn s3_f7a(&mut self, account: &mut PortfolioV16ViewMut<'_>) -> V16Result<()> {
        let _ = &account;
        if let Some(m) = self.markets.get_mut(0) {
            m.engine = bytemuck::Zeroable::zeroed();
        }
        Ok(())
    }
    pub fn s3_f9(&mut self, account: &mut PortfolioV16ViewMut<'_>) -> V16Result<()> {
        let _ = &account;
        self.header.mode = 77;
        Ok(())
    }
    pub fn s3_f10(&mut self) -> V16Result<()> {
        self.header.config = bytemuck::Zeroable::zeroed();
        Ok(())
    }
    pub fn s3_f11(&mut self) -> V16Result<()> {
        let h = &mut *self.header;
        let a = &mut h.materialized_portfolio_count;
        *a = V16PodU64::new(1);
        h.c_tot = V16PodU128::new(u128::MAX);
        Ok(())
    }
    pub fn s3_f12(&mut self) -> V16Result<()> {
        bytemuck::bytes_of_mut(self.header)[40] = 0xff;
        Ok(())
    }
    pub fn s3_f13(&mut self) -> V16Result<()> {
        let h = &mut *self.header;
        core::mem::swap(&mut h.materialized_portfolio_count, &mut h.slot_last);
        Ok(())
    }
    pub fn s3_f14(&mut self) -> V16Result<()> {
        let h = &mut *self.header;
        core::mem::swap(&mut h.stale_certificate_count, &mut h.b_stale_account_count);
        Ok(())
    }
    pub fn s3_f15(&mut self) -> V16Result<()> {
        self.s3_callee_read();
        Ok(())
    }
    pub fn s3_f15ok(&mut self) -> V16Result<()> {
        self.s3_callee_unread();
        Ok(())
    }
    pub fn s3_f16(&mut self, x: u64) -> V16Result<()> {
        let h = &mut *self.header;
        let p: &mut V16PodU64 = &mut h.materialized_portfolio_count;
        let q: &mut V16PodU64 = &mut h.current_slot;
        let r = if x == 0 { p } else { q };
        *r = V16PodU64::new(0);
        Ok(())
    }
    pub fn s3_f17(&mut self) -> V16Result<()> {
        let hs: [&mut V16PodU64; 2] = {
            let h = &mut *self.header;
            [&mut h.materialized_portfolio_count, &mut h.slot_last]
        };
        for p in hs {
            *p = V16PodU64::new(u64::MAX);
        }
        Ok(())
    }
    pub fn s3_f18(&mut self) -> V16Result<()> {
        // index 19 of a DIFFERENT struct reached from the header: config's fields
        self.header.config.max_market_slots = V16PodU32::new(0);
        Ok(())
    }
    pub fn s3_f19(&mut self) -> V16Result<()> {
        let s: &mut [Market<T>] = &mut *self.markets;
        let _ = s.len();
        self.header.vault = V16PodU128::new(u128::MAX);
        Ok(())
    }
