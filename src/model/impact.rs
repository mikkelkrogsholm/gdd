//! Advisory change impact in declared dependencies of one captured model.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, VecDeque, btree_map::Entry};
use std::fmt::Write as _;
use std::io;
use std::path::Path;

use serde::Serialize;
use serde_json::Value as Json;

use super::context::{ContextGraph, ContextLink, WitnessStep};
use super::{Model, entries, flag, get, string, strings, text};
use crate::bundle::{self, Bundle, Document, DocumentKind};
use crate::input::Value;
use crate::report::{Check, CheckResult, Diagnostic};
use crate::validation::{CORE_KINDS, to_json};

#[derive(Debug, Serialize)]
pub struct ImpactCoverage {
    pub result: &'static str,
    pub basis: &'static str,
    pub runtime_callers: &'static str,
    pub unlisted_dependencies: &'static str,
    pub unspecified_declarations: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ImpactRoot {
    pub id: String,
    pub items: Vec<ImpactItem>,
}

#[derive(Debug, Serialize)]
pub struct ImpactItem {
    pub id: String,
    pub kind: String,
    pub category: &'static str,
    pub reason: &'static str,
    pub witness: Vec<WitnessStep<String>>,
}

#[derive(Debug, Serialize)]
pub struct ImpactRecord {
    pub id: String,
    pub kind: &'static str,
    pub action: &'static str,
    pub targets: Vec<String>,
    pub roots: BTreeSet<String>,
    pub spec_revision: String,
    pub implementation: Option<String>,
    pub record_revision: String,
    pub declaration: Json,
    pub provenance: &'static str,
}

#[derive(Debug, Serialize)]
pub struct OmittedRecord {
    pub id: String,
    pub reason: &'static str,
}

#[derive(Debug, Serialize)]
pub struct RecordCoverage {
    pub basis: &'static str,
    pub code_bindings: &'static str,
}

#[derive(Debug, Serialize)]
pub struct ImpactReport {
    pub format: &'static str,
    pub spec_revision: Option<String>,
    pub scope: Vec<String>,
    pub implementation: Option<String>,
    pub coverage: ImpactCoverage,
    pub roots: Vec<ImpactRoot>,
    pub records: Vec<ImpactRecord>,
    pub records_omitted: Vec<OmittedRecord>,
    pub record_coverage: RecordCoverage,
    pub boundaries: Option<Json>,
    pub diagnostics: Vec<Diagnostic>,
    pub exit_code: u8,
}

/// Capture the manifest, its normative assets and declared ledgers once.
pub fn analyze_impact(
    path: &Path,
    scope: &[String],
    implementation: Option<&str>,
) -> io::Result<ImpactReport> {
    let mut report = ImpactReport::new(scope, implementation);
    if report.exit_code == 0 {
        report.assess(&bundle::load_for_inspection(path))?;
    }
    Ok(report)
}

/// Analyze a captured bundle without reading the filesystem.
pub fn analyze_impact_from_bundle(
    bundle: &Bundle,
    scope: &[String],
    implementation: Option<&str>,
) -> io::Result<ImpactReport> {
    let mut report = ImpactReport::new(scope, implementation);
    if report.exit_code == 0 {
        report.assess(bundle)?;
    }
    Ok(report)
}

impl ImpactReport {
    fn new(scope: &[String], implementation: Option<&str>) -> Self {
        let mut report = Self {
            format: "gdd-impact/0.1",
            spec_revision: None,
            scope: Vec::new(),
            implementation: implementation.map(str::to_owned),
            coverage: ImpactCoverage {
                result: "unavailable",
                basis: "declared-model",
                runtime_callers: "unassessed",
                unlisted_dependencies: "unassessed",
                unspecified_declarations: Vec::new(),
            },
            roots: Vec::new(),
            records: Vec::new(),
            records_omitted: Vec::new(),
            record_coverage: RecordCoverage {
                basis: "declared-records-only",
                code_bindings: "unassessed",
            },
            boundaries: None,
            diagnostics: Vec::new(),
            exit_code: 0,
        };
        if scope.is_empty() || scope.iter().any(|id| !crate::valid_id(id)) {
            report.issue(
                "IMPACT_SCOPE_INVALID",
                "Select one or more syntactically valid current-model node IDs.",
                None,
                CheckResult::Fail,
            );
        } else {
            report.scope = scope.to_owned();
            report.scope.sort();
            report.scope.dedup();
            if implementation.is_some_and(|id| !crate::valid_id(id)) {
                report.issue(
                    "IMPACT_IMPLEMENTATION_INVALID",
                    "Implementation must be a GDD ID.",
                    None,
                    CheckResult::Fail,
                );
            }
        }
        report
    }

