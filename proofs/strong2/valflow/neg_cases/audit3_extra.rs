#[inline(never)]
pub fn a3_free_w0<T>(m: &mut MarketGroupV16ViewMut<'_, T>, x: u128) -> V16Result<u128> {
    if x != 0 {
        m.header.asset_slot_capacity.bytes = [0xff; 4];
    }
    Ok(0)
}
pub trait A3Tr {
    fn a3_zn(&mut self, x: u128) -> V16Result<u128>;
}
impl<'a, T> A3Tr for MarketGroupV16ViewMut<'a, T> {
    #[inline(never)]
    fn a3_zn(&mut self, x: u128) -> V16Result<u128> {
        if x != 0 {
            self.header.asset_slot_capacity.bytes = [0xff; 4];
        }
        Ok(0)
    }
}
pub struct A3Wrap<'b, 'a, T> {
    pub m: &'b mut MarketGroupV16ViewMut<'a, T>,
}
impl<'b, 'a, T> A3Wrap<'b, 'a, T> {
    #[inline(never)]
    pub fn go(self, x: u128) -> V16Result<u128> {
        if x != 0 {
            self.m.header.asset_slot_capacity.bytes = [0xff; 4];
        }
        Ok(0)
    }
}
