use calculator::{Ledger, add, balance, divide, open_ledger};
use std::process::ExitCode;
use testferrum::asserts::{assert_equal, assert_len, assert_none, assert_not_empty};
use testferrum::cli::run_cli;
use testferrum::runner::{suite, test};
use testferrum::{TestResult, combine, skip};

fn close_ledger(_ledger: Ledger) {}

#[allow(unreachable_code, unused_variables)]
fn exports_csv(ledger: &Ledger) -> TestResult {
    return skip("CSV export not built yet");
    assert_len(3, &ledger.entries, "rows")
}

fn main() -> ExitCode {
    run_cli(vec![
        suite("Arithmetic", || (), |()| (), vec![
            test("adds", |()| assert_equal(5, add(2, 3), "sum")),
            test("divides", |()| {
                combine(
                    assert_equal(Some(4), divide(8, 2), "even split"),
                    assert_none(&divide(1, 0), "divide by zero"),
                )
            }),
        ]),
        suite("Ledger", open_ledger, close_ledger, vec![
            test("balances", |ledger: &Ledger| assert_equal(75, balance(ledger), "balance")),
            test("has entries", |ledger: &Ledger| assert_not_empty(&ledger.entries, "entries")),
            test("exports csv", exports_csv),
        ]),
    ])
}
