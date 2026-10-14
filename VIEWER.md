# Interaktiv GDD-graf

`gdd view` laver en selvstændig HTML-fil med den validerede GDD 0.2-model. Åbn filen i en browser; den fungerer offline uden server, CDN, Graphify-installation eller JavaScript-pakker. Pakkeversionen er 0.4.1, rendereren 0.1.3 med profilen `gdd-viewer-html/0.1`; standardformatet er fortsat GDD 0.2.

```sh
uv run gdd view examples/concert.gdd.yaml --output concert.html
uv run gdd view examples/concert.gdd.yaml --scope action.reserve --output backend.html
uv run gdd validate examples/concert.gdd.yaml --profile visualization --view concert.html
```

Outputfilen skal være ny. En eksisterende fil eller symlink overskrives aldrig. Modellen skal kunne indlæses og valideres med kendt snapshot. Skitser kan vises, selv om de mangler implementeringskontrakter; uløste modelreferencer og utilgængelige extensions afvises.

Den færdige [koncertvisning](examples/concert.graph.html) viser modellen med 38 noder og 85 selvstændige relationer. Den indgår ikke i modelsnapshot. Agenten ændrer GDD-filerne og genererer en ny visning; browseren skriver aldrig tilbage.

## Udforskning

Startvisningen viser et overblik uden scenarier. Checkboxen **Vis scenarier (20)**, de synlige/total-tællere og visningsmanifestet gør udeladelsen eksplicit. En søgning eller valg af nodetypen `scenario` kan også vise scenarier. **Nulstil filtre og foldning** viser alle noder inden for artifactets scope.

- Søg i node-ID'er, labels, beskrivelser, kontrakter, pseudokode og relationsrecords. Et relationsmatch medtager begge endpoints. Søgning kombineres med type- og communityfilter.
- Vælg nodetype, relation eller community. Communityfilteret viser communitynoden og dens direkte medlemmer, også medlemmer af andre communities. Relationsfilteret begrænser edges og beholder de valgte noder.
- Klik en node for at læse beskrivelse, maturity, kontrakt, pseudokode, maskine, kilder, spørgsmål og alle rå nodefelter. Inspectorens relationer kan åbnes og følges til deres endpoints, også hvis de er filtreret fra canvas.
- Klik en forbindelse for at se hver selvstændig relation. Retningen vises med pil og med `from → to` i inspector. `×N` samler flere edges med samme projicerede retning; hvert kanonisk edge-ID bevares. Modsat rettede forbindelser og selvloops får egne spor.
- Træk baggrunden for at panorere, træk noder for at flytte dem, rul eller brug zoomknapperne. **Tilpas** finder det aktuelle udsnit. Layout er visningstilstand.
- Objektlisten har både noder og relationer og kan bruges med Tab og Enter/Space. Den er tastaturalternativet til grafen. Alle tekstdata kan markeres og kopieres.

På smalle skærme placeres graf, objektliste og detaljer efter hinanden. Visningen følger browserens lyse/mørke farvetema og bruger ingen kontinuerlig bevægelse.

## Communities og scope

Foldning ændrer kun tegningen. En figur oplyser sine repræsenterede node-ID'er. Interne relationer, der ikke tegnes som særskilte forbindelser, tælles eksplicit og findes i communityinspector og relationslisten. Overlap bevares: en node repræsenteres én gang i den første synlige community efter kanonisk ID; inspector viser alle dens faktiske medlemskaber. Parallelle `member_of`-edges ændrer ikke antallet af forskellige communities.

`--scope` bruger validatorens kontekstclosure. For `action.reserve` er 34 noder og 73 edges tilgængelige; 4 noder og 12 edges ligger udenfor. Browserfiltre kan indsnævre dette artifact-scope, og reset går tilbage til det. Generér uden `--scope` for at kunne tegne hele modellen. Hele den kanoniske model bevares i HTML-data til inspektion og verificering; scope er ikke redaktion eller adgangskontrol.

Communities er gruppering. Foldning, søgematches og grafnaboer dokumenterer ikke uafhængige arbejdspakker eller implementeringsparathed. `refines` forbliver en eksplicit relation og bliver ikke automatisk communitymedlemskab.

## Sporbarhed og datakontrakter

Den indlejrede `#gdd-data` er et `gdd-view/0.2`-værktøjsobjekt:

| Felt | Betydning |
| --- | --- |
| generator | Navn, version og eksakt rendererprofil |
| spec_revision | Faktisk bytebaseret modelsnapshot |
| observation_inputs | Tom liste i denne renderer; ingen eksterne observationslag vises |
| scope | Context-closure seeds; tom liste betyder hele modellen |
| model | Samlet model med alle kanoniske records og felter |
| locations | Kanonisk ID → lokal fil og JSON Pointer, inklusive fragmenter |
| snapshot | Assetpaths og hashes brugt til modellen |
| text | Python-genereret eksakt inspectortekst, som også bevarer heltal uden for JavaScripts præcision |
| manifest | Komplet, disjunkt partition af viste og udeladte node-/edge-ID'er for artifactets scope |

