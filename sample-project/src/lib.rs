#[must_use]
pub fn add(left: i64, right: i64) -> i64 {
    left + right
}

#[must_use]
pub fn divide(numerator: i64, denominator: i64) -> Option<i64> {
    if denominator == 0 { None } else { Some(numerator / denominator) }
}

pub struct Ledger {
    pub entries: Vec<i64>,
}

#[must_use]
pub fn open_ledger() -> Ledger {
    Ledger { entries: vec![100, -40, 15] }
}

#[must_use]
pub fn balance(ledger: &Ledger) -> i64 {
    ledger.entries.iter().sum()
}
