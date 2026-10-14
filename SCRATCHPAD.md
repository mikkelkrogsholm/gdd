# GDD — scratchpad

Date: October 13, 2026.

Collected working notes from the conversation through October 13, 2026. These notes distinguish between the agreed direction, research findings, proposals, and open choices. They do not themselves change SPEC.md or concert.gdd.yaml.

**Subsequent status:** The agreements have now been incorporated into the [GDD 0.2 draft standard](SPEC.md), with normative chapters, schemas, and illustrative examples. The former 0.1 specification is explicitly archived. See the [README](README.md) for a reading guide and the [review report](evidence/standard-review.md) for completed checks. This scratchpad is retained as the historical basis for decisions; its original open questions below describe the starting point for the now completed standards work.

## Agreed direction and decisions

- **The method first.** We are developing the GDD method before turning it into a framework.
- **A standard for all agents.** GDD must be agent independent; it is not limited to Codex or Claude Code. Concrete tools implement the standard.
- **Through agents.** The primary workflow uses existing AI agents, as Graphify does. The agent reads, changes, and applies the model.
- **The human sees the result.** The graph view provides an overview, navigation, and feedback. Nodes and relationships are edited through the agent, rather than directly in the visualization.
- **Two entry points.** We must be able to begin with input for a new project and continue from an existing Graphify graph of code.
- **A maintained knowledge layer.** The framework must be able to build and maintain an LLM wiki/OKF from the input material. The precise format integration remains open.
- **Pseudo-graph with pseudocode.** The shared model describes both structure and behavior and becomes more precise through iterations.
- **Communities and parallelism.** The graph can be grouped, for example into backend and frontend, and used to find independent development tasks.
- **User experience and design.** The model must also describe screens, navigation, user actions, and visual preferences. DESIGN.md must be included.
- **The project chooses its working mode.** Both approval before implementation and automatic continuation within the agreed task must be supported. The user or project sets the working mode and approval rules; no universal default mode has been agreed.
- **Language-independent intent.** The same abstract model must be able to serve as the basis for implementation in different programming languages.
- **A validation tool.** There must be a tool, analogous to the validation tools around OKF and DESIGN.md, that agents can use to validate the graph.
- **A bounded first validator.** It checks graph model validity and the structural part of implementation readiness. The standard's links to code and test evidence must be considered from the start; broad implementation checks come later through separate checkers.
- **Multiple levels of detail.** The model starts with features, domain concepts, screens, and contracts. A node can be expanded into a subgraph when greater precision is needed. Files, classes, and functions are connected through code bindings.
- **Fixed contracts and free-form pseudocode.** Structured fields describe inputs, outputs, errors, dependencies, and rules. The flow itself can be described in free-form pseudocode. The concrete serialization is still a format proposal.
- **A small core with extensions.** The standard must support extensions with declared node types and relationships whose fields and connections can be validated.
- **Concrete scenarios.** An implementation-ready slice must contain a normal flow and relevant error or boundary cases. Scenarios can be expressed as given/when/then and linked to actions and rules.

## Current work boundary

Clarification of the overall direction is complete for now. The next step is to develop the standard and concept from the material and agreements we already have.

We must now formulate a coherent standard for the model's concepts, relationships, contracts, extensions, agent protocol, and validation. This is specification work, before implementing the framework or validation tool.

The proposal to choose and build a concrete demonstration project now was not selected. The reservation model can serve as existing illustrative material during the standards work. A later practical experiment, frontend implementation, and trial on an existing codebase are not the next task in this phase.

The open details below are design work for the standard. They do not mean that another round of questions about the main direction must now take place.

## Direction

We want to develop the method before the framework.

A human or an agent supplies input about the desired system. Through iterations, the input becomes a pseudo-graph with pseudocode. Once the relevant part of the model is clarified, the graph drives development: AI agents translate bounded parts into code and tests, and independent tasks can run in parallel. Checks and evidence connect the implementation back to the model.

