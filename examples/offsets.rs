use std::mem::offset_of;
fn main() {
    use percolator::{RiskEngine, RiskParams};
    println!("insurance_floor in RiskParams: {}", offset_of!(RiskParams, insurance_floor));
    println!("params in RiskEngine: {}", offset_of!(RiskEngine, params));
    let total = offset_of!(RiskEngine, params) + offset_of!(RiskParams, insurance_floor);
    println!("insurance_floor in RiskEngine: {}", total);
}
