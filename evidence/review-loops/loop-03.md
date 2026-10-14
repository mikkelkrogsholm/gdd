# Loop 3 — a different perspective and concrete boundary cases

Status: verified after fixes. The two model and protocol reviewers switched areas. Graphify and the viewer were reviewed again, and the lead examined CLI, file paths, numeric capacity, and packaging. Ten finding groups, R031–R040, are recorded in [loops.json](loops.json).

The viewer's tablet row could be shorter than its panels. At 740×360 and 850×600, the inspector covered the graph/filters/list. A content-based minimum height and a bounded object list fix the overlap; the browser case checks five sizes and actual scenario, fold, and reset clicks. Renderer identity changed to 0.1.2, and the example file was regenerated.

Schema-valid model assets with NUL in their path now produce a report diagnostic before filesystem calls. A trusted readiness_review with project scope can cover a single screen. The standard's whole-project readiness includes custom nodes whose unknown readiness semantics could previously be omitted. Model-ID collisions in the selected slice are no longer incorrectly treated as unrelated record errors.

Revision-incompatible verified bindings cannot retire a current mapping. Current evidence scope must reference existing model IDs; an invalid receipt yields model FAIL and UNKNOWN for the affected check. Historical/null revisions are not rebound. Independent valid evidence retains its result, and extra evidence scope does not prove additional verification.

Graphify inputs at runtime limits could receive PASS from observation and then crash during serialization or be reported as a corrupt manifest when reloaded. Parser depth and finite JSON numbers outside runtime capacity were also incorrectly reported as invalid JSON. These limitations are handled as UNKNOWN with raw bytes for recovery. An explicit raw profile may also be used on a raw artifact even when the file is named observations.json.

Triage, without a confirmed defect: Logical write areas and declared workspace isolation are not a physical checkout/symlink/worktree audit; the tool has no verified code root. Empty models are valid and have no implementation targets to reject. An explicit host denial can veto. Approval expiry outside the standard's exception contract was not introduced as a new runtime feature. An invalid direct API path can raise the documented ValueError; model content, however, must be diagnosed in the report.

Negative reproductions are retained separately: [asset paths](loop-03-asset-before.txt), [bindings/evidence](loop-03-records-before.txt), [tablet layout](loop-03-landscape-before.json), [raw filename](loop-03-graphify-locator-before.json), and [numeric runtime](loop-03-numeric-runtime-before.json). They are not the final status. A patch backup from loop 1 was moved out of the active schema directory; the wheel check rejects .orig/.rej files.

Integrated result: **542 pytest tests pass**; **58/58** standard, **18/18** Graphify, and **16/16** renderer cases match. **21/21** browser checks pass without script/console errors or networking. All ten finding groups were fixed with regressions. Loop 4 has started as a fresh review of the complete solution after these changes.
