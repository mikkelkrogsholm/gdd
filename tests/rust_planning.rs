use gdd::bundle::{self, Bundle};
use gdd::input;
use gdd::report::{CheckResult, Report};
use gdd::validation::{self, Profile, ValidationOptions};
use serde_json::{Value, json};
use std::fs;
use std::path::Path;
use std::process::{Command, Output};

fn write(path: &Path, value: &Value) {
    fs::write(path, serde_json::to_vec(value).unwrap()).unwrap();
}

fn validate(bundle: &Bundle, value: &Value, package: bool) -> Report {
    let input = input::parse(&serde_json::to_vec(value).unwrap()).unwrap();
    validation::validate_with_options(
        bundle,
        ValidationOptions {
            profiles: &[Profile::Readiness],
            package: package.then_some(&input),
            parallel_plan: (!package).then_some(&input),
            ..Default::default()
        },
    )
    .unwrap()
}

fn cli(path: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_gdd"))
        .arg("validate")
        .arg(path)
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn candidate_package_compares_fixed_point_and_structural_readiness_independently() {
    let path = Path::new("examples/concert.gdd.yaml");
    let scope = ["action.reserve".to_owned()];
    let bundle = bundle::load_for_readiness(path, &scope);
    let context = gdd::context::context_manifest(&bundle, &scope).unwrap();
    let mut candidate = serde_json::to_value(context).unwrap();
    let valid = validate(&bundle, &candidate, true);
    assert_eq!(valid.result("readiness"), CheckResult::Pass);
    assert_eq!(valid.scope, scope);
    candidate["spec_revision"] = json!("obsolete");
    assert_eq!(
        validate(&bundle, &candidate, true).result("readiness"),
        CheckResult::Unknown
    );
    candidate["nodes"]
        .as_array_mut()
        .unwrap()
        .retain(|id| !id.as_str().unwrap().starts_with("rule."));
    candidate["edges"] = json!(["edge.absent"]);
    let incomplete = validate(&bundle, &candidate, true);
    let check = incomplete
        .checks
        .iter()
        .find(|c| c.check_id == "context.package")
        .unwrap();
    let codes: Vec<_> = check.diagnostics.iter().map(|d| d.code).collect();
    assert!(codes.contains(&"APPLICABLE_RULE_OMITTED"));
    assert!(codes.contains(&"READINESS_CONTRACT_MISSING"));
    assert!(codes.contains(&"REFERENCE_UNRESOLVED"));
    assert_eq!(check.result, CheckResult::Fail);
    assert!(
        incomplete
            .checks
            .iter()
            .any(|c| c.check_id == "readiness.structural" && c.result == CheckResult::Pass)
    );
}

fn fixture(directory: &Path) -> (Bundle, Value, Value) {
    let manifest = directory.join("gdd.json");
    let records_file = directory.join("records.json");
    write(
        &manifest,
        &json!({
            "format":"gdd/0.2", "project":{"id":"project.test","title":"Plans","goal":"Check integration declarations."},
            "nodes":{"entity.x":{"kind":"entity","description":"One entity.","fields":{}},
                     "entity.y":{"kind":"entity","description":"Another entity.","fields":{}},
                     "rule.global":{"kind":"rule","description":"Model invariant.","rule":{
                        "category":"invariant","statement":"Check both methods.","scope":"global","blocking":true,
                        "verification":["method.a","method.b"]}}},
            "edges":[],"record_files":["records.json"]
        }),
    );
    let mut records = json!({"format":"gdd-records/0.2","project_id":"project.test","records":{}});
    write(&records_file, &records);
    let revision = bundle::load_model_only(&manifest)
        .snapshot()
        .unwrap()
        .revision
        .clone();
    for (id, method) in [("check.a", "method.a"), ("check.b", "method.b")] {
        records["records"][id] = json!({"kind":"check","method":method,"scope":["rule.global"],
            "spec_revision":revision,"required_environment":[],"negative_controls":[]});
    }
    write(&records_file, &records);
    let packages:Vec<_> = ["y","x"].iter().map(|name| json!({"id":format!("package.{name}"),"scope":[format!("entity.{name}")],
        "write_resources":[format!("src/{name}")],"read_resources":[],"workspace":format!("work/{name}"),
        "base_revision":revision,"integration_plan":"Run both checks."})).collect();
    let plan = json!({"format":"gdd-parallel-plan/0.2","serialized":[],"packages":packages,
        "integration":{"owner":"owner","checks":["check.a","check.b"]}});
    (bundle::load_model_only(&manifest), records, plan)
}

#[test]
fn parallel_uses_valid_current_checks_and_a_separate_structural_assessment() {
    let directory = tempfile::tempdir().unwrap();
    let (bundle, mut records, mut plan) = fixture(directory.path());
    let assessed = validate(&bundle, &plan, false);
    assert_eq!(assessed.result("readiness"), CheckResult::Pass);
    assert_eq!(assessed.scope, ["entity.y", "entity.x"]);
    let parallel = assessed
        .checks
        .iter()
        .find(|c| c.check_id == "parallel.independence")
        .unwrap();
    assert_eq!(parallel.scope, ["entity.x", "entity.y"]);
    assert!(
        !assessed
            .checks
            .iter()
            .any(|c| c.check_id == "readiness.structural")
    );
    plan["integration"]["checks"] = json!(["check.a"]);
    assert_eq!(
        validate(&bundle, &plan, false).result("readiness"),
        CheckResult::Unknown
    );

    plan["integration"]["checks"] = json!(["check.a", "check.b"]);
    let manifest = directory.path().join("gdd.json");
    let records_file = directory.path().join("records.json");
    records["records"]["check.next"] = records["records"]["check.b"].clone();
    records["records"]["check.next"]["supersedes"] = json!("check.b");
    write(&records_file, &records);
    assert_eq!(
        validate(&bundle::load_model_only(&manifest), &plan, false).result("readiness"),
        CheckResult::Unknown
    );
    plan["integration"]["checks"] = json!(["check.a", "check.next"]);
    assert_eq!(
        validate(&bundle::load_model_only(&manifest), &plan, false).result("readiness"),
        CheckResult::Pass
    );
    records["records"]["check.b"]["supersedes"] = json!("check.next");
    write(&records_file, &records);
    let cycle = validate(&bundle::load_model_only(&manifest), &plan, false);
    assert_eq!(cycle.result("readiness"), CheckResult::Unknown);
    assert!(
        cycle
            .checks
            .iter()
            .flat_map(|c| &c.diagnostics)
            .any(|d| d.message.contains("retired checks: ['check.next']"))
    );
}

#[test]
fn parallel_compares_logical_write_paths_and_resolved_workspaces_without_creating_them() {
    let directory = tempfile::tempdir().unwrap();
    let (bundle, _, mut plan) = fixture(directory.path());
    plan["packages"][0]["write_resources"] = json!(["src/a//child/.."]);
    plan["packages"][1]["write_resources"] = json!(["src/a/nested/b.rs"]);
    plan["packages"][1]["workspace"] = json!("unused/../work/y");
    let conflict = validate(&bundle, &plan, false);
    assert_eq!(conflict.result("readiness"), CheckResult::Fail);
    assert_eq!(
        conflict
            .checks
            .iter()
            .find(|c| c.check_id == "parallel.ownership")
            .unwrap()
            .diagnostics
            .len(),
        2
    );
    plan["serialized"] = json!([["package.x", "package.y"]]);
    assert_eq!(
        validate(&bundle, &plan, false).result("readiness"),
        CheckResult::Pass
    );
    plan["packages"][1]["write_resources"] = json!(["src/[ab].rs"]);
    assert_eq!(
        validate(&bundle, &plan, false).result("readiness"),
        CheckResult::Unknown
    );
    plan["serialized"] = json!([]);
    plan["packages"][0]["write_resources"] = json!(["src/\u{1}.rs"]);
    plan["packages"][1]["write_resources"] = json!(["src/\u{1}.rs"]);
    let control = validate(&bundle, &plan, false);
    assert!(
        control
            .checks
            .iter()
            .flat_map(|c| &c.diagnostics)
            .any(|d| d.message == r"Unserialized write overlap: [('src/\x01.rs', 'src/\x01.rs')].")
    );
    assert!(!directory.path().join("work").exists());
}

#[cfg(unix)]
#[test]
fn parallel_workspace_symlink_prefix_applies_before_parent_components() {
    let directory = tempfile::tempdir().unwrap();
    let (bundle, _, mut plan) = fixture(directory.path());
    std::fs::create_dir_all(directory.path().join("real/child")).unwrap();
    std::os::unix::fs::symlink("real/child", directory.path().join("link")).unwrap();
    plan["packages"][0]["workspace"] = json!("link/../future");
    plan["packages"][1]["workspace"] = json!("real/future");
    assert_eq!(
        validate(&bundle, &plan, false).result("readiness"),
        CheckResult::Fail
    );
    assert!(!directory.path().join("real/future").exists());
}

#[test]
fn cli_resolves_envelope_scope_for_all_profiles_and_rejects_conflicts_and_capacity_errors() {
    let directory = tempfile::tempdir().unwrap();
    let (bundle, _, plan) = fixture(directory.path());
    let manifest = directory.path().join("gdd.json");
    let plan_file = directory.path().join("plan.json");
    let package_file = directory.path().join("package.json");
    write(&plan_file, &plan);
    let plan_path = plan_file.to_str().unwrap();
    let package_path = package_file.to_str().unwrap();
    let model_only = cli(
        &manifest,
        &[
            "--parallel-plan",
            plan_path,
            "--scope",
            "entity.x",
            "--scope",
            "entity.y",
        ],
    );
    assert!(model_only.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&model_only.stdout).unwrap()["scope"],
        json!(["entity.y", "entity.x"])
    );
    let conflict = cli(
        &manifest,
        &["--parallel-plan", plan_path, "--scope", "entity.x"],
    );
    assert_eq!(conflict.status.code(), Some(1));
    assert!(conflict.stdout.is_empty());
    let scope = ["entity.y".to_string(), "entity.x".to_string()];
    write(
        &package_file,
        &serde_json::to_value(gdd::context::context_manifest(&bundle, &scope).unwrap()).unwrap(),
    );
    assert_eq!(
        cli(
            &manifest,
            &["--package", package_path, "--parallel-plan", plan_path]
        )
        .status
        .code(),
        Some(1)
    );
    write(&package_file, &Value::Null);
    assert!(
        cli(
            &manifest,
            &[
                "--package",
                package_path,
                "--parallel-plan",
                plan_path,
                "--profile",
                "readiness",
                "--explain"
            ]
        )
        .status
        .success()
    );
    fs::write(
        &plan_file,
        format!("{}0{}", "[".repeat(1800), "]".repeat(1800)),
    )
    .unwrap();
    assert_eq!(
        cli(&manifest, &["--parallel-plan", plan_path])
            .status
            .code(),
        Some(2)
    );
}

