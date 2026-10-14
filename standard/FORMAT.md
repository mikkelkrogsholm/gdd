# Filformat og revisionsprofil

Normativt kapitel til GDD 0.2. Felters maskinlæsbare form er specificeret i [schemas](../schemas/README.md). Semantiske regler gælder ud over JSON Schema.

## 1. Dokumenter og indlæsning

Et modelmanifest er JSON eller YAML med `format: gdd/0.2`, `project`, `nodes` og `edges`. Øvrige registries er valgfrie. Det anbefalede navn er `gdd.yaml`; navnet er ikke en semantisk identitet.

En YAML-consumer SKAL bruge YAML 1.2 JSON-kompatible scalars og sikre loaders. Kun JSON-værdier er tilladt: objekter med stringkeys, arrays, strings med gyldige Unicode-scalarværdier, endelige numbers, boolean og null. Duplicate mapping keys, brugerdefinerede tags, anchors, aliases og merge keys er forbudt. Der må ikke udføres constructors eller kode. En consumer, der kun understøtter JSON, SKAL deklarere dette.

Lokale `includes` peger på `gdd-fragment/0.2`-filer, som alene kan bidrage med nodes og edges. Fragmenter må ikke selv inkludere filer. Registry-ID'er og edge-ID'er må ikke overskrives. I 0.2 er bundle-root manifestets mappe. Includes, record_files, normative lokale sources og extensionpaths er relativt til manifestet og SKAL holde sig inden for denne root efter normalisering og symlinkresolution. Hvert asset indlæses én gang; cykler, dubletter og path escapes er fejl. Eksterne kilder bruger særskilte `sources`.

Node-, source-, question- og extension-ID'er er objektnøgler. Edges og transitions har `id`. ID-syntaks er små ASCII-bogstaver, cifre, underscore samt punktum/bindestreg mellem segmenter; første tegn er et bogstav. Unicode er tilladt i labels og tekst.

## 2. Manifest

| Felt | Betydning |
| --- | --- |
| format | Præcis formatversion, `gdd/0.2` |
| project | id, title, goal; valgfrit scope, assumptions, workflow |
| nodes | Map fra stabilt ID til node |
| edges | Liste af eksplicitte relationsrecords |
| includes | Relative lokale fragmentpaths |
| sources | Map over viden, input, design og observationskilder |
| questions | Map over scoped åbne eller løste spørgsmål |
| extensions | Map over deklarerede extensionassets |
| record_files | Relative paths til separate operational-records |

Workflow har `mode: manual|autonomous|custom`, et tekstligt `mandate` og gates for `model_changes`, `implementation` og `acceptance`. Hver gate har `requirement: explicit|standing|none`, `authorities` med actor-ID'er og `scope` med node-ID'er; tom scope betyder hele projektet. `none` betyder ingen særskilt GDD-gate, ikke ingen platform-/brugergrænser. `standing` kræver en udtrykkelig afgrænset autorisation i mandatet. Mode er en beskrivende label; gates og betroet mandat afgør handlingen. Der må ikke udledes ekstra tilladelser eller godkendelseskrav fra labelen.

Et manglende workflow er tilladt i en skitse. Afhængige autorisationsbeslutninger er UNRESOLVED, indtil betroet mandat foreligger. En fil i ubetroet input kan ikke tildele sig selv autoritet.

## 3. Noder og kontraktfelter

Alle noder: `kind`, `description`; valgfrit `label`, `maturity: sketch|specified`, `source_refs`, `annotations` og `realization`. Ukendte strukturelle felter afvises; ikke-normative udvidelser ligger i namespacede annotations.

