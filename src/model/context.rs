//! Scoped read context and implementation targets for a loaded model bundle.

use std::collections::{BTreeMap, BTreeSet, btree_map::Entry};
use std::io;

use serde::Serialize;

use super::{entries, flag, get, items, strings, text};
use crate::bundle::{Bundle, DocumentKind};
use crate::input::Value;
use crate::validation::CORE_RELATIONS;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct ContextLink<'a> {
    pub from: Option<&'a str>,
    pub to: &'a str,
    pub edge: Option<&'a str>,
    pub relation: Option<&'a str>,
    pub orientation: &'static str,
    pub derivation: &'static str,
}

impl<'a> ContextLink<'a> {
    fn semantic(from: Option<&'a str>, to: &'a str, derivation: &'static str) -> Self {
        Self {
            from,
            to,
            edge: None,
            relation: None,
            orientation: "semantic",
            derivation,
        }
    }
}

fn field<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    get(value, key).and_then(text)
}

fn type_references<'a>(value: &'a Value, targets: &mut BTreeSet<&'a str>) {
    match field(value, "type") {
        Some("reference") => targets.extend(field(value, "target")),
        Some("object") => {
            for (_, child) in entries(value, "properties") {
                type_references(child, targets);
            }
        }
        Some("array") => {
            if let Some(child) = get(value, "items") {
                type_references(child, targets);
            }
        }
        _ => {}
    }
}

/// Index the declared context arcs once for repeated queries over the same bundle.
pub struct ContextGraph<'a> {
    outgoing: BTreeMap<&'a str, Vec<ContextLink<'a>>>,
    refinements: BTreeMap<&'a str, Vec<ContextLink<'a>>>,
    global_rules: Vec<&'a str>,
    support: BTreeSet<&'a str>,
}

impl<'a> ContextGraph<'a> {
    pub fn new(bundle: &'a Bundle) -> Self {
        let mut graph = Self {
            outgoing: BTreeMap::new(),
            refinements: BTreeMap::new(),
            global_rules: Vec::new(),
            support: BTreeSet::new(),
        };
        for node in bundle.nodes() {
            graph.node_links(node.id, node.value);
        }
        let definitions: BTreeMap<_, _> = bundle
            .extensions()
            .flat_map(|(_, doc)| entries(&doc.value, "relations"))
            .map(|(id, definition)| (id.as_str(), definition))
            .collect();
        let version03 = bundle
            .manifest()
            .is_some_and(|doc| doc.kind == DocumentKind::Manifest03);
        for edge in bundle.edges() {
            let (Some(from), Some(to), Some(relation)) = (
                field(edge.value, "from"),
                field(edge.value, "to"),
                field(edge.value, "relation"),
            ) else {
                continue;
            };
            let definition = definitions.get(relation);
            let direction = definition.and_then(|def| field(def, "context"));
            let core_outgoing =
                CORE_RELATIONS.contains(&relation) && (relation != "member_of" || version03);
            let forward = ContextLink {
                from: Some(from),
                to,
                edge: Some(edge.id),
                relation: Some(relation),
                orientation: "forward",
                derivation: "declared-context",
            };
            let reverse = ContextLink {
                from: Some(to),
                to: from,
                orientation: "reverse",
                ..forward
            };
            if core_outgoing
                || matches!(direction, Some("outgoing" | "both"))
                || definition.is_some_and(|def| {
                    flag(def, "dependency")
                        || field(def, "effect").is_some_and(|effect| effect != "none")
                })
            {
                graph.outgoing.entry(from).or_default().push(forward);
            }
            if matches!(
                relation,
                "constrains" | "exercises" | "verifies" | "state_of"
            ) || matches!(direction, Some("incoming" | "both"))
            {
                graph.outgoing.entry(to).or_default().push(reverse);
            }
            if relation == "refines" {
                graph.refinements.entry(to).or_default().push(ContextLink {
                    derivation: "initial-refinement",
                    ..reverse
                });
            }
        }
        for links in graph.outgoing.values_mut() {
            links.sort_by_key(|link| (link.to, link.edge, link.derivation));
        }
        for links in graph.refinements.values_mut() {
            links.sort_by_key(|link| link.edge);
        }
        graph
    }

