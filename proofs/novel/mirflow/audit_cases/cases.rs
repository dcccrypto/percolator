    // Functions written by the independent audit to attack the extractor (AUDIT-EXTRACTOR.md).
    // They are inserted into a scratch copy of the engine source by audit_regression.py; they are
    // never part of the engine.

    // ---- audit mutations ----
    pub fn audit_a1<'b>(&'b mut self, other: &mut MarketGroupV16ViewMut<'b, T>) -> V16Result<()> {
        self.validate_shape()?;
        other.header = &mut *self.header;
        other.header.asset_slot_capacity.bytes = [0xff; 4];
        Ok(())
    }
    pub fn audit_a2<'b>(
        &mut self,
        a1: &'b mut PortfolioV16ViewMut<'_>,
        a2: &mut PortfolioV16ViewMut<'b>,
    ) -> V16Result<()> {
        a1.validate_with_market(&self.as_view())?;
        a2.header = &mut *a1.header;
        a2.header.owner = [0xee; 32];
        Ok(())
    }
    pub fn audit_a1s<'b>(
        &'b mut self,
        slot: &mut Option<&'b mut MarketGroupV16HeaderAccount>,
    ) -> V16Result<()> {
        self.validate_shape()?;
        *slot = Some(&mut *self.header);
        if let Some(h) = slot {
            h.asset_slot_capacity.bytes = [0xff; 4];
        }
        Ok(())
    }
    #[inline(never)]
    fn audit_note(_msg: &str, h: &mut MarketGroupV16HeaderAccount) {
        h.asset_slot_capacity.bytes = [0xff; 4];
    }
    pub fn audit_a3(&mut self) -> V16Result<()> {
        self.validate_shape()?;
        Self::audit_note("a < b", self.header);
        Ok(())
    }
    pub fn audit_a3ctl(&mut self) -> V16Result<()> {
        self.validate_shape()?;
        Self::audit_note("a b", self.header);
        Ok(())
    }
    pub fn audit_a3gt(&mut self) -> V16Result<()> {
        self.validate_shape()?;
        Self::audit_note("a > b", self.header);
        Ok(())
    }
    pub fn audit_a3br(&mut self) -> V16Result<()> {
        self.validate_shape()?;
        Self::audit_note("[", self.header);
        Ok(())
    }
    #[inline(never)]
    fn audit_note_c(_c: char, h: &mut MarketGroupV16HeaderAccount) {
        h.asset_slot_capacity.bytes = [0xff; 4];
    }
    pub fn audit_a3ch(&mut self) -> V16Result<()> {
        self.validate_shape()?;
        Self::audit_note_c('<', self.header);
        Ok(())
    }
    pub fn audit_b1(&mut self) -> V16Result<()> {
        self.validate_shape()?;
        let mut o = Some(&mut *self.header);
        if let Some(h) = o.as_mut() {
            h.asset_slot_capacity.bytes = [0xff; 4];
        }
        Ok(())
    }
    pub fn audit_b1t(&mut self) -> V16Result<()> {
        self.validate_shape()?;
        let mut o = Some(&mut *self.header);
        let h = o.take().ok_or(V16Error::LockActive)?;
        h.asset_slot_capacity.bytes = [0xff; 4];
        Ok(())
    }
    pub fn audit_b1p(&mut self) -> V16Result<()> {
        self.validate_shape()?;
        let o = Some(&mut *self.header);
        if let Some(h) = o {
            h.asset_slot_capacity.bytes = [0xff; 4];
        }
        Ok(())
    }
    pub fn audit_b2(&mut self) -> V16Result<()> {
        self.validate_shape()?;
        let hd = (&mut *self.header, 3u8);
        hd.0.asset_slot_capacity.bytes = [0xff; 4];
        Ok(())
    }
    pub fn audit_b4(&mut self) -> V16Result<()> {
        self.validate_shape()?;
        let arr = [&mut self.header.asset_slot_capacity];
        for p in arr {
            p.bytes = [0xff; 4];
        }
        Ok(())
    }
    pub fn audit_b5(&mut self) -> V16Result<()> {
        self.validate_shape()?;
        let _old = core::mem::replace(&mut self.header.asset_slot_capacity, V16PodU32 { bytes: [0xff; 4] });
        Ok(())
    }
    pub fn audit_b6(&mut self) -> V16Result<()> {
        let mut r = self.validate_shape();
        let rr = &mut r;
        *rr = Ok(());
        r?;
        Ok(())
    }
    pub fn audit_b6b(&mut self) -> V16Result<()> {
        let mut r = (self.validate_shape(), 1u8);
        r.0 = Ok(());
        r.0?;
        Ok(())
    }
    pub fn audit_b7(&mut self, c: bool) -> V16Result<()> {
        let r = self.validate_shape();
        let r2: V16Result<()> = if c { r } else { Ok(()) };
        r2?;
        Ok(())
    }
    pub fn audit_b8(&mut self) -> V16Result<()> {
        let mut i = 0u8;
        loop {
            self.validate_shape()?;
            if i == 1 {
                self.header.asset_slot_capacity.bytes = [0xff; 4];
            }
            if i == 2 {
                break;
            }
            i += 1;
        }
        Ok(())
    }
    pub fn audit_b9(&mut self) -> V16Result<()> {
        let mut f = |h: &mut MarketGroupV16HeaderAccount| h.asset_slot_capacity.bytes = [0xff; 4];
        self.validate_shape()?;
        f(self.header);
        Ok(())
    }
    pub fn audit_b11(&mut self) -> V16Result<()> {
        self.validate_shape()?;
        self.protection_observed[0] = 7;
        Ok(())
    }
    pub fn audit_b12(&mut self) -> V16Result<()> {
        let ok = self.validate_shape().is_ok();
        if !ok {
            return Ok(());
        }
        Err(V16Error::LockActive)
    }
    pub fn audit_b13(&mut self) -> V16Result<()> {
        match self.validate_shape() {
            Err(_) => Ok(()),
            Ok(()) => Err(V16Error::LockActive),
        }
    }
    pub fn audit_b14(&mut self) -> V16Result<()> {
        let r = self.validate_shape();
        let s = &mut *self.header;
        let t = &mut *s;
        t.asset_slot_capacity.bytes = [0xff; 4];
        r
    }
    pub fn audit_b15(&mut self) -> V16Result<()> {
        self.validate_shape()?;
        let h: &mut MarketGroupV16HeaderAccount = self.header;
        let b: &mut [u8; 4] = &mut h.asset_slot_capacity.bytes;
        b[0] = 0xff;
        Ok(())
    }
    pub fn audit_b16(&mut self) -> V16Result<()> {
        self.validate_shape()?;
        self.header.asset_slot_capacity.bytes.iter_mut().for_each(|x| *x = 0xff);
        Ok(())
    }
    pub fn audit_b17(&mut self) -> V16Result<()> {
        self.validate_shape()?;
        let V16PodU32 { bytes } = &mut self.header.asset_slot_capacity;
        bytes.fill(0xff);
        Ok(())
    }
    fn audit_chk(&self) -> V16Result<()> {
        self.validate_shape()
    }
    fn audit_chk_panic(&self) {
        if self.validate_shape().is_err() {
            panic!("bad");
        }
    }
    fn audit_chk2nd(&self, _a: &PortfolioV16View<'_>, b: &PortfolioV16View<'_>) -> V16Result<()> {
        b.validate_with_market(&self.as_view())
    }
    pub fn audit_c2(&mut self, other: &mut MarketGroupV16ViewMut<'_, T>) -> V16Result<()> {
        other.audit_chk()?;
        Ok(())
    }
    pub fn audit_c2ok(&mut self) -> V16Result<()> {
        self.audit_chk()?;
        Ok(())
    }
    pub fn audit_c3(&mut self, x: &PortfolioV16View<'_>, y: &PortfolioV16View<'_>) -> V16Result<()> {
        self.audit_chk2nd(y, x)?;
        Ok(())
    }
    pub fn audit_c5(&mut self) -> V16Result<()> {
        self.audit_chk_panic();
        Ok(())
    }
    pub fn audit_c5w(&mut self) -> V16Result<()> {
        self.audit_chk_panic();
        self.header.asset_slot_capacity.bytes = [0xff; 4];
        Ok(())
    }
    pub fn audit_s1(&mut self) -> V16Result<()> {
        self.validate_shape()?;
        Self::audit_note("(", self.header);
        Ok(())
    }
    pub fn audit_s2(&mut self) -> V16Result<()> {
        self.validate_shape()?;
        Self::audit_note(")", self.header);
        Ok(())
    }
    pub fn audit_s3(&mut self) -> V16Result<()> {
        self.validate_shape()?;
        Self::audit_note("x -> y {", self.header);
        Ok(())
    }
    pub fn audit_s4(&mut self) -> V16Result<()> {
        self.validate_shape()?;
        self.header.asset_slot_capacity = Self::audit_mk("<");
        Ok(())
    }
    #[inline(never)]
    fn audit_mk(_m: &str) -> V16PodU32 {
        V16PodU32 { bytes: [0xff; 4] }
    }
    pub fn audit_p1<'b>(&'b mut self, other: &mut MarketGroupV16ViewMut<'b, T>) -> V16Result<()> {
        other.header = &mut *self.header;
        other.header.asset_slot_capacity.bytes = [0xff; 4];
        Ok(())
    }
    // ---- audit round 2, finding 1 ----
    pub fn r2_c3(&mut self, n: u8) -> V16Result<()> {
        self.header.asset_slot_capacity.bytes = [0xff; 4];
        let mut r = Err(V16Error::LockActive);
        let mut i = 0u8;
        while i < n {
            r = Ok(Err::<(), V16Error>);
            i += 1;
        }
        match r {
            Ok(_) => Ok(()),
            Err(e) => Err(e),
        }
    }
    pub fn r2_c4(&mut self) -> V16Result<()> {
        self.r2_c3(1)?;
        Ok(())
    }
