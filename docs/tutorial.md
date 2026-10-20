# From a brief to code and current evidence

Build one pure Python function, inspect its GDD graph, run real assertions and
an isolated failing mutation, and assess current implementation evidence. Use
Python 3.11 or newer with `gdd` installed in that same environment, following
[installation](../INSTALL.md). Run these commands from the repository root;
`python` and `gdd` must select that environment. Keep the same shell open.
The command blocks use a POSIX shell, as in the installation guide.
No LLM, account, subscription or network service is used by this exercise.

From a development checkout, first run `uv sync --locked --dev --python 3.12`,
then `. .venv/bin/activate` in your POSIX shell. This puts both `python` and
`gdd` from that environment on PATH. The current tutorial assets are on `main`,
not in the older v0.7.0 release.

The sequence deliberately distinguishes **valid model**, **implementation
readiness**, **actual execution**, **trusted current evidence** and
**authorization**. A result in one dimension does not establish the others.
All receipts are generated on your machine; the repository supplies no copied
execution result to trust. The tiny check demonstrates the mechanism and its
limits, rather than certifying all inputs or a whole project.

## 1. Write a bounded brief

> A caller supplies two mathematical integers and receives their exact sum.
> The operation is pure and local. Transport, text parsing and bounded
> machine-integer overflow handling are outside this exercise.

This is deliberately smaller than a service. One component and one public
action are sufficient; a new component requires a reason to exist.

## 2. Start with an explicit sketch

Use a new temporary directory; `init` refuses to overwrite an existing bundle.
Keep this shell open so subsequent commands retain `$GDD_TUTORIAL_DIR`.

```sh
GDD_TUTORIAL_DIR="$(mktemp -d)"
gdd init "$GDD_TUTORIAL_DIR/project" --project-id project.integer-addition --title "Local integer addition"
gdd validate "$GDD_TUTORIAL_DIR/project/gdd.json" --profile model
gdd validate "$GDD_TUTORIAL_DIR/project/gdd.json" --profile readiness --scope action.main || test "$?" -eq 1
```

The model check exits **0**. Readiness exits **1**, intentionally: the sketch
has no action contract, behavior, component boundary or normal scenario, and
its implementation scope question is open. Read the diagnostic messages and
locations. A valid graph is not necessarily ready for implementation.

## 3. Make the modeling decisions visible

Read [the complete refined model](tutorial/refined.gdd.json) before copying it.
It is an explicitly authored worked example, not generated code or an automatic repair.
It replaces the sketch with `component.arithmetic`, `action.add` and a planned
normal scenario. The component explains its boundary and explicitly publishes
`action.add`; the action has a matching `owned_by` edge.

The contract specifies both integer inputs, exact integer output, behavior,
postcondition, empty context/errors and complete effects/dependencies. Each risk
has a concrete rationale. For this pure operation no identity, shared state,
external effect or numeric threshold is involved. A bounded integer API, parsing,
authentication or persistence would change that assessment and require further
contracts and scenarios. Empty errors and exclusions are deliberate decisions.

```sh
cp docs/tutorial/refined.gdd.json "$GDD_TUTORIAL_DIR/refined.gdd.json"
gdd validate "$GDD_TUTORIAL_DIR/refined.gdd.json" --profile model
gdd validate "$GDD_TUTORIAL_DIR/refined.gdd.json" --profile readiness --scope action.add
```

Both checks exit **0**. The refined example has no unresolved scope question;
the original sketch remains untouched. It supplies no mandate, trusted decision,
implementation record or executed scenario. Structural readiness is a planning
result, not implementation correctness or authorization to act.

## 4. Inspect the graph and hand context to your agent

```sh
gdd view "$GDD_TUTORIAL_DIR/refined.gdd.json" --output "$GDD_TUTORIAL_DIR/graph.html"
gdd context "$GDD_TUTORIAL_DIR/refined.gdd.json" --scope action.add --output "$GDD_TUTORIAL_DIR/context.json"
```

Open `graph.html` in your browser. The single HTML file works offline and exposes
the canonical model through its inspector. `context.json` gives your existing
coding agent revision-bound context for `action.add`, including its owner and
planned scenario. Review it before implementation. For this exercise we explicitly
choose local Python source files and standard-library `unittest`; your existing
agent can author the same bounded implementation. The next steps show every
selected source, test and host operation. Execute them only under your actual
working mandate; neither the model nor the copied example grants it.

