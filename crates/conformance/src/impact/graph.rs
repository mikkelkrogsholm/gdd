use super::{Project, analyze, edge, item};
use anyhow::{Context, Result, ensure};
use gdd::context::{ContextGraph, context_manifest};
use serde_json::json;
use std::{collections::BTreeSet, fs, path::Path};

pub(super) fn reverse(directory: &Path, _: &str) -> Result<()> {
    let project = Project::new(directory)?;
    let path = project.save()?;
    let report = analyze(path, &["action.add"])?;
    ensure!(report.exit_code == 0);
    let root = "action.add";
    ensure!(item(&report, root, "action.caller")?.category == "direct");
    let outer = item(&report, root, "action.outer")?;
    ensure!(outer.category == "transitive");
    let edges: Vec<_> = outer.witness.iter().map(|s| s.edge.as_deref()).collect();
    ensure!(edges == [Some("edge.caller"), Some("edge.outer")]);
    ensure!(outer.witness.iter().all(|s| s.orientation == "reverse"));
    for id in ["component.arithmetic", "scenario.add"] {
        ensure!(item(&report, root, id)?.category == "supporting");
    }
    ensure!(
        report.roots[0]
            .items
            .iter()
            .all(|i| i.id != "action.sibling")
    );
    let caller = analyze(path, &["action.caller"])?;
    ensure!(item(&caller, "action.caller", root)?.category == "supporting");
    Ok(())
}

pub(super) fn cycles(directory: &Path, _: &str) -> Result<()> {
    let mut project = Project::new(directory)?;
    project
        .edges()?
        .push(edge("edge.cycle", "action.add", "calls", "action.outer"));
    let path = project.save()?;
    let report = analyze(path, &["action.outer", "action.add", "action.add"])?;
    let second = analyze(path, &["action.add", "action.outer"])?;
    ensure!(serde_json::to_value(&report)? == serde_json::to_value(second)?);
    ensure!(report.exit_code == 0 && report.roots.len() == 2);
    for root in &report.roots {
        ensure!(item(&report, &root.id, &root.id)?.category == "selected");
        ensure!(root.items.iter().all(|item| item.witness.len() < 8));
    }
    Ok(())
}

pub(super) fn closure(directory: &Path, _: &str) -> Result<()> {
    let mut project = Project::new(directory)?;
    project.edges()?.push(edge(
        "edge.refine",
        "action.sibling",
        "refines",
        "action.outer",
    ));
    let path = project.save()?;
    let bundle = gdd::bundle::load_model_only(path);
    let graph = ContextGraph::new(&bundle);
    for seed in bundle.nodes() {
        let scope = [seed.id.to_owned()];
        let manifest = context_manifest(&bundle, &scope)?;
        let declared: BTreeSet<_> = manifest.nodes.into_iter().collect();
        let context = graph.scope([seed.id]);
        let mut witnessed = BTreeSet::new();
        for node in bundle.nodes() {
            if let Some(steps) = context.path_to(node.id) {
                let mut at = seed.id;
                for step in steps {
                    ensure!(
                        step.from == Some(at),
                        "broken path from {} to {}",
                        seed.id,
                        node.id
                    );
                    at = step.to;
                }
                ensure!(at == node.id);
                witnessed.insert(node.id);
            }
        }
        ensure!(
            witnessed == declared,
            "context closure differs for {}",
            seed.id
        );
    }
    let root = "action.outer";
    let actual = analyze(path, &[root])?;
    let child = item(&actual, root, "action.sibling")?;
    ensure!(child.category == "supporting" && child.witness.len() == 1);
    let refinement = &child.witness[0];
    ensure!(
        refinement.edge.as_deref() == Some("edge.refine")
            && refinement.derivation == "initial-refinement"
    );
    Ok(())
}

