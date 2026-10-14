"""Small JSON-first command line interface."""

import argparse
import json
import os
from pathlib import Path
import sys

from .loading import InputError, load_bundle, parse
from .graphify_io import import_graphify, verify_graphify, export_graphify
from .graphify_proposal import propose_graphify
from .report import exit_code
from .validator import PROFILES, validate
from .visualization import render_view
from .workflow import prepare_workflow, verify_workflow, context_manifest
from .workflow_run import create_run, run_status, advance_run

__all__ = ["main"]


def main() -> int:
    parser = argparse.ArgumentParser(prog="gdd", description="GDD 0.2 model tools")
    commands = parser.add_subparsers(dest="command", required=True)
    validation = commands.add_parser("validate")
    validation.add_argument("path", type=Path)
    validation.add_argument(
        "--profile",
        action="append",
        choices=sorted(PROFILES),
    )
    validation.add_argument("--scope", action="append", default=[])
    validation.add_argument(
        "--model", type=Path, help="Model manifest when path is a records file"
    )
    validation.add_argument(
        "--activity", choices=["model_changes", "implementation", "acceptance"]
    )
    for name in ("host-context", "package", "parallel-plan", "import-audit"):
        validation.add_argument("--" + name, type=Path)
    validation.add_argument("--unsupported-extension", action="append", default=[])
    validation.add_argument("--output", type=Path)
    validation.add_argument("--graphify", type=Path)
    validation.add_argument("--graphify-profile")
    validation.add_argument("--graphify-producer-version")
    validation.add_argument(
        "--view", type=Path, help="HTML artifact for visualization verification"
    )
    view = commands.add_parser("view", help="Generate an offline interactive GDD graph")
    view.add_argument("path", type=Path)
    view.add_argument("--output", type=Path, required=True)
    view.add_argument("--scope", action="append", default=[])
    workflow = commands.add_parser(
        "workflow", help="Prepare packets and coordinate existing development agents"
    )
    workflow_commands = workflow.add_subparsers(dest="workflow_command", required=True)
    for action in ("plan", "verify", "create", "status", "event"):
        sub = workflow_commands.add_parser(action)
        sub.add_argument("path", type=Path)
        sub.add_argument("--host-context", type=Path)
        sub.add_argument("--output", type=Path)
        if action == "plan":
            sub.add_argument("--request", type=Path, required=True)
        if action in {"verify", "create"}:
            sub.add_argument("--plan", type=Path, required=True)
        if action in {"create", "status", "event"}:
            sub.add_argument("--run", type=Path, required=True)
        if action == "event":
            sub.add_argument("--event", type=Path, required=True)
    graphify = commands.add_parser(
        "graphify", help="Import, verify, recover or propose from Graphify observations"
    )
    graph_commands = graphify.add_subparsers(dest="graph_command", required=True)
    for action in ("import", "validate", "export", "propose"):
        sub = graph_commands.add_parser(action)
        sub.add_argument("path", type=Path)
        if action in {"import", "export", "propose"}:
            sub.add_argument("--output", type=Path, required=True)
        if action in {"import", "validate"}:
            sub.add_argument("--profile", required=action == "import")
            sub.add_argument("--producer-version")
        if action == "import":
            sub.add_argument("--code-revision")
            sub.add_argument("--source-id", default="source.graphify")
        if action == "propose":
            sub.add_argument("--mapping", type=Path, required=True)
    for command in ("snapshot", "context"):
        sub = commands.add_parser(command)
        sub.add_argument("path", type=Path)
        sub.add_argument("--output", type=Path)
        if command == "context":
            sub.add_argument("--scope", action="append", required=True)
    args = parser.parse_args()
    if hasattr(args, "scope"):
        args.scope = list(dict.fromkeys(args.scope or []))
    try:
        if args.command == "workflow":
            if args.output and args.output.exists():
                raise FileExistsError(f"Output already exists: {args.output}")
            host = parse(args.host_context.read_bytes()) if args.host_context else None
            action = args.workflow_command
            if action == "plan":
                result = prepare_workflow(
                    args.path, parse(args.request.read_bytes()), host_context=host
                )
            elif action == "verify":
                result = verify_workflow(
                    args.path, parse(args.plan.read_bytes()), host_context=host
                )
            elif action == "create":
                result = create_run(
                    args.path,
                    parse(args.plan.read_bytes()),
                    args.run,
                    host_context=host,
                )
            elif action == "status":
                result = run_status(args.path, args.run, host_context=host)
            else:
                result = advance_run(
                    args.path,
                    args.run,
                    parse(args.event.read_bytes()),
                    host_context=host,
                )
            # Plan output is directly consumable by verify/create. Other outputs
            # retain receipts, current gates and whether an event was applied.
            report = result.get(
                "report", result.get("status", {}).get("report", result)
            )
            code = exit_code(report)
            if action == "event" and (result.get("applied") or result.get("replayed")):
                code = 0
            if action == "plan" and args.output:
                with args.output.open("x", encoding="utf-8") as file:
                    file.write(
                        json.dumps(
                            result["artifact"],
                            ensure_ascii=False,
                            indent=2,
                            allow_nan=False,
                        )
                        + "\n"
                    )
                args.output = None
        elif args.command == "view":
            result = render_view(args.path, args.output, scope=args.scope)
            code = exit_code(result)
        elif args.command == "graphify":
            if args.graph_command == "import":
                result = import_graphify(
                    args.path,
                    args.output,
                    profile=args.profile,
                    producer_version=args.producer_version,
                    code_revision=args.code_revision,
                    source_id=args.source_id,
                )
            elif args.graph_command == "validate":
                result = verify_graphify(
                    args.path,
                    profile=args.profile,
                    producer_version=args.producer_version,
                )
            elif args.graph_command == "export":
                result = export_graphify(args.path, args.output)
            else:
                try:
                    manifest = args.path.resolve()
                    root = manifest if manifest.is_dir() else manifest.parent
                    output_parent = args.output.parent.resolve()
                except RuntimeError as error:
                    raise ValueError(
                        f"Cannot resolve proposal paths: {error}"
                    ) from error
                result = propose_graphify(
                    args.path,
                    parse(args.mapping.read_bytes()),
                    source_uri=Path(
                        os.path.relpath(root / "graphify.raw.json", output_parent)
                    ).as_posix(),
                )
                with args.output.open("x", encoding="utf-8") as file:
                    file.write(json.dumps(result, ensure_ascii=False, indent=2) + "\n")
            code = 0 if args.graph_command == "propose" else exit_code(result)
        elif args.command == "validate":
            options = {
                name: (
                    parse(getattr(args, name).read_bytes())
                    if getattr(args, name)
                    else None
                )
                for name in ("host_context", "package", "parallel_plan", "import_audit")
            }
            result = validate(
                args.path,
                profiles=args.profile,
                scope=args.scope,
                model_path=args.model,
                activity=args.activity,
                unsupported_extensions=frozenset(args.unsupported_extension),
                graphify_path=args.graphify,
                graphify_profile=args.graphify_profile,
                graphify_producer_version=args.graphify_producer_version,
                view_path=args.view,
                **options,
            )
            code = exit_code(result)
        else:
            report = validate(args.path)
            code = exit_code(report)
            if code:
                result = report
            else:
                bundle = load_bundle(args.path)
                if args.command == "snapshot":
                    result = bundle.snapshot or report
                    if bundle.snapshot is None:
                        code = 2
                else:
                    result = context_manifest(bundle, args.scope)
        output = (
            json.dumps(result, ensure_ascii=False, indent=2, allow_nan=False) + "\n"
        )
        if args.command not in {"graphify", "view"} and args.output:
            with args.output.open("x", encoding="utf-8") as file:
                file.write(output)
        else:
            sys.stdout.write(output)
        return code
    except (OSError, ValueError) as error:
        sys.stderr.write(f"gdd: {error}\n")
        return (
            2
            if isinstance(error, InputError) and error.code == "FORMAT_UNSUPPORTED"
            else 1
        )
