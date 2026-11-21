
// ======== audit6 batch 2 hostile items (lib.rs) ========
#[allow(dead_code)]
pub fn v16() -> u32 {
    pub enum A6Fn { B, A }
    #[inline(never)]
    pub fn a6_use_A6Fn() -> u32 {
        let v = A6Fn::A;
        crate::v16::a6_sink(&v as *const A6Fn as usize);
        match v { A6Fn::A => 1, A6Fn::B => 2 }
    }
    a6_use_A6Fn()
}
