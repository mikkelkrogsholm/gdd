"""A starter is a valid draft, published once without manufacturing authority."""

from hashlib import sha256
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tomllib

import pytest

from gdd_validator import scaffold, validate
from gdd_validator.atomic import rename_exclusive
from gdd_validator.loading import load_bundle, parse, schema_checks
from gdd_validator.report import aggregate
from gdd_validator.scaffold import init_project

ROOT = Path(__file__).resolve().parents[1]


def test_cli_initializes_a_draft_and_reports_readiness_separately(tmp_path):
    project = tmp_path / "project"
    command = [sys.executable, "-m", "gdd_validator"]
    result = subprocess.run(
        [*command, "init", str(project), "--project-id", "project.cli", "--title", "CLI project"],
        capture_output=True, text=True,
    )
    assert result.returncode == 0, result.stderr
    assert json.loads(result.stdout)["project_id"] == "project.cli"
    readiness = subprocess.run(
        [*command, "validate", str(project / "gdd.json"), "--profile", "readiness", "--scope", "action.main"],
        capture_output=True, text=True,
    )
    assert readiness.returncode == 1, readiness.stderr
    report = json.loads(readiness.stdout)
    assert aggregate(report["checks"], "readiness") == "FAIL"
    assert report["tool"]["version"] == tomllib.loads((ROOT / "pyproject.toml").read_text())["project"]["version"]
    before = {p.name: p.read_bytes() for p in project.iterdir()}
    repeated = subprocess.run([*command, "init", str(project)], capture_output=True, text=True)
    assert repeated.returncode == 1 and "already exists" in repeated.stderr
    assert {p.name: p.read_bytes() for p in project.iterdir()} == before


def test_cli_rejects_invalid_identity_without_creating_output(tmp_path):
    project = tmp_path / "project"
    result = subprocess.run(
        [sys.executable, "-m", "gdd_validator", "init", str(project), "--project-id", "invalid id"],
        capture_output=True, text=True,
    )
    assert result.returncode == 1 and not project.exists()


def test_starter_is_valid_but_unready_and_contains_no_authority(tmp_path):
    receipt = init_project(tmp_path / "gdd")
    manifest = Path(receipt["manifest"])
    bundle = load_bundle(manifest)
    assert receipt["format"] == "gdd-init/0.1"
    assert Path(receipt["path"]) == manifest.parent
    assert set(receipt["files"]) == {"gdd.json", "SOURCES.md", "records.json"}
    assert set(p.name for p in manifest.parent.iterdir()) == set(receipt["files"])
    assert receipt["spec_revision"] == bundle.revision
    assert receipt["model"] == "PASS"
    assert receipt["readiness"] == "FAIL"
    assert bundle.model["format"] == "gdd/0.3"
    assert bundle.model["project"]["id"] == "project.new"
    assert "workflow" not in bundle.model["project"]
    assert bundle.model["sources"] == {}
    assert bundle.records == {}
    assert set(bundle.model["nodes"]) == {"component.main", "action.main"}
    assert {n["maturity"] for n in bundle.model["nodes"].values()} == {"sketch"}
    for name in ("gdd.json", "records.json"):
        assert aggregate(schema_checks(parse((manifest.parent / name).read_bytes()), name), "model") == "PASS"
    report = validate(manifest, profiles=["model", "readiness", "records"])
    assert aggregate(report["checks"], "model") == "PASS"
    assert aggregate(report["checks"], "readiness") == "FAIL"
    assert aggregate(report["checks"], "implementation") == "UNKNOWN"
    for scope in ("component.main", "action.main"):
        report = validate(manifest, profiles=["readiness"], scope=[scope])
        assert aggregate(report["checks"], "readiness") == "FAIL"
        codes = {d["code"] for c in report["checks"] for d in c["diagnostics"]}
        assert "QUESTION_BLOCKS_SCOPE" in codes
    authorization = validate(
        manifest, profiles=["records"], scope=["action.main"], activity="implementation"
    )
    assert aggregate(authorization["checks"], "authorization") == "UNRESOLVED"
    assert not list(tmp_path.glob(".gdd-init-*"))


