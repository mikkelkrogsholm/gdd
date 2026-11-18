//! Advisory journey, component and data views of one validated model snapshot.

use super::{Value, entries, get, has_namespace, string};
use crate::bundle::{self, Bundle};
use crate::projection::{ProjectionError, validate_projection_bundle};
use crate::validation::to_json;
use serde::Serialize;
use serde_json::{Map, Value as Json, json};
use std::borrow::Borrow;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

const PRODUCT: &str = "gdd.product";
const PRODUCT_DEFINITION: &[u8] = include_bytes!("../../gdd_validator/extensions/product-0.1.json");

/// One captured model admitted for product perspectives.
#[derive(Debug)]
pub struct PerspectiveBundle<B = Bundle> {
    bundle: B,
    product_hash: Option<String>,
}

impl<B: Borrow<Bundle>> PerspectiveBundle<B> {
    /// Share the same immutable model with other snapshot consumers.
    pub fn bundle(&self) -> &Bundle {
        self.bundle.borrow()
    }

    /// Admit an owned or borrowed capture without reopening its inputs.
    pub fn admit(bundle: B) -> Result<Self, ProjectionError> {
        validate_projection_bundle(bundle.borrow())?;
        Self::from_validated(bundle)
    }

    // Callers have already admitted the model and resolved its snapshot.
    pub(crate) fn from_validated(bundle: B) -> Result<Self, ProjectionError> {
        let mut product_hash = None;
        for (namespace, doc) in bundle.borrow().extensions() {
            let defines_product_name = ["node_kinds", "relations"]
                .iter()
                .any(|key| entries(&doc.value, key).any(|(name, _)| has_namespace(name, PRODUCT)));
            if namespace == PRODUCT || defines_product_name {
                // Admission already matched the declaration's namespace, version
                // and SHA-256 against the parsed document and captured bytes.
                if namespace != PRODUCT || doc.asset.bytes != PRODUCT_DEFINITION {
                    return Err(ProjectionError::unsupported(
                        "Unsupported gdd.product semantics: requires the exact shipped product-0.1.json definition and bytehash (version 0.1.0).",
                    ));
                }
                product_hash = Some(doc.asset.sha256.clone());
            }
        }
        Ok(Self {
            bundle,
            product_hash,
        })
    }
}

impl TryFrom<Bundle> for PerspectiveBundle {
    type Error = ProjectionError;

    fn try_from(bundle: Bundle) -> Result<Self, Self::Error> {
        Self::admit(bundle)
    }
}

/// Load and validate the manifest, fragments and local assets once.
pub fn load_perspective_bundle(path: &Path) -> Result<PerspectiveBundle, ProjectionError> {
    bundle::load_for_inspection(path).try_into()
}

/// Project a local model. Missing links in a valid sketch are advisory gaps.
pub fn perspectives(path: &Path) -> Result<Json, ProjectionError> {
    perspective_data(&load_perspective_bundle(path)?)
}

struct Node<'a> {
    value: &'a Value,
    kind: &'a str,
    primary: Option<&'static str>,
}

struct Edge<'a> {
    value: &'a Value,
    id: &'a str,
    from: &'a str,
    to: &'a str,
    relation: &'a str,
}

struct Graph<'a> {
    nodes: BTreeMap<&'a str, Node<'a>>,
    edges: Vec<Edge<'a>>,
    // Indices follow canonical edge ID order. A self-loop appears once.
    incident: BTreeMap<&'a str, Vec<usize>>,
}

impl<'a> Graph<'a> {
    fn new(capture: &'a PerspectiveBundle<impl Borrow<Bundle>>) -> Self {
        let nodes = capture
            .bundle()
            .nodes()
            .map(|record| {
                let kind = string(record.value, "kind");
                let primary = match kind {
                    "gdd.product.actor" | "gdd.product.journey_step"
                        if capture.product_hash.is_some() =>
                    {
                        Some("journey")
                    }
                    "component" | "action" | "screen" => Some("components"),
                    "entity" | "machine" | "rule" => Some("data"),
                    _ => None,
                };
                (
                    record.id,
                    Node {
                        value: record.value,
                        kind,
                        primary,
                    },
                )
            })
            .collect();
        let mut edges: Vec<_> = capture
            .bundle()
            .edges()
            .map(|record| Edge {
                value: record.value,
                id: record.id,
                from: string(record.value, "from"),
                to: string(record.value, "to"),
                relation: string(record.value, "relation"),
            })
            .collect();
        edges.sort_unstable_by_key(|e| e.id);
        let mut incident = BTreeMap::<_, Vec<_>>::new();
        for (index, edge) in edges.iter().enumerate() {
            incident.entry(edge.from).or_default().push(index);
            if edge.to != edge.from {
                incident.entry(edge.to).or_default().push(index);
            }
        }
        Self {
            nodes,
            edges,
            incident,
        }
    }

