//! Semantic checks over the schema-validated model graph.

use std::collections::{BTreeMap, BTreeSet};

use jsonschema_value::numeric::bignum::{try_parse_bigfraction, try_parse_bigint, whole};
use serde_json::Number;

use crate::bundle::{Bundle, Document, DocumentKind};
use crate::input::Value;
use crate::report::{Check, CheckResult, Diagnostic};
use crate::validation::{CORE_KINDS, CORE_RELATIONS, Profile, ValidationOptions, present};

mod authorization;
pub mod boundaries;
pub(crate) mod canonical;
pub mod context;
pub mod design_review;
pub(crate) mod explain;
mod extensions;
pub mod impact;
mod implementation;
mod inspection;
pub mod model_diff;
mod planning;
mod readiness;
mod records;
pub mod workflow;

use extensions::Definition;

type Entries = BTreeMap<String, Value>;
type Graph<K> = BTreeMap<K, Vec<K>>;
type Outgoing<'a> = BTreeMap<&'a str, Graph<&'a str>>;

const TARGETS: &[&str] = &["component", "entity", "action", "screen", "machine"];
const OWNED: &[&str] = &["action", "entity", "screen", "machine"];
const ACTION: &[&str] = &["action"];
const COMPONENT: &[&str] = &["component"];
const ENTITY: &[&str] = &["entity"];

pub(crate) struct ReadinessContexts<'a> {
    structural: context::ContextScope<'a>,
    // An empty package scope includes global context. Ordinary structural
    // readiness with no scope starts from every implementation target.
    empty_package: Option<context::ContextScope<'a>>,
}

pub(crate) fn check<'a>(
    bundle: &'a Bundle,
    manifest: &'a Document,
    options: &ValidationOptions<'a>,
    scope: &[&'a str],
) -> (Vec<Check>, Option<ReadinessContexts<'a>>) {
    let mut model = semantics(bundle, manifest, options.unsupported_extensions);
    let wants_records = options.activity.is_some() || options.profiles.contains(&Profile::Records);
    if wants_records || bundle.records().len() != 0 {
        model.records();
    }
    let mut context = None;
    let wants_readiness = options.profiles.contains(&Profile::Readiness);
    let graph = (wants_readiness || (wants_records && options.activity.is_none()))
        .then(|| context::ContextGraph::new(bundle));
    if wants_readiness {
        let graph = graph.as_ref().expect("readiness graph");
        let (checks, assessed) = assess_readiness(&model, graph, options, scope);
        model.checks.extend(checks);
        context = assessed;
    }
    if wants_records {
        let host = present(options.host_context).unwrap_or(&Value::Null);
        if let Some(activity) = options.activity {
            model
                .checks
                .push(authorization::check(&model, activity, scope, host));
        } else {
            let checks = implementation::check(
                &model,
                graph.as_ref().expect("implementation graph"),
                scope,
                host,
            );
            model.checks.extend(checks);
        }
    }
    (model.checks, context)
}

fn assess_readiness<'a>(
    model: &Model<'a>,
    graph: &context::ContextGraph<'a>,
    options: &ValidationOptions<'a>,
    scope: &[&'a str],
) -> (Vec<Check>, Option<ReadinessContexts<'a>>) {
    if let Some(plan) = present(options.parallel_plan) {
        return (planning::parallel(model, graph, plan), None);
    }
    let (mut checks, structural) = readiness::check(model, graph, scope, options.host_context);
    let package = present(options.package);
    let empty_package = (package.is_some() && scope.is_empty()).then(|| graph.scope([]));
    if let Some(package) = package {
        let selected = empty_package.as_ref().unwrap_or(&structural);
        checks.insert(0, planning::package(model, package, scope, selected));
    }
    (
        checks,
        Some(ReadinessContexts {
            structural,
            empty_package,
        }),
    )
}

pub(crate) fn projection_checks(bundle: &Bundle, manifest: &Document) -> Vec<Check> {
    semantics(bundle, manifest, &[]).checks
}

