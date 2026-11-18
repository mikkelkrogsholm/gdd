//! Source mutation, reachability and coverage controls.

use super::project::{Project, require_diagnostic, rows, snapshot};
use anyhow::{Context, Result, ensure};
use gdd::code_check::CodeCheckOptions;
use serde_json::{Value, json};
use std::{fs, path::Path};

fn witness(report: &Value) -> Result<&[Value]> {
    rows(report, "checks")?
        .iter()
        .filter(|check| check["result"] == "FAIL")
        .filter_map(|check| check["path"].as_array())
        .find(|path| !path.is_empty())
        .map(Vec::as_slice)
        .context("missing violation witness")
}

pub(super) fn independent_package_clean_injected_repaired(_: &Path, _: &str) -> Result<()> {
    let project = Project::package()?;
    let clean = project.expect("PASS")?;
    ensure!(
        clean["format"] == "gdd-code-observation/0.1",
        "unexpected observation format"
    );
    project.write("pkg/front.py", "from . import store\n")?;
    let broken = project.expect("FAIL")?;
    ensure!(
        broken["code_revision"] != clean["code_revision"],
        "injection did not change code revision"
    );
    ensure!(
        rows(&broken, "edges")?
            .iter()
            .any(|e| e["from"] == "component.front" && e["to"] == "component.store"),
        "missing injected component edge"
    );
    witness(&broken)?;
    project.write("pkg/front.py", "VALUE = 1\n")?;
    ensure!(
        project.expect("PASS")?["code_revision"] == clean["code_revision"],
        "repair did not recover revision"
    );
    for key in ["evidence_refs", "trusted_evidence", "authorization"] {
        ensure!(clean.get(key).is_none(), "unexpected authority field {key}");
    }
    Ok(())
}

pub(super) fn copied_concert_clean_injected_repaired_missing(
    directory: &Path,
    _: &str,
) -> Result<()> {
    let original = directory.join("../examples/concert-app/concert_app");
    let before = snapshot(&original)?;
    let mut project = Project::new(
        "front",
        "store",
        "direct",
        &["front", "middle", "store"],
        "implementation.control",
    )?;
    for (file, bytes) in &before {
        if file.extension().is_some_and(|s| s == "py") {
            project.write(Path::new("concert_app").join(file), bytes)?;
        }
    }
    for (module, component) in [
        ("api", "front"),
        ("storage", "store"),
        ("identity", "middle"),
        ("reservations", "middle"),
        ("__main__", "middle"),
    ] {
        project.bind(module, component, &format!("concert_app/{module}.py"))?;
    }
    project.save_records()?;
    project.expect("PASS")?;
    let api = project.source.join("concert_app/api.py");
    let clean = fs::read(&api)?;
    let mut injected = clean.clone();
    injected.extend_from_slice(b"\nfrom . import storage\n");
    fs::write(&api, injected)?;
    project.expect("FAIL")?;
    fs::write(&api, clean)?;
    project.expect("PASS")?;
    fs::remove_file(project.source.join("concert_app/storage.py"))?;
    project.expect("UNKNOWN")?;
    ensure!(
        snapshot(&original)? == before,
        "original concert files changed"
    );
    Ok(())
}

pub(super) fn direct_and_shortest_transitive_paths(_: &Path, reach: &str) -> Result<()> {
    let project = Project::chain(reach, "from . import middle\n")?;
    match reach {
        "direct" => {
            project.expect("PASS")?;
        }
        "transitive" => ensure!(
            witness(&project.expect("FAIL")?)?.len() == 2,
            "expected two-edge witness"
        ),
        _ => anyhow::bail!("unknown reach {reach}"),
    }
    Ok(())
}

pub(super) fn self_import_is_a_real_cycle(_: &Path, _: &str) -> Result<()> {
    let project = Project::package_with("transitive", "front")?;
    project.write("pkg/__init__.py", "pass\n")?;
    project.write("pkg/front.py", "from . import front\n")?;
    project.expect("FAIL")?;
    Ok(())
}

pub(super) fn excluded_endpoint_is_explicit_and_unresolved(_: &Path, _: &str) -> Result<()> {
    let project = Project::package()?;
    let exclude = ["pkg/store.py".into()];
    let report = project.expect_with(
        CodeCheckOptions {
            exclude: &exclude,
            ..project.options()
        },
        "UNKNOWN",
    )?;
    ensure!(
        report["exclude"] == json!(exclude),
        "missing explicit exclusion"
    );
    ensure!(
        report["files"].get("pkg/store.py").is_none(),
        "excluded file observed"
    );
    Ok(())
}

pub(super) fn source_symlink_is_not_followed(_: &Path, _: &str) -> Result<()> {
    let project = Project::package()?;
    let outside = project.dir.path().join("outside.py");
    fs::write(&outside, "raise RuntimeError('Must not execute')\n")?;
    let source = project.source.join("pkg/store.py");
    fs::remove_file(&source)?;
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside, &source)?;
    #[cfg(windows)]
    std::os::windows::fs::symlink_file(&outside, &source)?;
    ensure!(source.is_symlink(), "failed to create source symlink");
    project.expect("UNKNOWN")?;
    Ok(())
}