    fn issue(&mut self, code: &'static str, message: &str, id: Option<&str>, verdict: CheckResult) {
        self.diagnostics
            .push(Diagnostic::new(code, verdict, message, None, "").with_related_ids(id));
        self.combine(verdict);
    }

    fn combine(&mut self, verdict: CheckResult) {
        match verdict {
            CheckResult::Fail => self.exit_code = 1,
            CheckResult::Unknown if self.exit_code == 0 => self.exit_code = 2,
            _ => {}
        }
    }

    fn include_checks(&mut self, checks: &[Check]) {
        for check in checks {
            if check.result != CheckResult::Pass {
                self.combine(check.result);
                self.diagnostics.extend_from_slice(&check.diagnostics);
            }
        }
    }

    fn admit<'a>(&mut self, bundle: &'a Bundle) -> Option<&'a Document> {
        let inspected = super::inspection::inspect(bundle);
        self.spec_revision = inspected.revision;
        let manifest = inspected.root.filter(|doc| doc.kind.is_manifest());
        if manifest.is_none() {
            self.issue(
                "FORMAT_UNSUPPORTED",
                "Impact requires a supported current GDD model manifest.",
                None,
                CheckResult::Unknown,
            );
        }
        self.include_checks(&inspected.checks);
        manifest.filter(|_| self.exit_code == 0)
    }

    fn assess(&mut self, bundle: &Bundle) -> io::Result<()> {
        let Some(manifest) = self.admit(bundle) else {
            return Ok(());
        };
        // Record integrity is applied to individual candidates after model
        // admission. A historical or invalid check cannot hide a valid binding.
        let model = super::semantics(bundle, manifest, &[]);
        self.include_checks(&model.checks);
        let model_invalid = self.exit_code == 1;
        let unknown: Vec<_> = self
            .scope
            .iter()
            .filter(|id| !model.nodes.contains_key(id.as_str()))
            .cloned()
            .collect();
        for id in unknown {
            self.issue(
                "IMPACT_SCOPE_UNKNOWN",
                "Impact seeds must identify current-model nodes, not edges, sources or records.",
                Some(&id),
                CheckResult::Fail,
            );
        }
        if model_invalid {
            return Ok(());
        }
        let kinds: BTreeSet<_> = bundle
            .extensions()
            .flat_map(|(_, d)| entries(&d.value, "node_kinds"))
            .map(|(id, _)| id.as_str())
            .collect();
        for (&id, node) in &model.nodes {
            let kind = string(node, "kind");
            if !CORE_KINDS.contains(&kind) && !kinds.contains(kind) {
                self.issue(
                    "EXTENSION_UNSUPPORTED",
                    "Impact cannot establish semantics for this extension node kind.",
                    Some(id),
                    CheckResult::Unknown,
                );
            }
        }
        let graph = ContextGraph::new(bundle);
        let dependencies = dependencies(bundle, &graph);
        let mut affected: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
        for id in &self.scope {
            if let Some((&root, _)) = model.nodes.get_key_value(id.as_str()) {
                self.roots.push(impact_root(
                    &model,
                    &graph,
                    &dependencies,
                    root,
                    &mut affected,
                ));
            }
        }
        for &id in affected.keys() {
            let node = model.nodes[id];
            match get(node, "contract").and_then(|c| get(c, "dependencies_complete")) {
                Some(Value::Boolean(false)) => self.issue(
                    "IMPACT_DEPENDENCIES_INCOMPLETE",
                    "Declared action dependencies are explicitly incomplete; omitted consumers cannot be excluded.",
                    Some(id),
                    CheckResult::Unknown,
                ),
                None if string(node, "kind") == "action" => {
                    self.coverage.unspecified_declarations.push(id.to_owned());
                }
                _ => {}
            }
        }
        if manifest.kind == DocumentKind::Manifest03 {
            let scope: Vec<_> = affected.keys().map(|&id| id.to_owned()).collect();
            self.boundaries = Some(super::boundaries::boundary_view(bundle, Some(&scope))?);
        }
        self.select_records(&model, &affected)?;
        self.coverage.result = if self.exit_code == 0 {
            "complete"
        } else {
            "partial"
        };
        Ok(())
    }

    fn select_records(
        &mut self,
        model: &Model<'_>,
        affected: &BTreeMap<&str, BTreeSet<&str>>,
    ) -> io::Result<()> {
        let records: BTreeMap<_, _> = model.bundle.records().map(|r| (r.id, r.value)).collect();
        let revision = self.spec_revision.as_deref();
        let invalid = model.invalid_records(&records, revision);
        let current = records
            .iter()
            .filter(|(_, r)| get(r, "spec_revision").and_then(text) == revision)
            .map(|(&id, &r)| (id, r))
            .collect();
        // Without host verification only compatible check plans can retire.
        let retired = super::implementation::retired(&current, &invalid, &Value::Null);
        let mut code_bindings = "absent";
        for (id, record) in records {
            let (kind, action, field) = match string(record, "kind") {
                "check" => ("check", "rerun", "scope"),
                "binding" => ("binding", "inspect", "targets"),
                _ => continue,
            };
            let targets: BTreeSet<_> = strings(record, field)
                .filter(|target| affected.contains_key(target))
                .collect();
            if targets.is_empty() {
                continue;
            }
            let implementation = get(record, "implementation").and_then(text);
            let omitted = if retired.contains(id) {
                Some("Retired by a valid compatible current-model check-plan supersession.")
            } else if get(record, "spec_revision").and_then(text) != revision
                || invalid.contains(id)
            {
                Some("Historical revision or invalid record; never rebound to the selected model.")
            } else if self
                .implementation
                .as_deref()
                .is_some_and(|selected| implementation.is_some_and(|other| other != selected))
            {
                Some("Different implementation.")
            } else {
                None
            };
            if let Some(reason) = omitted {
                self.records_omitted.push(OmittedRecord {
                    id: id.into(),
                    reason,
                });
                continue;
            }
            if kind == "binding" && string(record, "role") == "code" {
                code_bindings = "present";
            }
            let roots = targets
                .iter()
                .flat_map(|target| affected[target].iter().copied())
                .map(str::to_owned)
                .collect();
            self.records.push(ImpactRecord {
                id: id.into(),
                kind,
                action,
                targets: targets.into_iter().map(str::to_owned).collect(),
                roots,
                spec_revision: string(record, "spec_revision").into(),
                implementation: implementation.map(str::to_owned),
                record_revision: super::records::record_digest(record),
                declaration: to_json(record)?,
                provenance: "Declared inspection/rerun candidate only; code freshness and execution unassessed.",
            });
        }
        self.record_coverage.code_bindings = code_bindings;
        Ok(())
    }
}

