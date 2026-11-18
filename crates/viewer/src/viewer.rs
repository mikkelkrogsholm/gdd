use std::sync::{Arc, mpsc};
use std::thread;

use eframe::egui::{self, Button, Color32, Key, Modifiers, RichText, Ui};

use crate::{
    canvas::Layout,
    model::{Filters, Graph, HumanConcept, Lens, Visible},
};

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
    filters: Filters,
    visible: Visible,
    zoom_percent: u16,
    reveal_selected: bool,
    filename: String,
    selected: Option<usize>,
}

impl Viewer {
    pub fn new(graph: Graph, filename: String, context: egui::Context) -> Self {
        let graph = Arc::new(graph);
        let layout = LayoutState::start(Arc::clone(&graph), context);
        let filters = Filters::default();
        Self {
            visible: graph.visible(&filters),
            filters,
            graph,
            layout,
            zoom_percent: 100,
            reveal_selected: false,
            filename,
            selected: None,
        }
    }

    fn refresh(&mut self) {
        self.visible = self.graph.visible(&self.filters);
    }

    fn navigate(&mut self, index: usize, lens: Lens) {
        self.selected = Some(index);
        if index < self.graph.shown_nodes {
            self.filters = Filters {
                lens,
                ..Default::default()
            };
            self.refresh();
            self.reveal_selected = true;
        }
    }

