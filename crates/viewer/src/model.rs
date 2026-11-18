use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use gdd::bundle::{self, Bundle, Location};
use gdd::input::Value;
use gdd::perspectives::{PerspectiveBundle, perspective_data};
use serde_json::Value as Json;

pub struct Graph {
    pub title: String,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    // Shown records occupy the leading slice. The inspector can address every index.
    pub shown_nodes: usize,
    pub shown_edges: usize,
    pub artifact: Option<Json>,
    pub perspectives: Result<Perspectives, String>,
}

#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub enum Lens {
    #[default]
    All,
    Journey,
    Components,
    Data,
}

impl Lens {
    pub const PRODUCT: [Self; 3] = [Self::Journey, Self::Components, Self::Data];

    pub fn label(self) -> &'static str {
        match self {
            Self::All => "All graph",
            Self::Journey => "User journey",
            Self::Components => "Components",
            Self::Data => "Data model",
        }
    }

    fn key(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Journey => "journey",
            Self::Components => "components",
            Self::Data => "data",
        }
    }
}

// Inspector links use full model membership. Visibility also applies the saved scope.
pub struct Perspectives {
    views: BTreeMap<Lens, Membership>,
    pub gaps: Vec<(usize, String)>,
    pub has_product_extension: bool,
}

struct Membership {
    nodes: Vec<bool>,
    edges: Vec<bool>,
}

impl Perspectives {
    // The shared reader checks the envelope schema before these ID lists are used.
    fn new(
        data: &Json,
        node_indices: &BTreeMap<&str, usize>,
        edges: &[Edge],
    ) -> Result<Self, String> {
        let edge_indices = edges
            .iter()
            .enumerate()
            .map(|(i, edge)| (edge.id.as_str(), i))
            .collect();
        let membership = |group: &Json, indices: &BTreeMap<&str, usize>| {
            let mut mask = vec![false; indices.len()];
            for id in group.as_array().unwrap() {
                let id = id.as_str().unwrap();
                let &index = indices
                    .get(id)
                    .ok_or_else(|| format!("Unknown perspective record {id}."))?;
                mask[index] = true;
            }
            Ok::<_, String>(mask)
        };
        let mut views = BTreeMap::new();
        for lens in Lens::PRODUCT {
            let shown = &data["views"][lens.key()]["shown"];
            views.insert(
                lens,
                Membership {
                    nodes: membership(&shown["node_ids"], node_indices)?,
                    edges: membership(&shown["edge_ids"], &edge_indices)?,
                },
            );
        }
        let mut gaps = Vec::new();
        for gap in data["gaps"].as_array().unwrap() {
            let id = gap["node_id"].as_str().unwrap();
            let index = node_indices
                .get(id)
                .ok_or_else(|| format!("Unknown advisory gap node {id}."))?;
            gaps.push((*index, gap["message"].as_str().unwrap().to_owned()));
        }
        Ok(Self {
            views,
            gaps,
            has_product_extension: !data["product_extension"].is_null(),
        })
    }

    pub fn includes(&self, lens: Lens, index: usize) -> bool {
        self.views.get(&lens).is_none_or(|view| view.nodes[index])
    }
}

#[derive(Default)]
pub struct Visible {
    pub nodes: Vec<usize>,
    pub edges: Vec<usize>,
}

pub struct Node {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub description: String,
    pub human: Option<HumanConcept>,
}

pub enum HumanConcept {
    Step {
        action: Option<String>,
        response: Option<String>,
    },
    Actor {
        goal: Option<String>,
    },
}

pub struct Edge {
    pub id: String,
    pub from: usize,
    pub relation: String,
    pub to: usize,
}

