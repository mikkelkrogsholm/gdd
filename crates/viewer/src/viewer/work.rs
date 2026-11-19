use std::sync::Arc;

use eframe::egui::{self, Ui};
use gdd::work_overview::WorkPackage;

use super::Viewer;

impl Viewer {
    pub(super) fn show_work(&mut self, ui: &mut Ui, node: Option<usize>) {
        let graph = Arc::clone(&self.graph);
        let Some(work) = &graph.work else { return };
        ui.separator();
        ui.heading("Work status");
        if !work.plan_current {
            ui.label("This workplan is out of date or unresolved. Package activity is historical.");
        }
        let selected = node.map(|i| graph.nodes[i].id.as_str());
        let mut packages = work
            .packages
            .iter()
            .filter(|package| {
                selected.is_none_or(|id| package.nodes.iter().any(|linked| linked == id))
            })
            .peekable();
        if packages.peek().is_none() {
            ui.label(if selected.is_some() {
                "No work package covers this node in this report."
            } else {
                "This report contains no work packages."
            });
            return;
        }
        ui.label(format!("Run at capture: {}", work.run_phase));
        if work.acceptance_current {
            ui.label("Acceptance verified at capture");
        } else if work.run_phase == "accepted" {
            ui.label("Current acceptance unverified at capture");
        }
        ui.collapsing("Report revisions", |ui| {
            ui.label("Planned model revision");
            ui.monospace(&work.plan_spec_revision);
            ui.label("Workflow journal revision");
            ui.monospace(&work.run_revision);
            ui.label("Integrated code revision");
            ui.monospace(&work.code_revision);
            ui.label("Evidence assessed for code revision");
            ui.monospace(work.evidence_revision.as_deref().unwrap_or("Unassessed"));
        });
        for package in packages {
            let state = if package.phase == "pending" && package.available {
                "available"
            } else if package.phase == "pending" && !package.blockers.is_empty() {
                "blocked"
            } else {
                &package.phase
            };
            let section = egui::CollapsingHeader::new(format!("{} · {state}", package.id))
                .id_salt(("work-package", selected, &package.id))
                .default_open(selected.is_some())
                .show(ui, |ui| {
                    if package.available {
                        ui.label("Prerequisites met. Dispatch checks authorization.");
                    }
                    for reason in &package.blockers {
                        ui.label(reason);
                    }
                    if let Some(attempt) = &package.attempt {
                        ui.label(format!("Attempt: {attempt}"));
                    }
                    for id in &package.scope {
                        if selected != Some(id.as_str())
                            && let Some(index) = graph.nodes.iter().position(|n| n.id == *id)
                        {
                            if ui.link(id).clicked() {
                                self.follow_node(index);
                            }
                        } else {
                            ui.monospace(id);
                        }
                    }
                    if let Some(issue) = &package.issue {
                        ui.label("Linked issue");
                        ui.monospace(&issue.url);
                        if let Some(metadata) = &issue.github {
                            ui.label(format!("{} · {}", metadata.state, metadata.title));
                            if !metadata.assignees.is_empty() {
                                ui.label(format!("Assigned to {}", metadata.assignees.join(", ")));
                            }
                        }
                    }
                    show_checks(
                        ui,
                        package,
                        &work.code_revision,
                        work.evidence_revision.as_deref(),
                    );
                });
            if selected.is_none() && package.phase == "pending" && section.body_returned.is_none() {
                for reason in &package.blockers {
                    ui.label(reason);
                }
            }
        }
    }
}

fn show_checks(ui: &mut Ui, package: &WorkPackage, integrated: &str, assessed: Option<&str>) {
    ui.strong("Implementation evidence");
    if let Some(result) = package.verification {
        if let Some(revision) = assessed
            && revision != integrated
        {
            ui.label(format!("{result} for another code revision"));
            ui.monospace(revision);
        } else {
            ui.label(format!("Assessment at capture: {result}"));
        }
    } else {
        ui.label("Unassessed at capture");
    }
    for (index, check) in package.evidence.iter().enumerate() {
        let label = check.rule.as_deref().unwrap_or(&check.id);
        egui::CollapsingHeader::new(format!("{label} · {}", check.result))
            .id_salt(("work-check", &package.id, index))
            .show(ui, |ui| {
                if check.rule.is_some() {
                    ui.monospace(&check.id);
                }
                ui.label(&check.reason);
                if let Some(revision) = &check.code_revision {
                    ui.label("Code revision");
                    ui.monospace(revision);
                }
                for reference in &check.evidence_refs {
                    ui.monospace(reference);
                }
            });
    }
}
