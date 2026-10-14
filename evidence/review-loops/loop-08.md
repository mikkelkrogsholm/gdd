# Loop 8 — parser capacity

Status: verified after the fix for R055.

R055: Both parser boundaries grouped RecursionError with known invalid JSON/YAML features. A valid document with 2,000 nested arrays in free-form annotation data therefore yielded YAML_FEATURE_FORBIDDEN/model FAIL, even though the cause was solely the runtime parser's capacity. The documented depth boundary and conservative result model require FORMAT_UNSUPPORTED/UNKNOWN. The fix changes the classification; it adds neither parser support nor a new arbitrary depth limit.

The model area passed 539 regressions and 162 independent Decimal-oracle cases for bounds and integer inhabitation, plus local ref/equality/non-mutation checks. The Graphify/CLI/filesystem area passed 383 targeted regressions and fresh integrity, identity, byte recovery, import audit, and exclusive output checks without new findings. R053's original five model reproductions were retested with the correct mathematical results. The lead's before check of valid JSON/YAML at depths 10 and 2,000 is saved separately.

The records/protocol area was clean: 248 fresh assertions (including a 144-case authority oracle) and 313 regressions passed. The viewer area was clean: 41 pytest tests, 23 browser checks, 35 independent browser checks, 13 API/schema checks, 16 numeric boundary checks, and 16 renderer cases passed. Four new numeric fidelity cases and actual Chrome checks of inspector/search confirmed preservation of stored types, canonical model, source bytes, and HTML values. No browser errors or networking.

R055 was fixed with separate RecursionError boundaries in the JSON and YAML parsers. The 34 new regressions had 17 FAIL/17 PASS before the fix and now pass; forbidden-feature and syntax errors remain FAIL. The lead's same positive/negative input matrix now yields PASS/exit0 at depth 10 and UNKNOWN/exit2 at depth 2,000.

Integrated result: **1,106 pytest tests**, **58/58** standard, **18/18** adapter, **16/16** renderer cases, and **23/23** offline browser checks pass. No browser errors or external requests. A fresh loop 9 starts after the last fix.