## 5. Review status before planning a change

```sh
gdd status "$GDD_TUTORIAL_DIR/refined.gdd.json" --scope action.add --format text
gdd impact "$GDD_TUTORIAL_DIR/refined.gdd.json" --scope action.add --format text
```

Both are read-only. Status shows structural readiness separately from unassessed
authorization and implementation evidence. Impact explains the selected action's
declared consequences and supporting owner/scenario context without ordering
edits or claiming an executed scenario. See [the inspection guide](inspection.md)
for directional caller/dependency examples and coverage limits.

## 6. State what must be verified

The planning model has no blocking rule: it cannot produce a vacuous evidence
PASS. Read [the new evidence model](tutorial/evidence.gdd.json), then copy it
alongside the original. It adds only `rule.sum`, a blocking invariant with method
`python_unittest`, its `constrains` edge to `action.add`, the scenario's `verifies`
edge, and a separate `record_files` ledger. No old model or record is rewritten.
The method is a descriptive identifier; validation never executes it.

```sh
cp docs/tutorial/evidence.gdd.json "$GDD_TUTORIAL_DIR/evidence.gdd.json"
cp docs/tutorial/tutorial_host.py "$GDD_TUTORIAL_DIR/tutorial_host.py"
python "$GDD_TUTORIAL_DIR/tutorial_host.py" prepare "$GDD_TUTORIAL_DIR"
gdd validate "$GDD_TUTORIAL_DIR/evidence.gdd.json" --profile model
gdd validate "$GDD_TUTORIAL_DIR/evidence.gdd.json" --profile readiness --scope action.add
```

Read [the complete host script](tutorial/tutorial_host.py) before running it.
`prepare` creates an empty exclusive `records.json`, resolves the finalized model
revision, and uses `append_records` to add immutable `check.sum`. Its scope names
`rule.sum` and `scenario.add`; it requires the actual Python environment and the
`isolated-subtraction-mutant` control. The model, check and evidence revisions are
separate identities. Adding records does not edit the finalized model.

## 7. Implement the selected action and inspect the assertions

Read [the implementation](tutorial/integer_add.py) and
[the unittest file](tutorial/test_integer_add.py). The implementation is exactly:

```python
def add(left: int, right: int) -> int:
    return left + right
```

The selected examples assert `(-7, 7) -> 0` for `scenario.add`, `(2, 3) -> 5`,
zero, two negatives and an integer larger than a machine word. These assertions
check concrete cases of `rule.sum`. The type annotations do not validate arbitrary
Python values: the contract is for caller-supplied mathematical integers, and
parsing or accepting floats is outside this exercise. Review the one-expression
implementation for the stated absence of state and external effects.

```sh
cp docs/tutorial/integer_add.py "$GDD_TUTORIAL_DIR/integer_add.py"
cp docs/tutorial/test_integer_add.py "$GDD_TUTORIAL_DIR/test_integer_add.py"
```

This is an explicit worked implementation, not code synthesized by the validator.
For a real task your agent writes and reviews these artifacts against the context
packet and contracts before the host selects checks.

## 8. Execute a real negative control, then check the current code

```sh
python "$GDD_TUTORIAL_DIR/tutorial_host.py" negative "$GDD_TUTORIAL_DIR"
python "$GDD_TUTORIAL_DIR/tutorial_host.py" check "$GDD_TUTORIAL_DIR"
```

`negative` creates a fresh `mutant/` copy and changes `return left + right` to
`return left - right` there. It runs the unchanged assertions with
`[sys.executable, "-B", "-m", "unittest", "-v", "test_integer_add"]` through
`execute_check`, requiring an actual exit **1 / FAIL**. Read
`negative.receipt.json`: the failure must come from the assertions, not an import
or environment error. Current implementation bytes remain untouched. This control
shows that the selected assertions distinguish subtraction from addition; it
cannot establish exhaustive correctness.