fn semantics<'a>(
    bundle: &'a Bundle,
    manifest: &'a Document,
    unsupported_extensions: &'a [String],
) -> Model<'a> {
    let mut model = Model {
        bundle,
        manifest,
        nodes: bundle.nodes().map(|node| (node.id, node.value)).collect(),
        outgoing: BTreeMap::new(),
        parents: BTreeMap::new(),
        namespaces: entries(&manifest.value, "extensions")
            .map(|(_, declaration)| string(declaration, "namespace"))
            .collect(),
        custom_kinds: BTreeMap::new(),
        custom_relations: BTreeMap::new(),
        unsupported_extensions,
        checks: bundle
            .diagnostics()
            .iter()
            .filter(|d| d.semantic_only)
            .map(Check::from)
            .collect(),
    };
    model.extensions();
    model.edges();
    model.nodes();
    model.manifest_references();
    if manifest.kind == DocumentKind::Manifest03 {
        model.boundaries();
    }
    if model.checks.is_empty() {
        model.checks.push(Check::new(
            "model.semantics",
            "model",
            CheckResult::Pass,
            "Core references, endpoints, identity, refinement, machines, types, boundaries and declared extension data checked.",
        ));
    }
    model
}

struct Model<'a> {
    bundle: &'a Bundle,
    manifest: &'a Document,
    nodes: BTreeMap<&'a str, &'a Value>,
    outgoing: Outgoing<'a>,
    parents: Graph<&'a str>,
    namespaces: BTreeSet<&'a str>,
    custom_kinds: BTreeMap<&'a str, Definition<'a>>,
    custom_relations: BTreeMap<&'a str, Definition<'a>>,
    unsupported_extensions: &'a [String],
    checks: Vec<Check>,
}

impl<'a> Model<'a> {
    fn issue(&mut self, code: &'static str, message: impl Into<String>, id: &str, suffix: &str) {
        self.diagnostic(code, CheckResult::Fail, message, id, suffix, [id]);
    }

    fn unresolved(
        &mut self,
        code: &'static str,
        message: impl Into<String>,
        id: &str,
        suffix: &str,
    ) {
        self.diagnostic(code, CheckResult::Unknown, message, id, suffix, [id]);
    }

