"""The public comparison surface preserves meaning, inputs and output files."""

import json
from pathlib import Path
import subprocess
import sys

import pytest

from gdd_validator import diff_models

ROOT = Path(__file__).resolve().parents[1]
READY = ROOT / "docs/tutorial/refined.gdd.json"


def cli(*arguments):
    return subprocess.run([sys.executable, "-m", "gdd_validator", "diff", *map(str, arguments)],
                          cwd=ROOT, capture_output=True, text=True, timeout=30)


def test_actual_contract_change_agrees_with_api_and_text(tmp_path):
    model = json.loads(READY.read_text())
    model["nodes"]["action.add"]["contract"]["postconditions"] = []
    head = tmp_path / "head.json"
    head.write_text(json.dumps(model))
    before = {READY: READY.read_bytes(), head: head.read_bytes()}
    execution = cli(READY, head)
    assert execution.returncode == 0, execution.stdout + execution.stderr
    result = json.loads(execution.stdout)
    assert result == diff_models(READY, head)
    assert result["format"] == "gdd-model-diff/0.1"
    assert result["comparison"]["semantic_changes"] is True
    assert result["advisories"]
    text = cli(READY, head, "--format", "text")
    assert text.returncode == 0 and "action.add" in text.stdout
    assert "postconditions" in text.stdout and text.stdout.endswith("\n")
    assert all(path.read_bytes() == data for path, data in before.items())


@pytest.mark.parametrize("format_name", ["json", "text"])
def test_output_is_exclusive_and_dangling_symlink_is_refused(tmp_path, format_name):
    output = tmp_path / "comparison"
    execution = cli(READY, READY, "--format", format_name, "--output", output)
    assert execution.returncode == 0 and not execution.stdout, execution.stderr
    before = output.read_bytes()
    repeated = cli(READY, READY, "--format", format_name, "--output", output)
    assert repeated.returncode == 1 and output.read_bytes() == before
    alias, target = tmp_path / "alias", tmp_path / "missing"
    alias.symlink_to(target)
    refused = cli(READY, READY, "--format", format_name, "--output", alias)
    assert refused.returncode == 1 and alias.is_symlink() and not target.exists()


def test_missing_side_is_not_a_successful_empty_diff(tmp_path):
    execution = cli(READY, tmp_path / "missing.json")
    assert execution.returncode in {1, 2}, execution.stdout + execution.stderr
    assert "Traceback" not in execution.stderr
    result = json.loads(execution.stdout)
    assert result["exit_code"] == execution.returncode
    assert result["comparison"]["coverage"] != "complete"
    assert result["diagnostics"]
