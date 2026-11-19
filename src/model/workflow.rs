//! Derive revision-bound work packets and verify their complete payloads.

use super::{
    Model, assess_readiness, boundaries, context, entries, flag, get, items, planning, semantics,
    string, strings,
};
use crate::bundle::{self, Bundle, DocumentKind};
use crate::input::Value;
use crate::knowledge::KnowledgeIndex;
use crate::report::{Check, CheckResult, Report};
use crate::validation::{self, Profile, ValidationOptions, to_json};
use serde::Serialize;
use serde_json::{Map, Value as Json, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    io,
    path::Path,
    sync::OnceLock,
};

use super::canonical;
pub mod run;

#[derive(Debug, Serialize)]
pub struct WorkflowResult {
    pub format: &'static str,
    pub report: Report,
    pub artifact: Json,
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}

fn require(value: &Value, kind: &'static str, label: &str) -> io::Result<()> {
    static REQUEST: OnceLock<jsonschema::Validator> = OnceLock::new();
    static PLAN: OnceLock<jsonschema::Validator> = OnceLock::new();
    static EVENT: OnceLock<jsonschema::Validator> = OnceLock::new();
    let slot = match kind {
        "request" => &REQUEST,
        "plan" => &PLAN,
        "event" => &EVENT,
        _ => unreachable!(),
    };
    let validator = slot.get_or_init(|| {
        let schema: Json = serde_json::from_str(include_str!("../../schemas/workflow.schema.json"))
            .expect("embedded workflow schema");
        let definition = json!({"$ref":format!("#/$defs/{kind}"),"$defs":schema["$defs"]});
        validation::build_schema(&definition).expect("workflow schema compiles offline")
    });
    validator
        .validate(&to_json(value)?)
        .map_err(|error| invalid(format!("Invalid {label}: {error}")))
}

const LEGACY_PLAN: &str = "gdd-workplan/0.1";
const KNOWLEDGE_PLAN: &str = "gdd-workplan/0.2";

fn has_wiki_context(plan: &Value) -> bool {
    string(plan, "format") == KNOWLEDGE_PLAN
}

pub(crate) fn capture_plan_inputs(path: &Path, scope: &[String], plan: &Value) -> Bundle {
    if has_wiki_context(plan) {
        bundle::load_for_knowledge(path, scope)
    } else {
        bundle::load_for_readiness(path, scope)
    }
}