pub(super) fn extension(directory: &Path, parameter: &str) -> Result<()> {
    let dependency: bool = parameter.parse()?;
    let mut project = Project::new(directory)?;
    let extension = json!({"format":"gdd-extension/0.2","namespace":"example.links","version":"1.0.0",
        "node_kinds":{},"relations":{"example.links.related":{
            "description":"Explicit read context with separately declared dependency.",
            "from":["action"],"to":["action"],"context":"both","dependency":dependency,
            "effect":"none","data_schema":{"type":"object","additionalProperties":false}
        }}
    });
    let bytes = serde_json::to_vec(&extension)?;
    project.model["extensions"] = json!({"extension.links":{"namespace":"example.links","version":"1.0.0",
        "path":"extension.json","sha256":gdd::sha256(&bytes)}});
    fs::write(project.temp.path().join("extension.json"), bytes)?;
    let edges = project.edges()?;
    edges.retain(|e| e["id"] != "edge.caller");
    let mut custom = edge(
        "edge.custom",
        "action.caller",
        "example.links.related",
        "action.add",
    );
    custom["data"] = json!({});
    edges.push(custom);
    let report = analyze(project.save()?, &["action.add"])?;
    ensure!(report.exit_code == 0, "{:?}", report.diagnostics);
    let caller = item(&report, "action.add", "action.caller")?;
    ensure!(caller.category == if dependency { "direct" } else { "supporting" });
    ensure!(caller.witness.first().and_then(|s| s.edge.as_deref()) == Some("edge.custom"));
    Ok(())
}

pub(super) fn unknown_extension(directory: &Path, _: &str) -> Result<()> {
    let mut project = Project::new(directory)?;
    project.model["extensions"] = json!({"extension.links":{"namespace":"example.links","version":"1.0.0",
        "path":"missing.json","sha256":"0".repeat(64)}});
    let report = analyze(project.save()?, &["action.add"])?;
    ensure!(report.exit_code == 2 && report.coverage.result != "complete");
    ensure!(
        report
            .diagnostics
            .iter()
            .any(|d| d.code == "EXTENSION_UNSUPPORTED")
    );
    Ok(())
}

pub(super) fn types(directory: &Path, _: &str) -> Result<()> {
    let mut project = Project::new(directory)?;
    project.model["nodes"]["entity.input"] =
        json!({"kind":"entity","description":"Input","fields":{}});
    project.model["nodes"]["action.add"]["contract"]["input"]["left"] =
        json!({"type":"reference","target":"entity.input"});
    let report = analyze(project.save()?, &["entity.input"])?;
    ensure!(report.exit_code == 0);
    let consumer = item(&report, "entity.input", "action.add")?;
    ensure!(
        consumer
            .witness
            .first()
            .context("missing type witness")?
            .derivation
            == "type-reference-consumer"
    );
    Ok(())
}

pub(super) fn entities(directory: &Path, relation: &str) -> Result<()> {
    let mut project = Project::new(directory)?;
    project.model["nodes"]["entity.shared"] =
        json!({"kind":"entity","description":"Shared entity","fields":{}});
    project
        .edges()?
        .push(edge("edge.data", "action.add", relation, "entity.shared"));
    check_direction(&project, "entity.shared", "action.add")
}

pub(super) fn navigation(directory: &Path, _: &str) -> Result<()> {
    let mut project = Project::new(directory)?;
    for name in ["entry", "destination"] {
        project.model["nodes"][format!("screen.{name}")] =
            json!({"kind":"screen","description":name});
    }
    project.edges()?.push(edge(
        "edge.navigation",
        "screen.entry",
        "navigates_to",
        "screen.destination",
    ));
    check_direction(&project, "screen.destination", "screen.entry")
}

fn check_direction(project: &Project, provider: &str, consumer: &str) -> Result<()> {
    let path = project.save()?;
    let changed_provider = analyze(path, &[provider])?;
    let changed_consumer = analyze(path, &[consumer])?;
    ensure!(changed_provider.exit_code == 0 && changed_consumer.exit_code == 0);
    ensure!(item(&changed_provider, provider, consumer)?.category == "direct");
    ensure!(item(&changed_consumer, consumer, provider)?.category == "supporting");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reverse_case_detects_a_shortcut_to_the_transitive_caller() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let tutorial = temp.path().join("docs/tutorial");
        let directory = temp.path().join("conformance");
        fs::create_dir_all(&tutorial)?;
        fs::create_dir(&directory)?;
        let original =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/tutorial/refined.gdd.json");
        let mut model: serde_json::Value = serde_json::from_slice(&fs::read(original)?)?;
        model["edges"].as_array_mut().unwrap().push(edge(
            "edge.shortcut",
            "action.outer",
            "calls",
            "action.add",
        ));
        crate::fixture::write_json(&tutorial.join("refined.gdd.json"), &model)?;
        let failure = reverse(&directory, "default").unwrap_err();
        assert!(failure.to_string().contains("transitive"), "{failure:#}");
        Ok(())
    }
}