    fn diagnostic<'id>(
        &mut self,
        code: &'static str,
        result: CheckResult,
        message: impl Into<String>,
        id: &str,
        suffix: &str,
        related_ids: impl IntoIterator<Item = &'id str>,
    ) {
        let location = self.bundle.location(id);
        let (file, prefix) = location.map_or((self.manifest.asset.path.as_str(), ""), |location| {
            (location.file.as_str(), location.pointer.as_str())
        });
        let message = message.into();
        let diagnostic = Diagnostic::new(
            code,
            result,
            &message,
            Some(file),
            format!("{prefix}{suffix}"),
        )
        .with_related_ids(related_ids);
        self.push(diagnostic, result);
    }

    fn push(&mut self, diagnostic: Diagnostic, result: CheckResult) {
        let mut check = Check::new(
            format!("check.{}", diagnostic.code.to_ascii_lowercase()),
            "model",
            result,
            &diagnostic.message,
        );
        check.diagnostics.push(diagnostic);
        self.checks.push(check);
    }

    fn kind(&self, id: &str) -> Option<&'a str> {
        self.nodes.get(id).map(|value| string(value, "kind"))
    }

    fn reference(&mut self, target: &str, kinds: &[&str], id: &str, path: &str) {
        match self.kind(target) {
            None => self.issue(
                "REFERENCE_UNRESOLVED",
                format!("Unknown node {target}."),
                id,
                path,
            ),
            Some(kind) if !kinds.is_empty() && !kinds.contains(&kind) => {
                self.issue(
                    "ENDPOINT_KIND_INVALID",
                    format!(
                        "{target} must have one of these kinds: {}.",
                        kinds.join(", ")
                    ),
                    id,
                    path,
                );
            }
            _ => {}
        }
    }

    fn targets(&self, id: &str, relation: &str) -> &[&'a str] {
        self.outgoing
            .get(id)
            .and_then(|relations| relations.get(relation))
            .map_or(&[], Vec::as_slice)
    }

    fn owner(&self, id: &str) -> Option<&'a str> {
        boundaries::unique_owner(self.targets(id, "owned_by"))
    }

    fn edges(&mut self) {
        for item in self.bundle.model_edges() {
            let id = item.id;
            let from = string(item.value, "from");
            let to = string(item.value, "to");
            let relation = string(item.value, "relation");
            self.outgoing
                .entry(from)
                .or_default()
                .entry(relation)
                .or_default()
                .push(to);
            if relation == "refines" && self.nodes.contains_key(from) {
                self.parents.entry(from).or_default().push(to);
            }

            if let Some((from_kinds, to_kinds)) = endpoints(relation) {
                self.reference(from, from_kinds, id, "/from");
                self.reference(to, to_kinds, id, "/to");
                if matches!(relation, "refines" | "member_of")
                    && let Some(kind) = self.kind(from)
                    && (kind == "community"
                        || (!CORE_KINDS.contains(&kind) && !self.custom_kinds.contains_key(kind)))
                    && !self
                        .unsupported_extensions
                        .iter()
                        .any(|namespace| has_namespace(kind, namespace))
                {
                    self.issue(
                        "ENDPOINT_KIND_INVALID",
                        format!("{from} cannot be a {relation} source."),
                        id,
                        "/from",
                    );
                }
            } else {
                self.reference(from, &[], id, "/from");
                self.reference(to, &[], id, "/to");
                debug_assert!(!CORE_RELATIONS.contains(&relation));
                self.custom(relation, true, id, item.value);
            }

            for (index, reference) in items(item.value, "source_refs").iter().enumerate() {
                self.source_reference(reference, id, &format!("/source_refs/{index}"));
            }
        }
        if has_cycle(&self.parents) {
            self.issue(
                "REFINEMENT_CYCLE",
                "Refines contains a cycle or self-loop.",
                "",
                "",
            );
        }
    }

    fn nodes(&mut self) {
        let mut transition_ids = BTreeSet::new();
        for item in self.bundle.nodes() {
            let id = item.id;
            let node = item.value;
            let kind = string(node, "kind");
            if !CORE_KINDS.contains(&kind) {
                self.custom(kind, false, id, node);
            }
            for (relation, code, message) in [
                (
                    "owned_by",
                    "OWNERSHIP_AMBIGUOUS",
                    "At most one ownership edge is allowed.",
                ),
                (
                    "refines",
                    "REFINEMENT_PARENT_AMBIGUOUS",
                    "At most one refinement parent is allowed.",
                ),
                (
                    "state_of",
                    "OWNERSHIP_AMBIGUOUS",
                    "Machine state_of is ambiguous.",
                ),
            ] {
                if self.targets(id, relation).len() > 1 {
                    self.issue(code, message, id, "");
                }
            }
            for (index, mapping) in items(node, "realization").iter().enumerate() {
                for (child_index, child) in strings(mapping, "children").enumerate() {
                    let path = format!("/realization/{index}/children/{child_index}");
                    self.reference(child, &[], id, &path);
                    if child == id || !reachable(&self.parents, child, id) {
                        self.issue(
                            "REALIZATION_INVALID",
                            format!("{child} is not a descendant of {id}."),
                            id,
                            &path,
                        );
                    }
                }
            }
            for (index, target) in strings(node, "interfaces").enumerate() {
                self.reference(target, ACTION, id, &format!("/interfaces/{index}"));
            }
            for (name, spec) in entries(node, "fields") {
                self.types(spec, id, &format!("/fields/{}", escape(name)), true, true);
            }
            if let Some(contract) = get(node, "contract") {
                self.contract(contract, id);
            }
            if let Some(rule) = get(node, "rule") {
                self.rule(rule, id);
            }
            if let Some(machine) = get(node, "machine") {
                self.machine(machine, id, &mut transition_ids);
            }
            for field in ["source_refs", "design_refs"] {
                for (index, reference) in items(node, field).iter().enumerate() {
                    let path = format!("/{field}/{index}");
                    if field == "design_refs" && string(reference, "purpose") != "contract" {
                        self.issue(
                            "CONSTRAINT_INVALID",
                            "Design references must have purpose=contract.",
                            id,
                            &path,
                        );
                    }
                    self.source_reference(reference, id, &path);
                }
            }
        }
    }

    fn contract(&mut self, contract: &Value, id: &str) {
        for field in ["input", "context"] {
            for (name, spec) in entries(contract, field) {
                self.types(
                    spec,
                    id,
                    &format!("/contract/{field}/{}", escape(name)),
                    true,
                    true,
                );
            }
        }
        if let Some(output) = get(contract, "output") {
            self.types(output, id, "/contract/output", false, true);
        }
        for (name, risk) in entries(contract, "risks") {
            for (index, target) in strings(risk, "scenarios").enumerate() {
                self.reference(
                    target,
                    &["scenario"],
                    id,
                    &format!("/contract/risks/{}/scenarios/{index}", escape(name)),
                );
            }
        }
        for (name, scenarios) in entries(contract, "error_cases") {
            let path = format!("/contract/error_cases/{}", escape(name));
            if get(contract, "errors")
                .and_then(|errors| get(errors, name))
                .is_none()
            {
                self.issue(
                    "CONSTRAINT_INVALID",
                    format!("Undeclared error outcome {name}."),
                    id,
                    &path,
                );
            }
            if let Value::Array(scenarios) = scenarios {
                for (index, target) in scenarios.iter().filter_map(text).enumerate() {
                    self.reference(target, &["scenario"], id, &format!("{path}/{index}"));
                }
            }
        }
    }

    fn types(&mut self, spec: &Value, id: &str, path: &str, field: bool, required: bool) {
        if get(spec, "optional").is_some() && !field {
            self.issue(
                "TYPE_CONSTRAINT_INVALID",
                "optional is only legal in input/context/entity fieldmaps.",
                id,
                path,
            );
        }
        let required = required && !flag(spec, "nullable") && !(field && flag(spec, "optional"));
        let kind = string(spec, "type");
        if let (Some(Value::Number(minimum)), Some(Value::Number(maximum))) =
            (get(spec, "minimum"), get(spec, "maximum"))
        {
            match interval_issue(minimum, maximum, kind == "integer" && required) {
                Ok(Some(message)) => self.issue("TYPE_CONSTRAINT_INVALID", message, id, path),
                Err(()) => self.unresolved(
                    "FORMAT_UNSUPPORTED",
                    "Numeric interval exceeds exact comparison capacity.",
                    id,
                    path,
                ),
                Ok(None) => {}
            }
        }
        match kind {
            "reference" => self.reference(
                string(spec, "target"),
                ENTITY,
                id,
                &format!("{path}/target"),
            ),
            "object" => {
                let properties = get(spec, "properties");
                let required_names: BTreeSet<_> = strings(spec, "required").collect();
                if required_names
                    .iter()
                    .any(|name| properties.and_then(|props| get(props, name)).is_none())
                {
                    self.issue(
                        "TYPE_CONSTRAINT_INVALID",
                        "Required names must exist in properties.",
                        id,
                        path,
                    );
                }
                for (name, child) in entries(spec, "properties") {
                    self.types(
                        child,
                        id,
                        &format!("{path}/properties/{}", escape(name)),
                        false,
                        required && required_names.contains(name.as_str()),
                    );
                }
            }
            "array" => {
                if let Some(child) = get(spec, "items") {
                    // An empty array is valid, regardless of the item's value domain.
                    self.types(child, id, &format!("{path}/items"), false, false);
                }
            }
            _ => {}
        }
    }

    fn rule(&mut self, rule: &Value, id: &str) {
        if get(rule, "scope").and_then(text) == Some("targeted")
            && self.targets(id, "constrains").is_empty()
        {
            self.issue(
                "CONSTRAINT_INVALID",
                "Targeted rule has no constrains target.",
                id,
                "",
            );
        }
        let Some(constraint) = get(rule, "constraint") else {
            return;
        };
        let architecture = string(rule, "category") == "architecture";
        let imports = string(constraint, "relation") == "imports";
        if !architecture {
            self.issue(
                "CONSTRAINT_INVALID",
                "Core constraints require architecture category.",
                id,
                "/rule/category",
            );
        }
        if !imports {
            self.issue(
                "CONSTRAINT_INVALID",
                "Only imports constraints are supported by this core profile.",
                id,
                "/rule/constraint/relation",
            );
        }
        let from = string(constraint, "from");
        let to = string(constraint, "to");
        self.reference(from, COMPONENT, id, "/rule/constraint/from");
        self.reference(to, COMPONENT, id, "/rule/constraint/to");
        let reach = string(constraint, "reach");
        if architecture
            && imports
            && string(constraint, "policy") == "forbid"
            && self.kind(from) == Some("component")
            && self.kind(to) == Some("component")
            && self.imports_path(from, to, reach)
        {
            self.issue(
                "CONSTRAINT_INVALID",
                format!("Declared imports have a forbidden {reach} path from {from} to {to}."),
                id,
                "/rule/constraint",
            );
        }
    }

    fn imports_path(&self, from: &str, to: &str, reach: &str) -> bool {
        let first = self.targets(from, "imports");
        if reach == "direct" {
            return first.contains(&to);
        }
        // Begin with an edge so a self-forbid detects actual cycles only.
        let mut pending = first.to_vec();
        let mut visited = BTreeSet::new();
        while let Some(current) = pending.pop() {
            if current == to {
                return true;
            }
            if self.kind(current) == Some("component") && visited.insert(current) {
                pending.extend(self.targets(current, "imports"));
            }
        }
        false
    }

    fn machine(&mut self, machine: &'a Value, id: &str, transition_ids: &mut BTreeSet<&'a str>) {
        let states: BTreeSet<_> = strings(machine, "states").collect();
        let terminal: BTreeSet<_> = strings(machine, "terminal").collect();
        for (path, state) in
            std::iter::once(("/machine/initial".into(), string(machine, "initial"))).chain(
                strings(machine, "terminal")
                    .enumerate()
                    .map(|(i, state)| (format!("/machine/terminal/{i}"), state)),
            )
        {
            if !states.contains(state) {
                self.issue(
                    "STATE_UNDECLARED",
                    format!("Undeclared state {state}."),
                    id,
                    &path,
                );
            }
        }
        let mut transitions = BTreeSet::new();
        for (index, transition) in items(machine, "transitions").iter().enumerate() {
            let path = format!("/machine/transitions/{index}");
            let transition_id = string(transition, "id");
            if self.bundle.location(transition_id).is_some()
                || !transition_ids.insert(transition_id)
            {
                self.diagnostic(
                    "ID_COLLISION",
                    CheckResult::Fail,
                    format!("ID occurs more than once: {transition_id}."),
                    id,
                    &format!("{path}/id"),
                    [transition_id],
                );
            }
            let from = string(transition, "from");
            let to = string(transition, "to");
            let action = string(transition, "trigger");
            for (side, state) in [("from", from), ("to", to)] {
                if !states.contains(state) {
                    self.issue(
                        "STATE_UNDECLARED",
                        format!("Undeclared state {state}."),
                        id,
                        &format!("{path}/{side}"),
                    );
                }
            }
            if !transitions.insert((from, action)) {
                self.issue(
                    "TRANSITION_AMBIGUOUS",
                    format!("Repeated transition from {from} with trigger {action}."),
                    id,
                    &path,
                );
            }
            if terminal.contains(from) {
                self.issue(
                    "TERMINAL_MUTATION",
                    "Terminal state has outgoing mutation.",
                    id,
                    &path,
                );
            }
            self.reference(action, ACTION, id, &format!("{path}/trigger"));
        }
    }

    fn source_reference(&mut self, reference: &Value, id: &str, path: &str) {
        let target = string(reference, "source");
        let source = get(&self.manifest.value, "sources").and_then(|sources| get(sources, target));
        if source.is_none() {
            self.issue(
                "REFERENCE_UNRESOLVED",
                format!("Unknown source {target}."),
                id,
                path,
            );
        }
        if let Some(selector) = get(reference, "selector") {
            let selector_value = get(selector, "value").and_then(text);
            match string(selector, "kind") {
                "json-pointer" if !selector_value.is_some_and(is_pointer) => {
                    self.issue("CONSTRAINT_INVALID", "Invalid JSON Pointer.", id, path);
                }
                "line-range"
                    if !selector_value.is_some_and(is_line_range)
                        || source
                            .and_then(|entry| {
                                get(entry, "revision").and_then(|revision| get(revision, "sha256"))
                            })
                            .is_none() =>
                {
                    self.issue(
                        "CONSTRAINT_INVALID",
                        "Line range must be valid and source hash pinned.",
                        id,
                        path,
                    );
                }
                _ => {}
            }
        }
    }

    fn manifest_references(&mut self) {
        let manifest = &self.manifest.value;
        for (id, question) in entries(manifest, "questions") {
            for (index, target) in strings(question, "targets").enumerate() {
                self.reference(target, &[], id, &format!("/targets/{index}"));
            }
        }
        if let Some(workflow) =
            get(manifest, "project").and_then(|project| get(project, "workflow"))
        {
            for (name, gate) in entries(workflow, "gates") {
                for (index, target) in strings(gate, "scope").enumerate() {
                    self.reference(
                        target,
                        &[],
                        "",
                        &format!("/project/workflow/gates/{}/scope/{index}", escape(name)),
                    );
                }
            }
        }
        let mut lineage = Graph::new();
        let sources = get(manifest, "sources");
        for (id, source) in entries(manifest, "sources") {
            let identity = (id.as_str(), revision(get(source, "revision")));
            let parents = lineage.entry(identity).or_default();
            for (index, parent) in items(source, "derived_from").iter().enumerate() {
                let path = format!("/derived_from/{index}");
                let target = string(parent, "source");
                let Some(current) = sources.and_then(|sources| get(sources, target)) else {
                    self.issue(
                        "REFERENCE_UNRESOLVED",
                        format!("Unknown derivation source {target}."),
                        id,
                        &path,
                    );
                    continue;
                };
                let parent_revision = get(parent, "revision");
                if parent_revision.is_none() || parent_revision != get(current, "revision") {
                    self.unresolved(
                        "REVISION_UNRESOLVED",
                        "Historical derivation revision is unavailable.",
                        id,
                        &path,
                    );
                }
                if parent_revision.is_some() {
                    parents.push((target, revision(parent_revision)));
                }
            }
        }
        if has_cycle(&lineage) {
            self.issue(
                "SOURCE_DERIVATION_CYCLE",
                "Source and revision derivation contains a cycle.",
                "",
                "",
            );
        }
    }

    fn boundaries(&mut self) {
        for item in self.bundle.nodes() {
            let id = item.id;
            let node = item.value;
            if string(node, "kind") == "component" {
                for (index, action) in strings(node, "interfaces").enumerate() {
                    if let Some(owner) = self.owner(action)
                        && owner != id
                    {
                        self.diagnostic(
                            "INTERFACE_OWNER_MISMATCH",
                            CheckResult::Fail,
                            format!("Public action {action} belongs to {owner}, not {id}."),
                            id,
                            &format!("/interfaces/{index}"),
                            [id, action, owner],
                        );
                    }
                }
            }
            if let Some(rule) = get(node, "rule")
                && get(rule, "principle").is_some()
                && (string(rule, "category") != "design" || flag(rule, "blocking"))
            {
                self.issue(
                    "DESIGN_PRINCIPLE_INVALID",
                    "Design principles require a non-blocking design rule.",
                    id,
                    "/rule/principle",
                );
            }
        }
        for item in self
            .bundle
            .model_edges()
            .filter(|item| string(item.value, "relation") == "calls")
        {
            let call =
                boundaries::resolve_call(item.id, item.value, &self.nodes, |id| self.owner(id));
            if let (Some(consumer), Some(provider), boundaries::CallStatus::Private) =
                (call.consumer, call.provider, call.status)
            {
                let from = call.source;
                let action = call.action;
                self.diagnostic(
                    "PRIVATE_ACTION_CALL",
                    CheckResult::Fail,
                    format!("Cross-component call must use {provider}'s public interface."),
                    item.id,
                    "",
                    [from, action, consumer, provider],
                );
            }
        }
    }
}

