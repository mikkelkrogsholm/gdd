//! Finder precedence, inventory consistency and the code observation contract.

use super::project::{FROZEN, Project, expect_verdict, require_diagnostic, rows};
use anyhow::{Result, bail, ensure};
use gdd::code_check::{CodeCheckOptions, check_code_with_capture_hook};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

pub(super) fn frozen_precedence(_: &Path, module: &str) -> Result<()> {
    let names: BTreeSet<_> = FROZEN.iter().map(|s| (*s).to_owned()).collect();
    let file = format!("{module}.py");
    let sources = BTreeMap::from([
        (
            "main.py".to_owned(),
            format!("import {module}").into_bytes(),
        ),
        (file, b"value = 1".to_vec()),
    ]);
    let analyze = |sources: &BTreeMap<String, Vec<u8>>| {
        let components = sources
            .keys()
            .map(|name| (name.clone(), name.clone()))
            .collect();
        gdd::python_imports::analyze_imports(sources, &components, &names)
    };
    let report = analyze(&sources);
    ensure!(
        rows(&report, "edges")?.is_empty(),
        "absolute import incorrectly resolved to local source"
    );
    let imports = rows(&report, "imports")?;
    ensure!(
        !imports.is_empty() && imports.iter().all(|r| r["external"] == true),
        "missing external import"
    );
    let target = format!("app/{module}.py");
    let relative = analyze(&BTreeMap::from([
        ("app/__init__.py".into(), vec![]),
        (
            "app/main.py".into(),
            format!("from .{module} import value").into_bytes(),
        ),
        (target.clone(), b"value = 1".to_vec()),
    ]));
    ensure!(
        rows(&relative, "edges")?
            .iter()
            .any(|e| e["target_file"] == target),
        "relative import lost its local target"
    );
    Ok(())
}

pub(super) fn excluded_import(_: &Path, _: &str) -> Result<()> {
    let project = Project::flat()?;
    project.write("api.py", "import helper\n")?;
    project.write("helper.py", "import store\n")?;
    project.expect("UNKNOWN")?;
    let report = project.expect_with(
        CodeCheckOptions {
            exclude: &["helper.py".into()],
            ..project.options()
        },
        "UNKNOWN",
    )?;
    require_diagnostic(&report, "CODE_IMPORT_UNRESOLVED")
}

pub(super) fn inventory_drift(_: &Path, _: &str) -> Result<()> {
    let project = Project::flat()?;
    project.write("api.py", "import store\n")?;
    project.expect("FAIL")?;
    let report = check_code_with_capture_hook(&project.path, project.options(), || {
        fs::write(project.source.join("api.py"), "pass\n")
    })?;
    expect_verdict(&report, "UNKNOWN")?;
    for check in rows(&report, "checks")? {
        ensure!(
            check
                .get("path")
                .is_none_or(|v| v.as_array().is_some_and(Vec::is_empty)),
            "drift produced a witness"
        );
    }
    require_diagnostic(&report, "CODE_SOURCE_CHANGED")?;
    project.expect("PASS")?;
    Ok(())
}

pub(super) fn output_schema(directory: &Path, parameter: &str) -> Result<()> {
    let (source, expected) = match parameter {
        "clean" => ("pass\n", "PASS"),
        "violation" => ("import store\n", "FAIL"),
        "relative" => ("from . import store\n", "UNKNOWN"),
        "syntax" => ("def invalid(\n", "UNKNOWN"),
        _ => bail!("unknown schema control {parameter}"),
    };
    let project = Project::flat()?;
    project.write("api.py", source)?;
    let report = project.expect(expected)?;
    let schema: Value = serde_json::from_slice(&fs::read(
        directory.join("../schemas/code-observation.schema.json"),
    )?)?;
    gdd::validation::build_schema(&schema)
        .map_err(|error| anyhow::anyhow!("{error}"))?
        .validate(&report)
        .map_err(|error| anyhow::anyhow!("{error}"))?;
    ensure!(
        report["format"] == "gdd-code-observation/0.1",
        "unexpected observation format"
    );
    ensure!(
        report["runtime"]["python"]
            .as_str()
            .is_some_and(|s| !s.is_empty()),
        "missing Python target version"
    );
    for check in rows(&report, "checks")? {
        ensure!(
            check.get("evidence_refs").is_none(),
            "unexpected evidence reference"
        );
    }
    Ok(())
}
