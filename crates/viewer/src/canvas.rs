mod communities;
mod drag;
mod groups;
pub use communities::{CommunityGeometry, community_color};
use drag::NodeDrag;
pub use drag::RouteWorker;
use std::sync::Arc;

use eframe::egui::{
    self, FontId, Painter, Pos2, Rect, Sense, Stroke, StrokeKind, TextFormat, Vec2,
    emath::TSTransform,
};
use graphlay::{GraphBuilder, LayoutOptions, Size};

use crate::model::{FoldProjection, Graph, Selection, Visible};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Zoom {
    Percent(u16),
    Fit,
}

#[derive(Clone, Copy)]
pub struct Camera {
    pub zoom: Zoom,
    frame: Option<CameraFrame>,
    reset_scroll: bool,
}

#[derive(Clone, Copy)]
struct CameraFrame {
    scroll_id: egui::Id,
    bounds: Rect,
    viewport_size: Vec2,
    center: Pos2,
    scale: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            zoom: Zoom::Percent(100),
            frame: None,
            reset_scroll: false,
        }
    }
}

impl Camera {
    pub fn reset(&mut self, zoom: Zoom) {
        self.zoom = zoom;
        self.reset_scroll = true;
    }

    pub fn label(&self) -> String {
        match self.zoom {
            Zoom::Percent(percent) => format!("{percent}%"),
            Zoom::Fit => self.frame.map_or_else(
                || "Fit".into(),
                |frame| format!("Fit ({:.1}%)", frame.scale * 100.0),
            ),
        }
    }
}