    fn empty_message(&self) -> String {
        if self.filters.lens != Lens::All && self.graph.perspectives.is_err() {
            "Product perspectives unavailable for this model.".into()
        } else if self.filters.narrowed() {
            "No nodes match the current filters.".into()
        } else if self.filters.lens == Lens::All {
            "This view has no nodes.".into()
        } else {
            format!(
                "No {} concepts in this scope.",
                self.filters.lens.label().to_lowercase()
            )
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
                    let old_lens = self.filters.lens;
                    egui::ComboBox::from_label("Perspective")
                        .selected_text(self.filters.lens.label())
                        .show_ui(ui, |ui| {
                            for lens in [Lens::All, Lens::Journey, Lens::Components, Lens::Data] {
                                ui.selectable_value(&mut self.filters.lens, lens, lens.label());
                            }
                        });
                    if self.filters.lens != old_lens {
                        self.refresh();
                        self.reveal_selected = true;
                    }
                    if matches!(self.layout, LayoutState::Ready(_))
                        && !self.visible.nodes.is_empty()
                    {
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
                ui.horizontal_wrapped(|ui| {
                    let choices = &self.graph.choices;
                    let kind =
                        filter_choice(ui, "Kind", &mut self.filters.kind, choices.kinds.iter());
                    let relation = filter_choice(
                        ui,
                        "Relation",
                        &mut self.filters.relation,
                        choices.relations.iter(),
                    );
                    let community = filter_choice(
                        ui,
                        "Community",
                        &mut self.filters.community,
                        choices.communities.keys(),
                    );
                    if ui.button("Reset filters").clicked() {
                        self.filters = Filters::default();
                        self.refresh();
                    } else if kind || relation || community {
                        self.refresh();
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
                if self.visible.nodes.is_empty() {
                    ui.centered_and_justified(|ui| ui.label(self.empty_message()));
                    return;
                }
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
                    &self.visible,
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
        ui.heading("Nodes");
        let search_label = ui.label("Search graph");
        let focus_search = ui.input_mut(|input| input.consume_key(Modifiers::COMMAND, Key::F));
        let search = ui
            .add_sized(
                [ui.available_width(), 28.0],
                egui::TextEdit::singleline(&mut self.filters.search)
                    .hint_text("Node or edge text")
                    .background_color(Color32::from_rgb(240, 245, 251))
                    .margin(egui::Margin::symmetric(8, 6)),
            )
            .labelled_by(search_label.id);
        if focus_search {
            search.request_focus();
        }
        let choose_first = search.lost_focus() && ui.input(|input| input.key_pressed(Key::Enter));
        if search.changed() {
            self.refresh();
        }
        ui.label(format!(
            "{} of {} nodes · {} of {} edges",
            self.visible.nodes.len(),
            self.graph.nodes.len(),
            self.visible.edges.len(),
            self.graph.edges.len()
        ));

        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                if choose_first {
                    let query = self.filters.search.trim();
                    if let Some(&index) = self
                        .visible
                        .nodes
                        .iter()
                        .find(|&&i| self.graph.nodes[i].id.eq_ignore_ascii_case(query))
                        .or_else(|| self.visible.nodes.first())
                    {
                        self.selected = Some(index);
                        self.reveal_selected = true;
                    }
                }
                for &index in &self.visible.nodes {
                    let node = &self.graph.nodes[index];
                    let row = ui.add_sized(
                        [ui.available_width(), 30.0],
                        Button::selectable(self.selected == Some(index), &node.label).truncate(),
                    );
                    if row.on_hover_text(&node.id).clicked() {
                        self.selected = Some(index);
                        self.reveal_selected = true;
                    }
                }
                let graph = Arc::clone(&self.graph);
                match &graph.perspectives {
                    Ok(projection) if !projection.gaps.is_empty() => {
                        ui.separator();
                        ui.heading(format!("Advisory gaps ({})", projection.gaps.len()));
                        ui.label("These gaps describe the sketch. They never establish readiness.");
                        for (index, message) in &projection.gaps {
                            let node = &graph.nodes[*index];
                            if ui.link(format!("{} · {}", node.id, message)).clicked() {
                                self.navigate(*index, Lens::All);
                                ui.ctx().request_repaint();
                            }
                        }
                    }
                    Err(error) => {
                        ui.collapsing("Product perspectives unavailable", |ui| {
                            ui.label(error);
                        });
                    }
                    _ => {}
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
        let graph = Arc::clone(&self.graph);
        let node = &graph.nodes[index];
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
                if index >= graph.shown_nodes {
                    ui.label("Outside the saved view scope");
                } else if self.visible.nodes.binary_search(&index).is_err() {
                    ui.label("Hidden by current filters");
                }
                if let Ok(projection) = &graph.perspectives {
                    if projection.has_product_extension
                        && let Some(human) = &node.human
                    {
                        match human {
                            HumanConcept::Step { action, response } => {
                                ui.strong("User action");
                                ui.label(
                                    action.as_deref().unwrap_or("Not declared (advisory gap)"),
                                );
                                ui.strong("System response");
                                ui.label(
                                    response.as_deref().unwrap_or("Not declared (advisory gap)"),
                                );
                            }
                            HumanConcept::Actor { goal } => {
                                ui.strong("Goal");
                                ui.label(goal.as_deref().unwrap_or("Not declared"));
                            }
                        }
                    }
                    let related: Vec<_> = Lens::PRODUCT
                        .into_iter()
                        .filter(|&lens| projection.includes(lens, index))
                        .collect();
                    if !related.is_empty() {
                        ui.strong("Related perspectives");
                        ui.horizontal_wrapped(|ui| {
                            for lens in related {
                                if ui.link(lens.label()).clicked() {
                                    self.navigate(index, lens);
                                    ui.ctx().request_repaint();
                                }
                            }
                        });
                    }
                }
                if let Some(data) = &graph.artifact {
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

                let mut connections = graph
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
                            let neighbor_node = &graph.nodes[neighbor];
                            if ui
                                .link(&neighbor_node.label)
                                .on_hover_text(&neighbor_node.id)
                                .clicked()
                            {
                                let in_lens = self.filters.lens == Lens::All
                                    || graph
                                        .perspectives
                                        .as_ref()
                                        .is_ok_and(|p| p.includes(self.filters.lens, neighbor));
                                if in_lens && self.visible.nodes.binary_search(&neighbor).is_err() {
                                    self.navigate(neighbor, self.filters.lens);
                                } else {
                                    self.selected = Some(neighbor);
                                    self.reveal_selected = true;
                                }
                                ui.ctx().request_repaint();
                            }
                        });
                    });
                }
            });
    }
}