pub(super) fn source_uncertainty_is_unknown(_: &Path, parameter: &str) -> Result<()> {
    let (source, code) = match parameter {
        "syntax" => ("def broken(:\n", "CODE_SOURCE_UNPARSEABLE"),
        "dynamic" => (
            "import importlib\nimportlib.import_module('pkg.store')\n",
            "CODE_DYNAMIC_IMPORT",
        ),
        "absent" => ("from . import absent\n", "CODE_IMPORT_UNRESOLVED"),
        _ => anyhow::bail!("unknown source variant {parameter}"),
    };
    let project = Project::package()?;
    project.write("pkg/front.py", source)?;
    require_diagnostic(&project.expect("UNKNOWN")?, code)
}

pub(super) fn unbound_import_into_bound_component_is_unknown(_: &Path, _: &str) -> Result<()> {
    let project = Project::package()?;
    project.write("pkg/unmapped.py", "from . import store\n")?;
    require_diagnostic(&project.expect("UNKNOWN")?, "CODE_UNBOUND_IMPORT")
}

pub(super) fn witnessed_violation_wins_over_unrelated_parse_uncertainty(
    _: &Path,
    _: &str,
) -> Result<()> {
    let project = Project::package()?;
    project.write("pkg/front.py", "from . import store\n")?;
    project.write("pkg/broken.py", "def broken(:\n")?;
    require_diagnostic(&project.expect("FAIL")?, "CODE_SOURCE_UNPARSEABLE")
}

pub(super) fn observation_never_executes_project_code(_: &Path, _: &str) -> Result<()> {
    let project = Project::package()?;
    let marker = project.dir.path().join("executed");
    let literal = serde_json::to_string(&marker)?;
    project.write(
        "pkg/front.py",
        format!("from pathlib import Path\nPath({literal}).write_text('executed')\n"),
    )?;
    project.expect("PASS")?;
    ensure!(!marker.try_exists()?, "project code wrote the marker");
    let mut directories = vec![project.source.clone()];
    while let Some(directory) = directories.pop() {
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            ensure!(
                entry.file_name() != "__pycache__",
                "observer produced a Python bytecode cache"
            );
            if entry.file_type()?.is_dir() {
                directories.push(entry.path());
            }
        }
    }
    Ok(())
}

pub(super) fn scoped_transitive_rule_retains_intermediate_inventory(
    _: &Path,
    _: &str,
) -> Result<()> {
    let project = Project::chain("transitive", "from . import middle\n")?;
    let report = project.expect_with(
        CodeCheckOptions {
            scope: &["rule.boundary".into()],
            ..project.options()
        },
        "FAIL",
    )?;
    ensure!(
        report["files"].get("pkg/middle.py").is_some(),
        "missing intermediate source file"
    );
    ensure!(
        rows(&report, "edges")?
            .iter()
            .any(|edge| edge["from"] == "component.middle"),
        "missing intermediate edge"
    );
    Ok(())
}

pub(super) fn successful_observation_is_deterministic_and_preserves_inputs(
    _: &Path,
    _: &str,
) -> Result<()> {
    let project = Project::package()?;
    let before = snapshot(project.dir.path())?;
    let first = project.expect("PASS")?;
    ensure!(
        project.expect("PASS")? == first,
        "repeated observation changed"
    );
    ensure!(
        before == snapshot(project.dir.path())?,
        "input files changed"
    );
    for check in rows(&first, "checks")? {
        ensure!(check["dimension"] == "integration", "unexpected dimension");
        for diagnostic in rows(check, "diagnostics")? {
            for key in ["file", "pointer", "line", "column"] {
                ensure!(
                    diagnostic["location"].get(key).is_some(),
                    "missing diagnostic location field {key}"
                );
            }
        }
    }
    Ok(())
}

pub(super) fn transitive_witness_chooses_shortest_available_path(_: &Path, _: &str) -> Result<()> {
    let project = Project::chain("transitive", "from . import middle, store\n")?;
    let report = project.expect("FAIL")?;
    let path = witness(&report)?;
    ensure!(
        path.len() == 1 && path[0]["source_file"] == "pkg/front.py",
        "expected direct witness from front"
    );
    Ok(())
}

pub(super) fn fixed_infrastructure_is_excluded_from_source_inventory(
    _: &Path,
    _: &str,
) -> Result<()> {
    let project = Project::package()?;
    project.write(".venv/broken.py", "def invalid(:\n")?;
    ensure!(
        project.expect("PASS")?["files"]
            .get(".venv/broken.py")
            .is_none(),
        "ignored infrastructure was observed"
    );
    Ok(())
}
