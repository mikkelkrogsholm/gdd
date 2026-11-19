# Track work on the graph

Use the model and an explicit run directory to see which packages can start
together, which must wait, and what the integrated code revision has passed.
A draft can contain unplanned areas. Plan the slice you are ready to build.

```sh
gdd work examples/agent-native/model.gdd.json --run examples/agent-native/native-run \
  --scope component.quote --format text
gdd work examples/agent-native/model.gdd.json --run examples/agent-native/native-run \
  --host-context examples/agent-native/host.acceptance.json --output work.json
gdd-viewer examples/agent-native/model.gdd.json --work work.json
```

The saved example is the original integer quote run. At its initial journal
head discount and tax are independently available. Quote waits for both to
be integrated. At the saved final head all three packages are integrated. The
acceptance host file supplies the original checked revision and evidence for
that execution. The [Rust demo](../examples/agent-native/README.md) creates a
new run with its own receipts and files.

Use repeated `--scope` options to select any of several nodes. A package
matches its declared scope, its implementation targets and their owning
components. A known node can have no work assigned. The workplan declares
ordering explicitly. A call between components describes architecture and
creates no scheduling dependency on its own.

`available` means the workflow's readiness and ordering checks currently allow
the package to be considered. Actual dispatch also requires the host's
implementation authorization and code binding. A submitted package awaits
integration review. Integration updates the recorded code revision.
`acceptance_current` evaluates the complete run against the supplied inputs.

Per-package verification shows checks for global rules and rules that directly
constrain the package's implementation targets or owning components. Each
result names the applicable rule. Run acceptance also covers obligations from
the broader read context. Both use the check plans frozen in the journal. Without a host context the
report leaves verification unassessed. A mismatched model, implementation, code
revision or conflicting check selection leaves verification unknown. Admitted
evidence names the original record and code revision. A saved report captures
the stated journal head. Rerun the command to see later activity.

Rust callers use `gdd::work_overview::work_overview`. The returned
`gdd-work-overview/0.1` report supports JSON serialization. Use
`WorkOverview::retain_scope` to validate a graph-node selection and filter
packages, or `packages_for` to look up packages for a known viewer selection.
