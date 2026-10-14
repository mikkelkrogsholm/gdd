# Graphify → GDD

Adapter 0.1.2 i `gdd-validator` 0.4.1 importerer Graphify som et separat observationslag. Den bevarer det modtagne artefakts bytes og indekserer hver node og relation uden en Graph/DiGraph-builder. Agenten kan derefter vælge en eksplicit fortolkning og forberede et uaccepteret GDD-forslag. Kendte 0.1.0-/0.1.1-bundles genvalideres efter de aktuelle regler; importerens versionsfelt omgår ingen kontrol.

```sh
uv run gdd graphify import graphify-out/graph.json \
  --profile graphify-0.9.75-node-link --output observations
uv run gdd validate observations --profile import.graphify
uv run gdd graphify export observations --output recovered.graph.json
```

`recovered.graph.json` er byteidentisk med det importerede input, inklusive whitespace, unknown fields og rækkefølge. Outputfiler og bundles oprettes eksklusivt: eksisterende filer overskrives ikke. Vælg et nyt outputnavn ved genkørsel.

## Bundle og identitet

| Fil | Indhold |
| --- | --- |
| graphify.raw.json | Præcis kopi af det modtagne input; ingen normalisering eller sorting |
| observations.json | Agentlæsbart indeks, rå payloadprojektioner, revisionsbundne identiteter, producentpåstande og bindingskandidater |
| import.report.json | Faktiske checks i `gdd-report/0.2`; modelrevision er null, fordi import ikke skaber en autoritativ model |
| README.md | Observationslagets betydning og tillidsgrænse |

En observationsrecords kanoniske identitet er `(artifact_revision, JSON Pointer)`, eksempelvis `sha256:…` og `/links/4`. Det korte `observation.edge.h…`-ID afledes deterministisk af denne identitet. Producentens eget ID og dets JSON-type bevares separat. Nye bytes eller ny arrayrækkefølge giver nye snapshotidentiteter; de er ikke stabile symbol-ID'er gennem re-extraction.

To identiske relationer får hver sin record. Et `multigraph:false`-flag bruges aldrig som begrundelse for at deduplikere et råt array. Communities, `community=0`, top-level/nested hyperedges og ukendte metadata bevares. Hyperedges ekspanderes ikke til en clique.

## Eksplicit understøttede profiler

| Profil | Inputkontrakt |
| --- | --- |
| graphify-0.9.75-ast | `nodes` og `edges` fra den undersøgte extractor. Noder har id, label, file_type og source_file; edges har source, target, relation, confidence og source_file. Confidence-score er valgfri. Rå source→target-orden har den deklarerede AST-profils retning, når intet directed-flag modsiger den. |
| graphify-0.9.75-node-link | `nodes`, `links`, boolean directed/multigraph og object graph fra JSON-eksporten. Hyperedges, communities, norm_label, ekstra felter og commitpåstande bevares. |

