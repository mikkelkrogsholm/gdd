//! Check the fixed concert cases against the archived phase-A and phase-B outcomes.

use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};

// The two passing cases have no phase-A assertion mismatch. Phase B passes all ten.
const LIMIT: &str = "Err(ActiveReservationLimitReached)";
const CASES: [(&str, Option<(&str, &str)>); 10] = [
    (
        "global_limit_rejects_third_across_concerts_and_leaves_key_unbound",
        Some(("Ok(Reserved)", LIMIT)),
    ),
    (
        "principal_allowances_are_independent",
        Some(("Ok(Reserved)", LIMIT)),
    ),
    (
        "cancellation_frees_allowance_and_failed_key_can_then_succeed",
        Some(("Ok(Reserved)", LIMIT)),
    ),
    (
        "active_and_cancelled_successful_replay_precede_limit_and_capacity",
        None,
    ),
    (
        "precedence_is_input_then_replay_then_existence_then_limit_then_capacity",
        Some(("Err(SoldOut)", LIMIT)),
    ),
    (
        "concurrent_new_keys_across_concerts_share_one_remaining_allowance",
        Some(("2", "1")),
    ),
    ("concurrent_replay_remains_one_reservation_at_limit", None),
    (
        "server_exposes_single_current_policy_value",
        Some(("Some(Null)", "Some(Number(2))")),
    ),
    (
        "incompatible_existing_a_data_is_rejected_without_history_mutation",
        Some(("Ok(())", "Err(PolicyMigrationRequired)")),
    ),
    (
        "compatible_a_history_and_cancelled_keys_survive_activation",
        Some(("None", "Some(2)")),
    ),
];

fn case_result(lines: &[&str], name: &str, expected: &str) -> Result<Value> {
    let prefix = format!("test {name} ... ");
    let actual = lines.iter().find_map(|line| line.strip_prefix(&prefix));
    ensure!(
        actual == Some(expected),
        "expected {name} to report {expected}"
    );
    Ok(json!({ "name": name, "outcome": actual }))
}

pub(super) fn confirm(stdout: &str, phase_a: bool) -> Result<Vec<Value>> {
    let lines: Vec<_> = stdout.lines().collect();
    let summary = if phase_a {
        "test result: FAILED. 2 passed; 8 failed; 0 ignored; 0 measured; 0 filtered out;"
    } else {
        "test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out;"
    };
    let starts = |prefix: &str| lines.iter().filter(|line| line.starts_with(prefix)).count();
    ensure!(
        starts("running ") == 1
            && lines.contains(&"running 10 tests")
            && starts("test ") == CASES.len() + 1
            && starts("test result:") == 1
            && starts(summary) == 1,
        "expected exactly ten concert test results and '{summary}'"
    );
    let failures = if phase_a { 8 } else { 0 };
    ensure!(
        starts("---- ") == failures && stdout.matches("panicked at").count() == failures,
        "expected {failures} assertion failures in the concert test report"
    );
    let mut outcomes = Vec::new();
    for (name, phase_a_mismatch) in CASES {
        let mismatch = if phase_a { phase_a_mismatch } else { None };
        let outcome = if mismatch.is_some() { "FAILED" } else { "ok" };
        let mut case = case_result(&lines, name, outcome)?;
        if let Some((left, right)) = mismatch {
            let header = format!("---- {name} stdout ----");
            let mut report = stdout.lines().skip_while(|&line| line != header).skip(1);
            ensure!(report.next() == Some(""), "missing panic report for {name}");
            // libtest identifies the case in its capture header. Rust may remap
            // the panic location or include a platform-specific thread ID.
            let equality_assertion = report
                .next()
                .is_some_and(|line| line.contains("panicked at "))
                && report.next() == Some("assertion `left == right` failed");
            let observed = (
                report.next().and_then(|line| line.strip_prefix("  left: ")),
                report.next().and_then(|line| line.strip_prefix(" right: ")),
            );
            ensure!(
                equality_assertion && observed == (Some(left), Some(right)),
                "expected {name} to fail its requirement assertion with left {left} and right {right}"
            );
            case["assertion"] = json!({ "left": observed.0, "right": observed.1 });
        }
        outcomes.push(case);
    }
    Ok(outcomes)
}

pub(super) const TRANSITION: &str = "phase_a_to_b_requires_explicit_reconciliation";

