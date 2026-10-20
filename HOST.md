# Portable host execution and evidence

The GDD CLI prepares and validates models, work packages and run journals. Your
existing agent runtime owns reasoning, actual edits, command execution, review
and trusted policy. A small Python host API avoids recreating snapshot and
evidence plumbing for every project. It has no LLM or provider dependency.

For a complete small example before using these API details, follow the
[graph-to-code tutorial](docs/tutorial.md). It runs real local tests and records
their evidence using the helpers below.

These helpers are **explicit trusted-caller operations**. The validator and CLI
never invoke them from graph content, pseudocode or a check method label. The
caller must already have authorization for the selected command and edits.
Worker results and externally supplied receipts do not grant host trust.

## Choose a scope and actual code base

Refine the intended slice, assess readiness, and preserve the model snapshot.
Choose the implementation identity and actual files independently of graph
labels. The manifest may live in `gdd/` while application files live elsewhere;
code resources are relative to the caller-selected project root.

```python
from pathlib import Path
from gdd_validator.host import snapshot_code

root = Path.cwd()
files = ["src/domain.py", "src/cli.py", "tests/test_domain.py"]
baseline = snapshot_code(root, files)
```

The result is `gdd-code-snapshot/0.1`: selected paths, actual SHA-256 file hashes
and a deterministic revision. Paths are explicit, relative and normalized; globs,
duplicates, missing files and symlinks are rejected. An empty list represents an
explicitly empty initial code base. Include all implementation and verification
files relevant to the change; do not include the output receipt itself.

The return shape is `{"format": "gdd-code-snapshot/0.1", "files":
{"src/domain.py": "<64-character file SHA-256>"}, "revision": "sha256:<digest>"}`.
`files` is a path-to-hash mapping, not a list of file objects; the selected code
revision hashes that mapping. File hashes have no `sha256:` prefix.

Create immutable check plans in the model's separate records document after the
normative model is finalized. A check names its actual current spec revision,
target rule scope, method, required environment and selected negative controls.
See [the records schema](schemas/records.schema.json) and
[evidence applicability](VALIDATOR.md). A method string does not execute anything.

## Preserve history and reading artifacts

Preserve an earlier model and its records as a frozen revision bundle. A new
specification can use its own ledger, as in the separate A/B journey. Never
rewrite old IDs or bind historical targets to the new graph to obtain PASS.

This validator does not load historical snapshots. Mixing records from old spec
revisions into the current ledger can therefore make global model assessment
UNKNOWN while current scoped readiness and acceptance pass. In that state,
`gdd snapshot` and `gdd context` return a diagnostic report instead of their usual
artifact. Inspect the `format` field and dimensions before consuming output.
A verified workplan includes a current byte snapshot and scoped package context;
preserve those reading artifacts separately from the global historical report.
See [the snapshot limitation](VALIDATOR.md) and [workplan format](WORKFLOW.md).

## Plan, dispatch and integrate

