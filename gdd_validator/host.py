"""Opt-in host utilities for actual checks and immutable evidence authoring.

The caller selects executable argv, files, coverage and trust. Validation never
calls these helpers or interprets a model's method labels as executable commands.
"""

from datetime import datetime, timezone
from hashlib import sha256
import json
import os
from pathlib import Path, PurePosixPath
import platform
import subprocess
import sys
import tempfile

from jsonschema import Draft202012Validator

from .loading import canonical, digest, load_bundle, parse, schema
from .model import validate_model
from .record_integrity import invalid_record_ids, model_ids
from .report import aggregate


def _resource(value):
    if not isinstance(value, str) or not value or "\\" in value or "\0" in value:
        raise ValueError("Resources must be nonempty relative POSIX paths.")
    path = PurePosixPath(value)
    if path.is_absolute() or any(part in {"", ".", ".."} for part in value.split("/")):
        raise ValueError("Resources must be normalized relative POSIX paths.")
    return path.as_posix()


def snapshot_code(root, files):
    """Hash explicit regular files; no globs, generated evidence or graph inference."""
    root = Path(root).resolve(strict=True)
    if not root.is_dir() or not isinstance(files, (list, tuple)):
        raise ValueError("Select an existing root and an explicit file list.")
    names = [_resource(value) for value in files]
    if len(set(names)) != len(names):
        raise ValueError("A code snapshot cannot repeat a resource.")
    hashes = {}
    for name in sorted(names):
        current = root
        for part in PurePosixPath(name).parts:
            current = current / part
            if current.is_symlink():
                raise ValueError("Code snapshot resources cannot follow symlinks.")
        if not current.is_file():
            raise ValueError("Code snapshot resource is not a regular file: " + name)
        hashes[name] = sha256(current.read_bytes()).hexdigest()
    return {"format": "gdd-code-snapshot/0.1", "files": hashes, "revision": digest(hashes)}


def execute_check(root, command, files, *, output, timeout=120):
    """Run caller-selected argv and preserve a fresh actual receipt.

    Exit0 is PASS, exit1 is FAIL; other exits/timeouts/unavailable execution are
    UNKNOWN. Source drift also prevents PASS. Choose a checker with this exit
    contract or adapt its actual receipt explicitly outside this helper.
    """
    root = Path(root).resolve(strict=True)
    if (not isinstance(command, (list, tuple)) or not command
            or any(not isinstance(value, str) or not value or "\0" in value for value in command)):
        raise ValueError("The trusted host must select a nonempty argv list.")
    if isinstance(timeout, bool) or not isinstance(timeout, (int, float)) or not 0 < timeout <= 86400:
        raise ValueError("Choose a positive finite timeout of at most one day.")
    canonical(list(command))
    before = snapshot_code(root, files)
    output = Path(output)
    receipt = {"format": "gdd-host-check/0.1", "command": list(command), "cwd": str(root),
               "started_at": datetime.now(timezone.utc).isoformat(), "executed": False,
               "exit_code": None, "result": "UNKNOWN", "stdout": "", "stderr": "",
               "snapshot": before, "code_revision": before["revision"],
               "runner": {"python": sys.version, "platform": platform.platform()},
               "reason": "Execution has not been established."}
    # Reserve the receipt before execution; retry cannot silently repeat effects.
    with output.open("x", encoding="utf-8") as file:
        try:
            completed = subprocess.run(list(command), cwd=root, capture_output=True,
                                       text=True, encoding="utf-8", errors="replace", timeout=timeout)
            receipt.update(executed=True, exit_code=completed.returncode,
                           stdout=completed.stdout, stderr=completed.stderr,
                           result={0: "PASS", 1: "FAIL"}.get(completed.returncode, "UNKNOWN"),
                           reason="Actual caller-selected checker process completed.")
        except subprocess.TimeoutExpired as error:
            def text(value):
                return value.decode("utf-8", "replace") if isinstance(value, bytes) else value or ""
            receipt.update(executed=True, stdout=text(error.stdout), stderr=text(error.stderr),
                           reason="The checker timed out; no completed result is established.")
        except OSError as error:
            receipt.update(reason="The checker could not be started: " + str(error))
        try:
            after = snapshot_code(root, files)
            receipt["code_revision_after"] = after["revision"]
            if after != before:
                receipt.update(result="UNKNOWN", reason="Selected source bytes changed during execution.")
        except (OSError, ValueError) as error:
            receipt.update(result="UNKNOWN", code_revision_after=None,
                           reason="Selected source bytes became unresolved: " + str(error))
        receipt["finished_at"] = datetime.now(timezone.utc).isoformat()
        file.write(json.dumps(receipt, ensure_ascii=False, indent=2, allow_nan=False) + "\n")
    return receipt


