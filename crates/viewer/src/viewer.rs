use std::sync::{Arc, mpsc};
use std::thread;

use eframe::egui::{self, Button, Color32, Key, Modifiers, RichText, Ui};

use crate::{
    canvas::{Layout, Scene},
    model::{EdgeBundle, Filters, FoldProjection, Graph, HumanConcept, Lens, Selection, Visible},
};

enum LayoutState {
    Pending(mpsc::Receiver<Result<Layout, String>>),
    Ready(Layout),
    Failed(String),
}

impl LayoutState {
    fn start(graph: Arc<Graph>, fold: Option<Arc<FoldProjection>>, context: egui::Context) -> Self {
        let (sender, receiver) = mpsc::channel();
        match thread::Builder::new()
            .name("graph-layout".into())
            .spawn(move || {
                let result = std::panic::catch_unwind(|| match fold {
                    Some(fold) => Layout::folded(&graph, &fold),
                    None => Layout::new(&graph),
                })
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
    selected: Option<Selection>,
    folded: Option<Arc<FoldProjection>>,
    folded_layout: Option<(Arc<FoldProjection>, LayoutState)>,
}

impl Viewer {
    pub fn new(graph: Graph, filename: String, context: egui::Context) -> Self {
        let graph = Arc::new(graph);
        let layout = LayoutState::start(Arc::clone(&graph), None, context);
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
            folded: None,
            folded_layout: None,
        }
    }

    fn refresh(&mut self) {
        let visible = self.graph.visible(&self.filters);
        if self.visible.nodes != visible.nodes || self.visible.edges != visible.edges {
            self.visible = visible;
            self.set_folded(self.folded.is_some());
        }
    }

    fn set_folded(&mut self, enabled: bool) {
        if !enabled && self.folded.is_none() {
            return;
        }
        let next = enabled.then(|| Arc::new(self.visible.project(&self.graph, true)));
        if let Some(Selection::Bundle(from, to)) = self.selected
            && !next
                .as_ref()
                .is_some_and(|fold| fold.bundles.iter().any(|b| (b.from, b.to) == (from, to)))
        {
            self.selected = self
                .folded
                .as_ref()
                .and_then(|fold| fold.bundles.iter().find(|b| (b.from, b.to) == (from, to)))
                .map(|b| Selection::Edge(b.edges[0]));
        }
        self.folded = next;
        self.reveal_selected = true;
    }

    fn update_layout(&mut self, context: &egui::Context) {
        self.layout.update();
        if let Some((_, state)) = &mut self.folded_layout {
            state.update();
        }
        if let Some(fold) = &self.folded
            && !fold.glyphs.is_empty()
            && !self.folded_layout.as_ref().is_some_and(|(prior, state)| {
                Arc::ptr_eq(prior, fold) || matches!(state, LayoutState::Pending(_))
            })
        {
            // At most one folded computation runs. While it runs, newer filter
            // changes replace the requested projection; only the latest is laid out.
            let state = LayoutState::start(
                Arc::clone(&self.graph),
                Some(Arc::clone(fold)),
                context.clone(),
            );
            self.folded_layout = Some((Arc::clone(fold), state));
        }
    }

    fn current_layout(&self) -> Option<&LayoutState> {
        match &self.folded {
            None => Some(&self.layout),
            Some(current) => self
                .folded_layout
                .as_ref()
                .filter(|(prior, _)| Arc::ptr_eq(prior, current))
                .map(|(_, state)| state),
        }
    }

    fn navigate(&mut self, index: usize, lens: Lens) {
        self.selected = Some(Selection::Node(index));
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
        self.update_layout(ui.ctx());
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
                    if matches!(self.current_layout(), Some(LayoutState::Ready(_)))
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
                    let mut fold = self.folded.is_some();
                    if ui.checkbox(&mut fold, "Fold communities").changed() {
                        self.set_folded(fold);
                        ui.ctx().request_repaint();
                    }
                    if ui.button("Reset filters").clicked() {
                        self.set_folded(false);
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
                self.update_layout(ui.ctx());
                let reveal = self.reveal_selected;
                let layout = match self.current_layout() {
                    Some(LayoutState::Ready(layout)) => layout,
                    None | Some(LayoutState::Pending(_)) => {
                        ui.centered_and_justified(|ui| ui.label("Calculating graph layout…"));
                        return;
                    }
                    Some(LayoutState::Failed(error)) => {
                        ui.centered_and_justified(|ui| {
                            ui.label(format!("Cannot lay out this graph: {error}"))
                        });
                        return;
                    }
                };
                let zoom = f32::from(self.zoom_percent) / 100.0;
                let scene = Scene {
                    graph: &self.graph,
                    visible: &self.visible,
                    folded: self.folded.as_deref(),
                    selected: self.selected,
                };
                let clicked = layout.show(ui, &scene, zoom, reveal);
                self.reveal_selected = false;
                if let Some(selection) = clicked {
                    self.selected = Some(selection);
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

        if let Some(fold) = &self.folded {
            let internal: usize = fold.glyphs.iter().map(|g| g.internal.len()).sum();
            ui.label(format!(
                "{} figures · {} bundles · {} internal edges",
                fold.glyphs.len(),
                fold.bundles.len(),
                internal
            ));
        }

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
                        self.selected = Some(Selection::Node(index));
                        self.reveal_selected = true;
                    }
                }
                for &index in &self.visible.nodes {
                    let node = &self.graph.nodes[index];
                    let row = ui.add_sized(
                        [ui.available_width(), 30.0],
                        Button::selectable(
                            self.selected == Some(Selection::Node(index)),
                            &node.label,
                        )
                        .truncate(),
                    );
                    if row.on_hover_text(&node.id).clicked() {
                        self.selected = Some(Selection::Node(index));
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
        let index = match self.selected {
            Some(Selection::Node(index)) => index,
            Some(Selection::Edge(index)) => {
                self.show_edge(ui, index);
                return;
            }
            Some(Selection::Bundle(from, to)) => {
                self.show_bundle(ui, from, to);
                return;
            }
            None => {
                ui.heading("Node details");
                ui.label("Select a node or connection to see its details.");
                return;
            }
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
                self.show_folded_node(ui, index);
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
                    .enumerate()
                    .filter(|(_, edge)| edge.from == index || edge.to == index)
                    .peekable();
                ui.heading("Connections");
                if connections.peek().is_none() {
                    ui.label("No incident edges.");
                }
                for (edge_index, edge) in connections {
                    let (direction, neighbor) = match (edge.from == index, edge.to == index) {
                        (true, true) => ("Self-loop", index),
                        (true, false) => ("Outgoing to", edge.to),
                        (false, _) => ("Incoming from", edge.from),
                    };
                    ui.separator();
                    ui.push_id(&edge.id, |ui| {
                        if ui.link(RichText::new(&edge.id).monospace()).clicked() {
                            self.selected = Some(Selection::Edge(edge_index));
                        }
                        ui.label(RichText::new(&edge.relation).monospace());
                        ui.horizontal_wrapped(|ui| {
                            ui.label(direction);
                            let neighbor_node = &graph.nodes[neighbor];
                            if ui
                                .link(&neighbor_node.label)
                                .on_hover_text(&neighbor_node.id)
                                .clicked()
                            {
                                self.follow_node(neighbor);
                                ui.ctx().request_repaint();
                            }
                        });
                    });
                }
            });
    }

    fn follow_node(&mut self, node: usize) {
        let in_lens = self.filters.lens == Lens::All
            || self
                .graph
                .perspectives
                .as_ref()
                .is_ok_and(|p| p.includes(self.filters.lens, node));
        if in_lens && self.visible.nodes.binary_search(&node).is_err() {
            self.navigate(node, self.filters.lens);
        } else {
            self.selected = Some(Selection::Node(node));
            self.reveal_selected = true;
        }
    }

    fn node_link(&mut self, ui: &mut Ui, index: usize) {
        let node = &self.graph.nodes[index];
        if ui.link(&node.label).on_hover_text(&node.id).clicked() {
            self.follow_node(index);
        }
    }

    fn edge_link(&mut self, ui: &mut Ui, index: usize) {
        let edge = &self.graph.edges[index];
        if ui.link(&edge.id).on_hover_text(&edge.relation).clicked() {
            self.selected = Some(Selection::Edge(index));
        }
    }

    fn bundle_name(&self, b: &EdgeBundle) -> String {
        format!(
            "{} → {} ({})",
            self.graph.nodes[b.from].label,
            self.graph.nodes[b.to].label,
            b.edges.len()
        )
    }

    fn show_folded_node(&mut self, ui: &mut Ui, index: usize) {
        let Some(fold) = self.folded.clone() else {
            return;
        };
        let Some(glyph) = fold
            .glyph_for
            .get(index)
            .copied()
            .flatten()
            .map(|i| &fold.glyphs[i])
        else {
            return;
        };
        let graph = Arc::clone(&self.graph);
        let memberships = &graph.choices.memberships[index];
        if !memberships.is_empty() {
            ui.strong(format!("Direct communities ({})", memberships.len()));
            for &community in memberships {
                ui.horizontal_wrapped(|ui| {
                    self.node_link(ui, community);
                    if glyph.node == community && glyph.node != index {
                        ui.weak("Representative");
                    }
                });
            }
        }
        if glyph.node != index {
            return;
        }
        if glyph.members.len() > 1 {
            ui.strong(format!("Represented nodes ({})", glyph.members.len()));
            for &member in &glyph.members {
                self.node_link(ui, member);
            }
        }
        if !glyph.internal.is_empty() {
            ui.strong(format!("Internal edges ({})", glyph.internal.len()));
            for &edge in &glyph.internal {
                self.edge_link(ui, edge);
            }
        }
        let bundles: Vec<_> = fold
            .bundles
            .iter()
            .filter(|b| b.from == index || b.to == index)
            .collect();
        if !bundles.is_empty() {
            ui.strong(format!("Directed bundles ({})", bundles.len()));
            for b in bundles {
                if ui.link(self.bundle_name(b)).clicked() {
                    self.selected = Some(Selection::Bundle(b.from, b.to));
                }
            }
        }
    }

    fn show_bundle(&mut self, ui: &mut Ui, from: usize, to: usize) {
        let Some(fold) = self.folded.clone() else {
            return;
        };
        let Some(b) = fold.bundles.iter().find(|b| (b.from, b.to) == (from, to)) else {
            return;
        };
        egui::ScrollArea::vertical()
            .id_salt(("bundle", from, to))
            .show(ui, |ui| {
                ui.heading("Directed bundle");
                ui.label("From");
                self.node_link(ui, from);
                ui.label("To");
                self.node_link(ui, to);
                ui.strong(format!("Edges ({})", b.edges.len()));
                for &edge in &b.edges {
                    self.edge_link(ui, edge);
                }
            });
    }

    fn show_edge(&mut self, ui: &mut Ui, index: usize) {
        let graph = Arc::clone(&self.graph);
        let edge = &graph.edges[index];
        egui::ScrollArea::vertical()
            .id_salt(("edge", index))
            .show(ui, |ui| {
                ui.heading("Edge details");
                ui.label(RichText::new(&edge.id).monospace().strong());
                ui.label(RichText::new(&edge.relation).monospace());
                if index >= graph.shown_edges {
                    ui.label("Outside the saved view scope");
                } else if self.visible.edges.binary_search(&index).is_err() {
                    ui.label("Hidden by current filters");
                } else if let Some(fold) = self.folded.clone() {
                    let glyph = &fold.glyphs[fold.glyph_for[edge.from].unwrap()];
                    if glyph.internal.binary_search(&index).is_ok() {
                        ui.strong("Internal edge in");
                        self.node_link(ui, glyph.node);
                    } else {
                        let to = fold.glyphs[fold.glyph_for[edge.to].unwrap()].node;
                        if ui.link("Inspect directed bundle").clicked() {
                            self.selected = Some(Selection::Bundle(glyph.node, to));
                        }
                    }
                }
                ui.strong("From");
                self.node_link(ui, edge.from);
                ui.strong("To");
                self.node_link(ui, edge.to);
                if let Some(data) = &graph.artifact {
                    ui.collapsing("Canonical edge record", |ui| {
                        if let Some(record) = data["model"]["edges"].as_array().and_then(|items| {
                            items
                                .iter()
                                .find(|item| item["id"].as_str() == Some(edge.id.as_str()))
                        }) {
                            ui.monospace(format!("{record:#}"));
                        }
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
            selected: Some(Selection::Node(0)),
            folded: None,
            folded_layout: None,
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
        assert_eq!(viewer.selected, Some(Selection::Node(1)));
        assert!(viewer.reveal_selected);

        sender.send(Layout::new(&graph)).unwrap();
        frame(&mut viewer, Vec::new());
        assert!(matches!(viewer.layout, LayoutState::Ready(_)));
        assert_eq!(viewer.filters.search, "entity.note");
        assert_eq!(viewer.selected, Some(Selection::Node(1)));
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
            folded: None,
            folded_layout: None,
        };
        viewer.navigate(entity, Lens::All);
        assert_eq!(viewer.selected, Some(Selection::Node(entity)));
        assert_eq!(viewer.filters.lens, Lens::Components);
        assert_eq!(viewer.filters.search, "missing");
        assert_eq!(viewer.filters.community, "absent");
        assert_eq!(viewer.filters.kind, "component");
        assert_eq!(viewer.filters.relation, "owns");
        assert!(viewer.visible.nodes.is_empty() && !viewer.reveal_selected);
        viewer.navigate(step, Lens::All);
        assert_eq!(viewer.filters.lens, Lens::All);
        assert_eq!(viewer.selected, Some(Selection::Node(step)));
        assert!(!viewer.filters.narrowed());
        assert_eq!(viewer.visible.nodes, [step]);
        assert!(viewer.reveal_selected);
        viewer.navigate(step, Lens::Journey);
        assert_eq!(viewer.filters.lens, Lens::Journey);
        assert_eq!(viewer.selected, Some(Selection::Node(step)));
        assert_eq!(viewer.visible.nodes, [step]);
        assert!(matches!(viewer.layout, LayoutState::Pending(_)));
    }

    #[test]
    fn folded_layout_finishes_current_work_before_starting_latest_projection() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../conformance/fixtures/viewer-multigraph.json");
        let graph = Arc::new(Graph::load(&path).unwrap());
        let context = egui::Context::default();
        let visible = graph.visible(&Filters::default());
        let prior = Arc::new(visible.project(&graph, true));
        let (sender, receiver) = mpsc::channel();
        let mut viewer = Viewer {
            filters: Filters::default(),
            visible,
            graph,
            layout: LayoutState::Failed("Unfolded layout fixture".into()),
            zoom_percent: 100,
            reveal_selected: false,
            filename: "fixture".into(),
            selected: None,
            folded: Some(Arc::clone(&prior)),
            folded_layout: Some((Arc::clone(&prior), LayoutState::Pending(receiver))),
        };
        for query in ["backend", "second", "community"] {
            viewer.filters.search = query.into();
            viewer.refresh();
            viewer.update_layout(&context);
            let (in_flight, state) = viewer.folded_layout.as_ref().unwrap();
            assert!(Arc::ptr_eq(&prior, in_flight));
            assert!(matches!(state, LayoutState::Pending(_)));
            assert!(viewer.current_layout().is_none());
        }

        sender.send(Err("Previous layout finished".into())).unwrap();
        viewer.update_layout(&context);
        let (latest, state) = viewer.folded_layout.take().unwrap();
        assert!(Arc::ptr_eq(&latest, viewer.folded.as_ref().unwrap()));
        let LayoutState::Pending(receiver) = state else {
            panic!("Expected latest layout worker")
        };
        assert!(receiver.recv().unwrap().is_ok());
        viewer.set_folded(false);
        assert!(matches!(
            viewer.current_layout(),
            Some(LayoutState::Failed(_))
        ));
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