enum Source<'a> {
    Path(&'a Path),
    Bundle(&'a Bundle),
}

/// Prepare a plan and assess its declarations using a single scoped capture.
pub fn prepare_workflow(
    path: &Path,
    request: &Value,
    host_context: Option<&Value>,
) -> io::Result<WorkflowResult> {
    prepare(Source::Path(path), request, host_context, true)
}

/// Prepare from frozen inputs. Capture all package scopes with
/// `bundle::load_for_knowledge` to include advisory wiki bytes and backlinks.
pub fn prepare_workflow_from_bundle(
    bundle: &Bundle,
    request: &Value,
    host_context: Option<&Value>,
) -> io::Result<WorkflowResult> {
    prepare(Source::Bundle(bundle), request, host_context, true)
}

/// Re-derive every package from its request and compare the complete plan.
pub fn verify_workflow(
    path: &Path,
    plan: &Value,
    host_context: Option<&Value>,
) -> io::Result<Report> {
    verify(Source::Path(path), plan, host_context)
}

/// Verify against an existing model, ledger and contract capture.
pub fn verify_workflow_from_bundle(
    bundle: &Bundle,
    plan: &Value,
    host_context: Option<&Value>,
) -> io::Result<Report> {
    verify(Source::Bundle(bundle), plan, host_context)
}

fn verify(source: Source<'_>, plan: &Value, host: Option<&Value>) -> io::Result<Report> {
    require(plan, "plan", "workplan")?;
    verify_valid_plan(source, plan, host)
}

fn verify_valid_plan(source: Source<'_>, plan: &Value, host: Option<&Value>) -> io::Result<Report> {
    let expected = prepare(
        source,
        get(plan, "request").expect("validated plan"),
        host,
        has_wiki_context(plan),
    )?;
    let mut report = expected.report;
    let mut check = Check::new(
        "workflow.packet_integrity",
        "readiness",
        if canonical::bytes(&to_json(plan)?) == canonical::bytes(&expected.artifact) {
            CheckResult::Pass
        } else {
            CheckResult::Fail
        },
        "Complete packet must match the current model, assets, selected check declarations and original request.",
    );
    check.scope.clone_from(&report.scope);
    report.checks.push(check);
    Ok(report)
}

fn prepare(
    source: Source<'_>,
    request: &Value,
    host: Option<&Value>,
    include_knowledge: bool,
) -> io::Result<WorkflowResult> {
    require(request, "request", "workflow request")?;
    if let Some(host) = validation::present(host) {
        validation::validate_input("host", host)?;
    }
    let scope: Vec<_> = items(request, "packages")
        .iter()
        .flat_map(|p| strings(p, "scope"))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(str::to_owned)
        .collect();
    let capture;
    let bundle = match source {
        Source::Path(path) => {
            capture = if include_knowledge {
                bundle::load_for_knowledge(path, &scope)
            } else {
                bundle::load_for_readiness(path, &scope)
            };
            &capture
        }
        Source::Bundle(bundle) => bundle,
    };
    let known = "A known model snapshot is required to derive a workplan.";
    let manifest = bundle.manifest().ok_or_else(|| invalid(known))?;
    let snapshot = bundle.snapshot().ok_or_else(|| invalid(known))?;
    let mut report = validation::validate(bundle, Profile::Representation);
    if report.spec_revision.is_none() || bundle.nodes().len() == 0 {
        return Err(invalid(known));
    }
    // An operational error can fail the report on an otherwise derivable graph.
    // Validate core semantics independently before including record diagnostics.
    let available = report.result("model") == CheckResult::Pass;
    let mut model = semantics(bundle, manifest, &[]);
    if model.checks.iter().any(|c| c.result != CheckResult::Pass) {
        return Err(invalid(
            "Cannot derive packets from unresolved model semantics; run gdd validate first.",
        ));
    }
    if available {
        if bundle.records().len() != 0 {
            model.records();
        }
        report.checks.extend_from_slice(&model.checks);
    }
    report.profiles = vec![Profile::Model, Profile::Readiness];
    report.scope.clone_from(&scope);
    if scope
        .iter()
        .any(|id| !model.nodes.contains_key(id.as_str()))
    {
        return Err(invalid("Unknown context scope."));
    }
    let graph = context::ContextGraph::new(bundle);
    let integration = get(request, "integration").expect("validated request");
    let selected: BTreeSet<_> = strings(integration, "checks").collect();
    let active = selected_checks(&model, &selected, available);
    let knowledge = if include_knowledge
        && entries(&manifest.value, "sources")
            .any(|(_, source)| crate::knowledge::is_wiki_source(source))
    {
        Some(KnowledgeIndex::from_bundle(bundle)?)
    } else {
        None
    };
    let packages = items(request, "packages")
        .iter()
        .map(|p| packet(&model, &graph, p, &active, knowledge.as_ref()))
        .collect::<io::Result<Vec<_>>>()?;
    let required = requirements(
        &model,
        graph.scope(scope.iter().map(String::as_str)).nodes(),
        &active,
    );
    let used: BTreeSet<_> = required.iter().flat_map(|r| &r.checks).copied().collect();
    let complete = required.iter().all(|r| r.checks.len() == 1)
        && used == selected
        && active.keys().copied().collect::<BTreeSet<_>>() == selected;
    let readiness = |package: Option<&Value>, parallel_plan: Option<&Value>| {
        let options = ValidationOptions {
            profiles: &[Profile::Readiness],
            package,
            parallel_plan,
            host_context: host,
            ..Default::default()
        };
        let scope = options.effective_scope()?;
        if available {
            Ok(assess_readiness(&model, &graph, &options, &scope).0)
        } else {
            let scope = scope.into_iter().map(str::to_owned).collect::<Vec<_>>();
            Ok(vec![validation::unavailable_check(
                "readiness.unavailable",
                "readiness",
                &scope,
            )])
        }
    };
    assess_packages(&model, request, &packages, complete, readiness, &mut report)?;
    // Preserve the wiki-aware profile even while no packet links to a page.
    // A later backlink can be the first reference connecting a page to work.
    let plan_format = if knowledge.is_some() {
        KNOWLEDGE_PLAN
    } else {
        LEGACY_PLAN
    };
    let mut artifact = json!({
        "format":plan_format, "request":to_json(request)?,
        "project_id": string(get(&manifest.value, "project").expect("validated manifest"), "id"),
        "spec_revision":snapshot.revision, "snapshot":snapshot, "scope":scope, "packages":packages,
        "notice":"Derived product data; not authorization, executed evidence or an instruction to launch tools."
    });
    artifact["revision"] = canonical::digest(&artifact).into();
    Ok(WorkflowResult {
        format: "gdd-workflow-result/0.1",
        report,
        artifact,
    })
}

fn selected_checks<'a>(
    model: &Model<'a>,
    selected: &BTreeSet<&str>,
    available: bool,
) -> BTreeMap<&'a str, &'a Value> {
    let bundle = model.bundle;
    let records = if available {
        bundle.records().map(|r| (r.id, r.value)).collect()
    } else {
        // Invalid ledgers cannot contribute selections or shadow a later valid
        // declaration. Keep valid sibling ledgers in the derivation.
        let mut records = BTreeMap::new();
        for (index, doc) in bundle.documents().iter().enumerate() {
            if doc.kind == DocumentKind::Records
                && bundle
                    .representation_check(index)
                    .is_some_and(|c| c.result == CheckResult::Pass)
            {
                for (id, record) in entries(&doc.value, "records") {
                    records.entry(id.as_str()).or_insert(record);
                }
            }
        }
        records
    };
    let mut active = planning::select_active_checks(model, records);
    active.retain(|id, _| selected.contains(id));
    active
}

