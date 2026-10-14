# Graph-Driven Development — v0.1

**Status:** forslag til metode og notation. Udarbejdet 13. oktober 2026.
Dette er ikke en eksisterende standard, Graphify-syntaks eller et officielt Graphify-produkt.

## 1. Definition

Graph-Driven Development er her en udviklingsmetode, hvor en versionsstyret,
typet graf beskriver systemets tilsigtede struktur, adfærd og verificerbare krav.
Implementeringen udvikles i små dele og vurderes mod grafen gennem kildekodeanalyse,
tests og eksplicitte beslutninger.

Grafen er en kontrakt om det, vi har valgt at specificere. Den er ikke en påstand om,
at alt ved systemet er modelleret, og den erstatter ikke kildekode, tests eller review.

Den accepterede specifikation er normativ. Kildekode og observationer er deskriptive.
En afvigelse må derfor ikke automatisk omskrive kravet, så implementeringen ser korrekt ud.
En tilsigtet kravændring skal reviewes som en ændring af specifikationen.

## 2. Én model, flere visninger

Arkitekturvisningen viser komponenter, dataansvar, grænser og afhængigheder.
Adfærdsvisningen viser handlinger, kontrakter, tilstande, overgange, betingelser og effekter.
Sporbarhedsvisningen forbinder krav med de handlinger, kodeelementer, checks og beslutninger,
der handler om dem.

En komponent er ikke det samme som en domæneentitet. `component.reservations` kan eje både
`entity.reservation` og `entity.concert`. `action.reserve` har en ejerkomponent og optræder
som trigger i en tilstandsmaskine. Det er sådanne forbindelser, som gør modellen til én
sammenhængende graf frem for to uafhængige diagrammer.

Alle elementer har stabile id'er. Et id ændres ikke alene, fordi et navn eller en filsti ændres.
Regler og scenarier har også id'er, så deres relationer kan bruges i ændringsanalyse.

## 3. Pseudo-graf før fuld præcision

En første skitse må gerne være kort:

```text
API kalder Reservations
Reservations bruger Storage
Reservations ejer Reservation og Concert

ReserveSeat: absent -> reserved, hvis der er plads
CancelSeat: reserved -> cancelled, hvis brugeren er ejeren

Kapaciteten må aldrig overskrides
En gentagelse må ikke reservere en ekstra plads
API må ikke importere Storage direkte
```

Dette er en skitse, ikke en parser-syntaks. Før implementering af en bestemt del skal
handlingernes input, output, fejl, adgangsregler, gentagelser og relevante grænsetilfælde
præciseres. Resten af projektet må fortsat være en skitse.

Designmodenhed og verificeringsstatus er forskellige akser. En præcist specificeret
funktion kan være uimplementeret. En implementeret funktion kan have utilstrækkelig evidens.
En skitse bliver ikke mere præcis af, at en test tilfældigvis passer.

## 4. Filformat og semantik

`concert.gdd.yaml` er det første konkrete eksempel. YAML er den redigerbare repræsentation;
der behøves ikke en grafdatabase i denne første version. En fremtidig visuel editor skal
redigere den samme model, ikke skabe en konkurrerende specifikation.

Topniveauet indeholder `format`, `project`, `nodes`, `edges`, `machines`, `rules`,
`scenarios`, `bindings` og `verification_policy`.

`nodes` rummer komponenter, entiteter og handlinger. `owner` forbinder en handling eller
entitet med den ansvarlige komponent; `stored_in` forbinder en entitet med dens persistence-grænse.
`edges` beskriver eksplicitte strukturelle relationer. En `imports`-relation er rettet og
betyder her en tilladt, tilsigtet direkte import mellem komponentgrænser; den er ikke i sig
selv bevis for et runtime-kald. `calls` beskriver tilsigtet kald af en handling.

`machines` knytter tilstandsmaskiner til entiteter. En overgang har id, starttilstand,
sluttilstand, handling som trigger, betingelse og effekt. Profilen i denne prototype tillader
kun én muterende overgang pr. kombination af tilstand og trigger. Flere guarded alternativer,
hierarkiske tilstande, events og parallelle regioner kræver en senere semantisk udvidelse.

