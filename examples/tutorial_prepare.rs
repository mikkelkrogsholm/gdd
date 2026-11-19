//! Prepare the native integer addition check plan from the tutorial evidence model.

use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use clap::Parser;
use gdd::{bundle, host};
use serde_json::{Value, json};

mod tutorial_plan;

#[derive(Parser)]
#[command(about = "Prepare the Rust integer addition model and check plan")]
struct Args {
    /// New directory for the model and its records ledger.
    output: PathBuf,
}

fn write_new(path: &Path, document: &Value) -> io::Result<()> {
    let mut file = File::create_new(path)?;
    serde_json::to_writer_pretty(&mut file, document)?;
    writeln!(file)
}

fn prepare(output: &Path) -> io::Result<()> {
    let model = tutorial_plan::model();
    let ledger = json!({
        "format": "gdd-records/0.2", "project_id": model["project"]["id"], "records": {}
    });

    // Claim the exact directory name exclusively and clean up incomplete output on error.
    let name = output.file_name().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "Supply a new directory name.")
    })?;
    let directory = tempfile::Builder::new()
        .prefix(name)
        .rand_bytes(0)
        .tempdir_in(output.parent().unwrap_or(Path::new(".")))?;
    let model_path = directory.path().join("evidence.gdd.json");
    let records_path = directory.path().join("records.json");
    write_new(&model_path, &model)?;
    write_new(&records_path, &ledger)?;

    let bundle = bundle::load(&model_path);
    let snapshot = bundle.snapshot().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "The model revision is unresolved.",
        )
    })?;
    host::append_records(
        &records_path,
        &json!({"records": {"check.sum": tutorial_plan::check(&snapshot.revision)}}),
    )?;
    let _ = directory.keep();
    Ok(())
}

fn main() -> io::Result<()> {
    let args = Args::parse();
    prepare(&args.output)?;
    println!("Prepared check.sum in {}", args.output.display());
    Ok(())
}

#[test]
fn preparation_creates_only_a_current_check_and_refuses_reuse() {
    use gdd::report::CheckResult;
    use gdd::validation::{Profile, ValidationOptions, validate_with_options};
    use std::fs;
    use tutorial_plan::{METHOD, MODEL};

    let parent = tempfile::tempdir().unwrap();
    let output = parent.path().join("sum");
    prepare(&output).unwrap();

    let model_path = output.join("evidence.gdd.json");
    let records_path = output.join("records.json");
    let model = fs::read_to_string(&model_path).unwrap();
    assert_eq!(
        model,
        MODEL.replace("\"python_unittest\"", &format!("\"{METHOD}\""))
    );
    let records = fs::read(&records_path).unwrap();
    let ledger: Value = serde_json::from_slice(&records).unwrap();
    assert_eq!(ledger["project_id"], "project.integer-addition");
    assert_eq!(ledger["records"].as_object().unwrap().len(), 1);
    let plan = &ledger["records"]["check.sum"];
    let loaded = bundle::load(&model_path);
    assert_eq!(loaded.exit_code(), 0);
    assert_eq!(plan["spec_revision"], loaded.snapshot().unwrap().revision);
    assert_eq!(plan["scope"], json!(["rule.sum", "scenario.add"]));
    assert_eq!(plan["method"], METHOD);
    assert_eq!(plan["required_environment"], json!(["rustc"]));
    assert_eq!(
        plan["negative_controls"],
        json!(["isolated-subtraction-mutant"])
    );
    let report = validate_with_options(
        &loaded,
        ValidationOptions {
            profiles: &[Profile::Records],
            scope: &["action.add".to_owned()],
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(report.result("implementation"), CheckResult::Unknown);
    assert_eq!(report.exit_code(), 2);

    assert_eq!(
        prepare(&output).unwrap_err().kind(),
        io::ErrorKind::AlreadyExists
    );
    assert_eq!(fs::read_to_string(model_path).unwrap(), model);
    assert_eq!(fs::read(records_path).unwrap(), records);
}
