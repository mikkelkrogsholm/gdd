# Eksisterende arbejde før udvikling af GDD

Undersøgt 13. oktober 2026. Udgangspunkt: brugerens SPEC.md, concert.gdd.yaml og vision om en redigerbar pseudo-graf med pseudokode som grundlag for AI-genereret software.

## Konklusion

Andre har allerede implementeret væsentlige dele af ideen, og der findes meget tæt konceptuelt overlap. Microsofts RPG/ZeroRepo og Spec Growth Engine er de vigtigste offentlige implementeringer at sammenligne med. grAIph ligger tæt på den samlede vision, men den aktuelle implementering beskrives som privat. Kenneth Tyler bruger allerede navnet Graph-Driven Development om et beslægtet koncept og har offentlig kode.

Undersøgelsen fastslår ikke, at nogen løsning opfylder alle vores ønsker. Den giver heller ikke grundlag for at hævde, at kombinationen er ny. Et muligt særkende skal demonstreres gennem en konkret notation og arbejdsgang.

## 1. Microsoft RPG / ZeroRepo / CoderMind

ZeroRepo implementerer krav → Repository Planning Graph → repository. Grafen forbinder funktionalitet med implementeringsstruktur og afhængigheder; kodegenerering følger grafen og bruger testvalidering. RPG-Encoder går fra kode til graf. CoderMind stiller grafarbejdet til rådighed for coding agents. Der findes offentlig implementering og en ICLR 2026-publikation, oprindeligt publiceret september 2025. [Repository](https://github.com/microsoft/RPG-ZeroRepo), [paper](https://arxiv.org/abs/2509.16198).

Jeg har også læst udvalgte dele af kodegeneratoren og dens prompts ved commit `a4e6966c4ff59d84299bc56d6a47ae1f4f7b2885`. Der findes særskilte forløb for testgenerering, kodegenerering og testkørsel. [Kodegenerator](https://github.com/microsoft/RPG-ZeroRepo/blob/a4e6966c4ff59d84299bc56d6a47ae1f4f7b2885/zerorepo/code_gen/code_gen.py).

Vurdering: den stærkeste direkte reference for graf → kode. Vi skal undersøge forskellen mellem deres planlægningsgraf og vores menneskeligt redigerbare pseudokode med eksplicitte adfærdsregler. Den forskel er endnu ikke påvist som en unik capability.

## 2. Spec Growth Engine

Software beskrives som en graf af SPEC.md-filer med kontrakter og kodebindings. En agent kan implementere en node ud fra samlet kontekst. Deterministiske checks kontrollerer struktur, importafvigelser og evidensstatus. Der findes offentlig Python-implementering, CLI, cockpit og tests under Apache-2.0. [Repository](https://github.com/grabow/spec-growth-engine), [implementeringspaper fra 8. oktober 2026](https://arxiv.org/abs/2610.11725).

Udvalgt kode er læst ved commit `e2319cb8a777242491e7bb8c3dfc535190c4c537`: parser, kontekstsamling, importobservationer, driftcheck og drift-tests. Imports analyseres med heuristik/regex, og de dokumenterede begrænsninger omfatter blandt andet TS-aliaser og kontrol af import gennem offentlige kontrakter. [Kontekstsamling](https://github.com/grabow/spec-growth-engine/blob/e2319cb8a777242491e7bb8c3dfc535190c4c537/cockpit/engine/context.py), [importanalyse](https://github.com/grabow/spec-growth-engine/blob/e2319cb8a777242491e7bb8c3dfc535190c4c537/cockpit/engine/evidence.py), [begrænsninger](https://github.com/grabow/spec-growth-engine/blob/e2319cb8a777242491e7bb8c3dfc535190c4c537/cockpit/README.md#not-yet-implemented).

Vurdering: meget stort overlap med vores kontrakter, bindings, afgrænset agentkontekst og verifikation. Mulig base eller referenceimplementering; genbrug er ikke besluttet.

## 3. grAIph

Beskriver et visuelt udviklingsmiljø, hvor grafen styrer kontekst, genereringsrækkefølge, kontrakter, kodekandidater og kontrol. Den offentlige status angiver en privat udviklingsimplementation med lokal evidens og uafsluttet releasekvalifikation. Den er derfor ikke verificeret som en offentlig, genbrugelig implementering i denne undersøgelse. [Projekt](https://github.com/mateors1/grAIph), [status](https://github.com/mateors1/grAIph/blob/main/CURRENT_STATE.md).

Vurdering: meget tæt konceptuelt overlap med en samlet grafbaseret arbejdsflade. Brug som designreference, og hold dens egen statusrapport adskilt fra uafhængig verifikation.

## 4. Kenneth Tyler: GraphDrivenDevelopment

Forfatteren beskriver grafen som den autoritative repræsentation og kode, skema og tests som afledte projektioner. Der findes et offentligt Node/PostgreSQL-projekt med intent-operationer, afhængighedsnavigation, MCP og tests. [Forfatterens beskrivelse](https://thetimethatremains.substack.com/p/graph-driven-development), [repository](https://github.com/kentyler/GraphDrivenDevelopment).

Jeg har læst udvalgte operationer ved commit `30ad5713bae5d074e5ab877e54bbd5468c36d057`. `buildProjection` samler en grafkontekst; den er ikke i sig selv en kildekodegenerator. `recordExpression` registrerer artefakter og deres forbindelse til intentioner. [buildProjection](https://github.com/kentyler/GraphDrivenDevelopment/blob/30ad5713bae5d074e5ab877e54bbd5468c36d057/src/operations/buildProjection.js), [recordExpression](https://github.com/kentyler/GraphDrivenDevelopment/blob/30ad5713bae5d074e5ab877e54bbd5468c36d057/src/operations/recordExpression.js).

Vurdering: både navnet og den overordnede vision har en direkte forgænger. Det læste materiale dokumenterer ikke en komplet pseudo-graf/pseudokode-til-kode-compiler.

## 5. Intent Architect og JetBrains MPS

Intent Architect kombinerer modeller med deterministisk og AI-baseret kodegenerering. Dokumentationen beskriver eksempelvis modellering af Angular-navigation og services, efterfulgt af AI-genererede views, samt AI-implementering af handlers. [Angular-workflow](https://docs.intentarchitect.com/application-development/modelling/ui-designer/angular-modeling/angular-modeling.html), [AI-handlergenerering](https://docs.intentarchitect.com/whats-new/2025/06/index.html).

JetBrains MPS er en etableret reference for egne sprog, strukturerede modeller og transformation fra domænemodeller til kildekode. [MPS-koncept](https://www.jetbrains.com/mps/concepts/).

Vurdering: model → kode er veletableret. Vores mulige bidrag er en enklere, gradvist præciseret notation og dens samspil med agenter og kontroller; det skal afprøves.

## 6. Andre relevante projekter

- **Intent Framework:** eksperimentelt TypeScript-framework med en semantisk applikationsgraf og materialisering af UI/runtime. Dets README angiver alpha-status og at det endnu ikke er en compiler. [Repository](https://github.com/intent-framework/intent).
- **NaCl:** offentlig metode og værktøjer, der forbinder analyse, arkitektur, udvikling og verifikation omkring en Neo4j-graf. [Repository](https://github.com/ITSalt/NaCl).
- **AINL:** et konkret sprog med canonical graph IR, runtime og adapters til AI-workflows. Relevant for at lære om eksplicit semantik; anvendelsesområdet er primært AI-workers og workflows. [Produktbeskrivelse](https://www.ainativelang.com/product), [repository](https://github.com/sbhooley/ainativelang).
- **Proof:** leverandøren beskriver en software intent graph med forbindelser til kode, tests og evidens samt analyse af ændringers konsekvenser. Funktionaliteten er ikke afprøvet her. [Produktbeskrivelse](https://reqproof.com/product).
- **Graphito:** npm-pakken beskriver GDD som en dokumentationsgraf med afhængighedsresolution og validering. Den angivne GitHub-kilde gav 404 ved undersøgelsen; en aktuel offentlig repositoryimplementation blev derfor ikke verificeret. [npm](https://www.npmjs.com/package/@graphito/core).
- **Atomic og Grafema:** bruger også betegnelsen graph-driven development, med fokus på henholdsvis ændrings-/intent-sporbarhed og kodeanalyse. [Atomic](https://atomic.dev/index.html), [Grafema](https://grafema.dev/).

Et vivaxy-plugin med GDD-navnet blev fundet i kataloger, men de angivne GitHub-repositories gav 404. Det tæller som et ubekræftet spor, ikke en verificeret aktuel løsning.

## Betydning for vores framework

Anbefalingen er at sammenligne især ZeroRepo og Spec Growth Engine med ét identisk eksempel, før vi beslutter at bygge alle mekanismer selv. Eksemplet kan være den eksisterende reservationsmodel og en efterfølgende ændring af en regel.

Vores ønskede brugeroplevelse er fortsat: mennesket redigerer pseudo-graf og pseudokode; agenten implementerer en afgrænset ændring; resultater og evidens forbindes til modellen. Det væsentlige forsøg bliver, om den oplevelse kan gøres enklere og mere præcis end de eksisterende tilgange.

Der er ikke udpeget en vinder, installeret konkurrentværktøjer eller ændret den accepterede GDD-specifikation. Næste tekniske beslutning bør være genbrug, adapter eller egen implementering pr. mekanisme.

## Undersøgelsens afgrænsning

Grundlag: websøgninger, primære repositories, officielle produktdokumenter, forskningspapers, GitHub-metadata og udvalgt kildekode. Projekternes tests og kodegenereringsforløb er ikke kørt. Testfiler og forfatterrapporter er ikke behandlet som uafhængigt bevis for korrekthed. Feltet ændrer sig hurtigt; negative søgeresultater dokumenterer ikke fravær af en løsning.
