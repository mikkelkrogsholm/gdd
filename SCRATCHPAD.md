# GDD — scratchpad

Dato: 13. oktober 2026.

Samlede arbejdsnoter fra samtalen frem til 13. oktober 2026. Notatet skelner mellem aftalt retning, undersøgelsesfund, forslag og åbne valg. Det ændrer ikke i sig selv SPEC.md eller concert.gdd.yaml.

**Efterfølgende status:** Aftalerne er nu omsat til [GDD 0.2-standardudkastet](SPEC.md) med normative kapitler, schemas og illustrative eksempler. Den tidligere 0.1-specifikation er arkiveret eksplicit. Se [README](README.md) for læsevej og [reviewrapport](evidence/standard-review.md) for udførte kontroller. Denne scratchpad bevares som historisk beslutningsgrundlag; dens oprindelige åbne spørgsmål nedenfor beskriver udgangspunktet for det nu afsluttede standardarbejde.

## Aftalt retning og beslutninger

- **Metoden først.** Vi udvikler GDD-metoden, før vi omsætter den til et framework.
- **En standard for alle agenter.** GDD skal være agentuafhængig; det afgrænses ikke til Codex eller Claude Code. Konkrete værktøjer implementerer standarden.
- **Gennem agenter.** Den primære arbejdsgang er gennem eksisterende AI-agenter, ligesom Graphify. Agenten læser, ændrer og anvender modellen.
- **Mennesket ser resultatet.** Grafvisningen bruges til overblik, navigation og feedback. Redigering af noder og relationer sker gennem agenten, ikke direkte i visualiseringen.
- **To indgange.** Vi skal kunne begynde med input til et nyt projekt og arbejde videre fra en eksisterende Graphify-graf over kode.
- **Et vedligeholdt videnslag.** Frameworket skal kunne opbygge og vedligeholde en LLM-wiki/OKF ud fra inputmaterialet. Den præcise formatintegration er endnu åben.
- **Pseudo-graf med pseudokode.** Den fælles model beskriver både struktur og adfærd og præciseres gennem iterationer.
- **Communities og parallelitet.** Grafen kan grupperes, eksempelvis i backend og frontend, og bruges til at finde uafhængige udviklingsopgaver.
- **Brugeroplevelse og design.** Modellen skal også beskrive skærmbilleder, navigation, brugerhandlinger og visuelle ønsker. DESIGN.md skal indgå.
- **Projektet vælger arbejdsmåde.** Både godkendelse før implementering og automatisk fortsættelse inden for den aftalte opgave skal understøttes. Brugeren eller projektet sætter selv arbejdsmåde og godkendelsesregler; der er ikke aftalt én universel standardmode.
- **Sproguafhængig intention.** Samme abstrakte model skal kunne danne grundlag for implementering i forskellige kodesprog.
- **Et valideringsværktøj.** Der skal være et værktøj, analogt med valideringsværktøjer omkring OKF og DESIGN.md, som agenter kan bruge til at validere grafen.
- **Første validator afgrænses.** Den kontrollerer grafmodellens gyldighed og den strukturelle del af implementeringsparathed. Standardens forbindelser til kode og test-evidens skal tænkes ind fra starten; bred implementeringskontrol kommer senere gennem særskilte checkere.
- **Flere detaljeringsniveauer.** Modellen begynder med funktioner, domænebegreber, skærme og kontrakter. En node kan foldes ud i en undergraf, når mere præcision er nødvendig. Filer, klasser og funktioner forbindes gennem kodebindings.
- **Faste kontrakter og fri pseudokode.** Strukturerede felter beskriver input, output, fejl, afhængigheder og regler. Selve forløbet kan beskrives i fri pseudokode. Den konkrete serialisering er stadig et formatforslag.
- **Lille kerne med udvidelser.** Standarden skal kunne udvides med deklarerede nodetyper og relationer, hvis felter og forbindelser kan valideres.
- **Konkrete scenarier.** Et implementeringsklart udsnit skal indeholde et normalforløb og relevante fejl- eller grænsetilfælde. Scenarier kan udtrykkes som given/when/then og forbindes til handlinger og regler.

