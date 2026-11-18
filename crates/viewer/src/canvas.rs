use eframe::egui::{
    self, Color32, FontId, Painter, Pos2, Rect, Sense, Stroke, StrokeKind, TextFormat, Vec2,
    emath::TSTransform,
};
use graphlay::{GraphBuilder, LayoutOptions, Size};

use crate::model::Graph;

pub struct Layout {
    nodes: Vec<Rect>,
    routes: Vec<Route>,
    bounds: Rect,
}

struct Route {
    points: Vec<Pos2>,
    bounds: Rect,
    arrow_direction: Vec2,
}

impl Layout {
    pub fn new(graph: &Graph) -> Result<Self, String> {
        // Both graphlay result vectors retain insertion order, including
        // routes whose edges were reversed internally for layer assignment.
        let mut builder = GraphBuilder::new();
        let ids: Vec<_> = graph
            .nodes
            .iter()
            .map(|node| {
                let width = (24.0 + 7.0 * node.label.chars().count() as f64).clamp(132.0, 236.0);
                builder.add_box(Size::new(width, 44.0))
            })
            .collect();
        for edge in &graph.edges {
            let (Some(&from), Some(&to)) = (ids.get(edge.from), ids.get(edge.to)) else {
                return Err(format!("Edge {} refers to an unknown node.", edge.id));
            };
            builder.connect(from, to);
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
                    graph.edges[index].id
                ));
            };
            routes.push(Route {
                bounds: Rect::from_points(&points),
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
        graph: &Graph,
        selected: Option<usize>,
        zoom: f32,
        reveal: bool,
    ) -> Option<usize> {
        if self.nodes.is_empty() {
            ui.centered_and_justified(|ui| ui.label("This model has no nodes."));
            return None;
        }
        egui::ScrollArea::both()
            .auto_shrink([false; 2])
            .show_viewport(ui, |ui, viewport| {
                let size = (self.bounds.size() * zoom + Vec2::splat(40.0)).max(viewport.size());
                let (area, _) = ui.allocate_exact_size(size, Sense::hover());
                let transform = TSTransform::new(
                    area.center().to_vec2() - self.bounds.center().to_vec2() * zoom,
                    zoom,
                );
                if reveal && let Some(index) = selected {
                    ui.scroll_to_rect((transform * self.nodes[index]).expand(12.0), None);
                }
                self.draw(ui, graph, selected, transform)
            })
            .inner
    }

    fn draw(
        &self,
        ui: &mut egui::Ui,
        graph: &Graph,
        selected: Option<usize>,
        transform: TSTransform,
    ) -> Option<usize> {
        let zoom = transform.scaling;
        let accent = Color32::from_rgb(32, 91, 165);
        for highlighted in [false, true] {
            let stroke = if highlighted {
                Stroke::new(2.0, accent)
            } else {
                Stroke::new(1.2, Color32::from_rgb(139, 153, 171))
            };
            for (edge, route) in graph.edges.iter().zip(&self.routes) {
                let incident = selected == Some(edge.from) || selected == Some(edge.to);
                if incident == highlighted
                    && ui.is_rect_visible((transform * route.bounds).expand(10.0 * zoom))
                {
                    let points: Vec<_> = route.points.iter().map(|&p| transform * p).collect();
                    let tip = points[points.len() - 1];
                    ui.painter().add(egui::Shape::line(points, stroke));
                    arrowhead(ui.painter(), tip, route.arrow_direction, stroke, zoom);
                }
            }
        }

        let mut clicked = None;
        for (index, &node_rect) in self.nodes.iter().enumerate() {
            let rect = transform * node_rect;
            if !ui.is_rect_visible(rect) {
                continue;
            }
            let node = &graph.nodes[index];
            // The node list provides the keyboard and assistive technology controls.
            let response = ui.interact(
                rect,
                ui.scope_id().with(("canvas-node", index)),
                Sense::CLICK,
            );
            let response = response.on_hover_text(&node.id);
            let active = selected == Some(index);
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
            painter.galley(rect.center() - label.size() * 0.5, label, Color32::BLACK);
            if response.clicked() {
                clicked = Some(index);
            }
        }
        clicked
    }
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
            nodes: ids
                .iter()
                .map(|&id| Node {
                    id: id.into(),
                    label: id.into(),
                    kind: "component".into(),
                    description: id.into(),
                })
                .collect(),
            edges: pairs
                .iter()
                .enumerate()
                .map(|(i, &(from, to))| Edge {
                    id: format!("edge.{i}"),
                    relation: "links".into(),
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
    fn empty_graphs_and_bad_endpoints_do_not_panic() {
        let empty = Layout::new(&graph(&[], &[])).unwrap();
        assert!(empty.nodes.is_empty() && empty.routes.is_empty());
        let invalid = graph(&["a"], &[(0, 2)]);
        assert!(Layout::new(&invalid).err().unwrap().contains("edge.0"));
    }
}
