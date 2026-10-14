# Loop 1 — ordinary and adversarial review

Status: verified after fixes. A fresh baseline yielded 172 passing pytest tests. Four parallel read-only reviews covered model/readiness/planning, authorization/records, Graphify, and the viewer; the lead reviewed CLI, packaging, and cross-contracts. All implementation changes in checkout are made by the lead. Workers have only prepared reviewable diffs in isolated temporary copies.

19 finding groups are recorded in [loops.json](loops.json). They cover false passing results, false negative results, uncontrolled input errors, information fidelity, and concrete navigation. Each finding requires a reproduction/negative regression and relevant positive controls before closure. Speculative large-graph performance, new product features, and full external-format certification were not added to the task.

The lead independently reproduced duplicate `--scope` through the context schema, an initial manifest symlink loop through the public API, and no-replace operations on local files, empty directories, and symlinks. Worker regressions are run first against the old code when it can be isolated, and then against the integrated code. Browser checks use `file://`, offline Chromium, and fresh fixtures; output is saved separately from the history of previous phases.

Preliminary evidence: [baseline](baseline-tests.txt), [CLI before fix](loop-01-cli-before.txt), [manifest symlink before fix](loop-01-manifest-symlink-before.json), [viewer before fix](loop-01-viewer-before.txt). These failure outputs are deliberately retained as reproductions and are not the final status.

Exclusive, atomic bundle publication uses the platform's no-replace primitive. The local macOS SDK `sys/stdio.h` specifies `RENAME_EXCL=0x00000004`; [Apple describes exclusive rename](https://developer.apple.com/documentation/foundation/urlresourcevalues/volumesupportsexclusiverenaming). The Linux variant follows [renameat2/RENAME_NOREPLACE](https://man7.org/linux/man-pages/man2/rename.2.html). There is no fallback to an operation that replaces existing output.

Integrated result: 310 pytest tests pass; 58/58 standard cases, 18/18 Graphify cases, and 16/16 renderer cases match. 20/20 offline Chromium checks pass without script/console errors or external requests. The 19 finding groups were fixed with regressions in `tests/test_review_*.py` and the new browser cases. A fresh loop 2 has started; it has not yet been documented that no further findings exist.
