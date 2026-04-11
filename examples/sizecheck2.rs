use percolator::*;
use core::mem::offset_of;
fn main() {
    // All offsets within RiskEngine
    println!("VAULT={}", offset_of!(RiskEngine, vault));
    println!("INSURANCE={}", offset_of!(RiskEngine, insurance_fund));
    println!("C_TOT={}", offset_of!(RiskEngine, c_tot));
    println!("PNL_POS_TOT={}", offset_of!(RiskEngine, pnl_pos_tot));
    println!("NUM_USED={}", offset_of!(RiskEngine, num_used_accounts));
    println!("USED_BITMAP={}", offset_of!(RiskEngine, used));
    println!("FUNDING_RATE={}", offset_of!(RiskEngine, funding_rate_e9_per_slot_last));
    println!("PARAMS={}", offset_of!(RiskEngine, params));
    println!("PARAMS_SIZE={}", std::mem::size_of::<RiskParams>());
    println!("INS_FLOOR_IN_PARAMS={}", offset_of!(RiskParams, insurance_floor));
}