type Adjacency<'a> = BTreeMap<&'a str, Vec<ContextLink<'a>>>;
type Paths<'a> = BTreeMap<&'a str, Vec<ContextLink<'a>>>;

fn dependencies<'a>(bundle: &'a Bundle, graph: &ContextGraph<'a>) -> Adjacency<'a> {
    let custom: BTreeSet<_> = bundle
        .extensions()
        .flat_map(|(_, doc)| entries(&doc.value, "relations"))
        .filter(|(_, r)| flag(r, "dependency"))
        .map(|(name, _)| name.as_str())
        .collect();
    let mut adjacency = Adjacency::new();
    for link in graph.links() {
        let (derivation, orientation) = match (link.derivation, link.orientation, link.relation) {
            ("type-reference", _, _) => ("type-reference-consumer", "semantic"),
            ("machine-trigger", _, _) => ("machine-trigger-consumer", "semantic"),
            ("declared-context", "forward", Some(relation))
                if matches!(
                    relation,
                    "calls"
                        | "depends_on"
                        | "imports"
                        | "reads"
                        | "creates"
                        | "changes"
                        | "navigates_to"
                        | "state_of"
                ) || custom.contains(relation) =>
            {
                ("declared-dependency-consumer", "reverse")
            }
            _ => continue,
        };
        let consumer = link.from.expect("declared context arc starts at a node");
        adjacency.entry(link.to).or_default().push(ContextLink {
            from: Some(link.to),
            to: consumer,
            orientation,
            derivation,
            ..*link
        });
    }
    for links in adjacency.values_mut() {
        links.sort_by_key(|l| (l.to, l.edge, l.derivation));
        links.dedup();
    }
    adjacency
}

fn consumer_paths<'a>(root: &'a str, adjacency: &Adjacency<'a>) -> Paths<'a> {
    let mut paths = Paths::from([(root, Vec::new())]);
    let mut pending = VecDeque::from([root]);
    while let Some(source) = pending.pop_front() {
        if let Some(links) = adjacency.get(source) {
            for &link in links {
                if !paths.contains_key(link.to) {
                    let mut path = paths[source].clone();
                    path.push(link);
                    paths.insert(link.to, path);
                    pending.push_back(link.to);
                }
            }
        }
    }
    paths
}

