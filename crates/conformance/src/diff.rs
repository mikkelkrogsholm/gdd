//! Run stable-identity model diff cases against private fixtures and archived models.

mod advisories;
mod coverage;

use crate::fixture::{restore_concert, snapshot, write_json};
use crate::named::{self, Behavior};
use anyhow::{Context, Result, ensure};
use gdd::model_diff::{ModelDiff, diff_models, diff_text};
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    process::ExitCode,
};

macro_rules! diff_case {
    ($id:ident, $reference:ident, $execute:path, $parameters:expr) => {
        Behavior {
            id: concat!("diff.", stringify!($id)),
            reference: concat!("tests/test_model_diff.py::test_", stringify!($reference)),
            execute: $execute,
            parameters: $parameters,
        }
    };
    ($id:ident, $reference:ident, $execute:path) => {
        diff_case!($id, $reference, $execute, &["default"])
    };
}

const BEHAVIORS: &[Behavior] = &[
    diff_case!(
        semantic_unknown,
        semantic_unknown_is_partial_and_does_not_establish_absence,
        coverage::semantic_unknown,
        &["unknown", "invalid"]
    ),
    diff_case!(identity, identity_reformat_and_edge_reorder, identity),
    diff_case!(fragments, fragment_relocation_is_nonsemantic, fragments),
    diff_case!(fields, exact_fields_prose_types_and_escaped_pointer, fields),
    diff_case!(registries, registry_add_remove_change, registries),
    diff_case!(
        weakening,
        structural_advisories_positive_and_noop_negative,
        advisories::weakening,
        &[
            "nonblocking",
            "remove_rule",
            "direct",
            "endpoint",
            "remove_verifier",
            "interfaces",
            "kind"
        ]
    ),
    diff_case!(
        contracts,
        contract_removal_advisory_and_addition_negative,
        advisories::contracts,
        &["errors", "preconditions", "postconditions"]
    ),
    diff_case!(
        verifiers,
        retained_verifier_and_expanding_forbid_not_warned,
        advisories::verifiers
    ),
    diff_case!(
        partial,
        partial_inputs_do_not_fabricate_absence,
        coverage::partial
    ),
    diff_case!(errors, unknown_and_invalid_inputs_visible, coverage::errors),
    diff_case!(legacy, legacy_to_current_format, legacy),
    diff_case!(
        schema,
        result_schema_covers_partial_invalid_and_changed,
        coverage::schema
    ),
    diff_case!(readonly, schema_determinism_and_read_only, readonly),
    diff_case!(concert, archived_requirement_change_exposes_limit, concert),
    diff_case!(
        extensions,
        extension_semantics_not_asset_packaging,
        extensions
    ),
    diff_case!(
        source_identity,
        source_role_change_has_identity_review,
        advisories::source_identity
    ),
];

pub(super) fn command(directory: &Path, output: Option<PathBuf>) -> Result<ExitCode> {
    named::command(
        directory,
        output,
        "diff",
        "gdd-model-diff-conformance/0.1",
        BEHAVIORS,
        (),
    )
}

struct Pair {
    directory: tempfile::TempDir,
    base: PathBuf,
    head: PathBuf,
    model: Value,
}

impl Pair {
    fn new(directory: &Path) -> Result<Self> {
        let bytes = fs::read(directory.join("../docs/tutorial/refined.gdd.json"))
            .context("read the refined tutorial model")?;
        let model = serde_json::from_slice(&bytes)?;
        let directory = tempfile::tempdir()?;
        let base = directory.path().join("base.json");
        let head = directory.path().join("head.json");
        Ok(Self {
            directory,
            base,
            head,
            model,
        })
    }

    fn compare(&self, head: &Value) -> Result<ModelDiff> {
        write_json(&self.base, &self.model)?;
        write_json(&self.head, head)?;
        Ok(diff_models(&self.base, &self.head)?)
    }
}

fn edges(model: &mut Value) -> Result<&mut Vec<Value>> {
    model["edges"]
        .as_array_mut()
        .context("expected tutorial edges")
}