## Aktuel arbejdsgrænse

Afklaringen af den overordnede retning er afsluttet for nu. Næste skridt er at udvikle standarden og konceptet ud fra det materiale og de aftaler, vi allerede har.

Vi skal nu formulere en sammenhængende standard for modellens begreber, relationer, kontrakter, udvidelser, agentprotokol og validering. Det er specifikationsarbejde, før vi implementerer frameworket eller valideringsværktøjet.

Forslaget om at vælge og bygge et konkret demonstrationprojekt nu blev ikke valgt. Reservationsmodellen kan bruges som eksisterende illustrativt materiale under standardarbejdet. Et senere praktisk forsøg, frontendimplementering og afprøvning på en eksisterende kodebase er ikke næste opgave i denne fase.

De åbne detaljer nedenfor er designarbejde til standarden. De betyder ikke, at der nu skal gennemføres endnu en spørgerunde om hovedretningen.

## Retning

Vi vil udvikle metoden før frameworket.

Et menneske eller en agent leverer input om det ønskede system. Input bliver gennem iterationer omsat til en pseudo-graf med pseudokode. Når den relevante del af modellen er afklaret, driver grafen udviklingen: AI-agenter omsætter afgrænsede dele til kode og tests, og uafhængige opgaver kan udføres parallelt. Kontroller og evidens forbinder implementeringen tilbage til modellen.

Grafen beskriver både systemets struktur og dets adfærd. Den er det primære arbejdsgrundlag, når vi udtrykker og ændrer systemets intention.

## Fra input til udvikling

**Input → LLM-wiki/OKF → pseudo-graf med pseudokode → iteration og afklaring → grafstyret udvikling → evidens og videre ændringer.**

Input kan være ideer, samtaler, dokumenter, eksempler, eksisterende viden og begrænsninger. Både mennesker og agenter kan bidrage.

En LLM-wiki/OKF organiserer og forbinder materialet, så det er tilgængeligt under modelleringen. Agenten skal kunne opbygge og vedligeholde dette lag som del af processen. Om alle projekter skal have samme vidensstruktur, og præcis hvordan OKF indgår, er endnu ikke fastlagt.

Omsætningen til pseudo-graf er et modelleringsarbejde: identificér ønskede egenskaber, handlinger, data, regler og sammenhænge. Bevar referencer tilbage til kilderne. Modstridende input og agentens egne antagelser skal fremstå som spørgsmål eller forslag, indtil de er afklaret.

Grafen udvikles gennem nogle iterationer mellem menneske og agent. Antallet af iterationer er ikke fastlagt. Den relevante del er klar til udvikling, når dens adfærd, grænser, kontrakter og væsentlige åbne spørgsmål er tilstrækkeligt afklaret. Resten kan fortsat være grov.

## To indgange til samme metode

**Nyt projekt:** input og viden omsættes til en pseudo-graf, som itereres og derefter driver udviklingen.

**Eksisterende projekt:** en Graphify-graf over den eksisterende kode bruges som udgangspunkt for forståelse og videre modellering. Observationer om den aktuelle implementation og krav til den ønskede løsning skal fortsat kunne skelnes. En observeret relation bliver ikke automatisk en godkendt arkitekturregel.

Den fælles model skal kunne forbinde det eksisterende med det ønskede: hvad findes, hvad skal bevares, hvad ønskes ændret, og hvad mangler stadig afklaring eller evidens. Det konkrete importformat og reglerne for omsætning fra Graphify er endnu ikke designet.

## Agentuafhængig standard og værktøjer

GDD skal være en fælles standard, som alle relevante AI-agenter kan følge. Codex og Claude Code er eksempler på forbrugere af standarden; ingen bestemt model eller agent er valgt som nødvendig runtime.

