use std::{collections::BTreeSet, sync::Arc};

use eframe::egui::Ui;
use gdd::knowledge::KnowledgeLink;

use super::{Selection, Viewer};

fn locator(link: &KnowledgeLink) -> (&str, &str, Option<&str>, Option<usize>) {
    (
        &link.source,
        &link.selector,
        link.heading.as_deref().or(link.selector_value.as_deref()),
        match (link.heading.as_ref(), link.ordinal, link.status.as_str()) {
            (Some(_), None, "pinned" | "unpinned") => Some(1),
            _ => link.ordinal,
        },
    )
}

fn section_label(link: &KnowledgeLink) -> String {
    let mut label = match link.heading.as_deref().or(link.selector_value.as_deref()) {
        Some(value) => value.to_owned(),
        None if link.selector == "whole-document" => "Whole page".into(),
        None => link.selector.clone(),
    };
    if let Some(ordinal) = link.ordinal
        && ordinal > 1
    {
        label.push_str(&format!(" (occurrence {ordinal})"));
    }
    label
}

impl Viewer {
    pub(super) fn wiki_overview(&mut self, ui: &mut Ui) {
        let graph = Arc::clone(&self.graph);
        let Some(wiki) = &graph.wiki else { return };
        if wiki.pages.is_empty() {
            return;
        }
        ui.separator();
        ui.heading("Project wiki");
        for (index, page) in wiki.pages.iter().enumerate() {
            ui.horizontal_wrapped(|ui| {
                if ui.link(&page.title).on_hover_text(&page.id).clicked() {
                    self.inspect(Selection::WikiPage(index));
                }
                ui.weak(&page.status);
            });
        }
    }

    pub(super) fn show_knowledge_for(&mut self, ui: &mut Ui, target: &str) {
        let graph = Arc::clone(&self.graph);
        let Some(wiki) = &graph.wiki else { return };
        let links = wiki
            .links
            .iter()
            .enumerate()
            .filter(|(_, l)| l.target == target);
        let mut shown = BTreeSet::new();
        for (index, link) in links {
            if !shown.insert(locator(link)) {
                continue;
            }
            if shown.len() == 1 {
                ui.separator();
                ui.strong("Wiki references");
            }
            let page = wiki.pages.iter().find(|p| p.id == link.source);
            let title = page.map_or(link.source.as_str(), |p| p.title.as_str());
            ui.horizontal_wrapped(|ui| {
                if ui
                    .link(format!("{title} · {}", section_label(link)))
                    .clicked()
                {
                    self.inspect(Selection::WikiSection(index));
                }
                ui.weak(&link.status);
            });
        }
        for (index, question) in wiki
            .questions
            .iter()
            .enumerate()
            .filter(|(_, q)| q.targets.iter().any(|id| id == target))
        {
            if ui
                .link(format!("{} · {}", question.id, question.status))
                .on_hover_text(&question.statement)
                .clicked()
            {
                self.inspect(Selection::WikiQuestion(index));
            }
        }
    }

    pub(super) fn wiki_page(&mut self, ui: &mut Ui, index: usize) {
        let graph = Arc::clone(&self.graph);
        let wiki = graph.wiki.as_ref().expect("selected wiki page");
        let page = &wiki.pages[index];
        ui.heading(&page.title);
        ui.monospace(&page.uri);
        let role = if page.normative {
            "Normative source"
        } else {
            "Advisory source"
        };
        ui.label(format!("{role} · {}", page.status));
        ui.label(&page.detail);
        ui.collapsing("Source revisions", |ui| {
            ui.monospace(&page.id);
            if let Some(hash) = &page.declared_sha256 {
                ui.label("Declared SHA-256");
                ui.monospace(hash);
            }
            if let Some(hash) = &page.sha256 {
                ui.label("Captured SHA-256");
                ui.monospace(hash);
            }
        });
        if !page.derived_from.is_empty() {
            ui.strong("Sources used for this page");
        }
        for origin in &page.derived_from {
            ui.horizontal_wrapped(|ui| {
                if let Some(index) = graph.sources.keys().position(|id| id == &origin.source) {
                    if ui.link(&origin.source).clicked() {
                        self.inspect(Selection::Source(index));
                    }
                } else {
                    ui.monospace(&origin.source);
                }
                ui.weak(&origin.status);
            });
            ui.push_id(
                (&origin.source, &origin.sha256, &origin.producer_revision),
                |ui| {
                    ui.collapsing("Source used at capture", |ui| {
                        for (field, value) in [
                            ("Source URI", &origin.uri),
                            ("Referenced SHA-256", &origin.sha256),
                            ("Producer revision", &origin.producer_revision),
                            ("Captured SHA-256", &origin.observed_sha256),
                        ] {
                            if let Some(value) = value {
                                ui.label(field);
                                ui.monospace(value);
                            }
                        }
                    });
                },
            );
        }
        let mut seen = BTreeSet::new();
        for (link_index, link) in wiki
            .links
            .iter()
            .enumerate()
            .filter(|(_, l)| l.source == page.id)
        {
            if seen.insert(locator(link)) && ui.link(section_label(link)).clicked() {
                self.inspect(Selection::WikiSection(link_index));
            }
        }
        if seen.is_empty() {
            ui.label("This report contains no links to or from this wiki page.");
        }
    }

