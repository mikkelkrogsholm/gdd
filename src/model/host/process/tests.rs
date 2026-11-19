use super::*;
use std::{
    env, fs,
    path::{Path, PathBuf},
};

fn build_checker(dir: &Path) -> PathBuf {
    let binary = dir.join(format!("host checker{}", env::consts::EXE_SUFFIX));
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/host_checker.rs");
    let output = Command::new("rustc")
        .arg("--edition=2024")
        .arg(source)
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    binary
}

#[test]
fn captures_exit_status_literal_arguments_environment_and_working_directory() {
    let temp = tempfile::tempdir().unwrap();
    let binary = build_checker(temp.path());
    fs::write(temp.path().join("marker"), "in cwd").unwrap();
    let args = ["one argument", "; $HOME | * &", "", "another"];
    for exit in [0, 1, 37] {
        let mut command = Command::new(&binary);
        command
            .current_dir(temp.path())
            .stdin(std::process::Stdio::piped())
            .env_clear()
            .env("CHECK_VALUE", "set by caller")
            .args(["echo", &exit.to_string()])
            .args(args);
        let Execution::Completed(output) = collect(command, Duration::from_secs(5)).unwrap() else {
            panic!("checker should have completed");
        };
        assert_eq!(output.status.code(), Some(exit));
        assert_eq!(output.stdout, args.join("\n").as_bytes());
        assert_eq!(output.stderr, b"set by caller:in cwd");
    }
}

#[test]
fn captures_concurrent_binary_streams_larger_than_pipe_buffers() {
    let temp = tempfile::tempdir().unwrap();
    let mut command = Command::new(build_checker(temp.path()));
    command.arg("streams");
    let Execution::Completed(output) = collect(command, Duration::from_secs(5)).unwrap() else {
        panic!("checker should have completed");
    };
    assert!(output.status.success());
    assert_eq!(output.stdout, vec![0xff; 512 * 1024]);
    assert_eq!(output.stderr, vec![0xfe; 512 * 1024]);
}

#[test]
fn distinguishes_a_missing_executable_from_a_completed_check() {
    let temp = tempfile::tempdir().unwrap();
    let command = Command::new(temp.path().join("missing executable"));
    let Execution::NotStarted(error) = collect(command, Duration::from_secs(5)).unwrap() else {
        panic!("missing executable should fail to start");
    };
    assert_eq!(error.kind(), io::ErrorKind::NotFound);
}

#[test]
fn timeout_reaps_checker_and_returns_partial_output_while_a_descendant_holds_streams() {
    let temp = tempfile::tempdir().unwrap();
    let dir = temp.path();
    let mut command = Command::new(build_checker(dir));
    command.current_dir(dir).arg("timeout");
    let start = Instant::now();
    let execution = collect(command, Duration::from_secs(1)).unwrap();
    let elapsed = start.elapsed();
    let holder_still_running = dir.join("holding").exists() && !dir.join("released").exists();
    fs::write(dir.join("release"), b"").unwrap();
    let cleanup = Instant::now();
    while !dir.join("released").exists() && cleanup.elapsed() < Duration::from_secs(3) {
        thread::sleep(Duration::from_millis(10));
    }
    assert!(dir.join("released").exists(), "descendant did not finish");
    assert!(
        holder_still_running,
        "collector waited for inherited writers"
    );
    assert!(elapsed < Duration::from_secs(5), "timeout took {elapsed:?}");
    let Execution::TimedOut { stdout, stderr } = execution else {
        panic!("checker should have timed out");
    };
    assert_eq!(stdout, b"partial stdout\xff");
    assert_eq!(stderr, b"partial stderr\xfe");
    fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(dir.join("checker.lock"))
        .unwrap()
        .try_lock()
        .unwrap();
    #[cfg(target_os = "linux")]
    {
        let pid = fs::read_to_string(dir.join("checker.pid")).unwrap();
        assert!(
            !Path::new("/proc").join(pid).exists(),
            "checker was not reaped"
        );
    }
}

mod checked;
