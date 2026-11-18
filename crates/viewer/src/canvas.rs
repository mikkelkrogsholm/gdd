use eframe::egui::{
    self, Color32, FontId, Painter, Pos2, Rect, Sense, Stroke, StrokeKind, TextFormat, Vec2,
    emath::TSTransform,
};
use graphlay::{GraphBuilder, LayoutOptions, Size};

use crate::model::{FoldProjection, Graph, Selection, Visible};

pub struct Layout {
    nodes: Vec<Rect>,
    routes: Vec<Route>,
    bounds: Rect,
}

struct Route {
    points: Vec<Pos2>,
    bounds: Rect,
    arrow_direction: Vec2,
    midpoint: Pos2,
}

pub struct Scene<'a> {
    pub graph: &'a Graph,
    pub visible: &'a Visible,
    pub folded: Option<&'a FoldProjection>,
    pub selected: Option<Selection>,
}

impl Scene<'_> {
    fn glyph(&self, node: usize) -> Option<usize> {
        match self.folded {
            Some(fold) => fold.glyph_for.get(node).copied().flatten(),
            None => self.visible.nodes.binary_search(&node).ok().map(|_| node),
        }
    }

    fn selected_glyph(&self) -> Option<usize> {
        match self.selected? {
            Selection::Node(i) => self.glyph(i),
            Selection::Edge(i) if self.visible.edges.binary_search(&i).is_ok() => {
                let edge = &self.graph.edges[i];
                self.glyph(edge.from)
                    .filter(|glyph| Some(*glyph) == self.glyph(edge.to))
            }
            _ => None,
        }
    }
}

impl Layout {
    pub fn new(graph: &Graph) -> Result<Self, String> {
        Self::build(
            graph,
            0..graph.shown_nodes,
            graph.edges[..graph.shown_edges]
                .iter()
                .enumerate()
                .map(|(i, e)| (i, e.from, e.to)),
        )
    }

    pub fn folded(graph: &Graph, fold: &FoldProjection) -> Result<Self, String> {
        Self::build(
            graph,
            fold.glyphs.iter().map(|glyph| glyph.node),
            fold.bundles.iter().map(|b| {
                (
                    b.edges[0],
                    fold.glyph_for[b.from].unwrap(),
                    fold.glyph_for[b.to].unwrap(),
                )
            }),
        )
    }

    fn build(
        graph: &Graph,
        nodes: impl Iterator<Item = usize>,
        edges: impl Iterator<Item = (usize, usize, usize)>,
    ) -> Result<Self, String> {
        // Graphlay retains insertion order, including routes reversed for layer assignment.
        let mut builder = GraphBuilder::new();
        let ids: Vec<_> = nodes
            .map(|i| {
                let width =
                    (24.0 + 7.0 * graph.nodes[i].label.chars().count() as f64).clamp(132.0, 236.0);
                builder.add_box(Size::new(width, 44.0))
            })
            .collect();
        let mut edge_ids = Vec::new();
        for (i, source, target) in edges {
            let (Some(&from), Some(&to)) = (ids.get(source), ids.get(target)) else {
                return Err(format!(
                    "Edge {} refers to an unknown node.",
                    graph.edges[i].id
                ));
            };
            builder.connect(from, to);
            edge_ids.push(i);
        }
        let drawing = graphlay::layout(&builder.build(), &LayoutOptions::layered_default())
            .map_err(|error| error.to_string())?;
        let position = |point: graphlay::Point| egui::pos2(point.x as f32, point.y as f32);
        let nodes = drawing
            .nodes
            .iter()
            .map(|node| {
                Rect::from_center_size(
                    position(node.center),
                    egui::vec2(node.size.width as f32, node.size.height as f32),
                )
            })
            .collect();
        let mut routes = Vec::with_capacity(drawing.edges.len());
        for (index, route) in drawing.edges.iter().enumerate() {
            let points: Vec<_> = route.points.iter().copied().map(position).collect();
            let direction = points.split_last().and_then(|(tip, previous)| {
                previous
                    .iter()
                    .rev()
                    .find(|p| *p != tip)
                    .map(|p| (*tip - *p).normalized())
            });
            let Some(arrow_direction) = direction else {
                return Err(format!(
                    "Edge {} has no drawable route.",
                    graph.edges[edge_ids[index]].id
                ));
            };
            routes.push(Route {
                bounds: Rect::from_points(&points),
                midpoint: route_midpoint(&points),
                points,
                arrow_direction,
            });
        }
        Ok(Self {
            nodes,
            routes,
            bounds: Rect::from_min_max(position(drawing.bounds.min), position(drawing.bounds.max)),
        })
    }

