use std::{fs, path::Path};

use super::*;
use egui::accesskit::{Node, Role, TreeUpdate};

struct Window {
    viewer: Viewer,
    context: egui::Context,
    size: egui::Vec2,
    system_theme: Option<egui::Theme>,
}

impl Window {
    fn open(path: &Path) -> Self {
        let context = egui::Context::default();
        context.enable_accesskit();
        context.all_styles_mut(|style| style.animation_time = 0.0);
        let viewer = Viewer::new(
            Graph::load(path).unwrap(),
            "Fixture".into(),
            context.clone(),
        );
        Self {
            viewer,
            context,
            size: egui::vec2(1280.0, 900.0),
            system_theme: None,
        }
    }

    fn render(&mut self, events: Vec<egui::Event>) -> egui::FullOutput {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, self.size)),
            system_theme: self.system_theme,
            events,
            ..Default::default()
        };
        let mut output = self.context.run_ui(input, |ui| self.viewer.show(ui));
        output.textures_delta.clear();
        output
    }

    fn frame(&mut self, events: Vec<egui::Event>) -> TreeUpdate {
        self.render(events)
            .platform_output
            .accesskit_update
            .unwrap()
    }

    fn click(&mut self, role: Role, label: &str) {
        // Let the panel measure its contents before using the widget bounds.
        self.frame(Vec::new());
        let tree = self.frame(Vec::new());
        let bounds = widget(&tree, role, label)
            .unwrap_or_else(|| panic!("Missing {role:?} {label}"))
            .bounds()
            .unwrap();
        let pos = egui::pos2(
            (bounds.x0 + bounds.x1) as f32 / 2.0,
            (bounds.y0 + bounds.y1) as f32 / 2.0,
        );
        self.click_at(pos);
    }

    fn click_at(&mut self, pos: egui::Pos2) {
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

    fn key(&mut self, key: Key, modifiers: Modifiers) -> TreeUpdate {
        for pressed in [true, false] {
            self.frame(vec![egui::Event::Key {
                key,
                physical_key: None,
                pressed,
                repeat: false,
                modifiers,
            }]);
        }
        self.frame(Vec::new())
    }

    fn scroll(&mut self, pos: egui::Pos2, delta: f32) {
        let wheel = |phase| egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, delta),
            phase,
            source: egui::MouseWheelSource::Trackpad,
            modifiers: Modifiers::NONE,
        };
        self.frame(vec![
            egui::Event::PointerMoved(pos),
            wheel(egui::TouchPhase::Start),
            wheel(egui::TouchPhase::Move),
        ]);
    }

    fn wait_for_zoom(&mut self) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while widget(&self.frame(Vec::new()), Role::ComboBox, "Graph zoom").is_none() {
            assert!(
                std::time::Instant::now() < deadline,
                "No zoom control for {} visible nodes and {} visible edges",
                self.viewer.visible.nodes.len(),
                self.viewer.visible.edges.len()
            );
            std::thread::yield_now();
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

fn graph_clip(frame: &egui::FullOutput) -> egui::Rect {
    frame
        .shapes
        .iter()
        .find(|item| matches!(&item.shape, egui::Shape::Path(path) if !path.closed))
        .expect("A routed edge on the canvas")
        .clip_rect
}

fn assert_graph_fits(frame: &egui::FullOutput, nodes: usize, routes: usize) {
    let clip = graph_clip(frame);
    let inset = clip.shrink(15.0);
    let mut counts = (0, 0, 0);
    for item in frame.shapes.iter().filter(|item| item.clip_rect == clip) {
        match &item.shape {
            egui::Shape::Rect(node) if node.corner_radius.nw == 6 => {
                assert!(
                    inset.contains_rect(node.rect),
                    "Node {:?} vs {clip:?}",
                    node.rect
                );
                counts.0 += 1;
            }
            egui::Shape::Path(path) => {
                assert!(path.points.iter().all(|p| inset.contains(*p)));
                if !path.closed {
                    counts.1 += 1;
                } else if path.points.len() == 3 {
                    counts.2 += 1; // Arrowhead
                }
            }
            _ => {}
        }
    }
    assert_eq!(counts, (nodes, routes, routes));
}

#[test]
fn fit_pan_and_reset_operate_on_the_drawn_graph_in_wide_and_compact_windows() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("chain.json");
    let nodes: serde_json::Map<_, _> = (0..80)
        .map(|i| {
            (
                format!("part.{i:02}"),
                serde_json::json!({"kind": "component", "description": "Chain link"}),
            )
        })
        .collect();
    let edges: Vec<_> = (1..80)
        .map(|i| {
            serde_json::json!({
                "id": format!("edge.{i}"), "from": format!("part.{:02}", i-1),
                "to": format!("part.{i:02}"), "relation": "links"
            })
        })
        .collect();
    fs::write(&path, serde_json::to_vec(&serde_json::json!({
        "format": "gdd/0.2", "project": {"id":"chain","title":"Chain","goal":"Inspect all links"},
        "nodes": nodes, "edges": edges
    })).unwrap()).unwrap();
    let mut window = Window::open(&path);
    window.wait_for_zoom();
    let first_node = |frame: &egui::FullOutput| {
        let clip = graph_clip(frame);
        frame
            .shapes
            .iter()
            .find_map(|item| {
                if item.clip_rect == clip
                    && let egui::Shape::Text(text) = &item.shape
                    && text.galley.text() == "part.00"
                {
                    return Some(text.visual_bounding_rect().center());
                }
                None
            })
            .expect("First node in the viewport")
    };
    let initial = first_node(&window.render(Vec::new()));
    window.click_at(initial);
    assert_eq!(window.viewer.selected, Some(Selection::Node(0)));
    let clip = graph_clip(&window.render(Vec::new()));
    let start = clip.center_top() + egui::vec2(100.0, 20.0);
    let end = start - egui::vec2(55.0, 0.0);
    let button = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    window.frame(vec![egui::Event::PointerMoved(start), button(start, true)]);
    window.frame(vec![egui::Event::PointerMoved(end)]);
    window.frame(vec![button(end, false)]);
    let panned = first_node(&window.render(Vec::new()));
    assert!(
        (initial.x - panned.x - 55.0).abs() < 1.0,
        "{initial:?} to {panned:?}"
    );
    assert_eq!(window.viewer.selected, Some(Selection::Node(0)));
    window.key(Key::Home, Modifiers::ALT);
    assert_eq!(window.viewer.selected, None);
    assert_eq!(first_node(&window.render(Vec::new())), panned);
    window.click(Role::Button, "Reset camera");
    assert_eq!(first_node(&window.render(Vec::new())), initial);
    window.click_at(initial);

    window.click(Role::Button, "Fit graph");
    let frame = window.render(Vec::new());
    assert_graph_fits(&frame, 80, 79);
    let clip = graph_clip(&frame);
    assert!(
        frame
            .shapes
            .iter()
            .all(|item| item.clip_rect != clip || !matches!(item.shape, egui::Shape::Text(_)))
    );
    window.size = egui::vec2(320.0, 320.0);
    window.click(Role::Button, "Graph");
    assert_graph_fits(&window.render(Vec::new()), 80, 79);
    window.click(Role::Button, "Reset camera");
    assert_eq!(window.viewer.camera.zoom, Zoom::Percent(100));
    assert_eq!(window.viewer.selected, Some(Selection::Node(0)));
    assert!(first_node(&window.render(Vec::new())).x < 120.0);
    window.size = egui::vec2(1280.0, 900.0);
    window.click(Role::Button, "Reset camera");
    assert_eq!(first_node(&window.render(Vec::new())), initial);
}

