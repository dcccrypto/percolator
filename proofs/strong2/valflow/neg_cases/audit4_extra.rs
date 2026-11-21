
// ---- audit round 4: top-level items for the R10 / R9 / read-set attacks
#[inline(never)]
pub fn a4_once<R, F: FnOnce() -> R>(f: F) -> R {
    f()
}
pub struct A4Hold<'x> {
    pub p: &'x mut V16PodU64,
    pub q: &'x mut V16PodU64,
}
pub struct A4HoldH<'x> {
    pub h: &'x mut MarketGroupV16HeaderAccount,
    pub n: u8,
}

// R9-1: explicit discriminants on tuple variants, written in parentheses. Real discriminants:
// Clean = 1, Wrote = 0. Declaration order (what enum_table would use): Clean 0, Wrote 1.
#[repr(u8)]
pub enum A4Disc {
    Clean(u8) = (1),
    Wrote(u8) = (0),
}

// R9-2: explicit discriminant after a multi-line tuple variant.
#[repr(u8)]
pub enum A4Multi {
    Clean(
        u8,
    ) = 1,
    Wrote(
        u8,
    ) = 0,
}

// R9-3: a commented-out decoy with the opposite order; the real declaration carries its
// attribute on the same line, so the name is counted once.
/*
pub enum A4Decoy {
    Clean,
    Wrote,
}
*/
#[derive(Clone, Copy)] pub enum A4Decoy { Wrote, Clean }

// R9 control: an honest plain enum (Clean 0, Wrote 1)
pub enum A4Plain {
    Clean,
    Wrote,
}

// R9-5: cfg'd-out decoy, real one declared with `pub(in crate)` visibility
#[cfg(any())]
pub enum A4Cfg {
    Clean,
    Wrote,
}
pub(in crate) enum A4Cfg {
    Wrote,
    Clean,
}

