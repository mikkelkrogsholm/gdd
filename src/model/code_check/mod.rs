//! Observe static Python imports against a captured model's explicit architecture constraints.

mod bindings;
mod glob;
mod inventory;
mod runtime;
pub use runtime::PythonRuntime;

use super::{Model, get, inspection, semantics, string};
use crate::{bundle, python_imports, report::CheckResult, validation};
use inventory::Inventory;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::{io, path::Path};

/// Inputs for one code observation. `runtime` describes the selected CPython target.
#[derive(Debug)]
pub struct CodeCheckOptions<'a> {
    pub root: &'a Path,
    pub implementation: &'a str,
    pub profile: &'a str,
    pub records: Option<&'a Path>,
    pub scope: &'a [String],
    pub exclude: &'a [String],
    pub runtime: &'a PythonRuntime,
}

fn diagnostic(code: &str, message: &str, file: Option<&str>, ids: &[&str]) -> Value {
    let mut seen = BTreeSet::new();
    let ids: Vec<_> = ids.iter().filter(|id| seen.insert(**id)).collect();
    json!({"code":code,"severity":"warning","message":message,"location":{"file":file,"pointer":"","line":null,"column":null},"related_ids":ids})
}

fn unknown(mut report: Value, code: &str, message: &str) -> Value {
    report["checks"] = json!([{"check_id":"code.inputs","dimension":"integration","result":"UNKNOWN","scope":report["scope"],"reason":message,
                             "diagnostics":[diagnostic(code,message,None,&[])]}]);
    report
}

fn out_edges(edges: &[Value]) -> BTreeMap<&str, Vec<&Value>> {
    let mut adjacency: BTreeMap<&str, Vec<&Value>> = BTreeMap::new();
    for e in edges {
        adjacency
            .entry(e["from"].as_str().unwrap())
            .or_default()
            .push(e);
    }
    for values in adjacency.values_mut() {
        values.sort_by_key(|e| {
            (
                e["to"].as_str(),
                e["source_file"].as_str(),
                e["line"].as_u64(),
                e["module"].as_str(),
            )
        });
    }
    adjacency
}

fn shortest_path<'a>(
    edges: &BTreeMap<&str, Vec<&'a Value>>,
    source: &str,
    target: &str,
    transitive: bool,
) -> Vec<&'a Value> {
    let mut queue = VecDeque::from([source]);
    let mut visited = BTreeSet::from([source]);
    let mut previous = BTreeMap::new();
    while let Some(current) = queue.pop_front() {
        for &edge in edges.get(current).into_iter().flatten() {
            let next = edge["to"].as_str().unwrap();
            if next == target {
                let mut path = vec![edge];
                let mut cursor = current;
                while cursor != source {
                    let prev: &Value = previous[cursor];
                    path.push(prev);
                    cursor = prev["from"].as_str().unwrap();
                }
                path.reverse();
                return path;
            }
            if transitive && visited.insert(next) {
                previous.insert(next, edge);
                queue.push_back(next);
            }
        }
    }
    Vec::new()
}

/// Capture a model and selected source tree, then assess every requested rule.
/// A second inventory detects changes during this observation.
pub fn check_code(path: &Path, options: CodeCheckOptions<'_>) -> io::Result<Value> {
    observe(path, options, || Ok(()))
}

/// Exercise concurrent source changes deterministically in the conformance suite.
#[cfg(feature = "conformance")]
#[doc(hidden)]
pub fn check_code_with_capture_hook(
    path: &Path,
    options: CodeCheckOptions<'_>,
    after_imports: impl FnOnce() -> io::Result<()>,
) -> io::Result<Value> {
    observe(path, options, after_imports)
}