    fn incident(&self, node: &str) -> &[usize] {
        self.incident
            .get(node)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    fn view(&self, name: &str) -> serde_json::Result<Json> {
        let primary: BTreeSet<_> = self
            .nodes
            .iter()
            .filter(|(_, node)| node.primary == Some(name))
            .map(|(&id, _)| id)
            .collect();
        let relevant: BTreeSet<usize> = primary
            .iter()
            .flat_map(|id| self.incident(id))
            .copied()
            .collect();
        let mut shown = primary.clone();
        let mut edges = Vec::with_capacity(relevant.len());
        let mut edge_ids = Vec::with_capacity(relevant.len());
        for &index in &relevant {
            let edge = &self.edges[index];
            shown.extend([edge.from, edge.to]);
            edges.push(to_json(edge.value)?);
            edge_ids.push(edge.id);
        }
        let mut nodes = Map::with_capacity(shown.len());
        let mut cross_links = Vec::new();
        for &id in &shown {
            let node = &self.nodes[id];
            nodes.insert(id.to_owned(), to_json(node.value)?);
            if let Some(other) = node.primary.filter(|other| *other != name) {
                let witnesses: Vec<_> = self
                    .incident(id)
                    .iter()
                    .filter(|i| relevant.contains(i))
                    .map(|&i| self.edges[i].id)
                    .collect();
                cross_links.push(json!({"node_id": id, "views": [other], "edge_ids": witnesses}));
            }
        }
        let omitted_nodes: Vec<_> = self
            .nodes
            .keys()
            .filter(|id| !shown.contains(*id))
            .collect();
        let omitted_edges: Vec<_> = self
            .edges
            .iter()
            .enumerate()
            .filter(|(i, _)| !relevant.contains(i))
            .map(|(_, edge)| edge.id)
            .collect();
        let mut view = json!({
            "primary_node_ids": primary,
            "cross_links": cross_links,
            "shown": {"node_ids": shown, "edge_ids": edge_ids},
            "omitted": {"node_ids": omitted_nodes, "edge_ids": omitted_edges},
        });
        view["nodes"] = Json::Object(nodes);
        view["edges"] = Json::Array(edges);
        Ok(view)
    }

    fn component_data_links(&self, product: bool) -> Vec<DataLink<'a>> {
        let mut owners = BTreeMap::<_, Vec<_>>::new();
        for edge in &self.edges {
            if edge.relation == "owned_by" {
                owners.entry(edge.from).or_default().push(edge);
            }
        }
        let mut links = Vec::new();
        for edge in &self.edges {
            match edge.relation {
                "gdd.product.uses" if product => {
                    links.push(DataLink {
                        component: edge.from,
                        entity: edge.to,
                        via: "uses",
                        edge_ids: vec![edge.id],
                    });
                }
                "owned_by" | "stored_in" if self.nodes[edge.from].kind == "entity" => {
                    links.push(DataLink {
                        component: edge.to,
                        entity: edge.from,
                        via: edge.relation,
                        edge_ids: vec![edge.id],
                    });
                }
                "reads" | "creates" | "changes" => {
                    for owner in owners.get(edge.from).into_iter().flatten() {
                        let mut edge_ids = vec![edge.id, owner.id];
                        edge_ids.sort_unstable();
                        links.push(DataLink {
                            component: owner.to,
                            entity: edge.to,
                            via: edge.relation,
                            edge_ids,
                        });
                    }
                }
                _ => {}
            }
        }
        links.sort_unstable();
        links
    }

    fn gaps(&self, links: &[DataLink]) -> Vec<Json> {
        let linked: BTreeSet<_> = links.iter().map(|link| link.entity).collect();
        let mut gaps = Vec::new();
        for (&id, node) in &self.nodes {
            let mut gap =
                |code, message| gaps.push(json!({"code": code, "node_id": id, "message": message}));
            if node.primary == Some("journey") && node.kind == "gdd.product.journey_step" {
                let mut incident = self.incident(id).iter().map(|&index| &self.edges[index]);
                if !incident
                    .clone()
                    .any(|e| e.relation == "gdd.product.participates" && e.to == id)
                {
                    gap("MISSING_ACTOR", "No human actor is linked to this step.");
                }
                let data = get(node.value, "data").expect("validated product step");
                if get(data, "user_action").is_none() {
                    gap(
                        "MISSING_USER_ACTION",
                        "The step has no declared user action.",
                    );
                }
                if get(data, "system_response").is_none() {
                    gap(
                        "MISSING_SYSTEM_RESPONSE",
                        "The step has no declared system response.",
                    );
                }
                if !incident.any(|e| e.relation == "gdd.product.realized_by" && e.from == id) {
                    gap(
                        "MISSING_RESPONSIBLE_COMPONENT",
                        "No responsible component is linked to this step.",
                    );
                }
            }
            if node.kind == "entity" && !linked.contains(id) {
                gap(
                    "MISSING_DATA_COMPONENT",
                    "No responsible or using component is linked to this entity.",
                );
            }
        }
        gaps
    }
}

// Field order is the canonical component, entity, relation and witness sort key.
#[derive(Eq, Ord, PartialEq, PartialOrd, Serialize)]
struct DataLink<'a> {
    component: &'a str,
    entity: &'a str,
    via: &'a str,
    edge_ids: Vec<&'a str>,
}

/// Derive detached JSON records without reading paths or repeating validation.
pub fn perspective_data(
    capture: &PerspectiveBundle<impl Borrow<Bundle>>,
) -> Result<Json, ProjectionError> {
    let graph = Graph::new(capture);
    let mut views = Map::new();
    for name in ["journey", "components", "data"] {
        let view = graph
            .view(name)
            .map_err(|e| ProjectionError::unsupported(e.to_string()))?;
        views.insert(name.to_owned(), view);
    }
    let links = graph.component_data_links(capture.product_hash.is_some());
    let gaps = graph.gaps(&links);
    let unsupported: Vec<_> = capture
        .bundle()
        .extensions()
        .map(|(namespace, _)| namespace)
        .filter(|&namespace| namespace != PRODUCT)
        .collect();
    let mut result = json!({
        "format": "gdd-perspectives/0.1",
        "spec_revision": capture.bundle().snapshot().expect("validated snapshot").revision,
        "advisory": true,
        "product_extension": capture.product_hash.as_ref().map(|hash| json!({"namespace": PRODUCT, "version": "0.1.0", "sha256": hash})),
        "unsupported_extensions": unsupported,
        "component_data_links": links,
        "gaps": gaps,
    });
    result["views"] = Json::Object(views);
    Ok(result)
}
