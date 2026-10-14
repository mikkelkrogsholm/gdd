# GDD 0.2 JSON Schemas

Alle schemas bruger JSON Schema Draft 2020-12 og kan anvendes med en eksisterende schema-validator. De definerer repræsentation, ikke en komplet GDD-validator.

| Schema | Dokument |
| --- | --- |
| gdd.schema.json | Modelmanifest og sikkert indlæst YAML/JSON |
| fragment.schema.json | Lokale node-/edgefragmenter |
| extension.schema.json | Deklaration af custom kinds/relations og deres dataschemas |
| records.schema.json | Bindings, decisions, checkplaner og evidens |
| report.schema.json | Scoped checkresultater og diagnoser |
| snapshot.schema.json | Byteassets og modelsnapshot-identitet |

[validator-inputs.schema.json](validator-inputs.schema.json) er et informativt værktøjsschema med `$defs` for host, package, parallel og import. Det beskriver [validatorens](../VALIDATOR.md) API-envelopes og er ikke et syvende normativt GDD-dokumentformat. Det afledes fra `gdd_validator.validator.input_schema`.

[observations.schema.json](observations.schema.json) og [graphify-mapping.schema.json](graphify-mapping.schema.json) beskriver [Graphify-adapterens](../GRAPHIFY.md) observationsbundle og eksplicitte mapping til et forslag. De er værktøjsformater uden for de seks GDD-core-dokumenter. Originalen bevares ved siden af indexet; schema-PASS alene verificerer ikke integritet/projektion.

Duplicate YAML-keys, aliases, fragmentindlæsning, globale ID-kollisioner, referenceopløsning, endpoint-kinds, state semantics, extensionsemantik, readiness og autorisation kræver yderligere checks efter [VALIDATION](../standard/VALIDATION.md). Et schema-PASS må ikke præsenteres som model-PASS.

Schemas har lokale identifier-URI'er under `https://gdd.example/`; dette er ikke en publiceret service. De har ingen remote `$ref`-afhængigheder. Validering skal kunne ske offline med de vedlagte filer. `format: date-time` skal valideres med format-assertion aktiveret, hvis undtagelsesudløb kontrolleres.

Den valgte Draft 2020-12-kontrakt er beskrevet af [JSON Schema](https://json-schema.org/draft/2020-12). De konkrete GDD-felter og semantikker er dette udkasts egne forslag.

[view.schema.json](view.schema.json) og [view-state.schema.json](view-state.schema.json) beskriver [viewerens](../VIEWER.md) indlejrede payload og interaktive synlighedsmanifest. De er informative værktøjskontrakter. Modelfelter, præcise records/ID-partitioner og rendererbytes kontrolleres separat; snapshotidentiteten ændres ikke af visningstilstand.

[workflow.schema.json](workflow.schema.json) beskriver agent-workflowets request-, workplan-, packet- og event-envelopes. Det er et informativt værktøjsformat (`0.1`), ikke en ændring af GDD 0.2. Hele afledte planer verificeres mod aktuelle modelbytes og valgte checkplaner; schema alene beviser ingen pakkeintegritet, autorisation eller udførelse.
