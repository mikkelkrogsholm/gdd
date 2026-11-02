# Versions and releases

GDD has three independent version lines:

| Surface | Current version | Meaning |
| --- | --- | --- |
| Python package and CLI | 0.8.0 (unreleased) | Distributed tool behavior and public Python APIs |
| GDD standard | Draft 0.3; explicit 0.2 support | Document schemas and interpretation contracts |
| Viewer renderer | 0.4.0 | Generated HTML renderer; profile remains `gdd-viewer-html/0.1` |

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

## GDD 0.2 compatibility — adopted October 20, 2026

Keep full explicit GDD 0.2 validation and legacy replay throughout the package
0.x series. Preserve the 0.2 schemas, version dispatch and installed-wheel
legacy replay checks, including fixes needed to honor the supported semantics.
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

This is the single current compatibility overview. Update affected rows when
changing a contract or releasing the package. It describes the current checkout,
not a claim that every historical package supports these versions. A shared
number in separate rows does not couple their release schedules.

| Surface | Current identifier or version | Version changes when |
| --- | --- | --- |
| Package / CLI / public Python API | 0.8.0 (unreleased) | Distributed behavior or public API changes; package rules above apply |
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
| Viewer | Renderer 0.4.0; `gdd-viewer-html/0.1`; `gdd-view/0.3`, `gdd-view-state/0.3` | Renderer implementation, verification profile or serialized projection changes independently |

## Python compatibility — adopted October 20, 2026

Keep package installation metadata at `requires-python = ">=3.11"`.
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

## Prepare the release locally

1. Choose the package version. Update `pyproject.toml`,
   `gdd_validator/__init__.py`, current package-version documentation and `uv.lock`
   together. Standard/viewer versions change only when their own contracts change.
2. Date the changelog entry and describe scope, compatibility and known limits.
   Leave an Unreleased section for subsequent contributions.
3. Synchronize the standalone skill with `uv run python scripts/sync_skill.py`
   when its authoritative references change. Review generated copies and preserve
   old execution identities rather than rebinding them to this release.
4. Run the full local checks in [CONTRIBUTING.md](CONTRIBUTING.md). Run the optional
   [browser checks](INSTALL.md#optional-browser-checks) when viewer/browser behavior
   changes. Record actual commands, environment and results; retain relevant
   raw reports in a fresh directory. GitHub Actions CI remains absent.
   `uv run --no-sync python scripts/check.py --release` collects the release
   checks in one fresh directory. Use `--matrix` explicitly for the installed
   interpreter lines maintained in that script; it does not download Python.
5. Commit the reviewed changes. Require a clean checkout. Build the wheel from
   that exact commit into a fresh directory and run the installed-wheel verifier:

```sh
GDD_RELEASE_DIR="$(mktemp -d)"
uv build --wheel --out-dir "$GDD_RELEASE_DIR/dist"
uv run --no-sync python conformance/verify_readiness_wheel.py \
  "$GDD_RELEASE_DIR"/dist/*.whl --output "$GDD_RELEASE_DIR/installed-wheel.json"
```

Confirm the package/module version matches the intended tag. Retain the built
wheel and its SHA-256 if distributing it; do not rebuild or replace a published
asset silently. Verification results from an older commit are historical evidence,
not proof that this exact wheel was checked.

## Tag and publish on GitHub

For a new package release, after the checks above, create an annotated tag on the verified
commit and push the branch and tag together:

```sh
GDD_TAG="v0.8.0"  # Set to the chosen package version; never reuse a published tag.
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
the exact verified wheel and its checksum when distributing a binary asset.
Check the published tag, release metadata and asset digest after upload.

GitHub source archives and an attached wheel are separate from a package-registry
release. Publishing to PyPI requires an explicit maintainer decision and its own
distribution verification. A GitHub release never implies PyPI availability.
