mod canvas;
mod model;
mod viewer;

use std::ffi::OsStr;
use std::path::Path;
use std::process::ExitCode;

use eframe::egui;

use model::Graph;
use viewer::Viewer;

const USAGE: &str = "Usage: gdd-viewer <MODEL.gdd.json | MODEL.gdd.yaml>";

fn run(path: &Path) -> Result<(), String> {
    let graph = Graph::load(path)?;
    let filename = path
        .file_name()
        .unwrap_or(path.as_os_str())
        .to_string_lossy();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1480.0, 800.0])
            .with_min_inner_size([850.0, 520.0]),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        "GDD Viewer Preview",
        options,
        Box::new(move |context| {
            context.egui_ctx.set_theme(egui::Theme::Light);
            Ok(Box::new(Viewer::new(
                graph,
                filename.into_owned(),
                context.egui_ctx.clone(),
            )))
        }),
    )
    .map_err(|error| error.to_string())
}

fn main() -> ExitCode {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let [path] = args.as_slice() else {
        eprintln!("{USAGE}");
        return ExitCode::FAILURE;
    };
    if path == OsStr::new("--help") || path == OsStr::new("-h") {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    match run(Path::new(path)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("gdd-viewer: {error}");
            ExitCode::FAILURE
        }
    }
}
