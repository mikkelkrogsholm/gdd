# Validator review — October 13, 2026

GDD validator 0.2.0 is implemented against the existing GDD 0.2 draft. The standard's normative rules, six core schemas, and 58 case expectations are preserved. Status descriptions in the documentation have been updated. The tool's host/context/parallel/import envelopes are separate informative API contracts.

## Actually performed

| Check | Result | Evidence |
| --- | --- | --- |
| The standard's 58 conformance cases | 58/58 expectations match actual validator execution | [validator-conformance.json](validator-conformance.json) |
| Pytest, including the conformance harness and regressions | 74 passed | [validator-tests.txt](validator-tests.txt) |
| Source distribution and wheel | Built | [validator-build.txt](validator-build.txt) |
| Wheel installation in a temporary environment outside the workspace | CLI/readiness PASS; all seven schema assets included in the package | [validator-wheel-smoke.json](validator-wheel-smoke.json) |
| Concert example backend, action.reserve | model PASS, readiness PASS | [validator-concert.readiness.json](validator-concert.readiness.json) |
| Concert example screen.reserve | readiness FAIL; unavailable design remains UNKNOWN | [validator-concert.screen.json](validator-concert.screen.json) |
| Concert example implementation | UNKNOWN; plans alone provide no execution evidence | [validator-concert.records.json](validator-concert.records.json) |
| Implementation without a trusted mandate | authorization UNRESOLVED | [validator-concert.authorization.json](validator-concert.authorization.json) |
| Snapshot and backend context | Same snapshot revision as the standard example; 34 nodes and 73 canonical edges | [snapshot](validator-concert.snapshot.json), [context](validator-concert.context.json) |

Commands from the project root:

```sh
uv run pytest -q
uv run python conformance/run.py --output evidence/validator-conformance.json
uv build
uv run gdd validate examples/concert.gdd.yaml --profile readiness --scope action.reserve
uv run gdd validate examples/concert.gdd.yaml --profile records
```

The installed wheel was tested with a separate temporary virtualenv and a working directory outside the repository directory. No global installation or publication was performed. [validator-assets.json](validator-assets.json) records the concrete implementation/test/standard assets and checksums for the check output. Reports have normal version identity; this separate manifest identifies the exact bytes underlying the checks.

## Review findings and fixes

Three read-only subagents reviewed model semantics, the scoped protocol, and integrations in parallel. The lead wrote and integrated all changes. The review led to regression tests for:

- Duplicate JSON/YAML keys, YAML 1.2 scalars, permitted built-in tags, custom tags, aliases, invalid Unicode, infinite numbers, and symlink escapes.
- Recursive type references and optional placement, duplicate owners, long acyclic graphs, edge-source selectors, and core constraints.
- Extension metaschemas before traversal; remote refs without networking; inert `$ref` under const and legitimate data fields such as code.
- Parent children in context; no context bypass of readiness; canonical edges; a community is not an empty implementation scope.
- Current rejections under explicit, standing, and none; untrusted/unrelated supersession cannot conceal them; corresponding protection of failed evidence.
- Evidence binding to relevant scope; revision-bound check hashes; actual host-attested environments, negative controls, and checker provenance.
- Missing records do not affect a computable model revision; standalone records do not gain model PASS merely from schema PASS; invalid API scopes produce controlled input errors.
- Known/unknown contract formats, DESIGN.md frontmatter, and normalized write overlap with explicit serialization.

The agent-readable-code linter was reviewed. The remaining four advisory findings are short, similar diagnostic blocks and the local source-lineage calculation in CLI/package checks. They were reviewed as advisory findings. No implementation file exceeds 800 lines or contains lines longer than 400 characters. No extra abstraction was introduced to eliminate these local similarities.

## Scope and limitations

This is structural and revision validation. Domain pseudocode, actual scenario coverage, and the truth of declarations require review or checkers. 58 matching expectations do not fully certify all possible standard requirements.

Condition cases use synthetic mandates, environment attestations, and receipts. They document validator behavior; they are not actual authorization or evidence for a concert application. No such application was built or tested.

The Graphify profile detects loss between original and reconstructed raw input, but a production Graphify adapter is not implemented and yields UNKNOWN. Visualization, custom-kind readiness, historical snapshots, markdown-heading/opaque-selector resolution, and automated policy-controlled exception acceptance are also not implemented. Dependent checks are UNKNOWN or require separate trusted host input as described in [VALIDATOR.md](../VALIDATOR.md).

Phase 1's standard review and authoring hashes are retained as historical artifacts. They no longer identify the current bytes of documentation files whose status was updated in this phase.