    fn node_links(&mut self, id: &'a str, node: &'a Value) {
        if matches!(field(node, "kind"), Some("rule" | "scenario" | "community")) {
            self.support.insert(id);
        }
        let links = self.outgoing.entry(id).or_default();
        let mut add = |to, derivation| links.push(ContextLink::semantic(Some(id), to, derivation));
        for target in strings(node, "interfaces") {
            add(target, "public-interface");
        }
        let contract = get(node, "contract").unwrap_or(&Value::Null);
        let specs = entries(node, "fields")
            .chain(entries(contract, "input"))
            .chain(entries(contract, "context"))
            .map(|(_, spec)| spec)
            .chain(get(contract, "output"));
        for spec in specs {
            let mut targets = BTreeSet::new();
            type_references(spec, &mut targets);
            for target in targets {
                add(target, "type-reference");
            }
        }
        if let Some(machine) = get(node, "machine") {
            for trigger in items(machine, "transitions")
                .iter()
                .filter_map(|t| field(t, "trigger"))
            {
                add(trigger, "machine-trigger");
            }
        }
        for (_, risk) in entries(contract, "risks") {
            for target in strings(risk, "scenarios") {
                add(target, "contract-risk-scenario");
            }
        }
        for (_, cases) in entries(contract, "error_cases") {
            if let Value::Array(cases) = cases {
                for target in cases.iter().filter_map(text) {
                    add(target, "contract-error-scenario");
                }
            }
        }
        if let Some(rule) = get(node, "rule") {
            if field(rule, "scope") == Some("global") {
                self.global_rules.push(id);
            }
            if let Some(constraint) = get(rule, "constraint") {
                for key in ["from", "to"] {
                    if let Some(target) = field(constraint, key) {
                        add(target, "rule-constraint-endpoint");
                    }
                }
            }
        }
    }

    /// Declared arcs in the same order used to resolve witness ties.
    pub fn links(&self) -> impl Iterator<Item = &ContextLink<'a>> {
        self.outgoing.values().flatten()
    }

    pub fn scope<'s>(&self, seeds: impl IntoIterator<Item = &'s str>) -> ContextScope<'s>
    where
        'a: 's,
    {
        let seeds: BTreeSet<_> = seeds.into_iter().collect();
        let mut predecessors: BTreeMap<_, _> = seeds.iter().map(|&id| (id, None)).collect();
        let mut order: Vec<_> = seeds.iter().copied().collect();
        // Preserve discovery order when the initial refinement scope becomes the
        // starting queue for ordinary read context. A read dependency's children
        // do not enter the scope solely because that dependency was reached.
        expand(&self.refinements, &mut order, &mut predecessors);
        let implementation_targets = order
            .iter()
            .copied()
            .filter(|id| seeds.contains(id) || !self.support.contains(id))
            .collect();
        for &rule in &self.global_rules {
            if let Entry::Vacant(entry) = predecessors.entry(rule) {
                entry.insert(Some(ContextLink::semantic(
                    seeds.first().copied(),
                    rule,
                    "global-rule-context",
                )));
                order.push(rule);
            }
        }
        expand(&self.outgoing, &mut order, &mut predecessors);
        ContextScope {
            predecessors,
            implementation_targets,
        }
    }
}

fn expand<'a>(
    adjacency: &BTreeMap<&str, Vec<ContextLink<'a>>>,
    order: &mut Vec<&'a str>,
    predecessors: &mut BTreeMap<&'a str, Option<ContextLink<'a>>>,
) {
    let mut cursor = 0;
    while cursor < order.len() {
        if let Some(links) = adjacency.get(order[cursor]) {
            for &link in links {
                if let Entry::Vacant(entry) = predecessors.entry(link.to) {
                    entry.insert(Some(link));
                    order.push(link.to);
                }
            }
        }
        cursor += 1;
    }
}

