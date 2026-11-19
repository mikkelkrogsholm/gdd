use super::*;
use egui::{Event, PointerButton, vec2};

struct Canvas {
    context: egui::Context,
    graph: Graph,
    visible: Visible,
    folded: Option<FoldProjection>,
    layout: Layout,
    camera: Camera,
    areas: Option<CommunityGeometry>,
    selected: Option<Selection>,
    worker: RouteWorker,
    focused: bool,
}

impl Canvas {
    fn new(fold: bool, zoom: Zoom) -> Self {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../conformance/fixtures/viewer-multigraph.json");
        // The original fixture puts its loop inside a folded member. Add a
        // loop on the representative too, so both drawings expose loop hits.
        let folder = tempfile::tempdir().unwrap();
        let mut model: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        model["edges"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({
                "id": "edge.representative.loop", "relation": "links",
                "from": "community.backend", "to": "community.backend"
            }));
        let path = folder.path().join("multigraph.json");
        std::fs::write(&path, serde_json::to_vec(&model).unwrap()).unwrap();
        let graph = Graph::load(&path).unwrap();
        let visible = graph.visible(&Default::default());
        let folded = fold.then(|| visible.project(&graph, true));
        let layout = match &folded {
            Some(projection) => Layout::folded(&graph, projection),
            None => Layout::new(&graph),
        }
        .unwrap();
        Self {
            context: Default::default(),
            graph,
            visible,
            folded,
            layout,
            camera: Camera {
                zoom,
                ..Default::default()
            },
            areas: None,
            selected: None,
            worker: Default::default(),
            focused: true,
        }
    }

    fn frame(&mut self, events: Vec<Event>) -> egui::FullOutput {
        let size = if self.camera.zoom == Zoom::Fit {
            vec2(600.0, 400.0)
        } else {
            // Keep every route selectable throughout this fixed-zoom gesture.
            // Scrollbars animate the viewport when a drop changes its bounds.
            vec2(2600.0, 2400.0)
        };
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
            events,
            focused: self.focused,
            ..Default::default()
        };
        let mut output = self.context.run_ui(input, |ui| {
            let scene = Scene {
                graph: &self.graph,
                visible: &self.visible,
                folded: self.folded.as_ref(),
                selected: self.selected,
                show_areas: true,
            };
            if let Some(clicked) =
                self.layout
                    .show(ui, &scene, &mut self.camera, false, &mut self.areas)
            {
                self.selected = Some(clicked);
            }
            self.worker.update(&mut self.layout, ui.ctx());
        });
        output.textures_delta.clear();
        output
    }

    fn settle(&mut self) -> egui::FullOutput {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while self.worker.running.is_some() || self.layout.needs_routing {
            assert!(std::time::Instant::now() < deadline);
            self.frame(vec![]);
            thread::yield_now();
        }
        assert_eq!(self.layout.route_error(), None);
        self.frame(vec![])
    }

    fn click(&mut self, point: Pos2) {
        self.frame(vec![Event::PointerMoved(point), button(point, true)]);
        self.frame(vec![button(point, false)]);
    }

    fn drag(&mut self, slot: usize, delta: Vec2) {
        let start = Self::glyphs(&self.frame(vec![]))[slot].center();
        self.frame(vec![Event::PointerMoved(start), button(start, true)]);
        self.frame(vec![Event::PointerMoved(start + delta)]);
        self.frame(vec![button(start + delta, false)]);
    }

    fn glyphs(frame: &egui::FullOutput) -> Vec<Rect> {
        frame
            .shapes
            .iter()
            .filter_map(|item| match &item.shape {
                // Scrollbar handles can have the same corner radius as a node.
                egui::Shape::Rect(r)
                    if r.corner_radius.nw == 6 && r.stroke_kind == egui::StrokeKind::Inside =>
                {
                    Some(r.rect)
                }
                _ => None,
            })
            .collect()
    }

    fn hulls(frame: &egui::FullOutput) -> Vec<Vec<Pos2>> {
        frame
            .shapes
            .iter()
            .filter_map(|item| match &item.shape {
                egui::Shape::Path(p) if p.closed && p.points.len() >= 4 => Some(p.points.clone()),
                _ => None,
            })
            .collect()
    }
}

fn button(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    }
}

fn near(a: Pos2, b: Pos2) {
    assert!(a.distance(b) < 0.05, "{a:?} vs {b:?}");
}

