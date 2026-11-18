use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use gdd::bundle::{self, Bundle, Location};
use gdd::input::Value;
use serde_json::Value as Json;

pub struct Graph {
    pub title: String,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    // Shown records occupy the leading slice. The inspector can address every index.
    pub shown_nodes: usize,
    pub shown_edges: usize,
    pub artifact: Option<Json>,
}

pub struct Node {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub description: String,
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
            std::fs::read(path)
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
        Self::project(title, nodes, None, |indices| {
            bundle
                .edges()
                .map(|record| {
                    project_edge(record.id, record.value, indices)
                        .map_err(|error| record_error(record.location, &error))
                })
                .collect()
        })
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
        let mut graph = Self::project(title, nodes, Some(&shown), |indices| {
            model["edges"]
                .as_array()
                .unwrap()
                .iter()
                .map(|edge| project_edge(edge["id"].as_str().unwrap(), &Value::from(edge), indices))
                .collect()
        })?;
        graph.artifact = Some(data);
        Ok(graph)
    }

    fn project(
        title: String,
        mut nodes: Vec<Node>,
        shown: Option<&BTreeSet<&str>>,
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
        Ok(Self {
            title,
            nodes,
            edges,
            shown_nodes,
            shown_edges,
            artifact: None,
        })
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
    Ok(Node {
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

    use super::Graph;

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