impl Graph {
    pub fn load(path: &Path) -> Result<Self, String> {
        let result = if path
            .extension()
            .is_some_and(|s| s.eq_ignore_ascii_case("gddview"))
        {
            gdd::view::read_view_bytes(path)
                .map_err(|e| e.to_string())
                .and_then(|bytes| Self::from_artifact(&bytes))
        } else {
            let bundle = bundle::load_model_only(path);
            if let Some(doc) = bundle.root_document()
                && matches!(&doc.value, Value::Object(fields) if matches!(fields.get("format"), Some(Value::String(s)) if s == gdd::view::FORMAT))
            {
                Self::from_artifact(&doc.asset.bytes)
            } else {
                Self::from_bundle(&bundle)
            }
        };
        result.map_err(|error| format!("Cannot open {}: {error}", path.display()))
    }

    fn from_bundle(bundle: &Bundle) -> Result<Self, String> {
        if !bundle.diagnostics().is_empty() {
            return Err(bundle
                .diagnostics()
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n"));
        }
        let manifest = bundle
            .manifest()
            .ok_or_else(|| "Expected a GDD 0.2 or 0.3 model manifest.".to_owned())?;
        let document = object(&manifest.value, "model")?;
        let project = object(field(document, "project")?, "project")?;
        let title = text(project, "title")?.to_owned();
        let nodes = bundle
            .nodes()
            .map(|record| {
                project_node(record.id, record.value)
                    .map_err(|error| record_error(record.location, &error))
            })
            .collect::<Result<_, _>>()?;
        let projection = PerspectiveBundle::admit(bundle)
            .and_then(|capture| perspective_data(&capture))
            .map_err(|e| e.to_string());
        Self::project(
            title,
            nodes,
            None,
            projection.as_ref().map_err(Clone::clone),
            |indices| {
                bundle
                    .edges()
                    .map(|record| {
                        project_edge(record.id, record.value, indices)
                            .map_err(|error| record_error(record.location, &error))
                    })
                    .collect()
            },
        )
    }

    fn from_artifact(bytes: &[u8]) -> Result<Self, String> {
        let data = gdd::view::read_view(bytes).map_err(|e| e.to_string())?;
        let model = &data["model"];
        let title = model["project"]["title"].as_str().unwrap().to_owned();
        let nodes = model["nodes"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(id, node)| project_node(id, &Value::from(node)))
            .collect::<Result<_, _>>()?;
        let shown = data["manifest"]["shown_nodes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|id| id.as_str().unwrap())
            .collect();
        let mut graph = Self::project(
            title,
            nodes,
            Some(&shown),
            Ok(&data["perspectives"]),
            |indices| {
                model["edges"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|edge| {
                        project_edge(edge["id"].as_str().unwrap(), &Value::from(edge), indices)
                    })
                    .collect()
            },
        )?;
        graph.artifact = Some(data);
        Ok(graph)
    }

    fn project(
        title: String,
        mut nodes: Vec<Node>,
        shown: Option<&BTreeSet<&str>>,
        projection: Result<&Json, String>,
        project_edges: impl FnOnce(&BTreeMap<&str, usize>) -> Result<Vec<Edge>, String>,
    ) -> Result<Self, String> {
        let shown_nodes = if let Some(shown) = shown {
            nodes.sort_by_key(|node| !shown.contains(node.id.as_str()));
            shown.len()
        } else {
            nodes.len()
        };
        let indices = nodes
            .iter()
            .enumerate()
            .map(|(index, node)| (node.id.as_str(), index))
            .collect();
        let mut edges = project_edges(&indices)?;
        let omitted = |edge: &Edge| edge.from >= shown_nodes || edge.to >= shown_nodes;
        let shown_edges = if shown_nodes < nodes.len() {
            edges.sort_by_key(omitted);
            edges.partition_point(|edge| !omitted(edge))
        } else {
            edges.len()
        };
        let perspectives = projection.and_then(|data| Perspectives::new(data, &indices, &edges));
        Ok(Self {
            perspectives,
            title,
            nodes,
            edges,
            shown_nodes,
            shown_edges,
            artifact: None,
        })
    }

    pub fn visible(&self, lens: Lens, search: &str) -> Visible {
        let view = if lens == Lens::All {
            None
        } else if let Ok(projection) = &self.perspectives {
            projection.views.get(&lens)
        } else {
            return Visible::default();
        };
        let query = search.trim().to_lowercase();
        let mut matched = vec![false; self.shown_nodes];
        let nodes = self.nodes[..self.shown_nodes]
            .iter()
            .enumerate()
            .filter_map(|(i, node)| {
                matched[i] = view.is_none_or(|v| v.nodes[i])
                    && (query.is_empty()
                        || [&node.id, &node.label, &node.kind]
                            .iter()
                            .any(|s| s.to_lowercase().contains(&query)));
                matched[i].then_some(i)
            })
            .collect();
        let edges = self.edges[..self.shown_edges]
            .iter()
            .enumerate()
            .filter_map(|(i, edge)| {
                (matched[edge.from] && matched[edge.to] && view.is_none_or(|v| v.edges[i]))
                    .then_some(i)
            })
            .collect();
        Visible { nodes, edges }
    }
}

fn project_node(id: &str, value: &Value) -> Result<Node, String> {
    let node = object(value, id)?;
    let kind = text(node, "kind")?;
    let description = text(node, "description")?;
    if id.is_empty() || kind.is_empty() || description.is_empty() {
        return Err(format!(
            "node {id:?} must have an ID, kind, and description"
        ));
    }
    let label = match node.get("label") {
        None => id,
        Some(Value::String(label)) if label.is_empty() => id,
        Some(Value::String(label)) => label,
        Some(_) => return Err(format!("label for node {id:?} must be a string")),
    };
    let detail = |key| match node.get("data") {
        Some(Value::Object(data)) => text(data, key).ok().map(str::to_owned),
        _ => None,
    };
    let human = match kind {
        "gdd.product.journey_step" => Some(HumanConcept::Step {
            action: detail("user_action"),
            response: detail("system_response"),
        }),
        "gdd.product.actor" => Some(HumanConcept::Actor {
            goal: detail("goal"),
        }),
        _ => None,
    };
    Ok(Node {
        human,
        id: id.to_owned(),
        label: label.to_owned(),
        kind: kind.to_owned(),
        description: description.to_owned(),
    })
}

fn project_edge(id: &str, value: &Value, indices: &BTreeMap<&str, usize>) -> Result<Edge, String> {
    let edge = object(value, "edge")?;
    let relation = text(edge, "relation")?;
    if id.is_empty() || relation.is_empty() {
        return Err(format!("edge {id:?} must have an ID and relation"));
    }
    let resolve = |field| {
        let end = text(edge, field)?;
        indices
            .get(end)
            .copied()
            .ok_or_else(|| format!("edge {id:?} refers to unknown node {end:?}"))
    };
    Ok(Edge {
        from: resolve("from")?,
        to: resolve("to")?,
        id: id.to_owned(),
        relation: relation.to_owned(),
    })
}

fn record_error(location: &Location, error: &str) -> String {
    format!("{}{}: {error}", location.file, location.pointer)
}

type Object = BTreeMap<String, Value>;

fn object<'a>(value: &'a Value, name: &str) -> Result<&'a Object, String> {
    match value {
        Value::Object(entries) => Ok(entries),
        _ => Err(format!("{name:?} must be an object")),
    }
}

