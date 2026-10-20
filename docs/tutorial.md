# Plan a small function with GDD

This repository tutorial goes from a brief to an implementation-ready plan. It
uses the installed `gdd` CLI; from a development checkout, prefix commands with
`uv run`. Run the commands from the repository root, after following
[installation](../INSTALL.md). No LLM, network service or execution evidence is
required. The example plans software; it does not claim to implement or test it.

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
gdd validate "$GDD_TUTORIAL_DIR/project/gdd.json" --profile readiness --scope action.main
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
planned scenario. Review it before implementation. Agree on the coding language,
actual code location, verification and working mandate in your own project;
this tutorial does not invent those decisions.

For a larger worked implementation and its recorded limitations, see the
[concert example](../examples/concert-app/README.md). Keep those historical
execution claims separate from this new planning exercise.