Den primære oplevelse er agentbaseret. Et menneske giver input og feedback; agenten vedligeholder viden og model, bruger validatoren og driver udviklingen. Målet omfatter visualiseringsmuligheder i stil med Graphify.

Det er besluttet, at et valideringsværktøj skal findes. Det er endnu åbent, om værktøjet skal være obligatorisk for alle former for GDD-brug, og hvordan agenter finder og kalder det. Et konkret CLI, SDK, MCP-interface, installationsformat eller implementeringssprog er ikke valgt.

## Visualisering og redigering

Agenten ændrer modellen. Mennesket ser resultatet og giver feedback gennem agenten.

Visningen skal kunne støtte udforskning af noder, relationer og communities. Interaktive grafvisninger som hos Graphify er ønsket. De præcise eksporttyper, filtre, layouts og statusmarkeringer er endnu åbne.

Visningen skal være afledt af den fælles model. Den må ikke udvikle sin egen konkurrerende version af kravene. Første retning omfatter ikke en grafeditor, hvor mennesket redigerer modellen ved at flytte eller forbinde noder direkte.

## Godkendelse og automatisk fortsættelse

Begge arbejdsmåder skal understøttes:

- Agenten præsenterer den relevante pseudo-graf og venter på menneskets godkendelse før kodearbejdet.
- Agenten fortsætter automatisk inden for den aftalte opgave og eksisterende autorisation.

Brugeren eller projektet sætter selv arbejdsmåde og godkendelsesregler. Forslaget om en fælles standardmode med menneskelig godkendelse af første graf og væsentlige ændringer blev ikke vedtaget som universel regel.

Automatisk fortsættelse gælder inden for projektets aftalte mandat. Den gør ikke uklarheder til accepterede krav og giver ikke i sig selv mandat til at svække kontrakter. Hvordan konfigurationen repræsenteres, og hvordan skift og relevante beslutninger registreres, skal beskrives i standardarbejdet.

## Communities og parallel udvikling

Grafen kan have communities, eksempelvis backend og frontend. De giver overblik, afgrænser agentens kontekst og kan bruges til at fordele ansvar. De kan også grupperes efter domæne eller funktion. Hvordan medlemskab og eventuelle overlap repræsenteres, er endnu åbent.

Communities er grupperinger i den fælles model. Relationer og kontrakter på tværs af grupper skal fortsat være synlige. En gruppering eller et automatisk fundet cluster er ikke i sig selv bevis for uafhængighed.

Parallelle arbejdsopgaver afledes af grafens relevante afhængigheder, fælles data, globale regler og implementeringsansvar. Backend og frontend kan eksempelvis arbejde samtidigt, når deres fælles kontrakt er afklaret, og der er en plan for integration.

En arbejdsopgave bør identificere graf-ID'er, nødvendige kontrakter, forudsætninger, ejerskab over implementeringsfiler og forventet evidens. Uafhængige Git-writers skal have isolerede worktrees; parallel udvikling kræver også koordination omkring ændringer af fælles kontrakter og modellen selv.

Parallelitet er en del af visionen. En konkret scheduler, et bestemt antal agenter eller krav om altid at bruge flere agenter er ikke besluttet.

## Detaljeringsniveauer og undergrafer

Grafen skal kunne forfines i flere niveauer. Den kan begynde med funktioner, domænebegreber, skærme og kontrakter. Når en del kræver mere præcision, kan en node foldes ud i en undergraf.

En undergraf skal fortsat være del af den samme model med sporbarhed til den overordnede node. Konkrete filer, klasser og funktioner forbindes til modellen gennem kodebindings.

Detaljeringsniveau og communities er forskellige begreber: en undergraf udfolder en del af modellen, mens en community grupperer relaterede elementer. De præcise relationer og regler for grænser, referencer og kontrakter på tværs af niveauer skal fastlægges.

