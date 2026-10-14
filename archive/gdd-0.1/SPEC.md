# Graph-Driven Development — v0.1

**Status:** proposed method and notation. Prepared on 13 October 2026.
This is not an existing standard, Graphify syntax or an official Graphify product.

## 1. Definition

Graph-Driven Development here is a development method in which a version-controlled,
typed graph describes the system's intended structure, behavior and verifiable requirements.
The implementation is developed in small parts and assessed against the graph through source-code analysis,
tests and explicit decisions.

The graph is a contract for what we have chosen to specify. It is not a claim that
everything about the system is modeled, and it does not replace source code, tests or review.

The accepted specification is normative. Source code and observations are descriptive.
A deviation must therefore not automatically rewrite the requirement to make the implementation look correct.
An intended requirements change must be reviewed as a specification change.

## 2. One model, multiple views

The architecture view shows components, data responsibility, boundaries and dependencies.
The behavior view shows actions, contracts, states, transitions, conditions and effects.
The traceability view connects requirements to the actions, code elements, checks and decisions
that concern them.

A component is not the same as a domain entity. `component.reservations` can own both
`entity.reservation` and `entity.concert`. `action.reserve` has an owner component and appears
as a trigger in a state machine. Such connections make the model one
coherent graph rather than two independent diagrams.

All elements have stable IDs. An ID does not change solely because a name or file path changes.
Rules and scenarios also have IDs so that their relations can be used in change analysis.

## 3. Pseudo-graph before full precision

An initial sketch can be short:

```text
API calls Reservations
Reservations uses Storage
Reservations owns Reservation and Concert

ReserveSeat: absent -> reserved, if there is capacity
CancelSeat: reserved -> cancelled, if the user is the owner

Capacity must never be exceeded
A repeated request must not reserve an extra seat
API must not import Storage directly
```

This is a sketch, not parser syntax. Before implementing a particular part,
the actions' input, output, errors, access rules, repetitions and relevant boundary cases
must be made precise. The rest of the project can remain a sketch.

Design maturity and verification status are different axes. A precisely specified
function can be unimplemented. An implemented function can have insufficient evidence.
A sketch does not become more precise because a test happens to pass.

## 4. File format and semantics

`concert.gdd.yaml` is the first concrete example. YAML is the editable representation;
a graph database is not needed in this first version. A future visual editor must
edit the same model, not create a competing specification.

The top level contains `format`, `project`, `nodes`, `edges`, `machines`, `rules`,
`scenarios`, `bindings` and `verification_policy`.

`nodes` contains components, entities and actions. `owner` connects an action or
entity to the responsible component; `stored_in` connects an entity to its persistence boundary.
`edges` describes explicit structural relations. An `imports` relation is directed and
here means a permitted, intended direct import between component boundaries; it is not itself
evidence of a runtime call. `calls` describes the intended call of an action.

`machines` connects state machines to entities. A transition has an ID, start state,
end state, action as trigger, condition and effect. The profile in this prototype allows
only one mutating transition per combination of state and trigger. Multiple guarded alternatives,
hierarchical states, events and parallel regions require a later semantic extension.

`absent` means that an entity does not exist yet. It is a modeling marker, not a
value in the database. Terminal states prohibit new mutations of the entity; authorized
reads and idempotent responses remain possible. Error and repetition paths are described in the action.

`rules` defines requirements and the necessary verification method. `scenarios` describes planned
checks with given/when/then and refers to rules through `verifies`. **The relation means a planned
check, not an already passing test result.**

The absence of an architecture edge does not automatically mean prohibition. A prohibition must be explicit,
or a precisely bounded area with a complete allowlist must be declared. Prohibitions must also
specify whether they apply to direct connections or transitive paths. For example, API may
reach Storage indirectly through Reservations even when a direct import is forbidden.

Descriptions, guards, effects and rule expressions are pseudocode. `eval` is not used, and
the linter does not interpret them. A later executable profile can define a small type-safe
expression language or refer to named, reviewed check functions. A syntax schema can
validate data types and mandatory fields, but not business semantics on its own.

## 5. Workflow

Start with a rough graph and identify one small feature from entry point to persistence and test.
For the reservation system, the first part is reserving and cancelling one seat.
Payment, expiry and ticket issuance are deliberately excluded. They must not be slipped in as
AI assumptions; when added, their contracts and error cases must be modeled.

Make the selected part precise. Review at least one normal example and the main negative
and concurrent flows before the agent writes the solution. This can lead to changes in the graph.

Give the implementation agent a bounded graph package: actions, responsible components,
relevant data, incoming and outgoing contracts, global rules, affected scenarios and bindings.
Do not use an arbitrary graph hop count as the sole boundary; critical global requirements can
be far from the local change.

