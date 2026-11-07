use crate::cli::{count, format_error};
use crate::runner::{SuiteOutcome, TestOutcome};
use crate::TestResult;

/// Escapes XML special characters. Control characters other than tab, newline and
/// carriage return are invalid in XML 1.0 even when escaped, so they become U+FFFD.
fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            '\t' | '\n' | '\r' => escaped.push(character),
            control if control.is_control() && (control as u32) < 0x20 => escaped.push('\u{FFFD}'),
            other => escaped.push(other),
        }
    }
    escaped
}

fn seconds(milliseconds: f64) -> String {
    format!("{:.3}", milliseconds / 1000.0)
}

fn testcase(outcome: &TestOutcome) -> String {
    let name = escape(&outcome.name);
    let time = seconds(outcome.duration_ms);
    match &outcome.result {
        TestResult::Pass => format!("    <testcase name=\"{name}\" time=\"{time}\"/>"),
        TestResult::Skip(reason) => format!(
            "    <testcase name=\"{name}\" time=\"0\">\n      <skipped message=\"{}\"/>\n    </testcase>",
            escape(reason)
        ),
        TestResult::Fail(errors) => {
            let messages: Vec<&str> = errors.iter().map(|error| error.message.as_str()).collect();
            let details: Vec<String> = errors.iter().map(format_error).collect();
            format!(
                "    <testcase name=\"{name}\" time=\"{time}\">\n      <failure message=\"{}\" type=\"AssertionError\">{}</failure>\n    </testcase>",
                escape(&messages.join("; ")),
                escape(&details.join("\n"))
            )
        }
    }
}

fn testsuite(suite: &SuiteOutcome) -> String {
    let counts = count(std::slice::from_ref(suite));
    let cases: Vec<String> = suite.outcomes.iter().map(testcase).collect();
    format!(
        "  <testsuite name=\"{}\" tests=\"{}\" failures=\"{}\" errors=\"0\" skipped=\"{}\" time=\"{}\">\n{}\n  </testsuite>",
        escape(&suite.name),
        suite.outcomes.len(),
        counts.failed,
        counts.skipped,
        seconds(suite.duration_ms),
        cases.join("\n")
    )
}

/// One `JUnit` document covering every suite.
#[must_use]
pub fn to_junit(suites: &[SuiteOutcome]) -> String {
    let counts = count(suites);
    let total = counts.passed + counts.failed + counts.skipped;
    let body: Vec<String> = suites.iter().map(testsuite).collect();
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<testsuites tests=\"{total}\" failures=\"{}\" errors=\"0\" skipped=\"{}\" time=\"{}\">\n{}\n</testsuites>\n",
        counts.failed,
        counts.skipped,
        seconds(counts.duration_ms),
        body.join("\n")
    )
}

#[cfg(test)]
mod checks {
    use super::*;
    use crate::{fail_with, pass, skip};

    #[test]
    fn writes_one_document() {
        let suites = vec![SuiteOutcome {
            name: "Math & <More>".to_string(),
            outcomes: vec![
                TestOutcome { name: "adds".to_string(), result: pass(), duration_ms: 1.0 },
                TestOutcome { name: "later".to_string(), result: skip("not \"ready\""), duration_ms: 0.0 },
                TestOutcome { name: "breaks".to_string(), result: fail_with("sum", "4", "5"), duration_ms: 2.0 },
            ],
            duration_ms: 3.0,
        }];
        let xml = to_junit(&suites);
        assert!(xml.contains("<testsuites tests=\"3\" failures=\"1\" errors=\"0\" skipped=\"1\" time=\"0.003\">"));
        assert!(xml.contains("<testsuite name=\"Math &amp; &lt;More&gt;\""));
        assert!(xml.contains("<skipped message=\"not &quot;ready&quot;\"/>"));
        assert!(xml.contains("<failure message=\"sum\" type=\"AssertionError\">"));
    }

    #[test]
    fn replaces_invalid_control_characters() {
        assert_eq!(escape("a\u{1b}[31mb\tc"), "a\u{FFFD}[31mb\tc");
    }
}