fn observe(
    path: &Path,
    options: CodeCheckOptions<'_>,
    after_imports: impl FnOnce() -> io::Result<()>,
) -> io::Result<Value> {
    let CodeCheckOptions {
        root,
        implementation,
        profile,
        records,
        scope,
        exclude,
        runtime,
    } = options;
    let bad = |s| io::Error::new(io::ErrorKind::InvalidInput, s);
    if !crate::valid_id(implementation) {
        return Err(bad("Select an explicit implementation ID."));
    }
    if scope.iter().any(|s| !crate::valid_id(s)) {
        return Err(bad("Scope must contain rule IDs."));
    }
    if profile.is_empty() {
        return Err(bad("Select an explicit observer profile."));
    }
    let mut exclusions = exclude.to_vec();
    for p in &exclusions {
        glob::validate(p)?;
    }
    exclusions.sort();
    exclusions.dedup();
    let requested: BTreeSet<_> = scope.iter().map(String::as_str).collect();
    let mut report = json!({"format":"gdd-code-observation/0.1","tool":{"name":"gdd","version":env!("CARGO_PKG_VERSION")},
        "runtime":{"python":runtime.version,"implementation":"CPython"},"profile":profile,"implementation":implementation,"spec_revision":null,"code_revision":null,
        "scope":requested,"source_root":std::path::absolute(root)?.display().to_string(),"exclude":exclusions,"ignored_directories":inventory::IGNORED,
        "omitted":[],"files":{},"bindings":{},"imports":[],"edges":[],"constraints":[],"checks":[],"result":"UNKNOWN","advisory":[runtime.provenance()],
        "notice":"Static source dependencies under an explicit local import-root assumption; no project code is executed. This observation grants no authority or trusted implementation evidence."});
    if profile != "python-imports/0.1" {
        return Ok(unknown(
            report,
            "CODE_PROFILE_UNSUPPORTED",
            &format!("Unsupported code observer profile: {profile}"),
        ));
    }
    let mut captured = bundle::load_for_inspection(path);
    let inspected = inspection::inspect(&captured);
    let Some(manifest) = inspected.root.filter(|m| m.kind.is_manifest()) else {
        return Ok(unknown(
            report,
            "CODE_MODEL_UNRESOLVED",
            "A supported model is required.",
        ));
    };
    let revision = inspected.revision;
    report["spec_revision"] = json!(revision);
    if revision.is_none()
        || inspected
            .checks
            .iter()
            .any(|c| c.result != CheckResult::Pass)
        || super::projection_checks(&captured, manifest)
            .iter()
            .any(|c| c.result != CheckResult::Pass)
    {
        return Ok(unknown(
            report,
            "CODE_MODEL_UNRESOLVED",
            "The model and its declared assets must resolve and validate before code observation.",
        ));
    }
    if let Some(records) = records
        && let Err(message) = admit_records(&mut captured, records)
    {
        return Ok(unknown(report, "CODE_INPUT_UNAVAILABLE", &message));
    }
    let root = match bundle::resolve_local(root) {
        Ok(path) if path.is_dir() => path,
        Ok(_) => {
            return Ok(unknown(
                report,
                "CODE_INPUT_UNAVAILABLE",
                "Select an existing Python source root directory.",
            ));
        }
        Err(e) => return Ok(unknown(report, "CODE_INPUT_UNAVAILABLE", &e.message)),
    };
    report["source_root"] = crate::receipt_path(&root)?.into();
    let manifest = captured.manifest().expect("admitted manifest");
    let model = semantics(&captured, manifest, &[]);
    let mut constraints = Vec::new();
    let mut rules = BTreeSet::new();
    for (&id, &node) in &model.nodes {
        if string(node, "kind") != "rule" {
            continue;
        }
        if let Some(rule) = get(node, "rule").and_then(|r| get(r, "constraint")) {
            if !requested.is_empty() && !requested.contains(id) {
                continue;
            }
            rules.insert(id);
            let mut c = validation::to_json(rule)?;
            c["rule"] = id.into();
            constraints.push(c);
        }
    }
    if !requested.is_subset(&rules) {
        return Ok(unknown(
            report,
            "CODE_SCOPE_UNRESOLVED",
            "Scope must select existing rules with explicit architecture constraints.",
        ));
    }
    report["scope"] = json!(rules);
    report["constraints"] = json!(constraints);
    if constraints.is_empty() {
        return Ok(unknown(
            report,
            "CODE_CONSTRAINT_MISSING",
            "No explicit architecture constraint was selected; no vacuous PASS.",
        ));
    }
    assess_sources(
        report,
        &model,
        &options,
        &root,
        &exclusions,
        &constraints,
        after_imports,
    )
}