#[test]
fn system_theme_updates_the_canvas_without_changing_the_view() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../conformance/fixtures/viewer-multigraph.json");
    let mut window = Window::open(&path);
    window.search("edge.call_one");
    window.click(Role::CheckBox, "Fold communities");
    window.wait_for_zoom();
    window.click(Role::ComboBox, "Graph zoom");
    window.click(Role::Button, "75%");
    window.click(Role::Button, "action.second");
    let selected = window.viewer.selected;
    window.size = egui::vec2(740.0, 500.0);
    window.click(Role::Button, "Graph");

    for reported in [None, Some(egui::Theme::Dark), Some(egui::Theme::Light)] {
        window.system_theme = reported;
        let frame = window.render(Vec::new());
        let theme = reported.unwrap_or(egui::Theme::Light);
        assert_eq!(window.context.theme(), theme);
        let visuals = theme.default_visuals();
        let fill_at = |position| {
            frame
                .shapes
                .iter()
                .rev()
                .find_map(|item| match &item.shape {
                    egui::Shape::Rect(rect)
                        if rect.rect.contains(position) && rect.fill.a() > 0 =>
                    {
                        Some(rect.fill)
                    }
                    _ => None,
                })
        };
        assert_eq!(fill_at(egui::pos2(1.0, 1.0)), Some(visuals.panel_fill));
        assert_eq!(
            fill_at(egui::pos2(2.0, 490.0)),
            Some(visuals.extreme_bg_color)
        );
        for (label, color) in [
            ("action.second", visuals.strong_text_color()),
            ("×3", visuals.selection.stroke.color),
        ] {
            let text = frame
                .shapes
                .iter()
                .find_map(|item| match &item.shape {
                    egui::Shape::Text(text) if text.galley.text() == label => Some(text),
                    _ => None,
                })
                .unwrap_or_else(|| panic!("Missing graph label {label}"));
            assert_eq!(text.galley.job.sections[0].format.color, color);
            assert_eq!(
                fill_at(text.visual_bounding_rect().center()),
                Some(visuals.selection.bg_fill)
            );
        }
        // Both the incident arrows and the unrelated self-loop remain visible.
        for color in [
            visuals.selection.stroke.color,
            visuals.widgets.hovered.bg_stroke.color,
        ] {
            assert!(frame.shapes.iter().any(|item|
                matches!(&item.shape, egui::Shape::Path(path) if path.closed && path.points.len() == 3 && path.fill == color)
            ));
        }
        assert_eq!(window.viewer.selected, selected);
        assert_eq!(window.viewer.camera.zoom, Zoom::Percent(75));
        assert_eq!(window.viewer.filters.search, "edge.call_one");
    }
}

