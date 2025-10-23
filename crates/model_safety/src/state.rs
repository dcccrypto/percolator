//! Pure state model for Kani verification

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Warmup {
    pub started_at_slot: u64,
    pub slope_per_step: u128, // Linear cap per step for Kani model
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Account {
    pub principal: u128,      // Never reduced by socialize/loss (I1)
    pub pnl_ledger: i128,     // Can be positive or negative
    pub reserved_pnl: u128,   // Pending withdrawals
    pub warmup_state: Warmup,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Params {
    pub max_users: u8,
    pub withdraw_cap_per_step: u128,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct State {
    pub vault: u128,
    pub insurance_fund: u128,
    pub fees_outstanding: u128,
    pub users: arrayvec::ArrayVec<Account, 6>, // Small fixed bound for Kani
    pub params: Params,
    pub authorized_router: bool, // For I3: authorization checks
}

impl Default for Warmup {
    fn default() -> Self {
        Self {
            started_at_slot: 0,
            slope_per_step: 1_000_000,
        }
    }
}

impl Default for Account {
    fn default() -> Self {
        Self {
            principal: 0,
            pnl_ledger: 0,
            reserved_pnl: 0,
            warmup_state: Warmup::default(),
        }
    }
}

impl Default for Params {
    fn default() -> Self {
        Self {
            max_users: 6,
            withdraw_cap_per_step: 1_000_000,
        }
    }
}

impl Default for State {
    fn default() -> Self {
        Self {
            vault: 0,
            insurance_fund: 0,
            fees_outstanding: 0,
            users: arrayvec::ArrayVec::new(),
            params: Params::default(),
            authorized_router: true,
        }
    }
}
