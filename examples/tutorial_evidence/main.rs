//! Run and explicitly attest the native integer addition tutorial.

mod checks;
#[cfg(test)]
mod tests;
#[path = "../tutorial_plan/mod.rs"]
mod tutorial_plan;

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};
use gdd::host::{self, Attestation};
use serde_json::{Value, json};

#[derive(Parser)]
#[command(about = "Execute and review the Rust integer addition example")]
struct Args {
    #[command(subcommand)]
    stage: Stage,
}

#[derive(Subcommand)]
enum Stage {
    /// Copy the standalone tutorial package, make its mutant, and run both.
    Run { plan: PathBuf, source: PathBuf },
    /// Append current evidence after personally reviewing both runs.
    Record {
        plan: PathBuf,
        #[arg(long)]
        attest_reviewed_checks: bool,
    },
    /// Change a separate copy and observe the historical receipt becoming stale.
    Drift { plan: PathBuf },
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn read(path: &Path) -> io::Result<Value> {
    let bytes = fs::read(path)?;
    let value = gdd::input::parse(&bytes).map_err(io::Error::other)?;
    Ok(gdd::validation::to_json(&value)?)
}

fn write_json(mut file: File, value: &Value) -> io::Result<()> {
    serde_json::to_writer_pretty(&mut file, value)?;
    writeln!(file)
}

// Reserve each stage directory exclusively. Failed stages roll back their own files.
fn stage_dir(parent: &Path, name: &str) -> io::Result<tempfile::TempDir> {
    tempfile::Builder::new()
        .prefix(name)
        .rand_bytes(0)
        .tempdir_in(parent)
}

fn prepared(plan: &Path) -> io::Result<PathBuf> {
    let plan = plan.canonicalize()?;
    let model = plan.join("evidence.gdd.json");
    let revision = gdd::bundle::load(&model)
        .snapshot()
        .ok_or_else(|| invalid("Unresolved tutorial model."))?
        .revision
        .clone();
    if read(&model)? != tutorial_plan::model()
        || read(&plan.join("records.json"))?["records"]["check.sum"]
            != tutorial_plan::check(&revision)
    {
        return Err(invalid(
            "Use the integer addition plan from tutorial_prepare.",
        ));
    }
    Ok(plan)
}

fn attest(
    plan: &Path,
    source: &Path,
    runner: &Value,
    binding: &str,
    evidence: &str,
) -> io::Result<Value> {
    let environment = BTreeMap::from([(
        "rustc".into(),
        runner["rustc"]
            .as_str()
            .ok_or_else(|| invalid("Missing rustc version."))?
            .to_owned(),
    )]);
    host::verification_records(
        &plan.join("evidence.gdd.json"),
        "check.sum",
        &plan.join("runs/pass.receipt.json"),
        &Attestation {
            root: source,
            implementation: "implementation.integer-addition.rust",
            binding_id: binding,
            evidence_id: evidence,
            test_resource: "src/main.rs",
            checker: &json!({"name": "cargo run --bin gdd-tutorial-add", "version": runner["cargo"]}),
            environment: &environment,
            verified_negative_controls: &[tutorial_plan::CONTROL.to_owned()],
        },
    )
}

fn record(plan: &Path, attested: bool) -> io::Result<()> {
    if !attested {
        return Err(invalid(
            "Review both receipts and sources, then pass --attest-reviewed-checks.",
        ));
    }
    let plan = prepared(plan)?;
    let runs = plan.join("runs");
    let runner = checks::review(&runs)?;
    let fragment = attest(
        &plan,
        &runs.join("current"),
        &runner,
        "binding.sum.v1",
        "evidence.sum.v1",
    )?;
    let review = stage_dir(&plan, "review")?;
    write_json(
        File::create_new(review.path().join("host-context.json"))?,
        &fragment["host"],
    )?;
    host::append_records(&plan.join("records.json"), &fragment)?;
    let _ = review.keep();
    println!("Appended binding.sum.v1 and evidence.sum.v1. See review/host-context.json.");
    Ok(())
}

fn drift(plan: &Path) -> io::Result<()> {
    let plan = prepared(plan)?;
    let mut context = read(&plan.join("review/host-context.json"))?;
    let runs = plan.join("runs");
    let runner = checks::review(&runs)?;
    let current = runs.join("current");
    // Establish that these new IDs and the current receipt are otherwise valid.
    attest(
        &plan,
        &current,
        &runner,
        "binding.sum.v2",
        "evidence.sum.v2",
    )?;
    let drift = stage_dir(&plan, "drift")?;
    let changed = drift.path().join("changed");
    checks::copy_sources(&current, &changed)?;
    writeln!(
        File::options()
            .append(true)
            .open(changed.join("src/lib.rs"))?,
        "// Local revision change."
    )?;
    let snapshot = host::snapshot_code(&changed, checks::FILES)?;
    if context["code_revision"] == snapshot.revision {
        return Err(invalid("Expected a new code revision."));
    }
    context["code_revision"] = json!(snapshot.revision);
    match attest(
        &plan,
        &changed,
        &runner,
        "binding.sum.v2",
        "evidence.sum.v2",
    ) {
        Ok(_) => {
            return Err(invalid(
                "The historical receipt unexpectedly covered changed sources.",
            ));
        }
        Err(error) if error.to_string() == "The selected code snapshot is stale or unresolved." => {
            println!("Old receipt rejected: {error}")
        }
        Err(error) => return Err(error),
    }
    write_json(
        File::create_new(drift.path().join("host-context.json"))?,
        &context,
    )?;
    let _ = drift.keep();
    Ok(())
}

fn main() -> io::Result<()> {
    match Args::parse().stage {
        Stage::Run { plan, source } => checks::run(&prepared(&plan)?, &source),
        Stage::Record {
            plan,
            attest_reviewed_checks,
        } => record(&plan, attest_reviewed_checks),
        Stage::Drift { plan } => drift(&plan),
    }
}