## Brugeroplevelse og DESIGN.md

Pseudo-grafen skal også kunne beskrive brugeroplevelsen: skærmbilleder, navigation, brugerhandlinger og visuelle ønsker. Frontend-agenter skal kunne udvikle ud fra denne del af modellen.

DESIGN.md skal indgå som fælles designgrundlag med visuel retning, tokens, komponentvejledning og relevante tilstande. Grafens UI-elementer skal kunne referere til det, så designbeslutninger kan genbruges på tværs af skærme og agenter.

Forslaget er at lade grafen beskrive produktets interaktion og forbinde den til DESIGN.md frem for at kopiere hele designgrundlaget ind i hver node. Det præcise bindingsformat og eventuelle designchecks er endnu åbne.

Der er endnu ikke valgt en visuel identitet eller skrevet et DESIGN.md til GDD-værktøjet eller reservations-eksemplet. Integration af DESIGN.md-formatet og design-md-skillen er ønsket; de konkrete integrationsmekanismer er ikke implementeret.

## Kodesprogsuafhængighed

Målet er en sprog- og teknologiuafhængig intent-model. Den beskriver domæne, adfærd og kontrakter med abstrakte typer og pseudokode. Teknologivalg, konkrete typer, frameworkdetaljer og kodebindings ligger i et særskilt implementeringslag, der refererer til modellens ID'er.

Dermed kan samme model være udgangspunkt for forskellige implementeringer, eksempelvis en backend i Python eller TypeScript. Hver implementering skal verificeres for sig; evidens fra én implementering overføres ikke automatisk til en anden.

Sproguafhængigheden kommer fra den valgte semantik og abstraktion. En graf kan også indeholde sprogspecifikke detaljer, så grafstrukturen alene giver ikke denne egenskab. Vi skal derfor holde pseudokode og kontrakter fri af utilsigtede teknologibindinger, samtidig med at krav om eksempelvis atomicitet og adgangskontrol er præcise.

## Hvad betyder native for AI-agenter?

Arbejdshypotesen er, at agenten skal kunne finde, følge og ændre modellens elementer gennem eksplicitte referencer. En handling skal kunne læses sammen med dens kontrakt, data, afhængigheder, relevante regler og scenarier.

Det afgørende er læsekontrakten mellem modellen og agenten. Filformatet er repræsentationen. At en model kan læse YAML er ikke i sig selv dokumentation for, at metoden giver bedre resultater.

## Aftalt udtryksform: faste kontrakter og fri pseudokode

Modellen bruger en hybrid:

- Faste felter til input, output, fejl, afhængigheder og regler, så værktøjet kan kontrollere deres struktur og referencer.
- Fri pseudokode til forløbet, som agenten fortolker og implementerer inden for kontrakten.

Det giver plads til naturligt udtrykte ideer og gradvis præcisering. En strukturel validering af felterne udfører ikke pseudokoden og dokumenterer ikke dens faglige korrekthed.

Enkelte pseudokodebegreber kan senere få standardiseret semantik, hvis erfaringerne viser et behov. Vi har ikke besluttet at begynde med et fuldt eksekverbart programmeringssprog.

## Forslag: en tekstbaseret, typet graf

Brug YAML til modellens struktur og multiline-felter til pseudokode. Det bygger videre på det eksisterende eksempel.

Nedenstående er en skitse til notation, ikke et vedtaget skema eller en komplet kontrakt klar til implementering. ID'erne er forkortede for læsbarhed; en konkret revision skal bevare de eksisterende stabile ID'er.