The graph describes both the system's structure and its behavior. It is the primary working basis for expressing and changing the system's intent.

## From input to development

**Input → LLM wiki/OKF → pseudo-graph with pseudocode → iteration and clarification → graph-driven development → evidence and further changes.**

Input can include ideas, conversations, documents, examples, existing knowledge, and constraints. Both humans and agents can contribute.

An LLM wiki/OKF organizes and connects the material so it is available during modeling. The agent must be able to build and maintain this layer as part of the process. Whether all projects must have the same knowledge structure, and exactly how OKF is involved, has not yet been established.

Translation into a pseudo-graph is a modeling task: identify desired properties, actions, data, rules, and relationships. Preserve references back to the sources. Conflicting input and the agent's own assumptions must remain questions or proposals until clarified.

The graph develops through several iterations between human and agent. The number of iterations is not fixed. The relevant part is ready for development when its behavior, boundaries, contracts, and significant open questions have been sufficiently clarified. The rest can remain rough.

## Two entry points to the same method

**New project:** input and knowledge become a pseudo-graph, which is iterated and then drives development.

**Existing project:** a Graphify graph of the existing code serves as the starting point for understanding and further modeling. Observations about the current implementation and requirements for the desired solution must remain distinguishable. An observed relationship does not automatically become an approved architectural rule.

The shared model must connect what exists with what is desired: what is present, what must be retained, what should change, and what still needs clarification or evidence. The concrete import format and rules for conversion from Graphify have not yet been designed.

## Agent-independent standard and tools

GDD must be a shared standard that all relevant AI agents can follow. Codex and Claude Code are examples of consumers of the standard; no particular model or agent has been selected as a required runtime.

The primary experience is agent based. A human supplies input and feedback; the agent maintains knowledge and the model, uses the validator, and drives development. The goal includes visualization capabilities similar to Graphify.

It has been decided that a validation tool must exist. Whether the tool must be mandatory for all forms of GDD use, and how agents find and call it, remains open. No concrete CLI, SDK, MCP interface, installation format, or implementation language has been selected.

## Visualization and editing

The agent changes the model. The human sees the result and gives feedback through the agent.

The view must support exploration of nodes, relationships, and communities. Interactive graph views like Graphify's are desired. The precise export types, filters, layouts, and status markers remain open.

The view must be derived from the shared model. It must not develop a competing version of the requirements. The initial direction does not include a graph editor where the human edits the model by moving or connecting nodes directly.

## Approval and automatic continuation

Both working modes must be supported:

- The agent presents the relevant pseudo-graph and waits for human approval before working on code.
- The agent continues automatically within the agreed task and existing authorization.

The user or project sets the working mode and approval rules. The proposed shared default mode with human approval of the first graph and significant changes was not adopted as a universal rule.

Automatic continuation applies within the project's agreed mandate. It does not turn ambiguities into accepted requirements and does not itself grant authority to weaken contracts. How the configuration is represented, and how changes and relevant decisions are recorded, must be described during the standards work.

## Communities and parallel development

The graph can have communities, for example backend and frontend. They provide an overview, bound the agent's context, and can allocate responsibility. They can also be grouped by domain or feature. How membership and possible overlaps are represented remains open.

Communities are groupings in the shared model. Relationships and contracts across groups must remain visible. A grouping or an automatically detected cluster is not itself evidence of independence.

Parallel work tasks are derived from the graph's relevant dependencies, shared data, global rules, and implementation ownership. Backend and frontend can, for example, work simultaneously when their shared contract is clarified and an integration plan exists.

A work task should identify graph IDs, necessary contracts, prerequisites, ownership of implementation files, and expected evidence. Independent Git writers must use isolated worktrees; parallel development also requires coordination of changes to shared contracts and the model itself.

Parallelism is part of the vision. A concrete scheduler, a particular number of agents, or a requirement to always use multiple agents has not been decided.

## Levels of detail and subgraphs

