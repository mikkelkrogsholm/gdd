//! Ownership, public interfaces and declared community membership in a GDD 0.3 model.
use super::{Value, blank_text, get, strings, text};
use crate::bundle::{Bundle, DocumentKind};
use crate::validation::to_json;
use serde::Serialize;
use serde_json::{Value as Json, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    io,
};

pub const PRINCIPLES: [(&str, &str); 4] = [
    (
        "kiss",
        "Do the layers and abstractions serve a current requirement?",
    ),
    (
        "dry",
        "Is the same authoritative business knowledge implemented in multiple places?",
    ),
    (
        "separation_of_concerns",
        "Are decisions and state changes with the responsible component?",
    ),
    (
        "modularity",
        "Can components change through public contracts without knowledge of other internals?",
    ),
];

type Nodes<'a> = BTreeMap<&'a str, &'a Value>;
fn field<'a>(value: &'a Value, name: &str) -> Option<&'a str> {
    get(value, name).and_then(text)
}

pub(super) fn unique_owner<'a>(candidates: &[&'a str]) -> Option<&'a str> {
    if let [owner] = candidates {
        Some(*owner)
    } else {
        None
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum CallStatus {
    Internal,
    Public,
    Private,
    Unresolved,
}
#[derive(Serialize)]
pub(super) struct Call<'a> {
    edge: &'a str,
    pub(super) source: &'a str,
    pub(super) consumer: Option<&'a str>,
    pub(super) provider: Option<&'a str>,
    pub(super) action: &'a str,
    pub(super) status: CallStatus,
}

pub(super) fn resolve_call<'a>(
    edge: &'a str,
    value: &'a Value,
    nodes: &Nodes<'a>,
    owner: impl Fn(&str) -> Option<&'a str>,
) -> Call<'a> {
    let source = field(value, "from").unwrap_or_default();
    let action = field(value, "to").unwrap_or_default();
    let consumer = if nodes.get(source).and_then(|n| field(n, "kind")) == Some("component") {
        Some(source)
    } else {
        owner(source)
    };
    let provider = owner(action);
    let status = match (consumer, provider) {
        (Some(c), Some(p)) if c == p => CallStatus::Internal,
        (Some(_), Some(p)) if nodes.get(p).is_some_and(|n| get(n, "interfaces").is_some()) => {
            if strings(nodes[p], "interfaces").any(|a| a == action) {
                CallStatus::Public
            } else {
                CallStatus::Private
            }
        }
        _ => CallStatus::Unresolved,
    };
    Call {
        edge,
        source,
        consumer,
        provider,
        action,
        status,
    }
}

