use super::*;
use crate::host::{CheckCommand, execute_check_v01};
use serde_json::{Map, Value, json};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

const TIMEOUT: Duration = Duration::from_secs(10);

fn command<'a>(argv: &'a [&'a str]) -> CheckCommand<'a> {
    CheckCommand { argv, env: &[] }
}

#[test]
fn receipt_records_actual_status_argv_environment_and_source_identity() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let binary = build_checker(root);
    fs::write(root.join("marker"), b"in cwd").unwrap();
    let files = &["marker"];
    let snapshot = snapshot_code(root, files).unwrap();
    // Include historical and producer-specific keys to exercise lossless copying.
    let runner = json!({"python": null, "platform": env::consts::OS, "fixture": [1, "é"]});
    let arg = "a literal $HOME ; | * é";
    for (code, label) in [(0, "PASS"), (1, "FAIL"), (37, "UNKNOWN")] {
        let path = root.join(label);
        // The sentinel spelling is a valid literal argument on Unix.
        let argv = [
            binary.to_str().unwrap(),
            "echo-args",
            &code.to_string(),
            arg,
            "<string-with-nul>",
        ];
        let command = CheckCommand {
            argv: &argv,
            env: &[("CHECK_VALUE".as_ref(), "caller".as_ref())],
        };
        let earliest = OffsetDateTime::now_utc();
        let receipt = execute_check_v01(
            &root.join("."),
            files,
            &path,
            command,
            TIMEOUT,
            runner.as_object().unwrap(),
        )
        .unwrap();
        let latest = OffsetDateTime::now_utc();
        assert_eq!(
            serde_json::from_slice::<Value>(&fs::read(&path).unwrap()).unwrap(),
            receipt
        );
        assert_eq!(receipt["format"], "gdd-host-check/0.1");
        assert_eq!(receipt["runner"], runner);
        assert_eq!(receipt["command"], json!(argv));
        assert_eq!(
            receipt["cwd"],
            crate::receipt_path(&root.canonicalize().unwrap()).unwrap()
        );
        assert_eq!(receipt["snapshot"], json!(snapshot));
        assert_eq!(receipt["code_revision"], snapshot.revision);
        assert_eq!(receipt["code_revision_after"], snapshot.revision);
        assert_eq!(receipt["executed"], true);
        assert_eq!(receipt["result"], label);
        assert_eq!(receipt["exit_code"], code);
        assert_eq!(receipt["stdout"], format!("{arg}\n<string-with-nul>"));
        assert_eq!(receipt["stderr"], "caller:in cwd");
        assert!(receipt["reason"].as_str().unwrap().contains("completed"));
        let time =
            |key: &str| OffsetDateTime::parse(receipt[key].as_str().unwrap(), &Rfc3339).unwrap();
        let (start, finish) = (time("started_at"), time("finished_at"));
        assert!(start.offset().is_utc() && finish.offset().is_utc());
        assert!(earliest <= start && start <= finish && finish <= latest);
    }
}

#[test]
fn receipt_preserves_output_for_completion_timeout_and_start_failure() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let binary = build_checker(root);
    let binary = binary.to_str().unwrap();
    let run = |name: &str, argv: &[&str], timeout| {
        execute_check_v01(
            root,
            &[],
            &root.join(name),
            command(argv),
            timeout,
            &Map::new(),
        )
        .unwrap()
    };
    let completed = run("completed", &[binary, "file-check", "output"], TIMEOUT);
    assert_eq!(completed["stdout"], "out�\nend\n");
    assert_eq!(completed["stderr"], "err�\nbar\n");
    let timed_out = run(
        "timeout",
        &[binary, "file-check", "output", "wait"],
        Duration::from_secs(1),
    );
    assert_eq!(timed_out["executed"], true);
    assert_eq!(timed_out["result"], "UNKNOWN");
    assert_eq!(timed_out["exit_code"], Value::Null);
    assert_eq!(timed_out["stdout"], "out�\r\nend\r");
    assert_eq!(timed_out["stderr"], "err�\rbar\r\n");
    assert!(timed_out["reason"].as_str().unwrap().contains("timed out"));

    let unavailable = run(
        "unavailable",
        &[root.join("missing").to_str().unwrap()],
        TIMEOUT,
    );
    assert_eq!(unavailable["executed"], false);
    assert_eq!(unavailable["result"], "UNKNOWN");
    assert_eq!(unavailable["exit_code"], Value::Null);
    assert_eq!(unavailable["stdout"], "");
    assert_eq!(unavailable["stderr"], "");
    assert!(
        unavailable["reason"]
            .as_str()
            .unwrap()
            .contains("could not be started")
    );
    assert_eq!(
        unavailable["code_revision"],
        unavailable["code_revision_after"]
    );

    #[cfg(unix)]
    {
        let signaled = run("signaled", &[binary, "file-check", "abort"], TIMEOUT);
        assert_eq!(signaled["executed"], true);
        assert_eq!(signaled["result"], "UNKNOWN");
        assert!(signaled["exit_code"].as_i64().unwrap() < 0);
    }
}

