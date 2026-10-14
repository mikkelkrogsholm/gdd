# Graphify adapter — completed checks, October 13, 2026

Delivered: Graphify adapter 0.1.0 in gdd-validator 0.3.0 for GDD standard format 0.2. The adapter loads two explicit 0.9.75 profiles, preserves raw bytes/metadata and every array record, validates revision/projection/direction, and can prepare a GDD sketch through an explicit mapping.

## Actual results

| Check | Result | Evidence |
| --- | --- | --- |
| Full pytest suite | 143 tests pass | [tests.txt](graphify/tests.txt) |
| Original standard cases | 58/58 expectations match; no expectations weakened | [standard-conformance.json](graphify/standard-conformance.json) |
| New Graphify adapter cases | 18/18 expectations match | [adapter-conformance.json](graphify/adapter-conformance.json) |
| Actual Graphify 0.9.75 AST and export | 3 nodes / 5 edges; original_links preserves 5; upstream simple export has 3 | [producer-evidence.json](../examples/graphify/producer-evidence.json) |
| Byte roundtrip | Recovered AST is byte-identical to the original fixture | [recovered.ast.json](graphify/recovered.ast.json), [report](graphify/roundtrip.report.json) |
| Markerless undirected export | Raw preservation/layout PASS, direction UNKNOWN | [undirected.report.json](graphify/undirected.report.json) |
| Unaccepted continuation | Valid GDD candidate with sketches, provenance, open questions, and unverified binding candidates | [mapping](../examples/graphify/mapping.json), [proposal](../examples/graphify/observations/proposal.json) |
| sdist and wheel | Build | [build.txt](graphify/build.txt) |
| Installed wheel outside the workspace | Import/validate/exact export/explicit proposal pass; nine schema assets; no Graphify runtime dependency | [wheel-smoke.json](graphify/wheel-smoke.json), [execution source](graphify/wheel-smoke.source.py) |

Producer fixtures were actually run through `extract_python`, `build_from_json`, and `to_json` in the installed Graphify 0.9.75. Input code, producer/dependency versions, relevant package source hashes, and output hashes are recorded. No LLM pipeline or business application was run. Code_revision is unknown; it is not replaced with the artifact hash.

## Parallel reviews and fixes

Three read-only subagents examined the producer format, observation/model contract, and conformance in parallel. They then reviewed the concrete changes. The lead wrote and integrated all files.

Review findings were fixed with regressions:

- Numeric rounding could connect two different producer IDs. The parser now checks the decimal value before reference resolution; precision outside the projection yields UNKNOWN with original bytes preserved.
- Version/revision claims under graph were overlooked. Known claims are now enumerated with actual JSON Pointers at both levels; unsupported or conflicting claims become UNKNOWN.
- `_src/_tgt` could contradict a directed graph's source/target. The conflict is rejected; marker restoration remains possible for undirected storage.
- Generic report output could overwrite input. CLI result files are now created exclusively, including protection against symlink/hardlink aliases.
- The projection's report text promised too much about caller metadata. Raw-derived fields are recomputed; caller claims are explicitly unverified inputs, not authenticated receipts. Coordinated changes to them provide no verified binding or implementation evidence.
- The raw path is fixed as graphify.raw.json in this tool version; alternative/escaping paths are rejected before reading, and escaping symlinks are rejected.

Identical multiedges, duplicate producer IDs, typed IDs, arrays with both links/edges, hyperedges/communities, opaque fields, JSON/Unicode errors, docs versus code, tamper, and no-clobber were also tested. A missing profile choice or missing input yields UNKNOWN rather than a guessed interpretation.

The agent-readable-code linter was run as advisory. [Output](graphify/agent-lint.json) shows short near-duplicates in diagnostic/schema fields; no runtime file exceeds 800 lines, and there are no AR004/AR006/AR008 findings. No additional generic abstraction layers were introduced to eliminate local similarity.

## Scope and limitations

Losslessness applies to **the received artifact**. The adapter cannot reconstruct relations deduplicated upstream or prove producer completeness. Unknown layouts/versions, ambiguous original direction, unknown code classification, and numeric precision outside the supported projection are visible UNKNOWN results.

`propose` creates a separate proposal with explicit rationale. It does not change existing accepted intent or create approvals, autonomous workflow gates, observed/checked bindings, or executed code evidence. Promotion takes place through the project's normal trusted mandate and model-change process. Import PASS is not a claim that the existing code satisfies GDD rules.

The six core schemas and 58 original expectations are preserved. The two old import conditions are now materialized with actual JSON layouts; a further 18 cases were added separately. New observation/mapping schemas are tool contracts outside the GDD core. The renderer and production app remain outside this task.

[GRAPHIFY.md](../GRAPHIFY.md) is the entry point for users/agents. [graphify-assets.json](graphify-assets.json) records the concrete delivery, fixture, and check bytes. Earlier 0.2.0 evidence and distributions are retained as history.
