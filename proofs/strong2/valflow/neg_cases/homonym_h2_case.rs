    #[inline(never)]
    fn a3_w(&mut self) {
        self.header.asset_slot_capacity.bytes = [0xff; 4];
    }
    pub fn a3_h2(&mut self, x: u128) -> V16Result<()> {
        let mut o: crate::core::option::Option<u128> = crate::core::option::Option::Some(x);
        let mut i = 0u128;
        while i < x {
            o = crate::core::option::Option::Some(i);
            i += 1;
        }
        match o {
            crate::core::option::Option::None => {}
            crate::core::option::Option::Some(_) => {
                self.a3_w();
            }
        }
        Ok(())
    }
