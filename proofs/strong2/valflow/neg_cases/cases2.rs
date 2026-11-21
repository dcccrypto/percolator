    // Functions written to attack the strong2 translator rules R1..R8 (proofs/strong2/RULES.md).
    // negative_tests.py inserts them into a scratch copy of the engine source; they are never
    // part of the engine. "w" = writes the market header with no check.
    #[inline(never)]
    fn s2_w(&mut self) {
        self.header.asset_slot_capacity.bytes = [0xff; 4];
    }
    #[inline(never)]
    fn s2_zero_nowrite(&mut self, x: u128) -> V16Result<u128> {
        if x == 0 {
            return Ok(0);
        }
        self.s2_w();
        Ok(x)
    }
    #[inline(never)]
    fn s2_zero_write(&mut self, x: u128) -> V16Result<u128> {
        self.s2_w();
        Ok(x)
    }
    #[inline(never)]
    fn s2_one_nowrite(&mut self, x: u128) -> V16Result<u128> {
        if x == 1 {
            return Ok(1);
        }
        self.s2_w();
        Ok(x)
    }
    #[inline(never)]
    fn s2_const_after_write(&mut self, x: u128) -> V16Result<u128> {
        if x == 0 {
            return Ok(0);
        }
        self.s2_w();
        Ok(0)
    }
    #[inline(never)]
    fn s2_mut_payload(&mut self, x: u128) -> V16Result<u128> {
        let mut v = x;
        if v == 0 {
            return Ok(v);
        }
        self.s2_w();
        v = v.wrapping_sub(x);
        Ok(v)
    }
    #[inline(never)]
    fn s2_trunc(&mut self, x: u128) -> V16Result<u64> {
        if x == 0 {
            return Ok(0);
        }
        self.s2_w();
        Ok(x as u64)
    }
    #[inline(never)]
    fn s2_arith(&mut self, x: u128) -> V16Result<u128> {
        if x == 0 {
            return Ok(0);
        }
        self.s2_w();
        Ok(x - 1)
    }
    #[inline(never)]
    fn s2_bool(&mut self, x: u128) -> V16Result<bool> {
        if x == 0 {
            return Ok(false);
        }
        self.s2_w();
        Ok(x > 5)
    }
    #[inline(never)]
    fn s2_opt_good(&mut self, x: u128) -> V16Result<Option<u8>> {
        if x == 0 {
            return Ok(None);
        }
        self.s2_w();
        self.validate_shape()?;
        Ok(Some(1))
    }
    #[inline(never)]
    fn s2_opt_bad(&mut self, x: u128) -> V16Result<Option<u8>> {
        if x == 0 {
            self.validate_shape()?;
            return Ok(None);
        }
        self.s2_w();
        Ok(Some(1))
    }
    #[inline(never)]
    fn s2_opt_w(&mut self, x: u128) -> V16Result<Option<u8>> {
        if x == 0 {
            return Ok(None);
        }
        self.s2_w();
        Ok(Some(1))
    }
    #[inline(never)]
    fn s2_opt_var(&mut self, x: u128) -> V16Result<Option<u8>> {
        let mut o = None;
        if x != 0 {
            self.s2_w();
            o = Some(1u8);
            if x == 7 {
                o = None;
            }
        }
        Ok(o)
    }
    #[inline(never)]
    fn s2_wrap_good(&mut self, x: u128) -> V16Result<u128> {
        self.s2_zero_nowrite(x)
    }
    #[inline(never)]
    fn s2_wrap_bad(&mut self, x: u128) -> V16Result<u128> {
        self.s2_zero_write(x)
    }
    #[inline(never)]
    fn s2_poke(r: &mut V16Result<u128>) {
        *r = Ok(0);
    }

    pub fn s2_n1(&mut self, x: u128) -> V16Result<u128> {
        let c = self.s2_zero_nowrite(x)?;
        if c != 0 {
            self.validate_shape()?;
        }
        Ok(c)
    }
    pub fn s2_n2(&mut self, x: u128) -> V16Result<u128> {
        let c = self.s2_zero_write(x)?;
        if c != 0 {
            self.validate_shape()?;
        }
        Ok(c)
    }
    pub fn s2_n3(&mut self, x: u128) -> V16Result<u128> {
        let c = self.s2_one_nowrite(x)?;
        if c != 0 {
            self.validate_shape()?;
        }
        Ok(c)
    }
    pub fn s2_n3ok(&mut self, x: u128) -> V16Result<u128> {
        let c = self.s2_one_nowrite(x)?;
        if c != 1 {
            self.validate_shape()?;
        }
        Ok(c)
    }
    pub fn s2_n4(&mut self, x: u128) -> V16Result<u128> {
        let c = self.s2_zero_nowrite(x)?;
        if c == 0 {
            self.validate_shape()?;
        }
        Ok(c)
    }
    pub fn s2_n5(&mut self, x: u128, flag: bool) -> V16Result<u128> {
        let mut r = self.s2_zero_nowrite(x);
        if flag {
            r = self.s2_zero_write(x);
        }
        let c = r?;
        if c != 0 {
            self.validate_shape()?;
        }
        Ok(c)
    }
    pub fn s2_n6(&mut self, x: u128) -> V16Result<u128> {
        let mut c = self.s2_zero_nowrite(x)?;
        c = c.wrapping_sub(x);
        if c != 0 {
            self.validate_shape()?;
        }
        Ok(c)
    }
    pub fn s2_n7(&mut self, x: u128, y: u128) -> V16Result<u128> {
        let c = self.s2_zero_nowrite(x)?;
        if y != 0 {
            self.validate_shape()?;
        }
        Ok(c)
    }
    pub fn s2_n8(&mut self, x: u128) -> V16Result<u128> {
        let c = self.s2_zero_nowrite(x)?;
        self.s2_w();
        if c != 0 {
            self.validate_shape()?;
        }
        Ok(c)
    }
    pub fn s2_n9(&mut self, x: u128) -> V16Result<()> {
        if let Some(_) = self.s2_opt_bad(x)? {
            return Ok(());
        }
        self.validate_shape()?;
        Ok(())
    }
    pub fn s2_n10(&mut self, x: u128) -> V16Result<()> {
        if let Some(_) = self.s2_opt_good(x)? {
            return Ok(());
        }
        Ok(())
    }
    pub fn s2_n11(&mut self, x: u128) -> V16Result<()> {
        let o = self.s2_opt_w(x)?;
        if o.is_some() {
            return Ok(());
        }
        self.validate_shape()?;
        Ok(())
    }
    pub fn s2_n12(&mut self, x: u128) -> V16Result<()> {
        let o = self.s2_opt_w(x)?;
        if o.is_some() {
            self.validate_shape()?;
        }
        Ok(())
    }
    pub fn s2_n12n(&mut self, x: u128) -> V16Result<()> {
        let o = self.s2_opt_w(x)?;
        if o.is_none() {
            self.validate_shape()?;
        }
        Ok(())
    }
    pub fn s2_n13(&mut self, x: u128, y: u128) -> V16Result<u128> {
        let mut c = self.s2_zero_nowrite(x)?;
        let d = c;
        c = y;
        if c != 0 {
            self.validate_shape()?;
        }
        Ok(d)
    }
    pub fn s2_n14(&mut self, x: u128) -> V16Result<()> {
        match self.s2_zero_write(x) {
            Ok(0) => Ok(()),
            Ok(_) => {
                self.validate_shape()?;
                Ok(())
            }
            Err(e) => Err(e),
        }
    }
    pub fn s2_n14ok(&mut self, x: u128) -> V16Result<()> {
        match self.s2_zero_nowrite(x) {
            Ok(0) => Ok(()),
            Ok(_) => {
                self.validate_shape()?;
                Ok(())
            }
            Err(e) => Err(e),
        }
    }
    pub fn s2_n16(&mut self, x: u128) -> V16Result<u128> {
        let c = self.s2_wrap_bad(x)?;
        if c != 0 {
            self.validate_shape()?;
        }
        Ok(c)
    }
    pub fn s2_n16ok(&mut self, x: u128) -> V16Result<u128> {
        let c = self.s2_wrap_good(x)?;
        if c != 0 {
            self.validate_shape()?;
        }
        Ok(c)
    }
    pub fn s2_n17(&mut self, x: u128) -> V16Result<u128> {
        let c = self.s2_const_after_write(x)?;
        if c != 0 {
            self.validate_shape()?;
        }
        Ok(c)
    }
    pub fn s2_n18(&mut self, x: u128) -> V16Result<u128> {
        let c = self.s2_mut_payload(x)?;
        if c != 0 {
            self.validate_shape()?;
        }
        Ok(c)
    }
    pub fn s2_n19(&mut self, x: u128) -> V16Result<u64> {
        let c = self.s2_trunc(x)?;
        if c != 0 {
            self.validate_shape()?;
        }
        Ok(c)
    }
    pub fn s2_n20(&mut self, x: u128) -> V16Result<u128> {
        let c = self.s2_arith(x)?;
        if c != 0 {
            self.validate_shape()?;
        }
        Ok(c)
    }
    pub fn s2_n21(&mut self, x: u128) -> V16Result<()> {
        if self.s2_bool(x)? {
            self.validate_shape()?;
        }
        Ok(())
    }
    pub fn s2_n22(&mut self, x: u128) -> V16Result<u128> {
        let mut last: V16Result<u128> = Ok(0);
        let mut i = 0u8;
        while i < 2 {
            last = if i == 0 {
                self.s2_zero_nowrite(0)
            } else {
                self.s2_zero_write(x)
            };
            i += 1;
        }
        let c = last?;
        if c != 0 {
            self.validate_shape()?;
        }
        Ok(c)
    }
    pub fn s2_n23(&mut self, x: u128) -> V16Result<u128> {
        let mut r = self.s2_zero_nowrite(x);
        Self::s2_poke(&mut r);
        let c = r?;
        if c != 0 {
            self.validate_shape()?;
        }
        Ok(c)
    }
    pub fn s2_n24(&mut self, x: u128) -> V16Result<()> {
        if let Some(_) = self.s2_opt_var(x)? {
            self.validate_shape()?;
        }
        Ok(())
    }
    pub fn s2_n25(&mut self, x: u128) -> V16Result<u128> {
        let c = self.s2_zero_nowrite(x)?;
        let d = self.s2_zero_write(x)?;
        if c != 0 {
            self.validate_shape()?;
        }
        Ok(d)
    }
    pub fn s2_n26(&mut self, x: u128) -> V16Result<u128> {
        let c = self.s2_zero_nowrite(x)?;
        if c > 1 {
            self.validate_shape()?;
        }
        Ok(c)
    }
    pub fn s2_n27(&mut self, x: u128) -> V16Result<u128> {
        let r = self.s2_zero_nowrite(x);
        let c = match r {
            Ok(v) => v,
            Err(_) => 0,
        };
        if c != 0 {
            self.validate_shape()?;
        }
        Ok(c)
    }
