# Portable CLI and first planning journey

This is local verification of the first bounded issue-triage milestone for the
unreleased development package 0.8.0. It does not publish a package or assign a
release tag. The [triage plan](../../research/2026-10-20-issue-triage.md) records
all 19 issue dispositions and the accepted scope.

## Delivered scope

- Explicit file-consumer nesting capacity and consistent symlink-loop errors
  across installed Python versions (#1/#2).
- Precise readiness messages with existing-location pointers (#4).
- Default GDD 0.3 Graphify proposal candidates and explicit byte-compatible
  GDD 0.2 selection, without inventing implementation decisions (#7).
- Current adapter/skill guidance (#12), a bounded executable planning tutorial
  from #11, and proportionate local verification with an explicit isolated
  interpreter matrix (#16).

## Execution identity and results

The [verification record](verification.json) identifies the clean source commit,
source file hashes, baseline and freshly built wheel digest. The [runner
summary](summary.json) records actual commands, interpreter versions, durations
and log paths. Original absolute paths identify the temporary execution
locations; copied receipts in this folder retain those claims unchanged.

The final matrix ran the full 1,521-test suite separately on Python 3.11.11,
3.12.12, 3.13.13 and 3.14.7. All passed. The named
[standard](standard.json), [Graphify](graphify.json),
[visualization](visualization.json), [workflow](workflow.json) and
[design](design.json) conformance runners also passed. The
[installed-wheel receipt](installed-wheel.json) records fresh installation and
CLI/skill verification outside the checkout. These runs are local to macOS;
no GitHub Actions workflow was added.

[Exact artifact verification](artifact-integrity.json) passed for all three
existing example HTML graphs, which remain byte-identical to the baseline.
No new interactive browser execution is claimed here. The earlier
[Cytoscape browser review](../viewer-cytoscape-0.3.0/README.md) remains tied to
its original execution. Existing standard, schema, example, archive and
historical execution bytes were preserved.

## Diagnosed attempts retained separately

The [infrastructure interruption](infrastructure-interruption.json) records an
incomplete parallel attempt that exhausted available disk. It establishes no
PASS. Identified owned scratch copies were removed, failed-only pytest scratch
retention was enabled, and the final matrix ran sequentially.

The [first capacity-boundary failure](capacity-boundary-test-failure/summary.json)
identified two older tests outside the newly explicit consumer limit. The
replacement cases cover the supported boundary, rejection immediately beyond it,
and the separate evaluator-recursion fallback.

The [proposal-capacity failure](proposal-capacity-test-failure/summary.json)
identified an interpreter-specific assumption that JSON serialization must fail
before schema validation at 2,000 levels. The final tests require controlled
rejection before observation I/O and separately simulate serializer exhaustion.
These failed runs have not been relabeled as successful executions. Independent
read-only review found no material defects in the integrated implementation or
the revised boundary/fallback coverage.

Only issue 11's planning tutorial slice is delivered. Code observation, model
diff, a new authoring language, external studies and registry publication remain
separate backlog work; this record makes no claim to complete them.
