# Loop 4 — revision integrity and native input boundaries

Status: verified after fixes. The reviewers switched areas again; Graphify was reviewed by the former viewer reviewer and the viewer by the former integration reviewer. All framework areas were examined through ordinary review and adversarial probes. Six finding groups, R041–R046, are recorded in [loops.json](loops.json).

R041: A `%TAG !! ...` directive could make the parser's token check mistake a custom resolved tag for a built-in tag. YAML validation must use the actually resolved tag on the composed node, still with JSON scalars and prohibitions on anchors/aliases/custom tags.

R042: A local integer interval containing no integers need not make an enclosing object/array type uninhabitable. An object property can be optional, an array can be empty, or a nullable parent can be null. Semantic validation must therefore use the context in which a concrete non-null value is required. Structural constraints and references are still checked everywhere.

R043: Correct global model FAIL was not enough to keep invalid used records out of positive dimensions. Reproductions covered self/cyclic supersession retiring a current rejection; binding targets with an unknown current ID; a model-ID collision on a used check/evidence/binding/approval; and unknown extra approval scope. The fix is limited to the records actually used. An unrelated model defect must not automatically override a valid host mandate or independent valid evidence. Malformed retirement cannot retire current negative receipts.

R044: The viewer API accepted a string as scope and split it into letters. With valid IDs a, b, and ab, `scope="ab"` gave PASS for a/b and omitted ab. Scope must be list[str] or None; incorrect types are rejected with ValueError before output is created. Negative and positive controls are in [viewer before fix](loop-04-viewer-before.txt) and test_review_viewer_loop4.py.

R045/R046: The desktop panel's minimum could be too low at 851×600, causing reset to extend beyond the panel and footer. A valid 500-character project title could also extend beyond the fixed header height. Both were reproduced in installed Chrome 155 and bundled Chromium 151. Normal desktop/tablet/mobile, filters, ID partitions, direction, exact text, CSP, and model data were otherwise rechecked without new concrete viewer findings.

Graphify review was clean: 184 existing adapter/CLI regressions passed. Fresh independent probes covered numeric/string identity, duplicate producer edge IDs, reversed direction, legacy versions, tamper, depth 256/257, numeric overflow, proposal claims, output exclusivity, and a relative locator through paths containing spaces.

The R043 check is consolidated in record_integrity.py and reused by readiness, authorization, and implementation. Six additional regression cases protect used readiness-review and question receipts against unknown scope or invalid check references. Unrelated record errors remain separate.

Integrated result: **691 pytest tests**, **58/58** standard, **18/18** adapter, **16/16** renderer cases, and **23/23** offline browser checks pass. No script/console errors or external requests. All six finding groups were fixed. The renderer is 0.1.3; the demo was regenerated. A fresh loop 5 has started.
