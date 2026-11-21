    #[inline(never)]
    fn a3_w(&mut self) {
        self.header.asset_slot_capacity.bytes = [0xff; 4];
    }
    pub fn a3_h1(&mut self, x: u128) -> V16Result<()> {
        let mut r: crate::Result<u128, u8> = crate::Result::Ok(x);
        let mut i = 0u128;
        while i < x {
            r = crate::Result::Ok(i);
            i += 1;
        }
        match r {
            crate::Result::Err(_) => {}
            crate::Result::Ok(_) => {
                self.a3_w();
            }
        }
        Ok(())
    }
