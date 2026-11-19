//! Code snapshot, caller-attested receipt and immutable ledger contracts.

use gdd::bundle::load_model_only;
use gdd::host::{
    Attestation, CheckCommand, append_records, execute_check, snapshot_code, verification_records,
};
use gdd::report::CheckResult;
use gdd::validation::{Profile, ValidationOptions, validate_with_options};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};

const CHECK: &str = "check.value";
const BINDING: &str = "binding.value";
const EVIDENCE: &str = "evidence.value";
const SOURCES: &[&str] = &["domain.rs", "checks/value.rs", "checks-too.rs"];

fn write(path: &Path, value: &Value) {
    fs::write(path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
}

fn read(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

struct Project {
    dir: tempfile::TempDir,
    model: PathBuf,
    ledger: PathBuf,
    receipt: PathBuf,
    checker: Value,
    environment: BTreeMap<String, String>,
}

impl Project {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let model = root.join("gdd.json");
        let ledger = root.join("records.json");
        write(
            &model,
            &json!({
                "format": "gdd/0.3", "project": {"id": "host.example", "title": "Host fixture", "goal": "Check current evidence."},
                "nodes": {"component.domain": {"kind": "component", "description": "Fixture domain."},
                    "rule.value": {"kind": "rule", "description": "VALUE equals two.", "rule": {
                        "category": "invariant", "scope": "targeted", "statement": "VALUE equals two.",
                        "blocking": true, "verification": ["compile_assertion"]}}},
                "edges": [{"id": "edge.value", "from": "rule.value", "relation": "constrains", "to": "component.domain"}],
                "record_files": ["records.json"],
            }),
        );
        let mut document =
            json!({"format":"gdd-records/0.2","project_id":"host.example","records":{}});
        write(&ledger, &document);
        let revision = load_model_only(&model).snapshot().unwrap().revision.clone();
        document["records"][CHECK] = json!({"kind": "check", "spec_revision": revision, "scope": ["rule.value"],
            "method": "compile_assertion", "required_environment": ["rustc"], "negative_controls": []});
        write(&ledger, &document);
        fs::create_dir(root.join("checks")).unwrap();
        fs::write(root.join("domain.rs"), "const VALUE: u8 = 2;\n").unwrap();
        fs::write(
            root.join("checks/value.rs"),
            "include!(\"../domain.rs\");\nconst _: () = assert!(VALUE == 2);\n",
        )
        .unwrap();
        fs::write(root.join("checks-too.rs"), "").unwrap();
        let receipt = root.join("run.json");
        let checker = json!({"name": "fixture-checker", "version": "1"});
        let environment = BTreeMap::from([
            ("rustc".to_owned(), "fixture".to_owned()),
            ("extra".to_owned(), "host-only".to_owned()),
        ]);
        Self {
            dir,
            model,
            ledger,
            receipt,
            checker,
            environment,
        }
    }

    fn options(&self) -> Attestation<'_> {
        Attestation {
            root: self.dir.path(),
            implementation: "implementation.host",
            binding_id: BINDING,
            evidence_id: EVIDENCE,
            test_resource: "checks/value.rs",
            checker: &self.checker,
            environment: &self.environment,
            verified_negative_controls: &[],
        }
    }

    fn attest(&self, opts: &Attestation<'_>) -> io::Result<Value> {
        verification_records(&self.model, CHECK, &self.receipt, opts)
    }

    // Input to rejection tests. Positive implementation verdicts below use
    // a real compiler exit and snapshots of its actual source files.
    fn receipt_fields(&self) -> Value {
        let snapshot = snapshot_code(self.dir.path(), SOURCES).unwrap();
        json!({"format":"gdd-host-check/0.1","executed":true,"result":"PASS","exit_code":0,
            "snapshot":snapshot, "code_revision":snapshot.revision, "code_revision_after":snapshot.revision})
    }

    fn assess(&self, host: Option<&Value>) -> CheckResult {
        let context = host.map(gdd::input::Value::from);
        validate_with_options(
            &load_model_only(&self.model),
            ValidationOptions {
                profiles: &[Profile::Records],
                scope: &["component.domain".to_string()],
                host_context: context.as_ref(),
                ..Default::default()
            },
        )
        .unwrap()
        .result("implementation")
    }
}

