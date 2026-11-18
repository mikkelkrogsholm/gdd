use std::collections::{BTreeMap, BTreeSet};

use gdd::input::{self, Value};

pub struct Graph {
    pub title: String,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
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
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        let value = input::parse(bytes).map_err(|error| error.to_string())?;
        let document = object(&value, "model")?;
        let format = text(document, "format")?;
        if !matches!(format, "gdd/0.2" | "gdd/0.3") {
            return Err(format!(
                "unsupported format {format:?}, expected gdd/0.2 or gdd/0.3"
            ));
        }
        if let Some(includes) = document.get("includes")
            && !array(includes, "includes")?.is_empty()
        {
            return Err("the preview requires an embedded graph and cannot load includes".into());
        }

        let project = object(field(document, "project")?, "project")?;
        let title = text(project, "title")?.to_owned();
        let node_records = object(field(document, "nodes")?, "nodes")?;
        let mut nodes = Vec::with_capacity(node_records.len());
        for (id, record) in node_records {
            let node = object(record, id)?;
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
            nodes.push(Node {
                id: id.clone(),
                label: label.clone(),
                kind: kind.to_owned(),
                description: description.to_owned(),
            });
        }
        let indices: BTreeMap<_, _> = nodes
            .iter()
            .enumerate()
            .map(|(index, node)| (node.id.as_str(), index))
            .collect();

        let edge_records = array(field(document, "edges")?, "edges")?;
        let mut edge_ids = BTreeSet::new();
        let mut edges = Vec::with_capacity(edge_records.len());
        for record in edge_records {
            let edge = object(record, "edge")?;
            let id = text(edge, "id")?;
            let relation = text(edge, "relation")?;
            if id.is_empty() || relation.is_empty() {
                return Err(format!("edge {id:?} must have an ID and relation"));
            }
            if !edge_ids.insert(id) {
                return Err(format!("duplicate edge ID {id:?}"));
            }
            let resolve = |field| {
                let end = text(edge, field)?;
                indices
                    .get(end)
                    .copied()
                    .ok_or_else(|| format!("edge {id:?} refers to unknown node {end:?}"))
            };
            edges.push(Edge {
                from: resolve("from")?,
                to: resolve("to")?,
                id: id.to_owned(),
                relation: relation.to_owned(),
            });
        }
        Ok(Self {
            title,
            nodes,
            edges,
        })
    }
}

type Object = BTreeMap<String, Value>;

fn object<'a>(value: &'a Value, name: &str) -> Result<&'a Object, String> {
    match value {
        Value::Object(entries) => Ok(entries),
        _ => Err(format!("{name:?} must be an object")),
    }
}

fn array<'a>(value: &'a Value, name: &str) -> Result<&'a [Value], String> {
    match value {
        Value::Array(items) => Ok(items),
        _ => Err(format!("{name:?} must be an array")),
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
    use serde_json::{Value, json};

    use super::Graph;

    fn document(format: &str) -> Value {
        json!({
            "format": format,
            "project": {"title": "Example"},
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

    #[test]
    fn keeps_parallel_reverse_and_self_edges() {
        for format in ["gdd/0.2", "gdd/0.3"] {
            let graph = Graph::parse(document(format).to_string().as_bytes()).unwrap();
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
            assert!(
                Graph::parse(model.to_string().as_bytes()).is_err(),
                "{field}"
            );
        }
        for field in ["nodes", "edges"] {
            let mut model = document("gdd/0.3");
            model.as_object_mut().unwrap().remove(field);
            assert!(
                Graph::parse(model.to_string().as_bytes()).is_err(),
                "{field}"
            );
        }
        let mut model = document("gdd/0.3");
        model["edges"][0]["to"] = json!("node.unknown");
        let error = Graph::parse(model.to_string().as_bytes()).err().unwrap();
        assert!(error.contains("edge.1") && error.contains("node.unknown"));
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
        for (bytes, title, node_ids, edge_count) in [
            (
                include_bytes!("../../../examples/concert.gdd.yaml").as_slice(),
                "Concert seat reservations",
                CONCERT_NODES,
                85,
            ),
            (
                include_bytes!("../../../examples/concert-0.3.gdd.yaml").as_slice(),
                "Concert seat reservations",
                concert_03_nodes.as_slice(),
                87,
            ),
            (
                include_bytes!("../../../examples/product/greenfield.gdd.json").as_slice(),
                "Reserve a seat",
                product_nodes.as_slice(),
                12,
            ),
        ] {
            let graph = Graph::parse(bytes).unwrap();
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

    #[test]
    fn shared_parser_checks_node_ids_and_ignored_fields() {
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
                "duplicate edge ID",
            ),
            (
                "format: gdd/0.3\nproject: {title: Example}\nnodes: {}\nedges: []\nannotations: &forbidden 1".into(),
                "Anchors and aliases are forbidden",
            ),
        ] {
            let error = Graph::parse(source.as_bytes()).err().unwrap();
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
        assert!(Graph::parse(with_deep_value.as_bytes()).is_ok());
    }
}