#[test]
fn receipt_rejects_changed_or_unavailable_sources_even_after_success() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let binary = build_checker(root);
    for action in ["change", "remove"] {
        fs::write(root.join("selected"), "original").unwrap();
        let files = &["selected"];
        let before = snapshot_code(root, files).unwrap();
        let receipt = execute_check_v01(
            root,
            files,
            &root.join(action),
            command(&[binary.to_str().unwrap(), "file-check", action]),
            TIMEOUT,
            &Map::new(),
        )
        .unwrap();
        assert_eq!(receipt["snapshot"], json!(before));
        assert_eq!(receipt["code_revision"], before.revision);
        assert_eq!(receipt["executed"], true);
        assert_eq!(receipt["exit_code"], 0);
        assert_eq!(receipt["result"], "UNKNOWN");
        let after = snapshot_code(root, files).ok().map(|s| s.revision);
        assert_ne!(after.as_ref(), Some(&before.revision));
        assert_eq!(receipt["code_revision_after"], json!(after));
        let reason = if action == "change" {
            "changed"
        } else {
            "unresolved"
        };
        assert!(receipt["reason"].as_str().unwrap().contains(reason));
    }
}

#[test]
fn receipt_preflight_and_exclusive_reservation_prevent_duplicate_effects() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let binary = build_checker(root);
    let path = root.join("receipt");
    let binary = binary.to_str().unwrap();
    let argv = &[binary, "file-check"];
    let run = |argv: &[&str], timeout| {
        execute_check_v01(root, &[], &path, command(argv), timeout, &Map::new())
    };
    for invalid in [
        &[][..],
        &[""],
        &["bad\0program"],
        &[binary, "file-check", ""],
        &[binary, "file-check", "bad\0arg"],
    ] {
        assert_eq!(
            run(invalid, TIMEOUT).unwrap_err().kind(),
            io::ErrorKind::InvalidInput
        );
    }
    let day = Duration::from_secs(86400);
    for timeout in [Duration::ZERO, day + Duration::from_nanos(1)] {
        assert_eq!(
            run(argv, timeout).unwrap_err().kind(),
            io::ErrorKind::InvalidInput
        );
    }
    assert!(!path.exists());
    assert!(!root.join("invocations").exists());
    assert!(
        execute_check_v01(
            root,
            &["missing"],
            &path,
            command(argv),
            TIMEOUT,
            &Map::new()
        )
        .is_err()
    );
    assert!(!path.exists());
    // A missing output directory is an I/O error, distinct from a checker that could not start.
    let missing_parent = root.join("absent/receipt");
    let reservation = execute_check_v01(
        root,
        &[],
        &missing_parent,
        command(argv),
        TIMEOUT,
        &Map::new(),
    );
    assert_eq!(reservation.unwrap_err().kind(), io::ErrorKind::NotFound);
    assert!(!root.join("invocations").exists());
    let receipt = run(argv, day).unwrap();
    assert_eq!(receipt["result"], "PASS");
    assert_eq!(
        run(argv, day).unwrap_err().kind(),
        io::ErrorKind::AlreadyExists
    );
    assert_eq!(fs::read(root.join("invocations")).unwrap(), b"run\n");
    assert_eq!(
        serde_json::from_slice::<Value>(&fs::read(path).unwrap()).unwrap(),
        receipt
    );
}