#[test]
fn snapshot_hashes_normalized_resources_in_canonical_order() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    fs::create_dir(root.join("bin")).unwrap();
    fs::write(root.join("bin/é.dat"), [255, 0, 90]).unwrap();
    fs::write(root.join("a.txt"), b"hello\n").unwrap();
    let snapshot = snapshot_code(root, &["bin/é.dat", "a.txt"]).unwrap();
    assert_eq!(
        snapshot,
        snapshot_code(root, &["a.txt", "bin/é.dat"]).unwrap()
    );
    // Portable Python hashlib/json values for the exact bytes above.
    assert_eq!(
        json!(snapshot),
        json!({"format":"gdd-code-snapshot/0.1","files":{
        "a.txt":"5891b5b522d5df086d0ff0b110fbd9d21bb4fc7163af34d08286a2e846f6be03",
        "bin/é.dat":"6ff3afd0c0c5de7c60201acabbd2817ff074350855816ed445ad14a1483a61e3"},
        "revision":"sha256:42a1545b3c79c7d0fe57ba556f8b096b898bbb42af5abc77134f05f3e442a2b9"})
    );
    assert_eq!(
        snapshot_code(root, &[]).unwrap().revision,
        "sha256:44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a"
    );
    assert!(snapshot_code(&root.join("a.txt"), &[]).is_err());
    for name in [
        "",
        ".",
        "./a.txt",
        "a.txt/",
        "bin//é.dat",
        "bin/../a.txt",
        "../a.txt",
        "bin/./é.dat",
        "/a.txt",
        "bin\\é.dat",
        "a\0.txt",
        "bin",
        "missing",
        "*.txt",
    ] {
        assert!(snapshot_code(root, &[name]).is_err(), "accepted {name:?}");
    }
    assert!(snapshot_code(root, &["a.txt", "a.txt"]).is_err());
}

#[test]
fn actual_compiler_result_remains_bound_to_explicit_host_trust() {
    for (value, verdict, label, code) in [
        (2, CheckResult::Pass, "PASS", 0),
        (3, CheckResult::Fail, "FAIL", 1),
    ] {
        let mut project = Project::new();
        let root = project.dir.path();
        fs::write(
            root.join("domain.rs"),
            format!("const VALUE: u8 = {value};\n"),
        )
        .unwrap();
        let version = Command::new("rustc").arg("--version").output().unwrap();
        let version = String::from_utf8(version.stdout).unwrap().trim().to_owned();
        project.environment.insert("rustc".into(), version.clone());
        project.checker = json!({"name":"rustc","version":version});
        let before = snapshot_code(root, SOURCES).unwrap();
        let command = CheckCommand {
            argv: &[
                "rustc",
                "--crate-type=lib",
                "--emit=metadata",
                "checks/value.rs",
            ],
            env: &[],
        };
        let receipt = execute_check(
            root,
            SOURCES,
            &project.receipt,
            command,
            Duration::from_secs(30),
        )
        .unwrap();
        assert_eq!(receipt["format"], "gdd-host-check/0.1");
        assert_eq!(
            receipt["runner"],
            json!({
                "name": "gdd", "version": env!("CARGO_PKG_VERSION"),
                "os": std::env::consts::OS, "arch": std::env::consts::ARCH,
            })
        );
        assert_eq!(receipt["exit_code"], code);
        assert_eq!(receipt["result"], label);
        assert_eq!(read(&project.receipt), receipt);

        let opts = project.options();
        let fragment = project.attest(&opts).unwrap();
        let evidence = &fragment["records"][EVIDENCE];
        assert_eq!(evidence["executed"], true);
        assert_eq!(evidence["result"], label);
        assert_eq!(
            evidence["check_revision"],
            gdd::canonical_digest(&read(&project.ledger)["records"][CHECK])
        );
        assert_eq!(evidence["environment"], json!({"rustc":version}));
        assert_eq!(fragment["host"]["environment"], json!(project.environment));
        assert!(fragment["host"].get("policy").is_none());
        assert!(fragment["host"].get("trusted_decisions").is_none());
        let binding = &fragment["records"][BINDING];
        assert_eq!(
            binding["revision"],
            format!("sha256:{}", before.files["checks/value.rs"])
        );
        let directory = project
            .attest(&Attestation {
                test_resource: "checks/",
                ..opts
            })
            .unwrap();
        let directory = &directory["records"][BINDING];
        assert_eq!(directory["resource"], "checks/");
        assert_eq!(
            directory["revision"],
            gdd::canonical_digest(&json!({"checks/value.rs":before.files["checks/value.rs"]}))
        );

        append_records(&project.ledger, &fragment).unwrap();
        assert_eq!(project.assess(None), CheckResult::Unknown);
        assert_eq!(project.assess(Some(&fragment["host"])), verdict);
    }
}

