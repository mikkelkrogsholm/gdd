//! Compare context packages and parallel work declarations with the captured model.

use super::{
    Graph, Model, blank_text, entries, flag, get, has_cycle, items, reachable, string, strings,
    text,
};
use crate::bundle::DocumentKind;
use crate::context::{ContextGraph, ContextScope};
use crate::input::Value;
use crate::report::{Check, CheckResult, Diagnostic};
use crate::validation::CORE_KINDS;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

mod paths;
use paths::{WriteResource, workspace};

const INDEPENDENCE: &str = "INDEPENDENCE_UNRESOLVED";
const CONFLICT: &str = "WRITE_OWNERSHIP_CONFLICT";

fn diagnostic(code: &'static str, result: CheckResult, message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(code, result, message, None, "")
}

fn uncertain(model: &Model<'_>) -> bool {
    let record_only = model.record_only_ids();
    model
        .checks
        .iter()
        .filter(|c| c.dimension == "model" && c.result != CheckResult::Pass)
        .any(|c| {
            let mut ids = c.diagnostics.iter().flat_map(|d| &d.related_ids).peekable();
            ids.peek().is_none() || ids.any(|id| !record_only.contains(id.as_str()))
        })
}

fn quoted(value: &str) -> String {
    let quote = if value.contains('\'') && !value.contains('"') {
        '"'
    } else {
        '\''
    };
    use std::fmt::Write;
    use unicode_general_category::{GeneralCategory as Category, get_general_category};

    let mut label = String::from(quote);
    for ch in value.chars() {
        match ch {
            '\n' => label.push_str("\\n"),
            '\t' => label.push_str("\\t"),
            '\r' => label.push_str("\\r"),
            ch if ch == quote || ch == '\\' => {
                label.push('\\');
                label.push(ch);
            }
            ch if ch != ' '
                && matches!(
                    get_general_category(ch),
                    Category::Control
                        | Category::Format
                        | Category::PrivateUse
                        | Category::Unassigned
                        | Category::LineSeparator
                        | Category::ParagraphSeparator
                        | Category::SpaceSeparator
                ) =>
            {
                let code = u32::from(ch);
                match code {
                    0..=0xff => write!(label, "\\x{code:02x}"),
                    0x100..=0xffff => write!(label, "\\u{code:04x}"),
                    _ => write!(label, "\\U{code:08x}"),
                }
                .expect("writing to a string cannot fail");
            }
            ch => label.push(ch),
        }
    }
    label.push(quote);
    label
}

fn list<'a>(ids: impl IntoIterator<Item = &'a str>) -> String {
    format!(
        "[{}]",
        ids.into_iter().map(quoted).collect::<Vec<_>>().join(", ")
    )
}

pub(super) fn package<'a>(
    model: &Model<'a>,
    package: &Value,
    scope: &[&'a str],
    context: &ContextScope<'a>,
) -> Check {
    let projected = super::context::project(
        model.bundle,
        model.manifest,
        scope.iter().copied().collect(),
        context,
    );
    let mut check = Check::new(
        "context.package",
        "readiness",
        CheckResult::Pass,
        "Candidate work package compared with fixed-point closure.",
    );
    check.scope = scope.iter().copied().map(str::to_owned).collect();
    let manifest = &model.manifest.value;
    let registry = |name| {
        entries(manifest, name)
            .map(|(id, _)| id.as_str())
            .collect::<BTreeSet<_>>()
    };
    let provided_nodes: BTreeSet<_> = strings(package, "nodes").collect();
    for &id in &projected.nodes {
        if !provided_nodes.contains(id) {
            let code = if model.kind(id) == Some("rule") {
                "APPLICABLE_RULE_OMITTED"
            } else {
                "READINESS_CONTRACT_MISSING"
            };
            check.diagnostics.push(
                diagnostic(code, CheckResult::Fail, format!("Context omits {id}."))
                    .with_related_ids([id]),
            );
        }
    }
    for (field, required, known) in [
        (
            "edges",
            projected.edges,
            model.bundle.model_edges().map(|e| e.id).collect(),
        ),
        ("sources", projected.sources, registry("sources")),
        (
            "questions",
            projected.questions.into_iter().collect(),
            registry("questions"),
        ),
        (
            "extensions",
            projected.extensions.into_iter().collect(),
            registry("extensions"),
        ),
        (
            "nodes",
            projected.nodes.into_iter().collect(),
            model.nodes.keys().copied().collect(),
        ),
    ] {
        let supplied: BTreeSet<_> = strings(package, field).collect();
        if field != "nodes" && !required.is_subset(&supplied) {
            check.diagnostics.push(diagnostic(
                "READINESS_CONTRACT_MISSING",
                CheckResult::Fail,
                format!(
                    "Context omits {field}: {}.",
                    list(required.difference(&supplied).copied())
                ),
            ));
        }
        if !supplied.is_subset(&known) {
            check.diagnostics.push(diagnostic(
                "REFERENCE_UNRESOLVED",
                CheckResult::Fail,
                format!(
                    "Context includes unknown {field}: {}.",
                    list(supplied.difference(&known).copied())
                ),
            ));
        }
    }
    if !check.diagnostics.is_empty() {
        check.result = CheckResult::Fail;
    } else if model
        .bundle
        .snapshot()
        .is_none_or(|s| s.revision != string(package, "spec_revision"))
        || uncertain(model)
    {
        check.result = CheckResult::Unknown;
        check.diagnostics.push(diagnostic(
            "REVISION_UNRESOLVED",
            check.result,
            "Package base revision or required model semantics are unknown.",
        ));
    }
    check
}

