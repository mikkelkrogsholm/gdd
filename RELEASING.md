# Versions and releases

GDD tracks these version lines independently.

| Surface | Current version | Meaning |
| --- | --- | --- |
| Rust crate and CLI | `gdd` 0.1.0 in Cargo.toml | Current source package and command-line tools |
| Tagged Python package and CLI | 0.8.0 | Released Python APIs and HTML generation in the [v0.8.0 checkout](https://github.com/mikkelkrogsholm/gdd/tree/0dbb10b8c193f55c9777405de3f4ae22ac2d0825) |
| GDD standard | Draft 0.3; explicit 0.2 support | Document schemas and interpretation contracts |
| Tagged HTML viewer renderer | 0.4.0 | Generated HTML renderer; profile remains `gdd-viewer-html/0.1` |
| Native viewer generator | 0.1.0 | Portable desktop artifact with profile `gdd-viewer-native/0.1` |

Git tags identify **package releases** and use `vMAJOR.MINOR.PATCH`, starting
with `v0.7.0`. A tag does not change the standard's version or certify it.
There are no retroactive release tags for earlier development commits.

Before 1.0, minor releases may include clearly announced public API changes;
patch releases are compatible fixes and documentation/packaging corrections.
Document breaking changes and migration steps. Preserve the semantics of
explicitly supported GDD formats; new interpretation requirements need a new
format version. Renderer changes have their own version and artifact checks.

Published tags are immutable. Never force-update or reuse a tag. A correction
to published release contents requires a new package version and tag. Contributors
use CHANGELOG's Unreleased section; the maintainer chooses the next version.

Integrations using the Rust source can follow the [Rust API migration guide](docs/rust-migration.md).
Existing Python releases and their verification evidence remain tied to their
original tags. The source cutover assigns no new public release tag.

## GDD 0.2 compatibility — adopted October 20, 2026

Keep full explicit GDD 0.2 validation and legacy replay throughout the package
0.x series. Preserve the 0.2 schemas, version dispatch and legacy replay checks,
including the installed native CLI controls and the tagged Python wheel checks.
New features need not be backported to the 0.2 format. Do not silently migrate
inputs or reduce support to loading only.

The support set for package 1.0 remains a separate maintainer decision. This
policy neither promises 0.2 support after 0.x nor schedules its removal. Any
later reduction requires an explicit decision, announced deprecation, migration
guidance preserving source provenance and a pinned historical replay route.

## Independent contract versions — adopted October 20, 2026

The package, graph standard, serialized tool contracts, analysis profiles and
viewer renderer retain independent versions. Change only the affected contract;
do not force unrelated formats to move together. Exact format and profile
identifiers select compatibility; the package version is not a substitute.

Documented machine-readable structures, status values, exit codes and public
Python APIs are public contracts. A change to required structure or documented
interpretation requires a new affected format or profile identifier. A fix that
restores the existing documented contract does not itself require a new format
identifier. New contracts must document their scope and compatibility rules.

Normative asset byte changes still change revision identity, even when the format
identifier stays appropriate. Preserve original historical digests and execution
claims. Never relabel old evidence as an execution of a new release.

### Compatibility inventory

This inventory identifies current source contracts and the tagged Python and
HTML implementations. Update the relevant row when its contract changes.

| Surface | Current identifier or version | Version changes when |
| --- | --- | --- |
| Rust crate / CLI | `gdd` 0.1.0 in Cargo.toml | Native tool behavior or Rust API changes |
| Tagged package / CLI / public Python API | 0.8.0 | Distributed behavior or public API changes; package rules above apply |
| Model and fragment | `gdd/0.3`, `gdd-fragment/0.3`; explicit `gdd/0.2`, `gdd-fragment/0.2` support | Required structure or interpretation changes |
| Extensions | `gdd-extension/0.2` | Extension contract changes |
| Product extension and advisory perspectives | `gdd.product` 0.1.0; `gdd-perspectives/0.1` | Exact hashed optional extension semantics or projection contract changes; supported on unchanged GDD 0.2/0.3 cores |
| Records, reports and snapshots | `gdd-records/0.2`, `gdd-report/0.2`, `gdd-snapshot/0.2` | The affected serialized contract changes |
| Context, parallel planning and import audit | `gdd-context/0.2`, `gdd-parallel-plan/0.2`, `gdd-import-audit/0.2` | The affected projection or planning contract changes |
| Workplan and workflow result | `gdd-workplan/0.1`, `gdd-workflow-result/0.1` | The affected workflow contract changes |
| Runs, events, status and dispatch | `gdd-run/0.1`, `gdd-run-event/0.1`, `gdd-run-status/0.1`, `gdd-dispatch/0.1` | The affected execution or dispatch contract changes |
| Host checks and code snapshots | `gdd-host-check/0.1`, `gdd-code-snapshot/0.1` | Host evidence or code identity contract changes |
| Project initialization | `gdd-init/0.1` | Initialization result contract changes |
| Project status, impact and comparison | `gdd-status/0.1`, `gdd-impact/0.1`, `gdd-model-diff/0.1` | The affected inspection contract changes |
| Code observations and Python observer | `gdd-code-observation/0.1`, `python-imports/0.1` | Observation structure or analysis interpretation changes independently |
| Boundaries and design review | `gdd-boundaries/0.3`, `gdd-design-review/0.3`, `gdd-design-review-status/0.3` | The affected design contract changes |
| Graphify observations and proposals | `gdd-observations/0.2`, `gdd-graphify-proposal/0.2` | The affected adapter output contract changes |
| Graphify input profiles | `graphify-0.9.75-ast`, `graphify-0.9.75-node-link` | A new source dialect or interpretation needs an explicit profile |
| Tagged Python viewer | Renderer 0.4.0; `gdd-viewer-html/0.1`; `gdd-view/0.3`, `gdd-view-state/0.3` | Renderer implementation, verification profile or serialized projection changes independently |
| Native viewer | Generator `gdd-viewer` 0.1.0, profile `gdd-viewer-native/0.1`, format `gdd-native-view/0.1` | Changes to the generator, interpretation profile and portable file contract are versioned independently. See the [artifact contract](VIEWER.md#share-a-native-view). |

## Python compatibility — adopted October 20, 2026

For the [Python distribution](https://github.com/mikkelkrogsholm/gdd/blob/0dbb10b8c193f55c9777405de3f4ae22ac2d0825/pyproject.toml), keep package
installation metadata at `requires-python = ">=3.11"`.
Installation eligibility does not establish verified support for every later
Python version or platform.

Document actual test coverage for each release: source revision, host platform,
exact interpreter patches, full versus focused test runs, conformance results
and installed-wheel verification. Retain the corresponding receipts. Focused
regression checks are not a claim that the full suite ran on that interpreter.

Before declaring a new Python minor line supported, its full local test suite,
relevant conformance runners and fresh installed-wheel verification must pass
on that interpreter. Record the results and update the supported-line inventory.
Missing interpreters or failed checks remain explicit. Verification runs locally;
GitHub Actions CI is not required.

The [tutorial verification](evidence/tutorial-0.1/README.md) records the current
historical baseline: a full suite on Python 3.12.12, focused checks on
3.11.11/3.12.12/3.13.13/3.14.7, conformance and corrected installed-wheel
verification. This is not four full-suite executions or full admission evidence
for every listed line. The receipts preserve the initial failed wheel attempt
and the successful corrected rerun separately. Adoption of this policy creates
no new execution claims.

## Compatibility decisions still under discussion

[The compatibility and 1.0 policy proposal](docs/proposals/version-policy.md)
records the adopted 0.2 lifecycle, independent contract version and Python
verification decisions. Measurable 1.0 criteria remain pending for issue #15.
Those proposed criteria do not change active policy or authorize a release.

## Verify the native distribution locally

From the checkout, choose a new evidence directory and run the following command.

```sh
cargo run --locked -p gdd-conformance -- local-check --release --output ../gdd-native-release
```

The driver runs the full local checks, including the nine named conformance
suites and the standalone Rust skill tests and asset checks. It then runs three
locked path installs from the selected checkout into `install/bin/` and
exercises the optimized binaries from a temporary recipient directory outside
the checkout. It packages and extracts `gdd`, `gdd-tutorial-add` and
`gdd-concert` independently. It runs the locked root tests, checks the named
end-to-end tutorial test with the extracted checker, and runs the ten fixed
requirements against both policies in the extracted concert app. Use
`--checkout PATH` before `local-check` to select the source
and fixtures. An explicit `CARGO_TARGET_DIR` supplies the build cache for
the checkout. The extracted sources build in a fresh directory shared across
their test runs. The concert tests build without debug symbols.

Keep the following artifacts in the output directory.

- `summary.json` records commands, working directories, timings, process
  outcomes, host, toolchain and the paths and SHA-256 identities of installed
  binaries and all three source packages. Each step has separate stdout and stderr logs.
- `gdd-<version>.crate` is the tested CLI and library source archive. Its path
  and SHA-256 are in `summary.json` under `source_package`. The
  `source-package-tests` logs capture the test run from its extracted sources.
- `gdd-tutorial-add-<version>.crate` contains the checker used by the tutorial.
  Its path and SHA-256 are under `tutorial_source_package`. The
  `tutorial-source-package-tests` logs capture the opt-in test run.
- `gdd-concert-<version>.crate` contains the reservation server, fixtures and
  fixed requirement tests. Its path and SHA-256 are under `concert_source_package`.
  The `concert-requirements-a` logs must show two passes and eight specific
  assertion mismatches (Cargo exit 101). The `concert-requirements-b` logs must
  show all ten cases passing (exit 0). Both steps record the real exit codes.
- `verify-install.json` records the installed CLI's 34 controls and the
  identities of the installed and source-built executables. The source-built
  executable comes from the workspace build's Cargo artifact report.
- `installed-viewer-help.stdout.txt` captures the installed desktop executable's
  headless loader and help check. Interactive desktop testing is a separate check.
- `workflow-demo/` contains the repaired quote project, ten-event journal and
  five Cargo test receipts. The replay checks the seven expected baseline
  failures and all 19 passing final tests before recording current acceptance.

### Repeat the packaged root and tutorial tests

From the checkout, unpack the retained archives into a new directory. For
offline verification with cached dependencies, export `CARGO_NET_OFFLINE=true`
before running the release check or repeating its tests.

```sh
GDD_SOURCE_TEST=$(mktemp -d)
(
  set -e
  tar -xzf ../gdd-native-release/gdd-[0-9]*.crate -C "$GDD_SOURCE_TEST"
  tar -xzf ../gdd-native-release/gdd-tutorial-add-*.crate -C "$GDD_SOURCE_TEST"
  cd "$GDD_SOURCE_TEST"/gdd-[0-9]*
  export CARGO_TARGET_DIR="$GDD_SOURCE_TEST/target"
  export CARGO_BUILD_BUILD_DIR="$CARGO_TARGET_DIR"
  set -- "$GDD_SOURCE_TEST"/gdd-tutorial-add-*
  export GDD_TUTORIAL_SOURCE="$1"
  cargo test --locked
  test_name=tests::attest_actual_checks_and_observe_drift
  cargo test --locked --example tutorial_evidence -- --ignored --exact "$test_name" \
    --format pretty --color never > "$GDD_SOURCE_TEST/tutorial.log"
  grep -F "test result: ok. 1 passed; 0 failed; 0 ignored;" "$GDD_SOURCE_TEST/tutorial.log"
)
```

To repeat the concert control, extract `gdd-concert-<version>.crate` into
a fresh directory and follow the two phase commands in its README. The
[concert requirement table](examples/concert-app/README.md#replay-the-requirement-change-in-rust)
lists the expected outcome of each case.

The package tests regenerate both concert model views and the greenfield
product view. The installed CLI verifier checks the concert application’s
gallery view against its model and normative sources in the matching checkout.

## Prepare the release locally

1. Have the maintainer select the release version and affected package surfaces.
   Update the corresponding Cargo manifests, lockfile and package documentation.
2. Date the changelog entry and describe the release scope, compatibility and
   known limits. Keep an Unreleased section for subsequent changes.
3. Regenerate the [standalone skill references](CONTRIBUTING.md#preserve-revisions-and-evidence)
   when their sources change. Review the generated copies and hashes.
4. From the reviewed commit and a clean checkout, run the
   [native distribution checks](#verify-the-native-distribution-locally) in a
   fresh directory. Retain its summary, receipts and exact package digests.
   Exercise the desktop interactions affected by viewer changes.

### Reproduce the Python release

Use the [pinned Python v0.8.0 worktree](INSTALL.md#install-from-source) for
the [wheel, release procedure and browser checks from that revision](https://github.com/mikkelkrogsholm/gdd/blob/0dbb10b8c193f55c9777405de3f4ae22ac2d0825/RELEASING.md).
The [migration source recovery instructions](docs/rust-migration.md#recover-the-python-sources)
identify the intermediate Python packaging and test scripts used during the port.

## Tag and publish on GitHub

For a new package release, after the checks above, create an annotated tag on the verified
commit and push the branch and tag together:

```sh
GDD_TAG="v${GDD_RELEASE_VERSION:?set the selected package version first}"
GDD_COMMIT="$(git rev-parse HEAD)"
git tag -a "$GDD_TAG" "$GDD_COMMIT" -m "GDD $GDD_TAG: draft standard 0.3; CLI and skill"
git push --atomic origin main "$GDD_TAG"
git ls-remote origin refs/heads/main "refs/tags/$GDD_TAG" "refs/tags/$GDD_TAG^{}"
```

Check that the remote tag's peeled commit matches the verified commit. Do not
re-run the tag command for an existing release. A pre-1.0 development release
is marked as a GitHub prerelease so its experimental status remains explicit.

Create the GitHub release from the **existing tag**. Include the package,
standard and renderer versions, user-visible changes, actual verification,
installation instructions, known limits and contribution links. Attach only
the exact verified artifacts chosen for this release and their checksums.
Check the published tag, release metadata and asset digest after upload.

GitHub source archives, explicitly attached artifacts and package-registry
releases are separate distributions. Registry publication requires an explicit
maintainer decision and its own distribution verification.
