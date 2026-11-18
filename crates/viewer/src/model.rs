use std::collections::{BTreeMap, BTreeSet};

use serde::de::{Error, MapAccess, Visitor, value::MapAccessDeserializer};
use serde::{Deserialize, Deserializer};

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

#[derive(Deserialize)]
struct Document {
    format: String,
    project: Object<Project>,
    #[serde(default)]
    includes: Vec<String>,
    #[serde(deserialize_with = "read_nodes")]
    nodes: BTreeMap<String, NodeDocument>,
    edges: Vec<Object<EdgeDocument>>,
}

#[derive(Deserialize)]
struct Project {
    title: String,
}

#[derive(Deserialize)]
struct NodeDocument {
    #[serde(default)]
    label: String,
    kind: String,
    description: String,
}

#[derive(Deserialize)]
struct EdgeDocument {
    id: String,
    from: String,
    relation: String,
    to: String,
}

// Structs derived with Serde also accept positional arrays, but these records are JSON objects.
struct Object<T>(T);

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Object<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ObjectVisitor<T>(std::marker::PhantomData<T>);

        impl<'de, T: Deserialize<'de>> Visitor<'de> for ObjectVisitor<T> {
            type Value = Object<T>;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a JSON object")
            }

            fn visit_map<A: MapAccess<'de>>(self, entries: A) -> Result<Self::Value, A::Error> {
                T::deserialize(MapAccessDeserializer::new(entries)).map(Object)
            }
        }

        deserializer.deserialize_map(ObjectVisitor(std::marker::PhantomData))
    }
}

fn read_nodes<'de, D>(deserializer: D) -> Result<BTreeMap<String, NodeDocument>, D::Error>
where
    D: Deserializer<'de>,
{
    struct Nodes;

    impl<'de> Visitor<'de> for Nodes {
        type Value = BTreeMap<String, NodeDocument>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("an object mapping node IDs to nodes")
        }

        fn visit_map<A: MapAccess<'de>>(self, mut entries: A) -> Result<Self::Value, A::Error> {
            let mut nodes = BTreeMap::new();
            while let Some((id, Object(node))) =
                entries.next_entry::<String, Object<NodeDocument>>()?
            {
                if nodes.contains_key(&id) {
                    return Err(A::Error::custom(format!("duplicate node ID {id:?}")));
                }
                nodes.insert(id, node);
            }
            Ok(nodes)
        }
    }

    deserializer.deserialize_map(Nodes)
}

impl Graph {
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        let document: Document =
            serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
        if !matches!(document.format.as_str(), "gdd/0.2" | "gdd/0.3") {
            return Err(format!(
                "unsupported format {:?}, expected gdd/0.2 or gdd/0.3",
                document.format
            ));
        }
        if !document.includes.is_empty() {
            return Err("the preview requires an embedded graph and cannot load includes".into());
        }

        let mut nodes = Vec::with_capacity(document.nodes.len());
        for (id, node) in document.nodes {
            if id.is_empty() || node.kind.is_empty() || node.description.is_empty() {
                return Err(format!(
                    "node {id:?} must have an ID, kind, and description"
                ));
            }
            let label = if node.label.is_empty() {
                id.clone()
            } else {
                node.label
            };
            nodes.push(Node {
                id,
                label,
                kind: node.kind,
                description: node.description,
            });
        }
        let indices: BTreeMap<_, _> = nodes
            .iter()
            .enumerate()
            .map(|(index, node)| (node.id.as_str(), index))
            .collect();

        let mut edge_ids = BTreeSet::new();
        let mut edges = Vec::with_capacity(document.edges.len());
        for Object(edge) in document.edges {
            if edge.id.is_empty() || edge.relation.is_empty() {
                return Err(format!("edge {:?} must have an ID and relation", edge.id));
            }
            if !edge_ids.insert(edge.id.clone()) {
                return Err(format!("duplicate edge ID {:?}", edge.id));
            }
            let resolve = |end: &str| {
                indices
                    .get(end)
                    .copied()
                    .ok_or_else(|| format!("edge {:?} refers to unknown node {end:?}", edge.id))
            };
            edges.push(Edge {
                from: resolve(&edge.from)?,
                to: resolve(&edge.to)?,
                id: edge.id,
                relation: edge.relation,
            });
        }
        Ok(Self {
            title: document.project.0.title,
            nodes,
            edges,
        })
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
}
