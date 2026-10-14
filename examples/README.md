# Standardeksempler

Standardfilerne nedenfor illustrerer format og metode. Det særskilte [agent-native-eksempel](agent-native/README.md) viser et faktisk udført, afgrænset kodeworkflow med rød/grøn-tests og revisionsbundet accept.

- `sketch.gdd.yaml`: gyldig minimal skitse; action mangler kontrakt og er ikke parat til implementation.
- `concert.gdd.yaml`: eksplicit migration af det eksisterende reservationsmateriale. Oprindelige node-, edge-, rule-, scenario-, machine- og transition-ID'er bevares. Nye scenarios gør fejl-/risikodækning strukturelt eksplicit.
- `concert.records.yaml`: planned bindings og checkplaner. Ingen udførte appchecks eller opfundet PASS-evidens.
- `concert.snapshot.json`: reproducerbart modelsnapshot, som de planlagte records refererer til. Det omfatter de normative modelassets; de planlagte designkilder er ikke en accepteret designkontrakt.
- `refinement.gdd.yaml`: heterogene børn under en action samt to forskellige relationer mellem samme endpoints. Skitse, ingen påstand om semantisk refinement eller readiness.
- `extension.gdd.yaml` og `extensions/sensors.extension.json`: lille custom nodetype/relation med pinned extensionasset.
- `fragmented.gdd.yaml` og `note.fragment.yaml`: én logisk model fordelt i lokale filer.
- `illustrative.report.json`: schemaeksempel med UNKNOWN, eksplicit ikke en udført validatorrapport.
- `sources/legacy-concert.gdd.yaml`: præcist snapshot af den tidligere intent som migrationskilde.

Frontend i koncertmodellen er med vilje en skitse. `source.design` peger på et planlagt DESIGN.md, som ikke findes; der er ikke opfundet et designgrundlag. Et screen-scope er derfor ikke implementeringsklart. Backend-kontrakterne illustrerer standardens struktur; deres faglige tilstrækkelighed og faktisk readiness skal vurderes af en senere validator/reviewer.

Der er ikke valgt workflowpolicy i standardeksemplerne nedenfor. Manglende policy forhindrer en implicit implementeringsautorisation. Et konkret projekt tilføjer sin egen policy og et betroet mandat.

Standardeksemplerne bruges til format- og metodesemantik. Det praktiske tilbudseksempel er adskilt under `agent-native/`; det udsteder ingen implicit autorisation til andre projekter.

[concert.graph.html](concert.graph.html) er en genereret, interaktiv og offline [GDD-visning](../VIEWER.md) af koncertmodellen. Den er read-only, indeholder ingen applikationsevidens og er ikke et normativt modelasset.
