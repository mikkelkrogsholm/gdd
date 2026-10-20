# GDD validator 0.8.0 — GDD 0.2 and 0.3

The validator loads version-dispatched GDD 0.2 and 0.3, checks the model, and assesses the structural readiness of a selected scope. It has a Python API and CLI and writes reports in `gdd-report/0.2`. Pseudocode, extension code, and application tests are not executed.

Start with the [graph-to-code tutorial](docs/tutorial.md) for a complete worked
example, or [INSTALL.md](INSTALL.md) for setup. This page is the command and API
reference. The [validation standard](standard/VALIDATION.md) defines model,
readiness, authorization and implementation assessments; [HOST.md](HOST.md)
explains how actual checks become trusted evidence.

Python 3.11 or newer is required. `uv.lock` pins the installation. Run from the project directory:

```sh
uv sync --locked
uv run gdd validate examples/concert.gdd.yaml
uv run gdd validate examples/concert.gdd.yaml --profile readiness --scope action.reserve
uv run gdd validate examples/concert.gdd.yaml --profile readiness --scope screen.reserve
uv run gdd validate examples/concert.gdd.yaml --profile records
uv run gdd validate examples/concert.gdd.yaml --profile records --activity implementation --scope action.reserve
```

The five validations yield model-PASS, backend-readiness-PASS, screen-readiness-FAIL, implementation-UNKNOWN, and authorization-UNRESOLVED, respectively. The screen's missing design source is also retained as UNKNOWN. Records contain plans, not executed application evidence.

Repeat `--scope` and `--profile` for multiple selections. `--output report.json` saves the report in a new file; existing files are not overwritten. Validation exit 0 means all reported requested checks are PASS/GRANTED; 1 means at least one FAIL/DENIED; 2 means UNKNOWN/UNRESOLVED without a known failure. Dimensions are separate; `aggregate(report['checks'], 'readiness')` in `gdd_validator.report` aggregates one dimension with FAIL before UNKNOWN before PASS.

## Read status and potential change consequences

```sh
gdd status model.gdd.yaml --scope action.reserve --format text
gdd impact model.gdd.yaml --scope action.reserve --format text
```

`status` composes existing assessments and optional explicitly selected workflow
state; it keeps model validity, scoped readiness, authorization and implementation
evidence separate. Next actions trace to diagnostics or absent inputs. `impact`
explains declared potential callers/dependents with directional witnesses and
keeps supporting context distinct from write ownership. Both are read-only and
JSON-first; text is an optional projection of the same result. Their local tool
contracts are `gdd-status/0.1` and `gdd-impact/0.1`, not core report profiles.
See [inspection, exact coverage and exit policies](docs/inspection.md).

To review an actual model change, use `gdd diff BASE HEAD --format text`.
It compares loaded declarations by stable IDs, separates byte revisions from
structural changes and flags possible weakening for review. Incomplete inputs
stay visible and cannot establish absence-based deletions. The separate local
contract is `gdd-model-diff/0.1`; see [model comparison](docs/model-diff.md).

## Create a draft project

`gdd init PATH --project-id project.example --title "Example"` creates
`gdd.json`, `SOURCES.md` and empty `records.json` in a new directory whose parent
already exists. It validates before publishing and refuses existing files,
directories and destination symlinks. Repeated initialization exits 1 and
preserves the original bytes.

Successful initialization exits 0 and emits a `gdd-init/0.1` receipt with the
manifest path and actual draft revision. Its model is PASS and readiness is
deliberately FAIL: unresolved scope and missing contracts still need refinement.
The starter supplies no workflow mandate, accepted input or execution evidence.
See [installation and skill setup](INSTALL.md).

## Supported checks

The JSON/YAML consumer accepts at most 256 nested containers, counting a root
mapping or sequence as one. Greater depth yields `FORMAT_UNSUPPORTED`/UNKNOWN,
independent of the Python parser's changing stack capacity. This is a consumer
resource limit, not a declaration that deeper GDD documents are invalid.
Symlink loops are invalid local paths (FAIL); genuinely missing inputs remain
unavailable (UNKNOWN). See [consumer input limits](docs/consumer-input-limits.md).