fn field<'a>(object: &'a Object, name: &str) -> Result<&'a Value, String> {
    object
        .get(name)
        .ok_or_else(|| format!("missing field {name:?}"))
}

fn text<'a>(object: &'a Object, name: &str) -> Result<&'a str, String> {
    match field(object, name)? {
        Value::String(value) => Ok(value),
        _ => Err(format!("{name:?} must be a string")),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use gdd::bundle;
    use serde_json::{Value, json};

    use super::{Graph, HumanConcept, Lens};
    use std::collections::BTreeSet;

    fn document(format: &str) -> Value {
        json!({
            "format": format,
            "project": {"id": "example.viewer", "title": "Example", "goal": "Preview a graph."},
            "nodes": {
                "node.a": {"kind": "component", "description": "A"},
                "node.b": {"kind": "action", "label": "B", "description": "B"}
            },
            "edges": [
                {"id": "edge.1", "from": "node.a", "relation": "owns", "to": "node.b"},
                {"id": "edge.2", "from": "node.a", "relation": "owns", "to": "node.b"},
                {"id": "edge.3", "from": "node.b", "relation": "owned_by", "to": "node.a"},
                {"id": "edge.4", "from": "node.a", "relation": "uses", "to": "node.a"}
            ]
        })
    }

    fn load_fixture(source: &str) -> Result<Graph, String> {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("model.gdd.yaml");
        fs::write(&file, source).unwrap();
        Graph::load(&file)
    }

    fn example(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples")
            .join(name)
    }

    #[test]
    fn keeps_parallel_reverse_and_self_edges() {
        for format in ["gdd/0.2", "gdd/0.3"] {
            let graph = load_fixture(&document(format).to_string()).unwrap();
            assert_eq!(graph.nodes[0].label, "node.a");
            assert_eq!(graph.nodes[1].label, "B");
            assert_eq!(
                graph
                    .edges
                    .iter()
                    .map(|edge| (edge.id.as_str(), edge.from, edge.to))
                    .collect::<Vec<_>>(),
                vec![
                    ("edge.1", 0, 1),
                    ("edge.2", 0, 1),
                    ("edge.3", 1, 0),
                    ("edge.4", 0, 0)
                ]
            );
        }
    }

    #[test]
    fn rejects_incomplete_or_unresolvable_graphs() {
        for (field, malformed) in [
            ("format", json!("gdd/0.4")),
            ("nodes", json!([])),
            ("nodes", json!({"node.a": ["A", "component", "A"]})),
            ("nodes", json!({"node.a": {"kind": "component"}})),
            ("edges", json!({})),
            ("edges", json!([["edge.1", "node.a", "owns", "node.b"]])),
            ("edges", json!([{"id": "edge.1", "from": "node.a"}])),
            ("includes", json!(["other.gdd.json"])),
            ("includes", json!(null)),
        ] {
            let mut model = document("gdd/0.3");
            model[field] = malformed;
            assert!(load_fixture(&model.to_string()).is_err(), "{field}");
        }
        for field in ["nodes", "edges"] {
            let mut model = document("gdd/0.3");
            model.as_object_mut().unwrap().remove(field);
            assert!(load_fixture(&model.to_string()).is_err(), "{field}");
        }
        let mut model = document("gdd/0.3");
        model["edges"][0]["to"] = json!("node.unknown");
        let error = load_fixture(&model.to_string()).err().unwrap();
        for part in ["model.gdd.yaml/edges/0", "edge.1", "node.unknown"] {
            assert!(error.contains(part), "{error}");
        }
    }

    #[test]
    fn loads_existing_standalone_examples() {
        const CONCERT_NODES: &[&str] = &[
            "action.cancel",
            "action.reserve",
            "community.backend",
            "community.frontend",
            "component.api",
            "component.frontend",
            "component.reservations",
            "component.storage",
            "entity.concert",
            "entity.reservation",
            "machine.reservation",
            "rule.api_boundary",
            "rule.atomicity",
            "rule.authorization",
            "rule.capacity",
            "rule.idempotency",
            "rule.state_machine",
            "scenario.api_boundary",
            "scenario.atomic_failure",
            "scenario.cancel",
            "scenario.cancel_concurrent",
            "scenario.cancel_invalid",
            "scenario.cancel_once",
            "scenario.cancel_unauthenticated",
            "scenario.concert_missing",
            "scenario.concurrent_same_key",
            "scenario.foreign_cancel",
            "scenario.key_conflict",
            "scenario.last_seat",
            "scenario.replay",
            "scenario.replay_after_cancel",
            "scenario.reservation_missing",
            "scenario.reserve",
            "scenario.reserve_invalid",
            "scenario.reserve_unauthenticated",
            "scenario.sold_out",
            "scenario.zero_capacity",
            "screen.reserve",
        ];
        let mut concert_03_nodes = CONCERT_NODES.to_vec();
        concert_03_nodes.extend([
            "community.reservation_feature",
            "rule.design.dry",
            "rule.design.kiss",
            "rule.design.modularity",
            "rule.design.separation_of_concerns",
        ]);
        concert_03_nodes.sort_unstable();

        let product_nodes = [
            "action.reserve",
            "actor.guest",
            "component.booking",
            "component.reservations",
            "entity.performance",
            "entity.reservation",
            "rule.capacity",
            "step.choose",
            "step.reserve",
        ];
        for (name, title, node_ids, edge_count) in [
            (
                "concert.gdd.yaml",
                "Concert seat reservations",
                CONCERT_NODES,
                85,
            ),
            (
                "concert-0.3.gdd.yaml",
                "Concert seat reservations",
                concert_03_nodes.as_slice(),
                87,
            ),
            (
                "product/greenfield.gdd.json",
                "Reserve a seat",
                product_nodes.as_slice(),
                12,
            ),
        ] {
            let graph = Graph::load(&example(name)).unwrap();
            assert_eq!(graph.title, title);
            assert_eq!(
                graph
                    .nodes
                    .iter()
                    .map(|node| node.id.as_str())
                    .collect::<Vec<_>>(),
                node_ids
            );
            assert_eq!(graph.edges.len(), edge_count);
        }
    }

    fn assert_fragmented_graph(graph: &Graph) {
        assert_eq!(graph.title, "Fragmented model");
        assert_eq!(
            graph
                .nodes
                .iter()
                .map(|node| node.id.as_str())
                .collect::<Vec<_>>(),
            ["component.notes", "entity.note"]
        );
        let [edge] = graph.edges.as_slice() else {
            panic!("expected one edge");
        };
        assert_eq!(edge.id, "edge.note.owner");
        assert_eq!(edge.relation, "owned_by");
        assert_eq!(graph.nodes[edge.from].id, "entity.note");
        assert_eq!(graph.nodes[edge.to].id, "component.notes");
    }

    #[test]
    fn opens_relocated_scoped_view_and_keeps_omitted_connection_in_inspector() {
        let inputs = tempfile::tempdir().unwrap();
        let destination = tempfile::tempdir().unwrap();
        let manifest = inputs.path().join("fragmented.gdd.yaml");
        fs::copy(example("fragmented.gdd.yaml"), &manifest).unwrap();
        fs::copy(
            example("note.fragment.yaml"),
            inputs.path().join("note.fragment.yaml"),
        )
        .unwrap();
        let path = destination.path().join("note.gddview");
        gdd::view::render_view(&manifest, &path, &["component.notes".into()]).unwrap();
        inputs.close().unwrap();
        for suffix in ["gddview", "json"] {
            let moved = destination.path().join(format!("received.{suffix}"));
            fs::copy(&path, &moved).unwrap();
            let graph = Graph::load(&moved).unwrap();
            assert_eq!((graph.shown_nodes, graph.shown_edges), (1, 0));
            assert_fragmented_graph(&graph);
            let layout = crate::canvas::Layout::new(&graph);
            assert!(layout.is_ok());
            assert_eq!(
                graph.artifact.unwrap()["locations"]["entity.note"][0],
                "note.fragment.yaml"
            );
        }
    }

    fn ids(values: &Value) -> BTreeSet<&str> {
        values
            .as_array()
            .unwrap()
            .iter()
            .map(|id| id.as_str().unwrap())
            .collect()
    }

    // Compare both sides of each visible partition with the portable contract.
    fn assert_lenses(graph: &Graph, artifact: &Value) {
        let scope = &artifact["manifest"];
        let all_nodes: BTreeSet<_> = graph.nodes.iter().map(|n| n.id.as_str()).collect();
        let all_edges: BTreeSet<_> = graph.edges.iter().map(|e| e.id.as_str()).collect();
        for lens in [Lens::All, Lens::Journey, Lens::Components, Lens::Data] {
            let visible = graph.visible(lens, "");
            let nodes = visible
                .nodes
                .iter()
                .map(|&i| graph.nodes[i].id.as_str())
                .collect();
            let edges = visible
                .edges
                .iter()
                .map(|&i| graph.edges[i].id.as_str())
                .collect();
            let view = &artifact["perspectives"]["views"][lens.key()];
            for (all, shown, scope_key, view_key) in [
                (&all_nodes, nodes, "shown_nodes", "node_ids"),
                (&all_edges, edges, "shown_edges", "edge_ids"),
            ] {
                let within = ids(&scope[scope_key]);
                let omitted: BTreeSet<_> = all.difference(&within).copied().collect();
                if lens == Lens::All {
                    assert_eq!(shown, within);
                } else {
                    let expected = ids(&view["shown"][view_key]);
                    let excluded = ids(&view["omitted"][view_key]);
                    assert_eq!(shown, within.intersection(&expected).copied().collect());
                    assert_eq!(
                        all.difference(&shown).copied().collect::<BTreeSet<_>>(),
                        omitted.union(&excluded).copied().collect()
                    );
                }
            }
        }
    }

    #[test]
    fn lenses_keep_canonical_context_and_exact_edge_sets_for_both_inputs() {
        let inputs = tempfile::tempdir().unwrap();
        let model = inputs.path().join("greenfield.gdd.json");
        let extension = inputs.path().join("extensions/product-0.1.json");
        fs::create_dir(extension.parent().unwrap()).unwrap();
        let model_bytes = fs::read(example("product/greenfield.gdd.json")).unwrap();
        let extension_bytes = fs::read(example("product/extensions/product-0.1.json")).unwrap();
        fs::write(&model, &model_bytes).unwrap();
        fs::write(&extension, &extension_bytes).unwrap();
        let destination = tempfile::tempdir().unwrap();
        let full_view = destination.path().join("full.gddview");
        let scoped_view = destination.path().join("scope.gddview");
        gdd::view::render_view(&model, &full_view, &[]).unwrap();
        gdd::view::render_view(&model, &scoped_view, &["actor.guest".into()]).unwrap();
        let raw = Graph::load(&model).unwrap();
        let captured = bundle::load_model_only(&model);
        assert_eq!(fs::read(&model).unwrap(), model_bytes);
        assert_eq!(fs::read(&extension).unwrap(), extension_bytes);
        inputs.close().unwrap();

        let full_bytes = fs::read(&full_view).unwrap();
        let full: Value = serde_json::from_slice(&full_bytes).unwrap();
        for graph in [
            raw,
            Graph::from_bundle(&captured).unwrap(),
            Graph::load(&full_view).unwrap(),
        ] {
            assert_lenses(&graph, &full);
            let steps = graph.visible(Lens::Components, " STEP.");
            assert_eq!(
                steps
                    .nodes
                    .iter()
                    .map(|&i| graph.nodes[i].id.as_str())
                    .collect::<Vec<_>>(),
                ["step.choose", "step.reserve"]
            );
            // Both contextual nodes survive, but their journey-only edge must be omitted.
            assert!(steps.edges.is_empty());
            let journey = graph.visible(Lens::Journey, "step.");
            assert_eq!(journey.edges.len(), 1);
            assert_eq!(graph.edges[journey.edges[0]].id, "edge.choose.reserve");
            let step = &graph.nodes[steps.nodes[0]];
            assert!(
                matches!(&step.human, Some(HumanConcept::Step { action, response })
                if action.as_deref() == Some("Select a performance.") && response.as_deref() == Some("Show available seats."))
            );
            let actor = &graph.nodes[graph.visible(Lens::Journey, "actor.guest").nodes[0]];
            assert!(
                matches!(&actor.human, Some(HumanConcept::Actor { goal }) if goal.as_deref() == Some("Reserve a seat."))
            );
        }
        let scoped_bytes = fs::read(&scoped_view).unwrap();
        let scoped = Graph::load(&scoped_view).unwrap();
        assert!(scoped.shown_nodes < scoped.nodes.len());
        assert_lenses(&scoped, &serde_json::from_slice(&scoped_bytes).unwrap());
        assert_eq!(fs::read(full_view).unwrap(), full_bytes);
        assert_eq!(fs::read(scoped_view).unwrap(), scoped_bytes);
    }

    #[test]
    fn legacy_and_unadmitted_models_have_distinct_empty_perspectives() {
        let legacy = Graph::load(&example("concert.gdd.yaml")).unwrap();
        assert!(!legacy.perspectives.as_ref().unwrap().has_product_extension);
        assert!(legacy.visible(Lens::Journey, "").nodes.is_empty());
        assert!(!legacy.visible(Lens::Components, "").nodes.is_empty());
        assert!(!legacy.visible(Lens::Data, "").edges.is_empty());
        let graph = load_fixture(&document("gdd/0.3").to_string()).unwrap();
        assert!(graph.perspectives.is_err());
        assert_eq!(graph.visible(Lens::All, "").edges.len(), 4);
        assert!(graph.visible(Lens::Components, "").nodes.is_empty());
    }

    #[test]
    fn a_repinned_product_definition_keeps_the_preview_and_reports_unsupported_semantics() {
        let dir = tempfile::tempdir().unwrap();
        let ext = dir.path().join("product.json");
        let mut bytes = fs::read(example("product/extensions/product-0.1.json")).unwrap();
        bytes.push(b'\n');
        fs::write(&ext, bytes).unwrap();
        let definition = bundle::load_model_only(&ext);
        let mut model: Value =
            serde_json::from_slice(&fs::read(example("product/partial.gdd.json")).unwrap())
                .unwrap();
        let declaration = &mut model["extensions"]["extension.product"];
        declaration["path"] = json!("product.json");
        declaration["sha256"] = json!(definition.root_document().unwrap().asset.sha256);
        let path = dir.path().join("model.json");
        fs::write(&path, model.to_string()).unwrap();

        let graph = Graph::load(&path).unwrap();
        assert!(
            graph
                .perspectives
                .as_ref()
                .err()
                .unwrap()
                .contains("Unsupported gdd.product semantics")
        );
        assert_eq!(graph.visible(Lens::All, "").nodes.len(), 2);
        assert!(graph.visible(Lens::Journey, "").nodes.is_empty());
    }

    #[test]
    fn edited_perspective_ids_cannot_escape_the_canonical_graph() {
        let destination = tempfile::tempdir().unwrap();
        let file = destination.path().join("partial.gddview");
        gdd::view::render_view(
            &example("product/partial.gdd.json"),
            &file,
            &["step.reserve".into()],
        )
        .unwrap();
        let source: Value = serde_json::from_slice(&fs::read(&file).unwrap()).unwrap();
        for pointer in [
            "/perspectives/gaps/0/node_id",
            "/perspectives/views/journey/shown/node_ids/0",
        ] {
            let mut altered = source.clone();
            *altered.pointer_mut(pointer).unwrap() = json!("missing.node");
            let graph = Graph::from_artifact(&serde_json::to_vec(&altered).unwrap()).unwrap();
            assert!(
                graph
                    .perspectives
                    .as_ref()
                    .err()
                    .unwrap()
                    .contains("missing.node")
            );
            assert_eq!(graph.visible(Lens::All, "").nodes.len(), 1);
            assert!(graph.visible(Lens::Journey, "").nodes.is_empty());
        }
    }

    #[test]
    fn opens_fragmented_example() {
        let graph = Graph::load(&example("fragmented.gdd.yaml")).unwrap();
        assert_fragmented_graph(&graph);
    }

    #[test]
    fn projects_retained_bundle_after_source_files_are_removed() {
        let root = tempfile::tempdir().unwrap();
        let manifest = root.path().join("fragmented.gdd.yaml");
        let fragment = root.path().join("note.fragment.yaml");
        fs::copy(example("fragmented.gdd.yaml"), &manifest).unwrap();
        fs::copy(example("note.fragment.yaml"), &fragment).unwrap();
        let bundle = bundle::load(&manifest);
        fs::remove_file(&manifest).unwrap();
        fs::remove_file(&fragment).unwrap();
        let graph = Graph::from_bundle(&bundle).unwrap();
        assert_fragmented_graph(&graph);
    }

    #[test]
    fn rejects_bad_fragments_with_source_diagnostics() {
        let root = tempfile::tempdir().unwrap();
        let manifest = root.path().join("fragmented.gdd.yaml");
        let fragment = root.path().join("note.fragment.yaml");
        fs::copy(example("fragmented.gdd.yaml"), &manifest).unwrap();

        let error = Graph::load(&manifest).err().unwrap();
        assert!(
            error.contains("note.fragment.yaml") && error.contains("SOURCE_UNAVAILABLE"),
            "{error}"
        );

        let source = fs::read_to_string(example("note.fragment.yaml")).unwrap();
        fs::write(
            &fragment,
            source.replace("entity.note:", "component.notes:"),
        )
        .unwrap();
        let error = Graph::load(&manifest).err().unwrap();
        assert!(
            error.contains("note.fragment.yaml/nodes/component.notes")
                && error.contains("ID_COLLISION"),
            "{error}"
        );
    }

    #[test]
    fn shared_loader_checks_node_ids_and_ignored_fields() {
        let json = document("gdd/0.3").to_string();
        for (source, expected) in [
            (
                json.replace(
                    r#""nodes":{"#,
                    r#""nodes":{"node.a":{},"#,
                ),
                "Duplicate mapping key \"node.a\"",
            ),
            (
                json.replace(
                    r#""kind":"component""#,
                    r#""kind":"component","annotations":{"x":1,"x":2}"#,
                ),
                "Duplicate mapping key \"x\"",
            ),
            (
                json.replace(
                    r#""kind":"component""#,
                    r#""kind":"component","annotations":{"<<":2}"#,
                ),
                "Merge keys are forbidden",
            ),
            (
                json.replace(
                    r#""kind":"component""#,
                    &format!(r#""kind":"component","annotations":{{"deep":{}0{}}}"#, "[".repeat(253), "]".repeat(253)),
                ),
                "nesting limit",
            ),
            (
                json.replace(r#""id":"edge.2""#, r#""id":"edge.1""#),
                "ID_COLLISION",
            ),
            (
                "format: gdd/0.3\nproject: {title: Example}\nnodes: {}\nedges: []\nannotations: &forbidden 1".into(),
                "Anchors and aliases are forbidden",
            ),
        ] {
            let error = load_fixture(&source).err().unwrap();
            assert!(error.contains(expected), "{error}");
        }
        let with_deep_value = json.replace(
            r#""kind":"component""#,
            &format!(
                r#""kind":"component","annotations":{{"deep":{}0{}}}"#,
                "[".repeat(252),
                "]".repeat(252)
            ),
        );
        assert!(load_fixture(&with_deep_value).is_ok());
    }
}
