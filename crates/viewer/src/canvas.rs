use std::collections::VecDeque;

use eframe::egui::{
    self, Color32, FontId, Painter, Pos2, Rect, Sense, Stroke, StrokeKind, TextFormat, Vec2,
};

use crate::model::{Edge, Graph};

pub struct Layout {
    columns: Vec<Vec<usize>>,
}

impl Layout {
    pub fn new(graph: &Graph) -> Self {
        let mut incoming = vec![0; graph.nodes.len()];
        let mut outgoing = vec![Vec::new(); graph.nodes.len()];
        for edge in &graph.edges {
            if edge.from != edge.to {
                incoming[edge.to] += 1;
                outgoing[edge.from].push(edge.to);
            }
        }
        let mut ready: VecDeque<_> = (0..graph.nodes.len())
            .filter(|&index| incoming[index] == 0)
            .collect();
        let mut layers = vec![0; graph.nodes.len()];
        while let Some(from) = ready.pop_front() {
            for &to in &outgoing[from] {
                layers[to] = layers[to].max(layers[from] + 1);
                incoming[to] -= 1;
                if incoming[to] == 0 {
                    ready.push_back(to);
                }
            }
        }

        // Put nodes left over from cycles in a final column.
        let after_acyclic = (0..graph.nodes.len())
            .filter(|&index| incoming[index] == 0)
            .map(|index| layers[index] + 1)
            .max()
            .unwrap_or(0);
        let mut columns = Vec::<Vec<_>>::new();
        for (index, layer) in layers.into_iter().enumerate() {
            let layer = if incoming[index] == 0 {
                layer
            } else {
                after_acyclic
            };
            if columns.len() <= layer {
                columns.resize_with(layer + 1, Vec::new);
            }
            columns[layer].push(index);
        }
        Self { columns }
    }

    pub fn show(&self, ui: &mut egui::Ui, graph: &Graph, selected: Option<usize>) -> Option<usize> {
        if graph.nodes.is_empty() {
            ui.centered_and_justified(|ui| ui.label("This model has no nodes."));
            return None;
        }

        egui::ScrollArea::both()
            .auto_shrink([false; 2])
            .show(ui, |ui| self.draw(ui, graph, selected))
            .inner
    }

    fn draw(&self, ui: &mut egui::Ui, graph: &Graph, selected: Option<usize>) -> Option<usize> {
        let row_count = self.columns.iter().map(Vec::len).max().unwrap_or(0);
        let size = ui.available_size().max(egui::vec2(
            150.0 * self.columns.len() as f32 + 20.0,
            124.0 * row_count as f32 + 20.0,
        ));
        let (area, _) = ui.allocate_exact_size(size, Sense::hover());
        let column_width = (area.width() - 20.0) / self.columns.len() as f32;
        let row_height = ((area.height() - 20.0) / row_count as f32).min(154.0);
        let node_size = egui::vec2((column_width - 22.0).min(164.0), 42.0);

        let mut rectangles = vec![Rect::NOTHING; graph.nodes.len()];
        for (column, nodes) in self.columns.iter().enumerate() {
            let x = area.left() + 10.0 + column_width * (column as f32 + 0.5);
            let stagger = if nodes.len() == 1 && self.columns.len() > 1 {
                if column % 2 == 0 { 48.0 } else { -48.0 }
            } else {
                0.0
            };
            let first_y = area.center().y + stagger - row_height * (nodes.len() as f32 - 1.0) * 0.5;
            for (row, &index) in nodes.iter().enumerate() {
                let center = egui::pos2(x, first_y + row_height * row as f32);
                rectangles[index] = Rect::from_center_size(center, node_size);
            }
        }
        let painter = ui.painter();
        let accent = Color32::from_rgb(32, 91, 165);
        for highlighted in [false, true] {
            let stroke = if highlighted {
                Stroke::new(2.0, accent)
            } else {
                Stroke::new(1.2, Color32::from_rgb(139, 153, 171))
            };
            for edge in &graph.edges {
                let incident = selected == Some(edge.from) || selected == Some(edge.to);
                if incident == highlighted {
                    draw_edge(painter, &rectangles, edge, stroke);
                }
            }
        }

        let mut clicked = None;
        for (index, rect) in rectangles.into_iter().enumerate() {
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
            painter.rect(rect, 7, fill, border, StrokeKind::Inside);
            let mut job = egui::text::LayoutJob::single_section(
                node.label.clone(),
                TextFormat::simple(FontId::proportional(13.0), Color32::from_rgb(20, 39, 68)),
            );
            job.wrap.max_width = rect.width() - 14.0;
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

fn draw_edge(painter: &Painter, rectangles: &[Rect], edge: &Edge, stroke: Stroke) {
    let from = rectangles[edge.from];
    let to = rectangles[edge.to];
    if edge.from == edge.to {
        let start = from.right_center() - egui::vec2(0.0, 10.0);
        let end = from.right_center() + egui::vec2(0.0, 10.0);
        let outside = egui::vec2(12.0, 0.0);
        painter.add(egui::Shape::line(
            vec![start, start + outside, end + outside, end],
            stroke,
        ));
        arrowhead(painter, end, -Vec2::X, stroke);
    } else {
        let direction = to.center() - from.center();
        let start = border_point(from, direction);
        let end = border_point(to, -direction);
        painter.line_segment([start, end], stroke);
        arrowhead(painter, end, direction.normalized(), stroke);
    }
}

fn border_point(rect: Rect, direction: Vec2) -> Pos2 {
    let width_ratio = direction.x.abs() / (rect.width() * 0.5);
    let height_ratio = direction.y.abs() / (rect.height() * 0.5);
    rect.center() + direction / width_ratio.max(height_ratio)
}

fn arrowhead(painter: &Painter, tip: Pos2, direction: Vec2, stroke: Stroke) {
    let back = tip - direction * 8.0;
    let side = egui::vec2(-direction.y, direction.x) * 4.0;
    painter.add(egui::Shape::convex_polygon(
        vec![tip, back + side, back - side],
        stroke.color,
        Stroke::NONE,
    ));
}
