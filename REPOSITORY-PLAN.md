# GDD repository readiness goal

Status: local delivery completed. See [readiness evidence](evidence/repository-readiness.md)
for the actual tested journeys, installation checks and limits. Verification is
local; registry publication remains a separate subsequent action.
Defined October 14, 2026, from the maintainer's direction.

Updated October 14, 2026: the maintainer rejected GitHub CI as too heavy.
Repeatable local checks replace the earlier CI requirement.

## Outcome

Make this repository a practical entry point for Graph-Driven Development:
**a method, a draft standard, a usable CLI and a reusable agent skill**.
A developer with an existing coding agent should be able to start from a new idea
or existing code, produce a valid pseudograph, inspect it, refine an implementable
scope, and drive a verified change through documented tools and contracts.

The concert app remains a reference experiment. Its domain features do not define
the product roadmap. The standard and CLI continue to be independent of agent
vendor and implementation language.

## Definition of ready

Readiness is established by observed user journeys, not by adding a prescribed
number of commands or files.

1. **Understand:** README explains the purpose, method, present capabilities and
   trust boundaries, and sends a first-time reader directly to a working quickstart.
2. **Install:** the documented source installation works in a fresh environment;
   an installed wheel works outside the checkout. The public package name and
   distribution status are explicit. No claim of registry publication precedes it.
3. **Start:** a small project scaffold is created through the CLI without overwriting
   existing work. It is valid as a draft and does not invent implementation readiness,
   accepted requirements, execution evidence or operational authority.
4. **Use an agent:** one installable GDD skill lets an independent agent turn a short
   brief into sources, questions, a graph, a view and a scoped development plan.
   The agent can follow it without bespoke concert-app host scripts or a lead filling
   undocumented protocol fields on its behalf.
5. **Continue existing code:** the documented Graphify journey preserves original
   bytes, creates observations and an explicit intent proposal, and reaches a valid
   refined scope. Unsupported profiles remain UNKNOWN.
6. **Handle a change:** the independent agent revises a requirement, gets red results
   from fixed meaningful assertions, integrates the change and verifies current
   evidence. A controlled architecture violation fails; missing evidence stays UNKNOWN.
7. **Maintain:** core tests and relevant conformance suites pass through repeatable local checks;
   documented commands, installation and skill assets are checked. License and
   contribution policy are explicit before treating the repo as ready for public adoption.

Record both the fresh-project and existing-code trials with input briefs, agent/runtime
identity, actual artifacts, commands, interventions and outcomes. Any intervention
needed to repair the instructions must be incorporated and the affected journey rerun.
The release-readiness bar is successful completion without undocumented intervention.
It does not claim that all architectures, languages or agent runtimes have been tested.

## Baseline when this plan was defined

The table records the starting point, not the delivered package. The current
entry points are `gdd init`, [the installable skill](skills/gdd/SKILL.md),
[installation](INSTALL.md), [host helpers](HOST.md) and
[contribution guidance](CONTRIBUTING.md). The maintainer selected MIT.

| Area | Present | Work needed |
| --- | --- | --- |
| Standard | 0.3 documents, schemas and explicit 0.2 compatibility | Preserve semantics; make the reading path clear |
| Method | Agent protocol and lead/worker instructions | Package a single discoverable onboarding workflow |
| CLI | Validation, context, snapshot, boundaries, viewer, Graphify and run workflow | Project scaffold and portable installation guidance |
| Skill | GDD.md and role guides | Installable skills/gdd/SKILL.md with a tested entry flow |
| Evidence | Real concert/native experiments and conformance | Generalize the host workflow where fresh use demonstrates friction; establish independent journeys |
| Examples | Extensive models, tests, views and original executions | A small curated entry example and indexed exhaustive evidence |
| Repository | Local Python packaging and tests | Repeatable local checks, contribution guide, changelog and maintainer-approved license metadata |
| README | Rewritten in this goal-definition pass | Keep commands and capability statements tested as implementation progresses |

There was no packaged skill, scaffold command or CI workflow in the inspected baseline.
The current distribution is named gdd-validator and exposes gdd. Prefer preserving that
installation contract in this readiness pass; a rename is a separate compatibility decision.

## Repository contents

Keep the existing normative locations and public references stable. Organize the
reader's path before moving byte-sensitive material.

| Surface | Responsibility |
| --- | --- |
| README.md | Product/method entry, verified quickstart, real capabilities and next reading paths |
| GDD.md and agents/ | Existing agent and coordination guidance; one authoritative workflow |
| SPEC.md, standard/, schemas/ | Normative contracts, versioned formats and validation semantics |
| gdd_validator/ | Deterministic CLI/Python tools; no mandatory LLM service |
| skills/gdd/ | Portable skill, concise entry instructions and exact synchronized authoritative references |
| templates/ | Minimal current-standard scaffold used by the CLI and skill |
| examples/ | Curated format examples and one reproducible end-to-end reference journey |
| tests/ and conformance/ | Behavioral regressions, explicit supported/unsupported cases and compatibility |
| evidence/ and archive/ | Indexed provenance, preserved executions and historical normative contracts |

