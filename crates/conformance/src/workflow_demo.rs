//! Reconstruct the obsolete Rust quote implementation and publish a freshly checked run.

mod checks;
mod project;

use anyhow::{Context, Result, ensure};
use checks::Stage;
use gdd::{
    host::append_records,
    input::Value as Input,
    workflow::prepare_workflow,
    workflow_run::{AdvanceResult, RunPhase, advance_run, create_run, run_status},
};
use serde::Serialize;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

fn write_json(path: &Path, value: &impl Serialize) -> Result<()> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    fs::write(path, bytes).with_context(|| format!("write {}", path.display()))
}

struct Replay {
    model: PathBuf,
    run: PathBuf,
    host: Value,
}

impl Replay {
    fn event(&self, mut fields: Value) -> Result<AdvanceResult> {
        let kind = fields["type"].as_str().context("missing event type")?;
        let package = fields["package"].as_str().unwrap_or("run");
        let key = format!("{kind}.{package}");
        fields["idempotency_key"] = json!(key);
        fields["expected_revision"] = json!(run_status(&self.model, &self.run, None)?.revision);
        let result = advance_run(
            &self.model,
            &self.run,
            &Input::from(&fields),
            Some(&Input::from(&self.host)),
        )?;
        ensure!(
            result.applied,
            "{key} blocked: {}",
            serde_json::to_string(&result.report)?
        );
        Ok(result)
    }

    fn dispatch(&self, stage: Stage) -> Result<()> {
        let name = stage.name();
        self.event(json!({"type":"dispatch", "package":format!("package.{name}"), "attempt":format!("replay.{name}")}))?;
        Ok(())
    }
}

fn execute(root: &Path, target: &Path) -> Result<Value> {
    fs::create_dir(root)?;
    fs::create_dir(root.join("evidence"))?;
    project::baseline(root)?;
    let spec = project::model(root)?;
    let versions = checks::versions(root)?;
    let baseline = checks::run(root, target, Stage::Baseline, &versions)?;
    let request = project::request(&baseline.snapshot.revision)?;
    write_json(&root.join("workflow.request.json"), &request)?;
    let mut replay = Replay {
        model: root.join("model.gdd.json"),
        run: root.join("run"),
        host: project::host(&spec, &baseline.snapshot.revision)?,
    };
    write_json(&root.join("host.implementation.json"), &replay.host)?;
    let plan = prepare_workflow(&replay.model, &Input::from(&request), None)?.artifact;
    ensure!(
        plan["spec_revision"] == spec,
        "workplan selected a different spec revision"
    );
    write_json(&root.join("plan.json"), &plan)?;
    let initial = create_run(&replay.model, &Input::from(&plan), &replay.run, None)?;
    ensure!(
        initial.available_packages == ["package.discount", "package.tax"],
        "unexpected initial packages"
    );
    replay.dispatch(Stage::Discount)?;
    replay.dispatch(Stage::Tax)?;
    for (stage, resource) in [
        (Stage::Discount, "src/discount.rs"),
        (Stage::Tax, "src/tax.rs"),
        (Stage::Quote, "tests/scenarios/quote.rs"),
    ] {
        let name = stage.name();
        let package = format!("package.{name}");
        let attempt = format!("replay.{name}");
        if stage == Stage::Quote {
            let status = run_status(&replay.model, &replay.run, None)?;
            ensure!(
                status.available_packages == ["package.quote"],
                "quote prerequisites are incomplete"
            );
            replay.dispatch(stage)?;
        }
        let submitted = format!("submissions/{resource}");
        replay.event(json!({
            "type":"submit", "package":package, "attempt":attempt,
            "summary":"Replay the bundled Rust repair and verify it locally.",
            "artifact_uri":submitted, "changed_resources":[resource],
        }))?;
        let base = run_status(&replay.model, &replay.run, None)?
            .state
            .code_revision;
        ensure!(
            project::snapshot(root)?.revision == base,
            "integration base differs from the journal"
        );
        fs::copy(root.join(submitted), root.join(resource))?;
        let reviewed = checks::run(root, target, stage, &versions)?;
        replay.host["code_revision"] = json!(reviewed.snapshot.revision);
        replay.event(json!({
            "type":"integrate", "package":package, "attempt":attempt,
            "base_code_revision":base, "code_revision":reviewed.snapshot.revision,
            "verified_resources":[resource], "review":{
                "reviewer":"actor.demo_lead","verdict":"PASS","evidence_uri":format!("evidence/{name}.run.json"),
            },
        }))?;
    }

    let verified = checks::run(root, target, Stage::Final, &versions)?;
    ensure!(
        replay.host["code_revision"] == verified.snapshot.revision,
        "final source revision changed"
    );
    write_json(&root.join("code.snapshot.json"), &verified.snapshot)?;
    attest(root, &verified, &versions, &mut replay.host)?;
    replay.host["policy"]["activity"] = json!("acceptance");
    write_json(&root.join("host.acceptance.json"), &replay.host)?;
    let accepted = replay.event(json!({"type":"accept"}))?;
    write_json(&root.join("acceptance.json"), &accepted)?;
    let status = run_status(&replay.model, &replay.run, Some(&Input::from(&replay.host)))?;
    ensure!(
        status.state.status == RunPhase::Accepted
            && status.acceptance_current
            && status.event_count == 10,
        "expected a current acceptance after ten events"
    );
    write_json(&root.join("status.json"), &status)?;
    let summary = json!({
        "format":"gdd-workflow-demo/0.1", "implementation":project::IMPLEMENTATION,
        "baseline_exit":baseline.test_exit, "baseline_cargo_exit":baseline.cargo_exit,
        "final_exit":verified.test_exit, "final_cargo_exit":verified.cargo_exit,
        "acceptance_current":status.acceptance_current, "events":status.event_count,
        "spec_revision":spec, "code_revision":verified.snapshot.revision,
    });
    write_json(&root.join("summary.json"), &summary)?;
    Ok(summary)
}

