use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::Barrier;

use gdd::graphify::{
    AST_PROFILE, GraphifyOptions, ImportReport, NODE_LINK_PROFILE, compact_observations,
    export_graphify, import_graphify, load_observations, verify_graphify,
};
use gdd::report::CheckResult::{self, Fail, Pass, Unknown};
use serde_json::{Value, json};
use tempfile::tempdir;

const RAW: &str = "graphify.raw.json";
const MANIFEST: &str = "observations.json";
const VALID_AST: &[u8] = br#" { "nodes" : [{"id":"n","label":"Node","file_type":"code","source_file":"a.rs"}], "edges": [] }
"#;

fn options() -> GraphifyOptions<'static> {
    GraphifyOptions {
        profile: Some(AST_PROFILE),
        ..Default::default()
    }
}

fn check(report: &ImportReport, id: &str) -> CheckResult {
    report
        .checks
        .iter()
        .find(|c| c.check_id == id)
        .unwrap()
        .result
}

fn index(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path.join(MANIFEST)).unwrap()).unwrap()
}

fn run_cli(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_gdd"))
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn archived_bundles_and_alternate_names_use_the_recorded_claims() {
    let archive = Path::new("examples/graphify/observations");
    let loaded = load_observations(archive).unwrap();
    assert_eq!(
        loaded.raw.unwrap(),
        include_bytes!("../examples/graphify/raw.ast.json")
    );
    assert_eq!(loaded.report.exit_code(), 0);
    let dir = tempdir().unwrap();
    let alias = dir.path().join("copied index.json");
    fs::copy(archive.join(MANIFEST), &alias).unwrap();
    fs::copy(archive.join(RAW), dir.path().join(RAW)).unwrap();
    let report = verify_graphify(&alias, Some(NODE_LINK_PROFILE), Some("99")).unwrap();
    assert_eq!(check(&report, "graphify.projection"), Pass);
    assert_eq!(check(&report, "graphify.profile"), Unknown);
    assert_eq!(check(&report, "graphify.version"), Unknown);

    let mut changed = fs::read(dir.path().join(RAW)).unwrap();
    changed.push(b' ');
    fs::write(dir.path().join(RAW), changed).unwrap();
    let corrupt = load_observations(&alias).unwrap();
    assert_eq!(check(&corrupt.report, "graphify.integrity"), Fail);
    let output = dir.path().join("rejected");
    assert!(export_graphify(&alias, &output).is_err());
    assert!(!output.exists());

    let raw_named_manifest = dir.path().join(MANIFEST);
    fs::copy(archive.join(RAW), &raw_named_manifest).unwrap();
    assert_eq!(
        verify_graphify(&raw_named_manifest, Some(AST_PROFILE), None)
            .unwrap()
            .exit_code(),
        0
    );
    assert_eq!(
        check(
            &verify_graphify(&raw_named_manifest, None, None).unwrap(),
            "graphify.manifest"
        ),
        Unknown
    );
}

#[test]
fn cli_receipts_preserve_failed_and_unknown_artifacts() {
    let dir = tempdir().unwrap();
    let source = dir.path().join("input.json");
    for (label, bytes, profile, exit) in [
        ("invalid", &b"{broken json\n"[..], AST_PROFILE, 1),
        ("unsupported", &b"{}\r\n"[..], "graphify-future", 2),
        ("valid", VALID_AST, AST_PROFILE, 0),
    ] {
        fs::write(&source, bytes).unwrap();
        let bundle = dir.path().join("new parent").join(label);
        let bundle_arg = bundle.to_str().unwrap();
        let args = [
            "graphify",
            "import",
            source.to_str().unwrap(),
            "--profile",
            profile,
            "--output",
            bundle_arg,
            "--source-id",
            "source.test",
            "--code-revision",
            "unverified-commit",
        ];
        let imported = run_cli(&args);
        assert_eq!(imported.status.code(), Some(exit), "{:?}", imported.stderr);
        assert!(imported.stderr.is_empty());
        let report: Value = serde_json::from_slice(&imported.stdout).unwrap();
        let saved: Value =
            serde_json::from_slice(&fs::read(bundle.join("import.report.json")).unwrap()).unwrap();
        assert_eq!(saved, report);
        assert_eq!(fs::read(bundle.join(RAW)).unwrap(), bytes);
        assert!(bundle.join("README.md").is_file());
        let index = index(&bundle);
        assert_eq!(index["source_id"], "source.test");
        assert_eq!(index["input_claims"]["code_revision"], "unverified-commit");
        fs::write(bundle.join("import.report.json"), b"stale").unwrap();
        let output = dir.path().join(format!("{label}.out"));
        let recovered = run_cli(&[
            "graphify",
            "export",
            bundle_arg,
            "--output",
            output.to_str().unwrap(),
        ]);
        assert_eq!(recovered.status.code(), Some(exit));
        assert_eq!(
            serde_json::from_slice::<Value>(&recovered.stdout).unwrap(),
            report
        );
        assert_eq!(fs::read(output).unwrap(), bytes);
    }
}

#[test]
fn typed_numbers_and_historical_compact_limits_are_checked_independently() {
    let dir = tempdir().unwrap();
    let source = dir.path().join("input");
    for (label, number, historically_representable) in [
        ("ordinary", "1.00", true),
        ("exponent", "1e23", true),
        ("signed-zero", "-0.0", true),
        ("tiny", "1e-4000", false),
        ("wide-integer", &"9".repeat(4301), false),
        ("zero-exponent", "0e1000000000000000000", false),
        ("admitted-zero", "0.00e1000000000000000000", true),
        ("precise", "0.1234567890123456789", false),
    ] {
        let bytes = format!(
            r#"{{"nodes":[{{"id":{number},"label":"A","file_type":"code","source_file":"a.rs"}}],"edges":[]}}"#
        );
        fs::write(&source, &bytes).unwrap();
        let bundle = dir.path().join(label);
        assert_eq!(
            import_graphify(&source, &bundle, options())
                .unwrap()
                .exit_code(),
            0
        );
        let full = index(&bundle);
        for version in ["0.1.0", "0.1.1", "0.1.2"] {
            let mut manifest = compact_observations(&full);
            manifest["importer"]["version"] = version.into();
            fs::write(
                bundle.join(MANIFEST),
                serde_json::to_vec(&manifest).unwrap(),
            )
            .unwrap();
            let report = verify_graphify(&bundle, None, None).unwrap();
            let expected = if historically_representable && version != "0.1.2" {
                Fail
            } else {
                Unknown
            };
            assert_eq!(
                check(&report, "graphify.projection"),
                expected,
                "{label} {version}"
            );
            let dest = dir.path().join(format!("{label}-{version}"));
            assert_eq!(export_graphify(&bundle, &dest).is_ok(), expected != Fail);
            if expected != Fail {
                assert_eq!(fs::read(dest).unwrap(), bytes.as_bytes());
            }
        }
        let mut manifest = full;
        manifest["artifact"]["byte_count"] =
            format!("{}0e-1", bytes.len()).parse::<Value>().unwrap();
        fs::write(
            bundle.join(MANIFEST),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        let report = verify_graphify(&bundle, None, None).unwrap();
        assert_eq!(check(&report, "graphify.integrity"), Pass);
        assert_eq!(check(&report, "graphify.projection"), Fail);
    }
}

#[test]
fn raw_and_embedded_depth_have_independent_capacity() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("input");
    for depth in [256, 257, 1200] {
        let raw = format!(
            r#"{{"nodes":[{{"id":"a","label":"A","file_type":"code","source_file":"a.py","vendor":{}0{}}}],"edges":[]}}"#,
            "[".repeat(depth - 3),
            "]".repeat(depth - 3)
        );
        fs::write(&path, &raw).unwrap();
        let bundle = dir.path().join(depth.to_string());
        let report = import_graphify(&path, &bundle, options()).unwrap();
        assert_eq!(
            check(&report, "graphify.projection"),
            if depth == 256 { Pass } else { Unknown }
        );
        let alias = bundle.join("alternate.json");
        fs::copy(bundle.join(MANIFEST), &alias).unwrap();
        let output = bundle.join("recovered");
        assert_eq!(
            export_graphify(&alias, &output).unwrap().exit_code(),
            report.exit_code()
        );
        assert_eq!(fs::read(output).unwrap(), raw.as_bytes());
    }

    // Reconstruct a full index at raw depth 257, one layer past today's
    // producer projection. Older full indexes were publishable at this depth.
    let bundle = dir.path().join("257");
    let mut legacy = index(&bundle);
    let mut full = load_observations(&dir.path().join("256"))
        .unwrap()
        .index
        .unwrap();
    let revision = &legacy["artifact"]["revision"];
    let id = format!(
        "observation.node.h{:x}",
        <sha2::Sha256 as sha2::Digest>::digest(format!("{}/nodes/0", revision.as_str().unwrap()))
    );
    let node = &mut full["nodes"][0];
    node["identity"]["artifact_revision"] = revision.clone();
    node["id"] = id.clone().into();
    node["raw"]["vendor"] = Value::Array(vec![node["raw"]["vendor"].take()]);
    full["binding_candidates"][0]["observation"] = id.into();
    legacy["nodes"] = full["nodes"].take();
    legacy["binding_candidates"] = full["binding_candidates"].take();
    for (version, result) in [("0.1.0", Unknown), ("0.1.1", Unknown), ("0.1.2", Fail)] {
        legacy["importer"]["version"] = version.into();
        fs::write(bundle.join(MANIFEST), serde_json::to_vec(&legacy).unwrap()).unwrap();
        let report = verify_graphify(&bundle, None, None).unwrap();
        assert_eq!(check(&report, "graphify.integrity"), Pass);
        assert_eq!(check(&report, "graphify.projection"), result);
        let dest = bundle.join(version);
        assert_eq!(export_graphify(&bundle, &dest).is_ok(), result != Fail);
        if result != Fail {
            assert_eq!(fs::read(dest).unwrap(), fs::read(bundle.join(RAW)).unwrap());
        }
    }

    let manifest = dir.path().join(MANIFEST);
    for (depth, result) in [(600, Fail), (1024, Fail), (1200, Unknown)] {
        let nested = format!(
            r#"{{"format":"gdd-observations/0.2","extra":{}0{}}}"#,
            r#"{"nested":"#.repeat(depth - 1),
            "}".repeat(depth - 1)
        );
        fs::write(&manifest, nested).unwrap();
        let loaded = load_observations(&manifest).unwrap();
        assert_eq!(check(&loaded.report, "graphify.manifest"), result);
        assert!(loaded.raw.is_none());
    }
    assert!(export_graphify(&manifest, &dir.path().join("unverified")).is_err());
}

#[test]
fn concurrent_publication_has_one_winner_and_cleans_staging() {
    let dir = tempdir().unwrap();
    let source = dir.path().join("input");
    let output = dir.path().join("bundle");
    let recovered = dir.path().join("recovered");
    fs::write(&source, VALID_AST).unwrap();
    for importing in [true, false] {
        let ready = Barrier::new(4);
        let success = std::thread::scope(|threads| {
            let jobs: Vec<_> = (0..4)
                .map(|_| {
                    threads.spawn(|| {
                        ready.wait();
                        if importing {
                            import_graphify(&source, &output, options())
                        } else {
                            export_graphify(&output, &recovered)
                        }
                    })
                })
                .collect();
            jobs.into_iter()
                .map(|j| j.join().unwrap())
                .filter(Result::is_ok)
                .count()
        });
        assert_eq!(success, 1);
    }
    assert_eq!(fs::read(&recovered).unwrap(), fs::read(&source).unwrap());
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 3);
}

