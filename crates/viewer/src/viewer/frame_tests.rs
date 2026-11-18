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
        context.global_style_mut(|style| style.animation_time = 0.0);
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
fn inspector_keeps_its_scroll_until_a_different_record_opens() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../conformance/fixtures/viewer-multigraph.json");
    let mut window = Window::open(&fixture);
    let graph = &window.viewer.graph;
    let first = graph.nodes[graph.actions["constructor"]].label.clone();
    let connections_y = |tree: &TreeUpdate| {
        widget(tree, Role::Label, "Connections")
            .unwrap()
            .bounds()
            .unwrap()
            .y0
    };

    window.click(Role::Button, "action.second");
    let second_top = connections_y(&window.frame(Vec::new()));
    window.click(Role::Button, &first);
    let top = connections_y(&window.frame(Vec::new()));
    let wheel = |phase| egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: egui::vec2(0.0, -180.0),
        phase,
        source: egui::MouseWheelSource::Trackpad,
        modifiers: Modifiers::NONE,
    };
    window.frame(vec![
        egui::Event::PointerMoved(egui::pos2(1150.0, 520.0)),
        wheel(egui::TouchPhase::Start),
        wheel(egui::TouchPhase::Move),
    ]);
    let scrolled = connections_y(&window.frame(Vec::new()));
    assert!(
        top - scrolled > 100.0,
        "Expected to scroll the record, got {top} → {scrolled}"
    );
    assert_eq!(connections_y(&window.frame(Vec::new())), scrolled);

    window.click(Role::Button, "action.second");
    assert_eq!(connections_y(&window.frame(Vec::new())), second_top);
    window.click(Role::Button, &first);
    assert_eq!(connections_y(&window.frame(Vec::new())), top);
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

fn assert_labels(tree: &TreeUpdate, labels: &[&str]) {
    for label in labels {
        assert!(
            widget(tree, Role::Label, label).is_some(),
            "Missing {label}"
        );
    }
}

fn assert_json_display(tree: &TreeUpdate, expected: &Json) {
    assert!(
        tree.nodes
            .iter()
            .filter_map(|(_, node)| node.value())
            .filter_map(|text| serde_json::from_str::<Json>(text).ok())
            .any(|value| value == *expected),
        "Missing complete JSON display: {expected}"
    );
}

#[test]
fn overview_opens_captured_project_questions_sources_and_snapshot() {
    for (format, scope_counts) in [
        ("gdd/0.2", "3 nodes · 6 edges"),
        ("gdd/0.3", "5 nodes · 9 edges"),
    ] {
        assert_captured_overview(format, scope_counts);
    }
}

