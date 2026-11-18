mod overview;

use std::sync::{Arc, mpsc};
use std::thread;

use eframe::egui::{self, Button, Color32, Key, Modifiers, RichText, Ui};

use serde_json::Value as Json;

use crate::{
    canvas::{Layout, Scene},
    model::{
        EdgeBundle, Filters, FoldProjection, Graph, HumanConcept, Lens, Node, NodeInspection,
        RecordDetails, Selection, Source, Visible,
    },
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
    inspector_scroll: Option<(Option<Selection>, egui::Id)>,
    folded: Option<Arc<FoldProjection>>,
    folded_layout: Option<(Arc<FoldProjection>, LayoutState)>,
}

impl Viewer {
    pub fn new(graph: Graph, filename: String, context: egui::Context) -> Self {
        let graph = Arc::new(graph);
        let layout = LayoutState::start(Arc::clone(&graph), None, context);
        let filters = graph.initial_filters();
        Self {
            visible: graph.visible(&filters),
            filters,
            graph,
            layout,
            zoom_percent: 100,
            reveal_selected: false,
            filename,
            selected: None,
            inspector_scroll: None,
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
        } else if self.filters.narrowed()
            || (self.filters.lens == Lens::All && self.graph.shown_nodes > 0)
        {
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
                    ui.heading(RichText::new(self.graph.title()).size(24.0).strong());
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
                    let scenarios = ui
                        .checkbox(
                            &mut self.filters.show_scenarios,
                            format!("Show scenarios ({})", choices.scenario_count),
                        )
                        .changed();
                    let mut fold = self.folded.is_some();
                    if ui.checkbox(&mut fold, "Fold communities").changed() {
                        self.set_folded(fold);
                        ui.ctx().request_repaint();
                    }
                    if ui.button("Reset filters").clicked() {
                        self.set_folded(false);
                        self.filters = Filters::default();
                        self.refresh();
                    } else if kind || relation || community || scenarios {
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
            ui.label(drawing_counts(fold));
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
        if self.selected.is_some() {
            let back = ui.button("Model overview").on_hover_text("Alt+Home");
            if back.clicked() || ui.input_mut(|input| input.consume_key(Modifiers::ALT, Key::Home))
            {
                self.selected = None;
                self.reveal_selected = false;
                ui.ctx().request_repaint();
            }
        }
        let selection = self.selected;
        if let Some((previous, id)) = self.inspector_scroll
            && previous != selection
        {
            egui::scroll_area::State::default().store(ui.ctx(), id);
        }
        let shrink = !matches!(selection, None | Some(Selection::Node(_)));
        let area = egui::ScrollArea::vertical()
            .id_salt("inspector")
            .auto_shrink([shrink; 2])
            .show(ui, |ui| match selection {
                Some(Selection::Node(index)) => self.show_node(ui, index),
                Some(Selection::Edge(index)) => self.show_edge(ui, index),
                Some(Selection::Bundle(from, to)) => self.show_bundle(ui, from, to),
                Some(Selection::Source(index)) => {
                    let (id, source) = self
                        .graph
                        .sources
                        .iter()
                        .nth(index)
                        .expect("selected source");
                    ui.heading("Source declaration");
                    ui.monospace(id);
                    show_source(ui, source, true);
                }
                None => self.show_overview(ui),
            });
        // An inspector link can change self.selected while this record is being drawn.
        self.inspector_scroll = Some((selection, area.id));
    }

    fn show_node(&mut self, ui: &mut Ui, index: usize) {
        let graph = Arc::clone(&self.graph);
        let node = &graph.nodes[index];
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
        self.show_boundary_details(ui, &graph, node);
        self.show_folded_node(ui, index);
        if let Ok(projection) = &graph.perspectives {
            if projection.has_product_extension
                && let Some(human) = &node.human
            {
                match human {
                    HumanConcept::Step { action, response } => {
                        ui.strong("User action");
                        ui.label(action.as_deref().unwrap_or("Not declared (advisory gap)"));
                        ui.strong("System response");
                        ui.label(response.as_deref().unwrap_or("Not declared (advisory gap)"));
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
        show_record_details(ui, &graph, &node.id, &node.details);
        if let Some(data) = graph.input.artifact() {
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
    }

    fn show_boundary_details(&mut self, ui: &mut Ui, graph: &Graph, node: &Node) {
        let boundaries = graph.boundaries();
        match &node.inspection {
            NodeInspection::Component {
                boundary,
                interfaces,
            } => {
                if boundary.is_some() || boundaries.is_some() {
                    let boundary = boundary.as_ref().unwrap_or(&Json::Null);
                    ui.strong("Component responsibility");
                    ui.label(
                        boundary["responsibility"]
                            .as_str()
                            .unwrap_or("Not declared"),
                    );
                    ui.collapsing("Boundary rationale and exclusions", |ui| {
                        ui.strong("Rationale");
                        ui.label(boundary["rationale"].as_str().unwrap_or("Not declared"));
                        ui.strong("Exclusions");
                        match boundary["excludes"].as_array() {
                            None => {
                                ui.label("Not declared");
                            }
                            Some(items) if items.is_empty() => {
                                ui.label("None");
                            }
                            Some(items) => {
                                for item in items.iter().filter_map(Json::as_str) {
                                    ui.label(item);
                                }
                            }
                        }
                    });
                }
                ui.strong(if boundaries.is_some() {
                    "Authoritative public action contracts"
                } else {
                    "Declared interfaces"
                });
                match interfaces.as_deref() {
                    None => {
                        ui.label("Not declared");
                    }
                    Some([]) => {
                        ui.label("None");
                    }
                    Some(actions) => {
                        for id in actions {
                            self.action_link(ui, id);
                        }
                    }
                }
            }
            NodeInspection::Action { contract } => {
                ui.collapsing("Action contract", |ui| {
                    if let Some(text) = contract {
                        ui.monospace(text);
                    } else {
                        ui.label("Not declared");
                    }
                });
            }
            NodeInspection::Other if node.kind == "community" => {
                if let Some(boundaries) = boundaries
                    && let Some(boundary) = boundaries["communities"].get(&node.id)
                {
                    self.show_community_boundary(ui, boundary, &boundaries["components"]);
                }
            }
            _ => {}
        }
    }

    fn action_link(&mut self, ui: &mut Ui, action: &str) {
        if let Some(&index) = self.graph.actions.get(action) {
            self.node_link(ui, index);
        } else {
            ui.label(format!("{action} (action unavailable)"));
        }
    }

    fn show_community_boundary(&mut self, ui: &mut Ui, boundary: &Json, components: &Json) {
        ui.strong("Derived public boundary");
        ui.label("Component ownership determines participation. Membership grants no permissions.");
        let basis = boundary["membership_basis"].as_object().unwrap();
        let contracts = boundary["public_contracts"].as_array().unwrap();
        ui.label(format!(
            "Participating components: {} · Public contracts: {}",
            basis.len(),
            contracts.len()
        ));
        for contract in contracts {
            ui.horizontal_wrapped(|ui| {
                ui.label(contract["provider"].as_str().unwrap());
                self.action_link(ui, contract["action"].as_str().unwrap());
            });
        }
        ui.collapsing("Membership basis, calls and omissions", |ui| {
            ui.strong("Membership basis");
            if basis.is_empty() {
                ui.label("No participating components in this scope.");
            }
            for (component, members) in basis {
                ui.label(component);
                ui.indent(component, |ui| {
                    for member in members.as_array().unwrap() {
                        ui.label(member.as_str().unwrap());
                    }
                    if components[component]["interfaces_known"] == false {
                        ui.weak("Interfaces not declared");
                    }
                });
            }
            for (label, ids) in [
                ("Inbound calls", &boundary["calls"]["inbound"]),
                ("Outbound calls", &boundary["calls"]["outbound"]),
                ("Internal calls", &boundary["calls"]["internal"]),
                ("Unresolved calls", &boundary["calls"]["unresolved"]),
                ("Unresolved members", &boundary["unresolved_members"]),
                ("Omitted calls", &boundary["omitted_calls"]),
                ("Omitted components", &boundary["omitted_components"]),
            ] {
                let ids = ids.as_array().unwrap();
                ui.strong(format!("{label} ({})", ids.len()));
                for id in ids {
                    ui.label(id.as_str().unwrap());
                }
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
        ui.heading("Directed bundle");
        ui.label("From");
        self.node_link(ui, from);
        ui.label("To");
        self.node_link(ui, to);
        ui.strong(format!("Edges ({})", b.edges.len()));
        for &edge in &b.edges {
            self.edge_link(ui, edge);
        }
    }

    fn show_edge(&mut self, ui: &mut Ui, index: usize) {
        let graph = Arc::clone(&self.graph);
        let edge = &graph.edges[index];
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
        show_record_details(ui, &graph, &edge.id, &edge.details);
        if let Some(data) = graph.input.artifact() {
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
    }
}

fn counted(number: usize, singular: &str) -> String {
    let suffix = if number == 1 { "" } else { "s" };
    format!("{number} {singular}{suffix}")
}

fn drawing_counts(fold: &FoldProjection) -> String {
    let internal = fold.glyphs.iter().map(|glyph| glyph.internal.len()).sum();
    format!(
        "{} · {} · {}",
        counted(fold.glyphs.len(), "figure"),
        counted(fold.bundles.len(), "bundle"),
        counted(internal, "internal edge")
    )
}

fn show_location(ui: &mut Ui, location: &gdd::bundle::Location) {
    ui.label(format!("Model file: {}", location.file));
    ui.label(format!("JSON Pointer: {}", location.pointer));
}

fn show_record_details(ui: &mut Ui, graph: &Graph, id: &str, details: &RecordDetails) {
    show_location(ui, &details.location);
    for (label, declared) in &details.references {
        ui.strong(*label);
        let Some(items) = declared.as_array() else {
            ui.monospace(format!("{declared:#}"));
            continue;
        };
        if items.is_empty() {
            ui.label("None");
        }
        for (index, reference) in items.iter().enumerate() {
            ui.push_id((id, label, index), |ui| {
                show_reference(ui, graph, reference)
            });
        }
    }
}

fn declared_text(value: Option<&Json>) -> String {
    match value {
        Some(Json::String(text)) => text.clone(),
        Some(value) => format!("{value:#}"),
        None => "Not declared".into(),
    }
}

fn show_reference(ui: &mut Ui, graph: &Graph, reference: &Json) {
    if !reference.is_object() {
        ui.monospace(format!("{reference:#}"));
        return;
    }
    ui.label(format!(
        "Source: {}",
        declared_text(reference.get("source"))
    ));
    ui.label(format!(
        "Purpose: {}",
        declared_text(reference.get("purpose"))
    ));
    ui.strong("Selector");
    ui.monospace(declared_text(reference.get("selector")));
    let Some(id) = reference["source"].as_str() else {
        return;
    };
    let Some(source) = graph.sources.get(id) else {
        ui.label("Source declaration unavailable");
        return;
    };
    ui.collapsing(format!("Inspect source {id}"), |ui| {
        show_source(ui, source, false);
    });
}

fn show_source(ui: &mut Ui, source: &Source, full_declaration: bool) {
    show_location(ui, &source.location);
    ui.strong("URI");
    ui.monospace(declared_text(source.declaration.get("uri")));
    ui.strong("Declared SHA-256");
    ui.monospace(declared_text(
        source.declaration.pointer("/revision/sha256"),
    ));
    egui::CollapsingHeader::new("Full source declaration")
        .default_open(full_declaration)
        .show(ui, |ui| {
            ui.monospace(format!("{:#}", source.declaration));
        });
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
            inspector_scroll: None,
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
            show_scenarios: false,
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
            inspector_scroll: None,
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
        assert!(!viewer.filters.show_scenarios);
        assert!(viewer.visible.nodes.is_empty() && !viewer.reveal_selected);
        viewer.navigate(step, Lens::All);
        assert_eq!(viewer.filters.lens, Lens::All);
        assert_eq!(viewer.selected, Some(Selection::Node(step)));
        assert!(!viewer.filters.narrowed());
        assert!(viewer.filters.show_scenarios);
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
            inspector_scroll: None,
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

#[cfg(test)]
mod frame_tests;
