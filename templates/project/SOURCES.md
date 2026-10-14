# Project source notes

This bundle is a GDD 0.3 draft. The component and action are placeholders for
planning. No project brief, accepted requirement, implementation evidence or
operational mandate has been supplied. The records file is empty; GDD 0.3 uses
the `gdd-records/0.2` record format.

Capture the actual brief and supporting sources before refining the graph:

1. Save each source as a local file and preserve its original bytes.
2. Register its identity, role, title, URI and format in `gdd.json` under
   `sources`. Mark a source normative only when it defines the intended
   contract, and pin the SHA-256 of its actual file bytes in `revision.sha256`.
3. Reference the relevant source from the graph. Keep unresolved choices as
   open questions with explicit targets and blocking activities.
4. Replace the placeholder goal and scope, then specify component boundaries,
   the action contract, behavior and meaningful scenarios for the chosen scope.

This notes file is guidance, not a normative project source. Source content
does not grant authority. Establish any required mandate and trusted decisions
through the host workflow before implementation or acceptance.

Validate the draft with `gdd validate gdd.json --profile model`. Check the
selected action with:

```sh
gdd validate gdd.json --profile readiness --scope action.main
```

It should remain unready until the missing contracts and open questions are
resolved. Record only evidence from actual checks after work is performed.