`absent` betyder, at en entitet ikke eksisterer endnu. Det er en modelleringsmarkør, ikke en
værdi i databasen. Terminale tilstande forbyder nye mutationer af entiteten; autoriserede
læsninger og idempotente svar er fortsat mulige. Fejl- og gentagelsesveje beskrives i handlingen.

`rules` definerer krav og den nødvendige verifikationsmetode. `scenarios` beskriver planlagte
checks med given/when/then og peger på regler via `verifies`. **Relationen betyder et planlagt
check, ikke et allerede bestået testresultat.**

Fravær af en arkitekturkant betyder ikke automatisk forbud. Et forbud skal være eksplicit,
eller man skal erklære et præcist afgrænset område med en komplet allowlist. Forbud skal også
angive, om de gælder direkte forbindelser eller transitive stier. API må eksempelvis godt
nå Storage indirekte gennem Reservations, selv om direkte import er forbudt.

Beskrivelser, guards, effects og regeludtryk er pseudokode. Der anvendes ikke `eval`, og
linteren fortolker dem ikke. En senere eksekverbar profil kan definere et lille typesikkert
udtrykssprog eller referere til navngivne, reviewede checkfunktioner. Et syntaksskema kan
validere datatyper og obligatoriske felter, men ikke alene forretningssemantik.

## 5. Arbejdsgang

Start med en grov graf og udpeg én lille funktion fra indgang til persistens og test.
For reservationssystemet er den første del reservation og annullering af én plads.
Betaling, udløb og billetudstedelse er med vilje ikke med. De må ikke sniges ind som
AI-antagelser; ved tilføjelse skal deres kontrakter og fejlsituationer modelleres.

Gør den valgte del præcis. Review mindst ét normalt eksempel og de vigtigste negative
og samtidige forløb, før agenten skriver løsningen. Det kan føre til ændringer i grafen.

Giv implementeringsagenten en afgrænset grafpakke: handlinger, ansvarlige komponenter,
relevante data, ind- og udgående kontrakter, globale regler, berørte scenarier og bindings.
Brug ikke et vilkårligt antal graf-hop som eneste afgrænsning; kritiske globale krav kan
ligge langt fra den lokale ændring.

Agenten implementerer inden for godkendt scope. Den returnerer kodeændring, bindings,
checkresultater og åbne spørgsmål. Den må foreslå en ændring af grafen, men må ikke skjult
lempe et krav eller ændre testforventninger for at få en grøn rapport.

Kør specifikationschecks, kildekodechecks og adfærdstests. Review resultatet og den ændrede
del af grafen. Accepter derefter ændringen eller behandl en begrundet kravændring separat.
En prototype må gerne afsløre, at modellen var forkert. Pointen er kontrolleret læring,
ikke at låse hele arkitekturen før første linje kode.

## 6. Sammenligning med implementeringen

Intent-graf og kodegraf forventes ikke at være ens. En logisk komponent kan blive til mange
filer, klasser og funktioner. Sammenligning sker via `bindings` til modulgrænser og eventuelt
kvalificerede symboler, hvorefter relevante observationer aggregeres til modellens niveau.

En binding skal være eksplicit, versionsbundet og uden tvetydig overlap. Planlagte stier
beviser ikke, at filer findes. Manglende eller flertydige mappings giver UNKNOWN for checks,
der er afhængige af dem. Lavniveauhjælpere behøver ikke blive individuelle intent-noder.

Adfærdsregler kontrolleres med passende tests, ikke ved at se, at to komponenter er forbundet.
Kapacitet under samtidighed kræver tests af den rigtige persistence-mekanisme og dens
transaktionsadfærd. En unit-test af en liste i hukommelsen giver ikke samme evidens.
Modelbaserede tests kan generere handlingssekvenser og sammenligne implementeringen med en
referenceadfærd. Korrekthed af referenceadfærden kræver selv review.

## 7. Graphifys rolle

Graphify er en mulig leverandør af observationer, ikke den normative model. Det aktuelle
open-source-repository dokumenterer `graphify-out/graph.json`, `graph.schema_version` og
`graph.graphify_version`. En adapter skal kontrollere formatversionen og bevare relationstype,
kildereference og proveniens. Denne startpakke indeholder IKKE en sådan adapter.

