//! User portfolio for cross-margin tracking

use pinocchio::pubkey::Pubkey;
use percolator_common::{MAX_INSTRUMENTS, MAX_SLABS};

/// Exposure key: (slab_index, instrument_index)
pub type ExposureKey = (u16, u16);

/// User portfolio tracking cross-margin state
/// PDA: ["portfolio", router_id, user]
#[repr(C)]
pub struct Portfolio {
    /// Router program ID
    pub router_id: Pubkey,
    /// User pubkey
    pub user: Pubkey,
    /// Total equity across all slabs
    pub equity: i128,
    /// Initial margin requirement
    pub im: u128,
    /// Maintenance margin requirement
    pub mm: u128,
    /// Free collateral (equity - IM)
    pub free_collateral: i128,
    /// Last mark timestamp
    pub last_mark_ts: u64,
    /// Number of exposures
    pub exposure_count: u16,
    /// Bump seed
    pub bump: u8,
    /// Padding
    pub _padding: [u8; 5],

    // Liquidation tracking
    /// Health (equity - MM)
    pub health: i128,
    /// Last liquidation timestamp (for rate limiting)
    pub last_liquidation_ts: u64,
    /// Cooldown period between deleveraging attempts (seconds)
    pub cooldown_seconds: u64,
    /// Padding for alignment
    pub _padding2: [u8; 8],

    /// Exposures: (slab_idx, instrument_idx) -> position qty
    /// Using fixed-size array for simplicity (can optimize with HashMap-like structure)
    pub exposures: [(u16, u16, i64); MAX_SLABS * MAX_INSTRUMENTS],
}

impl Portfolio {
    pub const LEN: usize = core::mem::size_of::<Self>();

    /// Initialize portfolio in-place (avoids stack allocation)
    ///
    /// This method initializes the portfolio fields directly without creating
    /// a large temporary struct on the stack (which would exceed BPF's 4KB limit).
    pub fn initialize_in_place(&mut self, router_id: Pubkey, user: Pubkey, bump: u8) {
        self.router_id = router_id;
        self.user = user;
        self.equity = 0;
        self.im = 0;
        self.mm = 0;
        self.free_collateral = 0;
        self.last_mark_ts = 0;
        self.exposure_count = 0;
        self.bump = bump;
        self._padding = [0; 5];

        // Initialize liquidation tracking
        self.health = 0;  // equity - MM = 0 - 0 = 0
        self.last_liquidation_ts = 0;
        self.cooldown_seconds = 60;  // 1 minute default cooldown
        self._padding2 = [0; 8];

        // Zero out the exposures array using ptr::write_bytes (efficient and stack-safe)
        unsafe {
            core::ptr::write_bytes(
                self.exposures.as_mut_ptr(),
                0,
                MAX_SLABS * MAX_INSTRUMENTS,
            );
        }
    }

    /// Initialize new portfolio (for tests only - uses stack)
    #[cfg(test)]
    pub fn new(router_id: Pubkey, user: Pubkey, bump: u8) -> Self {
        Self {
            router_id,
            user,
            equity: 0,
            im: 0,
            mm: 0,
            free_collateral: 0,
            last_mark_ts: 0,
            exposure_count: 0,
            bump,
            _padding: [0; 5],
            health: 0,
            last_liquidation_ts: 0,
            cooldown_seconds: 60,
            _padding2: [0; 8],
            exposures: [(0, 0, 0); MAX_SLABS * MAX_INSTRUMENTS],
        }
    }

    /// Update exposure for (slab, instrument)
    pub fn update_exposure(&mut self, slab_idx: u16, instrument_idx: u16, qty: i64) {
        // Find existing exposure or add new one
        for i in 0..self.exposure_count as usize {
            if self.exposures[i].0 == slab_idx && self.exposures[i].1 == instrument_idx {
                self.exposures[i].2 = qty;
                // Remove if qty is zero
                if qty == 0 {
                    self.remove_exposure_at(i);
                }
                return;
            }
        }

        // Add new exposure if non-zero
        if qty != 0 && (self.exposure_count as usize) < self.exposures.len() {
            let idx = self.exposure_count as usize;
            self.exposures[idx] = (slab_idx, instrument_idx, qty);
            self.exposure_count += 1;
        }
    }

    /// Remove exposure at index
    fn remove_exposure_at(&mut self, idx: usize) {
        if idx < self.exposure_count as usize {
            // Swap with last and decrement count
            let last_idx = (self.exposure_count - 1) as usize;
            if idx != last_idx {
                self.exposures[idx] = self.exposures[last_idx];
            }
            self.exposures[last_idx] = (0, 0, 0);
            self.exposure_count -= 1;
        }
    }

    /// Get exposure for (slab, instrument)
    pub fn get_exposure(&self, slab_idx: u16, instrument_idx: u16) -> i64 {
        for i in 0..self.exposure_count as usize {
            if self.exposures[i].0 == slab_idx && self.exposures[i].1 == instrument_idx {
                return self.exposures[i].2;
            }
        }
        0
    }

    /// Update margin requirements
    pub fn update_margin(&mut self, im: u128, mm: u128) {
        self.im = im;
        self.mm = mm;
        self.free_collateral = self.equity.saturating_sub(im as i128);
    }

    /// Update equity
    pub fn update_equity(&mut self, equity: i128) {
        self.equity = equity;
        self.free_collateral = equity.saturating_sub(self.im as i128);
    }

    /// Check if sufficient margin
    pub fn has_sufficient_margin(&self) -> bool {
        self.equity >= self.im as i128
    }

    /// Check if above maintenance margin
    pub fn is_above_maintenance(&self) -> bool {
        self.equity >= self.mm as i128
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_portfolio_exposures() {
        let mut portfolio = Portfolio::new(Pubkey::default(), Pubkey::default(), 0);

        portfolio.update_exposure(0, 0, 100);
        assert_eq!(portfolio.get_exposure(0, 0), 100);
        assert_eq!(portfolio.exposure_count, 1);

        portfolio.update_exposure(0, 1, 50);
        assert_eq!(portfolio.get_exposure(0, 1), 50);
        assert_eq!(portfolio.exposure_count, 2);

        portfolio.update_exposure(0, 0, 0);
        assert_eq!(portfolio.get_exposure(0, 0), 0);
        assert_eq!(portfolio.exposure_count, 1);
    }

    #[test]
    fn test_portfolio_margin() {
        let mut portfolio = Portfolio::new(Pubkey::default(), Pubkey::default(), 0);

        portfolio.update_equity(10000);
        portfolio.update_margin(5000, 2500);

        assert!(portfolio.has_sufficient_margin());
        assert!(portfolio.is_above_maintenance());
        assert_eq!(portfolio.free_collateral, 5000);

        portfolio.update_equity(4000);
        assert!(!portfolio.has_sufficient_margin());
        assert!(portfolio.is_above_maintenance());

        portfolio.update_equity(2000);
        assert!(!portfolio.is_above_maintenance());
    }
}
