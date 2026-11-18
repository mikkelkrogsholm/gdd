//! Packet integrity, scoped model context and independent parallel-plan controls.

use super::concert::{CANCEL, Concert, RESERVE};
use super::*;
use gdd::context::ContextGraph;
use std::collections::BTreeSet;
use std::io::Write;

pub(super) fn scope(directory: &Path, _: &str) -> Result<()> {
    let case = Concert::new(directory, &[RESERVE])?;
    let plan = case.current_plan()?;
    let packet = &plan["packages"][0];
    let bundle = bundle::load(&case.path);
    let graph = ContextGraph::new(&bundle);
    let closure = graph.scope([RESERVE]);
    ensure!(
        packet["scope"] == json!([RESERVE]) && packet["implementation_targets"] == json!([RESERVE])
    );
    let packet_nodes: BTreeSet<String> =
        serde_json::from_value(packet["context"]["nodes"].clone())?;
    ensure!(
        closure.contains(CANCEL) && packet_nodes == closure.nodes().map(str::to_owned).collect()
    );

    let mut objects = Vec::new();
    let mut pending = vec![packet];
    while let Some(value) = pending.pop() {
        match value {
            Value::Object(object) => {
                objects.push(object);
                pending.extend(object.values());
            }
            Value::Array(array) => pending.extend(array),
            _ => (),
        }
    }
    for node in bundle.nodes().filter(|node| closure.contains(node.id)) {
        let original = validation::to_json(node.value)?;
        let copies: Vec<_> = objects
            .iter()
            .filter_map(|o| o.get(node.id))
            .filter(|v| v.get("kind").is_some())
            .collect();
        ensure!(
            !copies.is_empty() && copies.iter().all(|&copy| copy == &original),
            "missing or altered node {}",
            node.id
        );
    }
    let edges = packet["model_slice"]["edges"]
        .as_array()
        .context("missing packet edges")?;
    for edge in bundle.edges() {
        let value = validation::to_json(edge.value)?;
        if closure.contains(value["from"].as_str().context("missing edge source")?)
            && closure.contains(value["to"].as_str().context("missing edge target")?)
        {
            ensure!(edges.contains(&value), "missing original edge {}", edge.id);
        }
    }
    Ok(())
}

pub(super) fn determinism(directory: &Path, _: &str) -> Result<()> {
    let case = Concert::new(directory, &[RESERVE])?;
    let before = crate::fixture::snapshot(case.temp.path())?;
    let request = case.request.clone();
    let first = case.prepare()?;
    let second = case.prepare()?;
    ensure!(first.artifact == second.artifact && first.artifact.is_object());
    ensure!(case.request == request && before == crate::fixture::snapshot(case.temp.path())?);
    ensure!(
        !first
            .report
            .checks
            .iter()
            .any(|c| c.dimension == "implementation" && c.result == CheckResult::Pass)
    );
    Ok(())
}

pub(super) fn model_drift(directory: &Path, _: &str) -> Result<()> {
    let case = Concert::new(directory, &[RESERVE])?;
    let plan = case.current_plan()?;
    fs::OpenOptions::new()
        .append(true)
        .open(&case.path)?
        .write_all(b"\n")?;
    case.rebase_planned_records()?;
    case.backend_ready()?;
    ensure!(case.integrity(&plan)? == CheckResult::Fail);
    Ok(())
}

pub(super) fn asset_drift(directory: &Path, _: &str) -> Result<()> {
    let case = Concert::new(directory, &[RESERVE])?;
    let source = case.temp.path().join("contract.txt");
    let first = b"Contract revision one\n";
    let second = b"Contract revision two\n";
    fs::write(&source, first)?;
    let mut model = read_document(&case.path)?;
    model["sources"]["source.contract.synthetic"] = json!({
        "role":"knowledge","title":"Synthetic contract","uri":"contract.txt","format":"text/plain",
        "normative":true,"revision":{"sha256":gdd::sha256(first)}
    });
    model["nodes"][RESERVE]["source_refs"]
        .as_array_mut()
        .context("missing source refs")?
        .push(json!({"source":"source.contract.synthetic","purpose":"contract"}));
    case.rewrite(&model)?;
    let plan = case.current_plan()?;
    fs::write(source, second)?;
    model["sources"]["source.contract.synthetic"]["revision"]["sha256"] =
        json!(gdd::sha256(second));
    case.rewrite(&model)?;
    case.backend_ready()?;
    ensure!(case.integrity(&plan)? == CheckResult::Fail);
    Ok(())
}