fn assert_captured_overview(format: &str, scope_counts: &str) {
    use serde_json::json;

    let source = tempfile::tempdir().unwrap();
    let saved = tempfile::tempdir().unwrap();
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../conformance/fixtures/viewer-multigraph.json");
    let mut model: Json = serde_json::from_slice(&fs::read(fixture).unwrap()).unwrap();
    model["format"] = json!(format);
    model["project"]["scope"] = json!("Author-declared project scope");
    model["project"]["assumptions"] = json!(["Literal <text> & Ω", "Second assumption"]);
    model["questions"] = json!({"question.review": {
        "statement": "Who reviews this contract? </script> & Ω",
        "targets": ["component.orphan"], "blocks": ["implementation"], "status": "open"
    }});
    let source_bytes = b"Actual source captured in the normative snapshot.\n";
    fs::write(source.path().join("source.txt"), source_bytes).unwrap();
    let source_digest = gdd::sha256(source_bytes);
    model["sources"]["source.pinned"] = json!({
        "title":"Captured observation", "role":"observation", "uri":"source.txt",
        "format":"text/plain", "normative":true, "revision":{"sha256":source_digest},
        "producer":{"name":"Fixture producer", "version":"2026.1"}
    });
    let path = source.path().join("overview.json");
    let bytes = serde_json::to_vec(&model).unwrap();
    fs::write(&path, &bytes).unwrap();
    let model_digest = gdd::sha256(&bytes);
    let revision = gdd::snapshot_revision(&[
        ("overview.json", &model_digest),
        ("source.txt", &source_digest),
    ])
    .unwrap();
    let snapshot = json!({
        "format":"gdd-snapshot/0.2", "revision":revision,
        "assets":[{"path":"overview.json", "sha256":model_digest}, {"path":"source.txt", "sha256":source_digest}]
    });
    let detached = saved.path().join("scoped.gddview");
    gdd::view::render_view(&path, &detached, &["scenario.hidden".into()]).unwrap();
    let direct = Window::open(&path);
    source.close().unwrap();

    for (mut window, scoped) in [(direct, false), (Window::open(&detached), true)] {
        let tree = window.frame(Vec::new());
        assert_labels(
            &tree,
            &[
                "Model overview",
                "Viewer fixture",
                "viewer-fixture",
                "Inspect without losing identity or executing data",
                "Full model: 7 nodes · 9 edges",
                "Node types: action, community, component, entity, scenario",
                &revision,
            ],
        );
        if scoped {
            assert_labels(
                &tree,
                &[
                    "Scope: scenario.hidden",
                    &format!("In saved scope: {scope_counts}"),
                    &format!("Saved view generator: gdd-viewer {}", gdd::view::VERSION),
                    "Observation inputs in saved view: 0",
                ],
            );
        } else {
            assert_labels(
                &tree,
                &[
                    "Scope: Entire model",
                    "No external observation layers are displayed",
                ],
            );
            assert!(
                !tree
                    .nodes
                    .iter()
                    .filter_map(|(_, node)| node.value())
                    .any(|value| value.contains("Saved view generator"))
            );
        }
        for (label, expected) in [
            ("Project fields", &model["project"]),
            ("question.review", &model["questions"]["question.review"]),
            ("Model snapshot", &snapshot),
        ] {
            window.click(Role::DisclosureTriangle, label);
            assert_json_display(&window.frame(Vec::new()), expected);
            window.click(Role::DisclosureTriangle, label);
        }
        window.click(Role::Link, "source.pinned");
        let tree = window.frame(Vec::new());
        assert_labels(
            &tree,
            &[
                "Source declaration",
                "source.pinned",
                "Model file: overview.json",
                "JSON Pointer: /sources/source.pinned",
                "source.txt",
                &source_digest,
            ],
        );
        assert_json_display(
            &window.frame(Vec::new()),
            &model["sources"]["source.pinned"],
        );
        window.click(Role::Button, "Model overview");
        window.click(Role::Link, "source.test");
        let tree = window.frame(Vec::new());
        assert_labels(
            &tree,
            &[
                "JSON Pointer: /sources/source.test",
                "javascript:alert(1)",
                "Not declared",
            ],
        );
        assert!(widget(&tree, Role::Link, "javascript:alert(1)").is_none());
        window.click(Role::Button, "Model overview");
        window.click(Role::DisclosureTriangle, "Current view");
        let tree = window.frame(Vec::new());
        let visible = if scoped {
            format!("Visible: {scope_counts}")
        } else {
            "Visible: 6 nodes · 8 edges".into()
        };
        assert!(widget(&tree, Role::DisclosureTriangle, &visible).is_some());
        assert_eq!(
            widget(&tree, Role::Button, "component.orphan").is_some(),
            !scoped
        );
    }
}