def test_custom_identity_and_title_round_trip_without_template_interpolation(tmp_path):
    title = 'Draft "quote": braces {project_id}, Unicode Ω and a newline\nSecond line'
    receipt = init_project(tmp_path / "gdd", project_id="project.my_app-2", title=title)
    model = parse(Path(receipt["manifest"]).read_bytes())
    records = parse((Path(receipt["path"]) / "records.json").read_bytes())
    assert model["project"]["id"] == "project.my_app-2"
    assert model["project"]["title"] == title
    assert records["project_id"] == "project.my_app-2"
    assert aggregate(validate(receipt["manifest"])["checks"], "model") == "PASS"


@pytest.mark.parametrize("existing", ["file", "directory", "symlink", "broken_symlink"])
def test_existing_output_is_preserved(tmp_path, existing):
    output = tmp_path / "gdd"
    target = tmp_path / "target"
    if existing == "file":
        output.write_text("original file")
    elif existing == "directory":
        output.mkdir()
        (output / "keep").write_text("original directory")
    else:
        if existing == "symlink":
            target.mkdir()
            (target / "keep").write_text("original symlink target")
        output.symlink_to(target, target_is_directory=True)
    before = output.lstat().st_ino
    paths_before = sorted(p.name for p in tmp_path.iterdir())
    with pytest.raises(FileExistsError, match="already exists"):
        init_project(output)
    assert output.lstat().st_ino == before
    assert sorted(p.name for p in tmp_path.iterdir()) == paths_before
    if existing == "file":
        assert output.read_text() == "original file"
    elif existing == "directory":
        assert (output / "keep").read_text() == "original directory"
    elif existing == "symlink":
        assert (target / "keep").read_text() == "original symlink target"
    else:
        assert not target.exists()


def test_repeated_initialization_preserves_every_original_byte(tmp_path):
    output = tmp_path / "gdd"
    init_project(output)
    before = {p.name: sha256(p.read_bytes()).hexdigest() for p in output.iterdir()}
    with pytest.raises(FileExistsError):
        init_project(output, project_id="project.different", title="Different title")
    after = {p.name: sha256(p.read_bytes()).hexdigest() for p in output.iterdir()}
    assert after == before
    assert not list(tmp_path.glob(".gdd-init-*"))


@pytest.mark.parametrize(
    "arguments",
    [
        {"project_id": ""},
        {"project_id": "Project.invalid"},
        {"project_id": "project.invalid\n"},
        {"project_id": "project.invalid space"},
        {"project_id": "project."},
        {"project_id": 4},
        {"project_id": None},
        {"title": ""},
        {"title": " \n\t"},
        {"title": None},
        {"title": 4},
        {"title": "unpaired \ud800"},
    ],
)
def test_invalid_arguments_are_rejected_before_filesystem_effects(tmp_path, monkeypatch, arguments):
    def no_staging(*args, **kwargs):
        pytest.fail("Invalid arguments reached filesystem staging.")

    monkeypatch.setattr(scaffold.tempfile, "TemporaryDirectory", no_staging)
    with pytest.raises(ValueError):
        init_project(tmp_path / "missing-parent" / "gdd", **arguments)
    assert list(tmp_path.iterdir()) == []


def test_nul_path_is_rejected_before_filesystem_effects(tmp_path, monkeypatch):
    def no_staging(*args, **kwargs):
        pytest.fail("Invalid path reached filesystem staging.")

    monkeypatch.setattr(scaffold.tempfile, "TemporaryDirectory", no_staging)
    with pytest.raises(ValueError, match="NUL"):
        init_project(Path(str(tmp_path / "gdd") + "\x00ignored"))
    assert list(tmp_path.iterdir()) == []


def test_missing_parent_is_not_created(tmp_path):
    with pytest.raises(FileNotFoundError):
        init_project(tmp_path / "missing-parent" / "gdd")
    assert list(tmp_path.iterdir()) == []


def test_write_failure_cleans_the_partial_staging_bundle(tmp_path, monkeypatch):
    actual_open = Path.open

    def fail_second_write(path, mode="r", *args, **kwargs):
        if mode == "xb" and path.name == "gdd.json":
            raise OSError("Injected write failure")
        return actual_open(path, mode, *args, **kwargs)

    monkeypatch.setattr(Path, "open", fail_second_write)
    with pytest.raises(OSError, match="Injected write failure"):
        init_project(tmp_path / "gdd")
    assert list(tmp_path.iterdir()) == []


