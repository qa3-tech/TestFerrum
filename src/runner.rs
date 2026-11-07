use crate::TestResult;
use std::sync::OnceLock;
use std::time::Instant;

/// Milliseconds on a monotonic clock. Injected so timing stays at the edge
/// and checks can use a fake clock.
pub type Clock = fn() -> f64;

/// Real monotonic clock, measured from its first use.
#[must_use]
pub fn system_clock() -> f64 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f64() * 1000.0
}

/// Where a test or suite was declared, captured by `#[track_caller]`.
#[derive(Debug, Clone, PartialEq)]
pub struct Location {
    pub file: &'static str,
    pub line: u32,
}

pub struct Test<Env> {
    pub name: String,
    pub location: Location,
    pub run: Box<dyn Fn(&Env) -> TestResult + Send>,
}

/// A test's name and location, available without running anything.
#[derive(Debug, Clone, PartialEq)]
pub struct TestInfo {
    pub name: String,
    pub location: Location,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Filter {
    All,
    Suite(String),
    Test { suite: String, test: String },
    Match(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct TestOutcome {
    pub name: String,
    pub result: TestResult,
    pub duration_ms: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SuiteOutcome {
    pub name: String,
    pub outcomes: Vec<TestOutcome>,
    pub duration_ms: f64,
}

/// Runs a suite's selected tests and returns their outcomes.
pub type SuiteRun = Box<dyn FnOnce(&Filter, Clock) -> SuiteOutcome + Send>;

/// A suite with its env type hidden, so suites with different envs share one list.
pub struct Suite {
    pub name: String,
    pub location: Location,
    pub tests: Vec<TestInfo>,
    pub run: SuiteRun,
}

#[track_caller]
fn caller_location() -> Location {
    let caller = std::panic::Location::caller();
    Location { file: caller.file(), line: caller.line() }
}

#[must_use]
#[track_caller]
pub fn test<Env>(name: &str, run: impl Fn(&Env) -> TestResult + Send + 'static) -> Test<Env> {
    Test { name: name.to_string(), location: caller_location(), run: Box::new(run) }
}

fn selected(filter: &Filter, suite_name: &str, test_name: &str) -> bool {
    match filter {
        Filter::All => true,
        Filter::Suite(suite) => suite == suite_name,
        Filter::Test { suite, test } => suite == suite_name && test == test_name,
        Filter::Match(pattern) => test_name.contains(pattern.as_str()),
    }
}

/// Setup runs once before the tests, teardown once after; tests run in order.
/// Setup is skipped when the filter selects no tests.
#[must_use]
#[track_caller]
pub fn suite<Env: 'static>(
    name: &str,
    setup: fn() -> Env,
    teardown: fn(Env),
    tests: Vec<Test<Env>>,
) -> Suite {
    let location = caller_location();
    let infos: Vec<TestInfo> = tests
        .iter()
        .map(|test| TestInfo { name: test.name.clone(), location: test.location.clone() })
        .collect();
    let suite_name = name.to_string();
    let run = move |filter: &Filter, clock: Clock| {
        let suite_start = clock();
        let chosen: Vec<Test<Env>> = tests
            .into_iter()
            .filter(|test| selected(filter, &suite_name, &test.name))
            .collect();

        let mut outcomes = Vec::new();
        if !chosen.is_empty() {
            let env = setup();
            for test in chosen {
                let test_start = clock();
                let result = (test.run)(&env);
                outcomes.push(TestOutcome { name: test.name, result, duration_ms: clock() - test_start });
            }
            teardown(env);
        }
        SuiteOutcome { name: suite_name, outcomes, duration_ms: clock() - suite_start }
    };
    Suite { name: name.to_string(), location, tests: infos, run: Box::new(run) }
}

/// Suites with no selected tests are left out of the outcome.
#[must_use]
pub fn run_all(suites: Vec<Suite>, filter: &Filter, clock: Clock) -> Vec<SuiteOutcome> {
    suites
        .into_iter()
        .map(|suite| (suite.run)(filter, clock))
        .filter(|outcome| !outcome.outcomes.is_empty())
        .collect()
}

/// Each suite runs on its own thread; tests within a suite stay sequential.
/// A panic in any suite is re-raised here. Outcome order matches suite order.
#[must_use]
pub fn run_all_parallel(suites: Vec<Suite>, filter: &Filter, clock: Clock) -> Vec<SuiteOutcome> {
    std::thread::scope(|scope| {
        let handles: Vec<_> = suites
            .into_iter()
            .map(|suite| scope.spawn(move || (suite.run)(filter, clock)))
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap_or_else(|payload| std::panic::resume_unwind(payload)))
            .filter(|outcome| !outcome.outcomes.is_empty())
            .collect()
    })
}

#[cfg(test)]
mod checks {
    use super::*;
    use crate::asserts::assert_equal;
    use crate::{pass, skip};
    use std::rc::Rc;

    struct Counter {
        start: i32,
        not_send: Rc<i32>, // env needs no Send: built inside the suite's thread
    }

    fn sample_suites() -> Vec<Suite> {
        vec![
            suite(
                "Math",
                || (),
                |()| (),
                vec![
                    test("adds", |()| assert_equal(4, 2 + 2, "sum")),
                    test("later", |()| skip("not ready")),
                ],
            ),
            suite(
                "Env",
                || Counter { start: 10, not_send: Rc::new(1) },
                |_| (),
                vec![test("reads env", |env: &Counter| assert_equal(11, env.start + *env.not_send, "env"))],
            ),
        ]
    }

    #[test]
    fn runs_sequential_and_parallel_alike() {
        let sequential = run_all(sample_suites(), &Filter::All, system_clock);
        let parallel = run_all_parallel(sample_suites(), &Filter::All, system_clock);
        assert_eq!(sequential.len(), 2);
        assert_eq!(sequential[0].outcomes[0].result, pass());
        assert_eq!(sequential[0].outcomes[1].result, skip("not ready"));
        assert_eq!(sequential[1].outcomes[0].result, pass());
        assert_eq!(parallel.len(), 2);
        assert_eq!(parallel[1].name, "Env");
    }

    #[test]
    fn filters() {
        let only_env = run_all(sample_suites(), &Filter::Suite("Env".to_string()), system_clock);
        assert_eq!(only_env.len(), 1);
        let matched = run_all(sample_suites(), &Filter::Match("add".to_string()), system_clock);
        assert_eq!(matched[0].outcomes.len(), 1);
    }

    fn ticking_clock() -> f64 {
        use std::sync::atomic::{AtomicU64, Ordering};
        static TICKS: AtomicU64 = AtomicU64::new(0);
        #[allow(clippy::cast_precision_loss)]
        let ticks = TICKS.fetch_add(1, Ordering::SeqCst) as f64;
        ticks * 10.0
    }

    #[test]
    fn durations_come_from_the_injected_clock() {
        let outcomes = run_all(sample_suites(), &Filter::Suite("Env".to_string()), ticking_clock);
        // suite start, test start, test end, suite end: each read advances 10 ms
        assert!((outcomes[0].outcomes[0].duration_ms - 10.0).abs() < f64::EPSILON);
        assert!((outcomes[0].duration_ms - 30.0).abs() < f64::EPSILON);
    }
}