fn remove_node(model: &mut Value, id: &str) -> Result<()> {
    model["nodes"]
        .as_object_mut()
        .and_then(|nodes| nodes.remove(id))
        .with_context(|| format!("missing {id}"))?;
    Ok(())
}

fn append_text(field: &mut Value, extra: &str) -> Result<()> {
    *field = json!(format!(
        "{}{extra}",
        field.as_str().context("expected tutorial text")?
    ));
    Ok(())
}

fn identity(directory: &Path, _: &str) -> Result<()> {
    let pair = Pair::new(directory)?;
    fs::write(&pair.base, serde_json::to_vec(&pair.model)?)?;
    let mut head = pair.model.clone();
    head.sort_all_objects();
    write_json(&pair.head, &head)?;
    let report = diff_models(&pair.base, &pair.head)?;
    ensure!(report.exit_code == 0 && report.base.spec_revision != report.head.spec_revision);
    ensure!(!report.comparison.semantic_changes);
    let mut reordered = pair.model.clone();
    edges(&mut reordered)?.reverse();
    fs::write(&pair.head, serde_json::to_vec(&reordered)?)?;
    ensure!(
        !diff_models(&pair.base, &pair.head)?
            .comparison
            .semantic_changes
    );
    Ok(())
}

fn fragments(directory: &Path, _: &str) -> Result<()> {
    let pair = Pair::new(directory)?;
    let mut head = pair.model.clone();
    let fragment =
        json!({"format":"gdd-fragment/0.3", "nodes":head["nodes"], "edges":head["edges"]});
    write_json(&pair.directory.path().join("part.json"), &fragment)?;
    head["nodes"] = json!({});
    head["edges"] = json!([]);
    head["includes"] = json!(["part.json"]);
    let report = pair.compare(&head)?;
    ensure!(
        report.exit_code == 0 && report.changes.is_empty() && report.metadata_changes.is_empty()
    );
    ensure!(!report.relocated.is_empty());
    Ok(())
}

fn fields(directory: &Path, _: &str) -> Result<()> {
    let pair = Pair::new(directory)?;
    let mut head = pair.model.clone();
    let action = &mut head["nodes"]["action.add"];
    append_text(&mut action["behavior"], "\nReturn exactly.")?;
    action["contract"]["input"]["left"]["type"] = json!("number");
    action["contract"]["input"]["a/b~c"] = json!({"type":"integer"});
    let report = pair.compare(&head)?;
    let change = report
        .changes
        .first()
        .context("missing action field changes")?;
    let pointers: BTreeSet<_> = change.fields.iter().map(|f| f.pointer.as_str()).collect();
    ensure!(
        pointers
            == BTreeSet::from([
                "/behavior",
                "/contract/input/left/type",
                "/contract/input/a~1b~0c"
            ])
    );
    ensure!(report.exit_code == 0 && report.advisories.is_empty());
    ensure!(diff_text(&report).contains("Return exactly."));
    Ok(())
}

fn registries(directory: &Path, _: &str) -> Result<()> {
    let pair = Pair::new(directory)?;
    let mut head = pair.model.clone();
    head["nodes"]["action.extra"] = head["nodes"]["action.add"].clone();
    remove_node(&mut head, "scenario.add")?;
    edges(&mut head)?.retain(|edge| edge["from"] != "scenario.add");
    head["questions"] = json!({"question.next":{"statement":"Should output be transported?", "status":"open",
        "blocks":["implementation"], "targets":["action.add"]}});
    append_text(
        &mut head["project"]["goal"],
        " Review transport separately.",
    )?;
    let report = pair.compare(&head)?;
    for (registry, id, change) in [
        ("nodes", "action.extra", "added"),
        ("nodes", "scenario.add", "removed"),
        ("edges", "edge.add.scenario", "removed"),
        ("questions", "question.next", "added"),
    ] {
        ensure!(
            report
                .changes
                .iter()
                .any(|c| c.registry == registry && c.id == id && c.change == change),
            "missing {registry} {id} {change}"
        );
    }
    ensure!(
        report
            .metadata_changes
            .first()
            .is_some_and(|f| f.pointer == "/project/goal")
    );
    Ok(())
}