struct PackageWrites<'a> {
    id: &'a str,
    resources: Vec<WriteResource>,
    workspace: Option<PathBuf>,
}

pub(super) fn parallel<'a>(
    model: &Model<'a>,
    graph: &ContextGraph<'a>,
    plan: &'a Value,
) -> Vec<Check> {
    let packages = items(plan, "packages");
    let ids: BTreeSet<_> = packages.iter().map(|p| string(p, "id")).collect();
    let mut ordered: Graph<_> = ids.iter().map(|&id| (id, Vec::new())).collect();
    let mut bad_order = ids.len() != packages.len();
    for pair in items(plan, "serialized") {
        let Value::Array(pair) = pair else {
            unreachable!("validated serialization pair")
        };
        let (a, b) = (text(&pair[0]).unwrap(), text(&pair[1]).unwrap());
        if !ids.contains(a) || !ids.contains(b) {
            bad_order = true;
        }
        if let Some(children) = ordered.get_mut(a) {
            children.push(b);
        }
    }

    let mut errors = Vec::new();
    let mut unknown = Vec::new();
    let mut unresolved =
        |message: String| unknown.push(diagnostic(INDEPENDENCE, CheckResult::Unknown, message));
    if bad_order || has_cycle(&ordered) {
        errors.push(diagnostic(
            CONFLICT,
            CheckResult::Fail,
            "Invalid or cyclic serialization plan.",
        ));
    }
    let mut writes = Vec::new();
    let mut required_pairs = BTreeSet::new();
    let mut overall = BTreeSet::new();
    for package in packages {
        let scope = strings(package, "scope");
        let context = graph.scope(scope);
        overall.extend(strings(package, "scope"));
        let resources: Vec<_> = strings(package, "write_resources")
            .map(WriteResource::parse)
            .collect();
        let work = get(package, "workspace").and_then(text);
        let workspace = workspace(model, work);
        if resources.iter().any(Option::is_none) || workspace.is_none() {
            unresolved("Ownership needs project-relative POSIX files/directories and a resolvable POSIX workspace; patterns or unsupported selectors are unknown.".into());
        }
        writes.push(PackageWrites {
            id: string(package, "id"),
            resources: resources.into_iter().flatten().collect(),
            workspace,
        });
        let locks: BTreeSet<_> = strings(package, "locked_contracts").collect();
        let mut missing_lock = false;
        for id in context.nodes() {
            let kind = model.kind(id);
            let contract = model
                .nodes
                .get(id)
                .and_then(|node| get(node, "contract"))
                .unwrap_or(&Value::Null);
            if kind.is_none_or(|kind| !CORE_KINDS.contains(&kind))
                || kind == Some("action")
                    && (!flag(contract, "effects_complete")
                        || !flag(contract, "dependencies_complete"))
            {
                unresolved(format!("Unknown effects/dependencies for {id}."));
            }
            if (matches!(kind, Some("action" | "screen"))
                || kind == Some("component") && model.manifest.kind == DocumentKind::Manifest03)
                && !locks.contains(id)
            {
                missing_lock = true;
            }
            if let Some(rule) = model.nodes.get(id).and_then(|node| get(node, "rule"))
                && flag(rule, "blocking")
            {
                required_pairs.extend(strings(rule, "verification").map(|method| (id, method)));
            }
        }
        let integration_plan = string(package, "integration_plan");
        if blank_text(integration_plan)
            || model
                .bundle
                .snapshot()
                .is_none_or(|s| s.revision != string(package, "base_revision"))
        {
            unresolved("Integration plan or compatible contract revision missing.".into());
        }
        if work.is_none() || get(package, "read_resources").is_none() || missing_lock {
            unresolved(
                "Workspace, read ownership or locked boundary contracts are undeclared.".into(),
            );
        }
        if strings(package, "scope")
            .any(|id| matches!(model.kind(id), Some("rule" | "scenario" | "community")))
        {
            unresolved("Package scope is not an implementation target.".into());
        }
    }
    integration_findings(model, plan, &required_pairs, &mut unknown);
    if uncertain(model) {
        unknown.push(diagnostic(
            INDEPENDENCE,
            CheckResult::Unknown,
            "Model context has unresolved validation checks.",
        ));
    }
    ownership_conflicts(&writes, &ordered, &mut errors);
    let mut checks = Vec::new();
    let mut emit = |id, result, message, diagnostics| {
        let mut c = Check::new(id, "readiness", result, message);
        c.scope = overall.iter().copied().map(str::to_owned).collect();
        c.diagnostics = diagnostics;
        checks.push(c);
    };
    if errors.is_empty() && unknown.is_empty() {
        emit(
            "parallel.independence",
            CheckResult::Pass,
            "Declared write resources, effects, revisions and integration plans checked; runtime safety is not proven.",
            Vec::new(),
        );
    } else {
        if !errors.is_empty() {
            emit(
                "parallel.ownership",
                CheckResult::Fail,
                "Parallel write ownership conflicts.",
                errors,
            );
        }
        if !unknown.is_empty() {
            emit(
                "parallel.independence",
                CheckResult::Unknown,
                "Independence requires known declarations and compatible integration plan.",
                unknown,
            );
        }
    }
    checks
}