fn admit_records(captured: &mut bundle::Bundle, path: &Path) -> Result<(), String> {
    const INVALID: &str = "Additional records must be a supported, schema-valid records document.";
    let before = captured.diagnostics().len();
    bundle::add_record_input(captured, path).map_err(|d| d.message.to_string())?;
    if let Some(issue) = captured.diagnostics()[before..].first() {
        let reason = match issue.code {
            "RECORD_INVALID" if issue.message.contains("another project") => {
                "Additional records belong to a different project."
            }
            "ID_COLLISION" if issue.message.contains("loaded twice") => {
                "Additional records duplicate existing record IDs."
            }
            _ => INVALID,
        };
        return Err(reason.into());
    }
    if inspection::inspect(captured)
        .checks
        .iter()
        .any(|c| c.result != CheckResult::Pass)
    {
        return Err(INVALID.into());
    }
    Ok(())
}

fn assess_sources(
    mut report: Value,
    model: &Model<'_>,
    options: &CodeCheckOptions<'_>,
    root: &Path,
    exclusions: &[String],
    constraints: &[Value],
    after_imports: impl FnOnce() -> io::Result<()>,
) -> io::Result<Value> {
    let implementation = options.implementation;
    let runtime = options.runtime;
    let inventory = inventory::scan(root, exclusions);
    let hashes: BTreeMap<_, _> = inventory
        .sources
        .iter()
        .map(|(p, b)| (p.clone(), crate::sha256(b)))
        .collect();
    report["omitted"] = json!(inventory.omitted);
    report["files"] = json!(hashes);
    if !hashes.is_empty() {
        report["code_revision"] = crate::canonical_digest(&report["files"]).into();
    }
    let bindings = bindings::select(
        model,
        implementation,
        report["spec_revision"].as_str().unwrap(),
        &inventory,
        &hashes,
    )?;
    report["bindings"] = json!(bindings.selected);
    let mut advisory = report["advisory"].take();
    let advisory_entries = advisory.as_array_mut().unwrap();
    advisory_entries.extend(bindings.advisory);
    let mut diagnostics = inventory.diagnostics.clone();
    diagnostics.extend(bindings.diagnostics);
    if hashes.is_empty() {
        diagnostics.push(diagnostic(
            "CODE_SOURCE_UNAVAILABLE",
            "No Python source bytes were available.",
            None,
            &[],
        ));
    }
    let mapped: BTreeSet<_> = bindings.components.values().map(String::as_str).collect();
    for c in constraints {
        for endpoint in ["from", "to"].map(|key| c[key].as_str().unwrap()) {
            if !mapped.contains(endpoint) {
                diagnostics.push(diagnostic(
                    "CODE_BINDING_SOURCE_MISSING",
                    "Constraint endpoint has no unambiguous Python binding.",
                    None,
                    &[c["rule"].as_str().unwrap(), endpoint],
                ));
            }
        }
    }
    let mut observed = python_imports::analyze_imports_for_version(
        &inventory.sources,
        &bindings.components,
        &runtime.names,
        runtime.grammar,
    );
    diagnostics.append(observed["diagnostics"].as_array_mut().unwrap());
    mark_omitted_imports(
        observed["imports"].as_array_mut().unwrap(),
        &inventory.omitted,
        runtime,
        &mut diagnostics,
    );
    after_imports()?;
    let drift = inventory != inventory::scan(root, exclusions);
    if drift {
        diagnostics.push(diagnostic(
            "CODE_SOURCE_CHANGED",
            "Source inventory/bytes became unresolved or changed during observation.",
            None,
            &[],
        ));
    }
    for name in inventory
        .sources
        .keys()
        .filter(|p| !bindings.components.contains_key(*p))
    {
        advisory_entries.push(json!({"code":"CODE_UNCLAIMED_SOURCE","file":name,"message":"No unambiguous component binding claims this Python source."}));
    }
    let edges = observed["edges"].as_array().unwrap();
    undeclared_imports(model.bundle, constraints, edges, advisory_entries);
    let (checks, overall) = assess_rules(constraints, edges, &diagnostics, drift);
    report["imports"] = observed["imports"].take();
    report["edges"] = observed["edges"].take();
    report["checks"] = json!(checks);
    report["advisory"] = advisory;
    report["result"] = overall.into();
    Ok(report)
}