Profilerne bygger på den faktisk installerede **Graphify 0.9.75** og dens kildehashes i [producer-evidence.json](examples/graphify/producer-evidence.json). De er ikke et løfte om alle Graphify-versioner eller alle upstream-ruter. [Den undersøgte exporter](https://github.com/Graphify-Labs/graphify/blob/48d7c0e832cd2d67d86850e716ddea16df6238ea/graphify/export.py) og [builder](https://github.com/Graphify-Labs/graphify/blob/48d7c0e832cd2d67d86850e716ddea16df6238ea/graphify/build.py) forklarer `original_links` og simpel grafstorage.

Den undersøgte exporter skriver ikke selv en Graphify-/schema-version. Derfor skal layoutprofilen vælges eksplicit; den opfinder ingen verificeret producentversion. Valgfri `--producer-version 0.9.75` og `--code-revision ...` gemmes som **uverificerede callerclaims**. Kendte producentclaims læses både på topniveau og under graph med deres faktiske JSON Pointers. Ukendte/konflikterende versions- eller revisionspåstande giver UNKNOWN. Inputs med både edges og links fortolkes ikke ved at vælge et array lydløst.

Strenge, heltal og sikkert bevarede finite numeriske identiteter understøttes uden stringcoercion; `1` og `"1"` er forskellige. Bool/null/object-identiteter er ugyldige. Tal, som den anvendte JSON-projektion ville afrunde til en anden decimalværdi eller ikke kan konvertere, giver UNKNOWN før referenceopløsning; originalbytes bevares og kan stadig eksporteres. JSON-konstanterne NaN/Infinity er ugyldige og giver FAIL.

Raw-projektionen understøtter JSON-containerdybde op til 256: root-object/array tæller 1, hvert indlejret object/array yderligere 1, scalars og keys tæller ikke. Større dybde eller udtømt runtimekapacitet giver UNKNOWN/FORMAT_UNSUPPORTED med et kompakt byteidentitetsindeks uden producerrecords. Originalen bevares til hashkontrolleret eksport; kompakte dybdeindeks giver ingen verificeret producerprojektion eller brugbare model-forslag. Læsbare historiske fulde dybe indekser vurderes som UNKNOWN i projektionskontrollen. Et indeks, som runtime slet ikke kan indlæse, kan ikke levere en verificeret originalhash til eksport.

## Hvad PASS betyder

Rapporten holder rå integritet, layout, noder/referencer, retning, kodekandidater og projektion adskilt. Intakte originalbytes kan have PASS, mens retningen er UNKNOWN. FAIL vinder i aggregatet, men uafklarede checks bevares.

Ved `directed:false` i en node-link-export bevares stored source/target-orden. Uden intakte `_src`/`_tgt` kan denne profil ikke fastslå den oprindelige retning og rapporterer UNKNOWN. Markører kan genfinde retningen i undirected storage; modstridende markører i en eksplicit directed graf afvises. Der dannes ingen syntetiske reverse edges.

Exit 0 betyder beståede anmodede importchecks. Exit 2 betyder, at inputtet er bevaret, men noget krævet ikke kan afgøres, eksempelvis retning eller layout. Exit 1 betyder kendt fejl, eksempelvis tvetydige endpoints, korrupte bytes eller ændret index. Eksport kan genskabe intakte bytes fra et input med ukendt/ugyldigt layout; den medfølgende rapport beholder det oprindelige semantiske resultat. Et korrupt index/originalhash kan ikke eksporteres gennem denne operation.

Kodekandidater kræver producentens file_type=code og en understøttet kodefilendelse. Dokumenter/papers/images/viden er separate observationer. Dette er en kandidatsortering, ikke verificeret kildeeksistens eller implementationsevidens. `source_file/source_location` er uverificerede locatorpåstande; artifacthash er hverken sourcefilehash, Git-revision eller GDD-modelrevision.

Raw-afledte indexfelter bliver genberegnet ved validering. Ændringer i records, klassifikationer, Pointers eller producentclaims opdages mod originalen. Callerclaims er derimod aktuelle inputs til projektionen og autentificeres ikke; heller ikke koordinerede ændringer af dem giver verificeret kode eller autoritet. `input_claims.trust` og bindingskandidaternes status er altid unverified. Bevar/pin artifactrevisionen gennem projektets betroede proces, når identiteten skal kunne sammenholdes med en tidligere accepteret snapshot.

Adapteren kan bevare alt i det modtagne input. Den kan ikke genskabe information, som Graphify allerede har fjernet før eksport. Den faktiske fixture viser 5 rå AST-edges og 5 ved eksport med original_links, men kun 3 gennem den simple builder/eksportvej. Dette er dokumenteret i [eksemplet](examples/graphify/README.md).

## Arbejd videre gennem agenter

Agenten læser observations.json og originalen, afklarer ønsket adfærd og vælger fortolkningen i et separat mappingdokument:

```sh
uv run gdd graphify propose examples/graphify/observations \
  --mapping examples/graphify/mapping.json --output reviewed-proposal.json
```

Mappingen opregner projektets id/title/goal, valgte observationer, ønskede model-ID'er/kernetyper og rationale. Relationer kræver eksplicit relation, ID og endpoints. Typer som function/class bliver ikke automatisk til action/entity. En observeret calls-edge bliver ikke automatisk en normativ GDD-edge.

Resultatet er `gdd-graphify-proposal/0.2` med `status: proposed`, `model_candidate`, den eksplicitte mapping, udeladte observations-ID'er og uverificerede bindingskandidater. Kandidaten er en gyldig GDD-skitse med provenance-ref til råinput, role=observation/normative=false og blokerende kontraktspørgsmål. Den får ingen workflowtilstand, accepted receipt, checkevidens eller opdigtet spec_revision.

Agenten kan derefter anvende forslaget som en almindelig autoriseret modelændring under projektets eksisterende mandat. Source-promotion/model-change skal være synlig; eksisterende accepterede krav ændres ikke ved import/propose. Source-URI'en peger relativt fra proposalfilens placering; materialisér model_candidate dér eller justér locatoren eksplicit. Først når targets, implementation, modelsnapshot og faktiske kilde-/koderevisioner kendes, kan en binding registreres som observed/checked.

```sh
uv run gdd validate examples/concert.gdd.yaml --profile model \
  --profile import.graphify --graphify examples/graphify/observations
```

Den sidste kommando samler separate model- og importchecks. Den beviser ikke, at koncerteksemplet er implementeret af den importerede fixture, eller at importerede codeclaims opfylder GDD-regler. Sådanne mappings og checks kræver revisionsbundne bindings og faktiske checkere.

## API og verifikation

```python
from gdd_validator import import_graphify, verify_graphify, export_graphify, propose_graphify

report = import_graphify("graph.json", "observations",
                        profile="graphify-0.9.75-node-link")
report = verify_graphify("observations")
export_graphify("observations", "recovered.json")
proposal = propose_graphify("observations", explicit_mapping)
```

Observations- og mappingschemas er [observations.schema.json](schemas/observations.schema.json) og [graphify-mapping.schema.json](schemas/graphify-mapping.schema.json). De er værktøjskontrakter uden for GDD-kernen. `source_uri` i proposal-API'et er en eksplicit locator; default er graphify.raw.json ved en model materialiseret i observationsmappen. Mapping og locator skal kunne materialiseres som JSON med gyldig UTF-8. Forslag kræver tilgængelige originalbytes og bestået integritets-/projektionskontrol.

```sh
uv run pytest -q
uv run python conformance/run.py
uv run python conformance/run_graphify.py
uv build
```

De oprindelige 58 forventninger er bevaret. De to oprindelige importconditions har nu konkrete Graphify-JSON-fixtures. Yderligere [18 adaptercases](conformance/graphify-cases.yaml) og pytest kontrollerer positive/negative imports, raw roundtrip, metadata, dubletter, precision, direction, tamper, CLI og eksplicit continuation. [Graphify-reviewet](evidence/graphify-review.md) indeholder faktiske resultater og resterende begrænsninger.
