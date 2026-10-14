# Eksisterende kode → Graphify → GDD-forslag

[code/reservation.py](code/reservation.py) er en lille kodefixture, ikke en koncertapplikation. Den er faktisk udtrukket af den installerede Graphify 0.9.75 uden en LLM-pipeline.

| Artefakt | Nodes | Edges | Produktion |
| --- | ---: | ---: | --- |
| [raw.ast.json](raw.ast.json) | 3 | 5 | extract_python |
| [export.directed.json](export.directed.json) | 3 | 5 | directed build + to_json(original_links) |
| [export.undirected.json](export.undirected.json) | 3 | 5 | undirected build + to_json(original_links) |
| [export.collapsed.json](export.collapsed.json) | 3 | 3 | simpel graf + to_json uden original_links |

De tre parallelle reserve→Reservation-records er parameter_type-reference, return_type-reference og calls. Den sidste eksport viser tab før adapterens input; adapteren kan ikke opfinde de manglende records. [producer-evidence.json](producer-evidence.json) registrerer faktiske input-/artefakthashes, pakkeversioner og producentkildehashes. Der er ingen kendt Git-koderevision eller udført forretningsverifikation.

[Observationsbundet](observations/observations.json) er importerens faktiske output fra raw.ast.json. [mapping.json](mapping.json) er en eksplicit, informativ fortolkning af modulet, reserve og constructoren. Den bruges til det [uaccepterede proposal](observations/proposal.json), som indeholder GDD-skitser og åbne kontraktspørgsmål. Det er ikke en automatisk genereret accepteret standardmodel.

```sh
uv run gdd validate examples/graphify/observations --profile import.graphify
uv run gdd graphify export examples/graphify/observations --output recovered.json
uv run gdd graphify propose examples/graphify/observations \
  --mapping examples/graphify/mapping.json --output proposal.json
```

Genproduktion af producentfixtures kræver et miljø med den pinnede Graphify 0.9.75 og dens AST-afhængigheder:

```sh
<graphify-python> conformance/generate_graphify_fixture.py
```

Graphify er ikke en runtimeafhængighed for GDD-adapteren. Se [GRAPHIFY.md](../../GRAPHIFY.md) for profiler, retning, tillidsgrænser og agentworkflow.
