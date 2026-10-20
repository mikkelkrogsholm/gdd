"""Local checks must expose failures and preserve earlier verification output."""

import json
import subprocess
import sys

import pytest

from scripts import check


def test_missing_child_command_is_a_recorded_failure(tmp_path):
    record = check.run_step("missing", [str(tmp_path / "missing-command")], tmp_path)
    assert record["exit_code"] == 127 and record["result"] == "FAIL"
    assert (tmp_path / record["stderr"]).read_text()


def test_failure_stops_release_before_build_and_is_preserved(tmp_path, monkeypatch):
    calls = []

    def execute(name, argv, output, **kwargs):
        calls.append(name)
        return {"name": name, "exit_code": 1 if name == "pytest-current" else 0}

    monkeypatch.setattr(check, "run_step", execute)
    report = check.run_checks(tmp_path, release=True)
    assert not report["passed"]
    assert calls == ["skill-sync", "pytest-current"]
    assert json.loads((tmp_path / "summary.json").read_text())["steps"][-1]["exit_code"] == 1


def test_matrix_is_isolated_and_missing_interpreter_does_not_pass(tmp_path, monkeypatch):
    calls = []

    def execute(name, argv, output, **kwargs):
        calls.append((name, argv))
        return {"name": name, "exit_code": 2 if name == "pytest-3.12" else 0}

    monkeypatch.setattr(check, "run_step", execute)
    report = check.run_checks(tmp_path, matrix=True, quick=True)
    assert not report["passed"]
    assert [name for name, _ in calls] == ["skill-sync", "pytest-3.11", "pytest-3.12"]
    for _, command in calls[1:]:
        assert "--isolated" in command and "--no-python-downloads" in command
        assert "--locked" in command and "no:cacheprovider" in command


def test_quick_selection_is_bounded_and_full_suite_has_no_file_filter():
    assert all((check.ROOT / path).is_file() for path in check.QUICK_TESTS)
    assert set(check.QUICK_TESTS) <= set(check.pytest_command(None, True))
    assert not any(path.startswith("tests/") for path in check.pytest_command(None, False))
    current = check.pytest_command(f"{sys.version_info.major}.{sys.version_info.minor}", True)
    assert current[current.index("--python") + 1] == sys.executable


def test_existing_output_is_not_reused(tmp_path):
    marker = tmp_path / "preserve.txt"
    marker.write_text("original")
    result = subprocess.run([sys.executable, str(check.ROOT / "scripts/check.py"),
                             "--output", str(tmp_path)], capture_output=True, text=True)
    assert result.returncode == 1
    assert marker.read_text() == "original"
    assert sorted(path.name for path in tmp_path.iterdir()) == ["preserve.txt"]


def test_mutually_exclusive_modes_do_not_start_checks(tmp_path):
    output = tmp_path / "unused"
    result = subprocess.run([sys.executable, str(check.ROOT / "scripts/check.py"),
                             "--quick", "--release", "--output", str(output)],
                            capture_output=True, text=True)
    assert result.returncode == 2 and not output.exists()


def test_logs_are_exclusive(tmp_path, monkeypatch):
    (tmp_path / "check.stdout.txt").write_text("earlier execution")

    def unexpected_execution(*args, **kwargs):
        pytest.fail("A reused log must be refused before starting another execution.")

    monkeypatch.setattr(check.subprocess, "run", unexpected_execution)
    with pytest.raises(FileExistsError):
        check.run_step("check", [sys.executable, "-c", "print('new execution')"], tmp_path)
    assert (tmp_path / "check.stdout.txt").read_text() == "earlier execution"
