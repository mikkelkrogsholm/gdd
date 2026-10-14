"""Regressions discovered by the bounded adversarial review loops."""

from pathlib import Path
import json
import subprocess
import sys

import pytest
from jsonschema import Draft202012Validator

from gdd_validator.atomic import rename_exclusive
from gdd_validator.validator import input_schema

ROOT = Path(__file__).resolve().parents[1]


def test_context_deduplicates_repeated_scope():
    result = subprocess.run(
        [
            sys.executable,
            "-m",
            "gdd_validator",
            "context",
            str(ROOT / "examples/concert.gdd.yaml"),
            "--scope",
            "action.reserve",
            "--scope",
            "action.reserve",
        ],
        capture_output=True,
        text=True,
    )
    assert result.returncode == 0, result.stderr
    data = json.loads(result.stdout)
    Draft202012Validator(input_schema("package")).validate(data)
    assert data["scope"] == ["action.reserve"]


@pytest.mark.parametrize("existing", ["file", "directory", "symlink"])
def test_atomic_publish_never_replaces_existing_destination(tmp_path, existing):
    source = tmp_path / "staging"
    source.mkdir()
    (source / "keep").write_text("source")
    destination = tmp_path / "destination"
    if existing == "file":
        destination.write_text("destination")
    elif existing == "directory":
        destination.mkdir()
    else:
        destination.symlink_to(tmp_path / "missing")
    before = destination.lstat().st_ino
    with pytest.raises(FileExistsError):
        rename_exclusive(source, destination)
    assert destination.lstat().st_ino == before
    assert (source / "keep").read_text() == "source"


def test_atomic_publish_moves_completed_bundle(tmp_path):
    source = tmp_path / "staging"
    source.mkdir()
    (source / "asset").write_text("complete")
    destination = tmp_path / "destination"
    rename_exclusive(source, destination)
    assert not source.exists()
    assert (destination / "asset").read_text() == "complete"


def test_import_refuses_destination_created_at_publication(tmp_path, monkeypatch):
    from gdd_validator import graphify_io
    from gdd_validator.graphify_io import import_graphify

    actual = rename_exclusive
    inode = []

    def competing_destination(source, target):
        target.mkdir()
        inode.append(target.stat().st_ino)
        actual(source, target)

    monkeypatch.setattr(graphify_io, "rename_exclusive", competing_destination)
    target = tmp_path / "bundle"
    with pytest.raises(FileExistsError):
        import_graphify(
            ROOT / "examples/graphify/raw.ast.json",
            target,
            profile="graphify-0.9.75-ast",
        )
    assert target.stat().st_ino == inode[0]
    assert list(target.iterdir()) == []
    assert not list(tmp_path.glob(".gdd-graphify-*"))
