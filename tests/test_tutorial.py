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
    before_inspection = refined.read_bytes()
    status = cli("status", refined, "--scope", "action.add")
    assert status["assessments"]["readiness"]["result"] == "PASS"
    assert status["assessments"]["authorization"]["assessed"] is False
    impact = cli("impact", refined, "--scope", "action.add")
    assert impact["coverage"]["result"] == "complete"
    assert refined.read_bytes() == before_inspection
    assert (project / "gdd.json").read_bytes() == sketch
    assert not list(tmp_path.rglob("*.py"))  # Planning did not invent an implementation.


def test_complete_documented_tutorial(tmp_path):
    """Execute every shell block verbatim; the Python excerpt is illustrative."""
    import os
    import re

    from gdd_validator import validate
    from gdd_validator.host import snapshot_code
    from gdd_validator.loading import load_bundle
    from gdd_validator.report import aggregate

    document = ROOT / "docs/tutorial.md"
    blocks = re.findall(r"```sh\n(.*?)\n```", document.read_text(), re.DOTALL)
    # Functions select the same interpreter/environment for both documented tools.
    # mktemp uses our own temporary root so all artifacts remain inspectable.
    preamble = '''set -eu
    gdd() { "$GDD_TEST_PYTHON" -m gdd_validator "$@"; }
    python() { "$GDD_TEST_PYTHON" "$@"; }
    '''
    completed = subprocess.run(
        ["sh", "-c", preamble + "\n".join(blocks) + '\nprintf "\\nTUTORIAL_ROOT=%s\\n" "$GDD_TUTORIAL_DIR"'],
        cwd=ROOT, capture_output=True, text=True, timeout=90,
        env={**os.environ, "GDD_TEST_PYTHON": sys.executable, "TMPDIR": str(tmp_path)},
    )
    assert completed.returncode == 0, completed.stdout + completed.stderr
    root = Path(completed.stdout.rsplit("TUTORIAL_ROOT=", 1)[1].strip())
    files = ["integer_add.py", "test_integer_add.py"]
    good = json.loads((root / "pass.receipt.json").read_text())
    bad = json.loads((root / "negative.receipt.json").read_text())
    assert good["result"] == "PASS" and good["exit_code"] == 0 and good["executed"] is True
    assert bad["result"] == "FAIL" and bad["exit_code"] == 1 and bad["executed"] is True
    assert "AssertionError" in bad["stderr"] and "FAILED (failures=" in bad["stderr"]
    assert "ImportError" not in bad["stderr"]
    assert good["snapshot"] == snapshot_code(root, files)
    assert good["code_revision_after"] == good["code_revision"]
    assert (root / "refined.gdd.json").read_bytes() == (ROOT / "docs/tutorial/refined.gdd.json").read_bytes()
    assert (root / "integer_add.py").read_bytes() == (ROOT / "docs/tutorial/integer_add.py").read_bytes()
    model = root / "evidence.gdd.json"
    ledger = root / "records.json"
    records = json.loads(ledger.read_text())["records"]
    assert set(records) == {"check.sum", "binding.sum.v1", "evidence.sum.v1"}
    assert records["check.sum"]["spec_revision"] == load_bundle(model).revision
    assert records["evidence.sum.v1"]["code_revision"] == good["code_revision"]
    host = json.loads((root / "host-context.json").read_text())
    assert "policy" not in host and "trusted_decisions" not in host
    assert aggregate(validate(model, profiles=["records"], scope=["action.add"])["checks"], "implementation") == "UNKNOWN"
    assert aggregate(validate(model, profiles=["records"], scope=["action.add"], host_context=host)["checks"], "implementation") == "PASS"
    authorization = validate(model, profiles=["records"], scope=["action.add"], host_context=host, activity="acceptance")
    assert aggregate(authorization["checks"], "authorization") == "UNRESOLVED"
    stale_host = json.loads((root / "stale-host-context.json").read_text())
    assert stale_host["code_revision"] == snapshot_code(root / "changed", files)["revision"]
    assert stale_host["code_revision"] != good["code_revision"]
    stale = validate(model, profiles=["records"], scope=["action.add"], host_context=stale_host)
    assert aggregate(stale["checks"], "implementation") == "UNKNOWN"
    assert "EVIDENCE_STALE" in json.dumps(stale)
    assert json.loads((root / "stale-authoring.json").read_text())["result"] == "REFUSED"
    # The drift experiment preserves the original ledger and actual receipt bytes.
    assert (root / "changed/records.json").read_bytes() == ledger.read_bytes()
    assert (root / "changed/pass.receipt.json").read_bytes() == (root / "pass.receipt.json").read_bytes()
    preserved = {name: (root / name).read_bytes() for name in ["records.json", "pass.receipt.json", "host-context.json", "evidence.gdd.json"]}
    unattested = subprocess.run([sys.executable, root / "tutorial_host.py", "record", root],
                               cwd=ROOT, capture_output=True, text=True, timeout=30)
    assert unattested.returncode == 2 and "requires --attest-reviewed-checks" in unattested.stderr
    for stage in ["check", "record", "prepare", "negative", "drift"]:
        repeated = subprocess.run([sys.executable, root / "tutorial_host.py", stage, root, *(["--attest-reviewed-checks"] if stage == "record" else [])],
                                  cwd=ROOT, capture_output=True, text=True, timeout=30)
        assert repeated.returncode != 0, stage
        assert all((root / name).read_bytes() == content for name, content in preserved.items())