#[test]
fn theme_controls_allow_keyboard_override_and_restoring_system() {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/product/greenfield.gddview");
    let mut window = Window::open(&path);
    window.system_theme = Some(egui::Theme::Light);
    window.size = egui::vec2(320.0, 330.0);
    let mut tree = window.key(Key::Home, Modifiers::ALT);
    assert_eq!(
        widget(&tree, Role::RadioButton, "System")
            .unwrap()
            .toggled(),
        Some(true.into())
    );
    for _ in 0..12 {
        tree = window.key(Key::Tab, Modifiers::NONE);
        if tree
            .nodes
            .iter()
            .any(|(id, node)| *id == tree.focus && node.label() == Some("Dark"))
        {
            break;
        }
    }
    let tree = window.key(Key::Enter, Modifiers::NONE);
    assert_eq!(
        widget(&tree, Role::RadioButton, "Dark").unwrap().toggled(),
        Some(true.into())
    );
    assert_eq!(window.context.theme(), egui::Theme::Dark);

    window.size = egui::vec2(1280.0, 900.0);
    window.system_theme = Some(egui::Theme::Dark);
    window.click(Role::RadioButton, "Light");
    assert_eq!(window.context.theme(), egui::Theme::Light);
    window.click(Role::RadioButton, "System");
    assert_eq!(window.context.theme(), egui::Theme::Dark);
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
    window.scroll(egui::pos2(1150.0, 520.0), -180.0);
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
    window.wait_for_zoom();
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

#[test]
fn compact_scoped_view_keeps_filters_and_selection_through_navigation_and_resize() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../conformance/fixtures/viewer-multigraph.json");
    let saved = tempfile::tempdir().unwrap();
    let title = "Project with a long descriptive name ".repeat(14);
    let mut model: Json = serde_json::from_slice(&fs::read(fixture).unwrap()).unwrap();
    model["project"]["title"] = title.clone().into();
    let manifest = saved.path().join("model.json");
    fs::write(&manifest, serde_json::to_vec(&model).unwrap()).unwrap();
    let path = saved.path().join("scoped.gddview");
    gdd::view::render_view(&manifest, &path, &["scenario.hidden".into()]).unwrap();
    fs::remove_file(manifest).unwrap();

    let mut window = Window::open(&path);
    window.size = egui::vec2(400.0, 710.0);
    let tree = window.frame(Vec::new());
    let search = widget(&tree, Role::TextInput, "Search graph")
        .unwrap()
        .bounds()
        .unwrap();
    assert!(search.x1 - search.x0 > 350.0 && search.x1 <= 400.0);
    assert!(widget(&tree, Role::Label, "Model overview").is_none());
    let title_box = widget(&tree, Role::Label, &title)
        .unwrap()
        .bounds()
        .unwrap();
    for tab in ["Nodes", "Graph", "Details"] {
        let bounds = widget(&tree, Role::Button, tab).unwrap().bounds().unwrap();
        assert!(bounds.y0 >= title_box.y1 && bounds.x1 <= 400.0);
    }
    window.click(Role::DisclosureTriangle, "View filters");
    window.click(Role::CheckBox, "Fold communities");
    window.click(Role::CheckBox, "Show scenarios (1)");
    window.click(Role::ComboBox, "Kind");
    window.click(Role::Button, "action");
    window.click(Role::DisclosureTriangle, "View filters");
    let tree = window.frame(Vec::new());
    assert_labels(&tree, &["2 of 7 nodes · 5 of 9 edges"]);
    assert!(widget(&tree, Role::TextInput, "Search graph").is_some());
    window.click(Role::Button, "action.second");
    let tree = window.frame(Vec::new());
    assert_labels(&tree, &["Second action"]);
    assert!(widget(&tree, Role::TextInput, "Search graph").is_none());

    window.click(Role::Button, "Graph");
    let tree = window.frame(Vec::new());
    assert!(widget(&tree, Role::Label, "Second action").is_none());
    window.wait_for_zoom();
    let selected = window.viewer.selected;
    window.click(Role::Button, "Fit graph");
    assert_graph_fits(&window.render(Vec::new()), 2, 3);
    assert_eq!(window.viewer.selected, selected);
    window.click(Role::ComboBox, "Graph zoom");
    window.click(Role::Button, "150%");
    window.size = egui::vec2(1280.0, 720.0);
    let tree = window.frame(Vec::new());
    assert_labels(&tree, &["2 of 7 nodes · 5 of 9 edges", "Second action"]);
    assert!(widget(&tree, Role::ComboBox, "Graph zoom").is_some());
    assert!(widget(&tree, Role::Button, "Details").is_none());
    window.size = egui::vec2(740.0, 360.0);
    let tree = window.frame(Vec::new());
    assert!(widget(&tree, Role::ComboBox, "Graph zoom").is_some());
    assert!(widget(&tree, Role::TextInput, "Search graph").is_none());
    window.click(Role::Button, "Details");
    assert_labels(&window.frame(Vec::new()), &["Second action"]);
    window.click(Role::Button, "Graph");
    let tree = window.key(Key::Home, Modifiers::ALT);
    assert_labels(&tree, &["Model overview", &title]);
    window.size = egui::vec2(400.0, 710.0);
    window.click(Role::DisclosureTriangle, "Current view");
    assert_labels(
        &window.frame(Vec::new()),
        &[
            "Scope: scenario.hidden",
            "Kind: action",
            "Show scenarios: off",
            "Folding: 2 figures · 3 bundles · 0 internal edges",
            "Graph zoom: 150%",
        ],
    );
    let tree = window.key(Key::F, Modifiers::COMMAND);
    let search = widget(&tree, Role::TextInput, "Search graph").unwrap();
    assert!(
        tree.nodes
            .iter()
            .any(|(id, n)| *id == tree.focus && n == search)
    );
    window.frame(vec![egui::Event::Text("action.second".into())]);
    assert_labels(&window.key(Key::Enter, Modifiers::NONE), &["Second action"]);
}

