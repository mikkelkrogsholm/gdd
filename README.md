# Graph-Driven Development

GDD is a proposed, agent-independent standard for developing software through a pseudo-graph with pseudocode.

**Input → maintained knowledge → model iterations → scoped development → evidence.**

The agent organizes input, models structure and behavior, and refines the relevant part through iterations. The human examines the result through derived views and provides feedback. The graph connects functions, domain concepts, screens, contracts, and rules.

## Status

**GDD 0.3 — draft standard, October 14, 2026.** This is a developed method and format proposal, not an established industry standard. The protocol is independent of vendor, agent, programming language, and any particular CLI.

The package contains standard documents, JSON Schemas, illustrative models, and a **GDD 0.2/0.3 validator with a Python API and CLI**. The package has a [Graphify adapter](GRAPHIFY.md) that preserves original bytes and all relations and can prepare explicit, unaccepted GDD proposals from code observations. [The validator documentation](VALIDATOR.md) describes support and limitations. An [interactive, offline graph view](VIEWER.md) provides search, communities, and inspection of contracts and pseudocode, with fixes from the [review rounds](evidence/review-loops/README.md). Version 0.5.0 introduced an [agent workflow](WORKFLOW.md) that creates complete work packages, coordinates dependencies, and manages integration/acceptance through existing agents. An [actual code example](examples/agent-native/README.md) demonstrates parallel repairs and revision-bound red/green verification.

```sh
uv run gdd validate examples/concert-0.3.gdd.yaml --profile readiness --scope action.reserve
uv run gdd view examples/concert-0.3.gdd.yaml --output concert.html
uv run pytest -q
uv run python conformance/run.py --output conformance-report.json
uv run python conformance/run_graphify.py
```

The validation CLI writes JSON in `gdd-report/0.2`; workflow commands have separate JSON envelopes with embedded reports. Exit codes: 0 for passed requested checks, 1 for known failures/denials, 2 for UNKNOWN/UNRESOLVED without a known failure. A valid backend model can be structurally ready even when its implementation is still missing.

## Reading guide

- [SPEC.md](SPEC.md): normative model, identity, relations, subgraphs, and extensions.
- [FORMAT.md](standard/FORMAT.md): files, fields, serialization, and revisions.
- [AGENT-PROTOCOL.md](standard/AGENT-PROTOCOL.md): input, iteration, mandate, context, parallelism, and delivery.
- [VALIDATION.md](standard/VALIDATION.md): validity, readiness, authorization, evidence, and diagnostics.
- [INTEGRATIONS.md](standard/INTEGRATIONS.md): wiki/OKF, DESIGN.md, Graphify, and views.
- [DESIGN-PRINCIPLES.md](standard/DESIGN-PRINCIPLES.md): component rationale, public contracts, community boundaries and advisory KISS/DRY review.
- [MIGRATION.md](standard/MIGRATION.md): explicit migration from gdd/0.1 and gdd/0.2.
- [DECISIONS.md](standard/DECISIONS.md): rationale for the draft's technical choices.
- [GDD.md](GDD.md): concise entry point for agents.
- [schemas/README.md](schemas/README.md): machine-readable contracts and limitations.
- [examples/README.md](examples/README.md): standard examples and a bounded, executed agent workflow.
- [VALIDATOR.md](VALIDATOR.md): usage, profiles, the host's trust boundary, and limitations.
- [WORKFLOW.md](WORKFLOW.md): development plans, agent packages, queue, and integration/acceptance.
- [VIEWER.md](VIEWER.md): interactive graph, scoped projections, traceability, and concrete artifact verification.
- [GRAPHIFY.md](GRAPHIFY.md): lossless import, profiles, observation bundle, and continuation into GDD through explicit mapping.
- [conformance/cases.yaml](conformance/cases.yaml): 58 positive and negative standard cases; run with [run.py](conformance/run.py).
- [evidence/standard-review.md](evidence/standard-review.md): performed checks and their scope.

Each project chooses its own working mode and approval rules. New projects start from input; existing projects can start from Graphify observations. Accepted intention and observed implementation remain separate.

A valid model, a structurally ready scope, authorization, and implementation evidence are four separate assessments. None automatically proves the others.

The decision basis is in [SCRATCHPAD.md](SCRATCHPAD.md). [Research](research/2026-10-13-existing-work.md) describes substantial overlap with existing work. The previous specification is preserved in [archive/gdd-0.1](archive/gdd-0.1/README.md). The root `concert.gdd.yaml` is still 0.1; the migrated example is under `examples/`.

## Repository language

All repository prose and user-facing assets are English. Package 0.6.0 and renderer 0.2.0 support the English GDD 0.3 standard and preserved GDD 0.2 semantics. Package 0.5.1 provided the earlier English presentation. [English verification](evidence/english-review.md) records that release's checks and the provenance of translated historical material.


## Component boundaries and design review in 0.3

Every relevant component has a responsibility, a short rationale for its boundary,
and explicit exclusions at implementation readiness. Public action contracts have
postconditions and belong to one provider; cross-component calls use that provider's
interface. Communities display derived public boundaries with overlap and omissions.
Agents review KISS, DRY, separation of concerns and modularity. Broad findings are
advisory; precise blocking constraints still require actual implementation evidence.

```sh
uv run gdd boundaries examples/concert-0.3.gdd.yaml
uv run gdd design-review examples/concert-0.3.gdd.yaml --review examples/design-review.planned.json --implementation implementation.concert.python --code-revision planned --scope action.reserve
uv run python conformance/run_design.py
```

The supplied review is explicitly planned and returns UNKNOWN (exit 2). A completed
review envelope can match revisions without proving execution, quality or acceptance.
See [the 0.3 verification](evidence/gdd-0.3-review.md). The unchanged 0.2 example and
native execution remain valid under their own contract; [the archive](archive/gdd-0.2/README.md)
and version-dispatched schemas preserve it.
