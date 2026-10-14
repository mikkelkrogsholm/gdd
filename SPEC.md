# Graph-Driven Development 0.2

**Status: standardudkast. Dato: 13. oktober 2026. Format: `gdd/0.2`.**

GDD er en metode og en agentuafhængig repræsentation for at udvikle software fra en pseudo-graf med pseudokode. Den accepterede model beskriver intentionen; kode, scanninger og testresultater beskriver implementationen og evidensen.

Dette kapitel samt [FORMAT](standard/FORMAT.md), [AGENT-PROTOCOL](standard/AGENT-PROTOCOL.md), [VALIDATION](standard/VALIDATION.md) og [INTEGRATIONS](standard/INTEGRATIONS.md) er normative. Eksempler, migration, research og scratchpad er informative. SKAL, MÅ IKKE, BØR og MÅ betyder krav, forbud, anbefaling med dokumenterbar undtagelse og tilladelse. Uoverensstemmelse mellem schema og tekst er en standarddefekt; et værktøj må ikke stiltiende vælge den lempeligste fortolkning.

## 1. Metode og mandat

Input fra mennesker og agenter organiseres i et vedligeholdt videnslag, eksempelvis en LLM-wiki i OKF. Modellen præciseres gennem iterationer. Den omfatter struktur, adfærd, regler, scenarier og brugeroplevelse. Kun det valgte udviklingsudsnit behøver være fuldt specificeret.

Nye projekter kan begynde fra input; eksisterende projekter kan begynde fra Graphify-observationer. Observationer SKAL forblive deskriptive, indtil de er valgt som intention gennem projektets autoriserede proces. De MÅ IKKE automatisk omskrive accepterede krav.

Projektet vælger arbejdsmåde og godkendelsesregler. Der findes ingen universel manuel eller autonom standardmode. Projektets mandat og platformens adgangsgrænser gælder. Agenten ændrer modellen; mennesket ser afledte visninger og giver feedback.

## 2. Én logisk model

Modellen er en rettet, typet multigraf. Noder har stabile ID'er, og hver edge har sit eget stabile ID. Flere edges med samme endpoints SKAL bevares, også ved samme relationstype. Edgerækkefølge har ingen semantisk betydning.

Lokale fragmenter SKAL samles uden implicit override til én model. Arkitektur, adfærd, sporbarhed og communities er visninger af denne model.

| Kernetype | Betydning |
| --- | --- |
| component | Ansvar, systemgrænse og offentlige kontrakter |
| entity | Domænebegreb og abstrakte datafelter |
| action | Handling med kontrakt og pseudokode |
| screen | Brugerflade, interaktioner, tilstande og designreferencer |
| rule | Krav, invariant eller arkitekturbegrænsning |
| scenario | Konkret planlagt normal-, fejl- eller grænseforløb |
| machine | Entitets tilstande og muterende overgange |
| community | Gruppering til navigation og kontekst |

Sources, questions, extensions og operational records er refererbare tilknytninger. En sporbarhedsvisning MÅ vise dem som vertices, når deres rolle er tydelig.

En node SKAL have `kind` og `description`; ID'et er registry-nøglen. En skitse MÅ udelade kontrakter, men angivne felter og referencer SKAL være gyldige. `maturity: specified` er en forfattererklæring, ikke beregnet parathed.

## 3. Identitet og revision

ID'er SKAL være unikke på tværs af modellens nodes, edges, transitions, sources, questions og extensions. Record-ID'er SKAL være unikke i projektets recordlog og ikke kollidere med model-ID'er. Global identitet er `(project.id, lokalt ID)`.

ID'er SKAL bevares ved omdøbning, flytning og nye kodebindings. Slettede ID'er MÅ IKKE genbruges til andre koncepter. Ændringer SKAL identificere base, ændrede ID'er og begrundelse.

Interne referencer SKAL resolve entydigt. En ekstern URI er alene en locator. En modelsnapshot-revision SKAL omfatte modelmanifest, fragmenter, udvidelser og normative designassets. [FORMAT](standard/FORMAT.md) definerer en SHA-256-profil. Kode og operational records har særskilte revisioner. Receipts og evidens ligger uden for det snapshot, de vurderer, så hashing ikke bliver selvrefererende.

## 4. Relationer

`edges` er autoritativt for følgende relationer. Parallelle autoritative owner-/members-/obeys-felter er ikke tilladt. Type-targets, transition-triggers og source-refs er indlejrede referencer med særskilt defineret betydning; forbindelser udledt af dem SKAL markeres som afledte.

| Relation | Fra → til | Betydning |
| --- | --- | --- |
| owned_by | action/entity/screen/machine → component | Ansvarlig komponent |
| stored_in | entity → component | Logisk persistence-grænse |
| imports | component → component | Tilsigtet tilladt direkte import |
| calls | component/action/screen → action | Tilsigtet aktivering |
| reads | action → entity | Læsning af domænedata |
| creates | action → entity | Oprettelse af instans |
| changes | action → entity | Mutation af eksisterende instans |
| constrains | rule → component/entity/action/screen/machine | Regel gælder for mål |
| exercises | scenario → action/screen/machine | Scenariets forløb |
| verifies | scenario → rule | Planlagt kontrol, ikke udført evidens |
| state_of | machine → entity | Entitetens mutationslivscyklus |
| navigates_to | screen → screen | Tilsigtet navigation |
| depends_on | component/entity/action/screen/machine → samme typer | Forudsætning for fuld realisering |
| refines | alle undtagen community → component/entity/action/screen | Barn → dekompositionsparent |
| member_of | alle undtagen community → community | Gruppering |