/// Called only after the seven named negative controls and the full final suite
/// have completed. These records describe the local Cargo checker.
fn attest(
    root: &Path,
    verified: &checks::TestRun,
    versions: &BTreeMap<String, String>,
    host: &mut Value,
) -> Result<()> {
    let ledger = root.join("model.records.json");
    let records: Value = serde_json::from_slice(&fs::read(&ledger)?)?;
    let check = &records["records"]["check.demo.all"];
    let snapshot = &verified.snapshot;
    ensure!(
        verified.cargo_exit == 0 && project::snapshot(root)? == *snapshot,
        "final Cargo check is stale"
    );
    let tests: BTreeMap<_, _> = snapshot
        .files
        .iter()
        .filter(|(name, _)| name.starts_with("tests/"))
        .collect();
    let binding = "binding.demo.verified";
    let evidence = "evidence.demo.final";
    let fragment = json!({"records":{
        (binding):{
            "kind":"binding", "spec_revision": check["spec_revision"], "targets":check["scope"],
            "implementation":project::IMPLEMENTATION, "role":"test", "resource":"tests/",
            "selector":{"kind":"whole-document"}, "status":"checked",
            "revision":gdd::canonical_digest(&json!(tests)), "code_revision":snapshot.revision,
        },
        (evidence):{
            "kind":"evidence", "check":"check.demo.all", "check_revision":gdd::canonical_digest(check),
            "implementation":project::IMPLEMENTATION, "spec_revision":check["spec_revision"],
            "code_revision":snapshot.revision, "scope":check["scope"], "result":"PASS", "executed":true,
            "checker":{"name":"cargo test", "version":versions["cargo"].lines().next()},
            "reason":"All 19 integrated Rust tests passed, including the compiled CLI. The half-up baseline failed the seven expected floor assertions.",
            "environment":versions, "evidence_uri":"evidence/final.run.json",
        },
    }});
    append_records(&ledger, &fragment)?;
    host["verified_bindings"] = json!([binding]);
    host["trusted_evidence"] = json!([evidence]);
    host["supported_methods"] = json!([check["method"]]);
    host["negative_controls"] = json!([project::NEGATIVE_CONTROL]);
    host["evidence_selection"] = json!({"check.demo.all":evidence});
    host["environment"] = json!(versions);
    Ok(())
}

/// Execute the bundled demo in isolation, then publish a complete detached project.
pub(crate) fn replay(output: &Path) -> Result<Value> {
    match fs::symlink_metadata(output) {
        Ok(_) => anyhow::bail!("destination already exists: {}", output.display()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
        Err(error) => return Err(error.into()),
    }
    let parent = output
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut builder = tempfile::Builder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        builder.permissions(fs::Permissions::from_mode(0o700));
    }
    let staging = builder.tempdir_in(parent)?;
    // Run Cargo outside the caller-selected destination, including its parent
    // .cargo/config files. Keep the build cache out of the published project.
    let private = builder.tempdir()?;
    let root = private.path().join("quote");
    let summary = execute(&root, &private.path().join("target"))?;
    let detached = staging.path().join("project");
    for (name, bytes) in crate::fixture::snapshot(&root)? {
        let path = detached.join(name);
        fs::create_dir_all(path.parent().context("missing staged directory")?)?;
        fs::write(path, bytes)?;
    }
    // Publish the directory in one operation, including across concurrent callers.
    gdd::publish_exclusive(&detached, output)?;
    Ok(summary)
}
