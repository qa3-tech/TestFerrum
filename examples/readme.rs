use std::process::ExitCode;
use testferrum::asserts::{
    assert_contains, assert_equal, assert_false, assert_greater, assert_in_delta, assert_len, assert_ok, assert_regexp,
};
use testferrum::cli::run_cli;
use testferrum::runner::{suite, test, Suite};
use testferrum::{and_then, combine, skip, TestResult};

struct User { name: String, age: i32, email: String }
fn load_user() -> User { User { name: "Ann".into(), age: 30, email: "a@b".into() } }

struct Database { rows: Vec<i32> }
fn open_database() -> Database { Database { rows: vec![1, 2, 3] } }
fn close_database(_database: Database) {}

#[allow(unreachable_code, unused_variables)]
fn can_query(database: &Database) -> TestResult {
    return skip("flaky on CI");
    assert_len(3, &database.rows, "rows")
}

fn doubling() -> Suite {
    let cases = [(1, 2), (2, 4), (5, 10)];
    let doubles = cases
        .iter()
        .map(|&(input, expected)| {
            test(&format!("{input} * 2 = {expected}"), move |()| {
                assert_equal(expected, input * 2, "double")
            })
        })
        .collect();
    suite("Doubling", || (), |()| (), doubles)
}

fn main() -> ExitCode {
    run_cli(vec![
        suite("Math", || (), |()| (), vec![
            test("adds", |()| assert_equal(4, 2 + 2, "sum")),
            test("compares", |()| assert_greater(5, 3, "order")),
            test("asserts", |()| combine(assert_contains(&3, &[1, 2, 3], "has three"),
                combine(assert_in_delta(0.3, 0.1 + 0.2, 1e-9, "float sum"), assert_regexp(r"^\d+$", "123", "digits")))),
            test("validates user", |()| {
                let user = load_user();
                combine(
                    assert_false(user.name.is_empty(), "name"),
                    combine(assert_greater(user.age, 0, "age"), assert_regexp("@", &user.email, "email")),
                )
            }),
            test("parses then checks", |()| {
                let parsed = "42".parse::<i32>();
                and_then(assert_ok(&parsed, "parses"), || assert_equal(Ok(42), parsed.clone(), "value"))
            }),
        ]),
        suite("Database", open_database, close_database, vec![
            test("counts rows", |database: &Database| assert_len(3, &database.rows, "rows")),
            test("finds row", |database: &Database| assert_contains(&2, &database.rows, "row 2")),
            test("can query", can_query),
        ]),
        doubling(),
    ])
}