These are responsibilities, not a mandate to reorganize every existing file or add
empty directories. Add only assets consumed by the accepted journeys.

## Delivery order

### 1. Make the purpose and entry clear

The [entry-definition review](evidence/repository-entry-review.md) records the
earlier documentation pass. The readiness review records the completed delivery.

Rewrite README around planning and development through graphs. Keep its first route
short, executable and honest. Explain the agent's role, supported CLI commands,
requirements, expected results and separate assessment dimensions. Link this plan
and retain the detailed normative/CLI guides.

Acceptance: every local link resolves; quickstart commands work in fresh output
locations; advertised commands exist; present and proposed features are labelled.
This pass changes entry documentation only, preserving normative and execution bytes.

### 2. Build the smallest usable starter and skill

Introduce a minimal GDD 0.3 draft template and a safe scaffold operation, delivered
as `gdd init`. Reuse current loaders,
validators, schemas and output conventions. Never infer a trusted mandate or seed PASS
receipts. Preserve existing files and give a clear result for repeated initialization.

Package skills/gdd/SKILL.md using the supported skill conventions. It should guide
input capture, uncertainty, graph iteration, component rationale/contracts, design
references, scoped readiness, parallel ownership, actual integration and evidence.
Reuse GDD.md and lead/worker guidance instead of copying normative rules into another
independent specification. Explain installation for the runtime actually tested.

Acceptance: scaffold is valid; incomplete scope remains unready; existing-directory
and repeated-use behavior is verified; an independent agent completes a fresh-project
planning journey from the skill without an undocumented protocol repair.

### 3. Make the full workflow portable and curate the evidence

Use the cold-start journeys to identify the remaining manual gaps. Current work-request
and host/evidence authoring required substantial lead work in the concert experiment.
Provide a reusable documented path for those fields; add a tool/helper only where
it removes demonstrated friction. Keep trust outside graph/source content.

Remove reliance on personal runtime paths from the public happy path. The current
browser harness can be configured with external paths, but needs a clean installation
recipe and an actual trial in another environment.

Keep a concise reference model, runnable code, meaningful tests, necessary baseline
fixtures, replay entry and result summary in the reader-facing example. Preserve the
full original experiment at commit 7e27757 and in a byte-indexed evidence bundle before
reducing duplicated packets, full journal projections, repeated snapshots or old captures
in the current example. The bundle may be a published downloadable artifact later;
publication requires a concrete reviewed artifact and the maintainer's authorization.

Use a manifest containing original path, byte hash, source Git identity and retrieval
location. Verify round-trip recovery and replay before removing any current path.
Update current documentation/test dependencies; do not rewrite old executions as new
ones, silently translate receipts, break supported 0.2 assets or rebind old hashes.
Moving/changing normative paths or bytes requires new revisions and derived artifacts.

Acceptance: the reference replay works without hidden temp directories; original
execution bytes can be recovered and verified; documented evidence distinguishes old
executions, new verification and reconstructed presentations. Relevant regression and
conformance checks still pass after curation.

### 4. Verify adoption and finish repository readiness

Document repeatable local checks covering the supported Python installation, unit tests and relevant
standard, Graphify, viewer, workflow and design conformance. Verify the installed wheel
outside the source checkout. Document optional browser-check dependencies explicitly.
Add contribution/release guidance and a changelog that separates standard, package and
viewer versions. Obtain the maintainer's license choice and record the corresponding
file/package metadata; do not invent that choice.

Run the independent new-project and existing-code journeys from fresh setups. Feed
actual confusion and failures back into the CLI/skill/docs, then rerun affected checks.
Publish a compact readiness report with exact scope and unresolved limitations.

Acceptance: the seven readiness outcomes above are observed and recorded. Only then
label the repository ready for broader adoption and prepare publication.

## Scope boundaries

Keep the current draft standard and explicit compatibility. This work does not
include a hosted product, mandatory agent service, model provider SDK, autonomous
runtime, new graph database, general-purpose code generator or further concert-app
features. Interactive editing and new generic commands need their own demonstrated
use case. Graph authoring remains agent-driven; tools preserve and check its contracts.

The accepted journeys and repository preparation define this delivery boundary.
They are recorded with actual outcomes in the readiness review. Further platform
work needs its own user outcome. Publication decisions remain with the maintainer;
the MIT license choice is now recorded in LICENSE and package metadata.