#[test]
fn only_exact_completed_current_receipts_can_be_attested() {
    let project = Project::new();
    let original = project.receipt_fields();
    for (key, value) in [
        ("format", json!("other")),
        ("executed", json!(false)),
        ("executed", json!(1)),
        ("result", json!("UNKNOWN")),
        ("result", json!("FAIL")),
        ("exit_code", json!(true)),
        ("exit_code", json!("0")),
        ("exit_code", json!(0.5)),
        ("exit_code", json!(2)),
        ("exit_code", Value::Null),
        ("code_revision", json!("old")),
        ("code_revision_after", Value::Null),
        ("snapshot", json!({"files": []})),
    ] {
        let mut damaged = original.clone();
        damaged[key] = value;
        write(&project.receipt, &damaged);
        assert!(
            project.attest(&project.options()).is_err(),
            "accepted {damaged}"
        );
    }
    for (status, label) in [
        ("false", "PASS"),
        ("0.0", "PASS"),
        ("-0.0", "PASS"),
        ("0e250", "PASS"),
        ("true", "FAIL"),
        ("1.0", "FAIL"),
        ("10e-1", "FAIL"),
    ] {
        let mut legacy = original.clone();
        legacy["exit_code"] = serde_json::from_str(status).unwrap();
        legacy["result"] = json!(label);
        write(&project.receipt, &legacy);
        let fragment = project.attest(&project.options()).unwrap();
        assert_eq!(fragment["records"][EVIDENCE]["result"], label);
    }
    let mut near_one = original.clone();
    near_one["exit_code"] = serde_json::from_str("1.00000000000000000000000001").unwrap();
    near_one["result"] = json!("FAIL");
    write(&project.receipt, &near_one);
    assert!(project.attest(&project.options()).is_err());
    let mut forged = original.clone();
    forged["snapshot"]["files"]["domain.rs"] = json!("substituted");
    write(&project.receipt, &forged);
    assert!(project.attest(&project.options()).is_err());
    write(&project.receipt, &original);
    fs::write(project.dir.path().join("domain.rs"), "changed").unwrap();
    assert!(
        project
            .attest(&project.options())
            .unwrap_err()
            .to_string()
            .contains("stale")
    );
}

#[test]
fn check_identity_and_actual_coverage_are_required() {
    let project = Project::new();
    write(&project.receipt, &project.receipt_fields());
    let ledger = read(&project.ledger);
    for (key, value) in [
        ("scope", json!(["rule.missing"])),
        ("spec_revision", json!(format!("sha256:{}", "0".repeat(64)))),
        ("supersedes", json!(CHECK)),
        ("supersedes", json!("check.missing")),
    ] {
        let mut damaged = ledger.clone();
        damaged["records"][CHECK][key] = value;
        write(&project.ledger, &damaged);
        assert!(
            project.attest(&project.options()).is_err(),
            "accepted {damaged}"
        );
    }
    write(&project.ledger, &ledger);
    let invalid_checker = json!({"name":"unknown"});
    for opts in [
        Attestation {
            binding_id: "edge.value",
            ..project.options()
        },
        Attestation {
            evidence_id: CHECK,
            ..project.options()
        },
        Attestation {
            evidence_id: BINDING,
            ..project.options()
        },
        Attestation {
            test_resource: "domain",
            ..project.options()
        },
        Attestation {
            test_resource: "../",
            ..project.options()
        },
        Attestation {
            checker: &invalid_checker,
            ..project.options()
        },
        Attestation {
            environment: &BTreeMap::new(),
            ..project.options()
        },
        Attestation {
            verified_negative_controls: &[String::new()],
            ..project.options()
        },
    ] {
        assert!(project.attest(&opts).is_err());
    }
    let mut ledger = ledger;
    ledger["records"][CHECK]["negative_controls"] = json!(["mutation"]);
    // A separate invalid historical reference cannot poison this valid check.
    ledger["records"]["check.unrelated"] = ledger["records"][CHECK].clone();
    ledger["records"]["check.unrelated"]["supersedes"] = json!("check.absent");
    write(&project.ledger, &ledger);
    assert!(project.attest(&project.options()).is_err());
    let controls = ["mutation".into(), "other".into()];
    let result = project
        .attest(&Attestation {
            verified_negative_controls: &controls,
            ..project.options()
        })
        .unwrap();
    assert_eq!(result["host"]["negative_controls"], json!(controls));
}

