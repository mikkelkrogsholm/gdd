use super::*;
use crate::canvas::Layout;
use eframe::egui::Vec2;
use serde_json::{Value, json};

fn load(nodes: Vec<(String, &str)>, edges: Vec<(String, &'static str, String)>) -> Graph {
    let nodes: serde_json::Map<_, _> = nodes
        .into_iter()
        .map(|(id, kind)| {
            (
                id,
                json!({"kind": kind, "label":"Record", "description":"Fixture"}),
            )
        })
        .collect();
    let edges: Vec<_> = edges.into_iter().enumerate().map(|(i, (a, relation, b))|
        json!({"id":format!("edge.{i}"), "from":a, "relation":relation, "to":b})).collect();
    let input = json!({"format":"gdd/0.2", "project":{"id":"test", "title":"Groups", "goal":"Read groups"}, "nodes":Value::Object(nodes), "edges":edges});
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("groups.json");
    std::fs::write(&path, serde_json::to_vec(&input).unwrap()).unwrap();
    Graph::load(&path).unwrap()
}

#[test]
fn dense_communities_and_ownership_have_separate_reproducible_drawings() {
    // Three densely connected groups, linked in a directed cycle. IDs interleave
    // groups, and every node has the same label.
    for explicit in [true, false] {
        let id = |group: usize, item: usize| format!("n{}", group + 3 * item);
        let mut nodes = vec![];
        let mut edges = vec![];
        for group in 0..3 {
            let owner = id(group, 8);
            let community = id(group, 9);
            nodes.push((owner.clone(), "component"));
            if explicit {
                nodes.push((community.clone(), "community"));
                edges.push((owner.clone(), "member_of", community.clone()));
            }
            for j in 0..8 {
                let action = id(group, j);
                nodes.push((action.clone(), "action"));
                edges.push((action.clone(), "owned_by", owner.clone()));
                if explicit {
                    edges.push((action.clone(), "member_of", community.clone()));
                }
                for k in 0..j {
                    edges.push((action.clone(), "calls", id(group, k)));
                }
            }
            edges.push((id(group, 0), "calls", id((group + 1) % 3, 0)));
        }
        let graph = load(nodes, edges);
        let layout = Layout::new(&graph).unwrap();
        let again = Layout::new(&graph).unwrap();
        assert_eq!(layout.nodes, again.nodes);
        let mut regions: Vec<(Vec2, f32)> = vec![];
        for group in 0..3 {
            let actions: Vec<_> = (0..8)
                .map(|k| {
                    let index = graph
                        .nodes
                        .iter()
                        .position(|n| n.id == id(group, k))
                        .unwrap();
                    layout.nodes[index].center().to_vec2()
                })
                .collect();
            let center = actions.iter().fold(Vec2::ZERO, |sum, &p| sum + p) / actions.len() as f32;
            let radius = (actions
                .iter()
                .map(|p| (*p - center).length_sq())
                .sum::<f32>()
                / actions.len() as f32)
                .sqrt();
            for &(other, spread) in &regions {
                assert!((center - other).length() > radius + spread);
            }
            regions.push((center, radius));
        }
        for (i, node) in layout.nodes.iter().enumerate() {
            for other in &layout.nodes[..i] {
                assert!(!node.intersects(*other));
            }
        }
        assert_eq!(layout.routes.len(), graph.edges.len());
        for (edge, route) in graph.edges.iter().zip(&layout.routes) {
            assert!(
                layout.nodes[edge.from]
                    .expand(0.02)
                    .contains(route.points[0])
            );
            assert!(
                layout.nodes[edge.to]
                    .expand(0.02)
                    .contains(*route.points.last().unwrap())
            );
        }
    }
}

#[test]
fn explicit_membership_unique_owners_and_three_simultaneous_waves_use_only_shown_nodes() {
    let nodes = [
        ("a", "community"),
        ("b", "community"),
        ("shared", "action"),
        ("root", "component"),
        ("chain", "action"),
        ("leaf", "action"),
        ("ambiguous", "action"),
        ("cycle1", "action"),
        ("cycle2", "action"),
        ("step1", "scenario"),
        ("step2", "rule"),
        ("step3", "scenario"),
        ("step4", "rule"),
        ("zzoutside", "community"),
    ];
    let edges = [
        ("shared", "member_of", "b"),
        ("shared", "member_of", "a"),
        ("shared", "owned_by", "root"),
        ("leaf", "owned_by", "chain"),
        ("chain", "owned_by", "root"),
        ("chain", "owned_by", "root"),
        ("ambiguous", "owned_by", "root"),
        ("ambiguous", "owned_by", "a"),
        ("cycle1", "owned_by", "cycle2"),
        ("cycle2", "owned_by", "cycle1"),
        ("step4", "applies_to", "step3"),
        ("step3", "exercises", "step2"),
        ("step2", "changes", "step1"),
        ("step1", "exercises", "b"),
        ("step1", "exercises", "shared"),
        ("root", "member_of", "zzoutside"),
    ];
    let mut graph = load(
        nodes.map(|(id, kind)| (id.into(), kind)).into(),
        edges.map(|(a, r, b)| (a.into(), r, b.into())).into(),
    );
    // Model the leading shown slices used by a saved view.
    let last = graph.nodes.len() - 1;
    assert_eq!(graph.nodes[last].id, "zzoutside");
    graph.edges.sort_by_key(|e| e.from == last || e.to == last);
    graph.shown_nodes = last;
    graph.shown_edges -= 1;
    let groups = placement_groups(&graph);
    let actual = |id: &str| {
        groups[graph.nodes.iter().position(|n| n.id == id).unwrap()]
            .map(|i| graph.nodes[i].id.as_str())
    };
    for (id, expected) in [
        ("a", Some("a")),
        ("b", Some("b")),
        ("shared", Some("a")),
        ("root", Some("root")),
        ("chain", Some("root")),
        ("leaf", Some("root")),
        ("ambiguous", None),
        ("cycle1", None),
        ("cycle2", None),
        ("step1", Some("a")),
        ("step2", Some("a")),
        ("step3", Some("a")),
        ("step4", None),
    ] {
        assert_eq!(actual(id), expected, "{id}");
    }
    graph.edges[..graph.shown_edges].reverse();
    assert_eq!(placement_groups(&graph), groups);
}
