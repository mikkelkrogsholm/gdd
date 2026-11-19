use super::*;

pub fn community_color(number: usize, dark: bool) -> egui::Color32 {
    let palette = if dark {
        [
            [112, 199, 247],
            [245, 175, 121],
            [126, 213, 176],
            [214, 167, 239],
            [232, 208, 120],
        ]
    } else {
        [
            [24, 94, 138],
            [147, 69, 22],
            [15, 106, 74],
            [123, 67, 143],
            [119, 89, 15],
        ]
    };
    let [r, g, b] = palette[(number - 1) % palette.len()];
    egui::Color32::from_rgb(r, g, b)
}

pub struct CommunityGeometry {
    pub(super) bounds: Rect,
    outlines: Vec<Outline>,
}

struct Outline {
    anchors: [Pos2; 2],
    vertices: Vec<Pos2>,
    bounds: Rect,
}

impl Layout {
    pub fn community_geometry(
        &self,
        visible: &Visible,
        fold: Option<&FoldProjection>,
    ) -> CommunityGeometry {
        let mut bounds = Rect::NOTHING;
        let mut outlines = Vec::with_capacity(visible.communities.len());
        for group in &visible.communities {
            let own = visible
                .nodes
                .binary_search(&group.node)
                .ok()
                .map(|_| group.node);
            let mut slots: Vec<_> = group
                .members
                .iter()
                .copied()
                .chain(own)
                .map(|node| fold.map_or(node, |projection| projection.glyph_for[node].unwrap()))
                .collect();
            slots.sort_unstable();
            slots.dedup();
            // Slightly different insets distinguish groups with identical representatives.
            let padding = 14.0 + (group.number % 5) as f32 * 3.0;
            let corners = slots
                .into_iter()
                .flat_map(|i| {
                    let r = self.nodes[i].expand(padding);
                    [
                        r.left_top(),
                        r.right_top(),
                        r.right_bottom(),
                        r.left_bottom(),
                    ]
                })
                .collect();
            let vertices = convex_hull(corners);
            let extent = Rect::from_points(&vertices);
            // Anchor headings to the horizontal hull edges. A corner of the
            // bounding rectangle can be far from a diagonal community outline.
            let anchors = [extent.top(), extent.bottom()].map(|y| {
                let mut edge = Rect::NOTHING;
                for &p in &vertices {
                    if p.y == y {
                        edge.extend_with(p);
                    }
                }
                edge.center()
            });
            outlines.push(Outline {
                anchors,
                vertices,
                bounds: extent,
            });
            bounds = bounds.union(extent);
        }
        CommunityGeometry { bounds, outlines }
    }
}

// Andrew's monotone chain. Positive turns are clockwise in screen coordinates.
fn convex_hull(mut points: Vec<Pos2>) -> Vec<Pos2> {
    points.sort_unstable_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    points.dedup();
    fn chain(points: impl Iterator<Item = Pos2>) -> Vec<Pos2> {
        let mut result: Vec<Pos2> = Vec::new();
        for p in points {
            while let [.., a, b] = result.as_slice() {
                let ab = *b - *a;
                let ap = p - *a;
                if ab.x * ap.y > ab.y * ap.x {
                    break;
                }
                result.pop();
            }
            result.push(p);
        }
        result.pop();
        result
    }
    let mut hull = chain(points.iter().copied());
    hull.extend(chain(points.into_iter().rev()));
    hull
}

impl CommunityGeometry {
    pub(super) fn paint(
        &self,
        ui: &egui::Ui,
        layout: &Layout,
        scene: &Scene<'_>,
        transform: TSTransform,
    ) {
        let clip = ui.clip_rect().shrink(2.0);
        if self.outlines.is_empty() || !clip.is_positive() {
            return;
        }
        let mut space = HeadingSpace::new(clip);
        if let Some(fold) = scene.folded {
            for rect in &layout.nodes[..fold.glyphs.len()] {
                space.reserve(transform * *rect);
            }
        } else {
            for &i in &scene.visible.nodes {
                space.reserve(transform * layout.nodes[i]);
            }
        }
        let painter = ui.painter();
        for (group, outline) in scene.visible.communities.iter().zip(&self.outlines) {
            let bounds = transform * outline.bounds;
            if !clip.intersects(bounds) {
                continue;
            }
            let active = scene.selected == Some(Selection::Node(group.node));
            let color = community_color(group.number, ui.visuals().dark_mode);
            let points = outline.vertices.iter().map(|&p| transform * p).collect();
            painter.add(egui::Shape::convex_polygon(
                points,
                color.gamma_multiply(if active { 0.14 } else { 0.055 }),
                Stroke::new(
                    if active { 2.2 } else { 1.3 },
                    color.gamma_multiply(if active { 1.0 } else { 0.75 }),
                ),
            ));
        }
        for (group, outline) in scene.visible.communities.iter().zip(&self.outlines) {
            let extent = (transform * outline.bounds).intersect(clip);
            if !extent.is_positive() || clip.width() < 40.0 {
                continue;
            }
            let color = community_color(group.number, ui.visuals().dark_mode);
            let mut job = egui::text::LayoutJob::single_section(
                group.heading.clone(),
                TextFormat::simple(FontId::proportional(13.0), color),
            );
            job.wrap.max_width = (clip.width() - 12.0).min(220.0);
            job.wrap.max_rows = 1;
            job.wrap.break_anywhere = true;
            let label = painter.layout_job(job);
            let anchors = outline.anchors.map(|anchor| transform * anchor);
            if let Some(rect) = space.place(anchors, label.size() + egui::vec2(10.0, 4.0)) {
                painter.rect_filled(rect, 3, ui.visuals().panel_fill);
                painter.galley(rect.center() - label.size() * 0.5, label, color);
            }
        }
    }
}

