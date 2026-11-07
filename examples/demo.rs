use std::process::ExitCode;
use testferrum::asserts::assert_equal;
use testferrum::cli::run_cli;
use testferrum::runner::{suite, test};
use testferrum::skip;

fn main() -> ExitCode {
    run_cli(vec![suite("Math", || (), |()| (), vec![
        test("adds", |()| assert_equal(4, 2 + 2, "sum")),
        test("later", |()| skip("not ready")),
        test("breaks", |()| assert_equal(4, 5, "sum")),
    ])])
}