Browseren vedligeholder `#gdd-view-manifest` som `gdd-view-state/0.2`. **Hent visningsmanifest** gemmer samme JSON. Det identificerer model/generator/observationsinputs, artifact-scope, aktive filtre, foldningsregel, viste/udeladte kanoniske ID'er, figurernes repræsenterede noder, rettede relationsbundter og foldede interne edges. Her betyder “vist” repræsenteret af den aktuelle projektion; et foldet medlem tælles stadig med. Bundter og interne edges dækker tilsammen hver vist edge præcis én gang.

Manifestet beskriver semantisk synlighed; kamerakoordinater og nodepositioner eksporteres ikke. De informative schemas [view.schema.json](schemas/view.schema.json) og [view-state.schema.json](schemas/view-state.schema.json) er værktøjskontrakter, ingen nye GDD-core-dokumenter. Schema-validering alene beviser ikke ID-partitioner eller korrekt grafprojektion.

## Verifikation og tillidsgrænse

`--profile visualization --view FILE` indlæser ingen aktiv HTML. Validatoren læser payloadet, sammenholder revision og kanoniske records/ID-partitioner, regenererer hele filen med den installerede renderer og sammenligner de præcise bytes. Kendt ændring eller revisionsmismatch giver FAIL; manglende input eller en ukendt rendererprofil/version giver UNKNOWN. PASS gælder dette konkrete artifact og denne profil. Interaktion og layout kræver de separate browsertests.

En gyldig model kan overstige rendererens JSON-serialiseringskapacitet. Det giver kontrolleret `FORMAT_UNSUPPORTED` og CLI-exit 2, før output åbnes. Den genererede payload prøves genindlæst før publicering. Parser-, serialiserings- eller regenereringskapacitet under verifikation giver UNKNOWN; det er ikke en konstateret artifactmodstrid.

Labels, kildemetadata og pseudokode indsættes med `textContent`. JSON escapes mod afsluttende script-tags; en indlejret CSP tillader kun rendererens kendte scriptdigest og blokerer netværk. URI'er i kilder bliver ikke automatisk aktive links. Dette er en revisionskontrol, ikke en signatur eller attestering af producentens troværdighed.

Visningen har ingen evidensoverlays og viser ingen modelgyldighed, readiness, autorisation eller implementation som et nodefarvet PASS. Maturity er forfatterens erklæring. Kilder inspiceres særskilt fra intention; rå Graphify-observationer, records og eksterne kildefilers indhold visualiseres ikke som ekstra vertices. Accepterede GDD-modeller, der er udviklet fra Graphify-forslag, kan vises på samme måde som andre GDD-modeller.

Layoutet bruger et deterministisk, afgrænset spring-layout op til 200 synlige figurer og et deterministisk seed-layout derefter. SVG-visningen er afprøvet på de vedlagte eksempler; store repositorygrafer har ingen målt performancegaranti. Brug scope og filtre til et læsbart udsnit.

## Python-API og checks

```python
from gdd_validator import render_view, verify_view, exit_code

report = render_view("examples/concert.gdd.yaml", "concert.html")
assert exit_code(report) == 0
report = verify_view("examples/concert.gdd.yaml", "concert.html")
assert exit_code(report) == 0
```

`render_view` og `verify_view` kræver en gyldig, resolved model; ugyldige model-/scopeargumenter giver `ValueError`, filskrivning giver `OSError`. Ved et gyldigt modelinput returnerer artifactkontrollen en `gdd-report/0.2`-rapport. CLI'en kontrollerer fejl uden traceback.

```sh
uv run pytest -q
uv run python conformance/run_visualization.py
# Separat dev-installation af Playwright + Chromium eller allerede tilgængelig runtime:
node conformance/run_browser.cjs
```

Browserharnesset bruger installeret `playwright`, starter Chromium headless, genererer nye fixtures i et midlertidigt directory og afprøver `file://` med netværket slået fra. `NODE_PATH` eller `GDD_PLAYWRIGHT_MODULE` kan angive en eksisterende Playwright-runtime; `GDD_BROWSER_EXECUTABLE` kan angive lokal Chrome, og `GDD_PYTHON` et miljø med denne pakke. Der installeres ingen browser fra harnesset. Rapporter og screenshots gemmes under `evidence/visualization/`; `GDD_BROWSER_OUTPUT` vælger en anden mappe ved nye reviewrunder.

[Reviewrunderne](evidence/review-loops/README.md) angiver aktuel verifikation. [Visualization-reviewet](evidence/visualization-review.md) bevarer resultaterne fra 0.4.0.