fn endpoints(relation: &str) -> Option<(&'static [&'static str], &'static [&'static str])> {
    Some(match relation {
        "owned_by" => (OWNED, COMPONENT),
        "stored_in" => (ENTITY, COMPONENT),
        "imports" => (COMPONENT, COMPONENT),
        "calls" => (&["component", "action", "screen"], ACTION),
        "reads" | "creates" | "changes" => (ACTION, ENTITY),
        "constrains" => (&["rule"], TARGETS),
        "exercises" => (&["scenario"], &["action", "screen", "machine"]),
        "verifies" => (&["scenario"], &["rule"]),
        "state_of" => (&["machine"], ENTITY),
        "navigates_to" => (&["screen"], &["screen"]),
        "depends_on" => (TARGETS, TARGETS),
        // Source admission also allows known extension kinds.
        "refines" => (&[], &["component", "entity", "action", "screen"]),
        "member_of" => (&[], &["community"]),
        _ => return None,
    })
}

fn get<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    match value {
        Value::Object(values) => values.get(key),
        _ => None,
    }
}

fn text(value: &Value) -> Option<&str> {
    match value {
        Value::String(text) => Some(text),
        _ => None,
    }
}

fn string<'a>(value: &'a Value, key: &str) -> &'a str {
    get(value, key)
        .and_then(text)
        .expect("schema-validated string")
}