fn ownership_conflicts<'a>(
    writes: &[PackageWrites<'a>],
    ordered: &Graph<&'a str>,
    errors: &mut Vec<Diagnostic>,
) {
    for (index, left) in writes.iter().enumerate() {
        for right in &writes[index + 1..] {
            let mut overlap = BTreeSet::new();
            for a in &left.resources {
                for b in &right.resources {
                    if a.contains(b) || b.contains(a) {
                        overlap.insert((&a.path, &b.path));
                    }
                }
            }
            let same_workspace = left.workspace.is_some() && left.workspace == right.workspace;
            if overlap.is_empty() && !same_workspace {
                continue;
            }
            if left.id == right.id
                || reachable(ordered, left.id, right.id)
                || reachable(ordered, right.id, left.id)
            {
                continue;
            }
            if !overlap.is_empty() {
                let paths = overlap
                    .into_iter()
                    .map(|(a, b)| format!("({}, {})", quoted(a), quoted(b)))
                    .collect::<Vec<_>>()
                    .join(", ");
                errors.push(diagnostic(
                    CONFLICT,
                    CheckResult::Fail,
                    format!("Unserialized write overlap: [{paths}]."),
                ));
            }
            if same_workspace {
                errors.push(diagnostic(
                    CONFLICT,
                    CheckResult::Fail,
                    "Concurrent writers declare the same workspace.",
                ));
            }
        }
    }
}

fn integration_findings(
    model: &Model<'_>,
    plan: &Value,
    required_pairs: &BTreeSet<(&str, &str)>,
    unknown: &mut Vec<Diagnostic>,
) {
    let mut unresolved =
        |message: String| unknown.push(diagnostic(INDEPENDENCE, CheckResult::Unknown, message));
    let integration = get(plan, "integration").unwrap_or(&Value::Null);
    let current = active_checks(model);
    let mut invalid = BTreeSet::new();
    let mut covered = BTreeSet::new();
    for id in strings(integration, "checks") {
        if let Some(check) = current.get(id) {
            covered.extend(strings(check, "scope").map(|target| (target, string(check, "method"))));
        } else {
            invalid.insert(id);
        }
    }
    if !invalid.is_empty() {
        unresolved(format!(
            "Integration references unknown, invalid, historical or retired checks: {}.",
            list(invalid)
        ));
    }
    if get(integration, "owner").is_none() || !required_pairs.is_subset(&covered) {
        unresolved(
            "Integration owner/checks do not cover every required blocking rule/method pair."
                .into(),
        );
    }
}

fn active_checks<'a>(model: &Model<'a>) -> BTreeMap<&'a str, &'a Value> {
    let Some(snapshot) = model.bundle.snapshot() else {
        return BTreeMap::new();
    };
    let records: BTreeMap<_, _> = model.bundle.records().map(|r| (r.id, r.value)).collect();
    let invalid = model.invalid_records(&records, Some(&snapshot.revision));
    let mut current: BTreeMap<_, _> = records
        .into_iter()
        .filter(|(id, r)| {
            !invalid.contains(id)
                && string(r, "kind") == "check"
                && get(r, "spec_revision").and_then(text) == Some(snapshot.revision.as_str())
        })
        .collect();
    let retired: BTreeSet<_> = current
        .values()
        .filter_map(|r| {
            let previous = get(r, "supersedes").and_then(text)?;
            let old = current.get(previous)?;
            (string(r, "method") == string(old, "method")
                && strings(old, "scope").all(|target| strings(r, "scope").any(|s| s == target)))
            .then_some(previous)
        })
        .collect();
    current.retain(|id, _| !retired.contains(id));
    current
}
