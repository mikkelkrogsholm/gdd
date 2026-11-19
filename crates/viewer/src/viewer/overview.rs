use super::projection::{Partition, projection_details};
use super::{
    Arc, Json, RichText, Selection, Ui, Viewer, counted, declared_text, drawing_counts, egui,
};

impl Viewer {
    pub(super) fn show_overview(&mut self, ui: &mut Ui) {
        let graph = Arc::clone(&self.graph);
        let overview = graph.input.overview();
        ui.heading("Model overview");
        ui.horizontal_wrapped(|ui| {
            ui.label("Theme");
            let mut preference = ui.ctx().options(|options| options.theme_preference);
            for (choice, label) in [
                (egui::ThemePreference::System, "System"),
                (egui::ThemePreference::Light, "Light"),
                (egui::ThemePreference::Dark, "Dark"),
            ] {
                if ui.radio_value(&mut preference, choice, label).changed() {
                    ui.ctx().set_theme(choice);
                    ui.ctx().request_repaint();
                }
            }
        });
        ui.strong(graph.title());
        if let Some(id) = overview.project.get("id") {
            ui.monospace(declared_text(Some(id)));
        }
        if let Some(goal) = overview.project.get("goal") {
            ui.label(declared_text(Some(goal)));
        }
        ui.collapsing("Project fields", |ui| {
            ui.monospace(format!("{:#}", overview.project));
        });

        ui.separator();
        ui.strong("The graph");
        ui.label(format!(
            "Full model: {}",
            graph_counts(graph.nodes.len(), graph.edges.len())
        ));
        let kinds: Vec<_> = graph.choices.kinds.iter().map(String::as_str).collect();
        if !kinds.is_empty() {
            ui.label(format!("Node types: {}", kinds.join(", ")));
        }
        let artifact = graph.input.artifact();
        let scope = artifact
            .and_then(|data| data["scope"].as_array())
            .filter(|ids| !ids.is_empty());
        if let Some(ids) = scope {
            let seeds: Vec<_> = ids.iter().map(|id| id.as_str().unwrap()).collect();
            ui.label(format!("Scope: {}", seeds.join(", ")));
            ui.label(format!(
                "In saved scope: {}",
                graph_counts(graph.shown_nodes, graph.shown_edges)
            ));
        } else {
            ui.label("Scope: Entire model");
        }
        ui.collapsing("Current view", |ui| {
            self.show_current_view(ui);
        });

        if !graph.sources.is_empty() {
            ui.separator();
            ui.strong("Declared sources");
            for (index, id) in graph.sources.keys().enumerate() {
                if ui.link(id).clicked() {
                    self.inspect(Selection::Source(index));
                    ui.ctx().request_repaint();
                }
            }
        }
        if let Some(questions) = overview.questions.and_then(Json::as_object)
            && !questions.is_empty()
        {
            ui.separator();
            ui.strong("Questions");
            for (id, question) in questions {
                ui.collapsing(id, |ui| {
                    ui.monospace(format!("{question:#}"));
                });
            }
        }

        ui.separator();
        show_traceability(ui, overview.snapshot, artifact);
    }

    fn show_current_view(&mut self, ui: &mut Ui) {
        ui.label(format!("Perspective: {}", self.filters.lens.label()));
        for (name, value) in [
            ("Search", &self.filters.search),
            ("Kind", &self.filters.kind),
            ("Relation", &self.filters.relation),
            ("Community", &self.filters.community),
        ] {
            if !value.is_empty() {
                ui.label(format!("{name}: {value}"));
            }
        }
        ui.label(format!(
            "Show scenarios: {}",
            if self.filters.show_scenarios {
                "on"
            } else {
                "off"
            }
        ));
        match &self.folded {
            None => {
                ui.label("Folding: off");
            }
            Some(fold) => {
                ui.label(format!("Folding: {}", drawing_counts(fold)));
            }
        }
        ui.label(format!("Graph zoom: {}", self.camera.label()));
        let graph = &self.graph;
        for (group, label) in [
            (Partition::Visible, "Visible"),
            (Partition::Filtered, "Filtered within scope"),
            (Partition::Outside, "Outside saved scope"),
        ] {
            let nodes = group.count(
                self.visible.nodes.len(),
                graph.shown_nodes,
                graph.nodes.len(),
            );
            let edges = group.count(
                self.visible.edges.len(),
                graph.shown_edges,
                graph.edges.len(),
            );
            let heading = format!("{label}: {}", graph_counts(nodes, edges));
            if nodes == 0 && edges == 0 {
                ui.label(heading);
                continue;
            }
            egui::CollapsingHeader::new(heading)
                .id_salt(label)
                .show(ui, |ui| {
                    for (name, ids) in [
                        (
                            "Node IDs",
                            group.ids(&graph.nodes, graph.shown_nodes, &self.visible.nodes, |n| {
                                &n.id
                            }),
                        ),
                        (
                            "Edge IDs",
                            group.ids(&graph.edges, graph.shown_edges, &self.visible.edges, |e| {
                                &e.id
                            }),
                        ),
                    ] {
                        if !ids.is_empty() {
                            ui.strong(name);
                            ui.add(
                                egui::Label::new(egui::RichText::new(ids.join("\n")).monospace())
                                    .selectable(true),
                            );
                        }
                    }
                });
        }
        ui.collapsing("Projection details (JSON)", |ui| {
            let text = self.projection_text.get_or_insert_with(|| {
                let details =
                    projection_details(graph, &self.filters, &self.visible, self.folded.as_deref());
                Arc::new(RichText::new(format!("{details:#}")).monospace())
            });
            ui.add(egui::Label::new(Arc::clone(text)).selectable(true));
        });
    }
}

fn graph_counts(nodes: usize, edges: usize) -> String {
    format!("{} · {}", counted(nodes, "node"), counted(edges, "edge"))
}

fn show_traceability(ui: &mut Ui, snapshot: Option<&Json>, artifact: Option<&Json>) {
    ui.strong("Traceability");
    if let Some(snapshot) = snapshot {
        ui.label("Revision");
        ui.monospace(declared_text(snapshot.get("revision")));
        ui.collapsing("Model snapshot", |ui| {
            ui.monospace(format!("{snapshot:#}"));
        });
    } else {
        ui.label("Model snapshot unavailable");
    }
    if let Some(data) = artifact {
        let generator = &data["generator"];
        ui.label(format!(
            "Saved view generator: {} {}",
            generator["name"].as_str().unwrap(),
            generator["version"].as_str().unwrap()
        ));
        ui.label(format!(
            "Observation inputs in saved view: {}",
            data["observation_inputs"].as_array().unwrap().len()
        ));
    } else {
        ui.label("No external observation layers are displayed");
    }
}
