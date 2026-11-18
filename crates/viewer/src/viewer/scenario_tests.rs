use std::{fs, path::Path};

use super::*;
use egui::accesskit::{Node, Role, TreeUpdate};

struct Window {
    viewer: Viewer,
    context: egui::Context,
}

impl Window {
    fn open(path: &Path) -> Self {
        let context = egui::Context::default();
        context.enable_accesskit();
        let viewer = Viewer::new(
            Graph::load(path).unwrap(),
            "Fixture".into(),
            context.clone(),
        );
        Self { viewer, context }
    }

    fn frame(&mut self, events: Vec<egui::Event>) -> TreeUpdate {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1280.0, 900.0),
            )),
            events,
            ..Default::default()
        };
        let mut output = self.context.run_ui(input, |ui| self.viewer.show(ui));
        output.textures_delta.clear();
        output.platform_output.accesskit_update.unwrap()
    }

    fn click(&mut self, role: Role, label: &str) {
        let tree = self.frame(Vec::new());
        let bounds = widget(&tree, role, label)
            .unwrap_or_else(|| panic!("Missing {role:?} {label}"))
            .bounds()
            .unwrap();
        let pos = egui::pos2(
            (bounds.x0 + bounds.x1) as f32 / 2.0,
            (bounds.y0 + bounds.y1) as f32 / 2.0,
        );
        for pressed in [true, false] {
            self.frame(vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Modifiers::NONE,
                },
            ]);
        }
    }

    fn search(&mut self, text: &str) {
        self.click(Role::TextInput, "Search graph");
        self.frame(vec![
            egui::Event::Key {
                key: Key::A,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::COMMAND,
            },
            egui::Event::Text(text.into()),
        ]);
    }

    fn assert_view(&mut self, checked: bool, nodes: usize, edges: usize, scenario_visible: bool) {
        let tree = self.frame(Vec::new());
        let checkbox = widget(&tree, Role::CheckBox, "Show scenarios (1)").unwrap();
        assert_eq!(checkbox.toggled(), Some(checked.into()));
        let count = format!("{nodes} of 7 nodes · {edges} of 9 edges");
        assert!(
            widget(&tree, Role::Label, &count).is_some(),
            "Expected {count}"
        );
        assert_eq!(
            widget(&tree, Role::Button, "scenario.hidden").is_some(),
            scenario_visible
        );
    }
}

fn widget<'a>(tree: &'a TreeUpdate, role: Role, name: &str) -> Option<&'a Node> {
    tree.nodes.iter().map(|(_, node)| node).find(|node| {
        node.role() == role
            && (node.label() == Some(name)
                || node.value() == Some(name)
                || tree.nodes.iter().any(|(id, label)| {
                    node.labelled_by().contains(id) && label.value() == Some(name)
                }))
    })
}

#[test]
fn scenario_controls_work_in_native_frames_for_source_and_detached_views() {
    let source = tempfile::tempdir().unwrap();
    let saved = tempfile::tempdir().unwrap();
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../conformance/fixtures/viewer-multigraph.json");
    let model = source.path().join("graph.json");
    fs::copy(fixture, &model).unwrap();
    let whole = saved.path().join("whole.gddview");
    let scoped = saved.path().join("scoped.gddview");
    let adjacent = saved.path().join("adjacent.gddview");
    let omitted = saved.path().join("omitted.gddview");
    for (file, scope) in [
        (&whole, vec![]),
        (&scoped, vec!["scenario.hidden".into()]),
        (&adjacent, vec!["constructor".into()]),
        (&omitted, vec!["component.orphan".into()]),
    ] {
        gdd::view::render_view(&model, file, &scope).unwrap();
    }
    let direct = Window::open(&model);
    source.close().unwrap();

    for mut window in [direct, Window::open(&whole)] {
        window.assert_view(false, 6, 8, false);
        window.click(Role::CheckBox, "Fold communities");
        let tree = window.frame(Vec::new());
        assert!(
            widget(
                &tree,
                Role::Label,
                "4 figures · 2 bundles · 3 internal edges"
            )
            .is_some()
        );
        window.click(Role::CheckBox, "Show scenarios (1)");
        window.assert_view(true, 7, 9, true);
        let tree = window.frame(Vec::new());
        assert!(
            widget(
                &tree,
                Role::Label,
                "5 figures · 3 bundles · 3 internal edges"
            )
            .is_some()
        );
        window.click(Role::CheckBox, "Show scenarios (1)");
        window.search("edge.scenario");
        window.assert_view(false, 2, 2, true);
        window.search("Toggle this scenario");
        window.assert_view(false, 1, 0, true);
        window.search("absent text");
        window.assert_view(false, 0, 0, false);
        window.search("  ");
        window.assert_view(false, 6, 8, false);
        window.click(Role::ComboBox, "Kind");
        window.click(Role::Button, "scenario");
        window.assert_view(false, 1, 0, true);
        window.click(Role::Button, "scenario.hidden");
        window.click(Role::Link, "Components");
        window.assert_view(true, 6, 9, true);
        window.click(Role::CheckBox, "Show scenarios (1)");
        window.assert_view(false, 5, 8, false);
        window.click(Role::Button, "Reset filters");
        window.assert_view(true, 7, 9, true);
        let tree = window.frame(Vec::new());
        assert_eq!(
            widget(&tree, Role::CheckBox, "Fold communities")
                .unwrap()
                .toggled(),
            Some(false.into())
        );
    }

    let mut window = Window::open(&scoped);
    window.assert_view(true, 3, 6, true);
    window.click(Role::CheckBox, "Show scenarios (1)");
    window.assert_view(false, 2, 5, false);
    window.search("entity.precision");
    window.assert_view(false, 0, 0, false);
    window.click(
        Role::Link,
        "entity.precision · No responsible or using component is linked to this entity.",
    );
    window.assert_view(false, 0, 0, false);
    let tree = window.frame(Vec::new());
    assert!(widget(&tree, Role::Label, "Outside the saved view scope").is_some());
    window.click(Role::Button, "Reset filters");
    window.assert_view(true, 3, 6, true);

    let mut window = Window::open(&adjacent);
    window.assert_view(false, 2, 5, false);
    window.click(Role::CheckBox, "Show scenarios (1)");
    window.assert_view(true, 3, 6, true);

    let mut window = Window::open(&omitted);
    window.assert_view(false, 1, 0, false);
    window.click(Role::CheckBox, "Show scenarios (1)");
    window.assert_view(true, 1, 0, false);
    window.search("scenario.hidden");
    window.assert_view(true, 0, 0, false);
}
