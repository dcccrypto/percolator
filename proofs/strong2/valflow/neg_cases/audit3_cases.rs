    // ---- audit3 helpers
    #[inline(never)]
    fn a3_w(&mut self) {
        self.header.asset_slot_capacity.bytes = [0xff; 4];
    }
    #[inline(never)]
    fn a3_zn(&mut self, x: u128) -> V16Result<u128> {
        if x == 0 {
            return Ok(0);
        }
        self.a3_w();
        Ok(x)
    }
    #[inline(never)]
    fn a3_zw(&mut self, x: u128) -> V16Result<u128> {
        self.a3_w();
        Ok(x)
    }
    #[inline(never)]
    fn a3_optw(&mut self, x: u128) -> V16Result<Option<u8>> {
        if x == 0 {
            return Ok(None);
        }
        self.a3_w();
        Ok(Some(1))
    }
    #[inline(never)]
    fn a3_ru(&mut self, x: u128) -> Result<u128, u128> {
        if x == 0 {
            return Ok(0);
        }
        self.a3_w();
        Err(0)
    }
    #[inline(never)]
    fn a3_tup(&mut self, x: u128) -> V16Result<(u128, u128)> {
        if x == 0 {
            return Ok((0, 0));
        }
        self.a3_w();
        Ok((0, x))
    }
    #[inline(never)]
    fn a3_nest(&mut self, x: u128) -> V16Result<V16Result<u128>> {
        if x == 0 {
            return Ok(Err(V16Error::InvalidConfig));
        }
        self.a3_w();
        Ok(Ok(1))
    }
    #[inline(never)]
    fn a3_cf(&mut self, x: u128) -> core::ops::ControlFlow<u128, u128> {
        if x == 0 {
            return core::ops::ControlFlow::Continue(0);
        }
        self.a3_w();
        core::ops::ControlFlow::Break(0)
    }
    #[inline(never)]
    fn a3_rec(&mut self, x: u128) -> V16Result<u128> {
        if x == 0 {
            return Ok(0);
        }
        self.a3_w();
        self.a3_rec(x - 1)
    }
    #[inline(never)]
    fn a3_mut_a(&mut self, x: u128) -> V16Result<u128> {
        if x == 0 {
            return Ok(0);
        }
        self.a3_mut_b(x)
    }
    #[inline(never)]
    fn a3_mut_b(&mut self, x: u128) -> V16Result<u128> {
        self.a3_w();
        self.a3_mut_a(x - 1)
    }
    #[inline(never)]
    fn a3_loopw(&mut self, x: u128) -> V16Result<u128> {
        let mut i = 0u128;
        let mut acc = 0u128;
        while i < x {
            if i == 1 {
                self.a3_w();
            }
            acc = 0;
            i += 1;
        }
        Ok(acc)
    }
    #[inline(never)]
    fn a3_i64(&mut self, x: u128) -> V16Result<i64> {
        if x == 0 {
            return Ok(0);
        }
        self.a3_w();
        Ok(-1)
    }
    #[inline(never)]
    fn a3_big(&mut self, x: u128) -> V16Result<u128> {
        if x == 0 {
            return Ok(0);
        }
        self.a3_w();
        Ok(1u128 << 100)
    }
    #[inline(never)]
    fn a3_boolr(&mut self, x: u128) -> V16Result<bool> {
        if x == 0 {
            return Ok(false);
        }
        self.a3_w();
        Ok(true)
    }
    #[inline(never)]
    fn a3_two(&mut self, o: &mut MarketGroupV16ViewMut<'_, T>, x: u128) -> V16Result<u128> {
        if x == 0 {
            return Ok(0);
        }
        o.a3_w();
        self.validate_shape()?;
        Ok(1)
    }
    #[inline(never)]
    fn a3_acct2(&mut self, a: &mut PortfolioV16ViewMut<'_>, b: &mut PortfolioV16ViewMut<'_>) -> V16Result<()> {
        let _ = b;
        a.validate_with_market(&self.as_view())?;
        Ok(())
    }
    #[inline(never)]
    fn a3_swapres(a: &mut V16Result<u128>, b: &mut V16Result<u128>) {
        core::mem::swap(a, b);
    }
    #[inline(never)]
    fn a3_cast_in_callee(&mut self, x: u128) -> V16Result<u64> {
        let v = match self.a3_zn(x) {
            Ok(v) => v as u64,
            Err(e) => return Err(e),
        };
        Ok(v)
    }
    #[inline(never)]
    fn a3_some_after_none(&mut self, x: u128) -> V16Result<Option<u128>> {
        let mut o: Option<u128> = None;
        if x != 0 {
            self.a3_w();
            o = Some(x);
            if x == 9 {
                o = o.and(None);
            }
        }
        Ok(o)
    }
    #[inline(never)]
    fn a3_field_res(&mut self, x: u128) -> V16Result<u128> {
        let mut t = (Ok::<u128, V16Error>(0), 1u8);
        if x != 0 {
            self.a3_w();
            t.1 = 2;
        }
        t.0
    }
    #[inline(never)]
    fn a3_arr_res(&mut self, x: u128) -> V16Result<u128> {
        let mut arr: [V16Result<u128>; 2] = [Ok(0), Ok(0)];
        if x != 0 {
            self.a3_w();
            arr[1] = Ok(0);
        }
        arr[(x & 1) as usize]
    }

    // ---- audit3 attack cases
    pub fn a3_c01(&mut self, x: u128) -> V16Result<()> {
        let c = match self.a3_zn(x) {
            Ok(v) => v as u64,
            Err(e) => return Err(e),
        };
        if c != 0 {
            self.validate_shape()?;
        }
        Ok(())
    }
    pub fn a3_c02(&mut self, x: u128) -> V16Result<()> {
        let c = self.a3_zn(x)? as u64;
        if c != 0 {
            self.validate_shape()?;
        }
        Ok(())
    }
    pub fn a3_c03(&mut self, x: u128) -> V16Result<()> {
        if let Ok(v) = self.a3_zn(x) {
            if v as u8 != 0 {
                self.validate_shape()?;
            }
        }
        Ok(())
    }
    pub fn a3_c04(&mut self, x: u128) -> V16Result<()> {
        let c = match self.a3_ru(x) {
            Ok(v) => v,
            Err(v) => v,
        };
        if c != 0 {
            self.validate_shape()?;
        }
        Ok(())
    }
    pub fn a3_c04b(&mut self, x: u128) -> V16Result<()> {
        let (Ok(c) | Err(c)) = self.a3_ru(x);
        if c != 0 {
            self.validate_shape()?;
        }
        Ok(())
    }
    pub fn a3_c05(&mut self, x: u128) -> V16Result<()> {
        let mut r = self.a3_zn(0);
        let mut r2 = self.a3_zw(x);
        Self::a3_swapres(&mut r, &mut r2);
        let c = r2?;
        let _ = r;
        if c != 0 {
            self.validate_shape()?;
        }
        Ok(())
    }
    pub fn a3_c05b(&mut self, x: u128) -> V16Result<()> {
        let mut r = self.a3_zn(0);
        let w = self.a3_zw(x);
        let old = core::mem::replace(&mut r, w);
        let c = old?;
        if c != 0 {
            self.validate_shape()?;
        }
        let _ = r;
        Ok(())
    }
    pub fn a3_c06(&mut self, x: u128) -> V16Result<()> {
        let mut t = (self.a3_zn(0), 1u8);
        if x != 0 {
            t.0 = self.a3_zw(0);
        }
        let c = t.0?;
        if c != 0 {
            self.validate_shape()?;
        }
        Ok(())
    }
    pub fn a3_c06b(&mut self, x: u128) -> V16Result<()> {
        let c = self.a3_field_res(x)?;
        if c != 0 {
            self.validate_shape()?;
        }
        Ok(())
    }
    pub fn a3_c06c(&mut self, x: u128) -> V16Result<()> {
        let c = self.a3_arr_res(x)?;
        if c != 0 {
            self.validate_shape()?;
        }
        Ok(())
    }
    pub fn a3_c07(&mut self, x: u128) -> V16Result<()> {
        let r = loop {
            let r = self.a3_zn(0);
            if x == 0 {
                break r;
            }
            let r = self.a3_zw(0);
            break r;
        };
        let c = r?;
        if c != 0 {
            self.validate_shape()?;
        }
        Ok(())
    }
    pub fn a3_c08(&mut self, x: u128) -> V16Result<()> {
        let mut o = Some(self.a3_zn(0));
        if x != 0 {
            o = Some(self.a3_zw(0));
        }
        if let Some(r) = o {
            let c = r?;
            if c != 0 {
                self.validate_shape()?;
            }
        }
        Ok(())
    }
    pub fn a3_c09(&mut self, x: u128) -> V16Result<()> {
        let r = self.a3_zn(0);
        let w = self.a3_zw(x);
        let rr = if x > 3 { &w } else { &r };
        if let Ok(v) = rr {
            if *v != 0 {
                self.validate_shape()?;
            }
        }
        Ok(())
    }
    pub fn a3_c09b(&mut self, x: u128) -> V16Result<()> {
        let r = self.a3_zw(x);
        let c = r.as_ref().map(|v| *v).unwrap_or(0);
        if c != 0 {
            self.validate_shape()?;
        }
        Ok(())
    }
    pub fn a3_c09c(&mut self, x: u128) -> V16Result<()> {
        let c = self.a3_zw(x).map(|v| v & 0).and_then(|v| Ok::<u128, V16Error>(v))?;
        if c != 0 {
            self.validate_shape()?;
        }
        Ok(())
    }
    pub fn a3_c10(&mut self, x: u128) -> V16Result<()> {
        let c = self.a3_zn(x)?;
        let mut d = c;
        d &= 0;
        let e = d;
        if e != 0 {
            self.validate_shape()?;
        }
        Ok(())
    }
    pub fn a3_c11(&mut self, x: u128) -> V16Result<()> {
        let c = self.a3_i64(x)?;
        if c > 0 {
            self.validate_shape()?;
        }
        Ok(())
    }
    pub fn a3_c11b(&mut self, x: u128) -> V16Result<()> {
        let c = self.a3_i64(x)?;
        if c as u8 as i8 as i64 + 1 != 1 && c != -1 {
            self.validate_shape()?;
        }
        Ok(())
    }
    pub fn a3_c12(&mut self, x: u128) -> V16Result<()> {
        let c = self.a3_big(x)?;
        if (c as u64) != 0 {
            self.validate_shape()?;
        }
        Ok(())
    }
    pub fn a3_c12b(&mut self, x: u128) -> V16Result<()> {
        let c = self.a3_big(x)?;
        if c == 1u128 << 99 {
            return Ok(());
        }
        if c != 0 {
            return Ok(());
        }
        Ok(())
    }
    pub fn a3_c13(&mut self, x: u128) -> V16Result<()> {
        let c = self.a3_zn(x)?;
        let b = (c & 1) != 0;
        if b {
            self.validate_shape()?;
        }
        Ok(())
    }
    pub fn a3_c13b(&mut self, x: u128) -> V16Result<()> {
        let c = self.a3_zn(x)?;
        let b = !(c == 0) & (x == 5);
        if b {
            self.validate_shape()?;
        }
        Ok(())
    }
    pub fn a3_c14(&mut self, x: u128) -> V16Result<()> {
        let t = (self.a3_optw(x)?, 0u8);
        if t.0.is_none() {
            self.validate_shape()?;
        }
        Ok(())
    }
    pub fn a3_c14b(&mut self, x: u128, flag: bool) -> V16Result<()> {
        let mut o = self.a3_optw(x)?;
        if flag {
            o = None;
        }
        if o.is_some() {
            self.validate_shape()?;
        }
        Ok(())
    }
    pub fn a3_c14c(&mut self, x: u128) -> V16Result<()> {
        let o = self.a3_optw(x)?;
        let p = o.and(None::<u8>);
        if p.is_some() {
            self.validate_shape()?;
        }
        Ok(())
    }
    pub fn a3_c14d(&mut self, x: u128) -> V16Result<()> {
        let o = self.a3_optw(x)?;
        let p: Option<u8> = None;
        let q = if x > 100 { &o } else { &p };
        if q.is_some() {
            self.validate_shape()?;
        }
        Ok(())
    }
    pub fn a3_c14e(&mut self, x: u128) -> V16Result<()> {
        let mut o = self.a3_optw(x)?;
        let mut i = 0;
        while i < 2 {
            if o.is_some() {
                self.validate_shape()?;
                return Ok(());
            }
            o = self.a3_optw(0)?;
            i += 1;
        }
        Ok(())
    }
    pub fn a3_c14f(&mut self, x: u128) -> V16Result<()> {
        let o = self.a3_some_after_none(x)?;
        if o.is_some() {
            self.validate_shape()?;
        }
        Ok(())
    }
    pub fn a3_c14g(&mut self, x: u128) -> V16Result<()> {
        let oo = Some(self.a3_optw(x)?);
        if let Some(None) = oo {
            return Ok(());
        }
        self.validate_shape()?;
        Ok(())
    }
    pub fn a3_c14h(&mut self, x: u128) -> V16Result<()> {
        let mut o = self.a3_optw(x)?;
        let t = o.take();
        let _ = t;
        if o.is_some() {
            self.validate_shape()?;
        }
        Ok(())
    }
    pub fn a3_c15(&mut self, x: u128) -> V16Result<()> {
        let c = self.a3_loopw(x)?;
        if c != 0 {
            self.validate_shape()?;
        }
        Ok(())
    }
    pub fn a3_c17(&mut self, x: u128) -> V16Result<()> {
        let c = self.a3_rec(x)?;
        if c != 0 {
            self.validate_shape()?;
        }
        Ok(())
    }
    pub fn a3_c17b(&mut self, x: u128) -> V16Result<()> {
        let c = self.a3_mut_a(x)?;
        if c != 0 {
            self.validate_shape()?;
        }
        Ok(())
    }
    pub fn a3_c17c(&mut self, x: u128) -> V16Result<()> {
        let c = self.a3_mut_b(x)?;
        if c != 0 {
            self.validate_shape()?;
        }
        Ok(())
    }
    pub fn a3_c18(&mut self, other: &mut MarketGroupV16ViewMut<'_, T>, x: u128) -> V16Result<()> {
        let c = other.a3_two(self, x)?;
        if c != 0 {
            return Ok(());
        }
        Ok(())
    }
    pub fn a3_c18b(&mut self, other: &mut MarketGroupV16ViewMut<'_, T>, x: u128) -> V16Result<()> {
        let c = other.a3_zn(x)?;
        self.a3_w();
        if c != 0 {
            self.validate_shape()?;
        }
        Ok(())
    }
    pub fn a3_c18c(&mut self, other: &mut MarketGroupV16ViewMut<'_, T>, x: u128) -> V16Result<()> {
        self.a3_w();
        other.validate_shape()?;
        let _ = x;
        Ok(())
    }
    pub fn a3_c19(&mut self, a: &mut PortfolioV16ViewMut<'_>, b: &mut PortfolioV16ViewMut<'_>) -> V16Result<()> {
        self.a3_acct2(b, a)?;
        Ok(())
    }
    pub fn a3_c21(&mut self, x: u128) -> V16Result<()> {
        let f: fn(&mut Self, u128) -> V16Result<u128> = if x == 3 { Self::a3_zn } else { Self::a3_zw };
        let c = f(self, x)?;
        if c != 0 {
            self.validate_shape()?;
        }
        Ok(())
    }
    pub fn a3_c22(&mut self, x: u128) -> V16Result<()> {
        let (a, _b) = self.a3_tup(x)?;
        if a != 0 {
            self.validate_shape()?;
        }
        Ok(())
    }
    pub fn a3_c23(&mut self, x: u128) -> V16Result<()> {
        let c = self.a3_nest(x)??;
        if c != 1 {
            self.validate_shape()?;
        }
        Ok(())
    }
    pub fn a3_c23b(&mut self, x: u128) -> V16Result<()> {
        let inner = self.a3_nest(x)?;
        if let Ok(v) = inner {
            if v != 1 {
                self.validate_shape()?;
            }
        }
        Ok(())
    }
    pub fn a3_c25(&mut self, x: u128) -> V16Result<()> {
        match self.a3_cf(x) {
            core::ops::ControlFlow::Continue(v) => {
                if v != 0 {
                    self.validate_shape()?;
                }
            }
            core::ops::ControlFlow::Break(_) => {}
        }
        Ok(())
    }
    pub fn a3_c25b(&mut self, x: u128) -> V16Result<()> {
        let v = self.a3_cf(x).continue_value().unwrap_or(0);
        if v != 0 {
            self.validate_shape()?;
        }
        Ok(())
    }
    pub fn a3_c29(&mut self, x: u128) -> V16Result<()> {
        let c = self.a3_zw(x)?;
        match c as usize {
            0 => {}
            _ => {
                self.validate_shape()?;
            }
        }
        Ok(())
    }
    pub fn a3_c30(&mut self, x: u128) -> V16Result<()> {
        let c = self.a3_zn(x)?;
        match c {
            v if v == 0 || v == 7 => {}
            0..=6 => {}
            _ => {
                self.validate_shape()?;
            }
        }
        Ok(())
    }
    pub fn a3_c31(&mut self, x: u128) -> V16Result<()> {
        if !self.a3_boolr(x)? {
            return Ok(());
        }
        self.validate_shape()?;
        Ok(())
    }
    pub fn a3_c31b(&mut self, x: u128) -> V16Result<()> {
        if self.a3_boolr(x)? {
            return Ok(());
        }
        self.validate_shape()?;
        Ok(())
    }
    pub fn a3_c31c(&mut self, x: u128) -> V16Result<()> {
        let b = self.a3_boolr(x)?;
        if b != true {
            self.validate_shape()?;
        }
        Ok(())
    }
    pub fn a3_c33(&mut self, x: u128, y: u128) -> V16Result<()> {
        let mut c = self.a3_zn(x)?;
        let d = c;
        c = y;
        let e = c;
        c = d;
        if e != 0 {
            self.validate_shape()?;
        }
        let _ = c;
        Ok(())
    }
    pub fn a3_c33b(&mut self, x: u128, y: u128) -> V16Result<()> {
        let c = self.a3_zn(x)?;
        let (mut p, mut q) = (c, y);
        core::mem::swap(&mut p, &mut q);
        if p != 0 {
            self.validate_shape()?;
        }
        let _ = q;
        Ok(())
    }
    pub fn a3_c34(&mut self, x: u128) -> V16Result<()> {
        let c = self.a3_cast_in_callee(x)?;
        if c != 0 {
            self.validate_shape()?;
        }
        Ok(())
    }
    pub fn a3_c35(&mut self, x: u128) -> V16Result<()> {
        let mut r = self.a3_zn(0);
        let mut k = 0u8;
        let mut cl = |m: &mut Self| {
            r = m.a3_zw(x);
            k += 1;
        };
        cl(self);
        let c = r?;
        if c != 0 {
            self.validate_shape()?;
        }
        let _ = k;
        Ok(())
    }
    pub fn a3_c36(&mut self, x: u128) -> V16Result<()> {
        let r = self.a3_zn(x);
        if let Ok(0) = r {
            return Ok(());
        }
        let r2 = self.a3_zw(x);
        if let Ok(1) = r2 {
            return Ok(());
        }
        self.validate_shape()?;
        Ok(())
    }
    pub fn a3_c37(&mut self, x: u128) -> V16Result<()> {
        let c = self.a3_zn(x)?;
        let z = if c == 0 { 1u8 } else { 0u8 };
        self.a3_w();
        if z == 0 {
            self.validate_shape()?;
        }
        Ok(())
    }
    pub fn a3_c38(&mut self, x: u128) -> V16Result<()> {
        let c = self.a3_zn(x)?;
        let s = c as i128;
        if s != 0 {
            self.validate_shape()?;
        } else if x != 0 {
            self.a3_w();
        }
        Ok(())
    }
    pub fn a3_d01(&mut self, x: u128) -> V16Result<()> {
        let c = match self.a3_zn(x) {
            Ok(ref v) => *v as u64,
            Err(e) => return Err(e),
        };
        if c != 0 {
            self.validate_shape()?;
        }
        Ok(())
    }
    pub fn a3_d02(&mut self, x: u128) -> V16Result<()> {
        let r = self.a3_zn(x);
        let c = match &r {
            Ok(v) => *v as u64,
            Err(_) => return Err(V16Error::InvalidConfig),
        };
        if c != 0 {
            self.validate_shape()?;
        }
        Ok(())
    }
    pub fn a3_d03(&mut self, x: u128) -> V16Result<()> {
        let r = self.a3_zn(x);
        if let Ok(ref v) = r {
            let c = *v as u8;
            if c != 0 {
                self.validate_shape()?;
            }
        }
        Ok(())
    }
    pub fn a3_d04(&mut self, x: u128) -> V16Result<()> {
        let r = self.a3_zn(x);
        let rr = &r;
        let c: u64 = match *rr {
            Ok(v) => v as u64,
            Err(_) => return Err(V16Error::InvalidConfig),
        };
        if c != 0 {
            self.validate_shape()?;
        }
        Ok(())
    }
    pub fn a3_d05(&mut self, x: u128) -> V16Result<()> {
        let r = self.a3_zn(x);
        let c = match r {
            Ok(v) => (v as u64, 0u8).0,
            Err(_) => return Err(V16Error::InvalidConfig),
        };
        if c != 0 {
            self.validate_shape()?;
        }
        Ok(())
    }
    pub fn a3_d08(&mut self, x: u128) -> V16Result<()> {
        let mut r = self.a3_zn(x);
        {
            let p = &mut r;
            *p = Ok(0);
        }
        let c = r?;
        if c != 0 {
            self.validate_shape()?;
        }
        Ok(())
    }
    pub fn a3_d09(&mut self, x: u128) -> V16Result<()> {
        let mut r = self.a3_zn(x);
        if let Ok(ref mut v) = r {
            *v = 0;
        }
        let c = r?;
        if c != 0 {
            self.validate_shape()?;
        }
        Ok(())
    }
    pub fn a3_d09b(&mut self, x: u128) -> V16Result<()> {
        let mut r = self.a3_zn(x);
        match r {
            Ok(ref mut v) => *v &= 0,
            Err(_) => {}
        }
        let c = r?;
        if c != 0 {
            self.validate_shape()?;
        }
        Ok(())
    }
    pub fn a3_d10(&mut self, x: u128) -> V16Result<()> {
        let mut o = self.a3_optw(x)?;
        if let Some(ref mut v) = o {
            *v = 0;
        }
        {
            let p = &mut o;
            *p = None;
        }
        if o.is_some() {
            self.validate_shape()?;
        }
        Ok(())
    }
    pub fn a3_d11(&mut self, x: u128) -> V16Result<()> {
        let c = a3_free_w0(self, x)?;
        if c != 0 {
            self.validate_shape()?;
        }
        Ok(())
    }
    pub fn a3_d12(&mut self, x: u128) -> V16Result<()> {
        let c = <Self as A3Tr>::a3_zn(self, x)?;
        if c != 0 {
            self.validate_shape()?;
        }
        Ok(())
    }
    pub fn a3_d13(&mut self, x: u128) -> V16Result<()> {
        let c = A3Wrap { m: self }.go(x)?;
        if c != 0 {
            self.validate_shape()?;
        }
        Ok(())
    }
    pub fn a3_d14(&mut self, x: u128) -> V16Result<()> {
        let r = self.a3_zn(x);
        let c = r?;
        let r2: V16Result<u128> = Ok(c);
        let r3 = if x == 77 { self.a3_zw(0) } else { r2 };
        let d = r3?;
        if d != 0 {
            self.validate_shape()?;
        }
        Ok(())
    }
    pub fn a3_d15(&mut self, x: u128) -> V16Result<u128> {
        let r = self.a3_zw(x & 0);
        if x == 5 {
            return r;
        }
        let c = r?;
        if c != 0 {
            self.validate_shape()?;
        }
        self.validate_shape()?;
        Ok(c)
    }
    pub fn a3_d16(&mut self, x: u128) -> V16Result<()> {
        let c = self.a3_zn(x)?;
        let k: u128 = 4611686018427387904;
        if c == k {
            return Ok(());
        }
        if c != 0 {
            self.validate_shape()?;
        } else if c == k + k + k + k {
            self.a3_w();
        }
        if x == 4611686018427387904 { self.a3_w(); }
        Ok(())
    }
    pub fn a3_d17(&mut self, x: u128) -> V16Result<()> {
        let c = self.a3_zn(x)?;
        let s = c as i8;
        if s == -1 {
            return Ok(());
        }
        self.validate_shape()?;
        Ok(())
    }
    pub fn a3_d18(&mut self, x: u128) -> V16Result<()> {
        let c = self.a3_i64(x)?;
        match c {
            -1 => {}
            0 => {}
            _ => {
                self.validate_shape()?;
            }
        }
        Ok(())
    }
    pub fn a3_d19(&mut self, x: u128) -> V16Result<()> {
        let c = self.a3_zn(x)?;
        let ch = if c == 0 { 'a' } else { 'b' };
        let ch2 = if x > 9 { 'a' } else { ch };
        if ch2 != 'a' {
            self.validate_shape()?;
        }
        Ok(())
    }
    pub fn a3_d20(&mut self, x: u128) -> V16Result<()> {
        let mut c = self.a3_zn(x)?;
        let mut i = 0u8;
        while i < 3 {
            if c == 0 {
                return Ok(());
            }
            c = c >> 1;
            i += 1;
        }
        self.validate_shape()?;
        Ok(())
    }
    pub fn a3_d21(&mut self, x: u128) -> V16Result<()> {
        let c = self.a3_zn(x)?;
        let d = self.a3_zw(c)?;
        if d != 0 {
            self.validate_shape()?;
        }
        Ok(())
    }
    pub fn a3_d22(&mut self, x: u128) -> V16Result<()> {
        let a = self.a3_zn(x);
        let b = self.a3_zw(0);
        let c = match (a, b) {
            (Ok(p), Ok(_q)) => p,
            _ => return Err(V16Error::InvalidConfig),
        };
        let _ = c;
        if let Ok(0) = self.a3_zn(0) {
            return Ok(());
        }
        self.validate_shape()?;
        Ok(())
    }
    pub fn a3_d23(&mut self, x: u128) -> V16Result<()> {
        let b = self.a3_zw(x);
        let a = self.a3_zn(0);
        let (p, q) = (a, b);
        let c = q?;
        let d = p?;
        if c != 0 {
            self.validate_shape()?;
        }
        let _ = d;
        Ok(())
    }
    pub fn a3_d24(&mut self, x: u128) -> V16Result<()> {
        let o = self.a3_optw(x)?;
        let o2: Option<u8> = None;
        let (mut p, mut q) = (o, o2);
        if x > 1 {
            core::mem::swap(&mut p, &mut q);
        }
        if p.is_some() || q.is_some() && x == 1 {
            self.validate_shape()?;
        }
        Ok(())
    }
    pub fn a3_d25(&mut self, x: u128) -> V16Result<()> {
        let o = self.a3_optw(x)?;
        let b = o.is_none();
        let nb = !b;
        let bb = if x == 3 { true } else { b };
        if nb & !bb {
            self.validate_shape()?;
        }
        Ok(())
    }
    pub fn a3_d26(&mut self, x: u128) -> V16Result<()> {
        let o = self.a3_optw(x)?;
        let r = &o;
        let b = r.is_none();
        self.a3_w();
        if !b {
            self.validate_shape()?;
        }
        Ok(())
    }
    pub fn a3_d27(&mut self, x: u128) -> V16Result<()> {
        let o = self.a3_optw(x)?;
        let n = o.is_none() as u8;
        let n2 = n.wrapping_add((x == 2) as u8);
        if n2 == 0 {
            self.validate_shape()?;
        }
        Ok(())
    }
    #[inline(never)]
    fn a3_shr(&mut self, x: u128) -> V16Result<u128> {
        let mut y = x;
        if y == 1 {
            return Ok(1);
        }
        self.a3_w();
        y >>= 1;
        Ok(y)
    }
    #[inline(never)]
    fn a3_bumpit(y: &mut u128) {
        *y = 1;
    }
    #[inline(never)]
    fn a3_bump(&mut self, x: u128) -> V16Result<u128> {
        let mut y = x;
        if y == 1 {
            return Ok(y);
        }
        self.a3_w();
        Self::a3_bumpit(&mut y);
        Ok(y)
    }
    #[inline(never)]
    fn a3_argtest(&mut self, x: u128) -> V16Result<u128> {
        if x == 1 {
            return Ok(x);
        }
        self.a3_w();
        Ok(x)
    }
    #[inline(never)]
    fn a3_countdown(&mut self, x: u128) -> V16Result<u128> {
        let mut y = x;
        loop {
            if y == 0 {
                return Ok(y);
            }
            self.a3_w();
            y -= 1;
        }
    }
    #[inline(never)]
    fn a3_xor(&mut self, x: u128) -> V16Result<u128> {
        let y = x;
        let z = y;
        if z == 0 {
            return Ok(0);
        }
        self.a3_w();
        Ok(y ^ y)
    }
    #[inline(never)]
    fn a3_optpay(&mut self, x: u128) -> V16Result<Option<u128>> {
        if x == 0 {
            return Ok(Some(0));
        }
        self.a3_w();
        Ok(None)
    }
    #[inline(never)]
    fn a3_plain(&mut self, x: u128) -> u128 {
        if x == 0 {
            return 0;
        }
        self.a3_w();
        x
    }
    #[inline(never)]
    fn a3_plain_shr(&mut self, x: u128) -> u128 {
        if x == 0 {
            return 0;
        }
        self.a3_w();
        x >> 1
    }
    #[inline(never)]
    fn a3_argswap(&mut self, x: u128, y: u128) -> V16Result<u128> {
        if x == 0 {
            return Ok(x);
        }
        self.a3_w();
        Ok(y)
    }
    #[inline(never)]
    fn a3_sel(&mut self, x: u128, y: u128) -> V16Result<u128> {
        let (a, b) = if x == 0 { (x, y) } else { (y, x) };
        if a != 0 {
            let _ = b;
            return Ok(0);
        }
        self.a3_w();
        Ok(b)
    }
    pub fn a3_e01(&mut self, x: u128) -> V16Result<()> {
        let c = self.a3_shr(x)?;
        if c != 1 {
            self.validate_shape()?;
        }
        Ok(())
    }
    pub fn a3_e02(&mut self, x: u128) -> V16Result<()> {
        let c = self.a3_bump(x)?;
        if c != 1 {
            self.validate_shape()?;
        }
        Ok(())
    }
    pub fn a3_e03(&mut self, x: u128) -> V16Result<()> {
        let c = self.a3_argtest(x)?;
        if c != 1 {
            self.validate_shape()?;
        }
        Ok(())
    }
    pub fn a3_e04(&mut self, x: u128) -> V16Result<()> {
        let c = self.a3_countdown(x)?;
        if c != 0 {
            self.validate_shape()?;
        }
        Ok(())
    }
    pub fn a3_e05(&mut self, x: u128) -> V16Result<()> {
        let c = self.a3_xor(x)?;
        if c != 0 {
            self.validate_shape()?;
        }
        Ok(())
    }
    pub fn a3_e06(&mut self, x: u128) -> V16Result<()> {
        let o = self.a3_optpay(x)?;
        if let Some(v) = o {
            if v != 0 {
                self.validate_shape()?;
            }
        }
        Ok(())
    }
    pub fn a3_e06b(&mut self, x: u128) -> V16Result<()> {
        let o = self.a3_optpay(x)?;
        if o.is_none() {
            self.validate_shape()?;
        }
        Ok(())
    }
    pub fn a3_e07(&mut self, x: u128) -> V16Result<()> {
        let c = self.a3_plain(x);
        if c != 0 {
            self.validate_shape()?;
        }
        Ok(())
    }
    pub fn a3_e07b(&mut self, x: u128) -> V16Result<()> {
        let c = self.a3_plain_shr(x);
        if c != 0 {
            self.validate_shape()?;
        }
        Ok(())
    }
    pub fn a3_e08(&mut self, x: u128, y: u128) -> V16Result<()> {
        let c = self.a3_argswap(x, y)?;
        if c != 0 {
            self.validate_shape()?;
        }
        Ok(())
    }
    pub fn a3_e09(&mut self, x: u128, y: u128) -> V16Result<()> {
        let c = self.a3_sel(x, y)?;
        if c != 0 {
            self.validate_shape()?;
        }
        Ok(())
    }
    pub fn a3_e10(&mut self, x: u128) -> V16Result<()> {
        let c = self.a3_zn(x)?;
        let d = self.a3_zn(0)?;
        let e = if x > 5 { d } else { c };
        if e != 0 {
            self.validate_shape()?;
        }
        Ok(())
    }
    pub fn a3_e11(&mut self, x: u128) -> V16Result<()> {
        let c = self.a3_zn(x)?;
        let z = self.a3_zn(0)?;
        let c2 = self.a3_zw(z)?;
        if c != 0 {
            self.validate_shape()?;
        } else if c2 != 0 {
            self.validate_shape()?;
        }
        Ok(())
    }
    #[inline(never)]
    fn a3_zero_or_two(&mut self, x: u128) -> V16Result<u128> {
        if x == 0 {
            return Ok(0);
        }
        self.a3_w();
        Ok(2)
    }
    #[inline(never)]
    fn a3_maybe_one(&mut self, x: u128) -> V16Result<u128> {
        if x == 0 {
            return Ok(0);
        }
        self.a3_w();
        let k = if x > 7 { 1u128 } else { 2u128 };
        Ok(k)
    }
    #[inline(never)]
    fn a3_poke0(r: &mut V16Result<u128>) {
        *r = Ok(0);
    }
    #[inline(never)]
    fn a3_nrvo(&mut self, x: u128) -> V16Result<u128> {
        let mut r: V16Result<u128> = Ok(5);
        if x == 0 {
            return Ok(0);
        }
        self.a3_w();
        Self::a3_poke0(&mut r);
        r
    }
    #[inline(never)]
    fn a3_swallow(&mut self, x: u128) -> V16Result<u128> {
        if x == 0 {
            return Ok(0);
        }
        self.a3_w();
        match self.validate_shape() {
            Ok(()) => Ok(1),
            Err(_) => Ok(0),
        }
    }
    pub fn a3_f06(&mut self, x: u128) -> V16Result<()> {
        let c = self.a3_zero_or_two(x)?;
        match c {
            0 | 2 => {}
            _ => {
                self.validate_shape()?;
            }
        }
        Ok(())
    }
    pub fn a3_f07(&mut self, x: u128) -> V16Result<()> {
        let c = self.a3_zero_or_two(x)?;
        if c != 0 {
            self.validate_shape()?;
        }
        if c == 1 {
            self.a3_w();
        }
        Ok(())
    }
    pub fn a3_f07b(&mut self, x: u128) -> V16Result<()> {
        let c = self.a3_maybe_one(x)?;
        if c != 0 {
            self.validate_shape()?;
        }
        if c == 1 {
            self.a3_w();
        }
        Ok(())
    }
    pub fn a3_f08(&mut self, x: u128) -> V16Result<()> {
        let c = self.a3_nrvo(x)?;
        if c != 0 {
            self.validate_shape()?;
        }
        Ok(())
    }
    pub fn a3_f10(&mut self, x: u128) -> V16Result<()> {
        let r = self.a3_zw(x);
        if r.is_ok() {
            return Ok(());
        }
        self.validate_shape()?;
        Ok(())
    }
    pub fn a3_f10b(&mut self, x: u128) -> V16Result<()> {
        let r = self.a3_zw(x);
        if matches!(r, Ok(0)) {
            return Ok(());
        }
        self.validate_shape()?;
        Ok(())
    }
    pub fn a3_f12(&mut self, x: u128) -> V16Result<()> {
        let c = self.a3_swallow(x)?;
        if c != 0 {
            return Ok(());
        }
        Ok(())
    }
    pub fn a3_f12b(&mut self, x: u128) -> V16Result<()> {
        let c = self.a3_swallow(x)?;
        if c == 1 {
            return Ok(());
        }
        Ok(())
    }
