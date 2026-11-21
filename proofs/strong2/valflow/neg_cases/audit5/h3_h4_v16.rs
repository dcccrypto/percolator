
// ======== audit6 batch 2 hostile items (v16.rs) ========
#[inline(never)]
pub fn a6_sink(_x: usize) {}
pub const A6_LEN: usize = include_str!("v16.rs").len();
pub const A6_N: usize = 00000000;
pub enum A6Inc {
    A = (A6_LEN != A6_N) as isize,
    B = (A6_LEN == A6_N) as isize,
}
#[inline(never)]
pub fn a6_use_A6Inc() -> u32 {
    let v = A6Inc::A;
    a6_sink(&v as *const A6Inc as usize);
    match v { A6Inc::A => 1, A6Inc::B => 2 }
}
pub enum A6Fn {
    A,
    B,
}
#[inline(never)]
pub fn a6_sw_A6Fn(x: &A6Fn) -> u32 { match *x { A6Fn::A => 1, A6Fn::B => 2 } }
pub enum A6Hy {
    A,
    B,
}
pub macro a6_hy($f:ident) {
    pub enum A6Hy { B, A }
    #[inline(never)]
    pub fn $f() -> u32 {
        let v = A6Hy::A;
        a6_sink(&v as *const A6Hy as usize);
        match v { A6Hy::A => 1, A6Hy::B => 2 }
    }
}
a6_hy!(a6_use_A6Hy);
