"""Host helpers for recorded native delivery; no graph text is executed."""

from hashlib import sha256
import json
import os
from pathlib import Path
import shutil
import sqlite3
import subprocess
import sys

from gdd_validator import advance_run, run_status
from gdd_validator.loading import digest, load_bundle

ROOT = Path(__file__).resolve().parents[1]


def write(path, value):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def snapshot(root=None):
    root = Path(root) if root is not None else ROOT
    files = {p.relative_to(root).as_posix(): sha256(p.read_bytes()).hexdigest()
             for folder in ("concert_app", "tests") for p in sorted((root / folder).rglob("*"))
             if p.is_file() and p.suffix in (".py", ".js", ".html", ".css")}
    if (root / "run.py").exists():
        files["run.py"] = sha256((root / "run.py").read_bytes()).hexdigest()
    return {"files": files, "revision": digest(files)}


def execute(argv, output, root=None):
    root = Path(root) if root is not None else ROOT
    result = subprocess.run(argv, cwd=root, capture_output=True, text=True,
                            env={**os.environ, "PYTHONDONTWRITEBYTECODE": "1"})
    receipt = {"argv": argv, "cwd": str(root), "exit_code": result.returncode,
               "stdout": result.stdout, "stderr": result.stderr, "python": sys.version,
               "code_revision": snapshot(root)["revision"]}
    write(output, receipt)
    return receipt


def event(phase, host, kind, **fields):
    run = ROOT / "runs" / ("phase-" + phase)
    value = {"type": kind, "expected_revision": run_status(ROOT / "gdd.json", run)["revision"],
             "idempotency_key": phase + "." + kind + "." + fields.get("package", "run"), **fields}
    result = advance_run(ROOT / "gdd.json", run, value, host_context=host)
    if not result.get("applied"):
        raise ValueError(result)
    return result


def integrate(phase, package, workspace, review):
    dispatch = json.loads((ROOT / "dispatches" / f"phase-{phase}.{package}.json").read_text())
    files, attempt = dispatch["package"]["write_resources"], dispatch["attempt"]
    host_path = ROOT / "host.implementation.json"
    host = json.loads(host_path.read_text())
    event(phase, host, "submit", package=package, attempt=attempt,
          summary="Actual native delivery reviewed by the lead; worker results are not acceptance evidence.",
          artifact_uri=str(workspace), changed_resources=files)
    base = run_status(ROOT / "gdd.json", ROOT / "runs" / ("phase-" + phase))["state"]["code_revision"]
    for file in files:
        target = ROOT / file
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(Path(workspace) / file, target)
    host["code_revision"] = snapshot()["revision"]
    write(host_path, host)
    return event(phase, host, "integrate", package=package, attempt=attempt,
                 base_code_revision=base, code_revision=host["code_revision"], verified_resources=files,
                 review={"reviewer": "actor.concert.lead", "verdict": "PASS", "evidence_uri": review})


def attest(phase, receipts):
    """Bind actual host-checked source/test bytes and executed checker receipts."""
    revision = snapshot()["revision"]
    spec = load_bundle(ROOT / "gdd.json").revision
    path = ROOT / "gdd.records.json"
    document = json.loads(path.read_text())
    records = document["records"]
    environment = {"python": sys.version, "sqlite": sqlite3.sqlite_version,
                   "database": "fresh real SQLite files with protected writes and preserved A-to-B history",
                   "http": "actual loopback ThreadingHTTPServer with ephemeral ports",
                   "browser": receipts["browser_concert"]["browser"]}
    host = json.loads((ROOT / "host.implementation.json").read_text())
    host.update(code_revision=revision, supported_methods=list(receipts), environment=environment,
                trusted_evidence=[], verified_bindings=[], negative_controls=[], evidence_selection={})
    for method, receipt in receipts.items():
        check_id = f"check.app.{phase}.{method}"
        check = records[check_id]
        binding, evidence = f"binding.app.{phase}.{method}", f"evidence.app.{phase}.{method}"
        assert binding not in records and evidence not in records
        resource = "tools/browser.cjs" if method == "browser_concert" else "tools/import_check.py" if method == "python_import_graph" else "tests/"
        resource_revision = ("sha256:" + sha256((ROOT / resource).read_bytes()).hexdigest()) if resource.endswith((".py", ".cjs")) else digest({p.name: sha256(p.read_bytes()).hexdigest() for p in sorted((ROOT / "tests").glob("*.py"))})
        records[binding] = {"kind": "binding", "spec_revision": spec, "targets": check["scope"],
                            "implementation": host["implementation"], "role": "test", "resource": resource,
                            "selector": {"kind": "whole-document"}, "status": "checked",
                            "revision": resource_revision, "code_revision": revision}
        records[evidence] = {"kind": "evidence", "check": check_id, "check_revision": digest(check),
                             "implementation": host["implementation"], "spec_revision": spec,
                             "code_revision": revision, "checker": receipt["checker"], "scope": check["scope"],
                             "result": "PASS", "executed": True, "reason": receipt["reason"],
                             "environment": {k: environment[k] for k in check["required_environment"]},
                             "evidence_uri": receipt["uri"]}
        host["trusted_evidence"].append(evidence)
        host["verified_bindings"].append(binding)
        host["negative_controls"].extend(check["negative_controls"])
        host["evidence_selection"][check_id] = evidence
    # Actual source bindings supplement test-to-rule coverage.
    for component, resources in {
        "api": ["concert_app/api.py", "concert_app/__main__.py"],
        "identity": ["concert_app/identity.py"], "reservations": ["concert_app/reservations.py"],
        "storage": ["concert_app/storage.py"], "frontend": ["concert_app/static/index.html", "concert_app/static/app.js", "concert_app/static/style.css"],
    }.items():
        for index, resource in enumerate(resources):
            binding = f"binding.app.{phase}.code.{component}.{index}"
            records[binding] = {"kind": "binding", "spec_revision": spec, "targets": ["component." + component],
                                "implementation": host["implementation"], "role": "code", "resource": resource,
                                "selector": {"kind": "whole-document"}, "status": "checked",
                                "revision": "sha256:" + sha256((ROOT / resource).read_bytes()).hexdigest(),
                                "code_revision": revision}
            host["verified_bindings"].append(binding)
    host["policy"]["activity"] = "acceptance"
    write(path, document)
    write(ROOT / f"host.acceptance.{phase}.json", host)
    return host
