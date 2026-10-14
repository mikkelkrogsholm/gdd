# Revalidation of GDD validator 0.2.0

The user's repeated validator task was reconciled with the already completed implementation. All 60 files recorded in the [delivery manifest](../validator-assets.json) have unchanged byte hashes. Existing code and deliverables are preserved; this round adds fresh check output.

| Check | Actual result | Output |
| --- | --- | --- |
| Full pytest suite | 74 passed tests | [tests.txt](tests.txt) |
| The standard's conformance cases | 58/58 expectations match | [conformance.json](conformance.json) |
| CLI, action.reserve | readiness PASS, exit 0 | [backend.json](backend.json) |
| CLI, implementation without executed evidence | UNKNOWN, exit 2 | [implementation.json](implementation.json) |
| CLI, implementation without a trusted mandate | authorization UNRESOLVED, exit 2 | [authorization.json](authorization.json) |

Commands from the project root:

```sh
uv run pytest -q
uv run python conformance/run.py --output evidence/revalidation-2026-10-13/conformance.json
uv run gdd validate examples/concert.gdd.yaml --profile readiness --scope action.reserve
uv run gdd validate examples/concert.gdd.yaml --profile records
uv run gdd validate examples/concert.gdd.yaml --profile records --activity implementation --scope action.reserve
```

Two read-only subagents rechecked the previous concrete review findings in parallel. Model review confirmed edge selectors, metaschema before traversal, inert const/$ref, and the separation of records from model revision. Protocol review confirmed context/readiness, parent children, protection against unrelated supersession, parallel-plan declarations, and community scope. No reproducible remaining defect was found within the bounded revalidation. The lead verified the integrated test run and CLI results.

The previous build/installation check is preserved in the [validator review](../validator-review.md). Package and code hashes are unchanged. No new global installation or publication was performed.

[VALIDATOR.md](../../VALIDATOR.md) describes profiles and limitations. Matching conformance expectations also include expected FAIL/UNKNOWN results; they are not application evidence or full certification of all possible standard requirements. Production Graphify import and rendering remain UNKNOWN.