@pytest.mark.parametrize("model_result,readiness_result", [("FAIL", "FAIL"), ("PASS", "PASS")])
def test_validation_failure_prevents_publication(tmp_path, monkeypatch, model_result, readiness_result):
    def invalid_report(*args, **kwargs):
        return {
            "checks": [
                {"dimension": "model", "result": model_result},
                {"dimension": "readiness", "result": readiness_result},
            ]
        }

    monkeypatch.setattr(scaffold, "validate", invalid_report)
    with pytest.raises(ValueError, match="model-valid and structurally unready"):
        init_project(tmp_path / "gdd")
    assert list(tmp_path.iterdir()) == []


@pytest.mark.parametrize("existing", ["directory", "symlink"])
def test_competing_destination_at_publication_is_preserved(tmp_path, monkeypatch, existing):
    output = tmp_path / "gdd"
    target = tmp_path / "missing-target"
    inodes = []

    def competing_destination(staging, destination):
        # Publication must see every file and its actual valid model together.
        assert set(p.name for p in staging.iterdir()) == set(scaffold.TEMPLATE_FILES)
        assert aggregate(validate(staging / "gdd.json")["checks"], "model") == "PASS"
        if existing == "directory":
            destination.mkdir()
            (destination / "keep").write_text("competing work")
        else:
            destination.symlink_to(target, target_is_directory=True)
        inodes.append(destination.lstat().st_ino)
        rename_exclusive(staging, destination)

    monkeypatch.setattr(scaffold, "rename_exclusive", competing_destination)
    with pytest.raises(FileExistsError):
        init_project(output)
    assert output.lstat().st_ino == inodes[0]
    if existing == "directory":
        assert (output / "keep").read_text() == "competing work"
    else:
        assert output.is_symlink()
        assert not target.exists()
    assert not list(tmp_path.glob(".gdd-init-*"))


@pytest.mark.parametrize("packaged", [False, True])
def test_template_resources_work_in_source_and_installed_layouts(tmp_path, monkeypatch, packaged):
    checkout = tmp_path / "installation"
    package = checkout / "gdd_validator"
    package.mkdir(parents=True)
    directory = (package if packaged else checkout) / "templates" / "project"
    shutil.copytree(ROOT / "templates" / "project", directory)
    # Simulate the module location without depending on the current checkout's
    # resource tree. The installed wheel puts templates inside the package.
    monkeypatch.setattr(scaffold, "__file__", str(package / "scaffold.py"))
    assert scaffold._template_directory() == directory
    receipt = init_project(tmp_path / "gdd")
    assert aggregate(validate(receipt["manifest"])["checks"], "model") == "PASS"


def test_parent_symlink_is_resolved_without_following_an_output_symlink(tmp_path):
    actual = tmp_path / "actual"
    actual.mkdir()
    alias = tmp_path / "alias"
    alias.symlink_to(actual, target_is_directory=True)
    receipt = init_project(alias / "gdd")
    assert Path(receipt["path"]) == actual.resolve() / "gdd"
    assert (actual / "gdd" / "gdd.json").is_file()


@pytest.mark.parametrize("bad_template", ["schema", "records", "authority", "missing_source"])
def test_invalid_or_authoritative_templates_do_not_create_output(tmp_path, monkeypatch, bad_template):
    directory = tmp_path / "templates"
    shutil.copytree(ROOT / "templates" / "project", directory)
    model = json.loads((directory / "gdd.json").read_text())
    if bad_template == "schema":
        model["nodes"]["action.main"]["kind"] = "invalid"
    elif bad_template == "authority":
        model["project"]["workflow"] = {
            "mode": "manual", "mandate": "Invented authority",
            "gates": {activity: {"requirement": "none", "authorities": [], "scope": []}
                      for activity in ("model_changes", "implementation", "acceptance")},
        }
    elif bad_template == "missing_source":
        model["sources"] = {
            "source.missing": {
                "role": "input", "title": "Unavailable contract source",
                "uri": "missing.md", "format": "text/markdown", "normative": True,
                "revision": {"sha256": "0" * 64},
            }
        }
    else:
        records = json.loads((directory / "records.json").read_text())
        records["records"] = {"evidence.fake": {"kind": "evidence"}}
        (directory / "records.json").write_text(json.dumps(records))
    (directory / "gdd.json").write_text(json.dumps(model))
    monkeypatch.setattr(scaffold, "_template_directory", lambda: directory)
    with pytest.raises(ValueError):
        init_project(tmp_path / "gdd")
    assert not (tmp_path / "gdd").exists()
    assert not list(tmp_path.glob(".gdd-init-*"))