fn mark_omitted_imports(
    imports: &mut [Value],
    omitted: &[String],
    runtime: &PythonRuntime,
    diagnostics: &mut Vec<Value>,
) {
    let omitted_roots: BTreeSet<_> = omitted
        .iter()
        .filter_map(|p| p.split('/').next())
        .map(|p| p.strip_suffix(".py").unwrap_or(p))
        .collect();
    for row in imports {
        let module = row["module"].as_str().unwrap();
        let root = module.split('.').next().unwrap();
        if row["external"] == true
            && omitted_roots.contains(root)
            && !runtime.names.contains(module)
            && !runtime.names.contains(root)
        {
            row["external"] = false.into();
            let mut d = diagnostic(
                "CODE_IMPORT_UNRESOLVED",
                "Import intersects omitted local source; external resolution is not established.",
                row["source_file"].as_str(),
                &[],
            );
            d["location"]["line"] = row["line"].clone();
            d["location"]["column"] = row["column"].clone();
            diagnostics.push(d);
        }
    }
}

fn undeclared_imports(
    bundle: &bundle::Bundle,
    constraints: &[Value],
    edges: &[Value],
    advisory: &mut Vec<Value>,
) {
    let mut declared: BTreeSet<_> = bundle
        .model_edges()
        .filter(|e| string(e.value, "relation") == "imports")
        .map(|e| (string(e.value, "from"), string(e.value, "to")))
        .collect();
    declared.extend(
        constraints
            .iter()
            .map(|c| (c["from"].as_str().unwrap(), c["to"].as_str().unwrap())),
    );
    for edge in edges {
        let from = edge["from"].as_str().unwrap();
        let to = edge["to"].as_str().unwrap();
        if from != to && !declared.contains(&(from, to)) {
            advisory.push(json!({"code":"CODE_UNDECLARED_IMPORT","from":from,"to":to,"file":edge["source_file"],"line":edge["line"],"message":"Observed import is not declared; missing imports edges are not a prohibition."}));
        }
    }
}

fn assess_rules(
    constraints: &[Value],
    edges: &[Value],
    diagnostics: &[Value],
    drift: bool,
) -> (Vec<Value>, &'static str) {
    let adjacency = out_edges(edges);
    let mut overall = "PASS";
    let mut checks = Vec::new();
    for (number, c) in constraints.iter().enumerate() {
        let rule = c["rule"].as_str().unwrap();
        let from = c["from"].as_str().unwrap();
        let to = c["to"].as_str().unwrap();
        let reach = c["reach"].as_str().unwrap();
        let supported = c["relation"] == "imports"
            && c["policy"] == "forbid"
            && matches!(reach, "direct" | "transitive");
        let path = if supported && !drift {
            shortest_path(&adjacency, from, to, reach == "transitive")
        } else {
            vec![]
        };
        let mut ds = diagnostics.to_vec();
        if !supported {
            ds.push(diagnostic(
                "CODE_CONSTRAINT_UNSUPPORTED",
                "Constraint has no supported Python import meaning.",
                None,
                &[rule],
            ));
        }
        if let Some(edge) = path.first() {
            let chain = std::iter::once(from)
                .chain(path.iter().map(|e| e["to"].as_str().unwrap()))
                .collect::<Vec<_>>()
                .join(" -> ");
            let mut d = diagnostic(
                "ARCHITECTURE_VIOLATION",
                &format!("{rule} forbids this {reach} import path: {chain}"),
                edge["source_file"].as_str(),
                &[rule, from, to],
            );
            d["severity"] = "error".into();
            d["location"]["line"] = edge["line"].clone();
            d["location"]["column"] = edge["column"].clone();
            ds.push(d);
        }
        let verdict = if !path.is_empty() {
            "FAIL"
        } else if ds.is_empty() {
            "PASS"
        } else {
            "UNKNOWN"
        };
        if overall == "PASS" || verdict == "FAIL" {
            overall = verdict;
        }
        checks.push(json!({"check_id":format!("code.rule.{number}"),"dimension":"integration","result":verdict,"reason":"Observed static Python imports against an explicit forbid constraint.","scope":[rule],"diagnostics":ds,"path":path}));
    }
    (checks, overall)
}
