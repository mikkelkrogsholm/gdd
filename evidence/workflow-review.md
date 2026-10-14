# Agent-workflow 0.1 — pakke 0.5.0

Leveret et værktøjslag oven på GDD 0.2: komplette agentpakker, særskilt
implementeringsscope og læsekontekst, deklareret parallelitet, afhængighedskø,
atomiske events, genoptagelse og gated integration/accept. Eksisterende native
agenter udfører kodearbejdet. Der er ingen ny runtimeafhængighed eller ændring af
GDD coreformatet. [Kontrakt og brug](../WORKFLOW.md).

| Faktisk kontrol | Resultat | Evidens |
| --- | --- | --- |
| Fuld pytest-suite | 1.328 PASS, 0 fejl | [JUnit](workflow-pytest.xml) |
| GDD-standard | 58/58 | [JSON](workflow-core-conformance.json) |
| Graphify | 18/18 | [JSON](workflow-graphify-conformance.json) |
| Viewerartifact | 16/16 | [JSON](workflow-visualization-conformance.json) |
| Nye workflow-forventninger | 20/20 | [JSON](workflow-conformance.json) |
| Offline Chrome | 23/23, ingen netværkskald/scriptfejl | [JSON](workflow-browser/browser-conformance.json) |
| Installeret wheel uden for checkout | 18 CLI-trin; 38 runtime-/asset-/schemafiler byteidentiske | [JSON](workflow-wheel-smoke.json), [reproducerbar kontrol](workflow-wheel-smoke.source.py) |
| Faktisk native kodearbejde | 18 tests/7 fejl → 19 PASS; 10 events; aktuel accept | [Demo](../examples/agent-native/README.md), [summary](../examples/agent-native/evidence/summary.json) |

Fire read-only underagenter undersøgte model/planning, protokol/persistens,
CLI/demo og adversarial inputs parallelt. Root var eneste checkoutwriter.
Workers leverede testforslag og senere faktiske kodeændringer i tempkopier.
Tre udstedte pakker blev udført: discount og tax parallelt, quote efter begge
integrationer. De oprindelige floor-assertions blev bevaret. Den nye regression
kontrollerer faktisk CLI-output for `10**30+1` uden flydende afrundingsfejl.

Bekræftede fund er rettet med regressioner: codebase/spec-base blev adskilt;
uparat scope kan ikke skjules af parallel-PASS; fulde pakker autentificeres mod
modellen før run-publicering; falsey hostinputs og newline-ID'er afvises;
dispatch/integration bindes til hostens konkrete implementation/kodebase;
dangling modelreferencer afvises før slicing; historiske record-only diagnoser
blokerer ikke gyldige scoped gates; cancellation og sene submissions bevares
ved en utilgængelig model. Idempotency kontrolleres før stale-head-afvisning,
attempts genbruges ikke, og konkurrerende publishers kan kun committe én event.

Accept kræver separat aktuel evidens og autorisation, præcis integreret
implementation/koderevision, frosne checkvalg og afklarede acceptance-spørgsmål.
Tests bruger syntetiske receipts til protokolnegative cases. Demoens
application-evidence er derimod baseret på faktisk udført kode og CLI, med rå
resultater, negative control og hostverificerede bindings. Worker-PASS blev ikke
brugt som integrationsbevis. Installeret wheel gennemførte et nyt faktisk replay.

Agent-readable-linteren finder 19 AR002-advisories i eksisterende kodeblokke og
ingen nye stærke boundaryfund. [Rå advisoryliste](workflow-agent-readable.json).
Den er en læsbarhedshjælp og ikke en implementationsevaluator.

V1 har et bevidst begrænset ansvar: hosten beskytter eventkald/filsystem,
etablerer faktisk arbejdsisolation, transporterer agentopgaver, udfører review,
merge og checks samt leverer eksplicit trust. Loghashes er tamper-detektion,
ikke kryptografisk autentificering. Modeldrift kræver en ny plan/run med bevaret
historik; integrerede jobs rebases eller invalideres ikke automatisk.

[Maskinlæsbar samlet status og kildehashes](workflow-review.json). Tidligere
versioners evidens og distributioner er bevaret. Den aktuelle stopgrænse er nået.