The graph must support refinement at multiple levels. It can start with features, domain concepts, screens, and contracts. When a part needs more precision, a node can be expanded into a subgraph.

A subgraph must remain part of the same model, with traceability to the parent node. Concrete files, classes, and functions connect to the model through code bindings.

Levels of detail and communities are different concepts: a subgraph expands a part of the model, while a community groups related elements. The precise relationships and rules for boundaries, references, and contracts across levels must be established.

## User experience and DESIGN.md

The pseudo-graph must also describe the user experience: screens, navigation, user actions, and visual preferences. Frontend agents must be able to develop from this part of the model.

DESIGN.md must serve as the shared design basis, with visual direction, tokens, component guidance, and relevant states. The graph's UI elements must be able to reference it so design decisions can be reused across screens and agents.

The proposal is for the graph to describe the product's interaction and connect it to DESIGN.md rather than copy the entire design basis into every node. The precise binding format and any design checks remain open.

No visual identity has yet been chosen, and no DESIGN.md has been written for the GDD tool or the reservation example. Integration of the DESIGN.md format and the design-md skill is desired; the concrete integration mechanisms have not been implemented.

## Programming-language independence

The goal is an intent model independent of language and technology. It describes the domain, behavior, and contracts using abstract types and pseudocode. Technology choices, concrete types, framework details, and code bindings belong to a separate implementation layer that references the model's IDs.

The same model can therefore be the starting point for different implementations, for example a backend in Python or TypeScript. Each implementation must be verified separately; evidence from one implementation does not automatically transfer to another.

Language independence comes from the chosen semantics and abstraction. A graph can also contain language-specific details, so the graph structure alone does not provide this property. We must therefore keep pseudocode and contracts free from unintended technology bindings, while precisely expressing requirements such as atomicity and access control.

## What does native to AI agents mean?

The working hypothesis is that the agent must be able to find, follow, and change the model's elements through explicit references. An action must be readable together with its contract, data, dependencies, relevant rules, and scenarios.

The essential element is the reading contract between model and agent. The file format is the representation. A model's ability to read YAML does not itself demonstrate that the method produces better results.

## Agreed expression: fixed contracts and free-form pseudocode

The model uses a hybrid:

- Fixed fields for inputs, outputs, errors, dependencies, and rules, allowing the tool to check their structure and references.
- Free-form pseudocode for the flow, which the agent interprets and implements within the contract.

This allows naturally expressed ideas and gradual clarification. Structural validation of the fields does not execute the pseudocode or demonstrate its domain correctness.

Individual pseudocode concepts may later receive standardized semantics if experience shows a need. We have not decided to start with a fully executable programming language.

## Proposal: a text-based, typed graph

Use YAML for the model's structure and multiline fields for pseudocode. This builds on the existing example.

The following is a notation sketch, not an adopted schema or a complete implementation-ready contract. IDs are shortened for readability; a concrete revision must preserve the existing stable IDs.

```yaml
format: gdd/0.1

entities:
  concert:
    fields:
      capacity: integer

  reservation:
    fields:
      concert: ref[concert]
      owner: authenticated_user
      state: reserved | cancelled

actions:
  reserve:
    input:
      concert: ref[concert]
      request_key: string
    returns: ref[reservation]

    creates: [reservation]
    obeys: [capacity, idempotency]

    behavior: |
      Validate authenticated user and input.

      If this request previously succeeded:
        Return its reservation in its current state.

      Atomically:
        Require an available seat.
        Create the reservation.
        Bind the request key to the reservation.
        Return the reservation.

rules:
  capacity:
    applies_to: [concert, reservation]
    must_hold: |
      Active reservations never exceed concert capacity.

  idempotency:
    applies_to: [reserve]
    must_hold: |
      Repeating a successful request creates no new reservation.
```

`creates`, `obeys`, and `applies_to` express typed relationships in the graph. The pseudocode describes the action's flow. A graph view must be derived from the same model.

Pseudocode may be flexible. The model's structure and relationships must be unambiguous. The meaning of the free text is not yet machine defined; structural validation does not prove that the implementation follows it.