fn blank_text(value: &str) -> bool {
    value
        .chars()
        .all(|ch| ch.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&ch))
}

fn flag(value: &Value, key: &str) -> bool {
    matches!(get(value, key), Some(Value::Boolean(true)))
}

pub(crate) fn items<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    match get(value, key) {
        Some(Value::Array(values)) => values,
        _ => &[],
    }
}

pub(crate) fn strings<'a>(value: &'a Value, key: &str) -> impl Iterator<Item = &'a str> + use<'a> {
    items(value, key).iter().filter_map(text)
}

fn entries<'a>(
    value: &'a Value,
    key: &str,
) -> impl Iterator<Item = (&'a String, &'a Value)> + use<'a> {
    let entries: Option<&Entries> = match get(value, key) {
        Some(Value::Object(values)) => Some(values),
        _ => None,
    };
    entries.into_iter().flatten()
}

fn has_namespace(name: &str, namespace: &str) -> bool {
    name.strip_prefix(namespace)
        .is_some_and(|suffix| suffix.starts_with('.'))
}

fn escape(segment: &str) -> String {
    segment.replace('~', "~0").replace('/', "~1")
}

fn is_pointer(value: &str) -> bool {
    (value.is_empty() || value.starts_with('/'))
        && value
            .split('~')
            .skip(1)
            .all(|part| part.starts_with(['0', '1']))
}

