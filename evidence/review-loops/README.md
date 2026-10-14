# Review- og adversarial-loops

Afsluttet 13. oktober 2026: **10 runder**, **57 bekræftede fundgrupper rettet med regressioner**. Den tiende friske samlede runde fandt ingen nye bekræftede fejl eller uafklarede kandidater. Brugerens maksimale ti runder er gennemført.

Scope var hele det aktuelle GDD-framework: standardkontrakter, indlæsning/model/readiness/planlægning, records/autorisation/evidens, Graphify, viewer, CLI/API, schemas og pakning. Fire parallelle read-only underagenter gennemgik afgrænsede områder; lead reproducerede, triagerede og integrerede alle ændringer. Workerpatches blev forberedt i tempkopier. Arbejdsområdet er fortsat uden Git; historiske dokumenter, evidens og distributioner er bevaret.

| Runde | Nye fundgrupper | Samlede tests | Resultat |
| --- | ---: | ---: | --- |
| 1 | 19 | 310 | Rettet og verificeret |
| 2 | 11 | 456 | Rettet og verificeret |
| 3 | 10 | 542 | Rettet og verificeret |
| 4 | 6 | 691 | Rettet og verificeret |
| 5 | 3 | 777 | Rettet og verificeret |
| 6 | 3 | 940 | Rettet og verificeret |
| 7 | 2 | 1.072 | Rettet og verificeret |
| 8 | 1 | 1.106 | Rettet og verificeret |
| 9 | 2 | 1.230 | Rettet og verificeret |
| 10 | 0 | 1.230 | Ren frisk runde |

[loops.json](loops.json) registrerer hvert fund, område, prioritet, rettelsesstatus og kontroloutput. De enkelte [loopnoter](loop-10.md) beskriver årsag, afgrænsning og friske kontroller; loop-01.md til loop-10.md dækker forløbet. Før-logs er bevidste negative reproduktioner, ikke den aktuelle status.

## Aktuel verifikation

- **1.230 pytest-tests består**: [samlet output](loop-09-tests.txt).
- **92/92 conformance-forventninger matcher**: [58 standardcases](loop-09-standard.json), [18 Graphify-cases](loop-09-graphify.json), [16 renderer-cases](loop-09-renderer.json).
- **23/23 offline browserchecks består**: [Chrome-rapport](loop-09-browser/browser-conformance.json). Separate friske browserkontroller i loop 10 bestod også uden script-/konsolfejl eller netværk.
- Den sidste runde ændrede ingen runtime-, schema-, test- eller normative filer. [121 filhashes](reviewed-state.json) binder den friske runde til den integrerede kode ovenfor; uændret status dokumenteres i [slutkontrollen](final-reviewed-state-proof.json). Derfor genbruges den beståede samlede kontrol fra loop 9 sammen med de friske områdekontroller fra [loop 10](loop-10.md).
- Version **0.4.1**, standardformat **GDD 0.2**, Graphify-importer **0.1.2**, renderer **0.1.3**. Koncertdemoens HTML-bytes er uændrede efter de sidste rettelser.

Den informative renderer-case viewer.deep_payload forventer nu UNKNOWN ved decoderkapacitet; den er en balanceret JSON-fixture. Kendte syntax-/payload-/assetmodstridscases forbliver FAIL. De oprindelige 58 normative caseforventninger er bevaret. Se [loop 9](loop-09.md).

## Distribution og installation

Det aktuelle wheel er installeret i et nyt miljø uden for projektmappen. **17 CLI-kontroller består**, inklusive render/verify/readiness, Graphify import/verify/byteidentisk export/propose, raw-filnavnsprofil, unsupported Graphify-dybde med recovery/proposalrefusal, præcise mixed numeric bounds, numerisk/parserkapacitet og ukendt primær model med sekundær recordlog. Genereret HTML matcher den vedlagte demo præcist.

[Installeret-wheel-kontrol](final-wheel-smoke.json), [reproducerbart script](final-wheel-smoke.source.py) og [package/source-match](final-package-source-match.json) dokumenterer den faktiske wheelhash og byteidentitet for alle 34 runtime-, viewer- og schemafiler. Wheel og sourcearkiv ligger i ../../dist/. Slutkontrol af sourcearkivet registreres i [sdist/source-match](final-sdist-source-match.json).

Den vejledende [agent-readability-lint](final-agent-lint.json) rapporterer 20 AR002-hints om nært ens kodefragmenter; de er ingen bekræftede adfærdsfejl eller hard release-gate. Reviewet tilføjer ingen applikationskørsel: hostmandater, receipts og evidens i testmaterialet er syntetiske. De understøttede profiler og deres faktiske begrænsninger beskrives i ../../VALIDATOR.md, ../../GRAPHIFY.md og ../../VIEWER.md.