Use the [work-request format](WORKFLOW.md#getting-started) with the observed
baseline code revision, concrete scope, read/write paths, workspace, deliverables,
integration owner and check IDs. Work order belongs in `serialized` only where
there is an actual implementation dependency. Keep context and write ownership separate.

The host context is a separate JSON document selected by the caller. Supply:

- `implementation` and current `code_revision`;
- `policy`: `revision`, `actor`, `activity`, current `spec_revision`, `scope`,
  `requirement`, `authorities`, and the actual mandate's `basis`;
- any `trusted_decisions` only after independently verifying those decisions.

Working mode and approval requirements follow the project and user. A graph label,
copied example or starter does not choose those policy values. Update `activity`
for implementation versus acceptance; preserve any denials or required decisions.

For a resolved blocking question, its referenced decision must be accepted,
current, valid, unsuperseded and included in the host's `trusted_decisions`.
Its explicit `scope` must contain every listed question target; naming only an
owning component does not expand that receipt's scope to its children. Use an
empty global scope only when the actual mandate permits it. A delegated actor
must be covered by the relevant trusted policy authorities. A source's human
author does not automatically supply that delegation.

For manually authored checks or decisions, `append_records` takes a fragment
with the shape `{"records": {"check.example": <actual check document>}}`, not a
bare record-ID map. Each record retains its own current revision and full schema
fields; never rewrite historical IDs after a model change.

```sh
gdd workflow plan gdd/gdd.json --request request.json --host-context host.json --output plan.json
gdd workflow verify gdd/gdd.json --plan plan.json --host-context host.json
gdd workflow create gdd/gdd.json --plan plan.json --host-context host.json --run run
gdd workflow status gdd/gdd.json --run run --host-context host.json
```

Each event names `type`, a unique `idempotency_key`, and the latest status's
`expected_revision`. `dispatch` additionally names `package` and new `attempt`.
Deliver its returned packet through the existing runtime or perform the bounded
work directly. No agent transport is inferred from the journal.

`submit` names the same package/attempt, actual summary, artifact URI and
`changed_resources`. Apply and review the actual patch, rerun relevant checks, and
only then `integrate` with `base_code_revision` equal to the latest run state,
the new actual `code_revision`, verified resource paths and a review containing
reviewer, PASS verdict and real evidence URI. Update host code revision to the
resulting combined snapshot. Use `gdd workflow event ... --event event.json
--host-context host.json` for each transition. The existing
[lead](agents/gdd-lead.md) guide and [event contract](WORKFLOW.md#events-and-progress)
describe cancellation, retry and reconciliation.

## Execute a host-selected check

Select the concrete argv from your own assessment. Execute checks on the combined
code after integration. The helper uses no shell and stores a fresh raw receipt:

```python
import sys
from gdd_validator.host import execute_check

receipt = execute_check(
    root, [sys.executable, "-B", "-m", "unittest", "discover", "-s", "tests"],
    files, output=root / "evidence" / "domain.run.json", timeout=120,
)
```

Create the evidence parent directory first. An existing receipt is refused before
execution so an accidental retry cannot repeat effects. Exit 0 is PASS; exit 1 is
FAIL; other codes, a timeout or unavailable execution are UNKNOWN. Use a checker
with that exit contract. The receipt includes actual stdout/stderr, argv,
timestamps, runner metadata and source snapshots before/after. Changed or missing
selected source during execution remains UNKNOWN, even after exit 0.

A passing test process still does not establish adequate test coverage. The host
must verify the assertions and run declared negative controls on isolated code
copies. Preserve the mutated-code results separately; they are not current-code
PASS evidence. Real source import checks require a concrete language/checker
profile; model `imports` edges establish only intended architecture.

## Append verified records and supply trust

After the host has verified the receipt, coverage, environment and performed
controls, author a new binding/evidence pair:

```python
from gdd_validator.host import verification_records, append_records

fragment = verification_records(
    root / "gdd" / "gdd.json", "check.domain", root / "evidence" / "domain.run.json",
    root=root, implementation="implementation.app", binding_id="binding.domain.v1",
    evidence_id="evidence.domain.v1", test_resource="tests/",
    checker={"name": "stdlib.unittest", "version": sys.version.split()[0]},
    environment={"python": sys.version},
    verified_negative_controls=["actual-isolated-mutation"],
)
append_records(root / "gdd" / "records.json", fragment)
```

Use new IDs. The helper verifies current check/spec identity, the receipt's source
freshness, explicit test-resource hashes and required metadata. A PASS or FAIL is
preserved; UNKNOWN cannot become a completed verification record. `append_records`
uses an exclusive writer lock and atomic replacement, validates the document and
refuses replacement of any existing ID. A stale lock must be reconciled, not deleted
blindly. No helper decides that an assertion or a listed control was adequate.

The returned `fragment["host"]` contains only implementation/evidence trust fields;
it has no operational policy or authority. Merge fragments from actually reviewed
checks by combining their verified binding/evidence/method/control lists, matching
environment values, and explicit selections. Preserve the caller's independent
policy and current implementation/code revision. Do not simply overwrite one
check's trust with another. Evidence from a different code/spec revision cannot be
reused for the new one.

Evaluate implementation with ordinary `--profile records`. Evaluate authorization
separately with `--profile records --activity acceptance`. With all packages
integrated and actual current checks verified, supply the merged host context to
the run's `accept` event. Inspect `acceptance_current` with that same context.

This API reduces record-writing mistakes; it does not authenticate a receipt,
approve its own execution, choose a project policy, certify KISS/DRY, or bypass
the standard's separate readiness, authority and evidence assessments.