pub struct Layout {
    nodes: Vec<Rect>,
    routes: Vec<Route>,
    bounds: Rect,
    topology: Arc<graphlay::Graph>,
    drag: Option<NodeDrag>,
    revision: u64,
    needs_routing: bool,
    route_error: Option<String>,
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
    pub show_areas: bool,
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
        let records: Vec<_> = nodes.collect();
        let ids: Vec<_> = records
            .iter()
            .map(|&i| {
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
        let mut topology = builder.build();
        let drawing = groups::layout(
            graph,
            &records,
            &mut topology,
            &layout_options(edge_ids.len()),
        )
        .map_err(|error| error.to_string())?;
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
        let routes = drawing
            .edges
            .iter()
            .zip(edge_ids)
            .map(|(edge, index)| {
                Route::from_points(edge.points.iter().copied().map(position).collect())
                    .ok_or_else(|| format!("Edge {} has no drawable route.", graph.edges[index].id))
            })
            .collect::<Result<_, _>>()?;
        Ok(Self {
            nodes,
            routes,
            bounds: Rect::from_min_max(position(drawing.bounds.min), position(drawing.bounds.max)),
            topology: Arc::new(topology),
            drag: None,
            revision: 0,
            needs_routing: false,
            route_error: None,
        })
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        scene: &Scene<'_>,
        camera: &mut Camera,
        reveal: bool,
        geometry: &mut Option<CommunityGeometry>,
    ) -> Option<Selection> {
        if scene.show_areas && geometry.is_none() {
            *geometry = Some(self.community_geometry(scene.visible, scene.folded));
        }
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
        let bounds = geometry
            .as_ref()
            .filter(|_| scene.show_areas)
            .map_or(bounds, |areas| bounds.union(areas.bounds));
        // Use the same world-to-screen mapping until the primary button is released.
        let bounds = self.drag.as_ref().map_or(bounds, |drag| drag.bounds);
        if camera.reset_scroll {
            if let Some(frame) = camera.frame.take() {
                egui::scroll_area::State::default().store(ui.ctx(), frame.scroll_id);
            }
            camera.reset_scroll = false;
        }
        // A released graph stays where the user placed it.
        let kinetic = &mut ui.spacing_mut().scroll.kinetic;
        kinetic.decay_time = 0.0;
        kinetic.rubber_band = false;
        let mut scroll = egui::ScrollArea::both()
            .id_salt("graph-camera")
            .content_margin(0)
            .min_scrolled_width(0.0)
            .min_scrolled_height(0.0)
            .auto_shrink([false; 2])
            .animated(false)
            .scroll_source(if self.drag.is_some() {
                egui::scroll_area::ScrollSource::NONE
            } else {
                egui::scroll_area::ScrollSource::ALL
            })
            .on_drag_cursor(egui::CursorIcon::Grabbing);
        if camera.zoom == Zoom::Fit {
            scroll = scroll
                .scroll_offset(Vec2::ZERO)
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden);
        }
        let output = scroll.show_viewport(ui, |ui, viewport| {
            let margin = Vec2::splat(20.0).min(viewport.size() * 0.2);
            let scale = match camera.zoom {
                Zoom::Percent(percent) => f32::from(percent) / 100.0,
                Zoom::Fit => {
                    let available = (viewport.size() - margin * 2.0).max(Vec2::splat(1.0));
                    (available / bounds.size().max(Vec2::splat(1.0)))
                        .min_elem()
                        .min(2.0)
                }
            };
            let size = (bounds.size() * scale + margin * 2.0).max(viewport.size());
            let (area, _) = ui.allocate_exact_size(size, Sense::hover());
            let transform = TSTransform::new(
                area.center().to_vec2() - bounds.center().to_vec2() * scale,
                scale,
            );
            if camera.zoom != Zoom::Fit {
                if reveal && let Some(index) = scene.selected_glyph() {
                    ui.scroll_to_rect((transform * self.nodes[index]).expand(12.0), None);
                } else if let Some(prior) = camera.frame
                    && (prior.bounds != bounds
                        || prior.viewport_size != viewport.size()
                        || prior.scale != scale)
                {
                    let target = Rect::from_center_size(transform * prior.center, Vec2::ZERO);
                    ui.scroll_to_rect(target, Some(egui::Align::Center));
                }
            }
            if self.update_drag(ui, scene, scale, bounds) {
                *geometry = scene
                    .show_areas
                    .then(|| self.community_geometry(scene.visible, scene.folded));
            }
            let areas = geometry.as_ref().filter(|_| scene.show_areas);
            (self.draw(ui, scene, transform, area, areas), size, scale)
        });
        let (clicked, size, scale) = output.inner;
        if camera.zoom == Zoom::Fit && camera.frame.is_none_or(|prior| prior.scale != scale) {
            // The toolbar displays the scale measured by this canvas pass.
            ui.ctx().request_repaint();
        }
        let viewport_size = output.inner_rect.size();
        // Store the world point at the viewport center after egui applies input.
        let center = bounds.center()
            + (output.state.clamped_offset() + (viewport_size - size) * 0.5) / scale;
        camera.frame = Some(CameraFrame {
            scroll_id: output.id,
            bounds,
            viewport_size,
            center,
            scale,
        });
        clicked
    }