def verification_records(model, check_id, receipt_path, *, root, implementation,
                         binding_id, evidence_id, test_resource, checker, environment,
                         verified_negative_controls=()):
    """Build current records after the host explicitly attests coverage and controls.

    Reading a receipt alone proves no authenticity. The caller must own/verify its
    execution and test coverage. The returned host fragment grants no authority.
    """
    bundle = load_bundle(Path(model))
    validate_model(bundle)
    if bundle.revision is None or aggregate(bundle.checks, "model") != "PASS":
        raise ValueError("Evidence authoring requires a resolved valid model.")
    check = bundle.records.get(check_id)
    if (not check or check["kind"] != "check" or check["spec_revision"] != bundle.revision
            or check_id in invalid_record_ids(bundle)):
        raise ValueError("Select a valid current immutable check plan.")
    if (binding_id == evidence_id
            or {binding_id, evidence_id} & (bundle.records.keys() | model_ids(bundle.model))):
        raise ValueError("Use new distinct record IDs; historical IDs are immutable.")
    receipt_path = Path(receipt_path)
    receipt = parse(receipt_path.read_bytes())
    if (not isinstance(receipt, dict) or receipt.get("format") != "gdd-host-check/0.1"
            or receipt.get("executed") is not True or receipt.get("result") not in {"PASS", "FAIL"}
            or receipt.get("exit_code") != {"PASS": 0, "FAIL": 1}[receipt["result"]]):
        raise ValueError("A completed actual PASS/FAIL host check is required.")
    current = snapshot_code(root, list(receipt["snapshot"]["files"]))
    if (receipt["snapshot"] != current or receipt["code_revision"] != current["revision"]
            or receipt.get("code_revision_after") != current["revision"]):
        raise ValueError("The selected code snapshot is stale or unresolved.")
    if (not isinstance(environment, dict) or any(not isinstance(k, str) or not isinstance(v, str)
                                               for k, v in environment.items())
            or not set(check["required_environment"]) <= environment.keys()):
        raise ValueError("Supply the actual declared checker environment.")
    if (not isinstance(verified_negative_controls, (list, tuple))
            or any(not isinstance(value, str) or not value for value in verified_negative_controls)):
        raise ValueError("Attested negative controls must be explicit names.")
    if not set(check["negative_controls"]) <= set(verified_negative_controls):
        raise ValueError("The host must attest every actually performed negative control.")
    resource = test_resource.rstrip("/") if isinstance(test_resource, str) else test_resource
    resource = _resource(resource)
    selected = {name: value for name, value in current["files"].items()
                if name == resource or name.startswith(resource + "/")}
    if not selected:
        raise ValueError("The test resource must be included in the checked code snapshot.")
    resource_revision = "sha256:" + selected[resource] if resource in selected else digest(selected)
    binding = {"kind": "binding", "spec_revision": bundle.revision, "targets": check["scope"],
               "implementation": implementation, "role": "test", "resource": test_resource,
               "selector": {"kind": "whole-document"}, "status": "checked",
               "revision": resource_revision, "code_revision": current["revision"]}
    evidence = {"kind": "evidence", "check": check_id, "check_revision": digest(check),
                "implementation": implementation, "spec_revision": bundle.revision,
                "code_revision": current["revision"], "checker": checker, "scope": check["scope"],
                "result": receipt["result"], "executed": True,
                "reason": "Actual host-selected execution; coverage and controls explicitly attested by its caller.",
                "environment": {name: environment[name] for name in check["required_environment"]},
                "evidence_uri": str(receipt_path)}
    records = {binding_id: binding, evidence_id: evidence}
    Draft202012Validator(schema("records")).validate(
        {"format": "gdd-records/0.2", "project_id": bundle.model["project"]["id"], "records": records})
    host = {"implementation": implementation, "code_revision": current["revision"],
            "verified_bindings": [binding_id], "trusted_evidence": [evidence_id],
            "supported_methods": [check["method"]], "environment": dict(environment),
            "negative_controls": list(verified_negative_controls), "evidence_selection": {check_id: evidence_id}}
    return {"records": records, "host": host}


def append_records(path, fragment):
    """Append schema-valid new IDs under a single-writer lock; never replace an ID."""
    path = Path(path)
    if path.is_symlink() or not path.is_file():
        raise ValueError("Select an existing regular records document.")
    additions = fragment.get("records") if isinstance(fragment, dict) else None
    if not isinstance(additions, dict) or not additions:
        raise ValueError("Supply a nonempty records fragment.")
    lock = path.with_name(path.name + ".lock")
    descriptor = os.open(lock, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    temporary = None
    try:
        os.close(descriptor)
        document = parse(path.read_bytes())
        Draft202012Validator(schema("records")).validate(document)
        if document["records"].keys() & additions.keys():
            raise ValueError("Existing record IDs are immutable; choose new IDs.")
        document["records"].update(additions)
        Draft202012Validator(schema("records")).validate(document)
        with tempfile.NamedTemporaryFile(mode="w", encoding="utf-8", dir=path.parent,
                                         prefix="." + path.name + ".", delete=False) as file:
            temporary = Path(file.name)
            file.write(json.dumps(document, ensure_ascii=False, indent=2, allow_nan=False) + "\n")
        os.chmod(temporary, path.stat().st_mode & 0o777)
        os.replace(temporary, path)
        temporary = None
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)
        lock.unlink()
