use core::mem::offset_of;
use percolator::*;
fn main() {
    println!("ACCOUNT_SIZE={}", std::mem::size_of::<Account>());
    println!("ACCOUNTS_OFF={}", offset_of!(RiskEngine, accounts));
    println!("CAPITAL_OFF={}", offset_of!(Account, capital));
    println!("PBQ_OFF={}", offset_of!(Account, position_basis_q));
    println!("ADL_A_BASIS_OFF={}", offset_of!(Account, adl_a_basis));
    println!("ADL_EPOCH_SNAP_OFF={}", offset_of!(Account, adl_epoch_snap));
    println!(
        "ADL_EPOCH_LONG_OFF={}",
        offset_of!(RiskEngine, adl_epoch_long)
    );
    println!(
        "ADL_EPOCH_SHORT_OFF={}",
        offset_of!(RiskEngine, adl_epoch_short)
    );
    println!("C_TOT_OFF={}", offset_of!(RiskEngine, c_tot));
    println!("VAULT_OFF={}", offset_of!(RiskEngine, vault));
    println!("INSURANCE_OFF={}", offset_of!(RiskEngine, insurance_fund));
    println!("PNL_POS_TOT_OFF={}", offset_of!(RiskEngine, pnl_pos_tot));
    println!("NUM_USED_OFF={}", offset_of!(RiskEngine, num_used_accounts));
    println!("ENGINE_SIZE={}", std::mem::size_of::<RiskEngine>());
}
// Append: A side offsets
