# Valideringskontrakt

Normativt kapitel til GDD 0.2. Den konkrete implementations støtte og begrænsninger står i [VALIDATOR.md](../VALIDATOR.md); kravene i dette kapitel er ikke i sig selv udførelsesevidens.

## 1. Fire uafhængige vurderinger

| Dimension | Spørgsmål | Resultater |
| --- | --- | --- |
| model | Kan repræsentation, referencer og kendt grafsemantik valideres? | PASS / FAIL / UNKNOWN |
| readiness | Er et bestemt udsnit strukturelt specificeret til implementation? | PASS / FAIL / UNKNOWN |
| authorization | Tillader det aktuelle mandat denne aktivitet? | GRANTED / DENIED / UNRESOLVED |
| implementation | Hvad viser et konkret udført check af en implementation? | PASS / FAIL / UNKNOWN |

Integrationchecks har PASS/FAIL/UNKNOWN og deklareret scope. En gyldig skitse kan være uparat. Parathed giver ikke autorisation. Godkendelse giver ikke evidens. Et schema-PASS er alene et representation-resultat.

PASS kræver, at checket er gennemført med tilstrækkeligt input, kendt semantik og opfyldt kriterium. FAIL kræver en kendt overtrædelse. UNKNOWN betyder, at det krævede forhold ikke kan afgøres: unsupported version, manglende analyseinput, utilgængelig ekstern kontrakt eller manglende kørsel. Hvis en dimension både har en kendt fejl og uafklarede checks, er aggregatet FAIL, og de uafklarede resultater bevares. Ellers medfører et påkrævet UNKNOWN et UNKNOWN-aggregat.

Ingen implicit samlet "projektet er verificeret"-status. En assertion om verificering SKAL navngive scope, implementation, revisioner og de krævede checks.

## 2. Profiler og indlæsningsrækkefølge

1. Identificér formatversion; unknown version giver FORMAT_UNSUPPORTED/UNKNOWN. Forsøg ikke at validere den som en kendt version.
2. Sikker parsing, duplicate-key-kontrol, JSON-kompatible values, pathgrænser og includes.
3. Valider hver inputfil mod relevant lokalt JSON Schema.
4. Saml registries uden override; kontrollér identitet og alle interne referencer.
5. Kontrollér endpoints, dekomposition, maskiner, constraints og erklærede udvidelser.
6. Beregn valgt context closure og scoped readiness, hvis dette er anmodet.
7. Vurder autorisation mod betroet policy/mandat, hvis en aktivitet er anmodet.
8. Læs records og integrationsresultater separat; udfør ikke applikationschecks som en skjult del af modelvalideringen.

Ukendt extensionsemantik må ikke droppes ved trin 5/6. En consumer kan bestå representation og samtidig have model-UNKNOWN. Et modelcheck SKAL opregne understøttede core/extension-regler. Ukendte annotations er ikke normative og kan ignoreres semantisk.

## 3. Semantiske modelchecks ud over schemas

SKAL-kontroller:

- Global ID-unikhed, inklusive transitions og recordkollisioner; alle interne ref-typer og endpoint-kinds.
- Source-/question-/designreferencer samt revisionkrav for normative sources. Utilgængelig ekstern source er integration-UNKNOWN, ikke en dangling intern reference.
- Højst én owned_by-edge til en component pr. action/entity/screen/machine, når ownership er angivet. To edges med forskellige ID'er og identisk ownersemantik er en ejer-ambiguitet, ikke en ekstra ejer.
- Refines uden cykler, selvloops eller flere parents. Realization-children er descendants af den erklærende parent.
- Member_of til en community fra en ikke-community; overlapping membership er tilladt.
- Targeted rule har constrains-targets; global rule gælder uanset lokal graph-distance. Architecture-constraint endpoints skal være components, og relation er imports i denne coreprofil. Andre constraintsemantikker kræver extension.
- Machine-states er unikke; initial/terminal/from/to findes; trigger er action; `(from,trigger)` er unik; terminal state har ingen udgående mutation. State_of skal være entydig, når den angives.
- Typereferencer målretter entity. Required-objectfelter findes i properties, constraints passer til typen, og minimum må ikke overstige maximum.
- Kilde-derivation er acyklisk over source+revision-identiteter; referenced sources eksisterer. Ældre historiske revisioner må ikke rebindes til nye. Unknown relation/kind uden extensiondeklaration er fejl.
- Custom kinds/relations tilhører deklareret namespace; assetets namespace/version skal præcist matche manifestet, og bytehash skal stemme. Der må højst være én version pr. namespace og ingen navnekollisioner. Data følger extensionens schema. Core semantics må ikke omdefineres. Extension-schemas må ikke hente remote refs eller executable code.

Calls-/navigationcycles og flere forskellige relationer mellem samme endpoints er positive kontroller. Et værktøj, som kaster dem væk for at gøre grafen acyklisk eller simpel, er ikke modelkonformant.

## 4. Scoped readiness-profil

Readiness-scope er en liste af implementationstargets; læsekontekst holdes separat. Uvedkommende skitser blokkerer ikke et udsnit. Alle angivne dele skal fortsat være gyldige; kendte modeldefekter i en anden del skal vises og forhindre et fuldt model-PASS. Scoped readiness kan rapporteres separat, hvis defecten beviseligt er uden for dens closure.

For hver valgt action kræves:

