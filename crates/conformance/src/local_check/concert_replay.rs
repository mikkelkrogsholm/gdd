//! Connect the selected concert models to new observations from a single Cargo archive.

use super::{
    Run, concert, new_output, os_value, release, source_package, timestamp, write_new_json,
};
use anyhow::{Context, Result, ensure};
use gdd::{
    bundle::{Bundle, load_model_only},
    model_diff::{ModelDiff, diff_models_from_bundles},
};
use serde_json::{Value, json};
use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    process::ExitCode,
};

const BASE: &str = "examples/concert-app/history/phase-a/gdd.json";
const HEAD: &str = "examples/concert-app/gdd.json";
const DIFF: &str = "model-diff.json";
// The packaged behavior and fixed assertions exercise this pair of model and contract revisions.
const POLICY_A: &str = "sha256:6e63e184d19f87e79056d66d9922c3d7dbbd62db481f50e59d5f194fbe7d1ab9";
const POLICY_B: &str = "sha256:fcc1dd8ef8d20fc4fa8d0718c27f3774b188d36caccb3d51037d82857bd77170";

fn retain_capture(bundle: &Bundle, directory: &Path) -> Result<()> {
    for asset in bundle
        .primary_documents()
        .iter()
        .map(|doc| &doc.asset)
        .chain(bundle.normative_sources().values())
    {
        let path = directory.join(&asset.path);
        fs::create_dir_all(path.parent().context("missing captured asset directory")?)?;
        File::create_new(path)?.write_all(&asset.bytes)?;
    }
    Ok(())
}

fn policy_rule(diff: &ModelDiff) -> Result<&str> {
    ensure!(
        diff.exit_code == 0
            && diff.comparison.coverage == "complete"
            && diff.comparison.semantic_changes,
        "expected a complete semantic concert model diff; see {DIFF}"
    );
    let rule = diff
        .changes
        .iter()
        .find(|c| c.registry == "nodes" && c.id == "rule.active_limit" && c.change == "added")
        .and_then(|c| c.after.as_ref())
        .filter(|node| node["kind"] == "rule")
        .and_then(|node| node["rule"]["statement"].as_str())
        .context("expected the addition of rule.active_limit in the concert model diff")?;
    ensure!(
        diff.base.spec_revision.as_deref() == Some(POLICY_A)
            && diff.head.spec_revision.as_deref() == Some(POLICY_B),
        "unsupported concert policy revision: the fixed Rust checks exercise {POLICY_A} to {POLICY_B}; see {DIFF} for the observed revisions"
    );
    Ok(rule)
}

fn check(run: &mut Run<'_>, summary: &mut Value) -> Result<()> {
    let recipient = release::recipient(run.root, "gdd-concert-replay-")?;
    let target = tempfile::tempdir_in(recipient.path())?;
    run.target = target.path().to_owned();
    let package = source_package::concert_package(run, recipient.path())?;

    let base = load_model_only(&run.root.join(BASE));
    let head = load_model_only(&run.root.join(HEAD));
    let diff = diff_models_from_bundles(&base, &head)?;
    write_new_json(&run.output.join(DIFF), &diff)?;
    summary["model_diff"] = json!(DIFF);
    retain_capture(&base, &run.output.join("models/phase-a"))?;
    retain_capture(&head, &run.output.join("models/phase-b"))?;
    let statement = policy_rule(&diff)?;
    let packaged_head = load_model_only(&package.join("gdd.json"));
    ensure!(
        packaged_head.snapshot().map(|s| &s.revision) == diff.head.spec_revision.as_ref(),
        "the packaged phase-B model and its normative assets differ from the checkout capture"
    );

    summary["policy_change"] = json!({
        "phase_a": { "model": BASE, "capture": "models/phase-a/gdd.json", "spec_revision": diff.base.spec_revision },
        "phase_b": { "model": HEAD, "capture": "models/phase-b/gdd.json", "spec_revision": diff.head.spec_revision },
        "added_rule": { "id": "rule.active_limit", "statement": statement }
    });
    for (phase_a, revision) in [
        (true, &diff.base.spec_revision),
        (false, &diff.head.spec_revision),
    ] {
        let next = run.steps.len();
        let result = source_package::requirements(run, &package, target.path(), phase_a);
        if let Some(step) = run.steps.get_mut(next) {
            step["policy_phase"] = json!(if phase_a { "A" } else { "B" });
            step["policy_spec_revision"] = json!(revision);
            step["requirements_spec_revision"] = json!(diff.head.spec_revision);
            if let Ok(outcomes) = &result {
                step["outcomes"] = json!(outcomes);
            }
        }
        result?;
    }
    let mut command = source_package::concert_tests(target.path(), "reservations");
    command.args(["--exact", concert::TRANSITION]);
    let next = run.steps.len();
    let result = run.step_checked("concert-database-transition", command, &package, 0, |log| {
        concert::confirm_transition(&fs::read_to_string(log)?)
    });
    if let Some(step) = run.steps.get_mut(next) {
        step["from_spec_revision"] = json!(diff.base.spec_revision);
        step["to_spec_revision"] = json!(diff.head.spec_revision);
        if let Ok(outcome) = &result {
            step["outcome"] = outcome.clone();
        }
    }
    result?;
    Ok(())
}

pub(crate) fn replay(root: &Path, output: PathBuf) -> Result<ExitCode> {
    let output = new_output(Some(output))?;
    println!("Concert policy replay output: {}", output.display());
    let mut summary = json!({
        "format": "gdd-concert-policy-replay/0.1",
        "provenance": "New local execution of the packaged Rust concert reservation policy tests.",
        "checkout": os_value(root.as_os_str()),
        "started_at": timestamp()?,
    });
    let mut run = Run::new(root, &output, output.join("target"));
    let result = check(&mut run, &mut summary);
    summary["source_package"] = json!(run.concert_source_package);
    summary["steps"] = json!(run.steps);
    summary["finished_at"] = json!(timestamp()?);
    summary["passed"] = json!(result.is_ok());
    if let Err(error) = &result {
        summary["error"] = json!(format!("{error:#}"));
    }
    write_new_json(&output.join("summary.json"), &summary)?;
    result?;
    Ok(ExitCode::SUCCESS)
}
