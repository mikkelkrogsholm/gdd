# Loop 9 — unknown primary project identity

Status: verified after fixes for R056–R057.

R056: The secondary record log's project_id was compared with None when the primary model could not be loaded. A missing model, unknown model version, or parser depth beyond capacity produced a fabricated RECORD_INVALID project mismatch and exit1. No such contradiction can be established without a known primary identity. Project IDs must be compared only when a gdd/0.2 model has actually loaded; other known primary/secondary errors must still be diagnosed, and a real known mismatch remains FAIL. The lead's six before reproductions with/without activity are saved separately.

R057: A valid model near the parser's depth capacity can still exceed the renderer's JSON serialization capacity. gdd view produced an uncontrolled RecursionError/traceback while building inspector text; the depth boundary varies with the call stack/runtime. Refusal must be controlled FORMAT_UNSUPPORTED without output, and a corresponding lack of capacity during artifact verification is UNKNOWN rather than known corruption. Actual syntax/payload/asset contradiction tests must still yield FAIL. No arbitrary new depth limit or parser extension is required.

The model area was clean: 573 regressions and 164 fresh probes for exact numbers, refs, unchanged canonical data/snapshots, scoped closure, multiedges, method coverage, and parser capacity passed. The Graphify/CLI/filesystem area was clean: 417 regressions and fresh raw-recovery/JSON/YAML-capacity/locator/exclusive-output checks passed. The viewer's other 41 pytest tests and 23 browser checks passed in addition to the separate R057 reproduction. The protocol area finished with R056 as the only finding; 42 new secondary-input checks, 248 other assertions, and 347 existing focused tests passed.

R056 was integrated with a bounded primary-format guard; the 100 new regressions had 64 FAIL/36 PASS on baseline. R057 was integrated with shared guarded JSON serialization, reload preflight before opening output, and separate UNKNOWN for capacity stops during verification. The 24 new viewer regressions had 16 FAIL/8 PASS before the fix. Five normal pre/post artifacts are byte-identical and pass verification; renderer 0.1.3 is preserved.

The informative renderer case viewer.deep_payload and its regression now expect UNKNOWN: the fixture is a balanced JSON array of depth 2,000, where decoder capacity is reached before the view envelope can be assessed. Invalid JSON, known payload/asset manipulation, CRLF, and duplicate payload retain FAIL. The 58 normative standard expectations are unchanged. The lead's original CLI reproduction now yields exit2 without traceback or output file.

Integrated result: **1,230 pytest tests**, **58/58** standard, **18/18** adapter, **16/16** renderer cases, and **23/23** offline browser checks pass. No browser errors or external requests. A fresh loop 10 starts after the last fix and is the maximum agreed round.