fn check_ports(layout: &Layout) {
    for (route, edge) in layout.routes.iter().zip(&layout.topology.edges) {
        let start = route.points[0];
        let tip = *route.points.last().unwrap();
        for (point, index) in [(start, edge.source.index()), (tip, edge.target.index())] {
            let rect = layout.nodes[index];
            assert!(
                rect.expand(0.02).contains(point) && !rect.shrink(0.02).contains(point),
                "{point:?} vs {rect:?}"
            );
        }
        assert!(
            layout.nodes[edge.target.index()]
                .expand(0.01)
                .contains(tip + route.arrow_direction * 0.1)
        );
    }
}

#[test]
fn pointer_drag_tracks_grab_offset_routes_and_shared_hulls_at_fixed_zoom_and_fit() {
    for (fold, zoom) in [(false, Zoom::Percent(150)), (true, Zoom::Fit)] {
        let mut canvas = Canvas::new(fold, zoom);
        let node = canvas
            .graph
            .nodes
            .iter()
            .position(|n| n.id == "constructor")
            .unwrap();
        let slot = canvas
            .folded
            .as_ref()
            .map_or(node, |f| f.glyph_for[node].unwrap());
        let selected = fold.then_some(Selection::Node(node));
        canvas.selected = selected;
        canvas.frame(vec![]);
        let before = canvas.frame(vec![]);
        let glyphs = Canvas::glyphs(&before);
        assert_eq!(glyphs.len(), canvas.layout.nodes.len());
        let old_nodes = canvas.layout.nodes.clone();
        let old_hulls = Canvas::hulls(&before);
        let scale = canvas.camera.frame.unwrap().scale;
        let start = glyphs[slot].center() + vec2(9.0, -3.0);
        canvas.frame(vec![Event::PointerMoved(start), button(start, true)]);
        for delta in [vec2(-32.0, -16.0), vec2(-48.0, -24.0)] {
            let frame = canvas.frame(vec![Event::PointerMoved(start + delta)]);
            assert!(
                canvas.layout.drag.is_some(),
                "Node did not capture pointer at {start:?}"
            );
            assert!(
                canvas.worker.running.is_none(),
                "A reroute started during motion"
            );
            assert_eq!(canvas.selected, selected);
            assert_eq!(scale, canvas.camera.frame.unwrap().scale);
            let moved = Canvas::glyphs(&frame);
            for (i, rect) in moved.iter().enumerate() {
                let motion = if i == slot { delta } else { Vec2::ZERO };
                near(glyphs[i].center() + motion, rect.center());
                near(
                    old_nodes[i].center() + motion / scale,
                    canvas.layout.nodes[i].center(),
                );
            }
            check_ports(&canvas.layout);
            let hulls = Canvas::hulls(&frame);
            assert_eq!(hulls.len(), 2);
            assert_ne!(hulls, old_hulls);
            for hull in hulls {
                for corner in [moved[slot].left_top(), moved[slot].right_bottom()] {
                    for (a, b) in hull.iter().zip(hull.iter().cycle().skip(1)) {
                        let edge = *b - *a;
                        let point = corner - *a;
                        assert!(edge.x * point.y >= edge.y * point.x);
                    }
                }
            }
        }
        let drop = start + vec2(-53.0, -28.0);
        canvas.frame(vec![
            button(drop, false),
            Event::PointerMoved(drop + vec2(23.0, 70.0)),
        ]);
        assert!(canvas.layout.drag.is_none() && canvas.worker.running.is_some());
        assert_eq!(canvas.selected, selected);
        let frame = canvas.settle();
        near(
            old_nodes[slot].center() + (drop - start) / scale,
            canvas.layout.nodes[slot].center(),
        );
        check_ports(&canvas.layout);
        let glyphs = Canvas::glyphs(&frame);
        let point = glyphs[slot].center();
        canvas.click(point);
        let canonical = canvas.folded.as_ref().map_or(node, |f| f.glyphs[slot].node);
        assert_eq!(canvas.selected, Some(Selection::Node(canonical)));
        let to_screen = TSTransform::new(
            point.to_vec2()
                - canvas.layout.nodes[slot].center().to_vec2() * canvas.camera.frame.unwrap().scale,
            canvas.camera.frame.unwrap().scale,
        );
        let edges: Vec<_> = if let Some(f) = &canvas.folded {
            f.bundles
                .iter()
                .enumerate()
                .map(|(index, b)| (index, Selection::Bundle(b.from, b.to)))
                .collect()
        } else {
            // All three parallel records, their reverse, and the moved node's loop.
            canvas
                .graph
                .edges
                .iter()
                .enumerate()
                .filter(|(_, e)| {
                    [
                        "edge.call_one",
                        "edge.call_two",
                        "edge.depends",
                        "edge.reverse",
                        "edge.loop",
                    ]
                    .contains(&e.id.as_str())
                })
                .map(|(index, _)| (index, Selection::Edge(index)))
                .collect()
        };
        for (index, selection) in edges {
            let p = selectable_point(&canvas.layout, index, to_screen);
            canvas.click(p);
            assert_eq!(canvas.selected, Some(selection), "route {index} at {p:?}");
        }
        if let Some(f) = &canvas.folded {
            let (index, b) = f
                .bundles
                .iter()
                .enumerate()
                .find(|(_, b)| b.edges.len() >= 3)
                .unwrap();
            let selection = Selection::Bundle(b.from, b.to);
            canvas.click(to_screen * canvas.layout.routes[index].midpoint);
            assert_eq!(canvas.selected, Some(selection));
        }
        let positions = canvas.layout.nodes.clone();
        canvas.camera.reset(Zoom::Percent(100));
        canvas.frame(vec![]);
        canvas.camera.reset(Zoom::Fit);
        canvas.frame(vec![]);
        assert_eq!(canvas.layout.nodes, positions);
    }
}

