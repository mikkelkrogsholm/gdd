# Issue triage and first delivery boundary

Defined October 20, 2026 from the maintainer's request to triage, set a goal and implement the accepted first work package.
GitHub tracking: [milestone 1](https://github.com/mikkelkrogsholm/gdd/milestone/1).
The preceding [issue review](2026-10-20-issue-review.md) retains its actual reproductions.

## Outcome and acceptance

Stabilize the current CLI and improve the first planning and existing-code journeys:

- Fix confirmed interpreter-dependent parser depth and symlink-loop behavior (#1/#2).
- Improve precise readiness diagnostics (#4) and current-format Graphify proposals with explicit legacy selection (#7).
- Correct stale version guidance (#12), provide proportionate local checks with explicit isolated interpreter coverage (#16), and deliver the planning tutorial slice of #11.
- Verify the combined result on installed Python 3.11–3.14, relevant named conformance suites, current offline artifacts and a wheel installed outside the checkout.

Keep draft GDD 0.3, explicit 0.2 semantics, original model/normative bytes, historical executions and the local-only/no-GitHub-Actions policy. No new authoring language, general drift checker, model-diff format, external study, runtime installation or package publication is part of this delivery.

Contributions and current discussions remain visible. Triage does not assign another contributor or close an issue before its accepted outcome is available. The tutorial is intentionally planning-only; issue 11's repository cleanup and full implementation/evidence walkthrough remain separate.

## All 19 dispositions

| Issue | Priority | Disposition | Accepted scope or follow-up |
| --- | --- | --- | --- |
| [#1](https://github.com/mikkelkrogsholm/gdd/issues/1) | high | current | Fix confirmed interpreter-dependent depth handling with an explicit consumer limit and boundary tests; preserve UNKNOWN for unsupported depth and historical execution identities. |
| [#2](https://github.com/mikkelkrogsholm/gdd/issues/2) | high | current | Fix confirmed symlink-loop classification across supported interpreters, preserving missing-file and containment semantics. Do not make every path strict indiscriminately. |
| [#3](https://github.com/mikkelkrogsholm/gdd/issues/3) | high | design | Next product milestone: static Python imports against explicit forbid constraints and current bindings. Define observation output versus trusted revision-bound implementation evidence first; the current report schema does not admit an arbitrary new profile. |
| [#4](https://github.com/mikkelkrogsholm/gdd/issues/4) | normal | current | Accepted as a bounded diagnostic improvement: sorted missing-field messages and pointers to the nearest existing location; preserve result/code semantics and the report format. An interested contributor has been noted; no open PR existed at triage. No contributor assignment is implied. |
| [#5](https://github.com/mikkelkrogsholm/gdd/issues/5) | normal | design | First improve diagnostics in #4. Assess a read-only explain/text projection before introducing another command, stable envelope, progress metric or independent readiness logic. |
| [#6](https://github.com/mikkelkrogsholm/gdd/issues/6) | normal | design | Start with exact added/removed/changed stable IDs and separate byte revisions from semantic changes. Weakening indicators are advisory; two snapshots cannot prove historical ID reuse or semantic weakening of prose. Incomplete inputs must not fabricate deletions. |
| [#7](https://github.com/mikkelkrogsholm/gdd/issues/7) | high | current | Default to model-valid, unready gdd/0.3 proposal candidates, with explicit byte-compatible gdd/0.2 selection. Invent no boundary rationales, interfaces, postconditions, acceptance or authority. |
| [#8](https://github.com/mikkelkrogsholm/gdd/issues/8) | normal | deferred | Do not implement a new authoring language/compiler in this milestone. First measure authoring friction in the external trial; avoid automatic risk rationales and identity churn. |
| [#9](https://github.com/mikkelkrogsholm/gdd/issues/9) | high | next | Prepare a bounded external pilot early. A new generic checker is not a prerequisite: a fixed independent observer can assess both arms. Agree the subject/task/metrics before executing a larger study. |
| [#10](https://github.com/mikkelkrogsholm/gdd/issues/10) | normal | next | Verify actual installed-skill journeys in a second runtime and record interventions. No runtime coverage claim before execution; no new runtime installation is authorized by this triage alone. |
| [#11](https://github.com/mikkelkrogsholm/gdd/issues/11) | normal | partial | This milestone includes only a small executable planning tutorial: brief, draft, explicit refinement, readiness, view and context. Moving historical notes and simplifying the rest of the documentation remain follow-up work. Five minutes/40 lines are not correctness gates for a full evidence journey. |
| [#12](https://github.com/mikkelkrogsholm/gdd/issues/12) | normal | current | Remove the stale package version from the adapter introduction and synchronize the skill. Avoid unnecessary mutable version prose and tests coupled to historical numbers. |
| [#13](https://github.com/mikkelkrogsholm/gdd/issues/13) | normal | deferred | Keep recoverable evidence curation as follow-up. Inventory indirect skill-test citation dependencies. Removing files from the latest tree does not shrink a normal full-history clone. Change browser output defaults separately when useful. |
| [#14](https://github.com/mikkelkrogsholm/gdd/issues/14) | normal | deferred | Reorganize bounded test areas when touched; preserve assertions, parametrization and finding provenance. Equal test counts alone do not prove unchanged coverage. |
| [#15](https://github.com/mikkelkrogsholm/gdd/issues/15) | normal | design | Extend existing RELEASING policy with tested Python coverage, legacy lifecycle and 1.0 criteria. Do not silently drop explicit 0.2 support or force unrelated envelope versions to move together. |
| [#16](https://github.com/mikkelkrogsholm/gdd/issues/16) | high | current | Provide one small local-only entry point with proportionate quick/default/release checks and explicit isolated Python matrix execution. Fresh logs, clear failures and actual runtime identity; no GitHub Actions or mandatory browser/matrix run on ordinary edits. |
| [#17](https://github.com/mikkelkrogsholm/gdd/issues/17) | normal | design | Publication remains a separate maintainer decision after correctness repairs and artifact verification. Preserve the established distribution name unless a concrete issue justifies a rename; verify name availability and sdist installation at publication time. |
| [#18](https://github.com/mikkelkrogsholm/gdd/issues/18) | normal | deferred | Optional enabling refactor when adding commands. Preserve command-specific output and exclusive creation; arbitrary line-count and registration-count targets are not product acceptance gates. |
| [#19](https://github.com/mikkelkrogsholm/gdd/issues/19) | normal | tracker | Use this issue as the roadmap index. Current milestone: #1 #2 #4 #7 #12 #16 plus planning tutorial only from #11. Next: scoped #3 with early #9/#10; design review for #5/#6/#15/#17; defer #8/#13/#14/#18. The whole backlog is not a release prerequisite. |

## Ownership and integration

- Isolated portability worker: shared loader/path handling, focused regressions and informative consumer-limit guidance.
- Isolated Graphify worker: candidate format selection, explicit legacy compatibility and adapter guidance.
- Isolated readiness/tutorial worker: precise diagnostics and an executable small planning example.
- Lead: GitHub triage, local check entry, coordinated CLI path use, documentation and skill synchronization, integrated verification and final delivery.

The main agent integrates focused worker commits and verifies the combined checkout. Completed local verification and its actual source identities are recorded separately; this plan does not itself establish PASS.

