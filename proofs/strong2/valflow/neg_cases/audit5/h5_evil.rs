#[inline(never)]
pub fn a6_sink(_x: usize) {}
pub enum A6Root { B, A }
#[inline(never)]
pub fn a6_use_A6Root() -> u32 {
    let v = A6Root::A;
    a6_sink(&v as *const A6Root as usize);
    match v { A6Root::A => 1, A6Root::B => 2 }
}
