# Check Python imports against the graph

On the development checkout, `gdd check-code` observes Python source dependencies
and compares them with explicit architecture constraints in a valid GDD model.
It does not import or run the project's modules. No Graphify installation or LLM
is required.

```sh
gdd check-code path/to/gdd.yaml --root path/to/source \
  --implementation implementation.main --profile python-imports/0.1 \
  --records path/to/current.records.json --scope rule.api_boundary \
  --output new-observation.json
```

`--records` is optional when the manifest already declares the records document.
A declared file is loaded once even if also selected explicitly. Additional
records must be schema-valid, project-matching and free of duplicate IDs.
`--scope` can be repeated and selects **rule IDs**, not components. Omit it to
assess every explicit architecture constraint. Source inventory remains complete
within the selected root/exclusions, including intermediate components needed
for transitive paths. `--profile` is required; an unknown profile returns UNKNOWN.

The model rule supplies `category: architecture` and a constraint such as:

```yaml
constraint:
  relation: imports
  from: component.api
  to: component.storage
  reach: direct
  policy: forbid
```

Direct means one observed component edge. Transitive means a nonempty path;
a self-prohibition requires an actual self-edge or cycle. Each failed rule
reports one deterministic shortest witness, including every edge's source file,
one-based line, zero-based UTF-8 byte column, module and destination file.
Static imports in functions, conditional branches and exception handlers count
as source dependencies even when that branch might not execute. This profile
checks source architecture, not runtime reachability.

## Native Rust command

With no `--python-runtime` flag, GDD uses the shipped descriptor captured from
the CPython 3.11.15 Linux aarch64 build used to compare the v0.8 Python
implementation. The descriptor selects the 3.11 grammar and that build's
builtin and frozen module names, which take precedence over project source.
The native checker uses this target on every host and reads no interpreter
from the environment. The Python v0.8 checker used its running interpreter,
so a different Python version, build or platform can change syntax acceptance
or whether an import resolves to local source.

Use `--python-runtime` to select another build. Supported target grammars are
3.11, 3.12, 3.13 and 3.14. Library callers can pass
`PythonRuntime::reference()?` or `PythonRuntime::from_json(bytes)?` through
`CodeCheckOptions::runtime`.

```sh
cargo run --locked -p gdd -- check-code path/to/gdd.yaml --root path/to/source \
  --implementation implementation.main --profile python-imports/0.1 \
  --python-runtime target-runtime.json
```

The descriptor is a UTF-8 JSON object. Its required fields are `version`
(the full CPython major.minor.patch string), `impl` (`cpython`), `platform`
(the target platform string), and the string arrays `builtin_names` and
`frozen_names`. A target owner can capture this once on the target and transfer
it to the machine running the Rust command. For example, use the target's Python
executable in a directory for new artifacts.

```sh
python3 -I -S - <<'PY'
import _imp, json, platform, sys
descriptor = {
    "version": platform.python_version(),
    "impl": sys.implementation.name,
    "platform": sys.platform,
    "builtin_names": sorted(sys.builtin_module_names),
    "frozen_names": sorted(n for n in _imp._frozen_module_names() if _imp.is_frozen(n)),
}
with open("target-runtime.json", "x", encoding="utf-8") as output:
    json.dump(descriptor, output, indent=2)
    output.write("\n")
PY
```

The native checker reads the descriptor as data. The report's `tool` names
the Rust producer and `runtime` names the selected target. The
`CODE_RUNTIME_TARGET` advisory identifies the shipped or caller-supplied
descriptor, its platform, its canonical digest and the Ruff parser version. `--output` creates a new file exclusively.
The exit status is 0 for PASS, 1 for FAIL, and 2 for UNKNOWN.

## Bindings and source roots

The checker reads `kind: binding`, `role: code` records for the requested
implementation and current model revision. It has no built-in component/file map.
Targets must identify one component, directly or through unique explicit
`owned_by` relations. Selectors must be `whole-document`; unsupported selectors,
competing record claims and multiple physical paths to the same file are unresolved.
The checker does not automatically retire records through unverified supersession.
Historical mappings are retained as advisory information and never rebound.

For example, author a current binding with a new ID in the records document:

```yaml
binding.api:
  kind: binding
  spec_revision: sha256:<the current model snapshot digest>
  targets: [component.api]
  implementation: implementation.main
  role: code
  resource: app/api.py
  selector: {kind: whole-document}
  status: planned
```