/// One predecessor per discovered member. Reconstruct full paths only on request.
pub struct ContextScope<'a> {
    predecessors: BTreeMap<&'a str, Option<ContextLink<'a>>>,
    pub implementation_targets: BTreeSet<&'a str>,
}

impl<'a> ContextScope<'a> {
    pub fn contains(&self, id: &str) -> bool {
        self.predecessors.contains_key(id)
    }

    pub fn nodes(&self) -> impl ExactSizeIterator<Item = &'a str> + '_ {
        self.predecessors.keys().copied()
    }

    pub fn path_to(&self, id: &str) -> Option<Vec<ContextLink<'a>>> {
        let mut path = Vec::new();
        let mut predecessor = self.predecessors.get(id)?;
        while let Some(link) = predecessor {
            path.push(*link);
            let Some(parent) = link.from else { break };
            predecessor = &self.predecessors[parent];
        }
        path.reverse();
        Some(path)
    }
}

#[derive(Debug, Serialize)]
pub struct ContextManifest<'a> {
    pub format: &'static str,
    pub spec_revision: Option<&'a str>,
    pub scope: BTreeSet<&'a str>,
    pub nodes: Vec<&'a str>,
    pub edges: BTreeSet<&'a str>,
    pub sources: BTreeSet<&'a str>,
    pub questions: Vec<&'a str>,
    pub extensions: Vec<&'a str>,
}

/// Project the fixed-point read context of a model-validated bundle.
pub fn context_manifest<'a>(
    bundle: &'a Bundle,
    scope: &'a [String],
) -> io::Result<ContextManifest<'a>> {
    let Some(manifest) = bundle.manifest() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Context requires a model manifest.",
        ));
    };
    let scope: BTreeSet<_> = scope.iter().map(String::as_str).collect();
    if bundle
        .nodes()
        .filter(|node| scope.contains(node.id))
        .count()
        != scope.len()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Unknown context scope.",
        ));
    }
    let graph = ContextGraph::new(bundle);
    let context = graph.scope(scope.iter().copied());
    let mut sources = BTreeSet::new();
    for node in bundle.nodes().filter(|node| context.contains(node.id)) {
        for key in ["source_refs", "design_refs"] {
            sources.extend(
                items(node.value, key)
                    .iter()
                    .filter_map(|r| field(r, "source")),
            );
        }
    }
    let mut edges = BTreeSet::new();
    for edge in bundle.edges() {
        if ["from", "to"]
            .iter()
            .all(|key| field(edge.value, key).is_some_and(|id| context.contains(id)))
        {
            edges.insert(edge.id);
            sources.extend(
                items(edge.value, "source_refs")
                    .iter()
                    .filter_map(|r| field(r, "source")),
            );
        }
    }
    let source_defs = get(&manifest.value, "sources").unwrap_or(&Value::Null);
    let mut pending: Vec<_> = sources.iter().copied().collect();
    while let Some(id) = pending.pop() {
        if let Some(source) = get(source_defs, id) {
            for ancestor in items(source, "derived_from")
                .iter()
                .filter_map(|r| field(r, "source"))
            {
                if sources.insert(ancestor) {
                    pending.push(ancestor);
                }
            }
        }
    }
    Ok(ContextManifest {
        format: "gdd-context/0.2",
        spec_revision: bundle.snapshot().map(|s| s.revision.as_str()),
        scope,
        nodes: context.nodes().collect(),
        edges,
        sources,
        questions: entries(&manifest.value, "questions")
            .filter(|(_, q)| strings(q, "targets").any(|id| context.contains(id)))
            .map(|(id, _)| id.as_str())
            .collect(),
        extensions: entries(&manifest.value, "extensions")
            .map(|(id, _)| id.as_str())
            .collect(),
    })
}
