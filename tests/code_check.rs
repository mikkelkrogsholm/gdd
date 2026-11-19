//! Public native code-observation API and CLI controls.

use gdd::code_check::{CodeCheckOptions, PythonRuntime, check_code};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

const RUNTIME: &str = r#"{"impl":"cpython","version":"3.11.15","platform":"linux","builtin_names":["sys"],"frozen_names":["os"]}"#;

struct Project {
    dir: tempfile::TempDir,
    model: PathBuf,
    source: PathBuf,
    runtime: PythonRuntime,
}

fn write(path: &Path, json: &Value) {
    fs::write(path, serde_json::to_vec(json).unwrap()).unwrap();
}

impl Project {
    fn new(reach: &str) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let model = dir.path().join("gdd.json");
        let source = dir.path().join("source");
        fs::create_dir(&source).unwrap();
        let mut nodes = json!({"rule.boundary":{"kind":"rule","description":"API boundary","rule":{"category":"architecture",
            "statement":"API must not import storage.","scope":"global","blocking":true,"verification":["python_imports"],
            "constraint":{"relation":"imports","from":"component.api","to":"component.storage","reach":reach,"policy":"forbid"}}}});
        for name in ["api", "middle", "storage"] {
            nodes[format!("component.{name}")] = json!({"kind":"component","description":name});
            fs::write(source.join(format!("{name}.py")), b"pass\n").unwrap();
        }
        write(
            &model,
            &json!({"format":"gdd/0.3","project":{"id":"project.demo","goal":"Check imports","title":"Import check"},
            "nodes":nodes,"edges":[],"record_files":["records.json"]}),
        );
        let ledger = dir.path().join("records.json");
        let mut records =
            json!({"format":"gdd-records/0.2","project_id":"project.demo","records":{}});
        write(&ledger, &records);
        let revision = gdd::bundle::load_model_only(&model)
            .snapshot()
            .unwrap()
            .revision
            .clone();
        for name in ["api", "middle", "storage"] {
            records["records"][format!("binding.{name}")] = json!({"kind":"binding","spec_revision":revision,
                "role":"code","implementation":"implementation.demo","targets":[format!("component.{name}")],
                "resource":format!("{name}.py"),"selector":{"kind":"whole-document"},"status":"planned"});
        }
        write(&ledger, &records);
        Project {
            dir,
            model,
            source,
            runtime: PythonRuntime::from_json(RUNTIME.as_bytes()).unwrap(),
        }
    }
    fn options(&self) -> CodeCheckOptions<'_> {
        CodeCheckOptions {
            root: &self.source,
            implementation: "implementation.demo",
            profile: "python-imports/0.1",
            records: None,
            scope: &[],
            exclude: &[],
            runtime: &self.runtime,
        }
    }
    fn cli(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_gdd"));
        command
            .env("PATH", self.dir.path())
            .arg("check-code")
            .arg(&self.model)
            .arg("--root")
            .arg(&self.source)
            .args([
                "--implementation",
                "implementation.demo",
                "--profile",
                "python-imports/0.1",
            ]);
        command
    }
    fn report(&self) -> Value {
        check_code(&self.model, self.options()).unwrap()
    }
}