| Profile/operation | Implemented |
| --- | --- |
| init | Valid, explicitly unready GDD 0.3 draft; exclusive publication with no overwrite or seeded authority |
| representation | YAML 1.2 with JSON scalars and JSON; safe built-in tags; no custom tags, anchors, aliases, merge keys, duplicate keys, invalid Unicode, or infinite numbers; local schemas and loading within the bundle root, including symlinks |
| model | Directed multigraph without edge collapse; identities/references, endpoints, ownership, refinement/realization, machines, abstract types, source lineage, and declarative extensions |
| readiness | Fixed-point closure; contracts, interfaces/data, five risk assessments, error outcomes, scenarios, applicable rules, scoped questions, and normative local contract/design sources |
| records | Project/ID/supersession checks; current check plans, immutable check hash, scope, revision matching, bindings, environment, and host-verified evidence; authorization against separately trusted host input |
| context | Candidate package compared with authoritative closure, including edge IDs, sources/lineage, questions, and extensions; structural readiness also runs |
| parallel | Separate plan assessment of completeness declarations, base, locked contracts, read/write files, declared isolation, integration owner/checks, and serialization |
| import.graphify | [The Graphify adapter](GRAPHIFY.md) validates explicit AST/node-link profiles, raw integrity, multiedges, references, direction, and observation projection. Unknown versions/directions are UNKNOWN |
| visualization | [Offline renderer](VIEWER.md); `--view FILE` checks the exact artifact/profile against the current model revision. Without input/with an unknown profile: UNKNOWN |
| check-code | [Static Python import observation](docs/python-imports.md) from current implementation-specific bindings and explicit forbid constraints; separate observation envelope, no project code execution |

Extensions use exact manifest versions and byte hashes. The generic declarative Draft 2020-12 profile checks `data_schema`, endpoints, and context/dependency/effect, including targets of active local schema references. There is no implicit substitutability with core kinds. Custom implementation readiness requires a separate profile and therefore yields UNKNOWN. Remote schema references are never fetched. Schema/data depth exceeding the runtime validator's capacity yields UNKNOWN. `--unsupported-extension example.sensors` makes missing consumer support explicit.

Numeric inputs use Python integers and finite floats when the number's decimal value can be preserved through round-trip. Ordinary values such as `0.1` are supported. Contract bounds, enum uniqueness, and local JSON Schema assertions are assessed on exact numeric values in temporary copies, including when integers are mixed with decimal numbers. Model data, snapshots, and artifacts retain ordinary JSON values. Decimals that would be rounded to a different decimal value, underflow/overflow, and integers beyond the runtime parser's capacity yield `FORMAT_UNSUPPORTED`/UNKNOWN. This applies to both JSON and YAML, including explicit built-in tags; no positive assessment is inferred from a changed number. Invalid NaN/Infinity literals still yield FAIL.

Declared imports edges are checked against architecture constraints in the model. A direct prohibition applies to the specific edge; a transitive prohibition applies to nonempty import paths, including cycles. A contradiction yields CONSTRAINT_INVALID. This proves no actual import relationships in the implementation and does not turn the graph into a complete allowlist.

Readiness scopes are implementation targets: component, entity, action, screen, or machine. Rule/scenario/community cannot alone constitute an implementation-ready scope. An empty ordinary readiness scope assesses all targets, including custom nodes with unknown readiness semantics. A child includes ancestors and necessary contracts; a parent scope includes its decomposed children. Distant rules can expand read context. Communities or hop counts do not limit obligations.

## Observe actual Python import relationships

```sh
gdd check-code model.gdd.yaml --root src --implementation implementation.main --profile python-imports/0.1 --records current.records.json --scope rule.api_boundary --output new-observation.json
```

This separate operation reads current binding records, inventories Python source
and compares static imports with explicit direct/transitive architecture rules.
It reports shortest violating paths, exact source positions, incomplete coverage
and advisory unclaimed/undeclared code. The required profile and root define the
bounded interpretation; unsupported/dynamic input stays UNKNOWN. Missing model
imports edges are not prohibitions. No project modules or pseudocode are executed.

The output is `gdd-code-observation/0.1`, not a new `gdd-report/0.2` profile or
trusted implementation evidence. It records actual bytes and binding digests;
the existing host boundary still governs execution evidence and acceptance.
See [Python profile, mappings and limitations](docs/python-imports.md) and the
informative [output schema](schemas/code-observation.schema.json).