// Collision checks visit only the screen cells touched by the candidate heading.
struct HeadingSpace {
    clip: Rect,
    stride: usize,
    cells: Vec<bool>,
}

impl HeadingSpace {
    const CELL: f32 = 8.0;

    fn new(clip: Rect) -> Self {
        let stride = (clip.width() / Self::CELL).ceil() as usize + 1;
        let rows = (clip.height() / Self::CELL).ceil() as usize + 1;
        Self {
            clip,
            stride,
            cells: vec![false; stride * rows],
        }
    }

    fn rows(&self, rect: Rect) -> impl Iterator<Item = std::ops::Range<usize>> + use<> {
        let stride = self.stride;
        let min = (rect.min - self.clip.min) / Self::CELL;
        let max = (rect.max - self.clip.min) / Self::CELL;
        (min.y as usize..=max.y as usize).map(move |y| {
            let row = y * stride;
            row + min.x as usize..row + max.x as usize + 1
        })
    }

    fn reserve(&mut self, rect: Rect) {
        let rect = rect.intersect(self.clip);
        if rect.is_positive() {
            for row in self.rows(rect) {
                self.cells[row].fill(true);
            }
        }
    }

    fn place(&mut self, anchors: [Pos2; 2], size: Vec2) -> Option<Rect> {
        if size.x > self.clip.width() || size.y > self.clip.height() {
            return None;
        }
        let has_visible_anchor = anchors.iter().any(|&p| self.clip.contains(p));
        for row in 0..4 {
            let offset = 2.0 + row as f32 * (size.y + Self::CELL);
            for (side, anchor) in anchors.iter().enumerate() {
                if has_visible_anchor && !self.clip.contains(*anchor) {
                    continue;
                }
                let y = if side == 0 {
                    anchor.y - size.y - offset
                } else {
                    anchor.y + offset
                };
                for x in [anchor.x - size.x / 2.0, anchor.x, anchor.x - size.x] {
                    let pos = egui::pos2(
                        x.clamp(self.clip.left(), self.clip.right() - size.x),
                        y.clamp(self.clip.top(), self.clip.bottom() - size.y),
                    );
                    let rect = Rect::from_min_size(pos, size);
                    if self
                        .rows(rect.intersect(self.clip))
                        .all(|row| !self.cells[row].contains(&true))
                    {
                        self.reserve(rect);
                        return Some(rect);
                    }
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Filters;
    use std::collections::BTreeSet;
    use std::path::Path;

    #[test]
    fn concert_outlines_contain_each_explicit_visible_member_and_its_folded_representative() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/concert-0.3.gddview");
        let graph = Graph::load(&path).unwrap();
        let reserve = graph.actions["action.reserve"];
        for kind in ["", "action", "community"] {
            let visible = graph.visible(&Filters {
                kind: kind.into(),
                ..Default::default()
            });
            let folded = visible.project(&graph, true);
            assert_eq!(
                visible.communities.len(),
                if kind == "action" { 2 } else { 3 }
            );
            assert_eq!(
                visible
                    .communities
                    .iter()
                    .filter(|g| g.members.contains(&reserve))
                    .count(),
                if kind == "community" { 0 } else { 2 }
            );
            if kind != "community" {
                assert_eq!(
                    folded
                        .glyphs
                        .iter()
                        .flat_map(|g| &g.members)
                        .filter(|&&n| n == reserve)
                        .count(),
                    1
                );
            }
            for fold in [None, Some(&folded)] {
                let layout = match fold {
                    None => Layout::new(&graph),
                    Some(fold) => Layout::folded(&graph, fold),
                }
                .unwrap();
                let geometry = layout.community_geometry(&visible, fold);
                for (group, outline) in visible.communities.iter().zip(&geometry.outlines) {
                    let expected: BTreeSet<_> = graph
                        .edges
                        .iter()
                        .filter(|e| {
                            e.relation == "member_of"
                                && e.to == group.node
                                && visible.nodes.contains(&e.from)
                        })
                        .map(|e| e.from)
                        .collect();
                    assert_eq!(group.members, expected.iter().copied().collect::<Vec<_>>());
                    let own = visible.nodes.contains(&group.node).then_some(group.node);
                    let slots: BTreeSet<_> = expected
                        .into_iter()
                        .chain(own)
                        .map(|n| fold.map_or(n, |f| f.glyph_for[n].unwrap()))
                        .collect();
                    let node_bounds = slots
                        .iter()
                        .map(|&i| layout.nodes[i])
                        .fold(Rect::NOTHING, Rect::union);
                    let padding = outline.bounds.right() - node_bounds.right();
                    assert!(padding > 0.0);
                    assert_eq!(outline.bounds, node_bounds.expand(padding));
                    for i in slots {
                        let rect = layout.nodes[i];
                        for corner in [
                            rect.left_top(),
                            rect.right_top(),
                            rect.right_bottom(),
                            rect.left_bottom(),
                        ] {
                            for (a, b) in outline
                                .vertices
                                .iter()
                                .zip(outline.vertices.iter().cycle().skip(1))
                            {
                                let ab = *b - *a;
                                let ap = corner - *a;
                                assert!(
                                    ab.x * ap.y > ab.y * ap.x,
                                    "{corner:?} outside {}",
                                    group.heading
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}
