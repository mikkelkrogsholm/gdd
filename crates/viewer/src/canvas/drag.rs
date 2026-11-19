use super::*;
use std::{sync::mpsc, thread};

pub(super) struct NodeDrag {
    slot: usize,
    widget: egui::Id,
    press: Pos2,
    original: Rect,
    pub(super) bounds: Rect,
    incident: Vec<DragRoute>,
}

struct DragRoute {
    index: usize,
    source_moves: bool,
    target_moves: bool,
    points: Vec<Pos2>,
}

impl DragRoute {
    fn new(index: usize, edge: &graphlay::Edge, route: &Route, slot: usize) -> Option<Self> {
        let source_moves = edge.source.index() == slot;
        let target_moves = edge.target.index() == slot;
        if !source_moves && !target_moves {
            return None;
        }
        let mut points = route.points.clone();
        points.dedup();
        if source_moves != target_moves && points.len() < 4 {
            // Keep a short segment on each port to preserve the arrow's approach
            // and the source's departure even when the node crosses its neighbor.
            let start = points[0];
            let end = points[points.len() - 1];
            let stub =
                |a: Pos2, b: Pos2| a + (b - a).normalized() * (a.distance(b) * 0.25).min(8.0);
            let first = stub(start, points[1]);
            let last = stub(end, points[points.len() - 2]);
            points.insert(1, first);
            points.insert(points.len() - 1, last);
        }
        Some(Self {
            index,
            source_moves,
            target_moves,
            points,
        })
    }

    fn preview(&self, delta: Vec2) -> Option<Route> {
        let points = if self.source_moves && self.target_moves {
            self.points.iter().map(|&point| point + delta).collect()
        } else {
            // Move each port and its neighbor together. Interpolate the interior
            // of the existing lane until the worker routes around obstacles.
            let intervals = (self.points.len() - 3) as f32;
            self.points
                .iter()
                .enumerate()
                .map(|(i, &point)| {
                    let t = (i.saturating_sub(1) as f32 / intervals).min(1.0);
                    point + delta * if self.source_moves { 1.0 - t } else { t }
                })
                .collect()
        };
        Route::from_points(points)
    }
}

impl Layout {
    pub(super) fn update_drag(
        &mut self,
        ui: &egui::Ui,
        scene: &Scene<'_>,
        scale: f32,
        bounds: Rect,
    ) -> bool {
        let (focused, down, press, pointer) = ui.input(|i| {
            let down = i.pointer.primary_down();
            let pointer = if down {
                i.pointer.interact_pos()
            } else if i.pointer.primary_released() {
                // The OS may deliver another pointer move after the release in
                // this same frame. The button event owns the final position.
                i.events.iter().rev().find_map(|event| match event {
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed: false,
                        ..
                    } => Some(*pos),
                    _ => None,
                })
            } else {
                None
            };
            (i.focused, down, i.pointer.press_origin(), pointer)
        });
        if !focused {
            self.end_drag();
            return false;
        }
        if down
            && let Some(id) = ui.ctx().drag_started_id()
            && let Some(press) = press
        {
            // egui arbitrates the press among nodes, badges and background pan.
            // Resolve the widget that actually won, including overlapping figures.
            let node = scene
                .visible
                .nodes
                .iter()
                .find(|&&i| ui.scope_id().with(("canvas-node", i)) == id);
            if let Some(slot) = node.and_then(|&i| scene.glyph(i)) {
                self.drag = Some(NodeDrag {
                    slot,
                    widget: id,
                    press,
                    original: self.nodes[slot],
                    bounds,
                    incident: self
                        .topology
                        .edges
                        .iter()
                        .zip(&self.routes)
                        .enumerate()
                        .filter_map(|(i, (edge, route))| DragRoute::new(i, edge, route, slot))
                        .collect(),
                });
            }
        }
        let Some(drag) = &self.drag else { return false };
        let owner = if down {
            ui.ctx().dragged_id()
        } else {
            ui.ctx().drag_stopped_id()
        };
        if owner != Some(drag.widget) {
            self.end_drag();
            return false;
        }
        let mut moved = false;
        if let Some(pointer) = pointer {
            let delta = (pointer - drag.press) / scale;
            let rect = drag.original.translate(delta);
            if self.nodes[drag.slot] != rect {
                self.nodes[drag.slot] = rect;
                for incident in &drag.incident {
                    if let Some(route) = incident.preview(delta) {
                        self.routes[incident.index] = route;
                    }
                }
                self.revision += 1;
                self.needs_routing = true;
                self.route_error = None;
                moved = true;
            }
        }
        if !down {
            self.end_drag();
        }
        if moved || !down {
            ui.ctx().request_repaint();
        }
        moved
    }

    pub fn end_drag(&mut self) {
        if self.drag.take().is_some() {
            self.update_bounds();
        }
    }

    fn update_bounds(&mut self) {
        self.bounds = self
            .nodes
            .iter()
            .copied()
            .chain(self.routes.iter().map(|r| r.bounds))
            .fold(Rect::NOTHING, Rect::union);
    }

    pub fn route_error(&self) -> Option<&str> {
        self.route_error.as_deref()
    }
}

