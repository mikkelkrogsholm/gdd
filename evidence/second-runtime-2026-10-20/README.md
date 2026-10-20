# Second-runtime trial: blocked before execution

Recorded October 20, 2026. This is a failed service-access trial, not completed
second-runtime coverage. Both requested journeys remain unverified; issue #10
must remain open.

## Inputs and installation

- Clean GDD source baseline: `b5bfcce` (package 0.8.0, unreleased).
- A fresh wheel was built from that baseline with `uv build --wheel` and
  installed outside the checkout in `/tmp/gdd-second-runtime-20261020/venv`.
- [Installation receipt](installation.json) records the wheel SHA-256, Python
  identity and every extracted skill-file hash. It applies to the baseline
  wheel, not later changes from the parallel `gdd diff` work.
- Already installed Claude Code: `2.1.186`; initialization selected
  `claude-opus-4-8[1m]`. No model inference succeeded.
- [Fresh brief](briefs/fresh.md): parcel-price library, remote surcharge change,
  fixed red/green tests and forbidden tariff-to-API dependency control.
- [Existing-code brief](briefs/existing.md): a transparent local inventory
  fixture, actual local Graphify extraction, explicit mapping, zero-quantity
  change and forbidden domain-to-service dependency control. This is not an
  external production-codebase experiment.

The original preparation and execution code is retained as
[executed-preparation.py](executed-preparation.py) and
[executed-runner.py](executed-runner.py), with its actual temporary paths. These
are historical records, not portable installation recommendations. Auth status
was captured as nonidentifying fields only; no credentials are included.

## Actual outcomes

The first fresh-project invocation used `--setting-sources ''`. Claude returned
`Unknown command: /gdd` without model execution, despite a zero process exit.
Those [events](receipts/fresh/disabled-project-source.runtime.jsonl) and
[invocation](receipts/fresh/disabled-project-source.invocation.json) are retained.
This was a harness configuration error; enabling project settings was a targeted
setup correction, not a successful model journey.

With `--setting-sources project`, both initializations listed `gdd` in `skills`
and `slash_commands`. Their tool lists excluded Agent and Task, no MCP servers
were connected, no plugins were loaded, and hooks were explicitly disabled.
The runtime listed built-in agent definitions, but no agent tool was available
and none was invoked.

Both actual journey invocations then failed with HTTP 401:
`OAuth access token is invalid`. The runtime made internal authentication
retries; the operator did not retry the failed model requests, repair credentials,
or switch provider/model. Raw receipts:

- Fresh: [invocation](receipts/fresh/invocation.json),
  [runtime events](receipts/fresh/runtime.jsonl).
- Existing: [invocation](receipts/existing/invocation.json),
  [runtime events](receipts/existing/runtime.jsonl).

Each returned process exit 1, `is_error: true`, zero input/output usage and no
application tool calls. A local `claude auth status` had reported logged-in
first-party status; that did not prove usable authorization. The maintainer
subsequently clarified that no Claude subscription is available here. Do not
replace that clarification with a recommendation merely to refresh the token.

Neither model, view, scope plan, implementation, Graphify extraction nor tests
were produced by Claude. No readiness, architecture-check, evidence or acceptance
success is claimed. The generated fixture was only prepared by the harness.
No global configuration, runtime installation, subscription or remote project
was changed.

## Reproduction and decision

Recommendation: **park #10 until the maintainer chooses an available second
runtime with usable access**. Do not buy a subscription or change authentication
as an automatic continuation of this trial.

The portable [rerun.py](rerun.py) prepares both projects without contacting the
agent service by default. It uses a local wheel, normal virtual-environment
installation (dependencies may be downloaded), the copied skill and preserved
briefs. Use a new output directory outside the source checkout:

```sh
python3.12 evidence/second-runtime-2026-10-20/rerun.py \
  --wheel /absolute/path/to/gdd_validator-0.8.0-py3-none-any.whl \
  --output /absolute/path/to/new-gdd-runtime-trial
```

Preparation-only was actually checked with the recorded baseline wheel. The
independent receipt is [harness-preparation.json](harness-preparation.json).
After the maintainer explicitly chooses usable Claude access, add `--run` with
another fresh output path to execute both bounded calls (600 seconds and 45
turns per journey). This future run is not represented as completed evidence.
Inspect actual artifacts and interventions before changing the runtime coverage
claim. [Runtime guidance](../../docs/runtime-trials.md) defines what remains.