## Declared extensions

GDD must have a small shared core and support declared extensions. A project or domain may, for example, need node types such as `sensor`, `robot`, or `training_job`.

Extensions must describe their fields and relationships so the validator can check them. The domain types mentioned are examples of extensions, not mandatory core types.

Namespaces, versioning rules, distribution format, and handling of unknown extensions must be established in the standard. Extension support must not make the core unnecessarily large or require the same domain concepts in every project.

## Scenarios as part of implementation readiness

A slice that is ready for development must have a concrete normal flow and relevant error or boundary cases. Scenarios can be described as given/when/then and connected to actions and rules.

The reservation model illustrates, for example:

- The last available seat is reserved.
- Two users try to reserve the last seat simultaneously.
- A previously successful request is repeated.

Scenarios give the human and agent concrete expectations against which to assess behavior. They describe planned checks; a described situation is not a passing test result.

The validator can check their fields and references. Whether the scenarios actually cover the important cases still requires domain review.

## Principles of the method

1. **Stable IDs.** Actions, entities, components, rules, and scenarios retain their identity when names or implementations change.
2. **Defined relationships.** Possible relationships such as `calls`, `reads`, `creates`, `changes`, and `obeys` must have precise meanings and permitted endpoint types. Multiple relationships between the same elements must be preserved.
3. **Visible open questions.** The agent must distinguish clarified requirements from matters that need clarification before implementation. Design maturity and verification status are different axes.
4. **Bounded reading.** A task collects the relevant contracts, entities, rules, scenarios, and global requirements. The work packet is derived from the model and must preserve references and revision; it is not a separate specification.
5. **Traceable changes.** The agent reports affected graph IDs, associated code, and concrete evidence. An observed deviation does not automatically change the accepted requirement.

## Validation — agreed initial boundary

We distinguish three questions:

1. **Is the model valid?** Check, for example, unique IDs, valid references, relationship endpoint types, state transitions, and structural references to design and sources.
2. **Is the selected slice ready for implementation?** Check whether necessary contract fields and concrete scenarios are present, prerequisites are declared, and significant open questions are marked. Domain adequacy still requires review.
3. **Does the implementation satisfy the requirements?** Use appropriate code analysis and tests; connect results to graph IDs and the concrete revision.

The first validator handles point 1 and the structural part of point 2. It must provide concrete feedback the agent can act on. It can detect a missing error contract; it cannot by itself establish that a completed contract is correct for the domain.

Point 3 must be connectable through separate checkers, for example a Graphify adapter for architecture, database tests for concurrency, and checks of the frontend against the design basis. These are later possibilities, not capabilities already implemented.

The standard's links between requirements, code, and test evidence must be considered from the start. A valid graph, an implementation-ready slice, and passing implementation checks are different assessments. PASS, FAIL, and UNKNOWN must always have a concrete scope.

Illustrative output — not a result from an existing tool:

```text
Model: valid

action.reserve:
  Ready for implementation: no
  Missing: expected result for an unknown concert

Implementation checks:
  rule.capacity: UNKNOWN
  No test evidence recorded
```

The CLI name, report format, schemas, and precise readiness profile have not yet been established.

## Agent protocol — first proposal

**Read slice → clarify gaps → implement the contract → check the result → connect evidence to the model.**

- Read the selected action and its relevant contracts, global rules, scenarios, and bindings.
- Make significant ambiguities visible. Propose clarifications before dependent implementation; the rest of the project may remain sketched.
- Implement within the accepted contract and bounded scope.
- Run the relevant structural and behavioral checks, including negative controls where necessary.
- Record results as PASS, FAIL, or UNKNOWN with scope, revision, and evidence location.
- Connect code and evidence to the model's stable IDs. Propose requirement changes explicitly.

## What we learned from Graphify

The installed version was examined as part of the conversation: Graphify 0.9.75. Extraction is separate from graph construction, analysis, and export. This makes its approach to navigation, context, dependency analysis, and visualization relevant to GDD.

