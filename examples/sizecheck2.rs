use core::mem::offset_of;
use percolator::*;
fn main() {
    println!("capital={}", offset_of!(Account, capital));
    println!("kind={}", offset_of!(Account, kind));
    println!("pnl={}", offset_of!(Account, pnl));
    println!("reserved_pnl={}", offset_of!(Account, reserved_pnl));
    println!("position_basis_q={}", offset_of!(Account, position_basis_q));
    println!("adl_a_basis={}", offset_of!(Account, adl_a_basis));
    println!("adl_k_snap={}", offset_of!(Account, adl_k_snap));
    println!("f_snap={}", offset_of!(Account, f_snap));
    println!("adl_epoch_snap={}", offset_of!(Account, adl_epoch_snap));
    println!("matcher_program={}", offset_of!(Account, matcher_program));
    println!("owner={}", offset_of!(Account, owner));
    println!("fee_credits={}", offset_of!(Account, fee_credits));
    println!("sched_present={}", offset_of!(Account, sched_present));
    println!("ACCOUNT_SIZE={}", std::mem::size_of::<Account>());
}
