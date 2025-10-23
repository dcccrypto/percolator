//! AMM instructions - initialize and commit_fill

use crate::AmmState;
use percolator_common::{PercolatorError, Side, SlabHeader};
use pinocchio::{account_info::AccountInfo, msg, pubkey::Pubkey, ProgramResult};

/// Initialize a new AMM pool
pub fn process_initialize(
    accounts: &[AccountInfo],
    lp_owner: Pubkey,
    router_id: Pubkey,
    instrument: Pubkey,
    mark_px: i64,
    taker_fee_bps: i64,
    contract_size: i64,
    bump: u8,
    x_reserve: i64,
    y_reserve: i64,
) -> ProgramResult {
    let [amm_account, payer] = accounts else {
        return Err(PercolatorError::InvalidAccount.into());
    };

    // Verify payer signed
    if !payer.is_signer() {
        return Err(PercolatorError::InvalidAccount.into());
    }

    // Get account data
    let data = amm_account.try_borrow_mut_data()?;
    if data.len() != AmmState::LEN {
        msg!("Error: AMM account has incorrect size");
        return Err(PercolatorError::InvalidAccount.into());
    }

    // Create header
    let header = SlabHeader::new(
        *amm_account.key(),
        lp_owner,
        router_id,
        instrument,
        mark_px,
        taker_fee_bps,
        contract_size,
        bump,
    );

    // Create AMM state
    let mut amm = AmmState::new(header, x_reserve, y_reserve, taker_fee_bps);

    // Synthesize initial quote cache
    amm.synthesize_quote_cache();

    // Write to account (unsafe cast from bytes)
    unsafe {
        let state_ptr = data.as_ptr() as *mut AmmState;
        *state_ptr = amm;
    }

    msg!("AMM initialized successfully");
    Ok(())
}

/// Commit a fill (same interface as slab's commit_fill)
pub fn process_commit_fill(
    _accounts: &[AccountInfo],
    _side: Side,
    _qty: i64,
    _limit_px: i64,
) -> ProgramResult {
    // TODO: Implement full commit_fill with receipt writing
    // For now, just a placeholder
    msg!("AMM commit_fill not yet implemented");
    Err(PercolatorError::InvalidInstruction.into())
}
