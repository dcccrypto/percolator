fn main() {
    println!(
        "ACCOUNTS_OFF={}",
        core::mem::offset_of!(percolator::RiskEngine, accounts)
    );
    println!(
        "C_TOT={}",
        core::mem::offset_of!(percolator::RiskEngine, c_tot)
    );
    println!(
        "PNL_POS_TOT={}",
        core::mem::offset_of!(percolator::RiskEngine, pnl_pos_tot)
    );
    println!(
        "ADL_MULT_LONG={}",
        core::mem::offset_of!(percolator::RiskEngine, adl_mult_long)
    );
    println!(
        "ADL_MULT_SHORT={}",
        core::mem::offset_of!(percolator::RiskEngine, adl_mult_short)
    );
    println!(
        "ADL_EPOCH_LONG={}",
        core::mem::offset_of!(percolator::RiskEngine, adl_epoch_long)
    );
    println!(
        "ADL_EPOCH_SHORT={}",
        core::mem::offset_of!(percolator::RiskEngine, adl_epoch_short)
    );
    println!(
        "NUM_USED={}",
        core::mem::offset_of!(percolator::RiskEngine, num_used_accounts)
    );
    println!(
        "BITMAP={}",
        core::mem::offset_of!(percolator::RiskEngine, used)
    );
    println!(
        "ENGINE_SIZE={}",
        std::mem::size_of::<percolator::RiskEngine>()
    );
    println!(
        "ACCOUNT_SIZE={}",
        std::mem::size_of::<percolator::Account>()
    );
    println!("MAX_ACCOUNTS={}", percolator::MAX_ACCOUNTS);
}