Fravær af en kant betyder ikke specificeret. `imports` er ikke en komplet allowlist. Forbud SKAL udtrykkes i en rule-constraint med relation, from, to, reach og policy. Policy er `forbid` i 0.2; komplette allowlists er ikke en kernecapability. Et direkte forbud forbyder ikke en lovlig indirekte sti.

`contract.effects_complete: true` erklærer, at reads/creates/changes-edges og eksterne effekter er opregnet. `dependencies_complete: true` erklærer, at kendte forudsætninger og nødvendige grænsekontrakter er opregnet. Disse er reviewbare erklæringer; de er ikke bevis for fuldstændighed. Manglende erklæringer MÅ IKKE tolkes som uafhængighed.

## 5. Undergrafer og communities

`refines` er dekomposition, ikke bevist logisk refinement. Børn MÅ have andre typer end parenten. Et barn MÅ højst have én direkte parent. Refines-grafen SKAL være acyklisk og uden selvloops.

Undergrafen SKAL bevare parentens synlige grænsekontrakt. Ved udvikling af et barn SKAL parentens relevante forpligtelser medtages. `realization` beskriver hvordan navngivne forpligtelser fordeles på børn; en strukturel mapping beviser ikke semantisk ækvivalens. Parentkrav MÅ IKKE svækkes uden en synlig autoriseret kravændring.

Communities MÅ overlappe. De beviser ikke ejerskab, rækkefølge eller uafhængighed. Kernen har flade communities; member_of mellem communities er ikke tilladt.

Calls, navigation og ikke-terminale stateforløb MÅ være cykliske. Hele modellen skal ikke være en DAG. Cykliske udviklingsafhængigheder kræver en eksplicit plan for grænsekontrakter eller samlet integration; de gør ikke automatisk modellen ugyldig.

## 6. Kontrakter og sprog

Actions bruger faste felter for input, context, output, errors, preconditions, eksterne effekter, fuldstændighedserklæringer og risikovurderinger. Tomme lister/objekter MÅ være eksplicitte fravalg. Udeladte felter betyder ukendt eller endnu ikke specificeret.

Behavior, guards, effects, statements og scenarietekster er fri pseudokode/prose. De MÅ IKKE køres med eval eller behandles som maskinelt bevis. Agenten fortolker dem inden for kontrakten og mandatet. Strukturel validering kontrollerer form og referencer, ikke faglig sandhed.

Domænetyper SKAL være abstrakte. Concrete paths, frameworktyper og sprogspecifikke symboler hører til implementationsbindings. Flere implementationer MÅ realisere samme model; evidens SKAL knyttes til hver implementation separat.

## 7. Regler, scenarier og maskiner

En rule har kategori, statement, scope og planlagte verification-metoder. Global scope gælder hele projektet; targeted scope kræver mindst én constrains-edge. En blocking rule kræver relevant bestået evidens eller en synlig policytilladt undtagelse ved implementeringsaccept. Den forhindrer ikke en gyldig skitse.

Et implementeringsklart udsnit SKAL have mindst ét normalforløb og relevante negative/grænseforløb. Scenarier har given/when/then og kategori. Risikovurderinger og error_cases forbinder erklærede risici og fejludfald til scenarier. Fravalg SKAL være synligt begrundet. Scenarier beskriver planlagte checks.

En machine har unikke lokale states, initial state, terminal states og transitions med stabile ID'er. Start/slut SKAL findes; trigger SKAL være en action. Profilen tillader én muterende transition pr. `(from, trigger)`; tekstguards gør ikke dubletter entydige. Uopregnede mutationer og udgående mutation fra terminal state er forbudt. Idempotente svar og autoriseret læsning kan fortsat beskrives i action-kontrakten.

`absent` MÅ betegne før-eksistens og er ikke automatisk en databaseværdi. Hierarkiske maskiner, parallelle regioner og flere guarded alternativer er uden for kernen.

## 8. Udvidelser

Extensions SKAL have namespace, præcis version, lokalt schemaasset og deklaration af node-kinds og relationer. Custom nodefelter ligger i `data`. Custom relations deklarerer endpoints, context traversal og eventuel betydning for udviklingsafhængigheder og effekter.

Extensions MÅ IKKE omdefinere kernens semantik, autorisation eller PASS. Der er ingen implicit substituerbarhed mellem custom og core kinds. Ikke-normative annotations har namespacede nøgler.

Ukendte extensions SKAL bevares tabsløst. Afhængige checks giver UNKNOWN; fuldt valideret eller parat må ikke påstås. Ukendt ikke-normativ annotation må ignoreres semantisk. Validering MÅ IKKE automatisk eksekvere extensionkode eller hente schemas over netværket.

## 9. Conformance

Værktøjer SKAL erklære profiler: representation, model, readiness, records, import.graphify og/eller visualization. Representation omfatter schemas og sikker indlæsning; model tilføjer grafsemantik; readiness kræver model og scoped parathed. Ingen profil er en universel adfærdsverifier.

Ukendte formater og manglende analyseinput giver scoped UNKNOWN med diagnose. Kendte overtrædelser giver FAIL. En ufuldstændig analyse med tom fejlliste MÅ IKKE give PASS. [VALIDATION](standard/VALIDATION.md) fastlægger resultater og rapporter.

Conformance kræver opfyldelse af normative regler og relevante [cases](conformance/cases.yaml). Der findes endnu ingen certificeringsinstans eller komplet referencevalidator. Udkastet bygger på eksisterende metoder; [research](research/2026-10-13-existing-work.md) dokumenterer overlap.
