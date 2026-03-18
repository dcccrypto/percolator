use std::mem::{size_of, offset_of};
fn main() {
    use percolator::RiskEngine;
    println!("sizeof RiskEngine = {}", size_of::<RiskEngine>());
    println!("accounts: {}", offset_of!(RiskEngine, accounts));
    println!("num_used_accounts: {}", offset_of!(RiskEngine, num_used_accounts));
    println!("materialized_account_count: {}", offset_of!(RiskEngine, materialized_account_count));
    use percolator::Account;
    println!("sizeof Account = {}", size_of::<Account>());
}