#[test]
fn short_window_supports_keyboard_pane_navigation_and_canvas_selection() {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/product/greenfield.gddview");
    let mut window = Window::open(&path);
    window.size = egui::vec2(740.0, 360.0);
    window.frame(Vec::new());
    // Move to Graph using keyboard focus and activate it with Enter.
    for _ in 0..10 {
        let tree = window.key(Key::Tab, Modifiers::NONE);
        let graph = widget(&tree, Role::Button, "Graph").unwrap();
        if tree
            .nodes
            .iter()
            .any(|(id, node)| *id == tree.focus && node == graph)
        {
            break;
        }
    }
    let tree = window.key(Key::Enter, Modifiers::NONE);
    assert!(widget(&tree, Role::TextInput, "Search graph").is_none());
    window.wait_for_zoom();
    let tree = window.key(Key::F, Modifiers::COMMAND);
    assert!(widget(&tree, Role::TextInput, "Search graph").is_some());
    window.click(Role::DisclosureTriangle, "View filters");
    window.click(Role::ComboBox, "Kind");
    window.click(Role::Button, "gdd.product.actor");
    window.key(Key::F, Modifiers::COMMAND);
    window.frame(vec![egui::Event::Text("actor.guest".into())]);
    let tree = window.frame(Vec::new());
    assert!(widget(&tree, Role::TextInput, "Search graph").is_some());
    window.click(Role::Button, "Graph");
    assert_eq!(window.viewer.visible.nodes.len(), 1);
    let glyph = window
        .render(Vec::new())
        .shapes
        .into_iter()
        .find_map(|item| {
            if let egui::Shape::Text(text) = item.shape
                && text.galley.text() == "actor.guest"
            {
                return Some(text.visual_bounding_rect().center());
            }
            None
        })
        .expect("Visible actor glyph in the graph");
    window.click_at(glyph);
    let tree = window.frame(Vec::new());
    assert!(widget(&tree, Role::Button, "Model overview").is_some());
    assert!(widget(&tree, Role::ComboBox, "Graph zoom").is_none());
    assert_labels(&tree, &["A human guest."]);
    window.key(Key::F, Modifiers::COMMAND);
    window.click(Role::Button, "Reset filters");
    window.scroll(egui::pos2(350.0, 300.0), -800.0);
    window.click(Role::Button, "step.reserve");
    assert_labels(&window.frame(Vec::new()), &["User action"]);
}

