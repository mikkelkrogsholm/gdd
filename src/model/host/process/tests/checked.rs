use super::*;
use std::io::Write;

const TIMEOUT: Duration = Duration::from_secs(10);

fn file_check(binary: &Path, actions: &[&str]) -> Command {
    let mut command = Command::new(binary);
    command.arg("file-check").args(actions);
    command
}

#[test]
fn completed_checks_report_unchanged_changed_and_unresolved_selected_files() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let binary = build_checker(root);
    let source = root.join("selected");
    for (name, actions) in [
        ("unchanged", &[][..]),
        ("changed", &["change"][..]),
        ("removed", &["remove"][..]),
    ] {
        fs::write(&source, "original").unwrap();
        let before = snapshot_code(root, &["selected"]).unwrap();
        let path = root.join(name);
        let mut command = file_check(&binary, actions);
        command.current_dir(root.join("overridden working directory"));
        let mut run = run_checked(root, &["selected"], &path, command, TIMEOUT).unwrap();
        assert!(matches!(run.execution, Execution::Completed(out) if out.status.success()));
        assert_eq!(run.before, before);
        match name {
            "unchanged" => assert_eq!(run.after.unwrap(), before),
            "changed" => {
                let after = run.after.unwrap();
                assert_ne!(after.revision, before.revision);
                assert_eq!(after, snapshot_code(root, &["selected"]).unwrap());
            }
            _ => assert_eq!(run.after.unwrap_err().kind(), io::ErrorKind::NotFound),
        }
        run.receipt.write_all(name.as_bytes()).unwrap();
        assert_eq!(fs::read(&path).unwrap(), name.as_bytes());
    }
    assert_eq!(
        fs::read(root.join("invocations")).unwrap(),
        b"run\nrun\nrun\n"
    );
}

#[test]
fn invalid_sources_and_existing_receipts_do_not_start_the_checker() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let binary = build_checker(root);
    fs::write(root.join("selected"), "original").unwrap();
    let path = root.join("receipt");
    for files in [
        &["../outside"][..],
        &["selected", "selected"][..],
        &["missing"][..],
    ] {
        assert!(run_checked(root, files, &path, file_check(&binary, &[]), TIMEOUT).is_err());
        assert!(!path.exists());
    }
    fs::write(&path, "keep this receipt").unwrap();
    let run = run_checked(
        root,
        &["selected"],
        &path,
        file_check(&binary, &[]),
        TIMEOUT,
    );
    assert_eq!(run.err().unwrap().kind(), io::ErrorKind::AlreadyExists);
    assert_eq!(fs::read(path).unwrap(), b"keep this receipt");
    assert!(!root.join("invocations").exists());
}

#[test]
fn reservation_blocks_overlapping_execution_and_retries_after_a_partial_attempt() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let binary = build_checker(root);
    let path = root.join("receipt");
    let run =
        |actions: &[&str]| run_checked(root, &[], &path, file_check(&binary, actions), TIMEOUT);
    thread::scope(|scope| {
        let first = scope.spawn(|| run(&["wait"]).unwrap());
        let start = Instant::now();
        while !root.join("invocations").exists() {
            assert!(start.elapsed() < TIMEOUT);
            thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(run(&[]).err().unwrap().kind(), io::ErrorKind::AlreadyExists);
        fs::write(root.join("release"), b"").unwrap();
        let mut checked = first.join().unwrap();
        assert!(matches!(checked.execution, Execution::Completed(out) if out.status.success()));
        checked.receipt.write_all(b"partial").unwrap();
    });
    assert_eq!(run(&[]).err().unwrap().kind(), io::ErrorKind::AlreadyExists);
    assert_eq!(fs::read(&path).unwrap(), b"partial");
    assert_eq!(fs::read(root.join("invocations")).unwrap(), b"run\n");
}

#[test]
fn timeout_and_start_failure_still_return_the_post_execution_snapshot() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let binary = build_checker(root);
    fs::write(root.join("selected"), "original").unwrap();
    let timed_out = run_checked(
        root,
        &["selected"],
        &root.join("timeout.receipt"),
        file_check(&binary, &["change", "wait"]),
        Duration::from_secs(1),
    )
    .unwrap();
    assert!(matches!(timed_out.execution, Execution::TimedOut { .. }));
    let after = timed_out.after.unwrap();
    assert_ne!(timed_out.before.revision, after.revision);
    assert_eq!(after, snapshot_code(root, &["selected"]).unwrap());

    let path = root.join("not-started.receipt");
    let unavailable = run_checked(
        root,
        &["selected"],
        &path,
        Command::new(root.join("missing executable")),
        TIMEOUT,
    )
    .unwrap();
    assert!(
        matches!(unavailable.execution, Execution::NotStarted(e) if e.kind() == io::ErrorKind::NotFound)
    );
    assert_eq!(unavailable.before, after);
    assert_eq!(unavailable.after.unwrap(), after);
    assert!(path.is_file());
}