```yaml
format: gdd/0.1

entities:
  concert:
    fields:
      capacity: integer

  reservation:
    fields:
      concert: ref[concert]
      owner: authenticated_user
      state: reserved | cancelled

actions:
  reserve:
    input:
      concert: ref[concert]
      request_key: string
    returns: ref[reservation]

    creates: [reservation]
    obeys: [capacity, idempotency]

    behavior: |
      Validate authenticated user and input.

      If this request previously succeeded:
        Return its reservation in its current state.

      Atomically:
        Require an available seat.
        Create the reservation.
        Bind the request key to the reservation.
        Return the reservation.

rules:
  capacity:
    applies_to: [concert, reservation]
    must_hold: |
      Active reservations never exceed concert capacity.

  idempotency:
    applies_to: [reserve]
    must_hold: |
      Repeating a successful request creates no new reservation.
```

`creates`, `obeys` og `applies_to` udtrykker typede relationer i grafen. Pseudokoden beskriver handlingens forløb. En grafvisning skal udledes af den samme model.

Pseudokoden må gerne være fleksibel. Modellens struktur og relationer skal være entydige. Betydningen af friteksten er endnu ikke maskinelt defineret; strukturel validering beviser ikke, at implementeringen følger den.

## Deklarerede udvidelser

GDD skal have en lille fælles kerne og mulighed for deklarerede udvidelser. Et projekt eller et domæne kan eksempelvis have brug for nodetyper som `sensor`, `robot` eller `training_job`.

Udvidelser skal beskrive deres felter og relationer, så validatoren kan kontrollere dem. De nævnte domænetyper er eksempler på udvidelser, ikke obligatoriske kernetyper.

Namespace, versionsregler, distributionsform og håndtering af ukendte udvidelser skal fastlægges i standarden. Udvidelsesmuligheden må ikke gøre kernen unødigt stor eller kræve de samme domænebegreber i alle projekter.

## Scenarier som del af implementeringsparathed

Et udsnit, der er klar til udvikling, skal have et konkret normalforløb og relevante fejl- eller grænsetilfælde. Scenarier kan beskrives som given/when/then og forbindes med handlinger og regler.

Reservationsmodellen illustrerer eksempelvis:

- Sidste ledige plads reserveres.
- To brugere forsøger at reservere den sidste plads samtidigt.
- En tidligere succesfuld request gentages.

Scenarierne giver mennesket og agenten konkrete forventninger at vurdere adfærden ud fra. De beskriver planlagte checks; en beskrevet situation er ikke et bestået testresultat.

Validatoren kan kontrollere deres felter og referencer. Om scenarierne faktisk dækker de væsentlige tilfælde, kræver stadig faglig vurdering.

## Principper for metoden

1. **Stabile ID'er.** Handlinger, entiteter, komponenter, regler og scenarier beholder deres identitet, når navne eller implementering ændres.
2. **Definerede relationer.** Mulige relationer som `calls`, `reads`, `creates`, `changes` og `obeys` skal have præcise betydninger og tilladte endpoint-typer. Flere relationer mellem samme elementer skal kunne bevares.
3. **Synlige åbne spørgsmål.** Agenten skal kunne skelne mellem afklarede krav og forhold, der skal afklares før implementering. Designmodenhed og verificeringsstatus er forskellige akser.
4. **Afgrænset læsning.** En opgave samler de relevante kontrakter, entiteter, regler, scenarier og globale krav. Arbejdspakken afledes fra modellen og skal bevare referencer og revision; den er ikke en særskilt specifikation.
5. **Sporbare ændringer.** Agenten rapporterer berørte graf-ID'er, tilknyttet kode og konkret evidens. En observeret afvigelse ændrer ikke automatisk det accepterede krav.

## Validering — aftalt første afgrænsning

Vi skelner mellem tre spørgsmål:

1. **Er modellen gyldig?** Kontroller eksempelvis unikke ID'er, gyldige referencer, relationernes endpoint-typer, tilstandsovergange og strukturelle referencer til design og kilder.
2. **Er det valgte udsnit klar til implementering?** Kontroller, om nødvendige kontraktfelter og konkrete scenarier er til stede, om forudsætningerne er erklæret, og om væsentlige åbne spørgsmål er markeret. Den faglige tilstrækkelighed kræver stadig review.
3. **Opfylder implementeringen kravene?** Brug passende kodeanalyse og tests; forbind resultater med graf-ID'er og den konkrete revision.

