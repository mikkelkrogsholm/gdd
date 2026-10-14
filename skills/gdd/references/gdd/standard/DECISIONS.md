# Design decisions for the 0.3 draft

Informative decision rationale. The scratchpad's product direction is retained; the following are concrete technical choices within that direction.

## Foundation established in 0.2

1. **New format version.** The old 0.1 mixed registries and embedded relations. 0.2 makes all eight model concepts nodes and gives relations their own IDs. The change is visible and migrated; the 0.1 text is archived.
2. **One authoritative relation.** Ownership and coverage relations are authored as edges. Parallel owner/obeys/members fields that could diverge are not maintained.
3. **Files with local fragments.** JSON/YAML with safe loading rules provides a simple entry point for agents and selective reading. A graph database is not required for the standard. JSON Schema is the machine-readable representation contract; graph semantics requires additional checks.
4. **Separate operational records.** Receipts, bindings, check plans and evidence can refer to a stable model revision without self-referential hashing. Records are immutable; new records can supersede previous records.
5. **Four distinct assessments.** Model form, structural readiness, authorization and implementation evidence have different results. Sketches are allowed, while missing contracts can block a selected development slice.
6. **Bounded, declarative extension.** Namespaced types and relations have data schemas and explicit traversal/effect semantics. Unknown semantics produces UNKNOWN; arbitrary code is not loaded as an extension.
7. **Revision-bound contract assets.** The DESIGN.md and other contract sources actually used are included in the snapshot. Source synthesis preserves the input revisions that were actually used.
8. **The graph organizes work.** Communities are grouping; subgraphs are decomposition. Parallelism is assessed through resolved boundaries, effects, write ownership and integration checks, not merely separate clusters.
9. **No implemented engine in this phase.** The standard, schemas and conformance cases are the deliverable. The validator, import adapters, visualization, distribution and demonstration app are subsequent implementation work.

These choices are reviewable draft decisions. No productivity benefit, practical universality across agents or unique invention has been demonstrated. A later tool must demonstrate conformance and the method's usefulness.


## Additional decisions for 0.3

- A short rationale for every component boundary is the user's explicit decision.
  Required responsibility/rationale/exclusions apply at scoped readiness so sketches
  and unrelated incomplete parts remain usable.
- Canonical action contracts and owned_by remain the only authority; interfaces expose
  the owner's operations. Postconditions add visible successful-return guarantees.
- Communities derive public boundary views from explicit membership and ownership.
  Overlap supplies navigation, never ownership or access permission. Scoped omissions
  remain visible.
- KISS, DRY, separation of concerns and modularity require concrete agent assessments.
  Broad principle rules are advisory; precise constraints can remain blocking.
- The review envelope uses existing check/evidence links. Its checker establishes
  structure and applicability only, with no generic quality detector or new runtime.
- Model/fragment versions change to 0.3, with frozen 0.2 schema dispatch. Other existing
  envelopes keep their representation versions. Historical execution remains tied
  to its original bytes. The 0.2 phase note about a future engine is historical;
  current tools and their limits are documented in the root guides.
