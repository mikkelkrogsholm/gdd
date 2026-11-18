use eframe::egui::{self, Button, Color32, Key, Modifiers, RichText, Ui};

use crate::{canvas::Layout, model::Graph};

pub struct Viewer {
    graph: Graph,
    layout: Layout,
    filename: String,
    search: String,
    selected: Option<usize>,
}

impl Viewer {
    pub fn new(graph: Graph, filename: String) -> Self {
        let layout = Layout::new(&graph);
        Self {
            graph,
            layout,
            filename,
            search: String::new(),
            selected: None,
        }
    }

    fn show(&mut self, ui: &mut Ui) {
        ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
        let white = Color32::WHITE;
        egui::Panel::top("model")
            .frame(egui::Frame::NONE.fill(white).inner_margin(16))
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.heading(RichText::new(&self.graph.title).size(24.0).strong());
                    ui.label(RichText::new(&self.filename).weak());
                });
            });

        egui::Panel::left("nodes")
            .default_size(226.0)
            .size_range(178.0..=330.0)
            .frame(egui::Frame::NONE.fill(white).inner_margin(14))
            .show(ui, |ui| self.show_nodes(ui));

        egui::Panel::right("details")
            .default_size(318.0)
            .size_range(250.0..=430.0)
            .frame(egui::Frame::NONE.fill(white).inner_margin(16))
            .show(ui, |ui| self.show_details(ui));

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(Color32::from_rgb(244, 247, 251)))
            .show(ui, |ui| {
                if let Some(index) = self.layout.show(ui, &self.graph, self.selected) {
                    self.selected = Some(index);
                    ui.ctx().request_repaint();
                }
            });
    }

    fn show_nodes(&mut self, ui: &mut Ui) {
        ui.heading(format!("Nodes ({})", self.graph.nodes.len()));
        let search_label = ui.label("Search nodes");
        let focus_search = ui.input_mut(|input| input.consume_key(Modifiers::COMMAND, Key::F));
        let search = ui
            .add_sized(
                [ui.available_width(), 28.0],
                egui::TextEdit::singleline(&mut self.search)
                    .hint_text("Label, ID, or kind")
                    .background_color(Color32::from_rgb(240, 245, 251))
                    .margin(egui::Margin::symmetric(8, 6)),
            )
            .labelled_by(search_label.id);
        if focus_search {
            search.request_focus();
        }
        let choose_first = search.lost_focus() && ui.input(|input| input.key_pressed(Key::Enter));
        let query = self.search.trim().to_lowercase();

        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                let mut found = false;
                for (index, node) in self.graph.nodes.iter().enumerate() {
                    if !query.is_empty()
                        && !node.label.to_lowercase().contains(&query)
                        && !node.id.to_lowercase().contains(&query)
                        && !node.kind.to_lowercase().contains(&query)
                    {
                        continue;
                    }
                    let row = ui.add_sized(
                        [ui.available_width(), 30.0],
                        Button::selectable(self.selected == Some(index), &node.label).truncate(),
                    );
                    if row.on_hover_text(&node.id).clicked() || (!found && choose_first) {
                        self.selected = Some(index);
                    }
                    found = true;
                }
                if !found && !self.graph.nodes.is_empty() {
                    ui.label("No matching nodes.");
                }
            });
    }

    fn show_details(&mut self, ui: &mut Ui) {
        ui.set_min_width(ui.available_width());
        let Some(index) = self.selected else {
            ui.heading("Node details");
            ui.label("Select a node to see its details.");
            return;
        };
        let node = &self.graph.nodes[index];
        egui::ScrollArea::vertical()
            .id_salt(index)
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                ui.heading(&node.label);
                if node.label != node.id {
                    ui.label(RichText::new(&node.id).monospace());
                }
                ui.label(RichText::new(&node.kind).monospace().weak());
                ui.label(&node.description);
                ui.add_space(10.0);

                let mut connections = self
                    .graph
                    .edges
                    .iter()
                    .filter(|edge| edge.from == index || edge.to == index)
                    .peekable();
                ui.heading("Connections");
                if connections.peek().is_none() {
                    ui.label("No incident edges.");
                }
                for edge in connections {
                    let (direction, neighbor) = match (edge.from == index, edge.to == index) {
                        (true, true) => ("Self-loop", index),
                        (true, false) => ("Outgoing to", edge.to),
                        (false, _) => ("Incoming from", edge.from),
                    };
                    ui.separator();
                    ui.push_id(&edge.id, |ui| {
                        ui.label(RichText::new(&edge.id).monospace().strong());
                        ui.label(RichText::new(&edge.relation).monospace());
                        ui.horizontal_wrapped(|ui| {
                            ui.label(direction);
                            let neighbor_node = &self.graph.nodes[neighbor];
                            if ui
                                .link(&neighbor_node.label)
                                .on_hover_text(&neighbor_node.id)
                                .clicked()
                            {
                                self.selected = Some(neighbor);
                                ui.ctx().request_repaint();
                            }
                        });
                    });
                }
            });
    }
}

impl eframe::App for Viewer {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        self.show(ui);
    }
}