fn scoped_check(id: impl Into<String>, passes: bool, reason: &str, scope: &[String]) -> Check {
    let mut check = Check::new(
        id,
        "readiness",
        if passes {
            CheckResult::Pass
        } else {
            CheckResult::Unknown
        },
        reason,
    );
    check.scope = scope.to_owned();
    check
}

fn assess_packages(
    model: &Model<'_>,
    request: &Value,
    packages: &[Json],
    complete: bool,
    readiness: impl Fn(Option<&Value>, Option<&Value>) -> io::Result<Vec<Check>>,
    report: &mut Report,
) -> io::Result<()> {
    let mut ids = BTreeSet::new();
    if items(request, "packages")
        .iter()
        .any(|p| !ids.insert(string(p, "id")))
    {
        return Err(invalid("Package IDs must be unique."));
    }
    for (p, packet) in items(request, "packages").iter().zip(packages) {
        let id = string(p, "id");
        let envelope = Value::from(&packet["context"]);
        for mut check in readiness(Some(&envelope), None)? {
            check.check_id = format!("{id}.{}", check.check_id);
            report.checks.push(check);
        }
        let scope = strings(&envelope, "scope")
            .map(str::to_owned)
            .collect::<Vec<_>>();
        report.checks.push(scoped_check(
            format!("{id}.ownership"),
            planning::valid_ownership(model, p),
            "Explicit logical file ownership and workspace declarations; actual isolation must be verified by the host.",
            &scope,
        ));
    }
    report.checks.push(scoped_check(
        "workflow.check_plans",
        complete,
        "Selected active checks must cover every blocking rule and method.",
        &report.scope,
    ));
    if packages.len() > 1 {
        let envelope = parallel_envelope(request, packages);
        report.checks.extend(readiness(None, Some(&envelope))?);
    } else if !items(request, "serialized").is_empty() {
        return Err(invalid("A singleton plan cannot contain serialized pairs."));
    }

    Ok(())
}

fn parallel_envelope(request: &Value, packets: &[Json]) -> Value {
    let packages = packets
        .iter()
        .map(|p| {
            let fields = [
                "id",
                "scope",
                "write_resources",
                "read_resources",
                "locked_contracts",
                "workspace",
                "base_revision",
                "integration_plan",
            ];
            Value::Object(
                fields
                    .into_iter()
                    .map(|field| (field.to_owned(), Value::from(&p[field])))
                    .collect(),
            )
        })
        .collect();
    Value::Object(BTreeMap::from([
        (
            "format".into(),
            Value::String("gdd-parallel-plan/0.2".into()),
        ),
        ("packages".into(), Value::Array(packages)),
        (
            "serialized".into(),
            get(request, "serialized")
                .expect("validated request")
                .clone(),
        ),
        (
            "integration".into(),
            get(request, "integration")
                .expect("validated request")
                .clone(),
        ),
    ]))
}

#[derive(Serialize)]
struct RequiredCheck<'a> {
    rule: &'a str,
    method: &'a str,
    checks: Vec<&'a str>,
}

fn requirements<'a>(
    model: &Model<'a>,
    nodes: impl Iterator<Item = &'a str>,
    active: &BTreeMap<&'a str, &'a Value>,
) -> Vec<RequiredCheck<'a>> {
    let mut required = Vec::new();
    for id in nodes {
        if let Some(rule) = get(model.nodes[id], "rule").filter(|r| flag(r, "blocking")) {
            for method in strings(rule, "verification") {
                let checks = active
                    .iter()
                    .filter(|(_, c)| {
                        string(c, "method") == method && strings(c, "scope").any(|s| s == id)
                    })
                    .map(|(id, _)| *id)
                    .collect();
                required.push(RequiredCheck {
                    rule: id,
                    method,
                    checks,
                });
            }
        }
    }
    required
}