#[test]
fn observes_current_bytes_and_shortest_witnesses() {
    let project = Project::new("transitive");
    let clean = project.report();
    assert_eq!(clean["result"], "PASS");
    assert_eq!(clean["files"]["api.py"], gdd::sha256(b"pass\n"));
    assert_eq!(
        clean["code_revision"],
        gdd::canonical_digest(&clean["files"])
    );
    let ledger: Value =
        serde_json::from_slice(&fs::read(project.dir.path().join("records.json")).unwrap())
            .unwrap();
    assert_eq!(
        clean["bindings"]["binding.api"]["record_revision"],
        gdd::canonical_digest(&ledger["records"]["binding.api"])
    );
    let desc: Value = serde_json::from_str(RUNTIME).unwrap();
    assert_eq!(
        clean["advisory"][0]["descriptor_revision"],
        gdd::canonical_digest(&desc)
    );
    assert_eq!(clean["runtime"]["python"], "3.11.15");
    fs::write(project.source.join("api.py"), "import middle").unwrap();
    fs::write(project.source.join("middle.py"), "import storage").unwrap();
    let failed = project.report();
    assert_eq!(failed["result"], "FAIL");
    assert_eq!(failed["checks"][0]["path"].as_array().unwrap().len(), 2);
    fs::write(project.source.join("api.py"), "import storage, middle").unwrap();
    fs::write(project.source.join("other.py"), "import importlib").unwrap();
    let failed = project.report();
    assert_eq!(failed["result"], "FAIL");
    assert_eq!(failed["checks"][0]["path"].as_array().unwrap().len(), 1);
    assert_eq!(failed["checks"][0]["path"][0]["source_file"], "api.py");
    assert_ne!(failed["code_revision"], clean["code_revision"]);
    assert_eq!(failed["spec_revision"], clean["spec_revision"]);
    let schema: Value =
        serde_json::from_str(include_str!("../schemas/code-observation.schema.json")).unwrap();
    jsonschema::validator_for(&schema)
        .unwrap()
        .validate(&failed)
        .unwrap();
}

#[test]
fn omitted_local_roots_and_secondary_record_admission_are_explicit() {
    let project = Project::new("direct");
    fs::write(project.source.join("api.py"), "import helper, os").unwrap();
    fs::write(project.source.join("helper.py"), "pass").unwrap();
    fs::write(project.source.join("os.py"), "pass").unwrap();
    let excludes = ["h[ae]lp?r.py".into(), "os.py".into()];
    let report = check_code(
        &project.model,
        CodeCheckOptions {
            exclude: &excludes,
            ..project.options()
        },
    )
    .unwrap();
    assert_eq!(report["result"], "UNKNOWN");
    assert_eq!(report["omitted"], json!(["helper.py", "os.py"]));
    for import in report["imports"].as_array().unwrap() {
        assert_eq!(import["external"], import["module"] == "os");
    }
    let ledger = project.dir.path().join("records.json");
    assert_eq!(
        project.report(),
        check_code(
            &project.model,
            CodeCheckOptions {
                records: Some(&ledger),
                ..project.options()
            }
        )
        .unwrap()
    );
    let malformed = project.dir.path().join("malformed.json");
    fs::write(&malformed, "{").unwrap();
    for path in [
        malformed,
        project.dir.path().join("missing"),
        project.source.clone(),
    ] {
        let report = check_code(
            &project.model,
            CodeCheckOptions {
                records: Some(&path),
                ..project.options()
            },
        )
        .unwrap();
        assert_eq!(
            report["checks"][0]["reason"],
            "Additional records must be a supported, schema-valid records document."
        );
        assert_eq!(report["result"], "UNKNOWN");
    }
}

#[cfg(unix)]
#[test]
fn physical_aliases_and_non_unicode_names_preserve_coverage() {
    use std::os::unix::{ffi::OsStrExt, fs::symlink};
    let project = Project::new("direct");
    fs::write(
        project
            .source
            .join(std::ffi::OsStr::from_bytes(b"ignored-\xff.txt")),
        [],
    )
    .unwrap();
    assert_eq!(project.report()["result"], "PASS");
    fs::hard_link(
        project.source.join("api.py"),
        project.source.join("alias.py"),
    )
    .unwrap();
    symlink(&project.source, project.source.join("link")).unwrap();
    let report = project.report();
    assert_eq!(report["result"], "UNKNOWN");
    assert!(report["files"].get("api.py").is_none());
    assert_eq!(report["omitted"], json!(["alias.py", "api.py", "link/"]));
    let diagnostics = report["checks"][0]["diagnostics"].as_array().unwrap();
    assert!(
        diagnostics
            .iter()
            .any(|d| d["code"] == "CODE_BINDING_AMBIGUOUS")
    );
    assert!(
        diagnostics
            .iter()
            .any(|d| d["code"] == "CODE_SOURCE_UNAVAILABLE")
    );
}