#[test]
fn overview_shortcut_preserves_scoped_filters_lens_folding_and_zoom() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../conformance/fixtures/viewer-multigraph.json");
    let saved = tempfile::tempdir().unwrap();
    let path = saved.path().join("scoped.gddview");
    gdd::view::render_view(&fixture, &path, &["scenario.hidden".into()]).unwrap();
    let mut window = Window::open(&path);
    window.click(Role::ComboBox, "Perspective");
    window.click(Role::Button, "Components");
    window.click(Role::ComboBox, "Kind");
    window.click(Role::Button, "action");
    window.click(Role::ComboBox, "Relation");
    window.click(Role::Button, "calls");
    window.search("edge.call_one");
    window.click(Role::CheckBox, "Show scenarios (1)");
    window.click(Role::CheckBox, "Fold communities");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while widget(&window.frame(Vec::new()), Role::ComboBox, "Graph zoom").is_none() {
        assert!(
            std::time::Instant::now() < deadline,
            "No zoom control for {} visible nodes and {} visible edges",
            window.viewer.visible.nodes.len(),
            window.viewer.visible.edges.len()
        );
        std::thread::yield_now();
    }
    window.click(Role::ComboBox, "Graph zoom");
    window.click(Role::Button, "150%");
    window.click(Role::Button, "action.second");
    window.click(Role::Link, "edge.call_one");
    let tree = window.frame(Vec::new());
    assert_labels(&tree, &["Edge details", "edge.call_one"]);
    let badge = widget(&tree, Role::Unknown, "Inspect 2 edges")
        .unwrap()
        .bounds();
    window.frame(vec![egui::Event::Key {
        key: Key::Home,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::ALT,
    }]);
    window.click(Role::DisclosureTriangle, "Current view");
    let tree = window.frame(Vec::new());
    assert_labels(
        &tree,
        &[
            "Model overview",
            "Full model: 7 nodes · 9 edges",
            "Scope: scenario.hidden",
            "In saved scope: 3 nodes · 6 edges",
            "Perspective: Components",
            "Search: edge.call_one",
            "Kind: action",
            "Relation: calls",
            "Show scenarios: off",
            "Folding: 2 figures · 3 bundles · 0 internal edges",
            "Graph zoom: 150%",
        ],
    );
    assert_eq!(
        widget(&tree, Role::Unknown, "Inspect 2 edges")
            .unwrap()
            .bounds(),
        badge
    );
    for (partition, nodes, edges) in [
        (
            "Visible: 2 nodes · 4 edges",
            "action.second\nconstructor",
            "edge.call_one\nedge.call_two\nedge.loop\nedge.reverse",
        ),
        (
            "Filtered within scope: 1 node · 2 edges",
            "scenario.hidden",
            "edge.depends\nedge.scenario",
        ),
        (
            "Outside saved scope: 4 nodes · 3 edges",
            "community.backend\ncommunity.frontend\ncomponent.orphan\nentity.precision",
            "edge.member_backend\nedge.member_frontend\nedge.second_frontend",
        ),
    ] {
        window.click(Role::DisclosureTriangle, partition);
        assert_labels(&window.frame(Vec::new()), &[nodes, edges]);
        window.click(Role::DisclosureTriangle, partition);
    }
    window.click(Role::Button, "action.second");
    window.click(Role::Button, "Model overview");
    assert_labels(
        &window.frame(Vec::new()),
        &["Model overview", "Perspective: Components"],
    );
    window.click(Role::Button, "action.second");
    window.click(Role::Link, "edge.call_one");
    window.click(Role::Link, "Inspect directed bundle");
    assert_labels(&window.frame(Vec::new()), &["Directed bundle"]);
    window.click(Role::Button, "Model overview");
    let tree = window.frame(Vec::new());
    assert_labels(
        &tree,
        &[
            "Model overview",
            "Search: edge.call_one",
            "Graph zoom: 150%",
        ],
    );
    assert_eq!(window.viewer.selected, None);
    assert!(!window.viewer.reveal_selected);
    assert!(widget(&tree, Role::Button, "scenario.hidden").is_none());
    window.click(Role::Button, "Reset filters");
    window.click(Role::CheckBox, "Show scenarios (1)");
    window.click(
        Role::DisclosureTriangle,
        "Filtered within scope: 1 node · 1 edge",
    );
    assert_labels(
        &window.frame(Vec::new()),
        &["scenario.hidden", "edge.scenario"],
    );
}