## Revisions and work packages

Readiness diagnostics name missing contract, behavior, boundary and interface
details in stable order. Locations identify an existing field or its nearest
existing parent in the owning document, including included fragments. The
messages describe obligations; they do not fill in contract content. The
[tutorial](docs/tutorial.md) demonstrates the refinement loop.

```sh
uv run gdd snapshot examples/concert.gdd.yaml --output snapshot.json
uv run gdd context examples/concert.gdd.yaml --scope action.reserve --output context.json
uv run gdd validate examples/concert.gdd.yaml --profile readiness --package context.json
uv run gdd validate examples/concert.gdd.yaml --profile readiness --parallel-plan plan.json
```

A snapshot uses raw bytes, normalized asset paths, and the standard's SHA-256 profile. Records, reports, and the snapshot file itself are excluded. Errors in a record file do not change the computable model revision. Missing normative model assets yield an unknown revision. Historical record targets are never rebound to the current model; this version does not load historical snapshots, and dependent checks become UNKNOWN.

With unresolved global validation, `snapshot` and `context` emit a diagnostic
report rather than a reading artifact. Inspect its `format` and assessment
dimensions. Current scoped workflow gates can pass independently of unrelated
historical UNKNOWN results. [HOST.md](HOST.md) explains preserving frozen
revision bundles and reading artifacts without rebinding historical claims.

`context` emits an ID manifest with `format: gdd-context/0.2`, `spec_revision`, `scope`, `nodes`, `edges`, `sources`, `questions`, and `extensions`. It points to complete contracts in the original bundle. It is the context part of a work package; deliverables and the concrete agent task still need to be specified by the orchestrator.

The parallel plan's tool format is `gdd-parallel-plan/0.2`. Each package has `id`, `scope`, `base_revision`, `write_resources`, `read_resources`, `locked_contracts`, `workspace`, and `integration_plan`. The top level has `packages`, `serialized` with `[before, after]` pairs, and `integration: {owner, checks}`. Missing plan obligations yield UNKNOWN. Exact POSIX file paths are normalized before overlap checking; a terminal `/`, `.`, or `..` owns the directory's subtree. Globs do not prove disjointness. Declared workspace isolation is a structural claim that the orchestrator must establish. Plan-PASS grants no authorization, action readiness, or runtime guarantee.

The tool's envelopes are local API contracts, not new core fields in GDD. Their schemas are in [validator-inputs.schema.json](schemas/validator-inputs.schema.json), derived directly from the API's `input_schema`.

## The host's trust boundary

Without host input, an accepted receipt provides no authority or proof of execution. The CLI flag `--host-context host.json` is an **explicit attestation boundary**: the calling user/host takes responsibility for the trusted claims in the file. The validator never reads this file automatically from the model. Do not use host context supplied by untrusted model/source content.

The opt-in [Python host helpers](HOST.md) capture actual snapshots, execute
caller-selected argv and append checked records. The validator never invokes
them automatically from graph content or method labels.

Authorization requires a `policy` with `revision`, `actor`, `activity`, `spec_revision`, `scope`, `requirement`, `authorities`, and `basis`. An empty policy scope covers the project. A bounded scope cannot grant whole-project permission. `requirement` is explicit, standing, or none. Current trusted denials also apply under standing/none. Acceptance additionally requires the host's `implementation` and `code_revision`. `trusted_decisions` identifies receipts whose identity and authority the host has actually verified. Unrelated, untrusted, or invalid supersession cannot revoke a denial. Invalid used record identities or current references cannot provide positive semantic results; unrelated record errors are reported separately.

For evidence evaluation, the host supplies `implementation`, `code_revision`, `supported_methods`, `trusted_evidence`, `verified_bindings`, `environment`, and `negative_controls`. These are attestations of performed checks, including raw output/provenance and the actual environment. They are not instructions to execute checks. Environment names from the check plan must appear in the evidence with the same host-verified values. A frontend binding does not satisfy a backend check's mapping requirements. A binding directly to a rule ID can specify explicit test mapping; otherwise, rule targets and constraint endpoints must be covered.

