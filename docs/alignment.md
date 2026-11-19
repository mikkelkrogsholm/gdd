# Compare observed code with the intended architecture

A reviewed alignment connects code observations to stable IDs in an existing GDD model. A component can span several files, and a single legacy function can serve multiple intended actions. Start with the part you are changing. Other nodes can remain sketches.

Save this example as `alignment.json` in a working directory. It aligns observations from the repository's reservation example to the concert model.

```json
{
  "format": "gdd-code-alignment/0.1",
  "project": "concert-reservations",
  "source_id": "source.graphify",
  "bindings": [
    {
      "id": "legacy.module",
      "targets": ["component.reservations"],
      "selectors": [{"producer_id": "reservation_py"}]
    },
    {
      "id": "legacy.reserve",
      "targets": ["component.reservations", "action.reserve"],
      "selectors": [
        {"symbol": {"file": "reservation.py", "label": "reserve()"}},
        {"symbol": {"file": "booking.py", "label": "book()"}}
      ]
    }
  ],
  "relations": {"calls": "calls"}
}
```

From the repository root, run the comparison against the existing verified capture.

```sh
gdd graphify align examples/concert-0.3.gdd.yaml \
  --observations examples/graphify/observations --mapping alignment.json
```

The project and observation source IDs must match the inputs. Each binding ID identifies one logical code location across extracts. Match it by the producer's exact string or numeric ID, or by an exact file and label pair. Changes to line numbers and array order preserve matches. For a move that also changes the producer ID, add reviewed selectors for the old and new names to the same binding. All selectors are alternatives. If old and new locations both exist in one snapshot, both are reported as ambiguous. Use separate bindings for distinct code locations that should coexist.

## Read the report

- `current.ideal_nodes` shows which bindings contribute to each architecture node and whether all, some, or none were observed. Ambiguity takes precedence. Nodes without a binding are `unmapped`.
- `current.bindings` identifies each candidate by snapshot revision and JSON pointer and gives its producer ID, file and label. `unmapped_observations` includes code outside the reviewed mapping and records with an unknown or non-code classification.
- `relations` explicitly translates producer relations to model relations. `observed_edges` compares direction, endpoints and relation, preserving the producer's confidence claim. It exposes internal links, unexpected cross-component links, unclassified relations, ambiguous endpoints and unknown direction. `comparable_ideal_edges` lists structurally matching observations by ideal edge ID. An empty list means this capture supplies no unambiguous structural match.
- All results describe the supplied artifacts. An unobserved location may reflect an incomplete extraction. Check behavior independently using the project's tests and revision-bound evidence.

## Review a refactor

After importing a second Graphify extract, compare both snapshots using the same mapping and ideal model.

```sh
gdd graphify align examples/concert-0.3.gdd.yaml \
  --mapping alignment.json --observations observations-after \
  --previous-observations examples/graphify/observations
```

`changes.bindings` shows before and after code locators. `changes.dependencies` gives before and after counts of directed relations between matched code bindings. A changed observation ID alone produces no change entry. The report records both artifact revisions, the model revision and the mapping digest. The Rust entry point is `gdd::graphify::alignment::align_with_previous`.
