# Agent runtime trials

The GDD skill uses a portable `SKILL.md` entry and bundled references. That
format is a portability design; it does not establish successful execution on
every agent runtime.

| Runtime | Observed scope | Evidence |
| --- | --- | --- |
| Native Codex agents | Installed-wheel fresh-project and existing-code journeys, including bounded changes and controls | [Repository readiness](../evidence/repository-readiness.md) |
| Claude Code 2.1.186 | Project-local skill discovery verified; both actual journey attempts blocked by HTTP 401 before model/tool execution | [October 20 trial](../evidence/second-runtime-2026-10-20/README.md) |

Claude Code's local authentication status reported an existing first-party login.
Its model requests rejected that token. The maintainer subsequently clarified
that they have no Claude subscription available here. These are separate facts;
a locally cached login does not establish usable service access. No login
changes, subscription purchase, runtime installation or fallback paid route was
performed. Issue #10 remains open.

## Claude Code project-local setup

This setup was tested for **skill discovery only**, with the installed wheel
built from GDD commit `b5bfcce`. It is not a verified development journey.
After installing a wheel using [INSTALL](../INSTALL.md), enter a fresh project
and copy the installed asset:

```sh
mkdir -p .claude/skills
"$GDD_ENV_DIR/venv/bin/python" -I - <<'PY'
from importlib.resources import files
from pathlib import Path
import shutil
source = Path(str(files("gdd_validator").joinpath("skill")))
shutil.copytree(source, Path(".claude/skills/gdd"))
PY
```

Keep the entire folder, including `references/`. In Claude Code, start with
`/gdd` followed by the software brief and the authorized scope. For a bounded
noninteractive run, `--setting-sources project` retains project skill discovery
while excluding personal settings. `--setting-sources ''` hid the project skill
in the tested version. The runtime's initialization event listed `gdd` under
both `skills` and `slash_commands` after enabling project settings. This confirms
discovery, not that the model read the skill or completed its workflow.

The preserved [rerun harness](../evidence/second-runtime-2026-10-20/rerun.py)
creates an environment and two isolated projects outside the source checkout,
extracts the wheel's skill, copies explicit briefs, and records bounded runtime
calls only when `--run` is passed. It disables hooks, MCP servers and the
Agent/Task tools. Its prompts prohibit network calls, global configuration
changes and nested agents. These prompt boundaries are not an operating-system
sandbox. No runtime is installed by the harness.

## What remains to verify

Park #10 until the maintainer chooses an available second runtime with usable
service access. If Claude Code becomes the chosen available runtime, run the
prepared journeys and independently verify their artifacts. Do not interpret a
successful command exit or a runtime summary alone as passing the journey.

Both journeys must preserve sources, graph, derived view and scoped plan. The
existing-code journey must extract the actual local fixture through Graphify,
preserve raw round-trip bytes and map observations explicitly. Both must retain
fixed requirement-change red/green checks, an actual architecture violation and
repair, and honest current/unknown evidence. Any instruction repair must be
recorded and the affected journey rerun. A local synthetic application tests
portability; it is not the external-codebase value experiment in issue #9.
