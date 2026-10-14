# Validatorreview — 13. oktober 2026

GDD-validator 0.2.0 er implementeret mod det eksisterende GDD 0.2-udkast. Standardens normative regler, seks core-schemas og de 58 caseforventninger er bevaret. Statusbeskrivelser i dokumentationen er ajourført. Værktøjets host/context/parallel/import-envelopes er særskilte informative API-kontrakter.

## Faktisk udført

| Kontrol | Resultat | Evidens |
| --- | --- | --- |
| Standardens 58 conformance-cases | 58/58 forventninger matcher faktisk validatorudførelse | [validator-conformance.json](validator-conformance.json) |
| Pytest, inklusive conformance-harness og regressioner | 74 passed | [validator-tests.txt](validator-tests.txt) |
| Source distribution og wheel | Bygget | [validator-build.txt](validator-build.txt) |
| Installation af wheel i midlertidigt miljø uden for arbejdsområdet | CLI/readiness PASS; alle syv schemaassets med i pakken | [validator-wheel-smoke.json](validator-wheel-smoke.json) |
| Koncerteksemplets backend, action.reserve | model PASS, readiness PASS | [validator-concert.readiness.json](validator-concert.readiness.json) |
| Koncerteksemplets screen.reserve | readiness FAIL; utilgængeligt design bevares som UNKNOWN | [validator-concert.screen.json](validator-concert.screen.json) |
| Koncerteksemplets implementation | UNKNOWN; planer alene giver ingen udførelsesevidens | [validator-concert.records.json](validator-concert.records.json) |
| Implementation uden betroet mandat | authorization UNRESOLVED | [validator-concert.authorization.json](validator-concert.authorization.json) |
| Snapshot og backend-context | Samme snapshotrevision som standardeksemplet; 34 noder og 73 kanoniske edges | [snapshot](validator-concert.snapshot.json), [context](validator-concert.context.json) |

Kommandoer fra projektroden:

```sh
uv run pytest -q
uv run python conformance/run.py --output evidence/validator-conformance.json
uv build
uv run gdd validate examples/concert.gdd.yaml --profile readiness --scope action.reserve
uv run gdd validate examples/concert.gdd.yaml --profile records
```

Den installerede wheel blev prøvet med en separat midlertidig virtualenv og et working directory uden for repositorymappen. Ingen global installation eller publicering blev foretaget. [validator-assets.json](validator-assets.json) registrerer de konkrete implementation-/test-/standardassets og checksums for kontroloutputtet. Rapporter har normal versionsidentitet; denne separate manifestfil identificerer kontrolgrundlagets præcise bytes.

## Reviewfund og rettelser

Tre read-only underagenter gennemgik modelsemantik, scoped protokol og integrationer parallelt. Lead skrev og integrerede alle ændringer. Reviewet førte til regressionstests for:

- Dublerede JSON/YAML-keys, YAML 1.2-scalars, tilladte built-in tags, custom tags, aliases, ugyldig Unicode, uendelige tal og symlinkescapes.
- Rekursive typereferencer og optional-placering, ownerdubletter, lange acykliske grafer, edge-source-selectors og core-constraints.
- Extension-metaschemas før traversal; remote refs uden netværk; inert `$ref` under const og legitime datafelter som code.
- Parentens børn i context; ingen context-bypass af readiness; canonical edges; community er ikke et tomt implementationsscope.
- Aktuelle afslag under både explicit, standing og none; ubetroet/urelateret supersession kan ikke skjule dem; tilsvarende beskyttelse af failed evidens.
- Evidensbinding til relevant scope; revisionsbundne checkhashes; faktiske hostattesterede miljøer, negative kontroller og checkerproveniens.
- Manglende records påvirker ikke en beregnelig modelrevision; standalone records får ikke model-PASS alene på grund af schema-PASS; ugyldige API-scopes giver kontrolleret inputfejl.
- Kendte/ukendte kontraktformater, DESIGN.md-frontmatter og normaliseret skriveoverlap med eksplicit serialisering.

Agent-readable-code-linteren er gennemgået. De resterende fire advisory-fund er korte lignende diagnoseblokke og den lokale source-lineageberegning i CLI/pakkecheck. De er gennemgået som advisory-fund. Der er ingen filer over 800 linjer eller linjer over 400 tegn i implementationen. Der er ikke indført en ekstra abstraktion for at fjerne disse lokale ligheder.

## Scope og begrænsninger

Dette er strukturel og revisionsmæssig validering. Faglig pseudokode, faktisk scenariedækning og erklæringers sandhed kræver review eller checkere. 58 matchede forventninger er ingen fuld certificering af alle mulige standardkrav.

Condition-cases bruger syntetiske mandater, miljøattestationer og receipts. De dokumenterer validatorens adfærd; de er ikke faktisk autorisation eller evidens for en koncertapplikation. Ingen sådan applikation er bygget eller testet.

Graphify-profilen påviser tab mellem originalt og rekonstrueret råinput, men en produktions-Graphify-adapter er ikke implementeret og giver UNKNOWN. Visualization, custom-kind readiness, historiske snapshots, markdown-heading/opaque-selectorresolution og automatiseret policybestemt exceptionaccept er heller ikke implementeret. Afhængige checks er UNKNOWN eller kræver særskilt betroet hostinput som beskrevet i [VALIDATOR.md](../VALIDATOR.md).

Fase 1's standardreview og authoring-hashes er bevaret som historiske artefakter. De angiver ikke længere de aktuelle bytes i dokumentationsfiler, hvis status er opdateret i denne fase.
