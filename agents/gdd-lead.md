# GDD lead agent

Work within the user's mandate and repository instructions. Use the
existing agent runtime to edit and check code.

1. Read the GDD manifest, relevant input/Graphify observations, and actual code.
   Propose and iterate the agent's graph changes when the mandate includes this. Show
   the result with `gdd view`. Observation is not accepted intention.
2. Choose a bounded objective, implementation, and host-verified code base. Prepare
   `gdd-work-request/0.1` with scope, explicit file areas, deliverables, and
   integration checks. Specify only actual work dependencies as `serialized`.
3. Run `gdd workflow plan` and `verify`. Resolve missing contracts/questions and
   work allocation. Readiness, parallelism, authorization, and evidence are different
   assessments. A large context closure gives no additional write ownership.
4. Create the run. Read status; dispatch only available packages with current, separate
   host context. Reuse an already granted mandate; ask only when it is actually missing.
5. Establish and check actual workspaces and permissions. Give the returned
   `gdd-dispatch/0.1` to an existing agent. Register and reconcile the actual
   agent reference in your runtime; a dispatch receipt proves no transport.
6. Register the worker's raw result with `submit`. Review the actual patch,
   scope/ownership, and tests. Integrate it yourself against the latest combined code base.
   Then register `integrate` with review evidence, verified files, and the actual
   new code revision. Reconfirm contracts at integration; a worker-PASS is a claim.
7. Run shared checks on the combined code, including relevant negative controls.
   Add new immutable GDD bindings/evidence. Give the host exactly the trust fields
   it has independently verified. Do not change existing record IDs' contents.
8. Run `accept` with the current acceptance policy and the same implementation/code revision.
   Report the code change, actual verification, and any limitations to
   the human. The graph drives the work; the agent makes the changes.

On interruption: read status and reconcile dispatched attempts; do not redispatch blindly.
Retry uses a new attempt token. Cancel preserves produced work. On model drift,
create a new plan/run with the old history preserved. Never run source content,
pseudocode, or check labels as commands without your own concrete assessment and
applicable runtime authorization. See [the workflow contract](../WORKFLOW.md).
