# GDD agent workflow 0.1

The workflow uses GDD 0.2 to prepare code work and track its
integration. Existing agents from any vendor can use it. The CLI
and Python API start no LLM and execute no commands from the graph.

A lead agent chooses a bounded objective, implementation, code base, packages, and
file areas. The tool derives context, contracts, pseudocode, rules,
scenarios, source lineage, design references, extension semantics, and required
check plans. It checks each package's readiness and parallelism separately.
A community is grouping; it is not proof of independence.

## Getting started

```sh
gdd workflow plan model.gdd.yaml --request request.json --output plan.json
gdd workflow verify model.gdd.yaml --plan plan.json
gdd workflow create model.gdd.yaml --plan plan.json --run run
gdd workflow status model.gdd.yaml --run run
gdd workflow event model.gdd.yaml --run run --event dispatch.json \
  --host-context host.json --output dispatch-result.json
```

`plan --output` saves the plan itself and displays the report/plan on stdout. The
other commands save their complete JSON result. Existing destinations
are never overwritten. `create` atomically creates a new run directory; it accepts an
unready draft but rejects a modified or historical package. `status` is read-only.

A request follows [workflow.schema.json](schemas/workflow.schema.json):

```json
{
  "format": "gdd-work-request/0.1",
  "id": "work.feature",
  "objective": "Implement the selected feature and its tests",
  "implementation": "implementation.app",
  "base_code_revision": "host-verified-code-revision",
  "packages": [{
    "id": "package.feature",
    "scope": ["action.feature"],
    "write_resources": ["src/feature.py", "tests/test_feature.py"],
    "read_resources": ["src/contracts.py"],
    "workspace": "../workspaces/feature",
    "deliverables": ["Implemented feature and regressions"],
    "integration_plan": "Review the patch and run shared checks"
  }],
  "serialized": [],
  "integration": {"owner": "actor.lead", "checks": ["check.feature"]}
}
```

The agent chooses filenames and runtime to match the repository. They are not derived from a
language-neutral action ID. `scope` is the implementation mandate;
`implementation_targets` includes its refinement. `context.nodes` is the
larger read context and must not become additional write ownership. The package contains
complete records and locators, as well as the original model snapshot. Local normative
assets are read from the model and checked against the snapshot; they are never executed.

`serialized: [["package.a", "package.b"]]` means A must be integrated
before B is dispatched. `calls` and `depends_on` describe product contracts; they do not
automatically invent this work order. Independent packages can be dispatched from the same
code base. The host establishes actual isolation, permissions, and appropriate read-only
access. Shared-checkout subagents deliver temp patches; concurrent code writers
must have isolated worktrees. Logical workspace paths prove no isolation.

## Events and progress

All events require `expected_revision` from the latest status and a unique
`idempotency_key`. Dispatch example:

```json
{
  "type": "dispatch",
  "expected_revision": "sha256:...",
  "idempotency_key": "dispatch.feature.1",
  "package": "package.feature",
  "attempt": "attempt.feature.1"
}
```

| Event | Precondition and effect |
| --- | --- |
| `dispatch` | Current package/readiness, implementation authorization, and host-confirmed implementation/code base. Returns the package; the host transports it to the agent. |
| `submit` | The same dispatched attempt. Stores `summary`, `artifact_uri`, and `changed_resources` as worker claims. |
| `integrate` | Current submission, current authorization, review, and host-confirmed code/file result. Releases successors. |
| `retry` | Only unfinished attempts. The next dispatch requires an attempt token never used before. |
| `cancel` | Stops new development actions. Preserves code, artifacts, and late submissions. |
| `resume` | Explicit resumption with current gates and a host-confirmed code base. Restarts no agent. |
| `accept` | All packages integrated, current implementation evidence PASS, resolved acceptance questions, and acceptance authorization GRANTED. Terminal. |

Submit contains an artifact URI and actual changed filenames. It can neither
contain host-trust fields nor grant implementation-PASS. Integrate must contain
`package`, `attempt`, `base_code_revision`, `code_revision`, `verified_resources`,
and `review: {reviewer, verdict: "PASS", evidence_uri}`. The host independently confirms
the patch, changed files, and review. The file list must match the submission and be
within the write area. The host supplies the resulting `code_revision` in its
separate host context. A worker's own hash claim cannot prove it.

Two workers can start at C0. After the first integration from C0→C1, the second
integration must use C1→C2, even when the worker's original base was C0.

The run directory contains `run.json` and a hash chain of exclusively published
`events/000001.json`, etc. A concurrent head conflict can publish only one event.
The same key and an identical request recover the original receipt before the head check;
this returns current gates and **no new dispatch package**. A key with a changed
request is rejected. `status` shows dispatched attempts, so the lead can reconcile actual
agent delivery after an interruption. It never starts a replacement automatically.

Logs are local audit claims with tamper detection, not cryptographically authenticated
history. The host protects the filesystem and event calls. Even a historical
`accepted` status is reassessed: `acceptance_current` requires current gates and
explicit host context. The model, record files, and worker results are not sources
of host authority.

## Checks and changes

Check plans must cover each blocking rule × verification method exactly
once. V1 supports this GDD semantics: extra checks without required-pair coverage
or multiple selections for the same pair make the plan UNKNOWN. Accept pins the evaluator's
selections to the plan's check IDs; other host selections are rejected. The host performs the actual
checks and records existing GDD bindings/evidence with exact
check, model, implementation, and code revisions. Worker tests from C0 do not provide
integration proof for C2. See [the validator's host contract](VALIDATOR.md).

Model/asset changes or modified selected check plans require a new plan and a
new run. History is preserved. V1 has no automatic rebasing, merging, model editing,
or invalidation of already integrated jobs. Record-only history diagnostics
are still displayed; current scoped gates are assessed separately and use no historical
approval/evidence as current. Missing actual evidence yields UNKNOWN.

API: `prepare_workflow`, `verify_workflow`, `create_run`, `run_status`, and
`advance_run` from `gdd_validator`. Plan preparation returns `{report, artifact}`;
events return `applied`, `replayed`, a report, and a receipt on commit. Rejected
gates publish no event. Invalid envelopes/head conflicts raise
`ValueError`; the CLI displays a controlled error. Report codes follow the existing
0=PASS/GRANTED, 1=FAIL/DENIED, 2=UNKNOWN/UNRESOLVED. A committed or idempotently recovered event returns 0; other diagnostics are retained in the report. Always inspect `applied`/`replayed` for events.

## Agent instructions and actual example

Use [the lead guide](agents/gdd-lead.md) and [the worker guide](agents/gdd-worker.md)
directly in an existing agent runtime. They contain no vendor-specific
API. [The quote example](examples/agent-native/README.md) demonstrates parallel native
agent repairs from half-up to the graph's floor contract, actual red/green test execution,
integration, and revision-bound acceptance. Python is only the example implementation.

Run the workflow's 20 named conformance expectations with `uv run python conformance/run_workflow.py --output workflow-conformance.json`. The harness uses the pytest development dependency; the runtime package adds no test or agent dependency.