    fn draw(
        &self,
        ui: &mut egui::Ui,
        scene: &Scene<'_>,
        transform: TSTransform,
        area: Rect,
        areas: Option<&CommunityGeometry>,
    ) -> Option<Selection> {
        let graph = scene.graph;
        let visible = scene.visible;
        let zoom = transform.scaling;
        let accent = ui.visuals().selection.stroke.color;
        let selected_fill = ui.visuals().selection.bg_fill;
        let node_fill = ui.visuals().panel_fill;
        let label_color = ui.visuals().strong_text_color();
        let outline = ui.visuals().widgets.hovered.bg_stroke.color;
        let mut clicked = None;
        let canvas = ui.interact(area, ui.scope_id().with("canvas"), Sense::click());
        let pointer = canvas.interact_pointer_pos().filter(|_| canvas.clicked());
        let mut best_distance = 7.0;
        let selected_glyph = scene.selected_glyph();
        if let Some(areas) = areas {
            areas.paint(ui, self, scene, transform);
        }
        let n_routes = scene
            .folded
            .map_or(visible.edges.len(), |f| f.bundles.len());
        for highlighted in [false, true] {
            let stroke = if highlighted {
                Stroke::new((2.0 * zoom).clamp(0.7, 2.0), accent)
            } else {
                Stroke::new((1.2 * zoom).clamp(0.15, 1.2), outline)
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
                if scene.folded.is_some() && records.len() > 1 && zoom >= 0.4 {
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
                    ui.painter().rect_filled(rect, 4, selected_fill);
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
                    Sense::click_and_drag(),
                )
                .on_hover_cursor(egui::CursorIcon::Grab)
                .on_hover_text(&node.id);
            if response.dragged() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
            }
            let active = selected_glyph == Some(slot);
            let fill = if active || response.hovered() {
                selected_fill
            } else if rect.height() < 6.0 {
                outline
            } else {
                node_fill
            };
            let border = if active || response.hovered() {
                Stroke::new(2.0, accent)
            } else {
                Stroke::new(1.0, outline)
            };
            let painter = ui.painter();
            painter.rect(rect, 6, fill, border, StrokeKind::Inside);
            if response.clicked() {
                clicked = Some(Selection::Node(index));
            }
            if zoom < 0.4 {
                continue;
            }
            let mut job = egui::text::LayoutJob::single_section(
                node.label.clone(),
                TextFormat::simple(FontId::proportional((13.0 * zoom).max(10.0)), label_color),
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
            painter.galley(center - label.size() * 0.5, label, label_color);
        }
        clicked
    }
}

const LARGE_EDGE_COUNT: usize = 1_000;

fn layout_options(edge_count: usize) -> LayoutOptions {
    let mut options = LayoutOptions::layered_default();
    // Straightening chains can dominate layout time on large projections.
    if edge_count >= LARGE_EDGE_COUNT {
        options.tuning.coords.flatten_room = 0;
    }
    options
}

fn position(point: graphlay::Point) -> Pos2 {
    egui::pos2(point.x as f32, point.y as f32)
}

impl Route {
    fn from_points(points: Vec<Pos2>) -> Option<Self> {
        let (tip, previous) = points.split_last()?;
        let before = previous.iter().rev().find(|p| *p != tip)?;
        Some(Self {
            bounds: Rect::from_points(&points),
            midpoint: route_midpoint(&points),
            arrow_direction: (*tip - *before).normalized(),
            points,
        })
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
        let details = || crate::model::RecordDetails {
            location: gdd::bundle::Location {
                file: "fixture".into(),
                pointer: String::new(),
            },
            references: Vec::new(),
        };
        Graph {
            sources: Default::default(),
            work: None,
            wiki: None,
            input: crate::model::ViewInput::Model(Box::new(crate::model::CapturedModel {
                project: serde_json::json!({"title": "Layout fixture"}),
                questions: None,
                snapshot: None,
                boundaries: None,
            })),
            shown_nodes: ids.len(),
            shown_edges: pairs.len(),
            actions: Default::default(),
            choices: Default::default(),
            perspectives: Err("Layout fixture".into()),
            nodes: ids
                .iter()
                .map(|&id| Node {
                    id: id.into(),
                    label: id.into(),
                    has_label: true,
                    kind: "component".into(),
                    description: id.into(),
                    human: None,
                    inspection: crate::model::NodeInspection::Other,
                    details: details(),
                    search_text: id.into(),
                })
                .collect(),
            edges: pairs
                .iter()
                .enumerate()
                .map(|(i, &(from, to))| Edge {
                    id: format!("edge.{i}"),
                    relation: "links".into(),
                    details: details(),
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
            communities: Vec::new(),
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
                    show_areas: false,
                },
                transform,
                ui.max_rect(),
                None,
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
            show_areas: false,
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
                        clicked = layout.draw(ui, &scene, transform, screen, None);
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
