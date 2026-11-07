// Assertions take values by value so calls read naturally: `assert_equal(4, 2 + 2, "sum")`.
#![expect(clippy::needless_pass_by_value, reason = "by-value arguments keep assertion calls free of `&`")]

use crate::{fail, fail_with, pass, TestResult};
use regex::Regex;
use std::fmt::Debug;

fn show(value: &impl Debug) -> String {
    format!("{value:?}")
}

// Equality and boolean

pub fn assert_equal<T: PartialEq + Debug>(expected: T, actual: T, message: &str) -> TestResult {
    if expected == actual { pass() } else { fail_with(message, &show(&expected), &show(&actual)) }
}

pub fn assert_not_equal<T: PartialEq + Debug>(unexpected: T, actual: T, message: &str) -> TestResult {
    if unexpected == actual { fail(&format!("{message}: got unexpected value {actual:?}")) } else { pass() }
}

pub fn assert_true(condition: bool, message: &str) -> TestResult {
    if condition { pass() } else { fail(message) }
}

pub fn assert_false(condition: bool, message: &str) -> TestResult {
    if condition { fail(message) } else { pass() }
}

// Option and Result

pub fn assert_some<T>(option: &Option<T>, message: &str) -> TestResult {
    match option {
        Some(_) => pass(),
        None => fail(&format!("{message}: expected Some, got None")),
    }
}

pub fn assert_none<T: Debug>(option: &Option<T>, message: &str) -> TestResult {
    match option {
        None => pass(),
        Some(value) => fail(&format!("{message}: expected None, got Some({value:?})")),
    }
}

pub fn assert_ok<T, E: Debug>(result: &Result<T, E>, message: &str) -> TestResult {
    match result {
        Ok(_) => pass(),
        Err(error) => fail(&format!("{message}: expected Ok, got Err({error:?})")),
    }
}

pub fn assert_error<T: Debug, E>(result: &Result<T, E>, message: &str) -> TestResult {
    match result {
        Err(_) => pass(),
        Ok(value) => fail(&format!("{message}: expected Err, got Ok({value:?})")),
    }
}

// Collections

pub fn assert_empty<T: Debug>(items: &[T], message: &str) -> TestResult {
    if items.is_empty() { pass() } else { fail(&format!("{message}: expected empty, got {items:?}")) }
}

pub fn assert_not_empty<T>(items: &[T], message: &str) -> TestResult {
    if items.is_empty() { fail(&format!("{message}: should not be empty")) } else { pass() }
}

pub fn assert_len<T>(expected: usize, items: &[T], message: &str) -> TestResult {
    if items.len() == expected {
        pass()
    } else {
        fail_with(&format!("{message}: length mismatch"), &expected.to_string(), &items.len().to_string())
    }
}

pub fn assert_contains<T: PartialEq + Debug>(element: &T, items: &[T], message: &str) -> TestResult {
    if items.contains(element) { pass() } else { fail(&format!("{message}: {element:?} not found in {items:?}")) }
}

pub fn assert_not_contains<T: PartialEq + Debug>(element: &T, items: &[T], message: &str) -> TestResult {
    if items.contains(element) { fail(&format!("{message}: {element:?} should not be in {items:?}")) } else { pass() }
}

pub fn assert_subset<T: PartialEq>(subset: &[T], items: &[T], message: &str) -> TestResult {
    if subset.iter().all(|element| items.contains(element)) {
        pass()
    } else {
        fail(&format!("{message}: not all elements of subset found in collection"))
    }
}

pub fn assert_not_subset<T: PartialEq>(subset: &[T], items: &[T], message: &str) -> TestResult {
    if subset.iter().all(|element| items.contains(element)) {
        fail(&format!("{message}: subset should not be contained in collection"))
    } else {
        pass()
    }
}

/// Same elements, any order, duplicates counted.
pub fn assert_elements_match<T: PartialEq + Debug>(first: &[T], second: &[T], message: &str) -> TestResult {
    if first.len() != second.len() {
        return fail_with(&format!("{message}: different lengths"), &first.len().to_string(), &second.len().to_string());
    }
    let mut used = vec![false; second.len()];
    for element in first {
        let found = (0..second.len()).find(|&index| !used[index] && second[index] == *element);
        match found {
            Some(index) => used[index] = true,
            None => return fail_with(&format!("{message}: elements don't match"), &show(&first), &show(&second)),
        }
    }
    pass()
}

// Numeric

pub fn assert_greater<T: PartialOrd + Debug>(actual: T, than: T, message: &str) -> TestResult {
    if actual > than { pass() } else { fail_with(&format!("{message}: not greater"), &format!("> {than:?}"), &show(&actual)) }
}

pub fn assert_greater_or_equal<T: PartialOrd + Debug>(actual: T, than: T, message: &str) -> TestResult {
    if actual >= than { pass() } else { fail_with(&format!("{message}: not greater or equal"), &format!(">= {than:?}"), &show(&actual)) }
}

pub fn assert_less<T: PartialOrd + Debug>(actual: T, than: T, message: &str) -> TestResult {
    if actual < than { pass() } else { fail_with(&format!("{message}: not less"), &format!("< {than:?}"), &show(&actual)) }
}

pub fn assert_less_or_equal<T: PartialOrd + Debug>(actual: T, than: T, message: &str) -> TestResult {
    if actual <= than { pass() } else { fail_with(&format!("{message}: not less or equal"), &format!("<= {than:?}"), &show(&actual)) }
}

pub fn assert_in_delta(expected: f64, actual: f64, delta: f64, message: &str) -> TestResult {
    let difference = (expected - actual).abs();
    if difference <= delta {
        pass()
    } else {
        fail(&format!("{message}: difference {difference} exceeds delta {delta}"))
    }
}

// Regex

pub fn assert_regexp(pattern: &str, text: &str, message: &str) -> TestResult {
    match Regex::new(pattern) {
        Ok(regex) if regex.is_match(text) => pass(),
        Ok(_) => fail(&format!("{message}: '{text}' doesn't match pattern '{pattern}'")),
        Err(error) => fail(&format!("{message}: invalid pattern '{pattern}': {error}")),
    }
}

pub fn assert_not_regexp(pattern: &str, text: &str, message: &str) -> TestResult {
    match Regex::new(pattern) {
        Ok(regex) if regex.is_match(text) => fail(&format!("{message}: '{text}' should not match pattern '{pattern}'")),
        Ok(_) => pass(),
        Err(error) => fail(&format!("{message}: invalid pattern '{pattern}': {error}")),
    }
}

#[cfg(test)]
mod checks {
    use super::*;

    #[test]
    fn equality_and_collections() {
        assert_eq!(assert_equal(4, 2 + 2, "sum"), pass());
        assert!(matches!(assert_equal(4, 5, "sum"), TestResult::Fail(_)));
        assert_eq!(assert_elements_match(&[1, 2, 2], &[2, 1, 2], "bag"), pass());
        assert!(matches!(assert_elements_match(&[1, 1, 2], &[1, 2, 2], "bag"), TestResult::Fail(_)));
        assert_eq!(assert_subset(&[1, 2], &[3, 2, 1], "sub"), pass());
    }

    #[test]
    fn numeric_and_regex() {
        assert_eq!(assert_in_delta(1.0, 1.05, 0.1, "close"), pass());
        assert_eq!(assert_greater(3, 2, "gt"), pass());
        assert_eq!(assert_regexp(r"^\d+$", "123", "digits"), pass());
        assert!(matches!(assert_regexp("(", "x", "bad"), TestResult::Fail(_)));
    }
}
