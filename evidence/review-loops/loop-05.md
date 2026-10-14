# Loop 5 — reference identity, model contradictions, and error reporting

Status: verified after fixes. A fresh combined review after loop 4 found three bounded finding groups, R047–R049 in [loops.json](loops.json).

R047: The shared integrity check rejected record references to colliding model IDs for checks/evidence, but lacked the same check for supersedes. A used record with such a globally ambiguous reference could still support implementation PASS, authorization GRANTED, or readiness PASS. The fix addresses precisely the reference identity, without blanket transitive invalidation of otherwise independent records.

R048: An imports edge is an intended permitted direct import in SPEC. It therefore cannot also be forbidden by a declared architecture constraint. Direct and transitive prohibitions are checked in the intended model graph, including nonempty cyclic paths. A direct prohibition still permits an indirect path, and no complete allowlist is inferred. This is model consistency; actual implementation imports still require executed checks.

R049: A duplicate JSON key containing a lone surrogate could be interpolated unescaped into InputError before the parser's normal Unicode check ran. The resulting report could not be materialized as UTF-8. Graphify import therefore also failed to publish a raw bundle. Both parsers' diagnostics must escape the untrusted key; the input remains FAIL, and Graphify preserves original bytes for exact recovery.

The viewer area was clean: renderer 0.1.3 passed 23/23 offline browser checks and 41 targeted renderer/scope regressions. Long titles, six viewport sizes, empty communities, scenario scope, fold/reset, and keyboard selection were tested without scope expansion, panel overlap, script/console errors, or networking.

Other fresh checks covered 299 relevant records/authority/evidence/envelope tests, 83 independent model/tag/type/closure/source/parallel assertions, and 184 adapter/CLI regressions, 18/18 adapter cases, and 41 standalone integration checks. The three findings were reproduced separately from these positive controls. The lead continues to write all changes; workers prepare only temporary patches.

Integrated result: **777 pytest tests**, **58/58** standard, **18/18** adapter, **16/16** renderer cases, and **23/23** offline browser checks pass. No script/console errors or external requests. All three finding groups were fixed with regressions. A fresh loop 6 has started after the last fix.
