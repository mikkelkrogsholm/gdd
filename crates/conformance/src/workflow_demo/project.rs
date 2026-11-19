//! Fixed, compiled-in inputs for the local Rust implementation of the quote contract.

use super::write_json;
use anyhow::{Context, Result, ensure};
use gdd::host::{CodeSnapshot, snapshot_code};
use serde_json::{Value, json};
use std::{fs, path::Path};

macro_rules! bundled {
    ($path:literal) => {
        include_bytes!(concat!("../../../../examples/agent-native/", $path))
    };
}

macro_rules! rust_files {
    ($($path:literal),+ $(,)?) => {
        &[$(($path, include_bytes!(concat!("../../../../examples/agent-native/rust/", $path)) as &[u8])),+]
    };
}

const SOURCES: &[(&str, &[u8])] = rust_files![
    "Cargo.toml",
    "Cargo.lock",
    "src/lib.rs",
    "src/main.rs",
    "src/discount.rs",
    "src/tax.rs",
    "src/quote.rs",
    "tests/acceptance.rs",
    "tests/scenarios/leaf.rs",
    "tests/scenarios/quote_common.rs",
    "tests/scenarios/quote.rs",
];
const BASELINE: &[(&str, &[u8])] = rust_files![
    "baseline/src/discount.rs",
    "baseline/src/tax.rs",
    "baseline/tests/scenarios/quote.rs",
];

pub(super) const IMPLEMENTATION: &str = "implementation.quote_demo.rust";
pub(super) const NEGATIVE_CONTROL: &str = "Run the shipped half-up baseline against unchanged floor assertions; the rounding tests and actual CLI integration must detect the defect.";

pub(super) fn source_names() -> Vec<&'static str> {
    SOURCES.iter().map(|&(name, _)| name).collect()
}

pub(super) fn snapshot(root: &Path) -> Result<CodeSnapshot> {
    Ok(snapshot_code(root, &source_names())?)
}

fn put(root: &Path, name: &str, bytes: &[u8]) -> Result<()> {
    let path = root.join(name);
    fs::create_dir_all(path.parent().context("missing resource directory")?)?;
    Ok(fs::write(path, bytes)?)
}

/// The detached project keeps the original three baseline files. Its submitted
/// resources are the verified final versions, copied into place at integration.
pub(super) fn baseline(root: &Path) -> Result<()> {
    for &(name, bytes) in SOURCES {
        put(root, name, bytes)?;
    }
    for &(overlay, bytes) in BASELINE {
        put(root, overlay, bytes)?;
        let resource = overlay
            .strip_prefix("baseline/")
            .context("invalid overlay")?;
        let submitted = root.join("submissions").join(resource);
        fs::create_dir_all(submitted.parent().context("missing submission directory")?)?;
        fs::copy(root.join(resource), submitted)?;
        put(root, resource, bytes)?;
    }
    Ok(())
}

pub(super) fn model(root: &Path) -> Result<String> {
    let mut model: Value = serde_json::from_slice(bundled!("model.gdd.json"))?;
    for rule in [
        "rule.discount_floor",
        "rule.tax_floor",
        "rule.quote_composition",
        "rule.input",
    ] {
        let method = &mut model["nodes"][rule]["rule"]["verification"];
        ensure!(
            *method == json!(["stdlib_unittest"]),
            "unexpected checker on {rule}"
        );
        *method = json!(["cargo_test"]);
    }
    let path = root.join("model.gdd.json");
    write_json(&path, &model)?;
    fs::write(root.join("REQUIREMENTS.md"), bundled!("REQUIREMENTS.md"))?;
    let ledger = root.join("model.records.json");
    let mut records: Value = serde_json::from_slice(bundled!("model.records.plan.json"))?;
    write_json(
        &ledger,
        &json!({"format":records["format"], "project_id":records["project_id"], "records":{}}),
    )?;
    let bundle = gdd::bundle::load(&path);
    let snapshot = bundle.snapshot().context("unresolved Rust quote model")?;
    write_json(&root.join("model.snapshot.json"), snapshot)?;
    let revision = &snapshot.revision;

    for (id, record) in records["records"]
        .as_object_mut()
        .context("missing demo records")?
    {
        record["spec_revision"] = json!(revision);
        if id == "check.demo.all" {
            record["method"] = json!("cargo_test");
            record["required_environment"] = json!(["cargo", "rustc"]);
            ensure!(
                record["negative_controls"] == json!([NEGATIVE_CONTROL]),
                "unexpected control"
            );
            continue;
        }
        record["implementation"] = json!(IMPLEMENTATION);
        record["resource"] = json!(match id.as_str() {
            "binding.discount.code" => "src/discount.rs",
            "binding.tax.code" => "src/tax.rs",
            "binding.quote.code" => "src/quote.rs",
            "binding.tax.test" | "binding.discount.test" => "tests/scenarios/leaf.rs",
            "binding.quote.test" => "tests/scenarios/quote.rs",
            _ => anyhow::bail!("unexpected planned record {id}"),
        });
    }
    write_json(&ledger, &records)?;
    Ok(revision.clone())
}

pub(super) fn request(code: &str) -> Result<Value> {
    let mut request: Value = serde_json::from_slice(bundled!("workflow.request.json"))?;
    request["implementation"] = json!(IMPLEMENTATION);
    request["base_code_revision"] = json!(code);
    for package in request["packages"]
        .as_array_mut()
        .context("missing demo packages")?
    {
        let mut reads = vec![
            "REQUIREMENTS.md",
            "model.gdd.json",
            "model.records.json",
            "Cargo.toml",
            "Cargo.lock",
            "src/lib.rs",
            "tests/acceptance.rs",
        ];
        let writes = match package["id"].as_str() {
            Some("package.discount") => vec!["src/discount.rs"],
            Some("package.tax") => vec!["src/tax.rs"],
            Some("package.quote") => {
                vec!["src/quote.rs", "src/main.rs", "tests/scenarios/quote.rs"]
            }
            _ => anyhow::bail!("unexpected demo package"),
        };
        if package["id"] == "package.quote" {
            reads.extend([
                "src/discount.rs",
                "src/tax.rs",
                "tests/scenarios/quote_common.rs",
            ]);
        } else {
            reads.push("tests/scenarios/leaf.rs");
            package["integration_plan"] = json!(
                "Repair the owned leaf module to the floor contract, preserve input validation and run the corresponding five Cargo acceptance tests before integrating."
            );
        }
        package["read_resources"] = json!(reads);
        package["write_resources"] = json!(writes);
        package["deliverables"] = json!(writes);
    }
    Ok(request)
}

pub(super) fn host(spec: &str, code: &str) -> Result<Value> {
    let mut host: Value = serde_json::from_slice(bundled!("host.implementation.json"))?;
    host["implementation"] = json!(IMPLEMENTATION);
    host["code_revision"] = json!(code);
    host["policy"]["spec_revision"] = json!(spec);
    Ok(host)
}
