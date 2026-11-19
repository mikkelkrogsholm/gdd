//! Check the fixed concert cases against the archived phase-A and phase-B outcomes.

use anyhow::{Result, ensure};

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

pub(super) fn confirm(stdout: &str, phase_a: bool) -> Result<()> {
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
    for (name, phase_a_mismatch) in CASES {
        let mismatch = if phase_a { phase_a_mismatch } else { None };
        let outcome = if mismatch.is_some() { "FAILED" } else { "ok" };
        ensure!(
            lines.contains(&format!("test {name} ... {outcome}").as_str()),
            "expected {name} to report {outcome}"
        );
        if let Some((left, right)) = mismatch {
            let header = format!("---- {name} stdout ----");
            let mut report = stdout.lines().skip_while(|&line| line != header).skip(1);
            ensure!(report.next() == Some(""), "missing panic report for {name}");
            // libtest identifies the case in its capture header. Rust may remap
            // the panic location or include a platform-specific thread ID.
            ensure!(
                report
                    .next()
                    .is_some_and(|line| line.contains("panicked at "))
                    && report.next() == Some("assertion `left == right` failed")
                    && report.next() == Some(&format!("  left: {left}"))
                    && report.next() == Some(&format!(" right: {right}")),
                "expected {name} to fail its requirement assertion with left {left} and right {right}"
            );
        }
    }
    Ok(())
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
