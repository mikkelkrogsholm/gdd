# Proposed external GDD pilot

Status: **proposal for maintainer review; no comparative run executed**.
Prepared for [issue #9](https://github.com/mikkelkrogsholm/gdd/issues/9).
Repository inspection date: October 20, 2026.

The next useful experiment is a bounded change to **Huey**, comparing a written
specification plus tests with GDD. Run one rehearsal per arm first to find
protocol and tooling problems. Treat those two runs as a feasibility pilot,
not evidence that GDD improves software development. If the protocol is viable,
freeze it again and run three fresh repetitions per arm.

## Candidate selection and inspected facts

| Candidate | Why it fits | Concern and disposition |
| --- | --- | --- |
| [Huey](https://github.com/coleifer/huey) | Independent Python task queue; public API delegates to a storage interface, including SQLite; existing API and storage tests. | Recommended. Multiple optional backends require an explicit compatibility scope. |
| [TinyDB](https://github.com/msiemens/tinydb) | Clear database/storage/middleware boundary; [MIT license](https://github.com/msiemens/tinydb/blob/edff96aeb2616402e9489d46e90a592c05565621/LICENSE); existing tests. | Inspected pin `edff96aeb2616402e9489d46e90a592c05565621` has only 2,303 physical runtime Python lines across 10 files, below the issue's preferred approximate 5k–30k range. Reserve candidate if Huey's qualification fails. |
| [DiskCache](https://github.com/grantjenks/python-diskcache) | Persistent Python cache with an [Apache-2.0 license](https://github.com/grantjenks/python-diskcache/blob/master/LICENSE) and an existing [test procedure](https://grantjenks.com/docs/diskcache/development.html). | Screened through official sources only. The [core implementation](https://github.com/grantjenks/python-diskcache/blob/master/diskcache/core.py) combines much of the public API and persistence, giving this particular boundary experiment a less clean subject. Not fully qualified or pinned. |

Huey subject revision: **`c5afa051a9663be479669e687c7460bdbbfd204d`**.
Use this exact commit, not a moving branch. Its [MIT license](https://github.com/coleifer/huey/blob/c5afa051a9663be479669e687c7460bdbbfd204d/LICENSE)
permits modification and redistribution with the required notice. Retain that
notice in any published derivative. This is a local experimental change, not an
upstream feature request or an assertion that Huey's maintainers want this API.

Inspection of that checkout found:

- 36 runtime `.py` files under `huey/`, excluding `huey/tests/`: 6,953 physical
  lines, 5,751 nonblank lines. This includes comments and is not a statement count.
- 21 test `.py` files under `huey/tests/`: 6,987 physical lines.
- [`huey/api.py`](https://github.com/coleifer/huey/blob/c5afa051a9663be479669e687c7460bdbbfd204d/huey/api.py)
  exposes `pending(limit=None)` and delegates through `storage.enqueued_items`.
- [`huey/storage.py`](https://github.com/coleifer/huey/blob/c5afa051a9663be479669e687c7460bdbbfd204d/huey/storage.py)
  contains `BaseStorage`, `MemoryStorage` and `SqliteStorage`. SQLite queue
  inspection orders by descending priority, then insertion ID.
- [`runtests.py`](https://github.com/coleifer/huey/blob/c5afa051a9663be479669e687c7460bdbbfd204d/runtests.py)
  uses `unittest`; the pinned suite includes `test_api.py`, `test_storage.py`
  and `test_sql_huey.py`. Optional services and packages have conditional tests;
  their absence must be recorded as skips rather than called a full backend pass.

The counts are reproducible by summing `len(path.read_text().splitlines())` for
those file sets. Source inspection and read-only clones were performed. No
upstream test suite, Graphify import, requirement implementation, hidden oracle,
agent comparison, cost measurement or human review study was executed for this
proposal. Passing qualification remains a condition of starting the pilot.

## Fixed brief to give both arms

> Queue administrators need to inspect a bounded page of pending tasks without
> consuming or executing them. Add `Huey.pending_page(offset=0, limit=20)` and a
> corresponding `BaseStorage.enqueued_page(offset=0, limit=20)` storage contract.
> The public method returns deserialized tasks; storage returns opaque task
> bytes. Both reject booleans, non-integers, negative offsets and limits outside
> 1–100 with `ValueError` before reading storage. Empty or out-of-range pages
> return an empty list. Results follow the backend's existing pending order.
> On Memory and SQLite, equal-priority tasks preserve insertion order and higher
> priority tasks come first. Inspection leaves pending counts, subsequent dequeue
> results, scheduled tasks and result data unchanged.
>
> SQLite must apply the page bound and offset in its database query instead of
> materializing the entire queue. Memory must provide equivalent behavior. A
> compatible default storage implementation may slice the result of the existing
> `enqueued_items()` method so other backends and custom subclasses retain a
> usable fallback without a new mandatory override. That fallback is explicitly
> allowed to materialize the queue. Existing `pending()`, `enqueued_items()` and
> consumer behavior remain compatible. No cross-request snapshot consistency is
> promised while other producers or consumers mutate the queue.
>
> Preserve the API/storage boundary: API code must not import a database driver,
> access storage-private fields or connections, or embed SQL. Storage code owns
> backend access and must not import the public API. Add focused public/API and
> storage tests and document pagination and its concurrent-mutation limitation.

The meaningful components are the public task/serialization API and storage
contract with concrete memory and persistence implementations. Pagination is a
plausible administration requirement and is absent from the inspected API; this
proposal does not claim it came from an external user request.

Allowed subject edits, fixed before either arm starts:

- `huey/api.py`, `huey/storage.py`.
- `huey/tests/test_api.py`, `huey/tests/test_storage.py`.
- `docs/api.rst`.

Method artifacts live outside the subject checkout in separate arm artifact
folders: Markdown specification and authored tests for A; Graphify output, GDD
model, records and run files for B. Record artifact bytes and effort separately
from subject-code scope. No new dependencies, public CLI, consumer execution,
backend rewrites, generated subject files, license changes or upstream writes.
Any necessary scope change stops the pair for a documented protocol revision;
do not silently relax the brief for one arm.

## Independent acceptance and architecture oracle

Before any implementation, an evaluator who will not implement either arm must
write and freeze an acceptance suite from the brief. Record its hash, author,
creation time and invocation in a private evaluator folder outside every agent's
accessible workspace. Do not commit hidden tests into either input tree or pass
their contents, names or failure hints through agent history. Agent-authored
acceptance tests are part of each process and are distinct from this oracle.
If the chosen runtime cannot enforce that separation, state the limitation and
use a separate evaluator host or OS account; a hidden filename alone is not
isolation. This proposal exposes the requirements; only test implementation and
fixtures are hidden, not undisclosed requirements.

The evaluator must cover these published obligations, with multiple fixtures:

1. Default page, offset, short tail, empty queue and out-of-range offset.
2. All argument rejection classes, with a storage spy confirming rejection
   precedes reads. Exercise the public and storage contracts separately.
3. Memory and SQLite ordering, including equal priorities; successive slices
   under a quiescent queue concatenate to the existing pending order.
4. Read-only inspection: queue, schedule and results unchanged; SQLite reopen
   preserves pending tasks and subsequent dequeue order.
5. Legacy calls and a custom storage subclass without an `enqueued_page`
   override remain usable. SQLite uses a bounded database query.
6. No public-layer backend access and no new reverse dependency from storage to
   API; changed files stay inside the allowlist.

Freeze a common architecture observer independently of the GDD model. Use the
same Python import observer for both patches, based on the pinned file boundary;
add evaluator inspection or a small fixed AST check for direct connection or
private-field access and embedded SQL, which import checks alone cannot detect.
Review observer positives against source and retain false positives. Do not
call Python import compliance proof of semantic architectural compliance.
Record baseline violations first and report *introduced* violations separately.
Seed an evaluator-only deliberately bad patch to confirm the observer catches
an API-layer database import and direct connection access; discard that fixture
before study runs. This calibration and oracle authoring are still pending.

The GDD arm may run `check-code` as part of its workflow. It receives its own
normal tool feedback; that is part of the treatment being tested. The common
post-run observer remains frozen, uses no arm-authored bindings, is never used
to repair one arm's graph, and assesses both arms identically. Neither arm sees
hidden oracle feedback before its first completion claim.

## Arms, environment and execution order

| Arm | Required process |
| --- | --- |
| A: baseline | Read the fixed brief; write a Markdown specification and public acceptance tests before production edits; implement and run available local tests. No GDD or Graphify. |
| B: GDD | Read the same brief; Graphify import of the pinned code; proposal and refined GDD 0.3 scope; readiness and workflow plan; implementation, `check-code`, and recorded evidence. |

Give both arms the same source snapshot, requirements, documentation, dependency
cache and test commands. Pin the GDD revision and installed Graphify version,
record their dependency versions and import configuration. Run Graphify within
B's measured setup; no free prebuilt graph or human-written refined model.
The evaluator's fixed observer is outside both agents' budgets. Give A and B
identical descriptions of the architecture rule and available upstream tests.

Use the **same actual agent runtime, model identifier, reasoning configuration,
OS, Python patch version, concurrency, network policy and stopping rules** in
both arms. Choose from the runtimes actually available at execution time; no
model, pricing or token-meter availability is assumed here. Use the current available Codex environment for both arms, subject to recording
its actual runtime/model and satisfying isolation. Claude is not a prerequisite:
the maintainer has no Claude subscription, and #10 reported authentication
failure rather than a successful Claude journey. The runtime being examined
separately in #10 is not a reason to mix runtimes between these arms.
Record any model alias whose underlying snapshot cannot be fixed.

Create fresh clones at the pin and fresh agent conversations for every run.
Disable shared memories and exclude previous patches, transcripts and study
results. Do not reuse an implementation agent from this proposal's preparation.
Run arms serially on the same machine to avoid resource contention. For the
pilot, draw and record AB or BA before starting. For the subsequent six-run
study use three paired repetitions, draw one balanced order from AB/BA/AB or
BA/AB/BA before execution, and save the draw. No adaptive ordering after results.
Report each run and the selected order; six runs of one task cannot establish a
general causal benefit or reliable statistical significance.

## Recommended caps and intervention policy

**Stage 0: qualification, at most 45 human minutes.** Pin Python and dependency
versions, provision Memory/SQLite, run existing relevant tests, record failures
and skips, freeze oracle and observer, and confirm artifact isolation. Proposed
upstream commands include `python runtests.py api storage sql_huey`; inspect the
actual environment and record optional backend skips. Check the full available
upstream suite as a regression reference. If qualification cannot establish a
usable baseline within the cap, stop and reconsider TinyDB; do not substitute
repositories during a pair. No comparative outcomes exist at this stage.

**Stage 1: feasibility pilot, one run per arm.** Recommend a hard 90-minute
wall-clock cap and 20 human minutes per run, plus 20 human minutes for paired
review. Start the clock at the agent's first access to task materials; include
planning, Graphify/GDD setup, tooling retries and all self-directed rework.
Record setup, implementation and verification time separately where measurable.
Use the same proposed 150,000 model-token cap per arm only if the runtime exposes
and can enforce the same accounting; otherwise explicitly choose wall-clock as
the enforceable cap and report token totals as unavailable. Do not claim equal
token budgets from wall-clock alone. No paid service provisioning is authorized
by this proposal. Agree any monetary ceiling after identifying actual billing.

**Stage 2: full comparison, optional after pilot review.** Three *additional*
fresh runs per arm, 90 minutes each, up to nine agent hours and 120 human minutes
for run interventions, with a separate agreed review allowance. Pilot runs do
not count toward the six if their protocol changes. Publish the protocol change
and keep failed pilot attempts visible. Do not start Stage 2 automatically.

Only a predeclared infrastructure repair may interrupt a run, such as a missing
locked dependency or runtime crash. Log the exact action, elapsed time and
whether context or files changed; keep the original failure. A rerun is a new
attempt, not replacement evidence. Requirement clarification must be written
once and supplied equally to both arms, with the comparison paused. No human
implementation help, selective extra hints or extra budget after a preferred
arm fails. A timeout is an outcome, with the last artifact state retained.

At the first explicit "done" claim, freeze and hash the patch and artifacts,
then run the hidden oracle and observer. For a bounded rework measure, permit
at most one further 15-minute repair round, inside the original cap, with the
same requirement-level failure summary supplied by the evaluator. Keep exact
hidden test code private. Score the first claim and final state separately.
No first claim means initial acceptance is unavailable, not an invented zero;
report timeout and final-state acceptance separately.

## Results record and interpretation

Create one JSONL run record per attempt, with null plus a reason for unavailable
measurements. Store detailed command/event logs separately; hash and reference
them. Required fields:

| Group | Fields |
| --- | --- |
| Identity | Protocol hash, subject commit, arm, repetition, order, attempt ID, agent/runtime/model identities, Python/OS, tool versions and dependency lock hash. |
| Inputs | Brief/oracle/observer hashes, prompt hash, allowed paths, actual limits, context/memory isolation method. |
| Outcomes | First-claim and final patch hashes; passed/failed/skipped oracle cases; introduced architecture findings and adjudication; scope violations; upstream test failures/skips; timeout or infrastructure failure. |
| Effort | Setup/implementation/verification/total wall seconds; all input/output/cached tokens when exposed, otherwise null; actual metered cost and currency when available; reviewer/intervention minutes; fix iterations before first claim and after evaluator feedback. |
| Provenance | Commands with exit codes; intervention reasons/actions; artifact/transcript paths and hashes; limitations or missing data. |

For reviewer confidence, randomize patch order and conceal arm labels and method
artifacts while reviewing code if possible. Ask the same questions for each
patch: correctness confidence (1–5), boundary clarity (1–5), change readability
(1–5), and willingness to merge (yes/no/needs changes), each with one brief
reason. Record reviewer identity and unblinding; recognizable generated styles
may defeat blinding. Method artifacts can be assessed separately after scoring.

Report all runs in a table, including failures and actual denominators. Keep
acceptance, architecture, scope and cost as separate outcomes; do not invent a
weighted winner score. For Stage 1 recommend proceeding only if both processes
can be executed and measured without protocol-breaking help. Mixed or negative
results are useful. Do not require GDD to win as an experiment acceptance rule.

Residual limits include one selected Python task, experimenter-chosen API,
researcher knowledge, possible model nondeterminism, incomplete backend tests,
observer coverage, human reviewer bias, same-machine history leakage and GDD
setup costs that may amortize differently over a longer project. This task tests
pagination and a narrow architecture boundary, not all of software planning.

## Decisions to take with the maintainer

1. **Subject and task:** approve pinned Huey and the pagination brief above.
   Recommendation: yes; reserve TinyDB only if qualification fails.
2. **First execution boundary:** approve qualification plus two-run feasibility
   pilot, not the full six-run study yet. Recommendation: Stage 0 + Stage 1.
3. **People and runtime:** name an independent evaluator/reviewer and select the
   same available runtime/model for both arms. Recommendation: use the current Codex
   environment for both arms if it provides enforceable workspace isolation and
   usage records; do not
   substitute a simulated second runtime or assume hidden tests are isolated.
4. **Budget:** approve the proposed wall/human caps and either measurable token
   limits or explicit unavailable token accounting. Set any monetary ceiling
   from actual billing before execution. Recommendation: no new paid services.

After those decisions, freeze the protocol and execute locally. Preserve compact
results under `evidence/external-trial-<date>.md` and store bulky transcripts in a
separately approved archive with hashes. Update the README only with observed
results and limitations. Keep #9 open until its executed-evidence deliverables
exist; this document completes preparation, not the external trial itself.
