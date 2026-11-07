pub mod asserts;
pub mod junit;
pub mod cli;
pub mod runner;

#[derive(Debug, Clone, PartialEq)]
pub struct TestError {
    pub message: String,
    pub expected: String,
    pub actual: String,
}

#[must_use]
#[derive(Debug, Clone, PartialEq)]
pub enum TestResult {
    Pass,
    Fail(Vec<TestError>),
    Skip(String),
}

pub fn pass() -> TestResult {
    TestResult::Pass
}

pub fn fail(message: &str) -> TestResult {
    fail_with(message, "", "")
}

pub fn fail_with(message: &str, expected: &str, actual: &str) -> TestResult {
    TestResult::Fail(vec![TestError {
        message: message.to_string(),
        expected: expected.to_string(),
        actual: actual.to_string(),
    }])
}

pub fn skip(reason: &str) -> TestResult {
    TestResult::Skip(reason.to_string())
}

/// Runs both checks; collects every error. Skip wins over Pass, Fail wins over all.
pub fn combine(first: TestResult, second: TestResult) -> TestResult {
    match (first, second) {
        (TestResult::Fail(mut first_errors), TestResult::Fail(second_errors)) => {
            first_errors.extend(second_errors);
            TestResult::Fail(first_errors)
        }
        (TestResult::Fail(errors), _) | (_, TestResult::Fail(errors)) => TestResult::Fail(errors),
        (TestResult::Skip(reason), _) | (_, TestResult::Skip(reason)) => TestResult::Skip(reason),
        (TestResult::Pass, TestResult::Pass) => TestResult::Pass,
    }
}

/// Runs `next` only if `result` passed; short-circuits on Fail or Skip.
pub fn and_then(result: TestResult, next: impl FnOnce() -> TestResult) -> TestResult {
    match result {
        TestResult::Pass => next(),
        stopped => stopped,
    }
}

#[cfg(test)]
mod checks {
    use super::*;
    #[test]
    fn combine_collects() {
        let result = combine(fail("first"), fail("second"));
        assert!(matches!(result, TestResult::Fail(errors) if errors.len() == 2));
    }
    #[test]
    fn and_then_stops() {
        let result = and_then(fail("first"), || panic!("must not run"));
        assert!(matches!(result, TestResult::Fail(_)));
    }
    #[test]
    #[allow(unreachable_code)]
    fn skip_returns_early() {
        fn sample() -> TestResult { return skip("later"); fail("never") }
        assert_eq!(sample(), skip("later"));
    }
}