Første validator håndterer punkt 1 og den strukturelle del af punkt 2. Den skal kunne give konkret feedback, som agenten kan handle på. Den kan opdage en manglende fejlkontrakt; den kan ikke alene fastslå, at en udfyldt kontrakt er fagligt korrekt.

Punkt 3 skal kunne kobles på gennem særskilte checkere, eksempelvis en Graphify-adapter til arkitektur, database-tests til samtidighed og checks af frontend mod designgrundlaget. Disse er senere muligheder, ikke allerede implementerede capabilities.

Standardens forbindelser mellem krav, kode og test-evidens skal tænkes ind fra begyndelsen. En gyldig graf, et implementeringsklart udsnit og beståede implementeringschecks er forskellige vurderinger. PASS, FAIL og UNKNOWN skal altid have et konkret scope.

Illustrativt output — ikke et eksisterende værktøjsresultat:

```text
Model: gyldig

action.reserve:
  Klar til implementering: nej
  Mangler: forventet resultat ved ukendt koncert

Implementeringschecks:
  rule.capacity: UNKNOWN
  Ingen test-evidens registreret
```

CLI-navn, rapportformat, schemas og den præcise readiness-profil er endnu ikke fastlagt.

## Agentprotokol — første forslag

**Læs udsnit → afklar huller → implementér kontrakten → kontrollér resultatet → forbind evidens til modellen.**

- Læs den valgte handling og dens relevante kontrakter, globale regler, scenarier og bindings.
- Gør væsentlige uklarheder synlige. Foreslå præciseringer før afhængig implementering; resten af projektet kan fortsat være skitseret.
- Implementér inden for den accepterede kontrakt og det afgrænsede scope.
- Kør de relevante strukturelle og adfærdsmæssige checks, inklusive negative kontroller hvor nødvendigt.
- Registrér resultater som PASS, FAIL eller UNKNOWN med scope, revision og evidensplacering.
- Forbind kode og evidens til modellens stabile ID'er. Foreslå kravændringer eksplicit.

## Hvad vi har lært af Graphify

Den installerede version blev undersøgt som del af samtalen: Graphify 0.9.75. Udtrækning er adskilt fra grafopbygning, analyse og eksport. Det gør dens tilgang til navigation, kontekst, afhængighedsanalyse og visning relevant for GDD.

En lille afprøvning i hukommelsen viste:

- En håndskrevet extraction-lignende graf blev accepteret af skemavalideringen.
- Stabile intent-ID'er og et ekstra pseudokodefelt på en node blev bevaret.
- Grafen kunne bygges som rettet graf.
- To forskellige relationer, `creates` og `returns`, mellem samme to noder blev slået sammen til én i den undersøgte build-rute.

Afprøvningen var en formatprobe. Den genererede ingen applikation og beviste ikke forretningsadfærd eller tabsløs kompatibilitet med hele GDD-modellen. Der blev ikke skrevet filer fra proben.

Læringen er, at Graphify kan være en observer og en kilde til genbrugelige ideer eller adapters. GDD's model skal bevare egne stabile ID'er, relationer og normative betydning. Graf → kode kræver også intention, kontrakter og pseudokode; flere implementeringer kan opfylde den samme model.

## Eksisterende arbejde og vores mulige bidrag

Undersøgelsen fandt væsentligt overlap, især:

- Microsoft RPG/ZeroRepo: krav → planlægningsgraf → repository, samt kode → graf gennem RPG-Encoder.
- Spec Growth Engine: specifikationsgraf, afgrænset agentkontekst, kodebindings, driftchecks og evidens.
- grAIph: tæt konceptuel lighed med et visuelt, grafstyret udviklingsmiljø; den aktuelle implementering beskrives som privat.
- Kenneth Tylers GraphDrivenDevelopment: både navnet og ideen om grafen som autoritativ repræsentation er allerede i brug.
- Intent Architect og JetBrains MPS: modeller og egne sprog som grundlag for kodegenerering.

