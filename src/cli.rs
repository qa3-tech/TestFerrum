use crate::junit::to_junit;
use crate::runner::{run_all, run_all_parallel, system_clock, Filter, Location, Suite, SuiteOutcome, TestOutcome};
use crate::{TestError, TestResult};
use std::io::IsTerminal;
use std::process::ExitCode;

#[derive(Debug, Clone, PartialEq)]
pub struct RunOptions {
    pub filter: Filter,
    pub parallel: bool,
    pub junit_file: Option<String>,
    /// Tests that take longer than this many milliseconds are marked slow.
    pub slow_ms: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Help,
    List,
    Run(RunOptions),
    Invalid(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Counts {
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub duration_ms: f64,
}

const HELP: &str = "Usage:
  <no args>                   Run all suites sequentially
  --suite <name>              Run specific suite
  --test <suite> <test>       Run specific test
  --match <pattern>           Run tests matching pattern
  --parallel                  Run suites in parallel (combines with the above)
  --junit <file>              Write JUnit XML to file instead of console output
  --slow <ms>                 Mark tests slower than <ms> milliseconds
  --list                      List suites and tests with source locations
  --help, -h                  Show this help";

/// Parses arguments after the program name.
#[must_use]
pub fn parse_args(arguments: &[String]) -> Command {
    let mut options = RunOptions { filter: Filter::All, parallel: false, junit_file: None, slow_ms: None };
    let mut index = 0;
    while index < arguments.len() {
        let remaining = &arguments[index..];
        match remaining[0].as_str() {
            "--help" | "-h" => return Command::Help,
            "--list" => return Command::List,
            "--parallel" => {
                options.parallel = true;
                index += 1;
            }
            "--suite" if remaining.len() >= 2 => {
                options.filter = Filter::Suite(remaining[1].clone());
                index += 2;
            }
            "--test" if remaining.len() >= 3 => {
                options.filter = Filter::Test { suite: remaining[1].clone(), test: remaining[2].clone() };
                index += 3;
            }
            "--junit" if remaining.len() >= 2 => {
                options.junit_file = Some(remaining[1].clone());
                index += 2;
            }
            "--slow" if remaining.len() >= 2 => match remaining[1].parse::<f64>() {
                Ok(milliseconds) if milliseconds >= 0.0 => {
                    options.slow_ms = Some(milliseconds);
                    index += 2;
                }
                _ => return Command::Invalid(format!("--slow needs a number of milliseconds, got: {}", remaining[1])),
            },
            "--match" if remaining.len() >= 2 => {
                options.filter = Filter::Match(remaining[1].clone());
                index += 2;
            }
            unknown => return Command::Invalid(format!("Unknown or incomplete argument: {unknown}. Use --help for usage.")),
        }
    }
    Command::Run(options)
}

/// Version of the `--list` format. Bump it when the format changes.
pub const LIST_VERSION: u32 = 1;

/// Escapes a list field so tabs and newlines cannot break the line format.
fn list_field(text: &str) -> String {
    text.replace('\\', "\\\\").replace('\t', "\\t").replace('\n', "\\n").replace('\r', "\\r")
}

fn list_location(location: &Location) -> String {
    format!("{}\t{}", list_field(location.file), location.line)
}

/// Machine-readable listing for editors. Fields are tab-separated:
///
/// ```text
/// testferrum-list 1
/// suite  <suite>  <file>  <line>
/// test   <suite>  <test>  <file>  <line>
/// error  <suite>  <message>
/// ```
#[must_use]
pub fn format_list(suites: &[Suite]) -> String {
    let mut lines = vec![format!("testferrum-list {LIST_VERSION}")];
    let mut seen_suites: Vec<&str> = Vec::new();
    for suite in suites {
        let suite_name = list_field(&suite.name);
        if seen_suites.contains(&suite.name.as_str()) {
            lines.push(format!("error\t{suite_name}\tduplicate suite name"));
        }
        seen_suites.push(&suite.name);
        lines.push(format!("suite\t{suite_name}\t{}", list_location(&suite.location)));

        let mut seen_tests: Vec<&str> = Vec::new();
        for test in &suite.tests {
            if seen_tests.contains(&test.name.as_str()) {
                lines.push(format!("error\t{suite_name}\tduplicate test name: {}", list_field(&test.name)));
            }
            seen_tests.push(&test.name);
            lines.push(format!("test\t{suite_name}\t{}\t{}", list_field(&test.name), list_location(&test.location)));
        }
    }
    lines.join("\n") + "\n"
}

#[must_use]
pub fn format_duration(milliseconds: f64) -> String {
    if milliseconds < 1.0 {
        format!("{milliseconds:.2} ms")
    } else if milliseconds < 1000.0 {
        format!("{milliseconds:.0} ms")
    } else {
        format!("{:.2} s", milliseconds / 1000.0)
    }
}

fn colorize(use_color: bool, color_code: &str, text: &str) -> String {
    if use_color { format!("\u{1b}[{color_code}m{text}\u{1b}[0m") } else { text.to_string() }
}

#[must_use]
pub fn format_error(error: &TestError) -> String {
    if error.expected.is_empty() && error.actual.is_empty() {
        format!("  {}", error.message)
    } else {
        format!("  {}\n    Expected: {}\n    Actual:   {}", error.message, error.expected, error.actual)
    }
}

#[must_use]
pub fn is_slow(outcome: &TestOutcome, slow_ms: Option<f64>) -> bool {
    match (slow_ms, &outcome.result) {
        (_, TestResult::Skip(_)) | (None, _) => false,
        (Some(threshold), _) => outcome.duration_ms > threshold,
    }
}

#[must_use]
pub fn format_outcome(outcome: &TestOutcome, use_color: bool, slow_ms: Option<f64>) -> String {
    let slow_marker = if is_slow(outcome, slow_ms) { format!(" {}", colorize(use_color, "33", "[slow]")) } else { String::new() };
    let duration = format!("{}{slow_marker}", format_duration(outcome.duration_ms));
    match &outcome.result {
        TestResult::Pass => format!("{} {} ({duration})", colorize(use_color, "32", "✓"), outcome.name),
        TestResult::Skip(reason) => format!("{} {} (skipped: {reason})", colorize(use_color, "90", "○"), outcome.name),
        TestResult::Fail(errors) => {
            let details: Vec<String> = errors.iter().map(format_error).collect();
            format!("{} {} ({duration})\n{}", colorize(use_color, "31", "✕"), outcome.name, details.join("\n"))
        }
    }
}

#[must_use]
pub fn format_suite(suite: &SuiteOutcome, use_color: bool, slow_ms: Option<f64>) -> String {
    let lines: Vec<String> = suite.outcomes.iter().map(|outcome| format_outcome(outcome, use_color, slow_ms)).collect();
    format!("\n=== {} ({}) ===\n{}", suite.name, format_duration(suite.duration_ms), lines.join("\n"))
}

#[must_use]
pub fn count(suites: &[SuiteOutcome]) -> Counts {
    let mut counts = Counts { passed: 0, failed: 0, skipped: 0, duration_ms: 0.0 };
    for suite in suites {
        counts.duration_ms += suite.duration_ms;
        for outcome in &suite.outcomes {
            match outcome.result {
                TestResult::Pass => counts.passed += 1,
                TestResult::Fail(_) => counts.failed += 1,
                TestResult::Skip(_) => counts.skipped += 1,
            }
        }
    }
    counts
}

#[must_use]
pub fn count_slow(suites: &[SuiteOutcome], slow_ms: Option<f64>) -> usize {
    suites.iter().flat_map(|suite| &suite.outcomes).filter(|outcome| is_slow(outcome, slow_ms)).count()
}

/// Appended to the summary only when a slow threshold is set.
#[must_use]
pub fn format_slow_summary(slow_count: usize, slow_ms: Option<f64>) -> String {
    match slow_ms {
        Some(threshold) => format!(", {slow_count} slow (> {})", format_duration(threshold)),
        None => String::new(),
    }
}

#[must_use]
pub fn format_summary(counts: &Counts) -> String {
    let total = counts.passed + counts.failed + counts.skipped;
    format!(
        "\n{}/{} passed, {} failed, {} skipped (Total: {})",
        counts.passed, total, counts.failed, counts.skipped, format_duration(counts.duration_ms)
    )
}

/// Entry point: `fn main() -> ExitCode { run_cli(suites) }`. Exit code 1 on any failure.
pub fn run_cli(suites: Vec<Suite>) -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    match parse_args(&arguments) {
        Command::Help => {
            println!("{HELP}");
            ExitCode::SUCCESS
        }
        Command::List => {
            print!("{}", format_list(&suites));
            ExitCode::SUCCESS
        }
        Command::Invalid(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
        Command::Run(options) => {
            let outcomes = if options.parallel {
                run_all_parallel(suites, &options.filter, system_clock)
            } else {
                run_all(suites, &options.filter, system_clock)
            };
            let counts = count(&outcomes);
            if let Some(path) = options.junit_file {
                match std::fs::write(&path, to_junit(&outcomes)) {
                    Ok(()) => println!("Results written to {path}"),
                    Err(error) => {
                        eprintln!("Cannot write '{path}': {error}");
                        return ExitCode::FAILURE;
                    }
                }
            } else {
                let use_color = std::io::stdout().is_terminal();
                for suite in &outcomes {
                    println!("{}", format_suite(suite, use_color, options.slow_ms));
                }
                let slow_summary = format_slow_summary(count_slow(&outcomes, options.slow_ms), options.slow_ms);
                println!("{}{slow_summary}", format_summary(&counts));
            }
            if counts.failed == 0 { ExitCode::SUCCESS } else { ExitCode::FAILURE }
        }
    }
}

#[cfg(test)]
mod checks {
    use super::*;
    use crate::{fail_with, pass, skip};

    fn strings(words: &[&str]) -> Vec<String> {
        words.iter().map(ToString::to_string).collect()
    }

    fn run_options() -> RunOptions {
        RunOptions { filter: Filter::All, parallel: false, junit_file: None, slow_ms: None }
    }

    #[test]
    fn parses_arguments() {
        assert_eq!(parse_args(&[]), Command::Run(run_options()));
        assert_eq!(
            parse_args(&strings(&["--parallel", "--suite", "Math"])),
            Command::Run(RunOptions { filter: Filter::Suite("Math".to_string()), parallel: true, ..run_options() })
        );
        assert_eq!(
            parse_args(&strings(&["--test", "Math", "adds"])),
            Command::Run(RunOptions { filter: Filter::Test { suite: "Math".to_string(), test: "adds".to_string() }, ..run_options() })
        );
        assert_eq!(
            parse_args(&strings(&["--junit", "out.xml", "--slow", "250"])),
            Command::Run(RunOptions { junit_file: Some("out.xml".to_string()), slow_ms: Some(250.0), ..run_options() })
        );
        assert!(matches!(parse_args(&strings(&["--slow", "fast"])), Command::Invalid(_)));
        assert_eq!(parse_args(&strings(&["-h"])), Command::Help);
        assert_eq!(parse_args(&strings(&["--parallel", "--list"])), Command::List);
        assert!(matches!(parse_args(&strings(&["--suite"])), Command::Invalid(_)));
    }

    #[test]
    fn formats_and_counts() {
        let suite = SuiteOutcome {
            name: "Math".to_string(),
            outcomes: vec![
                TestOutcome { name: "adds".to_string(), result: pass(), duration_ms: 0.5 },
                TestOutcome { name: "later".to_string(), result: skip("not ready"), duration_ms: 0.0 },
                TestOutcome { name: "breaks".to_string(), result: fail_with("sum", "4", "5"), duration_ms: 2.0 },
            ],
            duration_ms: 3.0,
        };
        let text = format_suite(&suite, false, Some(1.0));
        assert!(text.contains("✓ adds (0.50 ms)"));
        assert!(text.contains("○ later (skipped: not ready)"));
        assert!(text.contains("✕ breaks (2 ms [slow])\n  sum\n    Expected: 4\n    Actual:   5"));
        let counts = count(std::slice::from_ref(&suite));
        assert_eq!(format_summary(&counts), "\n1/3 passed, 1 failed, 1 skipped (Total: 3 ms)");
        assert_eq!(format_slow_summary(count_slow(std::slice::from_ref(&suite), Some(1.0)), Some(1.0)), ", 1 slow (> 1 ms)");
        assert_eq!(format_slow_summary(0, None), "");
    }

    #[test]
    fn lists_with_locations_and_duplicates() {
        use crate::runner::{suite, test};
        let suites = vec![suite("Math", || (), |()| (), vec![
            test("adds", |()| crate::pass()),
            test("tab\there", |()| crate::pass()),
            test("adds", |()| crate::pass()),
        ])];
        let listing = format_list(&suites);
        let lines: Vec<&str> = listing.lines().collect();
        let this_file = file!();
        assert_eq!(lines[0], "testferrum-list 1");
        assert!(lines[1].starts_with(&format!("suite\tMath\t{this_file}\t")));
        assert!(lines[2].starts_with(&format!("test\tMath\tadds\t{this_file}\t")));
        assert!(lines[3].starts_with("test\tMath\ttab\\there\t"));
        assert_eq!(lines[4], "error\tMath\tduplicate test name: adds");
        let suite_line: u32 = lines[1].rsplit('\t').next().unwrap().parse().unwrap();
        let test_line: u32 = lines[2].rsplit('\t').next().unwrap().parse().unwrap();
        assert_eq!(test_line, suite_line + 1);
    }
}
