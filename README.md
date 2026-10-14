# Graph-Driven Development

GDD er en foreslået, agentuafhængig standard for at udvikle software gennem en pseudo-graf med pseudokode.

**Input → vedligeholdt viden → modeliterationer → afgrænset udvikling → evidens.**

Agenten organiserer input, modellerer struktur og adfærd og præciserer den relevante del gennem iterationer. Mennesket undersøger resultatet gennem afledte visninger og giver feedback. Grafen forbinder funktioner, domænebegreber, skærme, kontrakter og regler.

## Status

**GDD 0.2 — standardudkast, 13. oktober 2026.** Dette er et udviklet metode- og formatforslag, ikke en etableret industristandard. Protokollen er uafhængig af leverandør, agent, kodesprog og bestemt CLI.

Pakken indeholder standardtekster, JSON Schemas, illustrative modeller og en **validator for GDD 0.2 med Python-API og CLI**. Pakken har en [Graphify-adapter](GRAPHIFY.md), som bevarer originalbytes og alle relationer og kan forberede eksplicitte, uaccepterede GDD-forslag fra kodeobservationer. [Validatorens dokumentation](VALIDATOR.md) angiver støtte og begrænsninger. En [interaktiv, offline grafvisning](VIEWER.md) giver søgning, communities og inspektion af kontrakter og pseudokode samt rettelser fra [reviewrunderne](evidence/review-loops/README.md). Version 0.5.0 tilføjer et [agent-workflow](WORKFLOW.md), som danner fulde arbejdspakker, koordinerer afhængigheder og styrer integration/accept gennem eksisterende agenter. Et [faktisk kodeeksempel](examples/agent-native/README.md) viser parallelle rettelser og revisionsbundet rød/grøn-verifikation.

```sh
uv run gdd validate examples/concert.gdd.yaml --profile readiness --scope action.reserve
uv run gdd view examples/concert.gdd.yaml --output concert.html
uv run pytest -q
uv run python conformance/run.py --output conformance-report.json
uv run python conformance/run_graphify.py
```

Validerings-CLI'en skriver JSON i `gdd-report/0.2`; workflowkommandoerne har særskilte JSON-envelopes med indlejrede rapporter. Exit codes: 0 for beståede anmodede checks, 1 for kendte fejl/afslag, 2 for UNKNOWN/UNRESOLVED uden kendt fejl. En gyldig backendmodel kan være strukturelt parat, selv om implementationen endnu mangler.

## Læsevej

- [SPEC.md](SPEC.md): normativ model, identitet, relationer, undergrafer og udvidelser.
- [FORMAT.md](standard/FORMAT.md): filer, felter, serialisering og revisioner.
- [AGENT-PROTOCOL.md](standard/AGENT-PROTOCOL.md): input, iteration, mandat, kontekst, parallelitet og levering.
- [VALIDATION.md](standard/VALIDATION.md): gyldighed, parathed, autorisation, evidens og diagnostik.
- [INTEGRATIONS.md](standard/INTEGRATIONS.md): wiki/OKF, DESIGN.md, Graphify og visninger.
- [MIGRATION.md](standard/MIGRATION.md): overgang fra gdd/0.1.
- [DECISIONS.md](standard/DECISIONS.md): begrundelser for udkastets tekniske valg.
- [GDD.md](GDD.md): kort indgang for agenter.
- [schemas/README.md](schemas/README.md): maskinlæsbare kontrakter og begrænsninger.
- [examples/README.md](examples/README.md): standardeksempler og et afgrænset, udført agent-workflow.
- [VALIDATOR.md](VALIDATOR.md): brug, profiler, hostens tillidsgrænse og begrænsninger.
- [WORKFLOW.md](WORKFLOW.md): udviklingsplaner, agentpakker, kø og integration/accept.
- [VIEWER.md](VIEWER.md): interaktiv graf, scoped projektioner, sporbarhed og konkret artifactkontrol.
- [GRAPHIFY.md](GRAPHIFY.md): tabsfri import, profiler, observationsbundle og continuation til GDD gennem eksplicit mapping.
- [conformance/cases.yaml](conformance/cases.yaml): 58 positive og negative standardcases; køres med [run.py](conformance/run.py).
- [evidence/standard-review.md](evidence/standard-review.md): udførte kontroller og deres scope.

Projektet vælger selv arbejdsmåde og godkendelsesregler. Nye projekter starter fra input; eksisterende projekter kan starte fra Graphify-observationer. Accepteret intention og observeret implementation holdes adskilt.

En gyldig model, et strukturelt parat udsnit, autorisation og implementeringsevidens er fire særskilte vurderinger. Ingen af dem beviser automatisk de andre.

Beslutningsgrundlaget findes i [SCRATCHPAD.md](SCRATCHPAD.md). [Research](research/2026-10-13-existing-work.md) beskriver væsentligt overlap med eksisterende arbejde. Den tidligere specifikation er bevaret i [archive/gdd-0.1](archive/gdd-0.1/README.md). Rodens `concert.gdd.yaml` er fortsat 0.1; det migrerede eksempel ligger under `examples/`.
