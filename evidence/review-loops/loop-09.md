# Loop 9 — ukendt primær projektidentitet

Status: verificeret efter rettelser af R056–R057.

R056: Den sekundære recordlogs project_id blev sammenlignet med None, når den primære model ikke kunne indlæses. Ved manglende model, ukendt modelversion eller parserdybde over kapacitet gav det et opdigtet RECORD_INVALID-projektmismatch og exit1. Ingen sådan modstrid kan fastslås uden kendt primær identitet. Projekt-ID’er må kun sammenlignes, når en gdd/0.2-model faktisk er indlæst; øvrige kendte primære/sekundære fejl skal fortsat diagnosticeres, og et reelt kendt mismatch forbliver FAIL. Leadens seks førreproduktioner med/uden aktivitet er gemt separat.

R057: En gyldig model tæt på parserens dybdekapacitet kan stadig overstige rendererens JSON-serialiseringskapacitet. gdd view gav ukontrolleret RecursionError/traceback ved opbygning af inspectortekst; dybdegrænsen varierer med kaldestak/runtime. Refusal skal være kontrolleret FORMAT_UNSUPPORTED uden output, og tilsvarende manglende kapacitet under artifactverifikation er UNKNOWN frem for kendt korruption. Reelle syntaks-/payload-/assetmodstridstests skal fortsat give FAIL. Ingen vilkårlig ny dybdegrænse eller parserudvidelse kræves.

Modelområdet var rent: 573 regressioner og 164 friske probes for exact numbers, refs, uændrede kanoniske data/snapshots, scoped closure, multiedges, metodecoverage og parserkapacitet bestod. Graphify/CLI/filesystemområdet var rent: 417 regressioner og friske raw-recovery-/JSON-/YAML-capacity-/locator-/eksklusive outputkontroller bestod. Viewerens øvrige 41 pytest og 23 browserchecks bestod ud over den særskilte R057-reproduktion. Protokolområdet afsluttede med R056 som eneste fund; 42 nye secondary-input-kontroller, 248 øvrige assertions og 347 eksisterende fokuserede tests bestod.

R056 er integreret med en afgrænset primary-formatguard; de 100 nye regressioner havde 64 FAIL/36 PASS på baseline. R057 er integreret med en fælles guarded JSON-serialisering, genindlæsningspreflight før output åbnes og særskilt UNKNOWN for kapacitetsstop under verifikation. De 24 nye viewerregressioner havde 16 FAIL/8 PASS før rettelsen. Fem normale pre/post-artefakter er byteidentiske og består verifikation; renderer 0.1.3 er bevaret.

Den informative renderer-case viewer.deep_payload og dens regression forventer nu UNKNOWN: fixture er en balanceret JSON-array med dybde 2.000, hvor decoderkapaciteten nås før view-envelopen kan vurderes. Invalid JSON, kendt payload-/assetmanipulation, CRLF og dubleret payload bevarer FAIL. De 58 normative standardforventninger er uændrede. Leadens oprindelige CLI-repro giver nu exit2 uden traceback og uden outputfil.

Integreret resultat: **1.230 pytest-tests**, **58/58** standard-, **18/18** adapter-, **16/16** renderer-cases og **23/23** offline browserchecks består. Ingen browserfejl eller eksterne requests. En frisk loop 10 starter efter sidste rettelse og er den maksimale aftalte runde.