pub(super) fn packet_tamper(directory: &Path, _: &str) -> Result<()> {
    let case = Concert::new(directory, &[RESERVE])?;
    let mut plan = case.current_plan()?;
    let behavior = plan
        .pointer_mut("/packages/0/model_slice/nodes/action.reserve/behavior")
        .context("missing embedded contract")?;
    *behavior = json!("Skip authorization");
    case.backend_ready()?;
    ensure!(case.integrity(&plan)? == CheckResult::Fail);
    Ok(())
}

pub(super) fn parallel_readiness(directory: &Path, _: &str) -> Result<()> {
    let case = Concert::new(directory, &[RESERVE, "screen.reserve"])?;
    let bundle = bundle::load(&case.path);
    let graph = ContextGraph::new(&bundle);
    let spec = revision(&case.path)?;
    let mut packages = case.request["packages"].clone();
    for package in packages.as_array_mut().context("missing packages")? {
        package
            .as_object_mut()
            .context("malformed package")?
            .remove("deliverables");
        let scope = package["scope"][0]
            .as_str()
            .context("missing package scope")?;
        let closure = graph.scope([scope]);
        let mut locked = BTreeSet::new();
        for node in bundle.nodes().filter(|n| closure.contains(n.id)) {
            let node_kind = &validation::to_json(node.value)?["kind"];
            if node_kind == "action" || node_kind == "screen" {
                locked.insert(node.id);
            }
        }
        package["locked_contracts"] = json!(locked);
        package["base_revision"] = json!(spec);
    }
    let parallel = Input::from(&json!({
        "format":"gdd-parallel-plan/0.2","packages":packages,
        "serialized":case.request["serialized"],"integration":case.request["integration"]
    }));
    let oracle = validation::validate_path(
        &case.path,
        ValidationOptions {
            profiles: &[Profile::Readiness],
            parallel_plan: Some(&parallel),
            ..Default::default()
        },
        None,
    )?;
    ensure!(oracle.result("readiness") == CheckResult::Pass);
    ensure!(case.prepare()?.report.result("readiness") == CheckResult::Fail);
    Ok(())
}

pub(super) fn ownership(directory: &Path, conflict: &str) -> Result<()> {
    let mut case = Concert::new(directory, &[RESERVE, CANCEL])?;
    case.current_plan()?;
    let packages = &mut case.request["packages"];
    match conflict {
        "same_file" => {
            packages[0]["write_resources"] = json!(["src/shared"]);
            packages[1]["write_resources"] = json!(["src/shared"]);
        }
        "directory" => {
            packages[0]["write_resources"] = json!(["src/"]);
            packages[1]["write_resources"] = json!(["src/cancel"]);
        }
        "same_workspace" => packages[1]["workspace"] = packages[0]["workspace"].clone(),
        _ => anyhow::bail!("unknown ownership conflict {conflict}"),
    };
    let report = case.prepare()?.report;
    ensure!(report.result("readiness") == CheckResult::Fail);
    ensure!(
        report
            .checks
            .iter()
            .flat_map(|c| &c.diagnostics)
            .any(|d| d.code == "WRITE_OWNERSHIP_CONFLICT")
    );
    Ok(())
}

pub(super) fn serialization(directory: &Path, _: &str) -> Result<()> {
    let mut case = Concert::new(directory, &[RESERVE, CANCEL])?;
    for package in case.request["packages"]
        .as_array_mut()
        .context("missing packages")?
    {
        package["write_resources"] = json!(["src/shared"]);
    }
    case.request["serialized"] = json!([["package.reserve", "package.cancel"]]);
    ensure!(case.current_plan()?["request"]["serialized"] == case.request["serialized"]);
    Ok(())
}

pub(super) fn graph_dependencies(directory: &Path, _: &str) -> Result<()> {
    let case = Concert::new(directory, &[RESERVE, CANCEL])?;
    let mut model = read_document(&case.path)?;
    let edges = model["edges"].as_array_mut().context("missing edges")?;
    for relation in ["calls", "depends_on"] {
        edges.push(json!({"id":format!("edge.synthetic.{relation}"),"from":RESERVE,"relation":relation,"to":CANCEL}));
    }
    case.rewrite(&model)?;
    ensure!(case.request["serialized"] == json!([]));
    ensure!(case.current_plan()?["request"]["serialized"] == json!([]));
    Ok(())
}
