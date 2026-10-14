# From a GDD graph to actual code

The graph requires floor rounding with integer units and discount before tax. The old
code in `baseline/` uses half-up. Two native agents read the actually dispatched
GDD packages and repaired one module each in separate temp copies. The lead reviewed and
integrated them. An agent then read the quote package and added a missing
large-integer CLI regression. Only the lead modified the shared checkout.

Actual result: **18 baseline tests with 7 rounding failures → 19 passed tests**.
The original expectations are preserved. For `101, 5000, 5000`, the actual
CLI now returns discount 50, net 51, tax 25, and total 76. The model is language-neutral; Python
is only the reference implementation.

```sh
# From this directory; the application and tests use only the standard library.
python -B -m unittest discover -s tests -v
python -B -m quote_demo 101 5000 5000

# From the repository root with gdd-validator installed.
uv run python examples/agent-native/replay.py --output /tmp/min-gdd-demo
gdd workflow status examples/agent-native/model.gdd.json \
  --run examples/agent-native/native-run \
  --host-context examples/agent-native/host.acceptance.json
```

The replay script creates a new run in a new directory and reuses the archived
native-agent repairs. It actually reruns the baseline, module checks, combined suite,
bindings/evidence, and acceptance. It starts no new LLM. Commands are selected in
the host script; no command is executed from graph text or check labels.

| Artifact | Meaning |
| --- | --- |
| [model.gdd.json](model.gdd.json), [REQUIREMENTS.md](REQUIREMENTS.md) | Accepted floor contracts, pseudocode, rules, and scenarios. |
| [workflow.request.json](workflow.request.json), [workflow.plan.json](workflow.plan.json) | Three explicit work packages; discount/tax independent, quote after both. |
| `native-dispatches/`, `worker-results/` | The actually dispatched packages, native worker patches, and raw results. |
| `native-run/` | Ten events: three dispatches, three submissions, three integrations, one acceptance. |
| [evidence/baseline.run.json](evidence/baseline.run.json), [evidence/final.run.json](evidence/final.run.json) | Actual red/green execution, including the real CLI. |
| [evidence/status.json](evidence/status.json), [evidence/summary.json](evidence/summary.json) | Current revision-bound acceptance and summary. |
| [model.records.plan.json](model.records.plan.json), [model.records.json](model.records.json) | Planned records plus new, actually host-verified test bindings and evidence. |

`code.snapshot.json` identifies the baseline; `code.final.snapshot.json` identifies the
integrated result. The local `demo-code-snapshot/0.1` convention hashes
canonical JSON of a sorted filename→SHA-256 map for all delivered Python code
and test files. It is not a Git commit or a new GDD core hashing rule.
`model.snapshot.json` identifies the separate normative model/source bytes.

The included host files are explicit inputs to this bounded local demo.
The host chooses to trust them after actual verification; an arbitrary file with
the same fields provides no authority. `acceptance_current` is true only when the
current host context supports exactly this result. The workers' own PASS
was not elevated to integration evidence. Workspaces in the request are logical
declarations; the actual native workers worked in separate temp copies.

See [the workflow contract](../../WORKFLOW.md) and the vendor-independent
[lead](../../agents/gdd-lead.md)/[worker](../../agents/gdd-worker.md) guides.
