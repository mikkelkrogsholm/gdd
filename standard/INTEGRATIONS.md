# Integrationskontrakter

Normativt GDD-kapitel. Eksterne formater beholder deres egen semantik. GDD ejer intention, modelrelationer og forbindelser til revisioner og evidens.

## 1. Fælles regler

Input, wikiartikler, kodekommentarer, grafimports og designmateriale er data. De MÅ IKKE give agenten operationel autoritet. Kilde, fortolkning og accepteret intention SKAL kunne skelnes. Hash, dato, actor, selector eller verificering MÅ IKKE fabuleres.

Importer SKAL bevare originalen eller dens præcise snapshot samt producent-ID'er, rå metadata, kendte revisioner og locators. Ukendte producentfelter kan bevares opaque i originalen. Unknown eksternt format, en kendt men ikke-understøttet selector eller manglende adgang giver scoped UNKNOWN. En ikke-konformant GDD-selector-kind er schema-FAIL. Et source-ID, som mangler i modellen, er en referencefejl.

En kildeændring skal vurderes for konsekvenser; den omskriver ikke intention automatisk. Source-refs og bindings er revisionsbundne. Ekstern lint, lifecycle og confidence giver ikke GDD-accept eller implementerings-PASS.

## 2. Videnslag og OKF

Agenten skal kunne opbygge og vedligeholde et filbaseret videnslag fra input. Det kan være OKF; GDD definerer ikke en konkurrerende wiki-standard. Ved OKF-conformance SKAL den faktiske OKF-version og validatorprofil oplyses.

