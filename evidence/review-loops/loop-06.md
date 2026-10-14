# Loop 6 — unambiguous identity, numbers, and integration plans

Status: verified after fixes for R050–R052. The lead integrated all changes; workers worked only in temporary copies.

R050: Model IDs are globally unique, but set-based membership lost multiplicity. A current check/evidence/binding could use a rule ID that also identified a source and still yield implementation PASS. A similarly ambiguous node in request scope could yield authorization GRANTED. Used references must resolve unambiguously. The fix is limited to concrete use and preserves independent negative results and valid disjoint scopes.

R051: Decimals such as minimum=1e-4000/maximum=0 or minimum=0.10000000000000001/maximum=0.1 lose the declared contradiction during float conversion. Input without a lossless decimal roundtrip must be visibly unsupported/UNKNOWN, rather than positively assessing an altered meaning. Ordinary representable values and actual supported contradiction tests are preserved.

R052: The integration plan checked only that rule IDs were covered, allowing a blocking rule with two required verification methods to use just one. Coverage must apply to (rule, method) pairs and current valid check references. The plan remains a declaration; executed implementation evidence is not required for this check.

Fresh read-only reviews: the viewer's 35 independent browser checks and 13 API/schema checks passed, as did 23/23 browser, 16/16 renderer cases, and 41 regression tests. Graphify/CLI/filesystem passed 199 targeted regressions, adversarial Unicode/UTF-8 raw recovery, competing exclusive publication, historical bundles, and Unicode proposal paths. The records area passed 120 new checks and 219 existing regressions, in addition to the R050 reproduction. The imports model was compared with an independent Warshall oracle on all 512 directed graphs with three vertices, direct/transitive: 9,216 forbid checks matched.

Integrated result: **940 pytest tests**, **58/58** standard, **18/18** adapter, **16/16** renderer cases, and **23/23** offline browser checks pass. No browser errors or external requests. R050 added 64 regressions (41 failed on baseline; 17 passed; six new helper tests were deselected before the helper existed). R051/R052 added 99 regressions (52 failed before the fix, 47 passed). Baseline outputs are deliberate negative reproductions; loop-06-tests.txt is the current combined result. A fresh loop 7 starts after the last fix.