#[test]
fn empty_and_unknown_package_scopes_preserve_their_distinct_closures() {
    let directory = tempfile::tempdir().unwrap();
    let (bundle, _, _) = fixture(directory.path());
    let mut package = json!({"format":"gdd-context/0.2", "spec_revision":bundle.snapshot().unwrap().revision,
        "scope":[], "nodes":["rule.global"], "edges":[], "sources":[], "questions":[], "extensions":[]});
    let assessment = validate(&bundle, &package, true);
    assert!(assessment.scope.is_empty());
    assert_eq!(
        assessment
            .checks
            .iter()
            .find(|c| c.check_id == "context.package")
            .unwrap()
            .result,
        CheckResult::Pass
    );
    assert_eq!(assessment.result("readiness"), CheckResult::Fail);
    package["nodes"] = json!([]);
    let path = directory.path().join("package.json");
    write(&path, &package);
    let explanation = cli(
        &directory.path().join("gdd.json"),
        &[
            "--profile",
            "readiness",
            "--package",
            path.to_str().unwrap(),
            "--explain",
        ],
    );
    let stderr = String::from_utf8(explanation.stderr).unwrap();
    assert!(
        stderr.contains(
            "Context omits rule.global.\n    <global> -> rule.global (global-rule-context)"
        )
    );
    package["nodes"] = json!(["rule.global"]);
    package["scope"] = json!(["entity.unknown"]);
    let assessment = validate(&bundle, &package, true);
    assert!(
        assessment
            .checks
            .iter()
            .find(|c| c.check_id == "context.package")
            .unwrap()
            .diagnostics
            .iter()
            .any(|d| d.related_ids == ["entity.unknown"] && d.code == "READINESS_CONTRACT_MISSING")
    );
    package["nodes"] = json!(["rule.global", "entity.unknown"]);
    let assessment = validate(&bundle, &package, true);
    assert!(
        assessment
            .checks
            .iter()
            .find(|c| c.check_id == "context.package")
            .unwrap()
            .diagnostics
            .iter()
            .any(|d| d.code == "REFERENCE_UNRESOLVED")
    );
}