- Component: `interfaces` er en liste af offentlige action-ID'er. En readiness-vurdering kræver, at relevante grænsekontrakter er tilgængelige.
- Entity: `fields` mapper feltnavne til abstrakte typespecifikationer.
- Action: `contract` og `behavior`. Contract har input/context fieldmaps, output-type, errors-map fra fejlnavn til beskrivelse, preconditions, external_effects, effects_complete, dependencies_complete, risks og error_cases.
- Screen: `interaction` beskriver states, accessibility, responsive og behavior; `design_refs` henviser til sources med `purpose: contract`. `calls` og `navigates_to` beskrives som edges.
- Rule: `rule` har category, statement, scope (`global|targeted`), blocking og verification (liste af metode-ID'er). En architecture-rule kan have constraint med relation/from/to/reach (`direct|transitive`)/policy (`forbid`). Metode-ID'er er labels til checkere, ikke kode der eksekveres.
- Scenario: `scenario` har category (`normal|negative|boundary|concurrency|architecture`), given, when og then.
- Machine: `machine` har states, initial, terminal og transitions. Hver transition har id/from/to/trigger/guard/effects. Alle transitions er muterende i denne profil.
- Community: fælles felter er tilstrækkelige; medlemskab beskrives kun i edges.

En contract SKAL i readiness-profilen opregne alle fem risikokategorier: authorization, idempotency, boundaries, concurrency og atomicity. Hver vurdering har relevant (boolean), rationale og scenarios (ID-liste). Når relevant er true, kræves mindst ét passende scenarie. Fravalg har en ikke-tom rationale. `error_cases` mapper hvert erklæret fejlnavn til mindst ét scenarie. Tilstedeværelse og referencer kan valideres; relevans og dækning kræver review.

Abstrakte typespecifikationer er objekter med `type`. Typer: string, integer, number, boolean, object, array, reference, enum og any. String kan have min_length; tal kan have minimum/maximum. Reference kræver target (entity-ID) og betegner en logisk entity-identitet, ikke et automatisk indlejret entity-objekt; fysisk ID-type mappes ved implementation. Enum kræver ikke-tom unik values-liste; array kræver items. Object kan have properties og required (navne skal findes i properties); manglende required betyder ingen obligatoriske properties. `origin: authenticated_context` kan markere trusted runtime-identitet. Det er et krav til implementationen, ikke en autentificeringsmekanisme. Typespecifikationer er deklarative; constraints skal være konsistente og gælde den valgte type.

Entries i contract.input, contract.context og entity.fields er obligatoriske, medmindre de har `optional: true`. Nullable vedrører værdien null, ikke feltfravær: `nullable: true` tillader null ud over typen. Default er optional=false og nullable=false. `any` betyder enhver ikke-null JSON-værdi, med null tilladt ved nullable=true. Optional må kun bruges på de tre navngivne fieldmaps; i object.properties styrer parentens required-liste feltfravær, og optional må ikke angives der. Optional må heller ikke stå på selvstændig output-type eller array.items. Output er altid én returneret værdi, medmindre en erklæret error indtræffer.

## 3a. Extensionassets

Manifestets extensionrecord har namespace, version, path og præcis sha256. Assetet er `gdd-extension/0.2` med samme namespace/version samt node_kinds og relations. Der må højst være én version pr. namespace i en model. Alle navne skal begynde med namespace plus punktum; kerneord må ikke overtages.

En node-kind beskriver description og data_schema. En relation beskriver description, from/to-lister af præcise kindnavne, context, dependency, effect og data_schema. Endpointnavne skal være kendte core-kinds eller deklarerede extension-kinds.

Context angiver bidrag til closure: outgoing følger relationen fra et inkluderet source til target; incoming følger fra inkluderet target til source; both gør begge dele; none tilføjer ingen closure. Dependency=true giver source en forudsætning til target for fuld realisering. Effect angiver source som læsende/oprettende/ændrende aktør på target (`read|create|change`), eller none. Det gør disse bidrag synlige i kontekst og parallelplanlægning; det eksekverer intet og gør ikke en custom kind substituerbar for en core-kind.

Data_schema er et JSON Schema Draft 2020-12-objekt for datafeltet. Referencer skal være lokale inden for dette schemaobjekt; remote refs, eksekverbare hooks og mutable schemaopslag er ikke tilladt. En consumer, der ikke understøtter denne deklarative semantik, skal rapportere UNKNOWN på afhængige checks.

`realization` er en liste med obligation (tekst) og children (node-ID'er). Børnene SKAL være descendants af noden via refines. Feltet må ikke bruges som bevis på, at parentkontrakten faktisk er realiseret.

## 4. Kilder og spørgsmål

Source har role (`input|knowledge|design|observation|code`), title, uri, format og normative (boolean). Revision kan indeholde sha256 og producer_revision. Producer kan have name/version. `derived_from` er records med source-ID og den faktisk anvendte revision (sha256 og/eller producer_revision). Ukendt revision udelades og giver UNKNOWN for lineagechecks; consumeren må ikke indsætte kildens senere revision som erstatning. Identiteten i en derivationssti er source+revision. En ældre revision af samme source kan være legitim; cykler mellem samme revisionsidentiteter afvises. Manglende historiske inputs giver UNKNOWN for afhængig lineagekontrol. En ekstern profilkontrakt kan pinnes i specification med uri/revision/sha256.

Source-ref har source-ID, purpose (`provenance|contract`) og valgfri selector. Selectors er `whole-document`, `json-pointer`, `markdown-heading`, `line-range` eller `opaque`. Value-syntaks: JSON Pointer følger [RFC 6901](https://www.rfc-editor.org/rfc/rfc6901); Markdown-heading er den nøjagtige headingsti adskilt med ` / `, med valgfri 1-baseret ordinal; line-range er `Lstart-Lend` med end ≥ start og præcis kildehash; opaque er bevaret ufortolket tekst. Kendte men uunderstøttede selectors giver integrations-UNKNOWN; en ikke-konformant selector-kind er schema-FAIL. YAML tokens refereres gennem JSON Pointer efter sikker YAML-parsing; der defineres ikke et nyt ustandardiseret yaml-path-sprog.

Et deklareret source-ID med ulæseligt eksternt mål er ikke en dangling intern reference. Det berørte source-check er UNKNOWN. Før readiness SKAL enhver faktisk anvendt purpose=contract-reference, inklusive design_refs, bruge normative=true, lokal materialiseret source og en verificeret revision.sha256, så assetet indgår i modelsnapshot. Planlagte nonnormative kontraktrefs er tilladt i skitser. Provenienskilder kan bevares som utilgængelige med synlig diagnose.

Question har statement, targets, blocks (`modeling|implementation|acceptance`), status (`open|resolved`) og ved resolved en resolution med decision-record-ID. En resolution er ikke tilstrækkelig, hvis beslutningen mangler eller ikke gælder den aktuelle ændring.

## 5. Operational records

`gdd-records/0.2` har project_id og records-map. Det er adskilt fra autoritativt modelsnapshot. Recordkind er:

- binding: spec_revision, targets, implementation, role (`code|test|design`), resource, selector og status (`planned|observed|checked`). Planned kan mangle ressourcerevision. Observed/checked kræver præcis ressourcerevision, og ved code/test også code_revision for den konkrete implementation. Overlap mellem bindings kan være legitimt; tvetydig mapping og konkurrerende skriveejerskab er forskellige fejl.
- decision: topic (`model_change|approval|readiness_review|exception|source_promotion`), scope, spec_revision, policy_revision, actor (id/kind), outcome (`proposed|accepted|rejected`) og rationale. Approval kræver activity (`model_changes|implementation|acceptance`). Acceptancereceipts og exceptions kræver også implementation og code_revision. Exceptions kræver expires_at og henvisning til de berørte checks. Receipt-identitet er en registreret påstand; hosten skal kontrollere aktørens autoritet.
- check: spec_revision, scope, method, required_environment og negative_controls. Det er en plan; method skal være understøttet af den udførende checker.
- evidence: check-ID og check_revision, implementation, spec_revision, code_revision, checker(name/version), scope, result, executed, reason, environment og evidence_uri. PASS kræver executed=true og tilstrækkeligt input. Skemaet beviser ikke, at en oplyst kørsel fandt sted.

Records er uforanderlige efter registrering. Ændring kræver et nyt record-ID; `supersedes` kan referere til det tidligere record af samme kind og skal være acyklisk. Checkplanens check_revision er `sha256:` plus hash af recordværdien serialiseret som UTF-8 JSON med sorterede objectkeys, ensure_ascii=false, separators=(',', ':') og uden newline. Evidens SKAL matche denne hash. Flere aktive planer for samme rule/metode kræver eksplicit valg; de må ikke sammensmeltes stiltiende.

Check- og binding-targets SKAL referere til model-ID'er i deres spec_revision-snapshot. Historiske records skal ikke resolves mod den seneste model, når noder er slettet. Bindingrevisionen identificerer ressource/kodegrundlaget. Decision-checks og evidence-check SKAL referere til check-records i samme projekt. Evidens og checkplan skal have samme spec_revision. Model-/code-revisioner, checkhash og raw evidence skal kontrolleres, før resultater bruges til accept.

## 6. Revisioner uden selvreference

Den portable profil `gdd-snapshot/0.2` indeholder assets: en liste af `{path, sha256}`. Paths er normaliserede relative POSIX-paths, sorteret stigende efter Unicode code point. Listen omfatter manifestet, alle fragmenter, anvendte extensionassets og sources med normative=true. Alle er lokale snapshotassets; nødvendige eksterne normative kilder skal først materialiseres eller give UNKNOWN. Der må ikke være dubletpaths.

Manifestidentiteten er `sha256:` efterfulgt af SHA-256 over UTF-8-bytes af en JSON-liste af `[path, sha256]`-par med `ensure_ascii=false`, ingen whitespace (`separators=(',', ':')`) og ingen newline. Assethashes er over de præcise filbytes. Rapportfilen, operational records og selve snapshotmanifestet er ikke i assetlisten. Paths og modelbyteændringer giver ny revision; graf-ID'er bevares. Dette er en reproducerbar byteprofil, ikke semantisk ækvivalens eller en digital signatur.

Et label eller en Git-commit MÅ være ekstra locator, men må ikke erstatte identifikation af relevante lokale ændringer. Kodesnapshot bruger særskilt implementation-ID og revision; flere implementationer må ikke dele evidens implicit.

## 7. Versionering

0.2 er eksplicit uforenelig med 0.1. Ingen silent upgrade. En consumer SKAL deklarere understøttede formater og extensionversioner. Fremtidige ændringer med nye krav til tolkning skal have ny formatversion. Schema-ID'ernes `https://gdd.example/` er et lokalt identifier-namespace, ikke en publiceret tjeneste; schemas skal resolves fra denne pakke uden netværk.