/// Preserve every direction and locator while shipping each piece of prose once.
fn package_knowledge(scoped: KnowledgeIndex) -> io::Result<Json> {
    let mut excerpts = BTreeMap::new();
    let mut links = Vec::with_capacity(scoped.links.len());
    for mut link in scoped.links {
        let excerpt = link.excerpt.take();
        let mut serialized = serde_json::to_value(link)?;
        let fields = serialized
            .as_object_mut()
            .expect("serialized knowledge link");
        fields.remove("excerpt");
        if let Some(text) = excerpt {
            let id = format!("sha256:{}", crate::sha256(text.as_bytes()));
            excerpts.entry(id.clone()).or_insert(text);
            fields.insert("excerpt_ref".into(), id.into());
        }
        links.push(serialized);
    }
    Ok(json!({
        "format": "gdd-package-knowledge/0.1",
        "project_id": scoped.project_id,
        "spec_revision": scoped.spec_revision,
        "pages": scoped.pages,
        "questions": scoped.questions,
        "links": links,
        "excerpts": excerpts,
    }))
}

fn packet<'a>(
    model: &Model<'a>,
    graph: &context::ContextGraph<'a>,
    package: &'a Value,
    active: &BTreeMap<&'a str, &'a Value>,
    knowledge: Option<&KnowledgeIndex>,
) -> io::Result<Json> {
    let scope: BTreeSet<_> = strings(package, "scope").collect();
    let selected = graph.scope(scope.iter().copied());
    let context = context::project(model.bundle, model.manifest, scope, &selected);
    let is_03 = model.manifest.kind == DocumentKind::Manifest03;
    let mut slice = json!({});
    let mut locations = Map::new();
    for (registry, ids) in [
        ("nodes", context.nodes.clone()),
        ("sources", context.sources.iter().copied().collect()),
        ("questions", context.questions.clone()),
        ("extensions", context.extensions.clone()),
    ] {
        let mut objects = Map::new();
        for id in ids {
            let value = if registry == "nodes" {
                model.nodes[id]
            } else {
                get(&model.manifest.value, registry)
                    .and_then(|objects| get(objects, id))
                    .expect("validated model reference")
            };
            objects.insert(id.into(), to_json(value)?);
            if let Some(loc) = model.bundle.location(id) {
                locations.insert(
                    id.into(),
                    json!([model.bundle.declared_fragment_path(&loc.file), loc.pointer]),
                );
            }
        }
        slice[registry] = Json::Object(objects);
    }
    let mut edges: BTreeMap<_, _> = BTreeMap::new();
    for e in model
        .bundle
        .edges()
        .filter(|e| context.edges.contains(e.id))
    {
        edges.insert(e.id, to_json(e.value)?);
        locations.insert(
            e.id.into(),
            json!([
                model.bundle.declared_fragment_path(&e.location.file),
                e.location.pointer
            ]),
        );
    }
    slice["edges"] = Json::Array(edges.into_values().collect());
    let locked: Vec<_> = context
        .nodes
        .iter()
        .copied()
        .filter(|id| {
            let kind = string(model.nodes[id], "kind");
            matches!(kind, "action" | "screen") || is_03 && kind == "component"
        })
        .collect();
    let required = requirements(model, selected.nodes(), active);
    let checks = active
        .iter()
        .map(|(id, v)| Ok(((*id).to_owned(), to_json(v)?)))
        .collect::<io::Result<Map<_, _>>>()?;
    let extensions = model
        .bundle
        .extensions()
        .map(|(id, doc)| Ok((id.to_owned(), to_json(&doc.value)?)))
        .collect::<io::Result<Map<_, _>>>()?;
    let mut packet = to_json(package)?;
    let out = packet.as_object_mut().expect("validated package");
    let Json::Object(fields) = json!({
        "scope":context.scope, "implementation_targets":selected.implementation_targets,
        "context":context, "model_slice":slice,"locations":locations,"locked_contracts":locked,
        "required_checks":required,"check_plans":checks,"extension_semantics":extensions,"base_revision":context.spec_revision,
    }) else {
        unreachable!()
    };
    out.extend(fields);
    if let Some(index) = knowledge {
        let targets = context
            .nodes
            .iter()
            .chain(&context.edges)
            .chain(&context.questions)
            .map(|id| (*id).to_owned())
            .collect::<Vec<_>>();
        let scoped = index.select(&targets, None)?;
        if !scoped.links.is_empty() {
            packet["knowledge"] = package_knowledge(scoped)?;
        }
    }
    if is_03 {
        let nodes: Vec<_> = context.nodes.iter().copied().map(str::to_owned).collect();
        packet["boundaries"] = boundaries::boundary_view(model.bundle, Some(&nodes))?;
        packet["design_review"] = boundaries::design_checklist(model.bundle, &nodes)?.into();
    }
    Ok(packet)
}
