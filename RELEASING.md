# Versions and releases

GDD has three independent version lines:

| Surface | Current version | Meaning |
| --- | --- | --- |
| Python package and CLI | 0.8.0 (unreleased) | Distributed tool behavior and public Python APIs |
| GDD standard | Draft 0.3; explicit 0.2 support | Document schemas and interpretation contracts |
| Viewer renderer | 0.3.0 | Generated HTML renderer; profile remains `gdd-viewer-html/0.1` |

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
