"""Portability and maintained-source integrity of the installable GDD skill."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
import re
import runpy
import shutil
import subprocess
import sys
from urllib.parse import unquote, urlsplit

import pytest


ROOT = Path(__file__).resolve().parents[1]
SKILL = ROOT / "skills/gdd"
SYNC = runpy.run_path(str(ROOT / "scripts/sync_skill.py"))
VERIFY = runpy.run_path(str(SKILL / "scripts/verify_assets.py"))["verify"]


def index(skill: Path = SKILL) -> dict:
    return json.loads((skill / "references/source-index.json").read_text(encoding="utf-8"))


@pytest.fixture
def source_copy(tmp_path: Path) -> Path:
    """Copy source inputs and citation targets, without a checkout or agent runtime."""
    root = tmp_path / "source"
    manifest = index()
    paths = {entry["source"] for entry in manifest["files"]}
    paths.update(link["source_target"] for link in manifest["repository_only_links"])
    for relative in paths:
        destination = root / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT / relative, destination)
    shutil.copytree(SKILL, root / "skills/gdd")
    return root


def test_generated_references_match_authoritative_bytes_and_index():
    assert SYNC["sync"](ROOT, check=True) == []
    manifest = index()
    assert manifest["origin"]["repository"]
    assert manifest["origin"]["git_base_revision"]
    assert {entry["source"] for entry in manifest["files"]} == set(SYNC["source_paths"](ROOT))
    for entry in manifest["files"]:
        source = (ROOT / entry["source"]).read_bytes()
        assert (SKILL / entry["path"]).read_bytes() == source
        assert entry["bytes"] == len(source)
        assert entry["sha256"] == hashlib.sha256(source).hexdigest()


def test_installed_folder_has_all_required_navigation(tmp_path):
    installed = tmp_path / "installed/gdd"
    shutil.copytree(SKILL, installed)
    manifest = index(installed)
    optional = {(entry["from"], entry["href"]) for entry in manifest["repository_only_links"]}
    cited = set()
    for document in installed.rglob("*.md"):
        for href in re.findall(r"\]\(([^)]+)\)", document.read_text(encoding="utf-8")):
            href = href.strip().strip("<>")
            url = urlsplit(href)
            if url.scheme or not url.path:
                continue
            target = (document.parent / unquote(url.path)).resolve()
            assert target.is_relative_to(installed), (document, href)
            if target.is_file():
                continue
            # Only exact source copies may cite explicitly indexed optional repository history.
            relative = document.relative_to(installed / "references/gdd").as_posix()
            assert (relative, href) in optional, (document, href)
            cited.add((relative, href))
    assert cited == optional


def test_standalone_integrity_script_needs_no_checkout_or_package(tmp_path):
    installed = tmp_path / "skills/gdd"
    shutil.copytree(SKILL, installed)
    result = subprocess.run([sys.executable, "-I", str(installed / "scripts/verify_assets.py")],
                            cwd=tmp_path, capture_output=True, text=True, check=False)
    assert result.returncode == 0, result.stderr
    assert VERIFY(installed) == []


def test_standalone_detects_changed_missing_and_unindexed_assets(tmp_path):
    installed = tmp_path / "gdd"
    shutil.copytree(SKILL, installed)
    entries = index(installed)["files"]
    modified = installed / entries[0]["path"]
    modified.write_bytes(modified.read_bytes() + b"\nChanged copy.\n")
    (installed / entries[1]["path"]).unlink()
    (installed / "references/gdd/unindexed.md").write_text("Unexpected generated file.\n")
    failures = VERIFY(installed)
    assert f"Reference hash mismatch: {entries[0]['path']}" in failures
    assert f"Missing reference: {entries[1]['path']}" in failures
    assert "Unindexed reference: references/gdd/unindexed.md" in failures


def test_check_is_read_only_and_sync_repairs_source_drift(source_copy):
    before = (source_copy / "skills/gdd/references/gdd/GDD.md").read_bytes()
    source = source_copy / "GDD.md"
    source.write_bytes(source.read_bytes() + b"\nNew maintained source guidance.\n")
    assert SYNC["sync"](source_copy, check=True)
    assert (source_copy / "skills/gdd/references/gdd/GDD.md").read_bytes() == before
    SYNC["sync"](source_copy)
    assert SYNC["sync"](source_copy, check=True) == []
    assert (source_copy / "skills/gdd/references/gdd/GDD.md").read_bytes() == source.read_bytes()
    assert VERIFY(source_copy / "skills/gdd") == []


def test_required_host_is_synchronized_and_missing_source_is_rejected(source_copy):
    original_origin = index(source_copy / "skills/gdd")["origin"]
    host = source_copy / "HOST.md"
    host.write_text("# Host guide\n\nRead [workflow](WORKFLOW.md).\n", encoding="utf-8")
    assert SYNC["sync"](source_copy, check=True)
    SYNC["sync"](source_copy)
    skill = source_copy / "skills/gdd"
    assert (skill / "references/gdd/HOST.md").read_bytes() == host.read_bytes()
    assert "[HOST.md](gdd/HOST.md)" in (skill / "references/catalog.md").read_text()
    assert index(skill)["origin"] == original_origin
    host.unlink()
    # Current 0.7 routes link directly to this required guide. Dropping it must
    # fail before publishing an incomplete standalone skill.
    with pytest.raises((FileNotFoundError, ValueError)):
        SYNC["sync"](source_copy, check=True)
    assert (skill / "references/gdd/HOST.md").is_file()


def test_all_bundled_schema_refs_are_locally_resolvable():
    documents = [json.loads(path.read_text())
                 for path in (SKILL / "references/gdd/schemas").glob("*.json")]
    schemas = {document["$id"]: document for document in documents if "$id" in document}

    def check_refs(value, document):
        if isinstance(value, dict):
            if "$ref" in value:
                uri, _, pointer = value["$ref"].partition("#")
                target = schemas[uri] if uri else document
                for part in pointer.lstrip("/").split("/") if pointer else []:
                    target = target[unquote(part).replace("~1", "/").replace("~0", "~")]
            for child in value.values():
                check_refs(child, document)
        elif isinstance(value, list):
            for child in value:
                check_refs(child, document)

    for schema in documents:
        check_refs(schema, schema)


def test_standalone_rejects_index_paths_outside_generated_references(tmp_path):
    installed = tmp_path / "gdd"
    shutil.copytree(SKILL, installed)
    manifest = index(installed)
    manifest["files"][0]["path"] = "../../outside.md"
    (installed / "references/source-index.json").write_text(json.dumps(manifest))
    with pytest.raises(ValueError, match="escapes its directory"):
        VERIFY(installed)
