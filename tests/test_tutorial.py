"""Run the documented planning tutorial against the real CLI, without services."""

import json
from pathlib import Path
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]


def test_planning_tutorial_sequence(tmp_path):
    def cli(*arguments, expected=0):
        process = subprocess.run([sys.executable, "-m", "gdd_validator", *map(str, arguments)],
                                 cwd=ROOT, capture_output=True, text=True, timeout=30)
        assert process.returncode == expected, process.stdout + process.stderr
        return json.loads(process.stdout) if process.stdout.strip() else None

    project = tmp_path / "project"
    cli("init", project, "--project-id", "project.integer-addition", "--title", "Local integer addition")
    sketch = (project / "gdd.json").read_bytes()
    cli("validate", project / "gdd.json", "--profile", "model")
    cli("validate", project / "gdd.json", "--profile", "readiness", "--scope", "action.main", expected=1)
    refined = tmp_path / "refined.gdd.json"
    shutil.copyfile(ROOT / "docs/tutorial/refined.gdd.json", refined)
    cli("validate", refined, "--profile", "model")
    ready = cli("validate", refined, "--profile", "readiness", "--scope", "action.add")
    assert all(c["result"] == "PASS" for c in ready["checks"])
    graph = tmp_path / "graph.html"
    cli("view", refined, "--output", graph)
    cli("validate", refined, "--profile", "visualization", "--view", graph)
    context = tmp_path / "context.json"
    cli("context", refined, "--scope", "action.add", "--output", context)
    packet = json.loads(context.read_text())
    assert packet["spec_revision"] == ready["spec_revision"]
    assert "action.add" in context.read_text() and "component.arithmetic" in context.read_text()
    assert (project / "gdd.json").read_bytes() == sketch
    assert not list(tmp_path.rglob("*.py"))  # Planning did not invent an implementation.
