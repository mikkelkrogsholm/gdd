# Existing work before GDD development

Investigated October 13, 2026. Starting point: the user's SPEC.md, concert.gdd.yaml, and vision of an editable pseudo-graph with pseudocode as the basis for AI-generated software.

## Conclusion

Others have already implemented substantial parts of the idea, and there is very close conceptual overlap. Microsoft's RPG/ZeroRepo and Spec Growth Engine are the most important public implementations to compare with. grAIph is close to the overall vision, but its current implementation is described as private. Kenneth Tyler already uses the name Graph-Driven Development for a related concept and has public code.

The investigation does not establish that any solution meets all our wishes. Nor does it provide a basis for claiming that the combination is new. A possible distinguishing feature must be demonstrated through concrete notation and a workflow.

## 1. Microsoft RPG / ZeroRepo / CoderMind

ZeroRepo implements requirements → Repository Planning Graph → repository. The graph connects functionality with implementation structure and dependencies; code generation follows the graph and uses test validation. RPG-Encoder goes from code to graph. CoderMind makes graph work available to coding agents. There is a public implementation and an ICLR 2026 publication, originally published in September 2025. [Repository](https://github.com/microsoft/RPG-ZeroRepo), [paper](https://arxiv.org/abs/2509.16198).

I also read selected parts of the code generator and its prompts at commit `a4e6966c4ff59d84299bc56d6a47ae1f4f7b2885`. There are separate flows for test generation, code generation, and test execution. [Code generator](https://github.com/microsoft/RPG-ZeroRepo/blob/a4e6966c4ff59d84299bc56d6a47ae1f4f7b2885/zerorepo/code_gen/code_gen.py).

Assessment: the strongest direct reference for graph → code. We should examine the difference between their planning graph and our human-editable pseudocode with explicit behavioral rules. That difference has not yet been demonstrated as a unique capability.

## 2. Spec Growth Engine

Software is described as a graph of SPEC.md files with contracts and code bindings. An agent can implement a node from combined context. Deterministic checks validate structure, import deviations, and evidence status. There is a public Python implementation, CLI, cockpit, and tests under Apache-2.0. [Repository](https://github.com/grabow/spec-growth-engine), [implementation paper from October 8, 2026](https://arxiv.org/abs/2610.11725).

Selected code was read at commit `e2319cb8a777242491e7bb8c3dfc535190c4c537`: parser, context assembly, import observations, drift checking, and drift tests. Imports are analyzed with heuristics/regex, and documented limitations include TS aliases and checking imports through public contracts. [Context assembly](https://github.com/grabow/spec-growth-engine/blob/e2319cb8a777242491e7bb8c3dfc535190c4c537/cockpit/engine/context.py), [import analysis](https://github.com/grabow/spec-growth-engine/blob/e2319cb8a777242491e7bb8c3dfc535190c4c537/cockpit/engine/evidence.py), [limitations](https://github.com/grabow/spec-growth-engine/blob/e2319cb8a777242491e7bb8c3dfc535190c4c537/cockpit/README.md#not-yet-implemented).

Assessment: very substantial overlap with our contracts, bindings, scoped agent context, and verification. A possible base or reference implementation; reuse has not been decided.

## 3. grAIph

Describes a visual development environment in which the graph controls context, generation order, contracts, code candidates, and checking. Public status describes a private development implementation with local evidence and unfinished release qualification. It has therefore not been verified as a public, reusable implementation in this investigation. [Project](https://github.com/mateors1/grAIph), [status](https://github.com/mateors1/grAIph/blob/main/CURRENT_STATE.md).

Assessment: very close conceptual overlap with an integrated graph-based workspace. Use it as a design reference and keep its own status report separate from independent verification.

## 4. Kenneth Tyler: GraphDrivenDevelopment

The author describes the graph as the authoritative representation and code, schema, and tests as derived projections. There is a public Node/PostgreSQL project with intent operations, dependency navigation, MCP, and tests. [The author's description](https://thetimethatremains.substack.com/p/graph-driven-development), [repository](https://github.com/kentyler/GraphDrivenDevelopment).

I read selected operations at commit `30ad5713bae5d074e5ab877e54bbd5468c36d057`. `buildProjection` assembles graph context; it is not itself a source-code generator. `recordExpression` records artifacts and their connection to intentions. [buildProjection](https://github.com/kentyler/GraphDrivenDevelopment/blob/30ad5713bae5d074e5ab877e54bbd5468c36d057/src/operations/buildProjection.js), [recordExpression](https://github.com/kentyler/GraphDrivenDevelopment/blob/30ad5713bae5d074e5ab877e54bbd5468c36d057/src/operations/recordExpression.js).

Assessment: both the name and the overall vision have a direct predecessor. The material read does not document a complete pseudo-graph/pseudocode-to-code compiler.

## 5. Intent Architect and JetBrains MPS

Intent Architect combines models with deterministic and AI-based code generation. Its documentation describes, for example, modeling Angular navigation and services followed by AI-generated views, as well as AI implementation of handlers. [Angular workflow](https://docs.intentarchitect.com/application-development/modelling/ui-designer/angular-modeling/angular-modeling.html), [AI handler generation](https://docs.intentarchitect.com/whats-new/2025/06/index.html).

JetBrains MPS is an established reference for custom languages, structured models, and transformation from domain models to source code. [MPS concept](https://www.jetbrains.com/mps/concepts/).

Assessment: model → code is well established. Our possible contribution is a simpler, progressively refined notation and its interaction with agents and checks; this must be tested.

## 6. Other relevant projects

- **Intent Framework:** experimental TypeScript framework with a semantic application graph and materialization of UI/runtime. Its README states alpha status and that it is not yet a compiler. [Repository](https://github.com/intent-framework/intent).
- **NaCl:** public method and tools connecting analysis, architecture, development, and verification around a Neo4j graph. [Repository](https://github.com/ITSalt/NaCl).
- **AINL:** a concrete language with canonical graph IR, runtime, and adapters for AI workflows. Relevant for learning about explicit semantics; its primary scope is AI workers and workflows. [Product description](https://www.ainativelang.com/product), [repository](https://github.com/sbhooley/ainativelang).
- **Proof:** the vendor describes a software intent graph with links to code, tests, and evidence, and analysis of the consequences of changes. The functionality has not been tested here. [Product description](https://reqproof.com/product).
- **Graphito:** the npm package describes GDD as a documentation graph with dependency resolution and validation. The specified GitHub source returned 404 during the investigation; a current public repository implementation was therefore not verified. [npm](https://www.npmjs.com/package/@graphito/core).
- **Atomic and Grafema:** also use the term graph-driven development, focusing on change/intent traceability and code analysis, respectively. [Atomic](https://atomic.dev/index.html), [Grafema](https://grafema.dev/).

A vivaxy plugin with the GDD name was found in directories, but the listed GitHub repositories returned 404. This counts as an unconfirmed lead, not a verified current solution.

## Implications for our framework

The recommendation is to compare ZeroRepo and Spec Growth Engine in particular with one identical example before deciding to build all mechanisms ourselves. The example could be the existing reservation model and a subsequent rule change.

Our desired user experience remains: the human edits the pseudo-graph and pseudocode; the agent implements a scoped change; results and evidence are linked to the model. The central experiment is whether that experience can be made simpler and more precise than existing approaches.

No winner has been selected, competitor tools installed, or accepted GDD specification changed. The next technical decision should be reuse, adapter, or custom implementation for each mechanism.

## Scope of the investigation

Basis: web searches, primary repositories, official product documentation, research papers, GitHub metadata, and selected source code. The projects' tests and code-generation flows have not been run. Test files and author reports have not been treated as independent proof of correctness. The field changes quickly; negative search results do not document the absence of a solution.