    pub(super) fn wiki_section(&mut self, ui: &mut Ui, index: usize) {
        let graph = Arc::clone(&self.graph);
        let wiki = graph.wiki.as_ref().expect("selected wiki section");
        let link = &wiki.links[index];
        let page_index = wiki
            .pages
            .iter()
            .position(|p| p.id == link.source)
            .expect("validated source");
        let page = &wiki.pages[page_index];
        if ui.link(&page.title).on_hover_text(&page.uri).clicked() {
            self.inspect(Selection::WikiPage(page_index));
        }
        ui.heading(section_label(link));
        let same_section = || {
            wiki.links
                .iter()
                .filter(|other| locator(other) == locator(link))
        };
        if matches!(page.status.as_str(), "pinned" | "unpinned")
            && let Some((excerpt, truncated)) = same_section()
                .filter(|other| matches!(other.status.as_str(), "pinned" | "unpinned"))
                .find_map(|other| {
                    other
                        .excerpt
                        .as_deref()
                        .map(|excerpt| (excerpt, other.truncated))
                })
        {
            ui.label(excerpt);
            if truncated {
                ui.weak("Excerpt truncated in this report");
            }
        }
        ui.separator();
        ui.strong("Graph references");
        for other in same_section() {
            ui.horizontal_wrapped(|ui| {
                self.wiki_target(ui, &other.target_kind, &other.target);
                let relation = if other.kind == "wiki-link" {
                    "Mentioned in wiki"
                } else {
                    "Cites wiki"
                };
                ui.label(format!("{relation} · {} · {}", other.purpose, other.status));
            });
            if other.status != "pinned" {
                ui.label(&other.detail);
            }
        }
    }

    pub(super) fn wiki_question(&mut self, ui: &mut Ui, index: usize) {
        let graph = Arc::clone(&self.graph);
        let wiki = graph.wiki.as_ref().expect("selected wiki question");
        let question = &wiki.questions[index];
        ui.heading("Project question");
        ui.monospace(&question.id);
        ui.label(&question.statement);
        ui.label(format!("Status: {}", question.status));
        if !question.blocks.is_empty() {
            ui.label(format!("Blocks: {}", question.blocks.join(", ")));
        }
        if let Some(decision) = &question.resolution {
            ui.label("Resolution decision");
            ui.monospace(decision);
        }
        for target in &question.targets {
            if graph.edges.iter().any(|e| e.id == *target) {
                self.wiki_target(ui, "edge", target);
            } else {
                self.wiki_target(ui, "node", target);
            }
        }
        self.show_knowledge_for(ui, &question.id);
    }

    fn wiki_target(&mut self, ui: &mut Ui, kind: &str, id: &str) {
        let selection = match kind {
            "node" => self
                .graph
                .nodes
                .iter()
                .position(|n| n.id == id)
                .map(Selection::Node),
            "edge" => self
                .graph
                .edges
                .iter()
                .position(|e| e.id == id)
                .map(Selection::Edge),
            "question" => self
                .graph
                .wiki
                .as_ref()
                .and_then(|wiki| wiki.questions.iter().position(|q| q.id == id))
                .map(Selection::WikiQuestion),
            _ => None,
        };
        if let Some(selection) = selection {
            if ui.link(id).clicked() {
                if let Selection::Node(index) = selection {
                    self.follow_node(index);
                } else {
                    self.inspect(selection);
                }
            }
        } else {
            ui.monospace(id);
        }
    }
}
