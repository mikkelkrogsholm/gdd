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

#[test]
fn reference_inspection_preserves_selectors_and_endpoint_navigation() {
    use serde_json::json;

    let source = tempfile::tempdir().unwrap();
    let saved = tempfile::tempdir().unwrap();
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../conformance/fixtures/viewer-multigraph.json");
    let mut model: Json = serde_json::from_slice(&fs::read(fixture).unwrap()).unwrap();
    let action_label = model["nodes"]["constructor"]["label"]
        .as_str()
        .unwrap()
        .to_owned();
    let reference = json!({
        "source": "source.test", "purpose": "provenance",
        "selector": {"kind": "markdown-heading", "value": "Title", "ordinal": 9007199254740993_u64},
    });
    model["nodes"]["constructor"]["source_refs"] = json!([reference]);
    model["edges"][0]["source_refs"] = json!([reference]);
    let mut design = reference;
    design["purpose"] = json!("contract");
    model["nodes"]["screen.references"] = json!({
        "kind": "screen", "description": "Inspect exact design references", "design_refs": [design]
    });
    model["sources"]["source.unpinned"] = model["sources"]["source.test"].clone();
    model["nodes"]["action.second"]["source_refs"] = json!([
        {"source": "source.unpinned", "purpose": "provenance"},
        {"source": "source.test", "purpose": "provenance",
         "selector": {"kind": "markdown-heading", "value": "Later", "ordinal": 36893488147419103233_u128}}
    ]);
    let digest = "a".repeat(64);
    model["sources"]["source.test"]["revision"] = json!({"sha256": digest});
    let manifest = source.path().join("references.json");
    fs::write(&manifest, serde_json::to_vec(&model).unwrap()).unwrap();
    let view = source.path().join("references.gddview");
    gdd::view::render_view(
        &manifest,
        &view,
        &["constructor".into(), "screen.references".into()],
    )
    .unwrap();
    let direct = Window::open(&manifest);
    let detached = saved.path().join("detached.gddview");
    fs::rename(view, &detached).unwrap();
    source.close().unwrap();

    for (mut window, scoped) in [(direct, false), (Window::open(&detached), true)] {
        let assert_reference = |window: &mut Window, group, purpose, pointer| {
            let tree = window.frame(Vec::new());
            for label in [
                group,
                purpose,
                "Selector",
                "Model file: references.json",
                pointer,
            ] {
                assert!(
                    widget(&tree, Role::Label, label).is_some(),
                    "Missing {label}"
                );
            }
            let text = tree
                .nodes
                .iter()
                .filter_map(|(_, node)| node.value())
                .collect::<Vec<_>>()
                .join("\n");
            assert!(text.contains("markdown-heading") && text.contains("Title"));
            assert!(text.contains("\"ordinal\": 9007199254740993"), "{text}");
            assert!(!text.contains("9007199254740992"));
            if scoped {
                assert!(widget(&tree, Role::Button, "component.orphan").is_none());
            }
        };
        window.click(Role::Button, "screen.references");
        assert_reference(
            &mut window,
            "Design references",
            "Purpose: contract",
            "JSON Pointer: /nodes/screen.references",
        );
        window.click(Role::DisclosureTriangle, "Inspect source source.test");
        let tree = window.frame(Vec::new());
        for label in [
            "URI",
            "javascript:alert(1)",
            "Declared SHA-256",
            &digest,
            "JSON Pointer: /sources/source.test",
        ] {
            assert!(
                widget(&tree, Role::Label, label).is_some(),
                "Missing {label}"
            );
        }
        assert!(widget(&tree, Role::Link, "javascript:alert(1)").is_none());
        window.click(Role::Button, &action_label);
        assert_reference(
            &mut window,
            "Source references",
            "Purpose: provenance",
            "JSON Pointer: /nodes/constructor",
        );
        window.click(Role::Link, "edge.call_one");
        assert_reference(
            &mut window,
            "Source references",
            "Purpose: provenance",
            "JSON Pointer: /edges/0",
        );
        window.click(Role::Link, "action.second");
        let tree = window.frame(Vec::new());
        assert!(widget(&tree, Role::Label, "JSON Pointer: /nodes/action.second").is_some());
        assert!(widget(&tree, Role::Label, "Not declared").is_some());
        assert!(
            tree.nodes
                .iter()
                .filter_map(|(_, node)| node.value())
                .any(|text| text.contains("\"ordinal\": 36893488147419103233"))
        );
        window.click(Role::DisclosureTriangle, "Inspect source source.unpinned");
        let tree = window.frame(Vec::new());
        assert!(widget(&tree, Role::Label, "JSON Pointer: /sources/source.unpinned").is_some());
        let undeclared = tree
            .nodes
            .iter()
            .filter(|(_, node)| node.role() == Role::Label && node.value() == Some("Not declared"))
            .count();
        assert_eq!(undeclared, 2, "Expected absent selector and absent digest");
    }
}
