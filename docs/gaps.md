# Find holes in a graph

Before handing a workflow to an agent, run the gap check on its entry node.
Omit the scope to inspect the whole graph. An unknown scope or invalid model
produces an error.

```sh
gdd gaps examples/product/partial.gdd.json
gdd gaps examples/product/partial.gdd.json --scope step.reserve --check
```

The first command reports five findings. The scoped command reports four on
the reservation step: missing actor, user action, system response and responsible
component. The unrelated reservation entity falls outside this scope.
Repeat `--scope` to combine the selected nodes and their declared context.
Use `--output FILE` to save the JSON report to a new file. By default a successful
inspection exits zero even when it finds gaps. With `--check`, any open finding
or review question makes the command exit 1. Exit 2 identifies incomplete extension coverage when no open finding takes precedence. Read stderr for input errors.

In the native viewer, expand **Graph gaps** in the node list. Select a finding
to inspect its target. A dot marks nodes with open findings on the canvas.
To explore a particular workflow, save the same scope with
`gdd view MODEL --scope NODE --output workflow.gddview` and open the file.

## What the checker can see

- Product perspectives identify missing links between journey steps, actors,
  components and domain data.
- Each action needs an owner and a normal scenario. Each declared error needs
  negative or boundary scenarios that exercise that action.
- Open model questions appear on their affected nodes with the activity they
  block. Resolve them through the existing GDD question workflow.
- Each blocking rule needs a scenario with a `verifies` edge to the rule.
- A state machine needs an initial state. Traversing its declared transitions
  reveals unreachable states and reachable nonterminal states without an exit.
  To check a known state/event combination, set the machine node annotation
  `"gdd.expected_triggers": {"reserved": ["action.cancel"]}`. The checker reports
  a missing transition for each declared expectation.
  Guards are free-form text. Reachability here considers only the topology.
- A full-project inspection flags normative knowledge sources that no node or
  edge references. An unlinked source has no known workflow scope.
  Coverage is measured at the source level. An agent or domain expert can use
  the review mechanism below to flag an uncovered rule within a linked page.

Each finding has a target ID and a stable key within that target. The report
includes the model revision. The checklist covers the declared graph. Use
scoped readiness, execution evidence and domain review for their respective
questions.

## Record a decision or ask a question

The viewer is read-only. A person or agent can edit the node's annotations
in the model and reopen the view. To record an intentional omission, use the
exact finding key and a nonblank reason. The finding remains visible with that
reason and no longer counts as open in the advisory check. The ordinary GDD
validator still applies its own requirements.

An agent comparing a wiki page, stakeholder interview or code observation to
the graph can also attach a question to the affected node. Give the question a
stable local key and cite a registered source. For example:

```json
{
  "annotations": {
    "gdd.gap_omissions": {
      "MISSING_ACTION_OWNER": "The external provider owns this step during discovery."
    },
    "gdd.gap_candidates": {
      "retry": {
        "source": "source.operations",
        "question": "The operations guide describes retrying a timed-out booking. Which result should the caller receive?"
      }
    }
  }
}
```

The checker displays the candidate as a review question with its source ID.
It does not call an AI service or interpret the source text. Resolve the question
by updating the model and removing the candidate, or record a review decision
under `CANDIDATE/retry` in the same omission map.
