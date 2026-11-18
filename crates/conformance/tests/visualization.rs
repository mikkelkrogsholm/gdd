//! Exercise the published CLI, receipts, and diagnostic distinctions without helper executables.

use anyhow::Result;
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs, path::Path, process::Command};

#[test]
fn named_visualization_cases_run_offline_from_an_empty_directory() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let mut command = Command::new(env!("CARGO_BIN_EXE_gdd-conformance"));
    command
        .current_dir(temp.path())
        .env("PATH", "")
        .arg("visualization");
    let stdout = command.output()?;
    assert!(
        stdout.status.success(),
        "{}",
        String::from_utf8_lossy(&stdout.stderr)
    );
    let receipt: Value = serde_json::from_slice(&stdout.stdout)?;
    assert_eq!(receipt["format"], "gdd-view-conformance/0.1");
    assert_eq!(receipt["profile"], gdd::view::PROFILE);
    assert!(receipt["executed_at"].as_str().unwrap().ends_with('Z'));
    assert_eq!(receipt["total"], 16);
    assert_eq!(receipt["matched"], 16);

    const MISMATCH: &str = "VIEW_ARTIFACT_MISMATCH";
    let expected = [
        ("concert", "PASS", None),
        ("fragments", "PASS", None),
        ("extension", "PASS", None),
        ("scope", "PASS", None),
        ("hostile_multigraph", "PASS", None),
        ("unknown_renderer", "UNKNOWN", Some("FORMAT_UNSUPPORTED")),
        ("stale_revision", "FAIL", Some("VIEW_REVISION_MISMATCH")),
        ("changed_direction", "FAIL", Some(MISMATCH)),
        ("omitted_parallel_edge", "FAIL", Some(MISMATCH)),
        ("missing_manifest_ids", "FAIL", Some(MISMATCH)),
        ("changed_script", "FAIL", Some(MISMATCH)),
        ("blank_html", "FAIL", Some(MISMATCH)),
        ("missing_input", "UNKNOWN", Some("VIEW_INPUT_MISSING")),
        (
            "unavailable_artifact",
            "UNKNOWN",
            Some("SOURCE_UNAVAILABLE"),
        ),
        ("changed_newlines", "FAIL", Some(MISMATCH)),
        ("deep_payload", "UNKNOWN", Some("FORMAT_UNSUPPORTED")),
    ];
    let rows = receipt["results"].as_array().unwrap();
    assert_eq!(rows.len(), expected.len());
    let checkout = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let fixture = temp.path().join("model.json");
    fs::copy(
        checkout.join("conformance/fixtures/viewer-multigraph.json"),
        &fixture,
    )?;
    let mut revisions = BTreeMap::new();
    for (base, path) in [
        ("fixture", fixture),
        ("concert", checkout.join("examples/concert.gdd.yaml")),
        ("fragmented", checkout.join("examples/fragmented.gdd.yaml")),
        ("extension", checkout.join("examples/extension.gdd.yaml")),
    ] {
        let captured = gdd::perspectives::load_perspective_bundle(&path)?;
        revisions.insert(base, captured.bundle().snapshot().unwrap().revision.clone());
    }
    for (id, status, diagnostic) in expected {
        let row = rows
            .iter()
            .find(|r| r["id"] == format!("viewer.{id}"))
            .unwrap();
        let base = match id {
            "concert" | "scope" => "concert",
            "fragments" => "fragmented",
            "extension" => "extension",
            _ => "fixture",
        };
        assert_eq!(row["base"], base, "{id}");
        assert_eq!(row["report"]["spec_revision"], revisions[base], "{id}");
        let check = &row["report"]["checks"][0];
        let scope = if id == "scope" {
            json!(["action.reserve"])
        } else {
            json!([])
        };
        assert_eq!(row["matched"], true, "{id}");
        assert_eq!(row["expected"], status, "{id}");
        assert_eq!(row["actual"], status, "{id}");
        assert_eq!(row["scope"], scope, "{id}");
        assert_eq!(row["report"]["scope"], scope, "{id}");
        assert_eq!(check["scope"], scope, "{id}");
        assert_eq!(check["check_id"], "integration.visualization", "{id}");
        assert_eq!(check["result"], status, "{id}");
        assert_eq!(check["diagnostics"][0]["code"].as_str(), diagnostic, "{id}");
    }

    command.args(["--output", "new.json"]);
    let run = command.output()?;
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let path = temp.path().join("new.json");
    let bytes = fs::read(&path)?;
    let saved: Value = serde_json::from_slice(&bytes)?;
    assert_eq!(saved["matched"], 16);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let ordinary = temp.path().join("ordinary.json");
        fs::write(&ordinary, b"{}")?;
        assert_eq!(
            fs::metadata(&path)?.permissions().mode() & 0o777,
            fs::metadata(ordinary)?.permissions().mode() & 0o777
        );
    }
    let collision = command.output()?;
    assert!(!collision.status.success());
    assert!(String::from_utf8_lossy(&collision.stderr).contains("create receipt new.json"));
    assert_eq!(fs::read(path)?, bytes);
    Ok(())
}
