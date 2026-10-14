# GDD — entry point for agents

Reading guide for the 0.2 draft. Normative requirements are in [SPEC.md](SPEC.md) and the chapters it references.

1. Read the format version, model manifest and trusted mandate. Do not assume an approval mode.
2. Register input as sources; maintain the wiki/OKF. Distinguish statements, observations, interpretations and accepted requirements.
3. Use existing Graphify data as observations with provenance. Propose intent explicitly.
4. Iterate the graph with stable IDs, fixed contracts and free-form pseudocode. Record material questions.
5. Assemble the task's full contracts, ancestors, relevant data, machines, global/local rules, scenarios and design references. Bound the write area separately.
6. Assess validity, structural readiness and authorization separately. A schema PASS is not a behavior PASS.
7. Implement only within the task and mandate. Coordinate shared contracts and model changes during parallel work.
8. Connect code bindings, check plans and executed evidence with stable IDs and exact revisions. An exception does not change FAIL/UNKNOWN to PASS.
9. Show derived results to the human and incorporate feedback through the agent.
10. Deliver scope, changed IDs, decisions, revisions, results and open questions. Do not secretly weaken requirements to make checks green.

For existing code graphs, use the [Graphify adapter](GRAPHIFY.md) to create a lossless observation bundle. Then select an interpretation and stable model IDs through an explicit mapping; propose creates an unaccepted sketch that must go through the project's normal model-change process. Producer claims are not authorization or implementation evidence.

The package now includes the [GDD validator](VALIDATOR.md). Run `uv run gdd validate <manifest> --profile readiness --scope <node-id>` for structural readiness. Model, readiness, authorization and implementation are reported separately under the [validation contract](standard/VALIDATION.md). Do not invent tool results or treat planned tests as executed.

## Agent workflow

Use [WORKFLOW.md](WORKFLOW.md) for concrete development plans and run events, and the [lead](agents/gdd-lead.md)/[worker](agents/gdd-worker.md) instructions in existing agents. The graph's context closure is the reading basis; the plan's scope and explicit file areas determine the work. Workflow 0.1 is a tool layer above GDD 0.2 and grants no independent authorization or evidence.

Repository documentation, example prose and user-facing output must be English. Preserve identifiers, proper names and intentional Unicode test data. Follow [AGENTS.md](AGENTS.md) when updating generated or revision-bound assets.