- Præcis én owned_by-component og adgang til relevante interfaces/grænsekontrakter.
- Input, context, output, errors, preconditions, external_effects, effects_complete=true, dependencies_complete=true og ikke-tom behavior. Tomme maps/lister kan være bevidste erklæringer.
- Typespecifikationer og refererede entiteters relevante felter er tilgængelige.
- Alle fem riskvurderinger med rationale. Relevant risk kræver scenarier; de skal exercises handlingen. Authorization/idempotency/boundaries/atomicity kræver negative eller boundary-scenarier, concurrency kræver concurrency-scenarie. Et scenarie kan dække flere risks.
- Hvert erklæret fejludfald har mindst én error_cases-reference til et negative/boundary-scenarie, som exercises handlingen.
- Mindst ét normal-scenarie, som exercises handlingen. Faglig dækning er fortsat et reviewspørgsmål.
- Alle gældende blocking rules har planlagt scenario-dækning gennem verifies. Arkitekturregler kan bruge architecture-scenarier uden at kalde en action.
- Ingen open question i closure, som blokkerer implementation. Resolved questions kræver en gyldig accepteret decision-receipt eller må behandles som uafklarede.
- Nødvendige kontrakt-/designsources er normative=true, lokalt materialiserede, resolved og pinned med verificeret bytehash og medtaget i modelsnapshot; unknown extensionsemantik må ikke påvirke closure.

For screen kræves interaction.states, behavior, accessibility og responsive; relevante calls/navigationgrænser; et normal-scenarie og resolved design_refs. Vurdering af relevant fejl-/grænseforløb registreres som readiness_review-decision. Entity/machine/component skal have tilstrækkelige relevante felter/tilstande/interfaces, når udsnittet afhænger af dem. En top-level komponent kan kun være parat som implementeringsscope, når dens realization-forpligtelser og nødvendige child-kontrakter er specificerede og strukturelt dækkede. Børn behøver ikke allerede være implementeret. "Skitse" på en parent gør ikke et fuldt parent-scope parat.

`maturity`, en håndskrevet ready-markering, planlagte bindings eller tekstligt "reviewed" er ikke readiness-evidens. Den første validator kontrollerer denne strukturelle profil. Et semantisk review kan udfordre erklæringerne, men må ikke maskeres som en deterministisk kontrol.

## 5. Autorisation og accept

Aktivitet, scope og revision sammenholdes med projektets gates og betroet mandat. Et passende accepted approval-receipt giver GRANTED, hvis aktør og policy er gyldige. Standing mandate kan også give GRANTED. Manglende eller tvetydigt mandat giver UNRESOLVED; et eksplicit afslag eller scopebrud giver DENIED.

Alle entries i rule.verification er krævede metoder, ikke implicitte alternativer. Hver blocking rule kræver en entydig aktiv checkplan for hver deklareret metode; checkplanen skal have regel-ID'et i scope og den præcise method-label. Manglende plan eller aktuel udført evidens giver UNKNOWN, aldrig vakuøs PASS. Planvalget og implementationen skal være eksplicitte.

Implementeringsaccept kræver, at alle disse checks er aktuelle og PASS, medmindre projektets policy tillader en eksplicit exception. Exceptions identificerer ansvarlig actor, scope, implementation/code_revision, checks, begrundelse og udløb. De ændrer ikke checkets FAIL/UNKNOWN. Modelændringer og exceptions er forskellige handlinger.

## 6. Bindings og evidens

En planned binding beviser ikke file-/symbol-eksistens. En observed binding beviser ikke korrekthed. Et check skal erklære nødvendigt miljø og relevant scope. Concurrency i databasekrav kræver den faktisk valgte persistence/transaktionsadfærd; stubs kan ikke give dette check PASS.

Evidence SKAL referere til eksisterende uforanderlig checkplan og matchende check_revision, spec_revision, implementation og code_revision samt checkeridentitet/version og raw output. Evidence-scope skal dække checkplanens krævede scope; ekstra scope er ikke automatisk verificeret. Manglende miljø, manglende revisionsbundne bindings, ikke-understøttet metode eller ukendt observation-proveniens giver UNKNOWN.

Ældre evidens beholder sit historiske resultat. Under 0.2's konservative profil er dens anvendelighed UNKNOWN ved ændret model-, kode- eller normativ assetrevision. Der er ingen automatisk semantisk cache-invalidering. Negative kontroller erklæres i checkplanen og skal udføres, før den samlede kontrol kan påstå at demonstrere defektdetektion.

## 7. Rapport og diagnoser

Rapporten følger [report.schema.json](../schemas/report.schema.json): format, tool(name/version), spec_revision, profiles, scope og checks. Hvert check har check_id, dimension, result, scope, reason og diagnostics. Implementation-PASS/FAIL skal også identificere implementation, code_revision og evidence_refs; reportens spec_revision skal være kendt. Authorization-GRANTED kræver activity, policy_revision og authority_basis, som kan identificere et receipt eller et betroet standing mandate. Diagnostics har code, severity (`error|warning|info`), message, location og related_ids. Location er projekt-relative file plus JSON Pointer; filen kan være null, hvis forholdet ikke er lokalt.

Ingen opdigtet hash: spec_revision kan være null, indtil snapshot kan beregnes. Checket må da ikke påstå revisionsbundet PASS. Exit codes for et CLI er anbefalet 0=alle anmodede krævede checks PASS/GRANTED, 1=kendt FAIL/DENIED, 2=UNKNOWN/UNRESOLVED uden kendt fejl. Et API kan bruge samme resultater uden procesexit.

Den maskinlæsbare [diagnoseoversigt](../conformance/diagnostics.csv) angiver stabile codes. [Cases](../conformance/cases.yaml) angiver forventet dimension/resultat og om kontrollen er schema-, semantik- eller readinessbaseret. En implementation skal deklarere hvilke cases/profiler den opfylder.