// Find a clickable point on the requested path, clear of nodes and other lanes.
fn selectable_point(layout: &Layout, index: usize, transform: TSTransform) -> Pos2 {
    let path = &layout.routes[index].points;
    let (distance, point) = path
        .windows(2)
        .flat_map(|p| (1..10).map(|i| p[0].lerp(p[1], (i as f32) / 10.0)))
        .filter(|p| {
            !layout
                .nodes
                .iter()
                .any(|rect| rect.expand(2.0).contains(*p))
        })
        .map(|p| {
            let clearance = layout
                .routes
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != index)
                .flat_map(|(_, route)| route.points.windows(2))
                .map(|segment| distance_to_segment(p, segment[0], segment[1]))
                .fold(f32::INFINITY, f32::min);
            (clearance, p)
        })
        .max_by(|a, b| a.0.total_cmp(&b.0))
        .unwrap();
    assert!(distance > 0.01, "route {index} has no distinct lane");
    transform * point
}

#[test]
fn repeated_pointer_drops_coalesce_and_discard_superseded_routes() {
    let mut canvas = Canvas::new(false, Zoom::Percent(100));
    canvas.frame(vec![]);
    // Hold a prior job open to exercise the same ordering on every machine.
    let (sender, receiver) = mpsc::channel();
    canvas.worker.running = Some(RoutingJob {
        topology: Arc::clone(&canvas.layout.topology),
        revision: canvas.layout.revision,
        receiver,
    });
    for i in 0..3 {
        canvas.drag(i, vec2(20.0, 9.0));
        assert!(canvas.layout.needs_routing);
        assert_eq!(canvas.worker.running.as_ref().unwrap().revision, 0);
    }
    let positions = canvas.layout.nodes.clone();
    sender.send(Err("superseded positions".into())).unwrap();
    canvas.frame(vec![]);
    assert_eq!(canvas.layout.route_error(), None);
    assert_eq!(
        canvas.worker.running.as_ref().unwrap().revision,
        canvas.layout.revision
    );
    canvas.settle();
    assert_eq!(canvas.layout.nodes, positions);
    check_ports(&canvas.layout);

    // A replacement folded projection can have the same revision.
    let other = Canvas::new(true, Zoom::Fit);
    let (sender, receiver) = mpsc::channel();
    canvas.worker.running = Some(RoutingJob {
        topology: Arc::clone(&other.layout.topology),
        revision: canvas.layout.revision,
        receiver,
    });
    sender.send(Err("superseded projection".into())).unwrap();
    canvas.frame(vec![]);
    assert!(canvas.worker.running.is_none());
    assert_eq!(canvas.layout.route_error(), None);
}

#[test]
fn focus_loss_finishes_a_drag_at_its_last_owned_position() {
    let mut canvas = Canvas::new(false, Zoom::Fit);
    canvas.frame(vec![]);
    let start = Canvas::glyphs(&canvas.frame(vec![]))[0].center();
    canvas.frame(vec![Event::PointerMoved(start), button(start, true)]);
    canvas.frame(vec![Event::PointerMoved(start + vec2(20.0, 12.0))]);
    assert!(canvas.layout.drag.is_some());
    let last = canvas.layout.nodes.clone();
    canvas.focused = false;
    let elsewhere = start + vec2(250.0, 30.0);
    canvas.frame(vec![
        Event::WindowFocused(false),
        Event::PointerMoved(elsewhere),
    ]);
    assert!(canvas.layout.drag.is_none());
    canvas.focused = true;
    canvas.frame(vec![Event::WindowFocused(true), button(elsewhere, false)]);
    canvas.settle();
    assert_eq!(canvas.layout.nodes, last);
    check_ports(&canvas.layout);
}