fn legacy(directory: &Path, _: &str) -> Result<()> {
    let report = diff_models(
        &directory.join("../archive/gdd-0.2/examples/concert.gdd.yaml"),
        &directory.join("../examples/concert-0.3.gdd.yaml"),
    )?;
    ensure!(report.base.format == Some("gdd/0.2") && report.head.format == Some("gdd/0.3"));
    ensure!(
        report
            .metadata_changes
            .iter()
            .any(|f| f.pointer == "/format")
    );
    Ok(())
}

fn readonly(directory: &Path, _: &str) -> Result<()> {
    let pair = Pair::new(directory)?;
    write_json(&pair.base, &pair.model)?;
    write_json(&pair.head, &pair.model)?;
    let original = snapshot(pair.directory.path())?;
    let first = diff_models(&pair.base, &pair.head)?;
    let second = diff_models(&pair.base, &pair.head)?;
    ensure!(serde_json::to_value(&first)? == serde_json::to_value(second)?);
    ensure!(snapshot(pair.directory.path())? == original);
    ensure!(diff_text(&first).ends_with('\n'));
    coverage::validate(directory, &first)
}

fn concert(directory: &Path, _: &str) -> Result<()> {
    let temp = tempfile::tempdir()?;
    let project = restore_concert(directory, temp.path())?;
    let report = diff_models(
        &project.join("history/phase-a/gdd.json"),
        &project.join("gdd.json"),
    )?;
    ensure!(report.comparison.coverage == "complete" && report.comparison.semantic_changes);
    let text = diff_text(&report);
    ensure!(text.contains("LimitReached") && text.contains("principal"));
    Ok(())
}

fn write_extension(dir: &Path, name: &str, raw: &[u8], model: &Value) -> Result<PathBuf> {
    let path = format!("{name}.json");
    fs::write(dir.join(&path), raw)?;
    let mut manifest = model.clone();
    manifest["extensions"] = json!({"extension.links":{"namespace":"example.links", "version":"1.0.0",
        "path":path, "sha256":gdd::sha256(raw)}});
    let manifest_path = dir.join("gdd.json");
    write_json(&manifest_path, &manifest)?;
    Ok(manifest_path)
}

fn extensions(directory: &Path, _: &str) -> Result<()> {
    let pair = Pair::new(directory)?;
    let mut extension = json!({"format":"gdd-extension/0.2", "namespace":"example.links", "version":"1.0.0",
        "node_kinds":{}, "relations":{"example.links.related":{"description":"Declared relation.",
        "from":["action"], "to":["action"], "context":"both", "dependency":false, "effect":"none",
        "data_schema":{"type":"object"}}}});
    let base_dir = pair.directory.path().join("base");
    let head_dir = pair.directory.path().join("head");
    fs::create_dir(&base_dir)?;
    fs::create_dir(&head_dir)?;
    let base = write_extension(
        &base_dir,
        "base",
        &serde_json::to_vec(&extension)?,
        &pair.model,
    )?;
    let head = write_extension(
        &head_dir,
        "head",
        &serde_json::to_vec_pretty(&extension)?,
        &pair.model,
    )?;
    let repacked = diff_models(&base, &head)?;
    ensure!(repacked.exit_code == 0 && !repacked.comparison.semantic_changes);
    extension["relations"]["example.links.related"]["dependency"] = json!(true);
    write_extension(
        &head_dir,
        "head",
        &serde_json::to_vec(&extension)?,
        &pair.model,
    )?;
    let changed = diff_models(&base, &head)?;
    ensure!(changed.exit_code == 0 && changed.comparison.semantic_changes);
    ensure!(
        changed
            .metadata_changes
            .first()
            .is_some_and(|f| f.pointer.ends_with("/dependency"))
    );
    Ok(())
}
