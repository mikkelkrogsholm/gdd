//! Build private model, mapping and source inputs for a single case instance.

use crate::fixture::write_json;
use anyhow::{Context, Result, ensure};
use gdd::code_check::{CodeCheckOptions, PythonRuntime, check_code};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub(super) const FROZEN: [&str; 4] = ["os", "abc", "io", "stat"];

pub(super) fn runtime_descriptor() -> Value {
    json!({"impl":"cpython", "version":"3.11.0", "platform":"synthetic-conformance",
        "builtin_names":[], "frozen_names": FROZEN})
}

pub(super) struct Project {
    pub dir: tempfile::TempDir,
    pub source: PathBuf,
    pub path: PathBuf,
    pub model: Value,
    pub records: Value,
    pub ledger: PathBuf,
    implementation: &'static str,
    runtime: PythonRuntime,
}

impl Project {
    pub fn package() -> Result<Self> {
        Self::package_with("direct", "store")
    }

    pub fn package_with(reach: &str, target: &str) -> Result<Self> {
        let mut project = Self::new(
            "front",
            target,
            reach,
            &["front", "middle", "store"],
            "implementation.control",
        )?;
        project.write("pkg/__init__.py", "\"\"\"Inert bootstrap.\"\"\"\n")?;
        project.write("pkg/front.py", "VALUE = 1\n")?;
        project.write("pkg/store.py", "VALUE = 2\n")?;
        project.bind("front", "front", "pkg/front.py")?;
        project.bind("store", "store", "pkg/store.py")?;
        project.save_records()?;
        Ok(project)
    }

    pub fn chain(reach: &str, front: &str) -> Result<Self> {
        let mut project = Self::package_with(reach, "store")?;
        project.write("pkg/__init__.py", "pass\n")?;
        project.write("pkg/front.py", front)?;
        project.write("pkg/middle.py", "from . import store\n")?;
        project.write("pkg/store.py", "VALUE=1\n")?;
        project.bind("middle", "middle", "pkg/middle.py")?;
        project.save_records()?;
        Ok(project)
    }

    pub fn flat() -> Result<Self> {
        let mut project = Self::new(
            "api",
            "store",
            "direct",
            &["api", "store"],
            "implementation.cli",
        )?;
        for name in ["api", "store"] {
            let file = format!("{name}.py");
            project.write(&file, "pass\n")?;
            project.bind(name, name, &file)?;
        }
        project.save_records()?;
        Ok(project)
    }

    pub fn new(
        from: &str,
        to: &str,
        reach: &str,
        components: &[&str],
        implementation: &'static str,
    ) -> Result<Self> {
        let dir = tempfile::Builder::new()
            .prefix("gdd-code-case-")
            .tempdir()?;
        let source = dir.path().join("source");
        fs::create_dir(&source)?;
        let path = dir.path().join("model.json");
        let ledger = dir.path().join("mapping.records.json");
        let mut model = json!({"format":"gdd/0.3", "project":{"id":"project.import-control",
            "title":"Import control", "goal":"Observe explicit Python import boundaries."},
            "nodes":{}, "edges":[], "record_files":["mapping.records.json"]});
        for name in components {
            model["nodes"][format!("component.{name}")] = json!({"kind":"component",
                "description":format!("Own {name} operations.")});
        }
        model["nodes"]["rule.boundary"] = json!({"kind":"rule","description":"Enforce an explicit import boundary.",
            "rule":{"category":"architecture","statement":"The selected import dependency is forbidden.",
            "scope":"global","blocking":true,"verification":["python_import_graph"],
            "constraint":{"relation":"imports","from":format!("component.{from}"),"to":format!("component.{to}"),
            "reach":reach,"policy":"forbid"}}});
        let records =
            json!({"format":"gdd-records/0.2","project_id":"project.import-control","records":{}});
        write_json(&path, &model)?;
        write_json(&ledger, &records)?;
        Ok(Self {
            dir,
            source,
            path,
            ledger,
            model,
            records,
            implementation,
            runtime: PythonRuntime::from_json(&serde_json::to_vec(&runtime_descriptor())?)?,
        })
    }

    fn revision(&self) -> Result<String> {
        Ok(gdd::bundle::load_model_only(&self.path)
            .snapshot()
            .context("fixture model has no snapshot")?
            .revision
            .clone())
    }
    pub fn bind(&mut self, name: &str, component: &str, file: &str) -> Result<()> {
        self.records["records"][format!("binding.{name}")] = json!({"kind":"binding","spec_revision": self.revision()?,
            "targets":[format!("component.{component}")],"implementation":self.implementation,"role":"code",
            "resource":file,"selector":{"kind":"whole-document"},"status":"planned"});
        Ok(())
    }
    pub fn save_records(&self) -> Result<()> {
        write_json(&self.ledger, &self.records)
    }
    pub fn save_model_and_update_bindings(&mut self) -> Result<()> {
        write_json(&self.path, &self.model)?;
        let revision = self.revision()?;
        for mapping in self.records["records"]
            .as_object_mut()
            .context("expected records")?
            .values_mut()
        {
            mapping["spec_revision"] = json!(revision);
        }
        self.save_records()
    }
    pub fn write(&self, name: impl AsRef<Path>, contents: impl AsRef<[u8]>) -> Result<()> {
        let path = self.source.join(name);
        fs::create_dir_all(path.parent().context("source has no parent")?)?;
        Ok(fs::write(path, contents)?)
    }
    pub fn options(&self) -> CodeCheckOptions<'_> {
        CodeCheckOptions {
            root: &self.source,
            implementation: self.implementation,
            profile: "python-imports/0.1",
            records: None,
            scope: &[],
            exclude: &[],
            runtime: &self.runtime,
        }
    }
    pub fn expect(&self, verdict: &str) -> Result<Value> {
        self.expect_with(self.options(), verdict)
    }
    pub fn expect_with(&self, options: CodeCheckOptions<'_>, verdict: &str) -> Result<Value> {
        let report = check_code(&self.path, options)?;
        expect_verdict(&report, verdict)?;
        Ok(report)
    }
}

pub(super) fn expect_verdict(report: &Value, verdict: &str) -> Result<()> {
    ensure!(
        report["result"] == verdict,
        "expected {verdict}, got {}: {}",
        report["result"],
        report["checks"]
    );
    Ok(())
}

pub(super) fn rows<'a>(object: &'a Value, key: &str) -> Result<&'a [Value]> {
    object[key]
        .as_array()
        .map(Vec::as_slice)
        .with_context(|| format!("expected array {key}"))
}

pub(super) fn require_diagnostic(report: &Value, code: &str) -> Result<()> {
    for check in rows(report, "checks")? {
        if rows(check, "diagnostics")?
            .iter()
            .any(|d| d["code"] == code)
        {
            return Ok(());
        }
    }
    anyhow::bail!("missing {code} in {}", report["checks"])
}