    pub fn show(
        &self,
        ui: &mut egui::Ui,
        scene: &Scene<'_>,
        zoom: f32,
        reveal: bool,
    ) -> Option<Selection> {
        let visible = scene.visible;
        let bounds = if scene.folded.is_some()
            || (visible.nodes.len() == self.nodes.len() && visible.edges.len() == self.routes.len())
        {
            self.bounds
        } else {
            visible
                .nodes
                .iter()
                .map(|&i| self.nodes[i])
                .chain(visible.edges.iter().map(|&i| self.routes[i].bounds))
                .fold(Rect::NOTHING, Rect::union)
        };
        egui::ScrollArea::both()
            .auto_shrink([false; 2])
            .show_viewport(ui, |ui, viewport| {
                let size = (bounds.size() * zoom + Vec2::splat(40.0)).max(viewport.size());
                let (area, _) = ui.allocate_exact_size(size, Sense::hover());
                let transform = TSTransform::new(
                    area.center().to_vec2() - bounds.center().to_vec2() * zoom,
                    zoom,
                );
                if reveal && let Some(index) = scene.selected_glyph() {
                    ui.scroll_to_rect((transform * self.nodes[index]).expand(12.0), None);
                }
                self.draw(ui, scene, transform, area)
            })
            .inner
    }

    fn draw(
        &self,
        ui: &mut egui::Ui,
        scene: &Scene<'_>,
        transform: TSTransform,
        area: Rect,
    ) -> Option<Selection> {
        let graph = scene.graph;
        let visible = scene.visible;
        let zoom = transform.scaling;
        let accent = Color32::from_rgb(32, 91, 165);
        let mut clicked = None;
        let canvas = ui.interact(area, ui.scope_id().with("canvas"), Sense::click());
        let pointer = canvas.interact_pointer_pos().filter(|_| canvas.clicked());
        let mut best_distance = 7.0;
        let selected_glyph = scene.selected_glyph();
        let n_routes = scene
            .folded
            .map_or(visible.edges.len(), |f| f.bundles.len());
        for highlighted in [false, true] {
            let stroke = if highlighted {
                Stroke::new(2.0, accent)
            } else {
                Stroke::new(1.2, Color32::from_rgb(139, 153, 171))
            };
            for i in 0..n_routes {
                let (route, source, target, records, selection) = if let Some(fold) = scene.folded {
                    let b = &fold.bundles[i];
                    (
                        &self.routes[i],
                        fold.glyph_for[b.from].unwrap(),
                        fold.glyph_for[b.to].unwrap(),
                        b.edges.as_slice(),
                        Selection::Bundle(b.from, b.to),
                    )
                } else {
                    let index = &visible.edges[i];
                    let edge = &graph.edges[*index];
                    (
                        &self.routes[*index],
                        edge.from,
                        edge.to,
                        std::slice::from_ref(index),
                        Selection::Edge(*index),
                    )
                };
                let active = match scene.selected {
                    Some(Selection::Node(_)) => {
                        selected_glyph == Some(source) || selected_glyph == Some(target)
                    }
                    Some(Selection::Edge(id)) => records.binary_search(&id).is_ok(),
                    other => other == Some(selection),
                };
                if active != highlighted
                    || !ui.is_rect_visible((transform * route.bounds).expand(32.0))
                {
                    continue;
                }
                let points: Vec<_> = route.points.iter().map(|&p| transform * p).collect();
                if let Some(pos) = pointer {
                    for pair in points.windows(2) {
                        let distance = distance_to_segment(pos, pair[0], pair[1]);
                        if distance < best_distance {
                            clicked = Some(selection);
                            best_distance = distance;
                        }
                    }
                }
                let tip = points[points.len() - 1];
                ui.painter().add(egui::Shape::line(points, stroke));
                arrowhead(ui.painter(), tip, route.arrow_direction, stroke, zoom);
                if scene.folded.is_some() && records.len() > 1 {
                    let center = transform * route.midpoint;
                    let galley = ui.painter().layout_no_wrap(
                        format!("×{}", records.len()),
                        FontId::proportional((12.0 * zoom).max(10.0)),
                        accent,
                    );
                    let rect = Rect::from_center_size(center, galley.size() + egui::vec2(8.0, 2.0));
                    let badge =
                        ui.interact(rect, ui.scope_id().with(("bundle", i)), Sense::click());
                    if badge
                        .on_hover_text(format!("Inspect {} edges", records.len()))
                        .clicked()
                    {
                        clicked = Some(selection);
                    }
                    ui.painter()
                        .rect_filled(rect, 4, Color32::from_rgb(230, 239, 252));
                    ui.painter()
                        .galley(center - galley.size() * 0.5, galley, accent);
                }
            }
        }

        let n_nodes = scene.folded.map_or(visible.nodes.len(), |f| f.glyphs.len());
        for i in 0..n_nodes {
            let (index, slot, member_count) = if let Some(fold) = scene.folded {
                let glyph = &fold.glyphs[i];
                (glyph.node, i, glyph.members.len())
            } else {
                (visible.nodes[i], visible.nodes[i], 1)
            };
            let rect = transform * self.nodes[slot];
            if !ui.is_rect_visible(rect) {
                continue;
            }
            let node = &graph.nodes[index];
            let response = ui
                .interact(
                    rect,
                    ui.scope_id().with(("canvas-node", index)),
                    Sense::CLICK,
                )
                .on_hover_text(&node.id);
            let active = selected_glyph == Some(slot);
            let fill = if active || response.hovered() {
                Color32::from_rgb(226, 238, 254)
            } else {
                Color32::WHITE
            };
            let border = if active || response.hovered() {
                Stroke::new(2.0, accent)
            } else {
                Stroke::new(1.0, Color32::from_rgb(166, 180, 198))
            };
            let painter = ui.painter();
            painter.rect(rect, 6, fill, border, StrokeKind::Inside);
            let mut job = egui::text::LayoutJob::single_section(
                node.label.clone(),
                TextFormat::simple(
                    FontId::proportional((13.0 * zoom).max(10.0)),
                    Color32::from_rgb(20, 39, 68),
                ),
            );
            job.wrap.max_width = rect.width() - 12.0 * zoom;
            job.wrap.max_rows = 1;
            job.wrap.break_anywhere = true;
            let label = painter.layout_job(job);
            let mut center = rect.center();
            if member_count > 1 {
                center.y -= 7.0 * zoom;
                painter.text(
                    rect.center() + egui::vec2(0.0, 9.0 * zoom),
                    egui::Align2::CENTER_CENTER,
                    format!("{member_count} nodes"),
                    FontId::proportional((11.0 * zoom).max(8.0)),
                    accent,
                );
            }
            painter.galley(center - label.size() * 0.5, label, Color32::BLACK);
            if response.clicked() {
                clicked = Some(Selection::Node(index));
            }
        }
        clicked
    }
}

