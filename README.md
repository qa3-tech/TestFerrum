# TestFerrum: Railway-Oriented Testing Framework for Rust

TestFerrum ports [TestTracks](https://github.com/qa3-tech/TestTracks) (F#) and [TestCracks](https://github.com/qa3-tech/TestCracks) (C) to Rust. Tests are pure functions that return results; failures travel as data, not panics.

## Philosophy

- **Pure functions:** a test takes an environment and returns a `TestResult`.
- **Explicit error flow:** failures are values; nothing panics or unwinds.
- **Composition:** `combine` collects every error; `and_then` stops at the first.
- **Explicit setup and teardown:** every suite declares both, even when empty.
- **No macros, no reflection:** suites are plain lists of plain functions.

## Installation

TestFerrum is not on crates.io yet. Add it as a path or git dependency:

```toml
[dev-dependencies]
testferrum = { path = "../testferrum" }
```

It depends only on `regex`.

## Quick Start

### 1. Register a test target

TestFerrum replaces Rust's built-in harness, so each test file needs a `[[test]]` entry with `harness = false`:

```toml
[[test]]
name = "math"
harness = false
```

### 2. Write tests in `tests/math.rs`

```rust
use std::process::ExitCode;
use testferrum::asserts::{assert_equal, assert_greater};
use testferrum::cli::run_cli;
use testferrum::runner::{suite, test};

fn main() -> ExitCode {
    run_cli(vec![
        suite("Math", || (), |()| (), vec![
            test("adds", |()| assert_equal(4, 2 + 2, "sum")),
            test("compares", |()| assert_greater(5, 3, "order")),
        ]),
    ])
}
```

### 3. Run

```bash
cargo test --test math                                # run all suites
cargo test --test math -- --help                      # show options
cargo test --test math -- --suite "Math"              # one suite
cargo test --test math -- --test "Math" "adds"        # one test
cargo test --test math -- --match "add"               # tests whose name contains "add"
cargo test --test math -- --parallel                  # suites in parallel
cargo test --test math -- --junit results.xml         # JUnit XML report
cargo test --test math -- --slow 250                  # mark tests slower than 250 ms
```

Flags combine: `--parallel --suite "Math" --junit results.xml` works.

### Full example

[`sample-project/`](sample-project) is a small library with its own TestFerrum target: suites with and without an environment, `combine`, and a skipped test. Run it with `cargo test --test calculator` from that folder.

## Terminal Output

```
=== Math (0.01 ms) ===
✓ adds (0.00 ms)
○ later (skipped: not ready)
✕ breaks (0.01 ms)
  sum
    Expected: 4
    Actual:   5

1/3 passed, 1 failed, 1 skipped (Total: 0.01 ms)
```

Colors appear in a terminal and drop out when you redirect output. The exit code is 1 when any test fails.

## Core Concepts

### Test Results

```rust
pub struct TestError {
    pub message: String,
    pub expected: String,
    pub actual: String,
}

pub enum TestResult {
    Pass,
    Fail(Vec<TestError>),
    Skip(String),
}
```

Build results with `pass()`, `fail(message)`, `fail_with(message, expected, actual)` and `skip(reason)`. `TestResult` is `#[must_use]`, so the compiler warns when you drop one.

### Assertions

Every assertion returns a `TestResult`; none panics. Generics catch type mismatches at compile time.

```rust
assert_equal(4, 2 + 2, "sum")
assert_contains(&3, &[1, 2, 3], "has three")
assert_in_delta(0.3, 0.1 + 0.2, 1e-9, "float sum")
assert_regexp(r"^\d+$", "123", "digits")
```

### Collect Every Error: `combine`

`combine` runs both checks and keeps every error, so you see all failures in one run:

```rust
test("validates user", |()| {
    let user = load_user();
    combine(
        assert_false(user.name.is_empty(), "name"),
        combine(
            assert_greater(user.age, 0, "age"),
            assert_regexp("@", &user.email, "email"),
        ),
    )
})
```

Fail outranks Skip, and Skip outranks Pass.

### Stop at the First Error: `and_then`

`and_then` runs the next step only if the previous one passed. Use it when later checks depend on earlier ones:

```rust
test("parses then checks", |()| {
    let parsed = "42".parse::<i32>();
    and_then(assert_ok(&parsed, "parses"), || {
        assert_equal(Ok(42), parsed.clone(), "value")
    })
})
```

Steps share values through ordinary `let` bindings.

## Setup and Teardown

Each suite owns a typed environment. Setup runs once before the tests, every test borrows the environment, and teardown receives it after the last test:

```rust
struct Database {
    rows: Vec<i32>,
}

fn open_database() -> Database {
    Database { rows: vec![1, 2, 3] }
}

fn close_database(_database: Database) {}

suite("Database", open_database, close_database, vec![
    test("counts rows", |database: &Database| assert_len(3, &database.rows, "rows")),
    test("finds row", |database: &Database| assert_contains(&2, &database.rows, "row 2")),
])
```

Suites without state pass empty functions: `|| ()` and `|()| ()`. Writing `|()|` rather than `|_|` states that the environment is empty, and keeps Clippy's pedantic lints quiet.

Setup cannot fail. If it must abort, let it panic. When a filter selects no tests from a suite, setup never runs.

## Skipping Tests

Put `return skip(reason)` on the first line. The reason is required, and the body below still compiles, so skipped tests keep up with API changes:

```rust
#[allow(unreachable_code, unused_variables)]
fn can_query(database: &Database) -> TestResult {
    return skip("flaky on CI");
    assert_len(3, &database.rows, "rows")
}

suite("Database", open_database, close_database, vec![
    test("can query", can_query),
])
```

The `#[allow]` silences two warnings: the code after `return` is unreachable, and the parameters go unused. Drop it if you want the compiler to flag every skipped test at build time.

## Data-Driven Testing

Tests are values, so a loop builds them:

```rust
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
```

For large data sets, load them in setup and read them from the environment.

## Parallel vs Sequential

- **Sequential (default):** suites run one after another.
- **Parallel (`--parallel`):** each suite runs on its own thread.

Tests within a suite always run in listed order. Setup runs inside the suite's thread, so the environment never crosses threads and needs neither `Send` nor `Sync`.

## Slow Tests

`--slow <ms>` marks every test that took longer than the threshold and adds a count to the summary:

```
✓ loads fixtures (1.20 s [slow])

12/12 passed, 0 failed, 0 skipped (Total: 1.31 s), 1 slow (> 250 ms)
```

TestFerrum cannot stop a test that hangs: Rust cannot kill a thread, and running each test in its own process would repeat suite setup. The threshold reports slowness after the fact.

## Panics

TestFerrum does not catch panics. A panic stops the run, and in parallel mode it surfaces when its suite finishes. If code under test may panic and you want a failure instead, wrap the call yourself with `std::panic::catch_unwind`.

## Command Line Options

| Option | Effect |
|---|---|
| *(none)* | Run all suites sequentially |
| `--suite <name>` | Run one suite |
| `--test <suite> <test>` | Run one test |
| `--match <pattern>` | Run tests whose name contains the pattern |
| `--parallel` | Run suites in parallel |
| `--junit <file>` | Write one JUnit XML report for all suites, in place of console output |
| `--slow <ms>` | Mark tests slower than `<ms>` milliseconds and count them in the summary |
| `--list` | Print suites and tests with source locations, for editors |
| `--help`, `-h` | Show help |

## Why TestFerrum over `#[test]`?

- **Every failure in one run:** `assert!` panics at the first failure; `combine` reports them all.
- **Failures as data:** each error carries message, expected and actual, which you can format, filter or export.
- **Suite lifecycle:** libtest has no suite setup or teardown. Here setup runs once and hands each test a typed environment.
- **Safe parallelism:** libtest runs tests on parallel threads by default, so tests sharing a resource can race. TestFerrum runs suites in parallel and tests in order.
- **Fixed order:** tests run as listed, every run.
- **JUnit on stable Rust:** libtest's machine-readable output requires nightly flags; `--junit` does not.
- **Data-driven tests without macros:** a loop is enough.

### Trade-offs

- Editor support needs the TestFerrum extension (VS Code today); rust-analyzer's built-in buttons only cover `#[test]`.
- No `cargo nextest` support: nextest runs each test in its own process, which would repeat suite setup per test.
- Custom-harness targets live in `tests/`, so they reach only your crate's public API.
- Doc tests still use libtest.
- Panics stop the run.

## Editor Support

### VS Code

The extension in [`editors/vscode/`](editors/vscode) adds TestFerrum suites to the Test Explorer, with run and debug buttons at each declaration. It debugs through CodeLLDB.

### `--list` format

Editors discover tests by running a binary with `--list`. Fields are tab-separated; tabs, newlines and backslashes inside names are escaped as `\t`, `\n` and `\\`.

```
testferrum-list 1
suite	<suite>	<file>	<line>
test	<suite>	<test>	<file>	<line>
error	<suite>	<message>
```

The first line names the format version. `error` lines report duplicate suite or test names. Locations come from `#[track_caller]` on `suite()` and `test()`, so generated tests point at the line that built them.

## Releases

Pushing a tag such as `0.2.0` (no `v` prefix) runs `.github/workflows/release.yml`. It checks that `Cargo.toml` and `editors/vscode/package.json` both carry that version, runs the crate and extension checks, then creates a GitHub Release with `testferrum-<version>.crate` and `testferrum-<version>.vsix` attached. Nothing is published to crates.io or the VS Code Marketplace.

## API Reference

### Results (`testferrum`)

| Function | Purpose |
|---|---|
| `pass()` | Passing result |
| `fail(message)` | Failure with a message |
| `fail_with(message, expected, actual)` | Failure with expected and actual values |
| `skip(reason)` | Skipped result |
| `combine(first, second)` | Run both; collect errors |
| `and_then(result, next)` | Run `next` only if `result` passed |

### Assertions (`testferrum::asserts`)

| Group | Functions |
|---|---|
| Equality | `assert_equal`, `assert_not_equal` |
| Boolean | `assert_true`, `assert_false` |
| Option | `assert_some`, `assert_none` |
| Result | `assert_ok`, `assert_error` |
| Collections | `assert_empty`, `assert_not_empty`, `assert_len`, `assert_contains`, `assert_not_contains`, `assert_subset`, `assert_not_subset`, `assert_elements_match` |
| Numeric | `assert_greater`, `assert_greater_or_equal`, `assert_less`, `assert_less_or_equal`, `assert_in_delta` |
| Strings | `assert_regexp`, `assert_not_regexp` |

### Runner (`testferrum::runner`)

| Item | Purpose |
|---|---|
| `test(name, function)` | Build a test |
| `suite(name, setup, teardown, tests)` | Build a suite |
| `run_all(suites, filter, clock)` | Run sequentially |
| `run_all_parallel(suites, filter, clock)` | Run suites in parallel |
| `system_clock` | Real monotonic clock in milliseconds; pass your own `fn() -> f64` for deterministic timing |
| `Filter` | `All`, `Suite`, `Test`, `Match` |

### Output (`testferrum::cli`, `testferrum::junit`)

| Function | Purpose |
|---|---|
| `run_cli(suites)` | Parse arguments, run, print or write the report, return the exit code |
| `parse_args(arguments)` | Parse arguments into a `Command` |
| `format_suite`, `format_summary` | Console text |
| `to_junit(outcomes)` | JUnit XML text |

## License

MIT
