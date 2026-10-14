# Loop 5 — referenceidentitet, modelmodstrid og fejlrapportering

Status: verificeret efter rettelser. Frisk samlet review efter loop 4 fandt tre afgrænsede fundgrupper, R047–R049 i [loops.json](loops.json).

R047: Den fælles integritetskontrol afviste recordreferencer til kolliderende model-ID'er for checks/evidence, men manglede samme kontrol for supersedes. En brugt record med en sådan globalt tvetydig reference kunne stadig understøtte implementation-PASS, authorization-GRANTED eller readiness-PASS. Rettelsen gælder netop referenceidentiteten, uden blanket transitive invalidering af ellers uafhængige records.

R048: En imports-edge er i SPEC en tilsigtet tilladt direkte import. Den kan derfor ikke samtidig være forbudt af en deklareret architecture-constraint. Direkte og transitive forbud kontrolleres i den tilsigtede modelgraf, inklusive ikke-tomme cykliske stier. Et direkte forbud tillader fortsat en indirekte sti, og der udledes ingen komplet allowlist. Dette er modelkonsistens; faktiske implementation-imports kræver fortsat udførte checks.

R049: En dubleret JSON-nøgle med lone surrogate kunne blive interpoleret uescaped i InputError, før parserens almindelige Unicodekontrol blev kørt. Den resulterende rapport kunne ikke materialiseres som UTF-8. Graphify-import kunne derfor heller ikke publicere raw-bundle. Begge parseres diagnoser skal escape den ubetroede nøgle; inputtet forbliver FAIL, og Graphify bevarer originalbytes til eksakt recovery.

Viewerområdet var rent: renderer 0.1.3 bestod 23/23 offline browserchecks og 41 målrettede renderer-/scope-regressioner. Lange titler, seks viewportstørrelser, tomme communities, scenarie-scope, fold/reset og tastaturvalg blev afprøvet uden scopeudvidelse, paneloverlap, script-/konsolfejl eller netværk.

Øvrig frisk kontrol dækkede 299 relevante records-/authority-/evidence-/envelope-tests, 83 uafhængige model/tag/type/closure/source/parallel-assertions samt 184 adapter-/CLI-regressioner, 18/18 adaptercases og 41 selvstændige integrationskontroller. De tre fund er reproduceret særskilt fra disse positive kontroller. Lead skriver fortsat alle ændringer; workers forbereder kun temp-patches.

Integreret resultat: **777 pytest-tests**, **58/58** standard-, **18/18** adapter-, **16/16** renderer-cases og **23/23** offline browserchecks består. Ingen script-/konsolfejl eller eksterne requests. Alle tre fundgrupper er rettet med regressioner. En frisk loop 6 er startet efter sidste rettelse.
