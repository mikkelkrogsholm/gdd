# Review a model change

`gdd diff` compares two loaded models by stable IDs. It reports exact changes
to declarations and points out structural changes that may weaken a contract.
It is read-only and does not execute code, authorize changes or refresh evidence.

```sh
gdd diff base/gdd.json head/gdd.json
gdd diff base/gdd.json head/gdd.json --format text
gdd diff base/gdd.json head/gdd.json --output new-comparison.json
```

JSON is the default. Text renders the same comparison; an output file must be
new, and destination symlinks are refused. The Python API is
`gdd_validator.diff_models(base, head)`; `diff_text(result)` in
`gdd_validator.model_diff` formats the result without reassessment.

## What is compared

Nodes, edges, sources and questions are matched by their declared IDs. Changed
fields use JSON pointers with their actual before/after values. Project metadata,
format transitions and extension declarations are shown separately. Prose and
pseudocode are compared as exact text; the tool does not interpret their meaning.

Object key order, YAML/JSON formatting and fragment packaging do not create
structural changes. Relocation between files is separate metadata, not deletion
and recreation. Byte-based snapshot revisions can change even when the loaded
declarations are equal; both revisions remain visible. Array order remains part
of exact declared values unless the existing loader normalizes that registry.

Both supported GDD formats can be compared, including 0.2 to 0.3. A format
transition is explicit; comparison does not migrate either input. Records are
evidence about particular revisions, not editable model declarations. They are
not rebound to the head revision by this command.

## Advisory structural indicators

Review indicators cover removal or relaxation of a blocking rule, narrower or
changed `forbid` constraints, loss of a rule's last verifying scenario, removed
contract errors/preconditions/postconditions and removal of public interfaces.
A changed identity kind or role is an identity concern, not proof of historical
ID reuse. Two snapshots cannot establish that an ID was previously deleted and
later reused for a different concept.

Indicators are advisory. A removed postcondition might be intentional; an edited
sentence might weaken a requirement without triggering a structural indicator.
Neither the presence nor absence of indicators establishes semantic safety.
Preserve the rationale and review actual requirements, scenarios and code.

## Partial input and exit codes

The existing loader and model validation assess each side. Their diagnostics,
coverage and revisions remain visible. Missing fragments, unavailable normative
assets and unsupported semantics cannot establish absence-based additions or
deletions. Known common declarations may still be compared, with partial coverage
clearly reported. A partial comparison is not an empty-safe result.
Fully loaded but semantically invalid models can still expose exact structural
differences; their validation failures remain visible and determine exit 1.

Exit 0 means a complete valid comparison, including when changes or advisories
are found. Known invalid input exits 1. Unresolved/partial input exits 2 when no
known failure takes precedence. These codes describe the comparison inputs;
they do not establish readiness, authorization or implementation correctness.

The informative tool contract is `gdd-model-diff/0.1`, described by
[model-diff.schema.json](../schemas/model-diff.schema.json). It does not revise
the standard or its core report contract.

## Use comparison with impact

Review `diff` first to identify changed declarations and their rationale. Then
run `gdd impact HEAD --scope CURRENT_NODE_ID` for potential consequences in the
current graph. Select supported current node IDs explicitly: changed edge IDs,
sources and deleted nodes are not automatically valid impact seeds. For a
deleted node, inspect its preserved base context and relationships as well.
See [inspection and impact coverage](inspection.md).

## Replay the archived requirement change

The concert reference experiment is archived with byte-preserving restoration.
From a repository checkout, use a new destination outside the repository:

```sh
python scripts/concert_archive.py restore \
  --archive archive/experiments/concert-7e27757.tar.gz \
  --index archive/experiments/concert-7e27757.index.json \
  --output "$PWD/../gdd-diff-concert-recovered"
gdd diff \
  "$PWD/../gdd-diff-concert-recovered/examples/concert-app/history/phase-a/gdd.json" \
  "$PWD/../gdd-diff-concert-recovered/examples/concert-app/gdd.json" --format text
```

Follow the [archive instructions](../archive/experiments/README.md) to restore
the experiment into a new destination with a canonical, existing parent. The change summary
surfaces the per-principal reservation limit and corresponding contract/scenario
declarations. This is a replay of historical model differences, not a new
execution of the application or its original experiment.

Run the local behavioral conformance suite with a new receipt path:

```sh
uv run --no-sync python conformance/run_diff.py --output /tmp/new-model-diff.json
```