En samlet [undersøgelse med kilder](research/2026-10-13-existing-work.md) og en [projektoversigt](research/2026-10-13-projects.csv) er gemt. Konkurrenternes tests og genereringsforløb er ikke kørt i undersøgelsen.

Vi har ikke dokumenteret, at den samlede GDD-idé er ny. Et muligt bidrag er den enkle pseudo-graf med pseudokode, den gradvise præcisering og en ensartet agentprotokol. Fordelene skal demonstreres. Genbrug, adapters eller egen implementation pr. mekanisme er endnu ikke valgt.

## Eksisterende materiale i projektet

- `gdd-codex-starter.zip` blev læst: 14 filer med metodetekst, agentregler, reservationsgraf, strukturel Python-linter, 15 linter-tests, prompts og gemte baseline-resultater.
- `SPEC.md` og `concert.gdd.yaml` blev derefter tilføjet og læst. De konkretiserer kontrakter, scenarier, bindings og evidenspolitik.
- De gemte baseline-resultater i zip-filen vedrører strukturel validering. De er ikke applikationsbevis og blev ikke genkørt ved læsningen.
- Reservationssystemet er et muligt demonstrationseksempel. Der er ikke implementeret en reservationsapp, en GDD-kodegenerator eller et nyt valideringsværktøj i denne samtale.
- Denne scratchpad samler den videre retning. Den eksisterende specifikation og eksempelgraf skal senere revideres eksplicit, hvis de nye metodebeslutninger skal indarbejdes der.

## Det næste metodearbejde

Find det mindste sæt begreber, som kan bære arbejdsgangen. Kandidater er komponent, entitet, handling, regel, scenarie, relation, binding og evidens.

Præcisér derefter:

- Hvilke relationer er nødvendige, og hvad betyder de?
- Hvad skal være angivet, før en handling er klar til implementering?
- Hvordan udtrykkes åbne spørgsmål og antagelser?
- Hvordan findes et tilstrækkeligt udsnit uden at miste globale krav?
- Hvordan ændres modellen og implementeringen sammen, mens kravets autoritet bevares?
- Hvilke dele af pseudokoden kræver defineret semantik, og hvilke kan forblive reviewet fritekst?
- Hvordan repræsenteres communities, UI-forløb og DESIGN.md-referencer?
- Hvordan importeres Graphify-observationer uden at blande dem sammen med accepterede krav?
- Hvordan opbygges og vedligeholdes videnslaget, og hvordan spores modellen tilbage til input?
- Hvad er minimumskravene for at kalde et projekt eller et værktøj GDD-kompatibelt?
- Hvordan repræsenteres projektets egne arbejdsmåder, godkendelsesregler og mandat til automatisk fortsættelse?
- Hvordan defineres undergrafer og kontrakter mellem detaljeringsniveauer?
- Hvordan deklareres, versioneres og valideres udvidelser til kernen?
- Hvilke første validator-diagnoser og output skal agenter kunne regne med?
- Hvilke visualiseringer og integrationsflader behøver den første referenceimplementation?

Reservationsmodellen bruges foreløbigt som illustrativt materiale til at formulere standarden. Et senere praktisk forsøg kan implementere en handling, ændre en regel gennem grafen og indføre en bevidst fejl for at udfordre kontrollen. Forsøget er ikke den aktuelle opgave, og der er endnu ikke valgt et demonstrationprojekt.

Eksisterende projekter som ZeroRepo og Spec Growth Engine skal indgå i sammenligningen. Se [undersøgelsen af eksisterende arbejde](research/2026-10-13-existing-work.md). Genbrug af deres mekanismer og valg af frameworkmotor er endnu ikke besluttet.