#[test]
fn cli_exit_codes_and_exclusive_output_follow_report_verdict() {
    let project = Project::new("direct");
    let runtime = project.dir.path().join("runtime.json");
    fs::write(&runtime, RUNTIME).unwrap();
    let output = project.dir.path().join("result.json");
    let run = |output: Option<&Path>| {
        let mut command = project.cli();
        command.arg("--python-runtime").arg(&runtime);
        if let Some(path) = output {
            command.arg("--output").arg(path);
        }
        command.output().unwrap()
    };
    assert!(run(Some(&output)).status.success());
    let original = fs::read(&output).unwrap();
    for (source, code) in [("import storage", 1), ("import importlib", 2), ("pass", 0)] {
        fs::write(project.source.join("api.py"), source).unwrap();
        assert_eq!(run(None).status.code(), Some(code));
    }
    assert_eq!(run(Some(&output)).status.code(), Some(1));
    assert_eq!(fs::read(output).unwrap(), original);
}

#[test]
fn omitted_runtime_selects_the_shipped_reference_build() {
    let project = Project::new("direct");
    fs::write(project.source.join("api.py"), "import math").unwrap();
    fs::write(project.source.join("math.py"), "pass").unwrap();
    let default = project.cli().output().unwrap();
    assert!(default.status.success());
    let default: Value = serde_json::from_slice(&default.stdout).unwrap();
    let reference = PythonRuntime::reference().unwrap();
    assert_eq!(
        default,
        check_code(
            &project.model,
            CodeCheckOptions {
                runtime: &reference,
                ..project.options()
            }
        )
        .unwrap()
    );
    assert_eq!(default["runtime"]["python"], "3.11.15");
    assert_eq!(default["imports"][0]["external"], true);
    let info = &default["advisory"][0];
    assert_eq!(info["platform"], "linux");
    assert_eq!(
        info["descriptor_revision"],
        "sha256:abdfb0684230ee45867e94c0cbd30b31d74dd73d692641c09b9deddca0ae5a1b"
    );
    assert!(
        info["message"]
            .as_str()
            .unwrap()
            .contains("GDD ships this reference target")
    );

    let descriptor = project.dir.path().join("target.json");
    fs::write(
        &descriptor,
        include_bytes!("../src/model/code_check/reference-cpython-3.11.15-linux-aarch64.json"),
    )
    .unwrap();
    let run = || {
        project
            .cli()
            .arg("--python-runtime")
            .arg(&descriptor)
            .output()
            .unwrap()
    };
    let explicit = run();
    assert!(explicit.status.success());
    let mut explicit: Value = serde_json::from_slice(&explicit.stdout).unwrap();
    assert_eq!(
        explicit["advisory"][0]["message"],
        "The caller supplied this CPython target. The native observer parses sources with Ruff."
    );
    explicit["advisory"][0]["message"] = info["message"].clone();
    assert_eq!(default, explicit);

    fs::write(&descriptor, RUNTIME).unwrap();
    let custom: Value = serde_json::from_slice(&run().stdout).unwrap();
    assert_eq!(custom["imports"][0]["external"], false);
    assert_ne!(
        custom["advisory"][0]["descriptor_revision"],
        info["descriptor_revision"]
    );
    fs::write(&descriptor, "{}").unwrap();
    assert!(!run().status.success());
}

#[test]
fn malformed_runtime_descriptors_have_no_fallback() {
    for bad in [
        RUNTIME.replace("3.11.15", "3.15.0"),
        RUNTIME.replace("cpython", "unknown"),
        RUNTIME.replace("linux", " "),
        RUNTIME.replace("\"sys\"", "\"bad/name\""),
        RUNTIME.replace("\"impl\":", "\"impl\":\"cpython\",\"impl\":"),
    ] {
        assert!(PythonRuntime::from_json(bad.as_bytes()).is_err(), "{bad}");
    }
}
