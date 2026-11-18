use std::sync::{Arc, mpsc};
use std::thread;

use eframe::egui::{self, Button, Color32, Key, Modifiers, RichText, Ui};

use crate::{canvas::Layout, model::Graph};

enum LayoutState {
    Pending(mpsc::Receiver<Result<Layout, String>>),
    Ready(Layout),
    Failed(String),
}

impl LayoutState {
    fn start(graph: Arc<Graph>, context: egui::Context) -> Self {
        let (sender, receiver) = mpsc::channel();
        match thread::Builder::new()
            .name("graph-layout".into())
            .spawn(move || {
                let result = std::panic::catch_unwind(|| Layout::new(&graph))
                    .unwrap_or_else(|_| Err("The layout worker panicked.".into()));
                if sender.send(result).is_ok() {
                    context.request_repaint();
                }
            }) {
            Ok(_) => Self::Pending(receiver),
            Err(error) => Self::Failed(format!("Could not start the layout worker: {error}")),
        }
    }

    fn update(&mut self) {
        if let Self::Pending(receiver) = self {
            *self = match receiver.try_recv() {
                Ok(Ok(layout)) => Self::Ready(layout),
                Ok(Err(error)) => Self::Failed(error),
                Err(mpsc::TryRecvError::Disconnected) => {
                    Self::Failed("The layout worker stopped before returning a result.".into())
                }
                Err(mpsc::TryRecvError::Empty) => return,
            };
        }
    }
}

pub struct Viewer {
    graph: Arc<Graph>,
    layout: LayoutState,
    zoom_percent: u16,
    reveal_selected: bool,
    filename: String,
    search: String,
    selected: Option<usize>,
}

impl Viewer {
    pub fn new(graph: Graph, filename: String, context: egui::Context) -> Self {
        let graph = Arc::new(graph);
        let layout = LayoutState::start(Arc::clone(&graph), context);
        Self {
            graph,
            layout,
            zoom_percent: 100,
            reveal_selected: false,
            filename,
            search: String::new(),
            selected: None,
        }
    }

    fn show(&mut self, ui: &mut Ui) {
        self.layout.update();
        ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
        let white = Color32::WHITE;
        egui::Panel::top("model")
            .frame(egui::Frame::NONE.fill(white).inner_margin(16))
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.heading(RichText::new(&self.graph.title).size(24.0).strong());
                    ui.label(RichText::new(&self.filename).weak());
                    if matches!(self.layout, LayoutState::Ready(_)) && self.graph.shown_nodes > 0 {
                        egui::ComboBox::from_label("Graph zoom")
                            .selected_text(format!("{}%", self.zoom_percent))
                            .width(70.0)
                            .show_ui(ui, |ui| {
                                for percent in [50, 75, 100, 125, 150, 200] {
                                    if ui
                                        .selectable_value(
                                            &mut self.zoom_percent,
                                            percent,
                                            format!("{percent}%"),
                                        )
                                        .changed()
                                    {
                                        self.reveal_selected = true;
                                    }
                                }
                            });
                    }
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
                let layout = match &self.layout {
                    LayoutState::Ready(layout) => layout,
                    LayoutState::Pending(_) => {
                        ui.centered_and_justified(|ui| ui.label("Calculating graph layout…"));
                        return;
                    }
                    LayoutState::Failed(error) => {
                        ui.centered_and_justified(|ui| {
                            ui.label(format!("Cannot lay out this graph: {error}"))
                        });
                        return;
                    }
                };
                let zoom = f32::from(self.zoom_percent) / 100.0;
                if let Some(index) = layout.show(
                    ui,
                    &self.graph,
                    self.selected,
                    zoom,
                    std::mem::take(&mut self.reveal_selected),
                ) {
                    self.selected = Some(index);
                    ui.ctx().request_repaint();
                }
            });
    }

    fn show_nodes(&mut self, ui: &mut Ui) {
        if self.graph.shown_nodes == self.graph.nodes.len() {
            ui.heading(format!("Nodes ({})", self.graph.shown_nodes));
        } else {
            ui.heading(format!(
                "Nodes ({} of {})",
                self.graph.shown_nodes,
                self.graph.nodes.len()
            ));
        }
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
                for (index, node) in self.graph.nodes[..self.graph.shown_nodes]
                    .iter()
                    .enumerate()
                {
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
                        self.reveal_selected = true;
                    }
                    found = true;
                }
                if !found && self.graph.shown_nodes > 0 {
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
                if let Some(data) = &self.graph.artifact {
                    if index >= self.graph.shown_nodes {
                        ui.label("Outside the saved view scope");
                    }
                    let origin = &data["locations"][&node.id];
                    ui.label(format!(
                        "{}{}",
                        origin[0].as_str().unwrap(),
                        origin[1].as_str().unwrap()
                    ));
                    ui.collapsing("Canonical node record", |ui| {
                        ui.monospace(format!("{:#}", data["model"]["nodes"][&node.id]));
                    });
                }
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
                                self.reveal_selected = true;
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

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn keyboard_selection_during_layout_is_revealed_when_the_drawing_arrives() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/fragmented.gdd.yaml");
        let graph = Arc::new(Graph::load(&path).unwrap());
        let (sender, receiver) = mpsc::channel();
        let mut viewer = Viewer {
            graph: Arc::clone(&graph),
            layout: LayoutState::Pending(receiver),
            zoom_percent: 100,
            reveal_selected: false,
            filename: "fragmented.gdd.yaml".into(),
            search: String::new(),
            selected: Some(0),
        };
        let context = egui::Context::default();
        let frame = |viewer: &mut Viewer, events| {
            let input = egui::RawInput {
                events,
                ..Default::default()
            };
            context
                .run_ui(input, |ui| viewer.show(ui))
                .textures_delta
                .clear();
        };
        let press = |key, modifiers| egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        };
        frame(&mut viewer, vec![press(Key::F, Modifiers::COMMAND)]);
        frame(&mut viewer, vec![egui::Event::Text("entity.note".into())]);
        frame(&mut viewer, vec![press(Key::Enter, Modifiers::NONE)]);
        assert!(matches!(viewer.layout, LayoutState::Pending(_)));
        assert_eq!(viewer.selected, Some(1));
        assert!(viewer.reveal_selected);

        sender.send(Layout::new(&graph)).unwrap();
        frame(&mut viewer, Vec::new());
        assert!(matches!(viewer.layout, LayoutState::Ready(_)));
        assert_eq!(viewer.search, "entity.note");
        assert_eq!(viewer.selected, Some(1));
        assert!(!viewer.reveal_selected);
    }

    #[test]
    fn layout_failure_and_disconnect_leave_the_pending_state() {
        let (sender, receiver) = mpsc::channel();
        let mut state = LayoutState::Pending(receiver);
        sender.send(Err("Invalid route".into())).unwrap();
        state.update();
        assert!(matches!(state, LayoutState::Failed(error) if error == "Invalid route"));

        let (sender, receiver) = mpsc::channel();
        let mut state = LayoutState::Pending(receiver);
        drop(sender);
        state.update();
        assert!(matches!(state, LayoutState::Failed(_)));
    }
}