#[derive(Default)]
pub struct RouteWorker {
    running: Option<RoutingJob>,
}

struct RoutingJob {
    topology: Arc<graphlay::Graph>,
    revision: u64,
    receiver: mpsc::Receiver<Result<Vec<Route>, String>>,
}

impl RouteWorker {
    pub fn update(&mut self, layout: &mut Layout, context: &egui::Context) {
        if context.input(|input| input.focused && input.pointer.primary_down()) {
            // A result may arrive between the press and egui's drag threshold.
            // Keep that initial frame stable until the gesture is resolved.
            return;
        }
        if let Some(job) = &self.running {
            let result = match job.receiver.try_recv() {
                Ok(result) => result,
                Err(mpsc::TryRecvError::Empty) => return,
                Err(mpsc::TryRecvError::Disconnected) => Err("The routing worker stopped.".into()),
            };
            if Arc::ptr_eq(&job.topology, &layout.topology)
                && job.revision == layout.revision
                && layout.drag.is_none()
            {
                match result {
                    Ok(routes) => {
                        layout.routes = routes;
                        layout.update_bounds();
                    }
                    Err(error) => layout.route_error = Some(error),
                }
                layout.needs_routing = false;
                context.request_repaint();
            }
            self.running = None;
        }
        if !layout.needs_routing || layout.drag.is_some() {
            return;
        }
        let topology = Arc::clone(&layout.topology);
        let centers: Vec<_> = layout.nodes.iter().map(|r| r.center()).collect();
        let (sender, receiver) = mpsc::channel();
        let repaint = context.clone();
        let worker = thread::Builder::new()
            .name("graph-reroute".into())
            .spawn(move || {
                let result = std::panic::catch_unwind(|| reroute(&topology, centers))
                    .unwrap_or_else(|_| Err("The routing worker panicked.".into()));
                if sender.send(result).is_ok() {
                    repaint.request_repaint();
                }
            });
        match worker {
            Ok(_) => {
                self.running = Some(RoutingJob {
                    topology: Arc::clone(&layout.topology),
                    revision: layout.revision,
                    receiver,
                })
            }
            Err(error) => {
                layout.route_error = Some(format!("Could not start the routing worker: {error}"));
                layout.needs_routing = false;
                context.request_repaint();
            }
        }
    }
}

fn reroute(topology: &graphlay::Graph, centers: Vec<Pos2>) -> Result<Vec<Route>, String> {
    let mut graph = topology.clone();
    for (node, center) in graph.nodes.iter_mut().zip(centers) {
        node.position = Some(graphlay::Point::new(center.x.into(), center.y.into()));
    }
    let drawing =
        graphlay::reroute(&graph, &layout_options(graph.edges.len())).map_err(|e| e.to_string())?;
    drawing
        .edges
        .into_iter()
        .enumerate()
        .map(|(index, edge)| {
            Route::from_points(edge.points.into_iter().map(position).collect())
                .ok_or_else(|| format!("Route {} has no drawable path.", index + 1))
        })
        .collect()
}

#[cfg(test)]
mod tests;