#[cfg(unix)]
#[test]
fn local_symlinks_non_unicode_uris_and_export_permissions() {
    use std::ffi::OsStr;
    use std::os::unix::{
        ffi::OsStrExt,
        fs::{PermissionsExt, symlink},
    };
    let dir = tempdir().unwrap();
    let source = dir.path().join(OsStr::from_bytes(b"test +:%\xff.json"));
    let bundle = dir.path().join("bundle");
    fs::write(&source, VALID_AST).unwrap();
    assert_eq!(
        import_graphify(&source, &bundle, options())
            .unwrap()
            .exit_code(),
        0
    );
    assert!(
        index(&bundle)["input_claims"]["locator"]
            .as_str()
            .unwrap()
            .ends_with("test%20%2B%3A%25%FF.json")
    );
    let dest = dir.path().join("export");
    let output = Command::new("sh")
        .args([
            "-c",
            r#"umask 027; exec "$@""#,
            "sh",
            env!("CARGO_BIN_EXE_gdd"),
            "graphify",
            "export",
        ])
        .arg(&bundle)
        .arg("--output")
        .arg(&dest)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(dest.metadata().unwrap().permissions().mode() & 0o777, 0o640);

    let raw = bundle.join(RAW);
    fs::remove_file(&raw).unwrap();
    for target in [&source, &raw] {
        symlink(target, &raw).unwrap();
        let loaded = load_observations(&bundle).unwrap();
        assert_eq!(check(&loaded.report, "graphify.raw"), Fail);
        assert_eq!(
            loaded.report.checks[0].diagnostics[0].code,
            "ASSET_PATH_INVALID"
        );
        assert!(loaded.raw.is_none());
        fs::remove_file(&raw).unwrap();
    }
    let dangling = dest.with_extension("dangling");
    symlink(&raw, &dangling).unwrap();
    assert!(import_graphify(&source, &dangling, options()).is_err());
    let loaded = load_observations(&bundle).unwrap();
    assert_eq!(check(&loaded.report, "graphify.raw"), Unknown);
    fs::write(bundle.join("inside"), fs::read(source).unwrap()).unwrap();
    symlink("inside", raw).unwrap();
    assert_eq!(verify_graphify(&bundle, None, None).unwrap().exit_code(), 0);
}

#[test]
fn malformed_manifest_prevents_recovery_and_explains_the_failure() {
    let dir = tempdir().unwrap();
    let manifest = dir.path().join(MANIFEST);
    fs::write(
        &manifest,
        br#"{"format":"gdd-observations/0.2","format":"gdd-observations/0.2"}"#,
    )
    .unwrap();
    assert_eq!(
        check(
            &load_observations(&manifest).unwrap().report,
            "graphify.manifest"
        ),
        Fail
    );
    let bundle = Path::new("examples/graphify/observations");
    let mut value = index(bundle);
    value["source_id"] = json!("source.valid\n");
    fs::write(&manifest, serde_json::to_vec(&value).unwrap()).unwrap();
    let loaded = load_observations(&manifest).unwrap();
    assert!(loaded.index.is_none());
    assert!(loaded.report.checks[0].reason.contains("/source_id"));
    let out = dir.path().join("out");
    assert!(export_graphify(&manifest, &out).is_err());
    assert!(!out.exists());
}