fn filter_choice<'a>(
    ui: &mut Ui,
    label: &str,
    selected: &mut String,
    choices: impl Iterator<Item = &'a String>,
) -> bool {
    let mut changed = false;
    egui::ComboBox::from_label(label)
        .width(140.0)
        .truncate()
        .selected_text(if selected.is_empty() {
            "All".to_owned()
        } else {
            selected.clone()
        })
        .show_ui(ui, |ui| {
            changed |= ui
                .selectable_value(selected, String::new(), "All")
                .changed();
            for value in choices {
                if ui.selectable_label(selected == value, value).clicked() {
                    selected.clone_from(value);
                    changed = true;
                }
            }
        });
    changed
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
            filters: Filters::default(),
            visible: graph.visible(&Filters::default()),
            graph: Arc::clone(&graph),
            layout: LayoutState::Pending(receiver),
            zoom_percent: 100,
            reveal_selected: false,
            filename: "fragmented.gdd.yaml".into(),
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
        assert_eq!(viewer.filters.search, "entity.note");
        assert_eq!(viewer.selected, Some(1));
        assert!(!viewer.reveal_selected);
    }

    #[test]
    fn gap_navigation_reveals_only_in_scope_targets_and_preserves_pending_layout() {
        let path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/product/partial.gdd.json");
        let output = tempfile::tempdir().unwrap();
        let saved = output.path().join("partial.gddview");
        gdd::view::render_view(&path, &saved, &["step.reserve".into()]).unwrap();
        let graph = Arc::new(Graph::load(&saved).unwrap());
        let gaps = &graph.perspectives.as_ref().unwrap().gaps;
        assert_eq!(gaps.len(), 5);
        let step = gaps
            .iter()
            .find(|(i, _)| graph.nodes[*i].id == "step.reserve")
            .unwrap()
            .0;
        let entity = gaps
            .iter()
            .find(|(i, _)| graph.nodes[*i].id == "entity.reservation")
            .unwrap()
            .0;

        let (_sender, receiver) = mpsc::channel();
        let filters = Filters {
            lens: Lens::Components,
            kind: "component".into(),
            relation: "owns".into(),
            community: "absent".into(),
            search: "missing".into(),
        };
        let mut viewer = Viewer {
            visible: graph.visible(&filters),
            graph,
            filters,
            layout: LayoutState::Pending(receiver),
            zoom_percent: 100,
            reveal_selected: false,
            filename: "partial.gddview".into(),
            selected: None,
        };
        viewer.navigate(entity, Lens::All);
        assert_eq!(viewer.selected, Some(entity));
        assert_eq!(viewer.filters.lens, Lens::Components);
        assert_eq!(viewer.filters.search, "missing");
        assert_eq!(viewer.filters.community, "absent");
        assert_eq!(viewer.filters.kind, "component");
        assert_eq!(viewer.filters.relation, "owns");
        assert!(viewer.visible.nodes.is_empty() && !viewer.reveal_selected);
        viewer.navigate(step, Lens::All);
        assert_eq!(viewer.filters.lens, Lens::All);
        assert_eq!(viewer.selected, Some(step));
        assert!(!viewer.filters.narrowed());
        assert_eq!(viewer.visible.nodes, [step]);
        assert!(viewer.reveal_selected);
        viewer.navigate(step, Lens::Journey);
        assert_eq!(viewer.filters.lens, Lens::Journey);
        assert_eq!(viewer.selected, Some(step));
        assert_eq!(viewer.visible.nodes, [step]);
        assert!(matches!(viewer.layout, LayoutState::Pending(_)));
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
