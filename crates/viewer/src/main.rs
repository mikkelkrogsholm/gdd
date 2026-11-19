mod canvas;
mod model;
mod viewer;

use std::ffi::{OsStr, OsString};
use std::path::Path;
use std::process::ExitCode;

use eframe::egui;

use model::Graph;
use viewer::Viewer;

const USAGE: &str = "Usage: gdd-viewer <MODEL.gdd.json | MODEL.gdd.yaml | FILE.gddview> [--work REPORT.json] [--wiki KNOWLEDGE.json]";

struct Inputs<'a> {
    model: &'a Path,
    work: Option<&'a Path>,
    wiki: Option<&'a Path>,
}

fn inputs(args: &[OsString]) -> Option<Inputs<'_>> {
    let mut model = None;
    let mut work = None;
    let mut wiki = None;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        if arg == "--work" || arg == "--wiki" {
            let slot = if arg == "--work" {
                &mut work
            } else {
                &mut wiki
            };
            let value = args.next()?;
            if value == "--wiki" || value == "--work" || slot.replace(Path::new(value)).is_some() {
                return None;
            }
        } else if model.replace(Path::new(arg)).is_some() {
            return None;
        }
    }
    Some(Inputs {
        model: model?,
        work,
        wiki,
    })
}

fn run(inputs: Inputs<'_>) -> Result<(), String> {
    let path = inputs.model;
    let mut graph = Graph::load(path)?;
    if let Some(report) = inputs.work {
        graph.load_work_report(report)?;
    }
    graph.load_knowledge(path, inputs.wiki)?;
    let filename = path
        .file_name()
        .unwrap_or(path.as_os_str())
        .to_string_lossy();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1480.0, 800.0])
            .with_min_inner_size([320.0, 300.0]),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        "GDD Viewer Preview",
        options,
        Box::new(move |context| {
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
    if matches!(args.as_slice(), [flag] if flag == OsStr::new("--help") || flag == OsStr::new("-h"))
    {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    let Some(inputs) = inputs(&args) else {
        eprintln!("{USAGE}");
        return ExitCode::FAILURE;
    };
    match run(inputs) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("gdd-viewer: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod argument_tests {
    use super::*;

    #[test]
    fn wiki_and_work_may_accompany_the_same_graph() {
        let args = [
            "--wiki",
            "knowledge.json",
            "model.gddview",
            "--work",
            "run.json",
        ]
        .map(OsString::from);
        let parsed = inputs(&args).unwrap();
        assert_eq!(parsed.model, Path::new("model.gddview"));
        assert_eq!(parsed.wiki, Some(Path::new("knowledge.json")));
        assert_eq!(parsed.work, Some(Path::new("run.json")));
        assert!(inputs(&["file", "--wiki", "--work", "report"].map(OsString::from)).is_none());
        assert!(inputs(&["file", "--wiki", "a", "--wiki", "b"].map(OsString::from)).is_none());
    }
}
