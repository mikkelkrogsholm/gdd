# GDD-viewer 0.1 — leverance og kontrol

Udført 13. oktober 2026. Pakke `gdd-validator 0.4.0`, GDD-standardformat 0.2. Målet var en interaktiv, read-only grafvisning med Graphify-lignende navigation, som bevarer GDD-identitet og sporbarhed. Målet er opfyldt inden for den dokumenterede rendererprofil.

## Leveret

- [Python-renderer/API](../gdd_validator/visualization.py), [CLI](../gdd_validator/cli.py) og [validatorintegration](../gdd_validator/validator.py).
- Fem pakkede, lokale [viewerassets](../gdd_validator/viewer/shell.html): HTML, CSS og tre JavaScript-filer. Den genererede fil indeholder alle assets og data.
- [Koncert-demo](../examples/concert.graph.html): 38 kanoniske noder, 85 kanoniske relationer. Startoverblik viser 18 noder og 39 relationer; checkboxen for scenarier og manifestet angiver de øvrige dele.
- [Dokumentation](../VIEWER.md), informative [payloadschema](../schemas/view.schema.json) og [visningsschema](../schemas/view-state.schema.json), 16 [artifactcases](../conformance/visualization-cases.yaml), 29 nye pytest-tests og et [browserharness](../conformance/run_browser.cjs).

Ingen ekstra runtime-/browserafhængigheder i produktpakken. Browserharnesset bruger en separat Playwright-runtime og faktisk lokal Chromium/Chrome.

## Faktisk evidens

| Kontrol | Resultat | Output |
| --- | --- | --- |
| Integrerede pytest-tests | 172 bestået | [tests.txt](visualization/tests.txt) |
| Oprindelige standardcases | 58/58 forventninger matcher | [standard-conformance.json](visualization/standard-conformance.json) |
| Graphify-adaptercases | 18/18 forventninger matcher | [adapter-conformance.json](visualization/adapter-conformance.json) |
| Rendererartifactcases | 16/16 forventninger matcher | [artifact-conformance.json](visualization/artifact-conformance.json) |
| Reelle offline browserchecks | 17/17 bestået, 0 script-/konsolfejl, 0 eksterne requests | [browser-conformance.json](visualization/browser-conformance.json) |
| Endelig koncertfil mod aktuel model/renderer | PASS | [concert.report.json](visualization/concert.report.json) |
| Faktisk downloadet visningsmanifest | Schema-PASS | [browser.view-state.json](visualization/browser.view-state.json), [state-schema.json](visualization/state-schema.json) |
| Sdist og wheel | Bygget | [build.txt](visualization/build.txt) |
| Installeret wheel uden for checkout | Render + verify + readiness PASS; samme HTML-bytes som checkout; alle fem assets og begge nye schemas pakket | [wheel-smoke.json](visualization/wheel-smoke.json), [kontrolkode](visualization/wheel-smoke.source.py) |
| Agent-readable lint | 14 advisory AR002 near-duplicate fund; ingen i den nye renderermodul | [agent-lint.json](visualization/agent-lint.json) |

Browsertesten kørte lokal Chrome/Chromium 155.0.8059.40 med `context.setOffline(true)`. Den kopierede én HTML-fil til en ellers tom mappe og åbnede den via `file://`. Runtime var den tilgængelige bundtede Playwright-installation; ingen browser blev downloadet. Pythonmiljøet er projektets låste 3.12-miljø. Ingen eksterne sider eller model/pseudokode blev eksekveret.

Browsercases omfatter canvas- og tastaturvalg, rettede bundter med identiske parallelle edges, modsat retning, selvloops, community-overlap og gentaget medlemskab, synlighedsmanifest, scenarie-reveal som første handling, tom søgning, reset, scopegrænse, zoom/pan/drag, manifestdownload, custom kind og tom model. Hostile labels/pseudokode med script-/HTML-tekst og Unicode-separatorer forbliver eksakt tekst. Både inspector og søgning bevarer `9007199254740993`.

[Desktop](visualization/concert-desktop.png), [mørkt tema](visualization/concert-dark.png) og [smal skærm](visualization/concert-mobile.png) er visuelt inspiceret. Der er tastaturalternativ i objektlisten, fokusmarkering, statusannoncering og responsivt layout. Dette er ikke en fuld WCAG-certificering.

## Review og rettelser

Tre read-only underagenter gennemgik uafhængigt model-/viewkontrakt, installeret Graphify-UI og integration/testdesign. Lead implementerede og verificerede ændringerne. Ingen underagent skrev i checkout eller oprettede flere agenter.

Konkrete reviewfund førte til rettelser af lazy positionsseeding, unik communitymetadata uden edge-tab, kumulativ drag-tærskel og cancel, CRLF-byteafvigelser, dybt/malformed payload, manglende input som UNKNOWN og numerisk præcision i både raw inspector og søgning. Desuden blev gyldig JSONs literal U+2028/U+2029 skilt fra YAML's quoted-string-foldning. Fragment- og edgelocators bevares nu eksplicit. Positive og negative regressioner indgår i de aktuelle checks.

## Påstandens grænse og stopstatus

Artifact-PASS betyder, at én fil matcher den aktuelle modelrevision, dens kanoniske records/ID-partitioner og den installerede `gdd-viewer-html/0.1`-renderer byte for byte. Det er ikke generel tredjeparts-renderercertificering, en producentsignatur, implementationsevidens eller fuld browserkompatibilitet. De faktiske interaktioner er særskilt afprøvet i Chromium.

Ingen rå observationsvertices, evidensoverlays, kodegenerering eller modelredigering er implementeret i denne profil. Store grafer har ingen målt performancegaranti; spring-layout er begrænset til 200 figurer, og derefter bruges seed-layout. Scope og filtrering ændrer kun visningen. Modelformat og tidligere GDD/Graphify-evidens er bevaret.

Stopgrænsen er nået: brugbar offline visning, API/CLI, conformance-cases, browserverifikation, installeret pakke, demo og dokumentation. [visualization-assets.json](visualization-assets.json) fastholder de relevante leverancehashes; ældre fasers manifests er historik.