/// Derive the boundary view from captured nodes and edges. An omitted scope
/// selects every node. An explicitly empty scope selects none.
pub fn boundary_view(bundle: &Bundle, scope: Option<&[String]>) -> io::Result<Json> {
    let manifest = bundle
        .manifest()
        .filter(|m| m.kind == DocumentKind::Manifest03)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "Boundary views require an explicit gdd/0.3 model.",
            )
        })?;
    let nodes: Nodes<'_> = bundle.nodes().map(|n| (n.id, n.value)).collect();
    let selected: BTreeSet<_> = scope.map_or_else(
        || nodes.keys().copied().collect(),
        |s| s.iter().map(String::as_str).collect(),
    );
    if selected.iter().any(|id| !nodes.contains_key(id)) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Unknown boundary view scope.",
        ));
    }
    let mut ownership: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    let mut membership: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    let mut call_edges = Vec::new();
    for edge in bundle.model_edges() {
        let (Some(from), Some(to)) = (field(edge.value, "from"), field(edge.value, "to")) else {
            continue;
        };
        match field(edge.value, "relation") {
            Some("owned_by") => ownership.entry(from).or_default().push(to),
            Some("member_of") => membership.entry(to).or_default().push(from),
            Some("calls") => call_edges.push(edge),
            _ => {}
        }
    }
    let owner = |id: &str| ownership.get(id).and_then(|v| unique_owner(v));
    let kind = |id: &str| nodes.get(id).and_then(|n| field(n, "kind"));
    let components: BTreeSet<_> = selected
        .iter()
        .filter(|id| kind(id) == Some("component"))
        .copied()
        .chain(selected.iter().filter_map(|id| owner(id)))
        .filter(|id| nodes.contains_key(id))
        .collect();
    call_edges.sort_by_key(|e| e.id);
    let (calls, omitted): (Vec<_>, Vec<_>) = call_edges
        .into_iter()
        .map(|e| resolve_call(e.id, e.value, &nodes, owner))
        .partition(|c| selected.contains(c.source) && selected.contains(c.action));
    let revision = bundle.snapshot().map(|s| s.revision.as_str());
    let project = get(&manifest.value, "project").and_then(|p| field(p, "id"));
    let contracts = |provider: &str| {
        strings(nodes[provider],"interfaces").map(|action|
        json!({"project_id":project,"provider":provider,"action":action,"spec_revision":revision})).collect::<Vec<_>>()
    };
    let mut unresolved: BTreeSet<_> = calls
        .iter()
        .filter(|c| c.status == CallStatus::Unresolved)
        .map(|c| c.edge)
        .collect();
    let mut component_views = BTreeMap::new();
    for &id in &components {
        let node = nodes[id];
        let boundary = get(node, "boundary");
        let interfaces_known = get(node, "interfaces").is_some();
        if !interfaces_known
            || ["responsibility", "rationale"]
                .iter()
                .any(|k| boundary.and_then(|b| field(b, k)).is_none_or(blank_text))
            || boundary.and_then(|b| get(b, "excludes")).is_none()
        {
            unresolved.insert(id);
        }
        unresolved.extend(strings(node, "interfaces").filter(|a| owner(a) != Some(id)));
        component_views.insert(id,json!({"boundary":boundary.map(to_json).transpose()?.unwrap_or(json!({})), "interfaces_known":interfaces_known,"public_contracts":contracts(id)}));
    }
    let mut communities = BTreeMap::new();
    for &id in selected.iter().filter(|id| kind(id) == Some("community")) {
        let members = membership.get(id).map(Vec::as_slice).unwrap_or(&[]);
        let member_set: BTreeSet<_> = members.iter().copied().collect();
        let mut basis: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
        let mut unresolved_members = Vec::new();
        for &member in members {
            let provider = if kind(member) == Some("component") {
                Some(member)
            } else {
                owner(member)
            };
            if let Some(provider) = provider {
                basis.entry(provider).or_default().insert(member);
            } else if matches!(
                kind(member),
                Some("action" | "entity" | "screen" | "machine")
            ) {
                unresolved_members.push(member);
            }
        }
        unresolved_members.sort_unstable();
        let omitted_calls: Vec<_> = omitted
            .iter()
            .filter(|call| {
                call.consumer.is_some_and(|c| basis.contains_key(c))
                    || call.provider.is_some_and(|p| basis.contains_key(p))
                    || member_set.contains(call.source)
                    || member_set.contains(call.action)
            })
            .map(|c| c.edge)
            .collect();
        let omitted_components: Vec<_> = basis
            .keys()
            .copied()
            .filter(|c| !components.contains(c))
            .collect();
        basis.retain(|c, _| components.contains(c));
        let mut classified = BTreeMap::from([
            ("inbound", Vec::new()),
            ("outbound", Vec::new()),
            ("internal", Vec::new()),
            ("unresolved", Vec::new()),
        ]);
        for call in &calls {
            let consumer = call.consumer.is_some_and(|c| basis.contains_key(c));
            let provider = call.provider.is_some_and(|p| basis.contains_key(p));
            let category = match (consumer, provider) {
                _ if (call.consumer.is_none() || call.provider.is_none())
                    && (consumer
                        || provider
                        || member_set.contains(call.source)
                        || member_set.contains(call.action)) =>
                {
                    "unresolved"
                }
                (true, true) => "internal",
                (true, false) => "outbound",
                (false, true) => "inbound",
                (false, false) => continue,
            };
            classified
                .get_mut(category)
                .expect("known category")
                .push(call.edge);
        }
        let public_contracts: Vec<_> = basis.keys().flat_map(|id| contracts(id)).collect();
        communities.insert(id,json!({"membership_basis":basis,"omitted_components":omitted_components,"omitted_calls":omitted_calls,
            "unresolved_members":unresolved_members,"public_contracts":public_contracts,"calls":classified}));
    }
    Ok(
        json!({"format":"gdd-boundaries/0.3","spec_revision":revision,"scope":selected,"components":component_views,
        "communities":communities,"calls":calls,"omitted_calls":omitted.iter().map(|c|c.edge).collect::<Vec<_>>(),"unresolved":unresolved}),
    )
}

/// Return the four advisory questions and the matching rules in the supplied scope.
pub fn design_checklist(bundle: &Bundle, scope: &[String]) -> io::Result<Vec<Json>> {
    let nodes: Nodes<'_> = bundle.nodes().map(|n| (n.id, n.value)).collect();
    let mut by_principle: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for id in scope {
        let node = nodes.get(id.as_str()).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "Unknown design checklist scope.",
            )
        })?;
        if let Some(principle) = get(node, "rule").and_then(|r| field(r, "principle")) {
            by_principle.entry(principle).or_default().push(id);
        }
    }
    Ok(PRINCIPLES
        .iter()
        .map(|(principle, question)| {
            let mut rules = by_principle.remove(principle).unwrap_or_default();
            rules.sort_unstable();
            json!({"principle":principle,"question":question,"mode":"advisory","rules":rules})
        })
        .collect())
}