Use the actual digest from `gdd snapshot`; the placeholder above is not a valid
record. Planned mappings are explicit intent, not execution evidence. For
observed/checked bindings the current resource digest must also match: an exact
file uses `sha256:` plus its raw byte hash; a directory/glob uses the canonical
digest of the relative-path-to-raw-hash map. A mismatch prevents absence-based
PASS; an independently witnessed forbidden source path can still fail. Declared
`code_revision` values remain provenance and are not silently equated with the
observer's differently scoped inventory digest.

`resource` is a normalized relative POSIX `.py` path, directory ending in `/`,
or glob. `*`, `?` and bracket patterns match within one path segment; a whole
`**` segment matches zero or more segments. Absolute paths, `.`/`..`, backslashes,
NUL and colon syntax are unsupported. Non-Python exact bindings are advisory;
each constrained endpoint still needs unambiguous Python coverage.

`--root` explicitly defines one local import root. It supports ordinary modules
and regular packages with `__init__.py`. A `src` layout should select its `src`
directory and use bindings relative to that directory. The checker inventories
all `.py` files under the root and never follows source symlinks. Fixed
infrastructure directories `.git`, `.venv`, `.tox`, `.nox`, `__pycache__` and
`node_modules` are omitted. Repeat `--exclude 'tests/**'` for an intentional
additional exclusion. Exclusions and actual omissions are reported; a binding
that touches omitted source cannot establish complete coverage.

## Results and limitations

- **FAIL / exit 1:** an unambiguous source dependency path violates a selected
  rule. Other uncertainty remains visible alongside the witness.
- **UNKNOWN / exit 2:** no witness is established and required coverage is
  incomplete: unavailable/malformed source, absent or competing bindings,
  unsupported imports/profile/scope, or source drift during observation.
- **PASS / exit 0:** selected rules have complete in-profile coverage and no
  forbidden source path. Selecting no constraints produces UNKNOWN rather than
  a vacuous PASS.

Package initializers contribute dependencies. An unbound initializer containing
only a docstring and `pass` can serve as an inert bootstrap. `from package import
child` resolves the child only with an inert initializer and a unique child
module; otherwise it retains definite package observations and uncertainty.
Missing local modules, namespace packages, wildcard package members and ambiguous
module/package identities remain unresolved. Built-in and frozen modules take precedence
over colliding source-root names. External imports are listed without checking
whether their packages are installed.

An import intersecting omitted local source remains unresolved; exclusions
cannot turn a known local dependency into an external import and manufacture PASS.

Recognized dynamic loaders, loader aliases/escaping values, reflective loading,
import-system namespaces and module-registry access yield UNKNOWN. Detection is
bounded: arbitrary reflection, custom import hooks, extra import roots, native
modules and runtime path changes are not proven absent. The report identifies
the Rust parser and the CPython target selected for analysis. A PASS is conditional on this stated static
profile and source-root assumption. Python's [import system](https://docs.python.org/3.14/reference/import.html)
and [import statement](https://docs.python.org/3.14/reference/simple_stmts.html#import)
explain why package and member resolution need these limits.

Unclaimed files and undeclared cross-component imports are advisory. An unbound
file importing bound code leaves coverage unresolved. Missing model `imports`
edges are not an allowlist and cannot alone fail the check.

## Observation and trusted evidence

Output is `gdd-code-observation/0.1`, described by the informative
[observation schema](../schemas/code-observation.schema.json). It deliberately
uses a separate envelope because `gdd-report/0.2` does not define this profile.
It includes model revision, selected implementation/rules, actual source hashes,
inventory digest, selected binding-record digests, runtime, imports, witnesses,
diagnostics and advisory findings. Source locations use a line/column; the JSON
Pointer is empty for Python source. Output files use exclusive creation.
The observer's [diagnostic index](../conformance/code.diagnostics.csv) is separate
from the preserved core-report diagnostic registry.

The command writes no binding, check, evidence or authorization records. A
trusted host can select this CLI through [execute_check](../HOST.md), choose
actual source/test coverage and controls, and retain the fresh receipt. Turning
that execution into revision-bound evidence still requires explicit check plans,
matching snapshots and the host's independent attestations. A raw observation or
`status: checked` declaration does not supply that trust.

For the copied-source concert control and independent package cases, run locally:

```sh
cargo run --locked -p gdd-conformance -- code --output /tmp/new-code-conformance.json
```

The harness copies application `.py` files, authors explicit current mapping
records, and checks clean → injected forbidden import → repair → missing source
as PASS → FAIL → PASS → UNKNOWN. Original concert models, checker, source and
historical evidence remain unchanged. This new control does not rebind the older
application experiment's executions.