#[test]
fn community_legend_preserves_graph_identity_and_opens_the_filtered_community_by_keyboard() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/concert-0.3.gddview");
    let mut window = Window::open(&path);
    window.wait_for_zoom();
    window.click(Role::Button, "Fit graph");
    window.click(Role::DisclosureTriangle, "Community areas");
    let before = (
        window.viewer.visible.nodes.clone(),
        window.viewer.visible.edges.clone(),
    );
    let assert_areas = |frame: &egui::FullOutput, expected: usize| {
        let mut headings = Vec::new();
        let mut hulls = 0;
        for item in &frame.shapes {
            let clip = item.clip_rect;
            match &item.shape {
                egui::Shape::Path(path)
                    if path.closed && path.points.len() >= 4 && path.fill.a() < 40 =>
                {
                    assert!(path.points.iter().all(|p| clip.shrink(14.0).contains(*p)));
                    hulls += 1;
                }
                egui::Shape::Text(text)
                    if ["1. backend", "2. frontend", "3. reservation feature"]
                        .contains(&text.galley.text()) =>
                {
                    let rect = text.visual_bounding_rect();
                    assert!(
                        text.galley.size().y >= 12.0 && clip.contains_rect(rect),
                        "{rect:?} heading {} outside {clip:?}",
                        text.galley.text()
                    );
                    assert!(
                        headings
                            .iter()
                            .all(|prior: &egui::Rect| !rect.intersects(*prior))
                    );
                    headings.push(rect);
                }
                _ => {}
            }
        }
        assert_eq!((hulls, headings.len()), (expected, expected));
    };
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        window.system_theme = Some(theme);
        assert_areas(&window.render(Vec::new()), 3);
    }
    window.click(Role::CheckBox, "Show community areas");
    assert_areas(&window.render(Vec::new()), 0);
    assert_eq!(
        (&window.viewer.visible.nodes, &window.viewer.visible.edges),
        (&before.0, &before.1)
    );
    window.click(Role::Button, "1. backend · 8");
    let backend = window.viewer.graph.choices.communities["community.backend"];
    assert_eq!(window.viewer.selected, Some(Selection::Node(backend)));

    window.click(Role::CheckBox, "Fold communities");
    window.wait_for_zoom();
    let fold = Arc::clone(window.viewer.folded.as_ref().unwrap());
    window.click(Role::CheckBox, "Show community areas");
    assert_areas(&window.render(Vec::new()), 3);
    assert!(Arc::ptr_eq(&fold, window.viewer.folded.as_ref().unwrap()));
    assert_eq!(
        (&window.viewer.visible.nodes, &window.viewer.visible.edges),
        (&before.0, &before.1)
    );

    window.click(Role::ComboBox, "Kind");
    window.click(Role::Button, "action");
    window.size = egui::vec2(620.0, 640.0);
    window.click(Role::Button, "Nodes");
    let label = "3. reservation feature · 1";
    let mut focused = false;
    for _ in 0..32 {
        let tree = window.key(Key::Tab, Modifiers::NONE);
        let button = widget(&tree, Role::Button, label).unwrap();
        if tree
            .nodes
            .iter()
            .any(|(id, n)| *id == tree.focus && n == button)
        {
            focused = true;
            break;
        }
    }
    assert!(focused, "Community legend entry must accept keyboard focus");
    let tree = window.key(Key::Enter, Modifiers::NONE);
    assert_eq!(window.viewer.pane, Pane::Details);
    let target = window.viewer.graph.choices.communities["community.reservation_feature"];
    assert_eq!(window.viewer.selected, Some(Selection::Node(target)));
    assert_labels(&tree, &["community.reservation_feature"]);
    assert!(!window.viewer.visible.nodes.contains(&target));
    assert_eq!(window.viewer.filters.kind, "action");
}