// ---- read-set probes: each reads header.mode (field 36, read by neither part of
// validate_shape) or a slot through a different path. Asked with readset.read_set(start=...).
pub trait A4Tr {
    fn a4_rd(&self) -> u8;
}
impl A4Tr for MarketGroupV16HeaderAccount {
    fn a4_rd(&self) -> u8 {
        self.mode
    }
}
pub struct A4W<'x> {
    pub h: &'x MarketGroupV16HeaderAccount,
}
impl<'x> A4W<'x> {
    #[inline(never)]
    pub fn get(&self) -> u8 {
        self.h.mode
    }
}
pub struct A4Sink;
impl core::fmt::Write for A4Sink {
    fn write_str(&mut self, _s: &str) -> core::fmt::Result {
        Ok(())
    }
}
#[inline(never)]
pub fn a4r_free(h: &MarketGroupV16HeaderAccount) -> u8 {
    h.mode
}
#[inline(never)]
pub fn a4r_gen<H: A4Tr>(h: &H) -> u8 {
    h.a4_rd()
}
#[inline(never)]
pub fn a4r_dup(_h: &MarketGroupV16HeaderAccount) -> u8 {
    0
}
pub mod a4m {
    #[inline(never)]
    pub fn a4r_dup(h: &super::MarketGroupV16HeaderAccount) -> u8 {
        h.mode
    }
    impl super::MarketGroupV16HeaderAccount {
        #[inline(never)]
        pub fn a4r_in(&self) -> u8 {
            self.mode
        }
    }
}
impl MarketGroupV16HeaderAccount {
    #[inline(never)]
    pub fn a4r_same(&self) -> u8 {
        self.mode
    }
}
impl V16PodU64 {
    #[inline(never)]
    pub fn a4r_same(&self) -> u8 {
        0
    }
}
impl<'a, T> MarketGroupV16View<'a, T> {
    #[inline(never)]
    fn a4r_h(&self) -> u8 {
        self.header.mode
    }
    #[inline(never)]
    fn a4r_bv(v: MarketGroupV16View<'_, T>) -> u8 {
        v.header.mode
    }
    #[inline(never)]
    fn a4r_sl(ms: &[Market<T>]) -> u32 {
        ms[0].engine.asset.market_id.get() as u32
    }
    pub fn a4r_01_direct(&self) -> u8 {
        self.header.mode
    }
    pub fn a4r_02_selfhelper(&self) -> u8 {
        self.a4r_h()
    }
    pub fn a4r_03_freefn(&self) -> u8 {
        a4r_free(self.header)
    }
    pub fn a4r_04_trait(&self) -> u8 {
        self.header.a4_rd()
    }
    pub fn a4r_05_closure_ref(&self) -> u8 {
        let f = || self.header.mode;
        f()
    }
    pub fn a4r_06_closure_move(&self) -> u8 {
        let h = self.header;
        let f = move || h.mode;
        f()
    }
    pub fn a4r_07_copy_header(&self) -> u8 {
        let c = *self.header;
        c.mode
    }
    pub fn a4r_08_copy_config(&self) -> u32 {
        let c = self.header.config;
        c.max_market_slots.get()
    }
    pub fn a4r_09_bytemuck(&self) -> u8 {
        bytemuck::bytes_of(self.header)[700]
    }
    pub fn a4r_10_partialeq(&self, o: &MarketGroupV16HeaderAccount) -> bool {
        *self.header == *o
    }
    pub fn a4r_11_debug(&self) -> bool {
        use core::fmt::Write;
        write!(A4Sink, "{:?}", self.header).is_ok()
    }
    pub fn a4r_12_fnptr(&self) -> u8 {
        let f: fn(&MarketGroupV16HeaderAccount) -> u8 = a4r_free;
        f(self.header)
    }
    pub fn a4r_13_generic(&self) -> u8 {
        a4r_gen(self.header)
    }
    pub fn a4r_14_option(&self) -> u8 {
        let o = Some(self.header);
        match o {
            Some(h) => h.mode,
            None => 0,
        }
    }
    pub fn a4r_15_tuple(&self) -> u8 {
        let t = (self.header, 1u8);
        t.0.mode
    }
    pub fn a4r_16_struct(&self) -> u8 {
        A4W { h: self.header }.get()
    }
    pub fn a4r_17_viewval(&self) -> u8 {
        let v = MarketGroupV16View { header: self.header, markets: self.markets };
        v.header.mode
    }
    pub fn a4r_18_pattern(&self) -> u8 {
        let &MarketGroupV16HeaderAccount { mode, .. } = self.header;
        mode
    }
    pub fn a4r_19_byvalue_view(&self) -> u8 {
        Self::a4r_bv(MarketGroupV16View { header: self.header, markets: self.markets })
    }
    pub fn a4r_20_iter(&self) -> u8 {
        core::iter::once(self.header).map(|h| h.mode).sum::<u8>()
    }
    pub fn a4r_21_array(&self) -> u8 {
        [self.header; 1][0].mode
    }
    pub fn a4r_22_samename(&self) -> u8 {
        self.header.a4r_same()
    }
    pub fn a4r_23_modfn(&self) -> u8 {
        a4m::a4r_dup(self.header)
    }
    pub fn a4r_24_modimpl(&self) -> u8 {
        self.header.a4r_in()
    }
    pub fn a4r_25_borrow(&self) -> u8 {
        let h: &MarketGroupV16HeaderAccount = core::borrow::Borrow::borrow(self.header);
        h.mode
    }
    pub fn a4r_26_refref(&self) -> u8 {
        let hh: &&MarketGroupV16HeaderAccount = &self.header;
        hh.mode
    }
    pub fn a4r_27_fieldref(&self) -> u8 {
        let m: &u8 = &self.header.mode;
        *m
    }
    pub fn a4r_28_clone(&self) -> u8 {
        let c = self.header.clone();
        c.mode
    }
    pub fn a4r_29_nested_closure(&self) -> u8 {
        let h = self.header;
        let f = move || {
            let g = move || h.mode;
            g()
        };
        f()
    }
    pub fn a4r_30_closure_arg(&self) -> u8 {
        let f = |h: &MarketGroupV16HeaderAccount| h.mode;
        f(self.header)
    }
    pub fn a4r_31_ptr_eq(&self, o: &MarketGroupV16HeaderAccount) -> bool {
        core::ptr::eq(self.header, o)
    }
    pub fn a4r_32_selfcopy(&self) -> u8 {
        let me: &Self = self;
        let me2 = me;
        me2.header.mode
    }
    // slots
    pub fn a4r_40_slot_direct(&self) -> u32 {
        self.markets[0].engine.asset.market_id.get() as u32
    }
    pub fn a4r_41_slot_helper(&self) -> u32 {
        Self::a4r_sl(self.markets)
    }
    pub fn a4r_42_slot_closure(&self) -> bool {
        (0..self.markets.len()).any(|i| self.markets[i].engine.asset.market_id.get() == 0)
    }
    pub fn a4r_43_slot_iter(&self) -> usize {
        self.markets.iter().filter(|m| m.engine.asset.market_id.get() == 0).count()
    }
    pub fn a4r_44_slot_first(&self) -> u32 {
        match self.markets.first() {
            Some(m) => m.engine.asset.market_id.get() as u32,
            None => 0,
        }
    }
    pub fn a4r_45_len_only(&self) -> usize {
        self.markets.len()
    }
    pub fn a4r_46_slot_pattern(&self) -> u32 {
        match self.markets {
            [m, ..] => m.engine.asset.market_id.get() as u32,
            [] => 0,
        }
    }
    pub fn a4r_47_slot_viewcopy(&self) -> u32 {
        let v = MarketGroupV16View { header: self.header, markets: self.markets };
        v.markets[0].engine.asset.market_id.get() as u32
    }
    pub fn a4r_48_slot_bytes(&self) -> usize {
        core::mem::size_of_val(self.markets)
    }
}
