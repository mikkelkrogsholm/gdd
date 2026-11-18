//! Verify the collected Cargo evidence in the fresh, detached replay project.

use super::*;
use gdd::workflow_run::{RunPhase, run_status};
use std::collections::BTreeSet;

pub(super) fn native_demo(_: &Path, _: &str) -> Result<()> {
    let temp = tempfile::tempdir()?;
    let root = temp.path().join("demo");
    let summary = crate::workflow_demo::replay(&root)?;
    ensure!(summary["baseline_exit"] == 1 && summary["final_exit"] == 0);
    ensure!(summary["implementation"] == "implementation.quote_demo.rust");

    let host = Input::from(&read_document(&root.join("host.acceptance.json"))?);
    let status = run_status(&root.join("model.gdd.json"), &root.join("run"), Some(&host))?;
    ensure!(summary["acceptance_current"] == true && summary["events"] == 10);
    ensure!(
        status.state.status == RunPhase::Accepted
            && status.acceptance_current
            && status.event_count == 10
    );

    for (stage, passed, failed) in [
        ("baseline", 11, 7),
        ("discount", 5, 0),
        ("tax", 5, 0),
        ("quote", 9, 0),
        ("final", 19, 0),
    ] {
        let uri = format!("evidence/{stage}.run.json");
        let receipt = read_document(&root.join(&uri))?;
        let tests = receipt["tests"]
            .as_object()
            .context("missing Cargo test results")?;
        ensure!(receipt["executed"] == true && tests.len() == passed + failed);
        ensure!(tests.values().filter(|v| *v == "FAILED").count() == failed);
        ensure!(tests.values().filter(|v| *v == "ok").count() == passed);
        ensure!(receipt["code_revision"] == receipt["code_revision_after"]);
        match stage {
            "baseline" => ensure!(receipt["exit_code"] == 101),
            "final" => ensure!(
                receipt["exit_code"] == 0 && receipt["code_revision"] == status.state.code_revision
            ),
            package => {
                let integration = status
                    .state
                    .packages
                    .get(&format!("package.{package}"))
                    .and_then(|p| p.integration.as_ref())
                    .context("missing journal integration")?;
                ensure!(
                    integration["review"]["evidence_uri"] == uri
                        && integration["code_revision"] == receipt["code_revision"]
                );
                let prefix = format!("{package}::");
                ensure!(
                    receipt["exit_code"] == 0 && tests.keys().all(|name| name.starts_with(&prefix))
                );
            }
        }
    }

    let ledger = read_document(&root.join("model.records.json"))?;
    let records = &ledger["records"];
    let evidence = &records["evidence.demo.final"];
    let check = &records["check.demo.all"];
    ensure!(check["method"] == "cargo_test" && evidence["check"] == "check.demo.all");
    ensure!(evidence["check_revision"] == gdd::canonical_digest(check));
    let rules: BTreeSet<String> = serde_json::from_value(evidence["scope"].clone())?;
    ensure!(
        rules
            == [
                "rule.discount_floor",
                "rule.input",
                "rule.quote_composition",
                "rule.tax_floor"
            ]
            .map(str::to_owned)
            .into()
    );
    ensure!(
        evidence["scope"] == check["scope"]
            && evidence["code_revision"] == status.state.code_revision
    );
    ensure!(
        evidence["evidence_uri"] == "evidence/final.run.json"
            && evidence["executed"] == true
            && evidence["result"] == "PASS"
    );
    ensure!(
        evidence["code_revision"] == summary["code_revision"]
            && evidence["spec_revision"] == summary["spec_revision"]
    );
    Ok(())
}