Multiple active plans/evidence receipts are ambiguous without selection. `check_selection` is `{rule_id: {method: check_id}}`; `evidence_selection` is `{check_id: evidence_id}`. Trusted evidence supersession must match check, implementation, spec/code revision, and cover the previous scope. Verified binding supersession requires the same implementation and revisions, with covering targets. Current evidence scope must reference existing model IDs; extra scope provides no extra verification. Exceptions never change FAIL/UNKNOWN into PASS; the tool does not interpret a policy-defined exception as automatic acceptance.

A separate record file can be assessed with `gdd validate records.yaml --model model.yaml --profile records`. A file already listed in the manifest's `record_files` is loaded once.

## Sources and known limitations

Whole-document and line-range are supported for text/plain, text, markdown, text/markdown, JSON/YAML, and DESIGN.md alpha, as well as complete OKF 0.2 text assets. JSON Pointer resolves in JSON/YAML or DESIGN.md's YAML frontmatter; all raw DESIGN.md bytes, including prose, are included in the hash. This does not certify the external format's own conformance. Markdown-heading and opaque selectors are preserved with UNKNOWN. Unknown contract formats yield UNKNOWN. External URIs are not materialized automatically.

Source lineage follows source+revision identities. Missing historical input revisions yield UNKNOWN; earlier self-revisions are not automatically cycles. ID reuse and immutable log changes over time require preserved history, which this file-based checker does not establish on its own. Domain truth, actual scenario coverage, complete effect declarations, authenticity of authority, and application behavior require review or trusted checkers, respectively.

## Verification and API

```sh
uv run pytest -q
GDD_CHECK_DIR="$(mktemp -d)"
uv run python conformance/run.py --output "$GDD_CHECK_DIR/validator-conformance.json"
uv build
```

The conformance runner executes the standard cases against concrete fixtures.
Regression tests also cover parser, paths, extensions, context, authority,
supersession, evidence and CLI behavior. Use [CONTRIBUTING.md](CONTRIBUTING.md)
for the current local check commands and [evidence](evidence/README.md) for
version-specific execution records. Synthetic mandates and receipts in tests
are fixtures, not actual approvals or application evidence.

```python
from gdd_validator import validate, exit_code

report = validate("examples/concert.gdd.yaml",
                  profiles=["readiness"], scope=["action.reserve"])
assert exit_code(report) == 0
```

The API returns a JSON-compatible report object. Invalid tool arguments raise `ValueError`; model/file failures produce report diagnostics. Revision and report formats are validated internally. [The code](gdd_validator/validator.py) is the entry point for orchestration; loading/model/readiness/planning/records contain the respective checks.

The [agent workflow](WORKFLOW.md) is a separate tool layer for preparing work
and tracking integration. For an executed parallel-work example, see the
[quote example](examples/agent-native/README.md).


## GDD 0.3 boundary and advisory checks

Legacy 0.2 manifests/fragments use preserved schemas and semantics. New 0.3
manifests/fragments require matching versions; records/extensions/report/snapshot
and existing workflow envelope versions remain unchanged.

The model checker rejects exports owned by a different component, resolved calls to
another component's private action, and broad principle rules marked blocking or
using another category. Unknown ownership/interfaces remain valid sketches.
Readiness requires relevant component responsibility/rationale/exclusions, unique
public providers and resolvable call consumers, and action postconditions. Nonblank
text is structurally checked; the quality of its content requires review.

`gdd boundaries MANIFEST` derives public interfaces, community membership basis,
call classification, unresolved inputs and explicit omitted calls. It writes no
intent. The Python API is `boundary_view(bundle, scope=None)` in
`gdd_validator.boundaries`; a supplied scope is a read-context ID list, not an
assertion that entire communities have been analyzed.

`gdd design-review MANIFEST --review FILE --implementation ID --code-revision REV
--scope NODE` checks the advisory envelope and its applicability to host-supplied
identities/scope. Repeat scope as needed. A missing scope selection reports only the
review's declared scope. COMPLETE returns 0, INVALID 1, UNKNOWN 2. Findings do not
change blocking acceptance checks. The tool does not inspect code or verify that a
review actually occurred. Its schema is [design-review](schemas/design-review.schema.json).
New cases run with `uv run python conformance/run_design.py`.
