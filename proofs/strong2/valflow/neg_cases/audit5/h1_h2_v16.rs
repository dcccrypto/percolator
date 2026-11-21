
// ======== audit6 batch 1 hostile items ========
pub const A6_LEN: usize = include_str!("v16.rs").len();
pub const A6_N: usize = 01323378;
pub enum A6Inc {
    A = (A6_LEN != A6_N) as isize,
    B = (A6_LEN == A6_N) as isize,
}
const fn a6_has_probe(s: &str) -> bool {
    let b = s.as_bytes();
    let p = b"/probe-";
    let mut i = 0;
    while i + p.len() <= b.len() {
        let mut j = 0;
        while j < p.len() && b[i + j] == p[j] { j += 1; }
        if j == p.len() { return true; }
        i += 1;
    }
    false
}
pub const A6_INPROBE: bool = a6_has_probe(env!("CARGO_MANIFEST_DIR"));
pub enum A6Env {
    A = A6_INPROBE as isize,
    B = !A6_INPROBE as isize,
}
#[repr(i8)]
pub enum A6Neg {
    A = -1,
    B = 1,
    C = -128,
}
#[repr(u64)]
pub enum A6Big {
    A = 1 << 63,
    B = u64::MAX,
    C = 0,
}
#[repr(i64)]
pub enum A6NegW {
    A = i64::MIN,
    B = -1,
    C = 5,
}
pub enum A6Niche {
    A(core::num::NonZeroU32),
    B,
    C(bool),
}
pub enum A6Un {
    A(core::convert::Infallible),
    B,
    C,
}
pub enum A6One {
    A,
}
#[non_exhaustive]
pub enum A6NonEx {
    B = 3,
    A = 2,
}
pub mod a6m {
pub enum A6Trim {
    B,
    A,
}
}
pub use a6m::A6Trim;
/*
enum A6Mac {
    A,
    B,
}
*/
macro_rules! a6mk { ($n:ident, $a:ident, $b:ident) => { pub enum $n { $b, $a } } }
a6mk!(A6Mac, A, B);
#[cfg(any())]
enum A6Cfg {
    A,
    B,
}
#[cfg(all())]
pub enum A6Cfg { B, A }
pub mod a6n {
    pub enum A6Alias { B, A }
}
const A6_STR: &str = r#"
enum A6Alias {
    A,
    B,
}
"#;
pub type A6Alias = a6n::A6Alias;
pub enum A6Sh {
    A,
    B,
}
#[inline(never)]
pub fn a6_let_A6ShInner(f: fn(&u32)) -> u32 {
    pub enum A6Sh { B, A }
    let v = A6Sh::A;
    f(&0);
    a6_sink(&v as *const A6Sh as usize);
    match v { A6Sh::A => 1, A6Sh::B => 2 }
}
#[inline(never)]
pub fn a6_sink(_x: usize) {}
#[inline(never)]
pub fn a6_let_A6Inc_A(f: fn(&A6Inc)) -> u32 {
    let v = A6Inc::A;
    f(&v);
    match v { A6Inc::A => 1, A6Inc::B => 2, }
}
#[inline(never)]
pub fn a6_let_A6Inc_B(f: fn(&A6Inc)) -> u32 {
    let v = A6Inc::B;
    f(&v);
    match v { A6Inc::A => 1, A6Inc::B => 2, }
}
#[inline(never)]
pub fn a6_sw_A6Inc(x: &A6Inc) -> u32 {
    match *x { A6Inc::A => 1, A6Inc::B => 2, }
}
#[inline(never)]
pub fn a6_let_A6Env_A(f: fn(&A6Env)) -> u32 {
    let v = A6Env::A;
    f(&v);
    match v { A6Env::A => 1, A6Env::B => 2, }
}
#[inline(never)]
pub fn a6_let_A6Env_B(f: fn(&A6Env)) -> u32 {
    let v = A6Env::B;
    f(&v);
    match v { A6Env::A => 1, A6Env::B => 2, }
}
#[inline(never)]
pub fn a6_sw_A6Env(x: &A6Env) -> u32 {
    match *x { A6Env::A => 1, A6Env::B => 2, }
}
#[inline(never)]
pub fn a6_let_A6Neg_A(f: fn(&A6Neg)) -> u32 {
    let v = A6Neg::A;
    f(&v);
    match v { A6Neg::A => 1, A6Neg::B => 2, A6Neg::C => 3, }
}
#[inline(never)]
pub fn a6_let_A6Neg_B(f: fn(&A6Neg)) -> u32 {
    let v = A6Neg::B;
    f(&v);
    match v { A6Neg::A => 1, A6Neg::B => 2, A6Neg::C => 3, }
}
#[inline(never)]
pub fn a6_let_A6Neg_C(f: fn(&A6Neg)) -> u32 {
    let v = A6Neg::C;
    f(&v);
    match v { A6Neg::A => 1, A6Neg::B => 2, A6Neg::C => 3, }
}
#[inline(never)]
pub fn a6_sw_A6Neg(x: &A6Neg) -> u32 {
    match *x { A6Neg::A => 1, A6Neg::B => 2, A6Neg::C => 3, }
}
#[inline(never)]
pub fn a6_let_A6Big_A(f: fn(&A6Big)) -> u32 {
    let v = A6Big::A;
    f(&v);
    match v { A6Big::A => 1, A6Big::B => 2, A6Big::C => 3, }
}
#[inline(never)]
pub fn a6_let_A6Big_B(f: fn(&A6Big)) -> u32 {
    let v = A6Big::B;
    f(&v);
    match v { A6Big::A => 1, A6Big::B => 2, A6Big::C => 3, }
}
#[inline(never)]
pub fn a6_let_A6Big_C(f: fn(&A6Big)) -> u32 {
    let v = A6Big::C;
    f(&v);
    match v { A6Big::A => 1, A6Big::B => 2, A6Big::C => 3, }
}
#[inline(never)]
pub fn a6_sw_A6Big(x: &A6Big) -> u32 {
    match *x { A6Big::A => 1, A6Big::B => 2, A6Big::C => 3, }
}
#[inline(never)]
pub fn a6_let_A6NegW_A(f: fn(&A6NegW)) -> u32 {
    let v = A6NegW::A;
    f(&v);
    match v { A6NegW::A => 1, A6NegW::B => 2, A6NegW::C => 3, }
}
#[inline(never)]
pub fn a6_let_A6NegW_B(f: fn(&A6NegW)) -> u32 {
    let v = A6NegW::B;
    f(&v);
    match v { A6NegW::A => 1, A6NegW::B => 2, A6NegW::C => 3, }
}
#[inline(never)]
pub fn a6_let_A6NegW_C(f: fn(&A6NegW)) -> u32 {
    let v = A6NegW::C;
    f(&v);
    match v { A6NegW::A => 1, A6NegW::B => 2, A6NegW::C => 3, }
}
#[inline(never)]
pub fn a6_sw_A6NegW(x: &A6NegW) -> u32 {
    match *x { A6NegW::A => 1, A6NegW::B => 2, A6NegW::C => 3, }
}
#[inline(never)]
pub fn a6_let_A6Un_B(f: fn(&A6Un)) -> u32 {
    let v = A6Un::B;
    f(&v);
    match v { A6Un::B => 1, A6Un::C => 2, _ => 0, }
}
#[inline(never)]
pub fn a6_let_A6Un_C(f: fn(&A6Un)) -> u32 {
    let v = A6Un::C;
    f(&v);
    match v { A6Un::B => 1, A6Un::C => 2, _ => 0, }
}
#[inline(never)]
pub fn a6_sw_A6Un(x: &A6Un) -> u32 {
    match *x { A6Un::B => 1, A6Un::C => 2, _ => 0, }
}
#[inline(never)]
pub fn a6_let_A6NonEx_A(f: fn(&A6NonEx)) -> u32 {
    let v = A6NonEx::A;
    f(&v);
    match v { A6NonEx::A => 1, A6NonEx::B => 2, }
}
#[inline(never)]
pub fn a6_let_A6NonEx_B(f: fn(&A6NonEx)) -> u32 {
    let v = A6NonEx::B;
    f(&v);
    match v { A6NonEx::A => 1, A6NonEx::B => 2, }
}
#[inline(never)]
pub fn a6_sw_A6NonEx(x: &A6NonEx) -> u32 {
    match *x { A6NonEx::A => 1, A6NonEx::B => 2, }
}
#[inline(never)]
pub fn a6_let_A6Trim_A(f: fn(&A6Trim)) -> u32 {
    let v = A6Trim::A;
    f(&v);
    match v { A6Trim::A => 1, A6Trim::B => 2, }
}
#[inline(never)]
pub fn a6_let_A6Trim_B(f: fn(&A6Trim)) -> u32 {
    let v = A6Trim::B;
    f(&v);
    match v { A6Trim::A => 1, A6Trim::B => 2, }
}
#[inline(never)]
pub fn a6_sw_A6Trim(x: &A6Trim) -> u32 {
    match *x { A6Trim::A => 1, A6Trim::B => 2, }
}
#[inline(never)]
pub fn a6_let_A6Mac_A(f: fn(&A6Mac)) -> u32 {
    let v = A6Mac::A;
    f(&v);
    match v { A6Mac::A => 1, A6Mac::B => 2, }
}
#[inline(never)]
pub fn a6_let_A6Mac_B(f: fn(&A6Mac)) -> u32 {
    let v = A6Mac::B;
    f(&v);
    match v { A6Mac::A => 1, A6Mac::B => 2, }
}
#[inline(never)]
pub fn a6_sw_A6Mac(x: &A6Mac) -> u32 {
    match *x { A6Mac::A => 1, A6Mac::B => 2, }
}
#[inline(never)]
pub fn a6_let_A6Cfg_A(f: fn(&A6Cfg)) -> u32 {
    let v = A6Cfg::A;
    f(&v);
    match v { A6Cfg::A => 1, A6Cfg::B => 2, }
}
#[inline(never)]
pub fn a6_let_A6Cfg_B(f: fn(&A6Cfg)) -> u32 {
    let v = A6Cfg::B;
    f(&v);
    match v { A6Cfg::A => 1, A6Cfg::B => 2, }
}
#[inline(never)]
pub fn a6_sw_A6Cfg(x: &A6Cfg) -> u32 {
    match *x { A6Cfg::A => 1, A6Cfg::B => 2, }
}
#[inline(never)]
pub fn a6_let_A6Alias_A(f: fn(&A6Alias)) -> u32 {
    let v = A6Alias::A;
    f(&v);
    match v { A6Alias::A => 1, A6Alias::B => 2, }
}
#[inline(never)]
pub fn a6_let_A6Alias_B(f: fn(&A6Alias)) -> u32 {
    let v = A6Alias::B;
    f(&v);
    match v { A6Alias::A => 1, A6Alias::B => 2, }
}
#[inline(never)]
pub fn a6_sw_A6Alias(x: &A6Alias) -> u32 {
    match *x { A6Alias::A => 1, A6Alias::B => 2, }
}
#[inline(never)]
pub fn a6_let_A6Sh_A(f: fn(&A6Sh)) -> u32 {
    let v = A6Sh::A;
    f(&v);
    match v { A6Sh::A => 1, A6Sh::B => 2, }
}
#[inline(never)]
pub fn a6_let_A6Sh_B(f: fn(&A6Sh)) -> u32 {
    let v = A6Sh::B;
    f(&v);
    match v { A6Sh::A => 1, A6Sh::B => 2, }
}
#[inline(never)]
pub fn a6_sw_A6Sh(x: &A6Sh) -> u32 {
    match *x { A6Sh::A => 1, A6Sh::B => 2, }
}
#[inline(never)]
pub fn a6_let_A6Niche_B(f: fn(&A6Niche)) -> u32 {
    let v = A6Niche::B;
    f(&v);
    match v { A6Niche::A(_) => 1, A6Niche::B => 2, A6Niche::C(_) => 3 }
}
