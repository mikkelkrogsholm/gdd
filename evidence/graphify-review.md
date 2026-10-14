# Graphify-adapter — udført kontrol, 13. oktober 2026

Leveret: Graphify-adapter 0.1.0 i gdd-validator 0.3.0 til GDD-standardformat 0.2. Adapteren indlæser to eksplicitte 0.9.75-profiler, bevarer rå bytes/metadata og hvert arrayrecord, validerer revision/projektion/retning og kan forberede en GDD-skitse gennem en eksplicit mapping.

## Faktiske resultater

| Kontrol | Resultat | Evidens |
| --- | --- | --- |
| Hele pytest-suiten | 143 tests består | [tests.txt](graphify/tests.txt) |
| Oprindelige standardcases | 58/58 forventninger matcher; ingen forventninger svækket | [standard-conformance.json](graphify/standard-conformance.json) |
| Nye Graphify-adaptercases | 18/18 forventninger matcher | [adapter-conformance.json](graphify/adapter-conformance.json) |
| Faktisk Graphify 0.9.75-AST og eksport | 3 nodes / 5 edges; original_links bevarer 5; upstream simpel eksport har 3 | [producer-evidence.json](../examples/graphify/producer-evidence.json) |
| Byte-roundtrip | Recovered AST er byteidentisk med originalfixture | [recovered.ast.json](graphify/recovered.ast.json), [rapport](graphify/roundtrip.report.json) |
| Markerløs undirected eksport | Rå bevarelse/layout PASS, direction UNKNOWN | [undirected.report.json](graphify/undirected.report.json) |
| Uaccepteret continuation | Gyldig GDD-kandidat med skitser, provenance, åbne spørgsmål og uverificerede bindingskandidater | [mapping](../examples/graphify/mapping.json), [proposal](../examples/graphify/observations/proposal.json) |
| sdist og wheel | Bygger | [build.txt](graphify/build.txt) |
| Installeret wheel uden for arbejdsområdet | Import/validate/eksakt export/eksplicit proposal består; ni schemaassets; ingen Graphify runtimeafhængighed | [wheel-smoke.json](graphify/wheel-smoke.json), [kørselskilde](graphify/wheel-smoke.source.py) |

Producentfixtures blev faktisk kørt gennem `extract_python`, `build_from_json` og `to_json` i den installerede Graphify 0.9.75. Inputkode, producer-/afhængighedsversioner, relevante packagekildehashes og outputhashes er registreret. Ingen LLM-pipeline eller forretningsapplikation blev kørt. Code_revision er ukendt; den erstattes ikke med artefaktets hash.

## Parallelle reviews og rettelser

Tre read-only underagenter undersøgte producentformat, observations-/modelkontrakt og conformance parallelt. De gennemgik derefter de konkrete ændringer. Lead skrev og integrerede alle filer.

Reviewfund blev rettet med regressioner:

- Numerisk afrunding kunne forbinde to forskellige producent-ID'er. Parseren kontrollerer nu decimalværdien før referenceopløsning; præcision uden for projektionen giver UNKNOWN med bevarede originalbytes.
- Versions-/revisionspåstande under graph blev overset. Kendte claims opregnes nu med faktiske JSON Pointers på begge niveauer; uunderstøttede eller modstridende claims bliver UNKNOWN.
- `_src/_tgt` kunne modsige en directed-grafs source/target. Konflikten afvises; markerrestaurering er fortsat mulig for undirected storage.
- Generisk rapportoutput kunne overskrive input. CLI-resultatfiler oprettes nu eksklusivt, også over for symlink/hardlink-aliaser.
- Projektionens rapporttekst lovede for meget om caller-metadata. Raw-afledte felter genberegnes; callerclaims er udtrykkeligt uverificerede inputs, ikke autentificerede receipts. Koordinerede ændringer af dem giver ingen verified binding eller implementeringsevidens.
- Raw-path er fastlagt som graphify.raw.json i denne værktøjsversion; alternative/escaping paths afvises før læsning, og escaping symlinks afvises.

Desuden er identiske multiedges, producer-ID-dubletter, typed IDs, arrays med både links/edges, hyperedges/communities, opaque fields, JSON-/Unicode-fejl, docs kontra code, tamper og no-clobber testet. Et manglende profilvalg eller manglende input giver UNKNOWN frem for en gættet fortolkning.

Agent-readable-code-linteren er kørt som advisory. [Output](graphify/agent-lint.json) viser korte nærduplikater i diagnose-/schemafelter; ingen runtimefil er over 800 linjer, og der er ingen AR004/AR006/AR008-fund. Der er ikke indført ekstra generiske abstraheringslag for at fjerne lokal lighed.

## Scope og begrænsninger

Tabsfrihed gælder **det modtagne artefakt**. Adapteren kan ikke genskabe upstream-deduplicerede relationer eller bevise producentens fuldstændighed. Ukendte layouts/versioner, uklar originalretning, ukendt kodeklassifikation og numerisk præcision uden for den støttede projektion er synlige UNKNOWN-resultater.

`propose` laver et separat forslag med eksplicit rationale. Det ændrer ikke eksisterende accepteret intention og opretter ingen godkendelser, autonome workflowgates, observed/checked bindings eller udført kodeevidens. Promotion sker gennem projektets normale betroede mandat og modelændringsproces. Import-PASS er ikke en påstand om, at den eksisterende kode opfylder GDD-regler.

De seks core-schemas og de 58 oprindelige forventninger er bevaret. De to gamle importconditions materialiseres nu med faktiske JSON-layouts; yderligere 18 cases er tilføjet separat. Nye observations-/mappingschemas er værktøjskontrakter uden for GDD-kernen. Renderer og produktionsapp ligger fortsat uden for denne opgave.

[GRAPHIFY.md](../GRAPHIFY.md) er bruger-/agentindgangen. [graphify-assets.json](graphify-assets.json) registrerer de konkrete leverance-, fixture- og kontrolbytes. Tidligere 0.2.0-evidens og distributioner er bevaret som historik.
