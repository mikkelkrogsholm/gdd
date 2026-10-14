# GDD viewer 0.1 — delivery and checks

Completed October 13, 2026. Package `gdd-validator 0.4.0`, GDD standard format 0.2. The goal was an interactive, read-only graph view with Graphify-like navigation that preserves GDD identity and traceability. The goal is met within the documented renderer profile.

## Delivered

- [Python renderer/API](../gdd_validator/visualization.py), [CLI](../gdd_validator/cli.py), and [validator integration](../gdd_validator/validator.py).
- Five packaged local [viewer assets](../gdd_validator/viewer/shell.html): HTML, CSS, and three JavaScript files. The generated file contains all assets and data.
- [Concert demo](../examples/concert.graph.html): 38 canonical nodes, 85 canonical relationships. The initial overview shows 18 nodes and 39 relationships; the scenario checkbox and manifest identify the remaining parts.
- [Documentation](../VIEWER.md), informative [payload schema](../schemas/view.schema.json) and [view schema](../schemas/view-state.schema.json), 16 [artifact cases](../conformance/visualization-cases.yaml), 29 new pytest tests, and a [browser harness](../conformance/run_browser.cjs).

No additional runtime/browser dependencies in the product package. The browser harness uses a separate Playwright runtime and actual local Chromium/Chrome.

## Actual evidence

| Check | Result | Output |
| --- | --- | --- |
| Integrated pytest tests | 172 passed | [tests.txt](visualization/tests.txt) |
| Original standard cases | 58/58 expectations match | [standard-conformance.json](visualization/standard-conformance.json) |
| Graphify adapter cases | 18/18 expectations match | [adapter-conformance.json](visualization/adapter-conformance.json) |
| Renderer artifact cases | 16/16 expectations match | [artifact-conformance.json](visualization/artifact-conformance.json) |
| Real offline browser checks | 17/17 passed, 0 script/console errors, 0 external requests | [browser-conformance.json](visualization/browser-conformance.json) |
| Final concert file against current model/renderer | PASS | [concert.report.json](visualization/concert.report.json) |
| Actually downloaded view manifest | Schema PASS | [browser.view-state.json](visualization/browser.view-state.json), [state-schema.json](visualization/state-schema.json) |
| Sdist and wheel | Built | [build.txt](visualization/build.txt) |
| Installed wheel outside checkout | Render + verify + readiness PASS; same HTML bytes as checkout; all five assets and both new schemas packaged | [wheel-smoke.json](visualization/wheel-smoke.json), [check code](visualization/wheel-smoke.source.py) |
| Agent-readable lint | 14 advisory AR002 near-duplicate findings; none in the new renderer module | [agent-lint.json](visualization/agent-lint.json) |

The browser test ran local Chrome/Chromium 155.0.8059.40 with `context.setOffline(true)`. It copied one HTML file to an otherwise empty directory and opened it through `file://`. The runtime was the available bundled Playwright installation; no browser was downloaded. The Python environment is the project's locked 3.12 environment. No external pages or model/pseudocode were executed.

Browser cases cover canvas and keyboard selection, directed bundles with identical parallel edges, opposite direction, self-loops, community overlap and repeated membership, the visibility manifest, scenario reveal as the first action, empty search, reset, scope boundary, zoom/pan/drag, manifest download, custom kinds, and an empty model. Hostile labels/pseudocode with script/HTML text and Unicode separators remain exact text. Both the inspector and search preserve `9007199254740993`.

[Desktop](visualization/concert-desktop.png), [dark theme](visualization/concert-dark.png), and [narrow screen](visualization/concert-mobile.png) were visually inspected. There is a keyboard alternative in the object list, focus indication, status announcements, and a responsive layout. This is not full WCAG certification.

## Review and fixes

Three read-only subagents independently reviewed the model/view contract, installed Graphify UI, and integration/test design. The lead implemented and verified the changes. No subagent wrote in checkout or created further agents.

Concrete review findings led to fixes for lazy position seeding, unique community metadata without edge loss, cumulative drag threshold and cancel, CRLF byte differences, deep/malformed payloads, missing input as UNKNOWN, and numeric precision in both the raw inspector and search. Valid JSON's literal U+2028/U+2029 was also distinguished from YAML quoted-string folding. Fragment and edge locators are now preserved explicitly. Positive and negative regressions are included in the current checks.

## Claim boundary and stopping status

Artifact PASS means one file matches the current model revision, its canonical records/ID partitions, and the installed `gdd-viewer-html/0.1` renderer byte for byte. It is not general third-party renderer certification, a producer signature, implementation evidence, or full browser compatibility. Actual interactions were tested separately in Chromium.

No raw observation vertices, evidence overlays, code generation, or model editing are implemented in this profile. Large graphs have no measured performance guarantee; spring layout is limited to 200 shapes, after which seed layout is used. Scope and filtering change only the view. The model format and earlier GDD/Graphify evidence are preserved.

The stopping boundary is reached: a usable offline view, API/CLI, conformance cases, browser verification, installed package, demo, and documentation. [visualization-assets.json](visualization-assets.json) records the relevant delivery hashes; manifests from older phases are history.
