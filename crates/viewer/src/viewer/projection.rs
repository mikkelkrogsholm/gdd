use crate::model::{Filters, FoldProjection, Graph, Visible};
use serde_json::{Value as Json, json};

pub(super) fn projection_details(
    graph: &Graph,
    filters: &Filters,
    visible: &Visible,
    folded: Option<&FoldProjection>,
) -> Json {
    let artifact = graph.input.artifact();
    let revision = graph
        .input
        .overview()
        .snapshot
        .and_then(|s| s.get("revision"));
    let node_id = |i: usize| graph.nodes[i].id.as_str();
    let edge_id = |i: usize| graph.edges[i].id.as_str();

    let unfolded;
    let drawing = match folded {
        Some(fold) => fold,
        None => {
            unfolded = visible.project(graph, false);
            &unfolded
        }
    };
    let glyphs: Vec<_> = drawing
        .glyphs
        .iter()
        .map(|glyph| {
            json!({
                "id": node_id(glyph.node),
                "node_ids": glyph.members.iter().copied().map(node_id).collect::<Vec<_>>(),
            })
        })
        .collect();
    let bundles: Vec<_> = drawing
        .bundles
        .iter()
        .map(|bundle| {
            json!({
                "from": node_id(bundle.from),
                "to": node_id(bundle.to),
                "edge_ids": bundle.edges.iter().copied().map(edge_id).collect::<Vec<_>>(),
            })
        })
        .collect();
    let internal = drawing
        .glyphs
        .iter()
        .flat_map(|g| g.internal.iter().copied())
        .map(edge_id);
    let empty = json!([]);
    let artifact_scope = match artifact {
        Some(data) => data["manifest"].clone(),
        None => {
            // Raw inputs have the same entire-model scope as an unscoped saved view.
            let mut nodes: Vec<_> = graph.nodes.iter().map(|n| n.id.as_str()).collect();
            let mut edges: Vec<_> = graph.edges.iter().map(|e| e.id.as_str()).collect();
            nodes.sort_unstable();
            edges.sort_unstable();
            json!({
                "shown_nodes": nodes, "shown_edges": edges,
                "omitted_nodes": [], "omitted_edges": [],
            })
        }
    };
    let mut details = json!({
        "spec_revision": revision,
        "observation_inputs": artifact.map_or(&empty, |a| &a["observation_inputs"]),
        "scope": artifact.map_or(&empty, |a| &a["scope"]),
        "filters": {
            "perspective": filters.lens.key(), "search": filters.search, "kind": filters.kind,
            "relation": filters.relation, "community": filters.community, "scenarios": filters.show_scenarios,
        },
        "folding": {
            "enabled": folded.is_some(),
            "overlap_policy": "first visible community by canonical ID; all memberships retained",
        }
    });
    details["artifact_scope"] = artifact_scope;
    // Use the full model boundary so omissions include both filtered and out-of-scope records.
    for (part, node_key, edge_key) in [
        (Partition::Visible, "shown_nodes", "shown_edges"),
        (Partition::Filtered, "omitted_nodes", "omitted_edges"),
    ] {
        details[node_key] =
            json!(part.ids(&graph.nodes, graph.nodes.len(), &visible.nodes, |n| &n.id));
        details[edge_key] =
            json!(part.ids(&graph.edges, graph.edges.len(), &visible.edges, |e| &e.id));
    }
    details["glyphs"] = glyphs.into();
    details["bundles"] = bundles.into();
    details["folded_internal_edges"] = json!(sort_view_ids(internal.collect()));
    details
}

#[derive(Clone, Copy)]
pub(super) enum Partition {
    Visible,
    Filtered,
    Outside,
}

impl Partition {
    pub(super) fn count(self, visible: usize, scope: usize, total: usize) -> usize {
        match self {
            Self::Visible => visible,
            Self::Filtered => scope - visible,
            Self::Outside => total - scope,
        }
    }

    pub(super) fn ids<'a, T>(
        self,
        records: &'a [T],
        scope: usize,
        visible: &[usize],
        id: impl Fn(&'a T) -> &'a str,
    ) -> Vec<&'a str> {
        let ids = match self {
            Self::Visible => visible.iter().map(|&i| id(&records[i])).collect(),
            Self::Filtered => records[..scope]
                .iter()
                .enumerate()
                .filter(|(i, _)| visible.binary_search(i).is_err())
                .map(|(_, record)| id(record))
                .collect(),
            Self::Outside => records[scope..].iter().map(id).collect(),
        };
        sort_view_ids(ids)
    }
}

fn sort_view_ids(mut ids: Vec<&str>) -> Vec<&str> {
    ids.sort_unstable_by(|left, right| left.encode_utf16().cmp(right.encode_utf16()));
    ids
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ViewInput;
    use std::path::Path;

    #[test]
    fn unavailable_snapshot_has_no_revision() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../conformance/fixtures/viewer-multigraph.json");
        let mut graph = Graph::load(&path).unwrap();
        let ViewInput::Model(capture) = &mut graph.input else {
            unreachable!()
        };
        capture.snapshot = None;
        let filters = graph.initial_filters();
        let visible = graph.visible(&filters);
        let details = projection_details(&graph, &filters, &visible, None);
        assert_eq!(details.get("spec_revision"), Some(&Json::Null));
        assert!(!details["shown_nodes"].as_array().unwrap().is_empty());
    }
}
