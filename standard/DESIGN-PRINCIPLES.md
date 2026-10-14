# Design principles and component boundaries

Normative chapter for GDD 0.3. The graph expresses responsibilities and obligations;
agents interpret them and check actual implementations. This chapter does not define
an automatic complexity score or a general code-quality proof.

## 1. Responsibility and rationale

Every component needed by an implementation-ready slice MUST have a nonblank
`boundary.responsibility`, a short nonblank `boundary.rationale`, explicit
`boundary.excludes`, and specified `interfaces`. `excludes: []` declares no additional
exclusions. An incomplete sketch can omit these fields; unrelated sketches MUST NOT
block a complete slice. A rationale explains a current reason for separation, such
as a coherent domain responsibility or a volatile external dependency. It helps
agents assess whether to retain, combine, or split responsibilities later.

Ownership remains expressed by `owned_by`. A boundary description MUST NOT become
a competing ownership registry. Textual completeness is structurally checkable;
adequacy of the rationale requires review. The agent MUST NOT add layers solely to
increase modularity or to anticipate unspecified future requirements.

## 2. Authoritative contracts

An action node holds its authoritative contract. A component's `interfaces` exposes
its own action IDs. Each consumer uses the target action of a `calls` edge and the
containing model revision to locate that same contract. Contract identity is
`(project.id, action ID, spec_revision)`. Derived packets or views MAY copy a contract
while preserving its provenance; they MUST NOT create a separately editable authority.

The contract's abstract input/context/output types, errors, preconditions,
postconditions, effects, risk assessments and scenarios describe its obligations.
Successful-return guarantees belong in postconditions; failure guarantees and
effects belong in error descriptions and relevant rules/scenarios. Explicit empty
postconditions are representable, but their domain adequacy still requires review.
Pseudocode and contract prose MUST NOT be executed as a checker.

A resolved cross-component call MUST use a public action of its unique provider.
Same-component calls MAY use private actions. Missing owners or unspecified interfaces
remain unresolved until dependent implementation readiness. An explicitly empty
interface is different from an omitted interface. Re-export requires an explicitly
owned forwarding action; communities and refinement MUST NOT change action ownership.
Domain reads/creates/changes describe effects and do not automatically assert physical
access to private storage code. Precise access/import restrictions remain explicit rules.

## 3. Derived community boundaries

For each community, derive participating components from directly membered components
and unique owners of directly membered actions/entities/screens/machines. Record the
membership basis so direct and owner-derived participation can be distinguished.
Rules, scenarios, refinement and proximity MUST NOT supply implicit ownership.

The public view collects participating components' interfaces with provider/action
IDs and the model revision. For a call with resolved endpoint components, both
participating means internal, only consumer participating means outbound, and only
provider participating means inbound. Classify each community independently. An
interaction can be internal to one community and inbound to an overlapping community.

Unknown ownership/interfaces and scope omissions MUST be visible. A reduced view MUST
identify its selected context and omitted participating components; it MUST NOT claim
the whole community is analyzed. Membership establishes no permission, complete
allowlist, architectural rule or development independence.

The informative [boundary schema](../schemas/boundaries.schema.json) describes the
reference tool's projection. It introduces no authoritative fields on communities.

## 4. Concrete review questions

Agents MUST assess these four principles for a planned change and its combined result:

| Principle | Required assessment |
| --- | --- |
| KISS | Explain whether layers, abstractions and indirection serve current requirements. Prefer the smallest clear solution satisfying the contracts. |
| DRY | Examine duplicated authoritative business knowledge and the risk of divergence. Similar syntax alone does not require extraction; small local duplication can preserve clarity. |
| Separation of concerns | Examine whether decisions and state changes fit the responsible components and their exclusions. Make necessary cross-boundary effects explicit. |
| Modularity | Examine whether consumers use stable public contracts and whether changes require knowledge of other components' internals. |

Counts, cycles, similar code and graph size MAY indicate investigation opportunities;
they MUST NOT alone establish a universal violation. A shared rule can apply to distant
nodes and MUST remain available through context closure. Reuse of knowledge does not
mandate a new component or abstraction for every shared rule.

Rules MAY attach a `principle` label to provide project-specific guidance. These broad
judgments MUST be `category: design` and `blocking: false`. A precise requirement,
including an accessibility or import restriction, can be a blocking ordinary rule;
its actual verification criterion and method MUST be explicit. Broad review findings
MUST NOT silently become blocking acceptance requirements.

## 5. Review evidence and its limits

A delivered advisory review MUST identify scope, model revision, implementation and
code revision, reviewer identity/version, whether review was performed, and an
assessment of each principle. Each assessment gives a rationale and identifies
reviewed, unknown, or not-applicable status. Not-applicable requires a concrete reason.
Findings MUST identify relevant graph IDs, actual code locations, consequences and
proportionate recommendations. A reviewed assessment MAY have no findings.

The informative [review schema](../schemas/design-review.schema.json) defines the
reference envelope `gdd-design-review/0.3`. Existing check/evidence records can refer
to its raw artifact through `evidence_uri`; `design_review` is a checker method label,
not an instruction to execute a graph command. The host must independently verify
execution and applicability before treating any review claim as evidence.

The reference envelope checker reports COMPLETE, UNKNOWN or INVALID for structural
completeness and applicability to host-supplied revisions. COMPLETE does not mean
KISS/DRY compliance or implementation acceptance. Missing, planned, stale, incomplete
or unsupported reviews remain UNKNOWN when requested. Advisory findings remain
visible without altering the result of a separate blocking implementation check.

Contract or rationale changes produce a new model revision. Agents MUST reconcile
affected consumers, plans and checks before delivery. Historical review and execution
claims retain their original revisions. Review changes neither the project's existing
mandate nor its configured approval mode.