Graphifys dokumentation adskiller strukturel kodeudtrækning fra semantisk udtrækning af
blandt andet dokumenter. Den nævner også huller fra dynamiske kald, manglende inputs og gamle
scanninger. Derfor må en relation udledt fra GDD-specifikationen aldrig tælle som evidens for,
at implementeringen overholder den. Kodeobservationer og normative dokumenter skal filtreres
eller scannes separat ved conformance-checks. `INFERRED` betyder heller ikke nødvendigvis,
at en LLM har skabt relationen; statisk referenceresolution kan også give den markering.

En første adapter kan fokusere på Python-imports i det afgrænsede applikationsområde. Gem
checker- og extractorversion samt præcis kode- og spec-revision. Ukendte formater eller
manglende analyseinput må ikke give en tom, tilsyneladende grøn afvigelsesliste.

## 8. Evidens og accept

Hvert check får `PASS`, `FAIL` eller `UNKNOWN`, med scope og begrundelse. PASS betyder,
at netop det check bestod på den angivne revision under de angivne forudsætninger. Det er
ikke universel korrekthed og ikke automatisk et formelt bevis. Tests, statisk analyse,
bounded model checking og et egentligt bevis er forskellige slags evidens.

Evidens skal som minimum identificere spec-revision, kode-revision, check-id,
checkerversion, scope, resultat og placering af output. Gammel evidens må ikke uden videre
genbruges til ændrede afhængigheder. Denne prototype anvender konservativ genkørsel;
smart cache-invalidering er senere arbejde.

En blokerende regel med FAIL eller UNKNOWN forhindrer accept i den foreslåede strenge
profil. En undtagelse kræver en synlig beslutning med ansvarlig, begrundelse og udløb;
undtagelsen ændrer ikke checkets resultat til PASS.

En acceptabel første demonstration er, at systemet opdager både en bevidst forbudt import,
en svækket kapacitetsbeskyttelse og en idempotensfejl. At den oprindelige kode passer sine
egne genererede tests er ikke nok. Reviewede eksempler og negative kontroller skal udfordre
både agenten og testgrundlaget.

## 9. Hvad pakken faktisk gør

`check_spec.py` kontrollerer udvalgte strukturelle egenskaber: duplicate YAML-nøgler,
graf-id'er, referencer, typer af endpoints, ejere, deklarerede tilstande, overgangstriggere,
terminale tilstande og planlagte scenarier for blokerende regler. Den er ikke en komplet
sprogvalidator. Den kontrollerer ikke betydningen af den frie tekst.

Der følger ingen reservationsapplikation, database, testadapter, kodegenerator, visual editor,
Graphify-integration eller formel verifier med. Adfærds- og arkitekturevidens for en applikation
er derfor UNKNOWN. De medfølgende unit-tests tester linteren, ikke reservationssystemet.

## 10. Eksisterende arbejde og kilder

Modellen bygger på eksisterende ideer om modeldrevet udvikling, tilstandsmaskiner og
modelbaseret test. Navnet Graph-Driven Development er allerede i brug andre steder.
Denne pakke gør ikke krav på at opfinde termen eller hele feltet.

Kilder læst 13. oktober 2026:

- OMG: Model Driven Architecture — https://www.omg.org/mda/
- W3C: SCXML, især transitions og guards — https://www.w3.org/TR/scxml/
- Graphify: officielt repository — https://github.com/Graphify-Labs/graphify
- Graphify: How Graphify works — https://docs.graphify.com/concepts/architecture
- Hypothesis: Stateful tests — https://hypothesis.readthedocs.io/en/latest/stateful.html
- JSON Schema: Draft 2020-12 — https://json-schema.org/draft/2020-12
- Atomic: egen brug af graph-driven development — https://atomic.dev/

Disse kilder dokumenterer beslægtede teknikker og integrationspunkter. De dokumenterer
ikke, at den foreslåede GDD-metode giver færre fejl eller lavere omkostninger. Det skal måles.