fn distance_to_segment(pos: Pos2, start: Pos2, end: Pos2) -> f32 {
    let delta = end - start;
    if delta == Vec2::ZERO {
        return pos.distance(start);
    }
    let fraction = ((pos - start).dot(delta) / delta.length_sq()).clamp(0.0, 1.0);
    pos.distance(start + fraction * delta)
}

fn route_midpoint(points: &[Pos2]) -> Pos2 {
    let mut remaining = points.windows(2).map(|p| p[0].distance(p[1])).sum::<f32>() * 0.5;
    for pair in points.windows(2) {
        let length = pair[0].distance(pair[1]);
        if length > 0.0 && remaining <= length {
            return pair[0].lerp(pair[1], remaining / length);
        }
        remaining -= length;
    }
    points[0]
}

fn arrowhead(painter: &Painter, tip: Pos2, direction: Vec2, stroke: Stroke, zoom: f32) {
    let back = tip - direction * (8.0 * zoom);
    let side = egui::vec2(-direction.y, direction.x) * (4.0 * zoom);
    painter.add(egui::Shape::convex_polygon(
        vec![tip, back + side, back - side],
        stroke.color,
        Stroke::NONE,
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Edge, Node};

    fn graph(ids: &[&str], pairs: &[(usize, usize)]) -> Graph {
        Graph {
            title: "Layout fixture".into(),
            shown_nodes: ids.len(),
            shown_edges: pairs.len(),
            artifact: None,
            choices: Default::default(),
            perspectives: Err("Layout fixture".into()),
            nodes: ids
                .iter()
                .map(|&id| Node {
                    id: id.into(),
                    label: id.into(),
                    kind: "component".into(),
                    description: id.into(),
                    human: None,
                    search_text: id.into(),
                })
                .collect(),
            edges: pairs
                .iter()
                .enumerate()
                .map(|(i, &(from, to))| Edge {
                    id: format!("edge.{i}"),
                    relation: "links".into(),
                    search_text: String::new(),
                    from,
                    to,
                })
                .collect(),
        }
    }

    fn on_border(rect: Rect, port: Pos2) -> bool {
        rect.expand(0.01).contains(port) && !rect.shrink(0.01).contains(port)
    }

    #[test]
    fn routes_keep_edge_identity_and_point_to_declared_targets_across_cycles() {
        let graph = graph(
            &["node.z", "node.b", "node.a", "isolated"],
            &[(2, 0), (2, 0), (0, 1), (1, 2), (1, 1), (1, 1)],
        );
        let drawing = Layout::new(&graph).unwrap();
        assert_eq!(drawing.nodes.len(), graph.nodes.len());
        assert_eq!(drawing.routes.len(), graph.edges.len());
        for (i, rect) in drawing.nodes.iter().enumerate() {
            assert!(drawing.bounds.expand(0.01).contains_rect(*rect));
            assert!(
                drawing.nodes[..i]
                    .iter()
                    .all(|other| !rect.intersects(*other))
            );
        }
        for (edge, route) in graph.edges.iter().zip(&drawing.routes) {
            let source = drawing.nodes[edge.from];
            let target = drawing.nodes[edge.to];
            let tip = *route.points.last().unwrap();
            assert!(on_border(source, route.points[0]), "{}", edge.id);
            assert!(on_border(target, tip), "{}", edge.id);
            assert!(target.contains(tip + route.arrow_direction), "{}", edge.id);
            assert!(drawing.bounds.expand(0.01).contains_rect(route.bounds));
            if edge.from == edge.to {
                assert!(
                    route
                        .points
                        .iter()
                        .any(|&p| !target.expand(1.0).contains(p))
                );
            }
        }
        assert_ne!(drawing.routes[0].points, drawing.routes[1].points);
        assert_ne!(drawing.routes[4].points, drawing.routes[5].points);
    }

    #[test]
    fn canvas_paints_only_the_visible_records_at_their_original_positions() {
        let graph = graph(&["first", "hidden", "last"], &[(0, 2), (0, 2), (0, 1)]);
        let layout = Layout::new(&graph).unwrap();
        let visible = Visible {
            nodes: vec![0, 2],
            edges: vec![1],
        };
        let transform = TSTransform::from_translation(egui::vec2(20.0, 20.0));
        let context = egui::Context::default();
        let mut frame = context.run_ui(Default::default(), |ui| {
            layout.draw(
                ui,
                &Scene {
                    graph: &graph,
                    visible: &visible,
                    folded: None,
                    selected: Some(Selection::Node(0)),
                },
                transform,
                ui.max_rect(),
            );
        });
        frame.textures_delta.clear();
        let mut labels = Vec::new();
        let mut routes = Vec::new();
        for item in frame.shapes {
            match item.shape {
                egui::Shape::Text(text) => labels.push(text.galley.text().to_owned()),
                egui::Shape::Path(route) if !route.closed => routes.push(route.points),
                _ => {}
            }
        }
        assert_eq!(labels, ["first", "last"]);
        let expected: Vec<_> = layout.routes[1]
            .points
            .iter()
            .map(|&p| transform * p)
            .collect();
        assert_eq!(routes, [expected]);
    }

    #[test]
    fn folded_canvas_selects_representatives_bundles_and_reverse_routes() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../conformance/fixtures/viewer-multigraph.json");
        let graph = Graph::load(&path).unwrap();
        let visible = graph.visible(&Default::default());
        let fold = visible.project(&graph, true);
        let layout = Layout::folded(&graph, &fold).unwrap();
        assert_eq!(layout.nodes.len(), fold.glyphs.len());
        assert_eq!(layout.routes.len(), fold.bundles.len());
        let scene = Scene {
            graph: &graph,
            visible: &visible,
            folded: Some(&fold),
            selected: None,
        };
        let context = egui::Context::default();
        let transform = TSTransform::from_translation(egui::vec2(20.0, 20.0));
        let screen = (transform * layout.bounds).expand(30.0);
        let frame = |events| {
            let mut clicked = None;
            context
                .run_ui(
                    egui::RawInput {
                        screen_rect: Some(screen),
                        events,
                        ..Default::default()
                    },
                    |ui| {
                        clicked = layout.draw(ui, &scene, transform, screen);
                    },
                )
                .textures_delta
                .clear();
            clicked
        };
        frame(Vec::new());
        let click = |point| {
            let pos = transform * point;
            let button = |pressed| egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Default::default(),
            };
            frame(vec![egui::Event::PointerMoved(pos), button(true)]);
            frame(vec![button(false)])
        };
        let backend = graph
            .nodes
            .iter()
            .position(|n| n.id == "community.backend")
            .unwrap();
        let frontend = graph
            .nodes
            .iter()
            .position(|n| n.id == "community.frontend")
            .unwrap();
        assert_eq!(
            click(layout.nodes[fold.glyph_for[backend].unwrap()].center()),
            Some(Selection::Node(backend))
        );
        for (from, to) in [(backend, frontend), (frontend, backend)] {
            let index = fold
                .bundles
                .iter()
                .position(|b| (b.from, b.to) == (from, to))
                .unwrap();
            assert_eq!(
                click(layout.routes[index].midpoint),
                Some(Selection::Bundle(from, to))
            );
        }
    }

    #[test]
    fn empty_graphs_and_bad_endpoints_do_not_panic() {
        let empty = Layout::new(&graph(&[], &[])).unwrap();
        assert!(empty.nodes.is_empty() && empty.routes.is_empty());
        let invalid = graph(&["a"], &[(0, 2)]);
        assert!(Layout::new(&invalid).err().unwrap().contains("edge.0"));
    }
}