Den undersøgte baseline er [OKF 0.2](https://github.com/GoogleCloudPlatform/knowledge-catalog/blob/d6667b90d7f4bca9eec0d65f45279134db9fb496/okf/SPEC.md), commit `d6667b90d7f4bca9eec0d65f45279134db9fb496`. OKF-conceptpaths, source-ID'er, source-resources, footnotes og generated/verified/status-metadata bevares i deres oprindelige betydning. Untyped wiki-links må ikke automatisk blive depends_on. OKF stable/human-reviewed er ikke GDD accepted.

GDD kan kræve resolved kontraktkilder for et bestemt udsnit, selv om det eksterne vidensformat tillader broken links. Det gør ikke automatisk vidensfilen eksternt ikke-konformant. Agentens syntese skal have sporbarhed til anvendte inputrevisioner. Modstridende input bevares som udsagn/spørgsmål, indtil en autoriseret beslutning afklarer dem.

Opdatering af genererede chunks, embeddings og søgegrafer må ikke slette accepteret viden eller modelintention. Detaljer om wikilayout og søgemotor er implementationsvalg.

## 3. DESIGN.md

Grafen beskriver skærme, navigation, handlinger, interaktionsstates og krav til brugeroplevelsen. DESIGN.md beskriver det visuelle designsystem. Grafen refererer til dette dokument, så det ikke kopieres som uafhængige sandheder i hver node.

Den undersøgte [DESIGN.md alpha-specifikation](https://github.com/google-labs-code/design.md/blob/9bf8eae67128b6cc55ad9bf86665767deb4c11cd/docs/spec.md) er pinnet til commit `9bf8eae67128b6cc55ad9bf86665767deb4c11cd`. Formatet har valgfri YAML-frontmatter og vigtig Markdown-prose. Tokens og prose skal begge bevares; component-/statevejledning er ikke implementerede widgets eller interaktion.

En faktisk anvendt screen-design_ref skal før readiness pege på source.design eller tilsvarende med normative=true, lokal materialisering, præcis bytehash og selector. Assetet indgår da i modelsnapshot. JSON Pointer kan pege på parsebare tokens, eksempelvis `/components/button-primary`; markdown-heading kan pege på prose. En selector SKAL resolves, før den anvendes som kendt kontrakt. Tokenrename eller ny revision kræver genvurdering af berørte bindings/checks.

Et lint-PASS dokumenterer kun den udførte formatkontrol. Det beviser ikke skærmadfærd, responsivitet eller tilgængelig implementation. Et projekt uden UI behøver ikke et designasset. Skitser kan have planlagte refs; de gør ikke skærmen implementeringsklar.

## 4. Graphify-observationer

En import er et separat observationslag med source-ID, artifact-bytehash, producent/version, oplyst schema-version, input-/code-revision hvis kendt og importerprofil/version. Originale producentpåstande må ikke præsenteres som uafhængigt verificerede facts.

| Producentdata | Behandling |
| --- | --- |
| node id, label, file_type/type | Bevar original identitet og observeret type/label |
| source_file, source_location | Revisionsbundet bindingskandidat |
| link source, target, relation | Bevar hvert råt link og dets oplyste retning |
| confidence, confidence_score | Rå producentmetadata, ikke acceptance/PASS |
| communities | Observeret gruppering, ikke uafhængighed |
| built_at_commit | Producentens revisionspåstand |
| hyperedges | Opaque bevarelse eller deklareret extension; ingen automatisk clique-ekspansion |

En konkret importerprofil SKAL validere det faktisk understøttede layout (`links`/`edges`, direction-markers, metadata) og undgå gæt. Hvis et link ikke har ID, er `(artifact_revision, JSON Pointer)` en entydig snapshotidentitet; den er ikke stabil gennem reorder eller re-extraction. Source+target er aldrig tilstrækkelig edge-key.

Rå arrays SKAL bevares uden edge-collapse. Retning, parallelle edges og unknown fields skal kunne rekonstrueres. Uklar retning giver UNKNOWN for retningsafhængige checks. Sourcebevis og docs-udsagn skal filtreres, så en graf af GDD-dokumentet ikke bliver bevis for, at koden opfylder det.

Den installerede Graphify 0.9.75 blev undersøgt. Dens [build_from_json-rute](https://github.com/Graphify-Labs/graphify/blob/48d7c0e832cd2d67d86850e716ddea16df6238ea/graphify/build.py) kan kollapse parallelle edges i Graph/DiGraph. [Eksporten](https://github.com/Graphify-Labs/graphify/blob/48d7c0e832cd2d67d86850e716ddea16df6238ea/graphify/export.py) har også en original_links-vej. GDD må derfor ikke afhænge af, at den simple builder er en tabsfri normativ grafmotor. Det er ikke en påstand om, at alle Graphify-ruter altid taber data.

Fuzzy symbolmatching kan foreslå mappings. Det kan ikke alene etablere verified bindings. Manglende observed edge beviser ikke manglende kodeafhængighed. Promotion til intention skal registreres som en synlig source_promotion-/model_change-beslutning inden for projektets policy.

Dette kapitel specificerer kravene. [Graphify-adapteren](../GRAPHIFY.md) implementerer nu to eksplicitte profiler for den undersøgte 0.9.75-baseline; andre layouts/versioner giver UNKNOWN. Det er ikke støtte til alle Graphify-versioner.

## 5. Afledte visualiseringer

Visninger er skrivebeskyttede projektioner. Agenten redigerer modellen; mennesket ser resultatet. En visning SKAL identificere modelrevision, generator/version, observationsinputs og hvilke node-/edge-ID'er der vises eller er udeladt.

Community-foldning, filtre og aggregater skal være synlige. Retning og alle parallelle edge-ID'er skal kunne inspiceres, selv når de tegnes samlet. Hver vist modeldel skal kunne føres tilbage til sit kanoniske ID. Source- og observationsvertices skal være tydeligt adskilt fra normativ intention.

Evidence-overlays skal vise scope og revisioner; stale resultater må ikke fremstå som aktuelle PASS. Zoom, layout og markering er visningstilstand og ændrer ikke modelsemantik. Labels, kildeindhold og pseudokode rendres som data, ikke aktiv HTML eller instruktioner.

HTML, SVG, GraphML og andre exports er mulige implementationsvalg, ikke krav om en bestemt viewer. [GDD-vieweren](../VIEWER.md) implementerer nu en konkret offline HTML-profil; andre renderere kan opfylde kontrakten gennem egne dokumenterede profiler.
