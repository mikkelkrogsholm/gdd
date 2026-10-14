"""Filesystem regressions from the second adversarial review."""

import json
from pathlib import Path
import subprocess
import sys

import pytest

from gdd_validator.atomic import rename_exclusive
from gdd_validator.graphify_io import import_graphify
from test_review_graphify import make_bundle

ROOT = Path(__file__).resolve().parents[1]


@pytest.mark.parametrize("argument", ["source", "destination"])
def test_atomic_rename_rejects_nul_without_touching_paths(tmp_path, argument):
    source = tmp_path / "source"
    source.mkdir()
    (source / "keep").write_text("intact")
    destination = tmp_path / "destination"
    arguments = {"source": source, "destination": destination}
    arguments[argument] = Path(str(arguments[argument]) + "\x00ignored")
    with pytest.raises(ValueError, match="null|NUL"):
        rename_exclusive(**arguments)
    assert (source / "keep").read_text() == "intact"
    assert not destination.exists()


def test_import_rejects_nul_output_without_publishing(tmp_path):
    destination = tmp_path / "output"
    with pytest.raises(ValueError, match="null|NUL"):
        import_graphify(
            ROOT / "examples/graphify/raw.ast.json",
            Path(str(destination) + "\x00ignored"),
            profile="graphify-0.9.75-ast",
        )
    assert not destination.exists()
    assert not list(tmp_path.glob(".gdd-graphify-*"))


@pytest.mark.parametrize("output_alias", [False, True])
def test_cli_proposal_source_locator_follows_manifest_and_output_aliases(
    tmp_path, output_alias
):
    folder, _, _, _, mapping = make_bundle(tmp_path)
    aliases = tmp_path / "aliases"
    aliases.mkdir()
    manifest = aliases / "manifest.json"
    manifest.symlink_to(folder / "observations.json")
    actual_output = tmp_path / "actual-output"
    actual_output.mkdir()
    if output_alias:
        output_parent = aliases / "output"
        output_parent.symlink_to(actual_output, target_is_directory=True)
    else:
        output_parent = actual_output
    mapping_path = tmp_path / "mapping.json"
    mapping_path.write_text(json.dumps(mapping))
    output = output_parent / "proposal.json"
    run = subprocess.run(
        [
            sys.executable,
            "-m",
            "gdd_validator",
            "graphify",
            "propose",
            str(manifest),
            "--mapping",
            str(mapping_path),
            "--output",
            str(output),
        ],
        capture_output=True,
        text=True,
    )
    assert run.returncode == 0, run.stderr
    proposal = json.loads(output.read_text())
    uri = next(iter(proposal["model_candidate"]["sources"].values()))["uri"]
    assert (output.parent / uri).resolve() == (folder / "graphify.raw.json").resolve()
    assert (output.parent / uri).is_file()