fn witness_order(left: &[ContextLink<'_>], right: &[ContextLink<'_>]) -> Ordering {
    // The portable tie-break compares the printed list of ordered dictionaries.
    // IDs use the GDD ASCII grammar, whose quoted strings sort before None.
    fn key<'a>(link: &ContextLink<'a>) -> impl Ord + use<'a> {
        (
            link.from.is_none(),
            link.from,
            link.to,
            link.edge.is_none(),
            link.edge,
            link.relation.is_none(),
            link.relation,
            link.orientation,
            link.derivation,
        )
    }
    left.len()
        .cmp(&right.len())
        .then_with(|| left.iter().map(key).cmp(right.iter().map(key)))
}

fn impact_root<'a>(
    model: &Model<'a>,
    graph: &ContextGraph<'a>,
    adjacency: &Adjacency<'a>,
    root: &'a str,
    affected: &mut BTreeMap<&'a str, BTreeSet<&'a str>>,
) -> ImpactRoot {
    let paths = consumer_paths(root, adjacency);
    let mut support = Paths::new();
    for (&consumer, prefix) in &paths {
        if paths.len() == model.nodes.len() {
            break;
        }
        let context = graph.scope([consumer]);
        for target in context.nodes().filter(|id| !paths.contains_key(id)) {
            let mut candidate = prefix.clone();
            candidate.extend(context.path_to(target).expect("member of context"));
            match support.entry(target) {
                Entry::Vacant(e) => {
                    e.insert(candidate);
                }
                Entry::Occupied(mut e) if witness_order(&candidate, e.get()).is_lt() => {
                    e.insert(candidate);
                }
                _ => {}
            }
        }
    }
    let mut items = BTreeMap::new();
    for (target, witness) in paths {
        let (category, reason) = if target == root {
            ("selected", "Selected change seed.")
        } else {
            (
                if witness.len() == 1 {
                    "direct"
                } else {
                    "transitive"
                },
                "Declared dependency consumer may need review.",
            )
        };
        items.insert(target, (category, reason, witness));
    }
    for (target, witness) in support {
        items.insert(
            target,
            (
                "supporting",
                "Established supporting read context; no change propagation inferred.",
                witness,
            ),
        );
    }
    ImpactRoot {
        id: root.into(),
        items: items
            .into_iter()
            .map(|(id, (category, reason, witness))| {
                affected.entry(id).or_default().insert(root);
                ImpactItem {
                    id: id.into(),
                    kind: string(model.nodes[id], "kind").into(),
                    category,
                    reason,
                    witness: witness.into_iter().map(Into::into).collect(),
                }
            })
            .collect(),
    }
}

/// Render the assessed result without loading a bundle or recomputing paths.
pub fn impact_text(report: &ImpactReport) -> String {
    let mut out = format!(
        "Impact: {} (exit {}; declared model; runtime callers unassessed)\nScope: {}\n",
        report.coverage.result,
        report.exit_code,
        report.scope.join(", ")
    );
    for root in &report.roots {
        writeln!(out, "Seed {}:", root.id).unwrap();
        for item in &root.items {
            write!(
                out,
                "  {}: {} — {} [{}",
                item.category, item.id, item.reason, root.id
            )
            .unwrap();
            for step in &item.witness {
                write!(out, " -> {}", step.to).unwrap();
            }
            out.push_str("]\n");
            for step in &item.witness {
                write!(
                    out,
                    "    {}: {}",
                    step.orientation,
                    step.edge.as_deref().unwrap_or(step.derivation)
                )
                .unwrap();
                if let Some(relation) = &step.relation {
                    write!(out, " ({relation})").unwrap();
                }
                out.push('\n');
            }
        }
    }
    if !report.coverage.unspecified_declarations.is_empty() {
        writeln!(
            out,
            "Dependency completeness unspecified: {}",
            report.coverage.unspecified_declarations.join(", ")
        )
        .unwrap();
    }
    writeln!(
        out,
        "Code bindings: {} (declared records only)",
        report.record_coverage.code_bindings
    )
    .unwrap();
    for record in &report.records {
        writeln!(
            out,
            "{}: {} ({})",
            record.action,
            record.id,
            record.targets.join(", ")
        )
        .unwrap();
    }
    for item in &report.records_omitted {
        writeln!(out, "Omitted record {}: {}", item.id, item.reason).unwrap();
    }
    for d in &report.diagnostics {
        writeln!(out, "{}: {}", d.code, d.message).unwrap();
    }
    out
}