#[test]
fn append_validates_original_and_merged_ledgers_and_cleans_up() {
    let project = Project::new();
    write(&project.receipt, &project.receipt_fields());
    let fragment = project.attest(&project.options()).unwrap();
    let lock = project.dir.path().join("records.json.lock");
    let before = fs::read(&project.ledger).unwrap();
    fs::write(&lock, b"other writer").unwrap();
    assert_eq!(
        append_records(&project.ledger, &fragment)
            .unwrap_err()
            .kind(),
        io::ErrorKind::AlreadyExists
    );
    assert_eq!(fs::read(&lock).unwrap(), b"other writer");
    fs::remove_file(&lock).unwrap();
    for invalid in [
        json!({}),
        json!({"records":{}}),
        json!({"records":{"evidence.bad":{"kind":"evidence"}}}),
    ] {
        assert!(append_records(&project.ledger, &invalid).is_err());
        assert_eq!(fs::read(&project.ledger).unwrap(), before);
        assert!(!lock.exists());
    }
    let malformed = b"format: gdd-records/0.2\nproject_id: host.example\n";
    fs::write(&project.ledger, malformed).unwrap();
    assert!(append_records(&project.ledger, &fragment).is_err());
    assert_eq!(fs::read(&project.ledger).unwrap(), malformed);
    assert!(!lock.exists());
    fs::write(&project.ledger, before).unwrap();

    #[cfg(unix)]
    fs::set_permissions(
        &project.ledger,
        std::os::unix::fs::PermissionsExt::from_mode(0o640),
    )
    .unwrap();
    append_records(&project.ledger, &fragment).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            project.ledger.metadata().unwrap().permissions().mode() & 0o777,
            0o640
        );
    }
    let after = fs::read(&project.ledger).unwrap();
    assert!(
        append_records(&project.ledger, &fragment)
            .unwrap_err()
            .to_string()
            .contains("immutable")
    );
    assert_eq!(fs::read(&project.ledger).unwrap(), after);
    assert!(!lock.exists());
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn resources_and_ledgers_reject_links_but_regular_receipts_allow_aliases() {
    use std::os::unix::fs::symlink;
    let project = Project::new();
    let root = project.dir.path();
    let fifo = root.join("pipe");
    assert!(
        Command::new("mkfifo")
            .args(["-m", "600"])
            .arg(&fifo)
            .status()
            .expect("create the FIFO fixture")
            .success()
    );
    symlink("domain.rs", root.join("alias")).unwrap();
    symlink("checks", root.join("alias-dir")).unwrap();
    for name in ["pipe", "alias", "alias-dir/value.rs"] {
        assert!(snapshot_code(root, &[name]).is_err());
    }
    write(&project.receipt, &project.receipt_fields());
    let receipt_link = root.join("receipt-link");
    symlink(&project.receipt, &receipt_link).unwrap();
    let ledger_link = root.join("ledger-link");
    symlink(&project.ledger, &ledger_link).unwrap();
    let fragment = project.attest(&project.options()).unwrap();
    assert!(verification_records(&project.model, CHECK, &fifo, &project.options()).is_err());
    let aliased =
        verification_records(&project.model, CHECK, &receipt_link, &project.options()).unwrap();
    assert_eq!(
        aliased["records"][EVIDENCE]["evidence_uri"],
        receipt_link.to_str().unwrap()
    );
    for path in [&fifo, &ledger_link] {
        assert!(append_records(path, &fragment).is_err());
    }
}

#[test]
fn competing_appends_read_the_current_ledger_after_acquiring_the_lock() {
    use std::sync::Barrier;
    use std::thread;
    use std::time::{Duration, Instant};

    let project = Project::new();
    let ledger = &project.ledger;
    let original = read(ledger);
    let check = &original["records"][CHECK];
    let gate = Barrier::new(4);
    thread::scope(|threads| {
        for worker in 0..4 {
            let gate = &gate;
            threads.spawn(move || {
                let fragment = json!({"records":{(format!("check.worker{worker}")):check}});
                gate.wait();
                let deadline = Instant::now() + Duration::from_secs(10);
                loop {
                    match append_records(ledger, &fragment) {
                        Ok(()) => break,
                        Err(error)
                            if error.kind() == io::ErrorKind::AlreadyExists
                                && Instant::now() < deadline =>
                        {
                            thread::sleep(Duration::from_millis(2));
                        }
                        Err(error) => panic!("Failed to append {worker}: {error}"),
                    }
                }
            });
        }
    });
    let published = read(ledger);
    assert_eq!(published["records"].as_object().unwrap().len(), 5);
    assert_eq!(&published["records"][CHECK], check);
    for worker in 0..4 {
        assert_eq!(
            &published["records"][format!("check.worker{worker}")],
            check
        );
    }
}
