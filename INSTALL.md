# Install and start

The Python distribution is named **gdd-validator** and installs the **gdd**
command. The commands below install from this checkout or a locally built wheel;
they do not assume a package has been published to a registry. Package 0.7.0
supports Python 3.11 and later. Local verification covers Python 3.11 and 3.12
on macOS; no GitHub Actions CI is configured.
The current standard is draft GDD 0.3, with explicit GDD 0.2 compatibility;
the viewer renderer is 0.2.0.

## Install from source

From the repository root, with [uv](https://docs.astral.sh/uv/getting-started/installation/)
and Python 3.11 or 3.12 available:

```sh
uv sync --locked --dev --python 3.12
uv run --no-sync gdd --help
uv run --no-sync gdd validate examples/concert-0.3.gdd.yaml
```

`uv sync` creates a local `.venv` and installs the source plus locked test
dependencies. The example validation should exit 0. You can also use a normal
virtual environment and `python -m pip install -e .`; this installs the CLI and
runtime dependencies, while `uv sync --locked --dev` is the reproducible route
for the repository's development checks.

## Install a local wheel

Build a wheel into a fresh location from the repository root:

```sh
GDD_BUILD_DIR="$(mktemp -d)"
uv build --wheel --out-dir "$GDD_BUILD_DIR"
uv run --no-sync python conformance/verify_readiness_wheel.py "$GDD_BUILD_DIR"/*.whl --output "$GDD_BUILD_DIR/installed-wheel.json"
```

The verifier creates a fresh environment, installs the wheel, and runs outside
the checkout without `PYTHONPATH`. It compares source, console and module CLI
results for current and legacy models, replays the native legacy example,
checks safe initialization, and compares the complete installed asset inventory
against exact source bytes. The resulting JSON is evidence for that local run.

For your own installation, create an environment outside the checkout. Set
`GDD_WHEEL` to the absolute path of the wheel printed by the build command:

```sh
GDD_WHEEL="/absolute/path/to/gdd_validator-0.7.0-py3-none-any.whl"
GDD_ENV_DIR="$(mktemp -d)"
python3.12 -m venv "$GDD_ENV_DIR/venv"
"$GDD_ENV_DIR/venv/bin/python" -m pip install "$GDD_WHEEL"
cd "$GDD_ENV_DIR"
"$GDD_ENV_DIR/venv/bin/gdd" --help
"$GDD_ENV_DIR/venv/bin/python" -m gdd_validator --help
```

These shell examples use POSIX paths. On Windows the environment executables
are under `venv\Scripts` instead of `venv/bin`. Runtime schemas, viewer files,
starter templates and the standalone GDD skill are included in the wheel.
Running the CLI does not require an LLM account or agent service.

## Create a draft project

With an installed `gdd` command, choose a project directory that does not exist:

```sh
gdd init ./my-project --project-id project.my-project --title "My project"
gdd validate ./my-project/gdd.json
gdd validate ./my-project/gdd.json --profile readiness --scope action.main
gdd view ./my-project/gdd.json --output ./my-project/graph.html
```

Initialization creates `gdd.json`, `SOURCES.md` and `records.json`. It refuses
an existing directory and repeated initialization exits 1 without overwriting
files. The model check should exit 0. The readiness check should exit 1 because
the scaffold is a draft with an unresolved scope question. The view should exit
0 and opens as an offline HTML file. A valid graph is separate from readiness,
authorization and executed implementation evidence.

For validation reports, exit 0 means the requested checks passed; exit 1 means
a FAIL or DENIED result; exit 2 means unresolved or unsupported assessment.
Read the JSON checks and their dimensions, particularly when FAIL and UNKNOWN
occur together. [VALIDATOR.md](VALIDATOR.md) documents individual commands and
[GDD.md](GDD.md) explains the method.

## Install the standalone agent skill

[skills/gdd](skills/gdd/SKILL.md) is a self-contained skill folder. Keep
`SKILL.md` and its `references/` directory together. It can be used independently
of this checkout after installing the CLI.

Choose the skill destination supported by your agent runtime, then copy the
whole folder there. From the repository root, this creates a reviewable local copy;
it does not modify personal agent configuration:

```sh
GDD_SKILL_COPY="$(mktemp -d)/gdd"
cp -R skills/gdd "$GDD_SKILL_COPY"
```

From an installed wheel you can instead copy the included skill asset. This
example uses the environment created in the wheel-installation recipe above:

```sh
GDD_SKILL_COPY="$(mktemp -d)/gdd"
"$GDD_ENV_DIR/venv/bin/python" -I - "$GDD_SKILL_COPY" <<'PY'
from importlib.resources import files
from pathlib import Path
import shutil
import sys
source = Path(str(files("gdd_validator").joinpath("skill")))
shutil.copytree(source, Path(sys.argv[1]))
PY
```

Move the reviewed copy to the location documented by your runtime or point the
agent directly at its `SKILL.md`. Installation location and skill discovery
depend on the agent; the repository does not automatically edit those settings.
Give the agent a brief, the intended project location and the skill. Follow its
workflow to capture sources and questions, refine an implementable scope,
produce a graph view and verify a bounded change. Existing-code work starts
with the [Graphify adapter](GRAPHIFY.md), preserves original observations,
and requires explicit review before observations become accepted intent.

## Optional browser checks

The generated viewer requires no JavaScript installation. Its browser
conformance harness needs Node.js 24 or later, Playwright 1.64.1 and the corresponding
Chromium build. Install these test-only dependencies into a fresh location:

```sh
GDD_BROWSER_DIR="$(mktemp -d)"
export GDD_PYTHON="$PWD/.venv/bin/python"
export GDD_PLAYWRIGHT_MODULE="$GDD_BROWSER_DIR/node_modules/playwright"
export PLAYWRIGHT_BROWSERS_PATH="$GDD_BROWSER_DIR/browsers"
export GDD_BROWSER_OUTPUT="$GDD_BROWSER_DIR/results"
npm install --prefix "$GDD_BROWSER_DIR" --no-audit --no-fund --save-exact playwright@1.64.1
node "$GDD_PLAYWRIGHT_MODULE/cli.js" install chromium
node conformance/run_browser.cjs
```

Run these commands from the repository root after source installation.
On supported Linux systems use `install --with-deps chromium` to install
required OS packages as well; that step may need administrator access.
[Playwright's browser installation documentation](https://playwright.dev/docs/browsers)
describes those dependencies. `GDD_BROWSER_EXECUTABLE` can optionally select
an existing compatible Chromium executable. `GDD_PYTHON`,
`GDD_PLAYWRIGHT_MODULE`, `PLAYWRIGHT_BROWSERS_PATH` and `GDD_BROWSER_OUTPUT`
make the test environment explicit and keep reports outside tracked evidence.
[VIEWER.md](VIEWER.md) explains what the browser checks assess.

Run the checks locally using [CONTRIBUTING.md](CONTRIBUTING.md). Core tests,
conformance suites, installed-wheel verification and optional Chromium checks
are available separately so you can select the checks relevant to your change.