A small in-memory experiment showed:

- A handwritten extraction-like graph was accepted by schema validation.
- Stable intent IDs and an additional pseudocode field on a node were preserved.
- The graph could be built as a directed graph.
- Two different relationships, `creates` and `returns`, between the same two nodes were merged into one in the build route examined.

The experiment was a format probe. It generated no application and did not prove business behavior or lossless compatibility with the entire GDD model. No files were written by the probe.

The lesson is that Graphify can be an observer and a source of reusable ideas or adapters. GDD's model must preserve its own stable IDs, relationships, and normative meaning. Graph → code also requires intent, contracts, and pseudocode; multiple implementations may satisfy the same model.

## Existing work and our possible contribution

The research found significant overlap, particularly:

- Microsoft RPG/ZeroRepo: requirements → planning graph → repository, and code → graph through RPG-Encoder.
- Spec Growth Engine: specification graph, bounded agent context, code bindings, drift checks, and evidence.
- grAIph: close conceptual similarity to a visual, graph-driven development environment; the current implementation is described as private.
- Kenneth Tyler's GraphDrivenDevelopment: both the name and the idea of the graph as an authoritative representation are already in use.
- Intent Architect and JetBrains MPS: models and custom languages as the basis for code generation.

A combined [research report with sources](research/2026-10-13-existing-work.md) and a [project overview](research/2026-10-13-projects.csv) have been saved. Competitors' tests and generation workflows were not run during the research.

We have not documented that the overall GDD idea is new. A possible contribution is the simple pseudo-graph with pseudocode, gradual clarification, and a consistent agent protocol. The benefits must be demonstrated. Reuse, adapters, or a custom implementation per mechanism have not yet been selected.

## Existing project material

- `gdd-codex-starter.zip` was read: 14 files containing method text, agent rules, a reservation graph, a structural Python linter, 15 linter tests, prompts, and saved baseline results.
- `SPEC.md` and `concert.gdd.yaml` were subsequently added and read. They make contracts, scenarios, bindings, and evidence policy concrete.
- The saved baseline results in the ZIP file concern structural validation. They are not application evidence and were not rerun when read.
- The reservation system is a possible demonstration example. No reservation app, GDD code generator, or new validation tool has been implemented in this conversation.
- This scratchpad collects the further direction. The existing specification and example graph must later be revised explicitly if the new method decisions are to be incorporated there.

## The next method work

Find the smallest set of concepts that can support the workflow. Candidates are component, entity, action, rule, scenario, relationship, binding, and evidence.

Then clarify:

- Which relationships are necessary, and what do they mean?
- What must be specified before an action is ready for implementation?
- How are open questions and assumptions expressed?
- How can a sufficient slice be found without losing global requirements?
- How are the model and implementation changed together while preserving the requirement's authority?
- Which parts of the pseudocode need defined semantics, and which can remain reviewed free text?
- How are communities, UI flows, and DESIGN.md references represented?
- How are Graphify observations imported without mixing them with accepted requirements?
- How is the knowledge layer built and maintained, and how is the model traced back to input?
- What are the minimum requirements for calling a project or tool GDD compatible?
- How are the project's own working modes, approval rules, and mandate for automatic continuation represented?
- How are subgraphs and contracts between levels of detail defined?
- How are extensions to the core declared, versioned, and validated?
- Which initial validator diagnostics and outputs must agents be able to rely on?
- Which visualizations and integration interfaces does the first reference implementation need?

For now, the reservation model serves as illustrative material for formulating the standard. A later practical experiment can implement an action, change a rule through the graph, and introduce a deliberate fault to challenge the checks. That experiment is not the current task, and no demonstration project has yet been selected.

Existing projects such as ZeroRepo and Spec Growth Engine must be included in the comparison. See the [research on existing work](research/2026-10-13-existing-work.md). Reuse of their mechanisms and selection of a framework engine have not yet been decided.
