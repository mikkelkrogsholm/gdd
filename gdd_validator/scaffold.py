"""Create an exclusive, validated GDD draft bundle from portable templates."""

import json
from pathlib import Path
import tempfile

from .atomic import rename_exclusive
from .loading import parse, schema_checks
from .report import aggregate
from .validator import validate

__all__ = ["init_project"]

TEMPLATE_FILES = ("SOURCES.md", "gdd.json", "records.json")


def _template_directory() -> Path:
    directory = Path(__file__).parent / "templates" / "project"
    if not directory.is_dir():
        directory = Path(__file__).parent.parent / "templates" / "project"
    return directory


def _encoded(value: dict) -> bytes:
    return (json.dumps(value, ensure_ascii=False, indent=2) + "\n").encode("utf-8")


def init_project(
    path: Path,
    *,
    project_id: str = "project.new",
    title: str = "New GDD project",
) -> dict:
    """Publish a new bundle directory; its parent must already exist.

    Model validity describes the draft's structure. The starter deliberately
    has incomplete contracts and an open implementation question. It supplies
    no mandate, accepted decision or execution evidence.
    """
    if not isinstance(project_id, str):
        raise ValueError("Project ID must be a GDD ID string.")
    if not isinstance(title, str) or not title.strip():
        raise ValueError("Project title must be a nonempty string.")
    try:
        title.encode("utf-8")
    except UnicodeError as error:
        raise ValueError("Project title must contain valid Unicode scalars.") from error
    path = Path(path)
    if "\x00" in str(path):
        raise ValueError("Embedded NUL in project path.")

    directory = _template_directory()
    contents = {name: (directory / name).read_bytes() for name in TEMPLATE_FILES}
    model = parse(contents["gdd.json"])
    records = parse(contents["records.json"])
    model["project"].update(id=project_id, title=title)
    records["project_id"] = project_id
    for name, document in (("gdd.json", model), ("records.json", records)):
        checks = schema_checks(document, name)
        if aggregate(checks, "model") != "PASS":
            messages = [d["message"] for c in checks for d in c["diagnostics"]]
            raise ValueError("Invalid project draft: " + "; ".join(messages))
    if model["project"].get("workflow") is not None or records["records"]:
        raise ValueError("Starter templates must not supply authority or records.")
    contents["gdd.json"] = _encoded(model)
    contents["records.json"] = _encoded(records)

    # Resolve the parent only: resolving the final name could follow a symlink
    # and allow publication at an unintended target.
    output = path.absolute()
    if output.exists() or output.is_symlink():
        raise FileExistsError(f"Project bundle already exists: {output}")
    output = output.parent.resolve(strict=True) / output.name
    with tempfile.TemporaryDirectory(prefix=".gdd-init-", dir=output.parent) as temporary:
        staging = Path(temporary)
        for name, raw in contents.items():
            with (staging / name).open("xb") as target:
                target.write(raw)
        report = validate(staging / "gdd.json", profiles=["model", "readiness"])
        model_result = aggregate(report["checks"], "model")
        readiness_result = aggregate(report["checks"], "readiness")
        if (
            model_result != "PASS"
            or readiness_result != "FAIL"
            or report.get("spec_revision") is None
        ):
            raise ValueError("Starter template must be model-valid and structurally unready.")
        rename_exclusive(staging, output)
    return {
        "format": "gdd-init/0.1",
        "path": str(output),
        "manifest": str(output / "gdd.json"),
        "project_id": project_id,
        "files": list(TEMPLATE_FILES),
        "spec_revision": report["spec_revision"],
        "model": model_result,
        "readiness": readiness_result,
    }