pub(super) fn confirm_transition(stdout: &str) -> Result<Value> {
    let lines: Vec<_> = stdout.lines().collect();
    let running: Vec<_> = lines
        .iter()
        .copied()
        .filter(|s| s.starts_with("running "))
        .collect();
    let results: Vec<_> = lines
        .iter()
        .copied()
        .filter(|s| s.starts_with("test "))
        .collect();
    ensure!(
        running == ["running 1 test"] && results.len() == 2,
        "expected exactly one transition test"
    );
    let case = case_result(&results, TRANSITION, "ok")?;
    const PREFIX: &str = "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; ";
    let filtered = results
        .iter()
        .find_map(|line| line.strip_prefix(PREFIX))
        .and_then(|rest| rest.split_once(" filtered out;"))
        .context("expected a passing transition test summary")?
        .0;
    filtered
        .parse::<usize>()
        .context("invalid transition test summary")?;
    ensure!(
        !stdout.contains("panicked at"),
        "unexpected transition test panic"
    );
    Ok(case)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt::Write;

    fn report(phase_a: bool) -> String {
        let mut output = "running 10 tests\n".to_owned();
        for (name, mismatch) in CASES {
            let failed = phase_a && mismatch.is_some();
            writeln!(
                output,
                "test {name} ... {}",
                if failed { "FAILED" } else { "ok" }
            )
            .unwrap();
            if phase_a && let Some((left, right)) = mismatch {
                writeln!(output, "---- {name} stdout ----\n\nthread '{name}' (123) panicked at tests/requirement_change.rs:20:3:\nassertion `left == right` failed\n  left: {left}\n right: {right}").unwrap();
            }
        }
        let totals = if phase_a {
            "FAILED. 2 passed; 8 failed"
        } else {
            "ok. 10 passed; 0 failed"
        };
        writeln!(
            output,
            "test result: {totals}; 0 ignored; 0 measured; 0 filtered out; finished in 0.20s"
        )
        .unwrap();
        output
    }

    #[test]
    fn rejects_filtered_missing_and_unexpected_tests() {
        let green = report(false);
        assert!(confirm(&green, false).is_ok());
        for output in [
            green.replace("0 filtered out", "1 filtered out"),
            green.replace(CASES[1].0, CASES[0].0),
            green.replace(" ... ok", " ... ignored"),
            green.replace("running 10 tests", "running 0 tests"),
            format!("{green}test extra ... ok\n"),
            format!("{green}unexpected thread panicked at src/lib.rs:1:2\n"),
        ] {
            assert!(confirm(&output, false).is_err());
        }
    }

    #[test]
    fn transition_requires_the_named_test_and_a_single_pass() {
        let report = format!(
            "running 1 test\ntest {TRANSITION} ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 12 filtered out; finished in 0.05s\n"
        );
        assert_eq!(
            confirm_transition(&report).unwrap(),
            json!({"name": TRANSITION, "outcome": "ok"})
        );
        for invalid in [
            report.replace(TRANSITION, "other_test"),
            report.replace("running 1 test", "running 0 tests"),
            report.replace(" ... ok", " ... ignored"),
            report.replace("1 passed", "0 passed"),
            report.replace("0 failed", "1 failed"),
            format!("{report}test another_case ... ok\n"),
        ] {
            assert!(confirm_transition(&invalid).is_err(), "{invalid}");
        }
    }

    #[test]
    fn red_requires_each_specific_assertion_failure() {
        let red = report(true);
        assert!(confirm(&red, true).is_ok());
        let remapped = red
            .replace("tests/requirement_change.rs", "/mapped/source.rs")
            .replace(" (123) panicked at", " panicked at");
        assert!(confirm(&remapped, true).is_ok());
        for output in [
            String::from("error: could not compile gdd-concert"),
            red.replace("Err(SoldOut)", "Err(PersistenceUnavailable)"),
            red.replace("panicked at ", "no panic here "),
            red.replace("assertion `left == right` failed", "storage unavailable"),
            red.replace(
                "---- principal_allowances_are_independent stdout ----",
                "---- other stdout ----",
            ),
            red.replace("Err(PolicyMigrationRequired)", "Ok(())"),
            red.replace("right: Some(Number(2))", "right: None"),
            red.replace("  left: 2", "  left: 0"),
            red.replace("test result:", "early exit:"),
            format!("{red}extra thread panicked at src/lib.rs:1:2\n"),
        ] {
            assert!(confirm(&output, true).is_err());
        }
        assert!(confirm(&red, false).is_err());
        assert!(confirm(&report(false), true).is_err());
    }
}
