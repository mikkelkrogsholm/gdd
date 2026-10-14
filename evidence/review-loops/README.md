# Review and adversarial loops

Completed October 13, 2026: **10 rounds**, **57 confirmed finding groups fixed with regressions**. The tenth fresh combined round found no new confirmed errors or unresolved candidates. The user's maximum of ten rounds has been completed.

Scope was the entire current GDD framework: standard contracts, loading/model/readiness/planning, records/authorization/evidence, Graphify, viewer, CLI/API, schemas, and packaging. Four parallel read-only subagents reviewed bounded areas; the lead reproduced, triaged, and integrated all changes. Worker patches were prepared in temporary copies. The workspace remains without Git; historical documents, evidence, and distributions are preserved.

| Round | New finding groups | Total tests | Result |
| --- | ---: | ---: | --- |
| 1 | 19 | 310 | Fixed and verified |
| 2 | 11 | 456 | Fixed and verified |
| 3 | 10 | 542 | Fixed and verified |
| 4 | 6 | 691 | Fixed and verified |
| 5 | 3 | 777 | Fixed and verified |
| 6 | 3 | 940 | Fixed and verified |
| 7 | 2 | 1,072 | Fixed and verified |
| 8 | 1 | 1,106 | Fixed and verified |
| 9 | 2 | 1,230 | Fixed and verified |
| 10 | 0 | 1,230 | Clean fresh round |

[loops.json](loops.json) records each finding, area, priority, fix status, and check output. The individual [loop notes](loop-10.md) describe causes, boundaries, and fresh checks; loop-01.md through loop-10.md cover the process. Before logs are deliberate negative reproductions, not the current status.

## Current verification

- **1,230 pytest tests pass**: [combined output](loop-09-tests.txt).
- **92/92 conformance expectations match**: [58 standard cases](loop-09-standard.json), [18 Graphify cases](loop-09-graphify.json), [16 renderer cases](loop-09-renderer.json).
- **23/23 offline browser checks pass**: [Chrome report](loop-09-browser/browser-conformance.json). Separate fresh browser checks in loop 10 also passed without script/console errors or networking.
- The last round changed no runtime, schema, test, or normative files. [121 file hashes](reviewed-state.json) bind the fresh round to the integrated code above; unchanged status is documented in the [final check](final-reviewed-state-proof.json). The passing combined checks from loop 9 are therefore reused together with the fresh area checks from [loop 10](loop-10.md).
- Version **0.4.1**, standard format **GDD 0.2**, Graphify importer **0.1.2**, renderer **0.1.3**. The concert demo's HTML bytes are unchanged after the final fixes.

The informative renderer case viewer.deep_payload now expects UNKNOWN at decoder capacity; it is a balanced JSON fixture. Known syntax/payload/asset contradiction cases remain FAIL. The original 58 normative case expectations are preserved. See [loop 9](loop-09.md).

## Distribution and installation

The current wheel is installed in a new environment outside the project directory. **17 CLI checks pass**, including render/verify/readiness, Graphify import/verify/byte-identical export/propose, the raw-filename profile, unsupported Graphify depth with recovery/proposal refusal, exact mixed numeric bounds, numeric/parser capacity, and an unknown primary model with a secondary record log. Generated HTML exactly matches the included demo.

[Installed-wheel check](final-wheel-smoke.json), [reproducible script](final-wheel-smoke.source.py), and [package/source match](final-package-source-match.json) document the actual wheel hash and byte identity of all 34 runtime, viewer, and schema files. The wheel and source archive are in ../../dist/. The final source-archive check is recorded in [sdist/source match](final-sdist-source-match.json).

The advisory [agent-readability lint](final-agent-lint.json) reports 20 AR002 hints about closely similar code fragments; these are not confirmed behavioral errors or a hard release gate. The review adds no application execution: host mandates, receipts, and evidence in the test material are synthetic. Supported profiles and their actual limitations are described in ../../VALIDATOR.md, ../../GRAPHIFY.md, and ../../VIEWER.md.