#[test]
fn switching_panes_during_a_drag_keeps_the_last_graph_position_and_saved_view() {
    let folder = tempfile::tempdir().unwrap();
    let source = folder.path().join("fixture.json");
    fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../conformance/fixtures/viewer-multigraph.json"),
        &source,
    )
    .unwrap();
    let saved = folder.path().join("fixture.gddview");
    gdd::view::render_view(&source, &saved, &[]).unwrap();
    let original_files = [fs::read(&source).unwrap(), fs::read(&saved).unwrap()];
    let open_folded = || {
        let mut window = Window::open(&saved);
        window.size = egui::vec2(850.0, 850.0);
        window.click(Role::DisclosureTriangle, "View filters");
        window.click(Role::CheckBox, "Fold communities");
        window.click(Role::Button, "Graph");
        window.wait_for_zoom();
        window.click(Role::Button, "Fit graph");
        window
    };
    let glyphs = |frame: egui::FullOutput| {
        let clip = graph_clip(&frame);
        frame
            .shapes
            .into_iter()
            .filter_map(|item| match item.shape {
                egui::Shape::Rect(rect) if item.clip_rect == clip && rect.corner_radius.nw == 6 => {
                    Some(rect.rect)
                }
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    // Fit can reframe the drawing after release. Compare positions in the
    // drawing's coordinate system, with another glyph as the reference.
    let relative = |glyphs: &[egui::Rect], index: usize| {
        let anchor = glyphs[(index + 1) % glyphs.len()];
        (glyphs[index].center() - anchor.center()) / anchor.width()
    };
    let mut window = open_folded();
    let before = glyphs(window.render(vec![]));
    let nodes = window.viewer.visible.nodes.clone();
    let edges = window.viewer.visible.edges.clone();
    let projection = Arc::clone(window.viewer.folded.as_ref().unwrap());
    let node = window.viewer.graph.choices.communities["community.backend"];
    let slot = window.viewer.folded.as_ref().unwrap().glyph_for[node].unwrap();
    let start = before[slot].center();
    let delta = egui::vec2(68.0, -68.0);
    let button = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    window.frame(vec![egui::Event::PointerMoved(start), button(start, true)]);
    let moved = glyphs(window.render(vec![egui::Event::PointerMoved(start + delta)]));
    assert!((moved[slot].center() - start - delta).length() < 0.1);
    window.key(Key::F, Modifiers::COMMAND);
    assert_eq!(window.viewer.pane, Pane::Nodes);
    let other = egui::pos2(25.0, 105.0);
    window.frame(vec![egui::Event::PointerMoved(other), button(other, false)]);
    window.click(Role::Button, "Graph");
    let restored = glyphs(window.render(vec![]));
    assert!((relative(&restored, slot) - relative(&moved, slot)).length() < 0.001);
    assert_eq!(window.viewer.selected, None);
    assert_eq!(window.viewer.visible.nodes, nodes);
    assert_eq!(window.viewer.visible.edges, edges);
    assert!(Arc::ptr_eq(
        &projection,
        window.viewer.folded.as_ref().unwrap()
    ));
    assert_eq!(
        [fs::read(source).unwrap(), fs::read(&saved).unwrap()],
        original_files
    );
    let fresh = glyphs(open_folded().render(vec![]));
    assert!((relative(&fresh, slot) - relative(&before, slot)).length() < 0.001);
}
