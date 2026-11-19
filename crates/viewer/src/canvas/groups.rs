//! Semantic placement affects the drawing only. Every canonical edge is routed
//! on the complete topology after the regions have been positioned.
use std::collections::BTreeMap;

use graphlay::{GraphBuilder, LayoutError, LayoutOptions, NodeId, Placement, Point};

use crate::model::Graph;

#[derive(Clone, Copy, Default)]
enum Owner {
    #[default]
    None,
    Unique(usize),
    Ambiguous,
}

fn placement_groups(graph: &Graph) -> Vec<Option<usize>> {
    let mut groups: Vec<_> = graph.nodes[..graph.shown_nodes]
        .iter()
        .enumerate()
        .map(|(i, node)| {
            if node.kind == "community" {
                Some(i)
            } else {
                graph
                    .choices
                    .memberships
                    .get(i)?
                    .iter()
                    .copied()
                    .find(|&c| c < graph.shown_nodes)
            }
        })
        .collect();
    let edges = &graph.edges[..graph.shown_edges];
    let mut owners = vec![Owner::None; groups.len()];
    for edge in edges.iter().filter(|e| e.relation == "owned_by") {
        owners[edge.from] = match owners[edge.from] {
            Owner::None => Owner::Unique(edge.to),
            Owner::Unique(owner) if owner == edge.to => Owner::Unique(owner),
            _ => Owner::Ambiguous,
        };
    }
    for (i, group) in groups.iter_mut().enumerate() {
        if group.is_none()
            && graph.nodes[i].kind == "component"
            && !matches!(owners[i], Owner::Unique(_))
        {
            *group = Some(i);
        }
    }
    // Resolve each link at most once, including cycles without an anchor.
    let mut visited = vec![false; groups.len()];
    let mut path = Vec::new();
    for i in 0..groups.len() {
        let mut current = i;
        while groups[current].is_none() && !visited[current] {
            visited[current] = true;
            path.push(current);
            let Owner::Unique(next) = owners[current] else {
                break;
            };
            current = next;
        }
        let group = groups[current];
        for child in path.drain(..) {
            groups[child] = group;
        }
    }
    // Taking the first canonical group commutes with taking the union of
    // target memberships. Each wave reads only the preceding wave.
    for _ in 0..3 {
        let mut additions = BTreeMap::new();
        for edge in edges {
            if groups[edge.from].is_some()
                || !matches!(
                    edge.relation.as_str(),
                    "exercises" | "applies_to" | "changes"
                )
            {
                continue;
            }
            if let Some(target) = groups[edge.to] {
                let group = additions.entry(edge.from).or_insert(target);
                if graph.nodes[target]
                    .id
                    .encode_utf16()
                    .cmp(graph.nodes[*group].id.encode_utf16())
                    .is_lt()
                {
                    *group = target;
                }
            }
        }
        if additions.is_empty() {
            break;
        }
        for (i, group) in additions {
            groups[i] = Some(group);
        }
    }
    groups
}

pub(super) fn layout(
    graph: &Graph,
    records: &[usize],
    topology: &mut graphlay::Graph,
    options: &LayoutOptions,
) -> Result<graphlay::Layout, LayoutError> {
    let groups = placement_groups(graph);
    let mut regions = BTreeMap::<_, Vec<usize>>::new();
    for (slot, &record) in records.iter().enumerate() {
        regions.entry(groups[record]).or_default().push(slot);
    }
    // A singleton affinity has no related figure to keep close. Leave those
    // nodes together with the unassigned ones in their original arrangement.
    let mut unassigned = regions.remove(&None).unwrap_or_default();
    regions.retain(|_, slots| {
        if slots.len() == 1 {
            unassigned.push(slots[0]);
            false
        } else {
            true
        }
    });
    if !unassigned.is_empty() {
        regions.insert(None, unassigned);
    }
    if regions.len() < 2 {
        return graphlay::layout(topology, options);
    }

    let regions: Vec<_> = regions.into_values().collect();
    let large = topology.edges.len() >= super::LARGE_EDGE_COUNT;
    let placements = if large {
        // Several separate solves of a long chain can cost considerably more
        // than one graphlay pass over the complete large projection.
        graphlay::layout(topology, options)?.nodes
    } else {
        regional_placements(topology, &regions, options)?
    };
    let gap = 72.0;
    let bounds: Vec<_> = regions
        .iter()
        .map(|slots| region_bounds(slots, &placements))
        .collect();
    let area: f64 = bounds
        .iter()
        .map(|r| (r.width() + gap) * (r.height() + gap))
        .sum();
    let row_width = (area * 1.5).sqrt();
    let (mut left, mut top, mut row_height) = (0.0, 0.0, 0.0_f64);
    for (slots, bounds) in regions.into_iter().zip(bounds) {
        // Large graphs retain their rank coordinates. This bounds the work
        // needed to route long edges through intermediate ranks.
        if left > 0.0 && (large || left + bounds.width() > row_width) {
            top += row_height + gap;
            left = 0.0;
            row_height = 0.0;
        }
        for i in slots {
            let center = placements[i].center;
            let x = if large {
                center.x
            } else {
                center.x + left - bounds.min.x
            };
            topology.nodes[i].position = Some(Point::new(x, center.y + top - bounds.min.y));
        }
        row_height = row_height.max(bounds.height());
        left += bounds.width() + gap;
    }
    graphlay::reroute(topology, options)
}

fn region_bounds(slots: &[usize], placements: &[Placement]) -> graphlay::Rect {
    let count = slots.len() as f64;
    let mean = slots.iter().fold(Point::default(), |mut sum, &i| {
        sum.x += placements[i].center.x / count;
        sum.y += placements[i].center.y / count;
        sum
    });
    let mut bounds = graphlay::Rect::from_point(mean);
    let mut squared_spread = 0.0;
    for &i in slots {
        let node = &placements[i];
        bounds = bounds.union(graphlay::Rect::from_center(node.center, node.size));
        squared_spread += (node.center.x - mean.x).powi(2) + (node.center.y - mean.y).powi(2);
    }
    // Wide, shallow groups need clearance on the cross axis too. Reserve their
    // actual center spread in addition to the rectangles containing the labels.
    let diameter = 2.0 * (squared_spread / count).sqrt();
    bounds.union(graphlay::Rect::from_center(
        mean,
        graphlay::Size::new(diameter, diameter),
    ))
}

// Place ordinary groups using just their internal edges. A long chain of
// cross-group calls can otherwise stretch even a small region across the graph.
fn regional_placements(
    topology: &graphlay::Graph,
    regions: &[Vec<usize>],
    options: &LayoutOptions,
) -> Result<Vec<Placement>, LayoutError> {
    let mut local = vec![(0, NodeId(0)); topology.nodes.len()];
    let mut builders: Vec<_> = regions
        .iter()
        .enumerate()
        .map(|(region, slots)| {
            let mut builder = GraphBuilder::new();
            for &slot in slots {
                local[slot] = (region, builder.add_box(topology.nodes[slot].size));
            }
            builder
        })
        .collect();
    for edge in &topology.edges {
        let (a, from) = local[edge.source.index()];
        let (b, to) = local[edge.target.index()];
        if a == b {
            builders[a].connect(from, to);
        }
    }
    let mut placements = vec![Placement::default(); topology.nodes.len()];
    for (slots, builder) in regions.iter().zip(builders) {
        let drawing = graphlay::layout(&builder.build(), options)?;
        for (&slot, node) in slots.iter().zip(drawing.nodes) {
            placements[slot] = node;
        }
    }
    Ok(placements)
}

#[cfg(test)]
mod tests;