fn is_line_range(value: &str) -> bool {
    let Some((start, end)) = value
        .strip_prefix('L')
        .and_then(|range| range.split_once("-L"))
    else {
        return false;
    };
    let valid = |line: &str| {
        matches!(line.as_bytes().first(), Some(b'1'..=b'9'))
            && line.bytes().all(|b| b.is_ascii_digit())
    };
    valid(start) && valid(end) && (start.len(), start) <= (end.len(), end)
}

fn revision(value: Option<&Value>) -> (Option<&str>, Option<&str>) {
    (
        value.and_then(|value| get(value, "sha256")).and_then(text),
        value
            .and_then(|value| get(value, "producer_revision"))
            .and_then(text),
    )
}

fn has_cycle<K: Copy + Ord>(graph: &Graph<K>) -> bool {
    let mut indegree: BTreeMap<_, usize> = graph.keys().map(|key| (*key, 0)).collect();
    for children in graph.values() {
        for child in children {
            *indegree.entry(*child).or_default() += 1;
        }
    }
    let mut pending: Vec<_> = indegree
        .iter()
        .filter_map(|(node, degree)| (*degree == 0).then_some(*node))
        .collect();
    let mut count = 0;
    while let Some(node) = pending.pop() {
        count += 1;
        if let Some(children) = graph.get(&node) {
            for child in children {
                let degree = indegree.get_mut(child).expect("indexed graph edge");
                *degree -= 1;
                if *degree == 0 {
                    pending.push(*child);
                }
            }
        }
    }
    count != indegree.len()
}

fn reachable<K: Copy + Ord>(graph: &Graph<K>, from: K, to: K) -> bool {
    let mut pending = graph.get(&from).cloned().unwrap_or_default();
    let mut visited = BTreeSet::new();
    while let Some(current) = pending.pop() {
        if current == to {
            return true;
        }
        if visited.insert(current)
            && let Some(children) = graph.get(&current)
        {
            pending.extend(children);
        }
    }
    false
}

fn interval_issue(
    min: &str,
    max: &str,
    integer_required: bool,
) -> Result<Option<&'static str>, ()> {
    let exact = |raw: &str| {
        let value = raw.parse::<Number>().ok()?;
        if let Some(integer) = value.as_i64() {
            Some(whole(integer.into()))
        } else if let Some(integer) = try_parse_bigint(&value) {
            Some(whole(integer))
        } else {
            try_parse_bigfraction(&value)
        }
    };
    let minimum = exact(min).ok_or(())?;
    let maximum = exact(max).ok_or(())?;
    if minimum > maximum {
        Ok(Some("minimum exceeds maximum."))
    } else if integer_required && minimum.ceil() > maximum.floor() {
        Ok(Some(
            "Required non-null integer interval contains no integer.",
        ))
    } else {
        Ok(None)
    }
}
