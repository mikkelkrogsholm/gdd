# Agent workflow 0.1 — package 0.5.0

Delivered a tool layer on top of GDD 0.2: complete agent packets, separate
implementation scope and reading context, declared parallelism, a dependency queue,
atomic events, resume, and gated integration/acceptance. Existing native
agents perform the code work. There is no new runtime dependency or change to
the GDD core format. [Contract and usage](../WORKFLOW.md).

| Actual check | Result | Evidence |
| --- | --- | --- |
| Full pytest suite | 1,328 PASS, 0 errors | [JUnit](workflow-pytest.xml) |
| GDD standard | 58/58 | [JSON](workflow-core-conformance.json) |
| Graphify | 18/18 | [JSON](workflow-graphify-conformance.json) |
| Viewer artifact | 16/16 | [JSON](workflow-visualization-conformance.json) |
| New workflow expectations | 20/20 | [JSON](workflow-conformance.json) |
| Offline Chrome | 23/23, no network calls/script errors | [JSON](workflow-browser/browser-conformance.json) |
| Installed wheel outside checkout | 18 CLI steps; 38 runtime/asset/schema files byte-identical | [JSON](workflow-wheel-smoke.json), [reproducible check](workflow-wheel-smoke.source.py) |
| Actual native code work | 18 tests/7 failures → 19 PASS; 10 events; current acceptance | [Demo](../examples/agent-native/README.md), [summary](../examples/agent-native/evidence/summary.json) |

Four read-only subagents examined model/planning, protocol/persistence,
CLI/demo, and adversarial inputs in parallel. Root was the sole checkout writer.
Workers provided test proposals and later actual code changes in temporary copies.
Three issued packets were executed: discount and tax in parallel, quote after both
integrations. The original floor assertions were preserved. The new regression
checks actual CLI output for `10**30+1` without floating-point rounding errors.

Confirmed findings were fixed with regressions: codebase/spec base were separated;
unready scope cannot be concealed by parallel PASS; complete packets are authenticated against
the model before run publication; falsey host inputs and newline IDs are rejected;
dispatch/integration bind to the host's concrete implementation/codebase;
dangling model references are rejected before slicing; historical record-only diagnostics
do not block valid scoped gates; cancellation and late submissions are preserved
when the model is unavailable. Idempotency is checked before stale-head rejection,
attempts are not reused, and competing publishers can commit only one event.

Acceptance requires separate current evidence and authorization, the exact integrated
implementation/code revision, frozen check selections, and resolved acceptance questions.
Tests use synthetic receipts for negative protocol cases. The demo's
application evidence, however, is based on actually executed code and CLI, with raw
results, a negative control, and host-verified bindings. Worker PASS was not
used as integration evidence. The installed wheel completed a new actual replay.

The agent-readable linter finds 19 AR002 advisories in existing code blocks and
no new strong boundary findings. [Raw advisory list](workflow-agent-readable.json).
It is a readability aid, not an implementation evaluator.

V1 has a deliberately limited responsibility: the host protects event calls/the filesystem,
establishes actual work isolation, transports agent tasks, performs review,
merge, and checks, and supplies explicit trust. Log hashes detect tampering;
they are not cryptographic authentication. Model drift requires a new plan/run with history
preserved; integrated jobs are not automatically rebased or invalidated.

[Machine-readable aggregate status and source hashes](workflow-review.json). Evidence
and distributions from earlier versions are preserved. The current stopping boundary is reached.