The agent implements within the approved scope. It returns the code change, bindings,
check results and open questions. It can propose a graph change, but must not secretly
relax a requirement or change test expectations to obtain a green report.

Run specification checks, source-code checks and behavior tests. Review the result and the changed
part of the graph. Then accept the change or handle a justified requirements change separately.
A prototype can reveal that the model was wrong. The point is controlled learning,
not locking the entire architecture before the first line of code.

## 6. Comparison with the implementation

The intent graph and code graph are not expected to be identical. A logical component can become many
files, classes and functions. Comparison uses `bindings` to module boundaries and, optionally,
qualified symbols, after which relevant observations are aggregated to the model's level.

A binding must be explicit, version-bound and without ambiguous overlap. Planned paths
do not prove that files exist. Missing or ambiguous mappings produce UNKNOWN for checks
that depend on them. Low-level helpers need not become individual intent nodes.

Behavior rules are checked with appropriate tests, not by observing that two components are connected.
Capacity under concurrency requires tests of the actual persistence mechanism and its
transaction behavior. A unit test of an in-memory list does not provide the same evidence.
Model-based tests can generate action sequences and compare the implementation with
reference behavior. The correctness of the reference behavior itself requires review.

## 7. Graphify's role

Graphify is a possible provider of observations, not the normative model. The current
open-source repository documents `graphify-out/graph.json`, `graph.schema_version` and
`graph.graphify_version`. An adapter must check the format version and preserve relation type,
source reference and provenance. This starter package does NOT contain such an adapter.

Graphify's documentation distinguishes structural code extraction from semantic extraction of,
among other things, documents. It also mentions gaps due to dynamic calls, missing input and old
scans. Therefore, a relation derived from the GDD specification must never count as evidence that
the implementation complies with it. Code observations and normative documents must be filtered
or scanned separately during conformance checks. `INFERRED` does not necessarily mean that
an LLM created the relation; static reference resolution can also produce that marker.

A first adapter can focus on Python imports within the bounded application area. Record
the checker and extractor versions, together with exact code and spec revisions. Unknown formats or
missing analysis input must not produce an empty, apparently green deviation list.

## 8. Evidence and acceptance

Each check receives `PASS`, `FAIL` or `UNKNOWN`, with scope and rationale. PASS means
that this specific check passed on the stated revision under the stated prerequisites. It is
not universal correctness and not automatically a formal proof. Tests, static analysis,
bounded model checking and an actual proof are different kinds of evidence.

Evidence must identify at least the spec revision, code revision, check ID,
checker version, scope, result and output location. Old evidence must not simply be
reused for changed dependencies. This prototype uses conservative re-execution;
smart cache invalidation is later work.

A blocking rule with FAIL or UNKNOWN prevents acceptance in the proposed strict
profile. An exception requires a visible decision with a responsible party, rationale and expiry;
the exception does not change the check's result to PASS.

An acceptable first demonstration is for the system to detect an intentionally forbidden import,
weakened capacity protection and an idempotency defect. The original code passing its
own generated tests is insufficient. Reviewed examples and negative controls must challenge
both the agent and the test basis.

## 9. What the package actually does

`check_spec.py` checks selected structural properties: duplicate YAML keys,
graph IDs, references, endpoint kinds, owners, declared states, transition triggers,
terminal states and planned scenarios for blocking rules. It is not a complete
language validator. It does not check the meaning of the free-form text.

No reservation application, database, test adapter, code generator, visual editor,
Graphify integration or formal verifier is included. Behavior and architecture evidence for an application
is therefore UNKNOWN. The included unit tests test the linter, not the reservation system.

## 10. Existing work and sources

The model builds on existing ideas about model-driven development, state machines and
model-based testing. The name Graph-Driven Development is already used elsewhere.
This package does not claim to have invented the term or the entire field.

Sources read on 13 October 2026:

- OMG: Model Driven Architecture — https://www.omg.org/mda/
- W3C: SCXML, especially transitions and guards — https://www.w3.org/TR/scxml/
- Graphify: official repository — https://github.com/Graphify-Labs/graphify
- Graphify: How Graphify works — https://docs.graphify.com/concepts/architecture
- Hypothesis: Stateful tests — https://hypothesis.readthedocs.io/en/latest/stateful.html
- JSON Schema: Draft 2020-12 — https://json-schema.org/draft/2020-12
- Atomic: its own use of graph-driven development — https://atomic.dev/

These sources document related techniques and integration points. They do not demonstrate
that the proposed GDD method produces fewer defects or lower costs. That must be measured.
