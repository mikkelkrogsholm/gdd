# From a brief to a development scope

This is an application guide. The copied standard supplies the field definitions;
the user's request supplies the project direction and mandate.

Confirm which portion of the outcome is authorized: planning, modeling, code
implementation, or acceptance. Preserve the brief in a local text/Markdown source
and record its actual SHA-256. Keep inferred requirements visibly proposed. Use a
small Markdown knowledge file or the project's existing wiki/OKF; do not add a
knowledge system just to satisfy the method.

Use the installed Rust CLI (`gdd`) and desktop viewer (`gdd-viewer`). From the
parent directory, create a draft in a new directory named `project`.

```sh
gdd init project --project-id project.example --title "Example project"
```

Read the draft manifest at `project/gdd.json` and the accompanying
`SOURCES.md` and `records.json` in the same directory. Fill the project's real goal
and preserve the input as a source. The scaffold contains no trusted mandate or
implementation evidence. If initialization is unavailable, install the documented
CLI or author a JSON/YAML manifest from [FORMAT](gdd/standard/FORMAT.md) and
[gdd.schema.json](gdd/schemas/gdd.schema.json). A valid draft needs `format:
gdd/0.3`, `project` with id/title/goal, `nodes`, and `edges`; it may omit unresolved
contracts and workflow. Do not mark it ready to bypass missing detail.

Model the important component responsibilities and boundaries, actions, domain
data, rules, and scenarios. Add screens, machines, or communities when the brief
needs them. Use stable domain IDs and explicit edges. Show unknowns as scoped
questions rather than inventing answers. Register provenance separately from
normative contract/design sources. A contract source used for readiness must be
materialized within the bundle with its verified revision.

From the same parent directory, validate the model, export the portable view
to a new file, and verify the view against the manifest. Open the file on a
desktop and close the viewer when you have inspected the graph.

```sh
gdd validate project/gdd.json --profile model
gdd view project/gdd.json --output project/graph.gddview
gdd validate project/gdd.json --profile visualization --view project/graph.gddview
gdd-viewer project/graph.gddview
```

Choose the stable ID of the node you want to refine. The scaffold provides
`action.main`. Set `target` to your chosen ID for both commands.

```sh
target=action.main
gdd context project/gdd.json --scope "$target" --output project/context.json
gdd validate project/gdd.json --profile readiness --scope "$target"
```

The fresh scaffold passes model validation. Its scoped readiness check reports
FAIL and exits 1 because the action lacks a contract and the scope question
is open. Use the diagnostics and saved context to refine the selected slice.
Use new output paths when saving another view or context. Follow
[VALIDATION](gdd/standard/VALIDATION.md): full contracts and pseudocode,
postconditions, explicit effects/dependencies, risk/error coverage, normal and
relevant negative/boundary scenarios, component public contracts, and necessary
design sources. Explain why each boundary exists and what it excludes.

Deliver input/sources, questions, manifest, view, actual reports, and a scoped plan
with write resources, dependencies, deliverables, and meaningful planned checks.
An unready draft can be a successful planning deliverable when the open questions
and next decisions are explicit. If code work is authorized and ready, continue
through [the change route](change.md).
Use [portable host execution and evidence](gdd/HOST.md) for actual code snapshots,
caller-selected checks and immutable records when implementation is authorized.