`check` selects the same argv in the actual code directory and requires exit
**0 / PASS**. Inspect `pass.receipt.json` for stdout/stderr, command, runner,
timestamps and identical before/after source snapshots. Both operations explicitly
hash `integer_add.py` and `test_integer_add.py`, not the receipts. An unavailable
checker, timeout or code change during execution yields UNKNOWN rather than PASS.
Receipts are created exclusively; rerunning a stage cannot silently replace one
or repeat its checker. The host script does not interpret graph text as commands.

## 9. Author current records and explicitly supply local trust

Only proceed after inspecting the implementation, assertions and both receipts.
You are the local caller attesting that these executions and this limited coverage
are the ones you actually reviewed. A copied receipt cannot authenticate itself.

```sh
python "$GDD_TUTORIAL_DIR/tutorial_host.py" record "$GDD_TUTORIAL_DIR" --attest-reviewed-checks
gdd validate "$GDD_TUTORIAL_DIR/evidence.gdd.json" --profile records --scope action.add || test "$?" -eq 2
gdd validate "$GDD_TUTORIAL_DIR/evidence.gdd.json" --profile records --scope action.add --host-context "$GDD_TUTORIAL_DIR/host-context.json"
```

`--attest-reviewed-checks` is your explicit local review attestation; `record`
refuses to proceed without it. It checks the isolated FAIL, then calls
`verification_records` with the current PASS receipt, `check.sum`, implementation identity
`implementation.integer-addition.python`, actual Python version, selected test
resource and explicitly performed control. It appends new `binding.sum.v1` and
`evidence.sum.v1` IDs through `append_records`. The test binding maps both named
verification targets to the actual hashed unittest file. The evidence references
the current model revision, immutable check digest and selected code snapshot.
It is not a separate assertion that all conceivable behavior has been tested.

The first validation deliberately omits host trust: implementation is **UNKNOWN**
and exit **2**. The second supplies `host-context.json`: the scoped implementation
assessment is **PASS**, exit **0**. Inspect the `implementation` dimension rather
than treating a raw process exit as a claim about every project dimension.
The host fragment lists the actual verified binding, trusted evidence, supported
method, environment, control and selected implementation/code revision. It has
**no operational policy, mandate, trusted decision or acceptance authority**.
Treat it as a local caller assertion, not a credential to share blindly.

You can see that distinction directly:

```sh
gdd validate "$GDD_TUTORIAL_DIR/evidence.gdd.json" --profile records --scope action.add --host-context "$GDD_TUTORIAL_DIR/host-context.json" --activity acceptance || test "$?" -eq 2
```

Authorization is **UNRESOLVED**, exit **2**, because this exercise supplies no
acceptance policy. Do not add a made-up mandate to turn that result green.
For actual authorized multi-agent execution and run acceptance, continue with
[the host guide](../HOST.md) and [workflow guide](../WORKFLOW.md).

## 10. Observe what happens when selected code changes

```sh
python "$GDD_TUTORIAL_DIR/tutorial_host.py" drift "$GDD_TUTORIAL_DIR"
gdd validate "$GDD_TUTORIAL_DIR/evidence.gdd.json" --profile records --scope action.add --host-context "$GDD_TUTORIAL_DIR/stale-host-context.json" || test "$?" -eq 2
```

`drift` makes a separate `changed/` copy, appends a comment to its implementation
and observes that copy's actual new source snapshot. It supplies the new code
revision in `stale-host-context.json`; the old evidence then yields implementation
**UNKNOWN**, exit **2**, with stale-evidence diagnostics. GDD does not read your
source automatically: the host must observe and supply the current revision.
Even a behavior-preserving edit changes which bytes the receipt covers.

The script also tries `verification_records` with new v2 IDs against the old
receipt and changed code. Authoring is **refused**; inspect
`stale-authoring.json`. No replacement records are appended. Original model,
records, receipts and implementation remain unchanged. To verify actual new code,
run new checks, assess coverage and controls, and append new identities; preserve
the earlier history. To repeat this entire tutorial, start a fresh directory.

You now have an authored graph, a bounded implementation, real PASS and FAIL
receipts and explicitly trusted revision-bound evidence. The separate dimensions,
limited assertions and unassessed acceptance authority remain visible. The
[concert example](../examples/concert-app/README.md) shows a larger historical
implementation; keep its execution claims separate from these new local runs.
