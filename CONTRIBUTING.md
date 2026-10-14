# Contributing

Contributions are welcome: documentation, reproducible bug reports, conformance
cases, viewer improvements, adapter profiles and proposals for the method or
standard. You do not need to build a whole application to help.

## Choose a contribution

Search [existing issues](https://github.com/mikkelkrogsholm/gdd/issues) first.
Use the bug-report form for unexpected behavior, or the proposal form to explain
a workflow that GDD should support. Questions and small documentation fixes are
welcome too. For a larger feature or standard change, open an issue before
implementing it so we can agree on scope and compatibility.

Start with [README.md](README.md) and [REPOSITORY-PLAN.md](REPOSITORY-PLAN.md).
Keep changes bounded to a concrete user journey, validator defect or documented
contract. A small bug report should identify the input, command, actual result
and expected behavior; preserve diagnostic codes and include a minimal fixture
when possible. Do not include credentials or private source material.

Install the source using [INSTALL.md](INSTALL.md). The repository prose is
English, including comments, examples, generated visuals and evidence summaries.
Proper names, identifiers and intentional Unicode test data keep their technical
meaning. Prefer Markdown, JSON, YAML, JSONL, CSV or HTML for new deliverables.

## Make a pull request

Fork the repository on GitHub, then clone your fork. Replace `YOUR-USERNAME`
below with your account name:

```sh
git clone https://github.com/YOUR-USERNAME/gdd.git
cd gdd
git remote add upstream https://github.com/mikkelkrogsholm/gdd.git
git switch -c contribution/my-change
uv sync --locked --dev --python 3.12
```

Make one focused change, run the relevant local checks, and commit it with a
message describing the result. Push your branch to your fork and open a pull
request against this repository's `main` branch. Use the PR template to describe
the problem, behavior after the change, actual checks and any revision/evidence
impact. Link the issue when one exists.

Small fixes can go directly to a PR. Add a regression case for a behavior change;
documentation-only changes need link/command checks rather than new unit tests.
Include a meaningful negative or UNKNOWN case when changing a validator rule.
Do not add GitHub CI as part of your contribution: checks are intentionally local.

Keep discussion respectful and concrete. The maintainer reviews contributions;
review may request a smaller scope, clarification or more evidence. No CLA or
special commit-signing requirement is imposed. AI-assisted contributions are
welcome; contributors remain responsible for understanding and verifying them.

## Check a change

Verification runs locally; GitHub Actions CI is intentionally absent. For an
ordinary change, run the checks relevant to the affected behavior. For release
verification, use the full set below from the repository root with a fresh
output directory so new verification does not overwrite historical evidence:

```sh
uv sync --locked --dev --python 3.12
GDD_CHECK_DIR="$(mktemp -d)"
uv run --no-sync python scripts/sync_skill.py --check
uv run --no-sync pytest -q
uv run --no-sync python conformance/run.py --output "$GDD_CHECK_DIR/standard.json"
uv run --no-sync python conformance/run_graphify.py --output "$GDD_CHECK_DIR/graphify.json"
uv run --no-sync python conformance/run_visualization.py --output "$GDD_CHECK_DIR/visualization.json"
uv run --no-sync python conformance/run_workflow.py --output "$GDD_CHECK_DIR/workflow.json"
uv run --no-sync python conformance/run_design.py --output "$GDD_CHECK_DIR/design.json"
uv build --wheel --out-dir "$GDD_CHECK_DIR/dist"
uv run --no-sync python conformance/verify_readiness_wheel.py "$GDD_CHECK_DIR"/dist/*.whl --output "$GDD_CHECK_DIR/installed-wheel.json"
```

For viewer interactions or browser behavior, also run the optional browser
recipe in [INSTALL.md](INSTALL.md). Artifact-byte verification and browser
interaction verification establish different facts. The recorded local core
checks cover Python 3.11 and 3.12 on macOS. Identify the runtime and environment
of new executions explicitly.

Keep tests focused on observable behavior and meaningful failure controls.
Use FAIL for a demonstrated violation and UNKNOWN for missing or unsupported
evidence. A passing schema check does not establish implementation quality,
operational authority or acceptance of a change.

## Preserve revisions and evidence

Changing model or normative asset bytes changes their revisions. Update current
snapshots and record references, regenerate affected derived artifacts and run
the relevant conformance checks. Keep original execution evidence tied to its
original Git state. Mark translations and reconstructed captures explicitly;
do not present them as new executions. [evidence/README.md](evidence/README.md)
describes the provenance policy.

The standalone skill contains synchronized reference copies. After changing
authoritative guidance, run `uv run --no-sync python scripts/sync_skill.py`,
review the resulting skill changes, then rerun `--check`. Keep the skill's
relative links self-contained so a copied folder works outside the checkout.

## Versions and releases

Explain the concrete problem, resulting behavior and validation in the pull
request. Include reproducible commands and material limits; a screenshot or
worker's PASS statement alone is insufficient evidence. Keep unrelated user
changes and historical fixtures intact.

Package, standard and viewer versions are separate. Update package metadata,
the lockfile and runtime version references together for a package release.
Change a standard or renderer version only when that contract changes; preserve
explicit legacy compatibility. Record the resulting scope in
[CHANGELOG.md](CHANGELOG.md). Describe user-visible changes in the Unreleased
section; the maintainer assigns a release version. Build and check the exact wheel
before proposing publication. [RELEASING.md](RELEASING.md) defines version tags,
release checks and publication. Run artifact verification locally.

The maintainer selected the [MIT License](LICENSE) for GDD's code and associated
documentation. Contributions are accepted under that license; preserve existing
third-party notices and provenance. Registry publication requires a separate
maintainer decision.
