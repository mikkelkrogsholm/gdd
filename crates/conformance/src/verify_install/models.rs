//! Inspect both model versions, derived views, product gaps and declared boundaries.

use super::{Program, Runner, check_json, read_json};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::{collections::BTreeSet, fs};

const CURRENT: &str = "examples/concert-0.3.gdd.yaml";
const RESERVE: &str = "action.reserve";

pub(super) fn check(run: &mut Runner) -> Result<()> {
    let expected_boundaries =
        read_json(&run.recipient.join("examples/concert-0.3.boundaries.json"))?;
    let schema = read_json(&run.recipient.join("schemas/perspectives.schema.json"))?;
    let validator = gdd::validation::build_schema(&schema)?;
    for (name, model, version, distributed) in [
        (
            "legacy",
            "examples/concert.gdd.yaml",
            "gdd/0.2",
            "examples/concert.gddview",
        ),
        (
            "current",
            CURRENT,
            "gdd/0.3",
            "examples/concert-0.3.gddview",
        ),
    ] {
        run.case(&format!("{name}-model"), 0, |case| {
            let report = case.compare(&["validate", model])?;
            let parsed = gdd::input::parse(&fs::read(case.recipient.join(model))?)?;
            let document = gdd::validation::to_json(&parsed)?;
            ensure!(
                document["format"] == version,
                "{model} must exercise {version}"
            );
            ensure!(
                report["format"] == "gdd-report/0.2",
                "expected a model validation report"
            );
            Ok(())
        })?;
        run.compare(
            &format!("{name}-readiness"),
            0,
            &[
                "validate",
                model,
                "--profile",
                "readiness",
                "--scope",
                RESERVE,
            ],
        )?;
        run.compare(&format!("{name}-snapshot"), 0, &["snapshot", model])?;
        run.compare(
            &format!("{name}-context"),
            0,
            &["context", model, "--scope", RESERVE],
        )?;
        let artifact = format!("{name}.gddview");
        run.case(&format!("{name}-view"), 0, |case| {
            let reference = format!("{name}-reference.gddview");
            let installed =
                case.json(Program::Installed, &["view", model, "--output", &artifact])?;
            let bytes =
                fs::read(case.recipient.join(&artifact)).context("read installed native view")?;
            let report = case.json(Program::Reference, &["view", model, "--output", &reference])?;
            check_json("native view generation reports", &report, &installed)?;
            let reference_bytes = fs::read(case.recipient.join(reference)).context("read reference native view")?;
            ensure!(
                bytes == reference_bytes,
                "independently generated {name} native view bytes differ; installed SHA-256 {}, reference SHA-256 {}",
                gdd::sha256(&bytes), gdd::sha256(&reference_bytes)
            );
            let shipped = fs::read(case.recipient.join(distributed))
                .with_context(|| format!("read distributed native view {distributed}"))?;
            ensure!(
                bytes == shipped,
                "stale distributed native view {distributed}: shipped SHA-256 {}, regenerated SHA-256 {}",
                gdd::sha256(&shipped), gdd::sha256(&bytes)
            );
            Ok(())
        })?;
        run.compare(
            &format!("{name}-verify-view"),
            0,
            &[
                "validate",
                model,
                "--profile",
                "visualization",
                "--view",
                &artifact,
            ],
        )?;
    }

    let mut greenfield = Value::Null;
    for scenario in ["greenfield", "partial", "refactoring"] {
        let model = format!("examples/product/{scenario}.gdd.json");
        run.case(&format!("product-perspectives-{scenario}"), 0, |case| {
            let projection = case.compare(&["perspectives", &model])?;
            validator
                .validate(&projection)
                .map_err(|error| anyhow::anyhow!("perspectives schema: {error}"))?;
            ensure!(
                projection["format"] == "gdd-perspectives/0.1"
                    && projection["advisory"] == true
                    && projection["product_extension"]["version"] == "0.1.0",
                "expected advisory product perspectives 0.1.0"
            );
            match scenario {
                "greenfield" => {
                    ensure!(
                        projection["gaps"] == json!([]),
                        "greenfield has unexpected advisory gaps"
                    );
                    greenfield = projection;
                }
                "partial" => partial_gaps(&projection)?,
                _ => {
                    ensure!(
                        projection["spec_revision"] != greenfield["spec_revision"],
                        "refactoring kept the same spec revision"
                    );
                    let journey = &greenfield["views"]["journey"];
                    let revised = &projection["views"]["journey"];
                    ensure!(
                        journey["primary_node_ids"] == revised["primary_node_ids"],
                        "refactoring changed primary journey IDs"
                    );
                    for id in journey["primary_node_ids"]
                        .as_array()
                        .context("missing primary journey IDs")?
                    {
                        let id = id.as_str().context("expected a journey node ID")?;
                        let original = journey["nodes"]
                            .get(id)
                            .context("primary journey node is missing")?;
                        ensure!(
                            revised["nodes"].get(id) == Some(original),
                            "refactoring changed primary journey node {id}"
                        );
                    }
                }
            }
            Ok(())
        })?;
    }
    run.case("current-boundaries", 0, |case| {
        let boundaries = case.compare(&["boundaries", CURRENT])?;
        check_json(
            "derived boundaries and examples/concert-0.3.boundaries.json",
            &expected_boundaries,
            &boundaries,
        )
    })?;
    run.case("planned-review-unknown", 2, |case| {
        let review = case.compare(&[
            "design-review",
            CURRENT,
            "--review",
            "examples/design-review.planned.json",
            "--implementation",
            "implementation.concert.python",
            "--code-revision",
            "planned",
            "--scope",
            RESERVE,
        ])?;
        ensure!(
            review["status"] == "UNKNOWN",
            "expected UNKNOWN for a planned review"
        );
        Ok(())
    })?;
    run.compare(
        "legacy-native-acceptance",
        0,
        &[
            "validate",
            "examples/agent-native/model.gdd.json",
            "--profile",
            "records",
            "--activity",
            "acceptance",
            "--scope",
            "action.quote",
            "--host-context",
            "examples/agent-native/host.acceptance.json",
        ],
    )?;
    Ok(())
}

fn partial_gaps(projection: &Value) -> Result<()> {
    let gaps = projection["gaps"].as_array().context("missing gaps")?;
    let mut observed = BTreeSet::new();
    for gap in gaps {
        let code = gap["code"].as_str().context("missing gap code")?;
        let node = gap["node_id"].as_str().context("missing gap node ID")?;
        ensure!(
            observed.insert((code, node)),
            "duplicate {code} gap at {node}"
        );
    }
    let expected = BTreeSet::from([
        ("MISSING_DATA_COMPONENT", "entity.reservation"),
        ("MISSING_ACTOR", "step.reserve"),
        ("MISSING_USER_ACTION", "step.reserve"),
        ("MISSING_SYSTEM_RESPONSE", "step.reserve"),
        ("MISSING_RESPONSIBLE_COMPONENT", "step.reserve"),
    ]);
    ensure!(
        observed == expected,
        "unexpected partial-model gap identities: {observed:?}"
    );
    Ok(())
}
