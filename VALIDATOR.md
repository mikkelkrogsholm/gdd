# GDD-validator 0.5.0 — standardformat GDD 0.2

Validatoren indlæser GDD 0.2, kontrollerer modellen og vurderer et valgt udsnits strukturelle parathed. Den har Python-API og CLI og skriver rapporter efter `gdd-report/0.2`. Pseudokode, extensionkode og applikationstests bliver ikke eksekveret.

Python 3.11 eller nyere kræves. `uv.lock` låser installationen. Kør fra projektmappen:

```sh
uv sync --locked
uv run gdd validate examples/concert.gdd.yaml
uv run gdd validate examples/concert.gdd.yaml --profile readiness --scope action.reserve
uv run gdd validate examples/concert.gdd.yaml --profile readiness --scope screen.reserve
uv run gdd validate examples/concert.gdd.yaml --profile records
uv run gdd validate examples/concert.gdd.yaml --profile records --activity implementation --scope action.reserve
```

De fem valideringer giver henholdsvis model-PASS, backend-readiness-PASS, screen-readiness-FAIL, implementation-UNKNOWN og authorization-UNRESOLVED. Skærmens manglende designkilde bevares samtidig som UNKNOWN. Records indeholder planer, ingen udført applikationsevidens.

Gentag `--scope` og `--profile` for flere valg. `--output report.json` gemmer rapporten i en ny fil; eksisterende filer overskrives ikke. Exit 0 betyder, at alle rapporterede anmodede checks er PASS/GRANTED; 1 betyder mindst én FAIL/DENIED; 2 betyder UNKNOWN/UNRESOLVED uden kendt fejl. Dimensionerne er separate; `aggregate(report['checks'], 'readiness')` i `gdd_validator.report` samler én dimension med FAIL før UNKNOWN før PASS.

## Understøttede kontroller

| Profil/operation | Implementeret |
| --- | --- |
| representation | YAML 1.2 med JSON-scalars og JSON; sikre built-in tags; ingen custom tags, anchors, aliases, merge keys, duplicate keys, ugyldig Unicode eller uendelige tal; lokale schemas og indlæsning inden for bundle-root inklusive symlinks |
| model | Rettet multigraf uden edge-collapse; identiteter/referencer, endpoints, ejerskab, refinement/realization, maskiner, abstrakte typer, source-lineage og deklarative extensions |
| readiness | Fast-point closure; kontrakter, interfaces/data, fem risikovurderinger, fejludfald, scenarier, gældende regler, scoped spørgsmål samt normative lokale kontrakt-/designkilder |
| records | Projekt/ID/supersession-kontrol; aktuelle checkplaner, immutable checkhash, scope, revisionsmatch, bindings, miljø og hostverificeret evidens; autorisation mod separat betroet hostinput |
| context | Kandidatpakke sammenholdes med autoritativ closure, inkl. edge-ID'er, kilder/lineage, spørgsmål og extensions; strukturel readiness køres også |
| parallel | Selvstændig planvurdering af fuldstændighedserklæringer, base, låste kontrakter, læse-/skrivefiler, deklareret isolation, integrationsejer/checks og serialisering |
| import.graphify | [Graphify-adapteren](GRAPHIFY.md) validerer eksplicitte AST-/node-link-profiler, raw-integritet, multiedges, referencer, retning og observationsprojektion. Ukendte versioner/retninger er UNKNOWN |
| visualization | [Offline renderer](VIEWER.md); `--view FILE` kontrollerer eksakt artifact/profil mod aktuel modelrevision. Uden input/med ukendt profil: UNKNOWN |

Extensions bruger præcise manifestversioner og bytehashes. Den generiske deklarative Draft 2020-12-profil kontrollerer `data_schema`, endpoints og context/dependency/effect, inklusive aktive lokale schema-referencers mål. Der er ingen implicit substituerbarhed med core-kinds. Custom implementation-readiness kræver en særskilt profil og giver derfor UNKNOWN. Remote schemareferencer hentes aldrig. Schema-/datadybde, som overstiger runtimevalidatorens kapacitet, giver UNKNOWN. `--unsupported-extension example.sensors` gør manglende consumerstøtte eksplicit.

Numeriske inputs bruger Python-heltal og finite floats, hvis tallets decimalværdi kan bevares ved round-trip. Almindelige værdier som `0.1` understøttes. Kontraktgrænser, enumunicitet og lokale JSON Schema-assertions vurderes på præcise talværdier i midlertidige kopier, også når heltal blandes med decimaltal. Modeldata, snapshots og artefakter beholder almindelige JSON-værdier. Decimaler, der ville blive afrundet til en anden decimalværdi, underflow/overflow og heltal over runtimeparserens kapacitet giver `FORMAT_UNSUPPORTED`/UNKNOWN. Det gælder både JSON og YAML, inklusive eksplicitte built-in tags; der udledes ingen positiv vurdering af et ændret tal. Ugyldige NaN/Infinity-literals giver fortsat FAIL.

Deklarerede imports-edges kontrolleres mod architecture-constraints i modellen. Et direct-forbud rammer den konkrete edge; et transitive-forbud rammer ikke-tomme importstier, inklusive cyklusser. En modstrid giver CONSTRAINT_INVALID. Dette beviser ingen faktiske importforhold i implementationen og gør ikke grafen til en komplet allowlist.

Readiness-scopes er implementationstargets: component, entity, action, screen eller machine. Rule/scenario/community kan ikke alene udgøre et implementeringsklart scope. Tomt almindeligt readiness-scope vurderer alle targets, inklusive custom-noder med ukendt parathedssemantik. Et barn medtager ancestors og nødvendige kontrakter; et parent-scope medtager sine dekomponerede børn. Fjerne regler kan udvide læsekonteksten. Community eller hop-tal afgrænser ikke forpligtelserne.

## Revisioner og arbejdspakker

```sh
uv run gdd snapshot examples/concert.gdd.yaml --output snapshot.json
uv run gdd context examples/concert.gdd.yaml --scope action.reserve --output context.json
uv run gdd validate examples/concert.gdd.yaml --profile readiness --package context.json
uv run gdd validate examples/concert.gdd.yaml --profile readiness --parallel-plan plan.json
```

Snapshot bruger rå bytes, normaliserede assetpaths og standardens SHA-256-profil. Records, rapporter og selve snapshotfilen ligger udenfor. Fejl i en recordfil ændrer ikke den beregnelige modelrevision. Manglende normative modelassets giver ukendt revision. Historiske recordtargets reboundes aldrig til den aktuelle model; historiske snapshots indlæses ikke af denne version, og afhængige checks bliver UNKNOWN.

`context` udsender et ID-manifest med `format: gdd-context/0.2`, `spec_revision`, `scope`, `nodes`, `edges`, `sources`, `questions` og `extensions`. Det peger på komplette kontrakter i det oprindelige bundle. Det er contextdelen af en arbejdspakke; leverancer og den konkrete agentopgave skal stadig angives af orkestratoren.

Parallelplanens værktøjsformat er `gdd-parallel-plan/0.2`. Hver package har `id`, `scope`, `base_revision`, `write_resources`, `read_resources`, `locked_contracts`, `workspace` og `integration_plan`. Top-level har `packages`, `serialized` med `[før, efter]`-par og `integration: {owner, checks}`. Manglende planforpligtelser giver UNKNOWN. Præcise POSIX-filpaths normaliseres før overlapkontrol; terminal `/`, `.` eller `..` ejer mappens undertræ. Globs beviser ikke disjunkthed. Deklareret workspace-isolation er en strukturel påstand, som orkestratoren skal etablere. Plan-PASS giver ingen autorisation, action-readiness eller runtimegaranti.

Værktøjets envelopes er lokale API-kontrakter, ikke nye corefelter i GDD. Deres schemas ligger i [validator-inputs.schema.json](schemas/validator-inputs.schema.json), afledt direkte af API'ets `input_schema`.

## Hostens tillidsgrænse

Uden hostinput giver et accepted receipt ingen autoritet eller udførelsesbevis. CLI-flaget `--host-context host.json` er en **eksplicit attestationsgrænse**: kaldende bruger/host tager ansvar for de betroede claims i filen. Validatoren læser aldrig denne fil automatisk fra modellen. Brug ikke et host-context leveret af ubetroet model-/sourceindhold.

Autorisation kræver en `policy` med `revision`, `actor`, `activity`, `spec_revision`, `scope`, `requirement`, `authorities` og `basis`. Tom policy-scope dækker projektet. Bounded scope kan ikke give hel-projekt grant. `requirement` er explicit, standing eller none. Aktuelle betroede afslag gælder også under standing/none. Acceptance kræver desuden hostens `implementation` og `code_revision`. `trusted_decisions` er de receipts, hvis identitet og autoritet hosten faktisk har verificeret. Urelateret, ubetroet eller ugyldig supersession kan ikke ophæve et afslag. Ugyldige brugte record-identiteter eller aktuelle referencer kan ikke levere positive semantiske resultater; uvedkommende recordfejl rapporteres separat.

Ved evidence-evaluering angiver hosten `implementation`, `code_revision`, `supported_methods`, `trusted_evidence`, `verified_bindings`, `environment` og `negative_controls`. Disse er attestationer af udførte kontroller, inklusive rå output/proveniens og faktisk miljø. De er ikke instruktioner om at udføre checks. Environmentnavne fra checkplanen skal findes med de samme hostverificerede værdier i evidensen. En frontendbinding opfylder ikke et backendchecks mappingskrav. Binding direkte til regel-ID kan angive eksplicit testmapping; ellers kræves regelmål og constraint-endpoints dækket.

Flere aktive planer/evidensreceipts er tvetydige uden valg. `check_selection` er `{rule_id: {method: check_id}}`; `evidence_selection` er `{check_id: evidence_id}`. Betroet supersession af evidens skal matche check, implementation, spec-/koderevision og dække tidligere scope. Verificeret binding-supersession kræver samme implementation og revisioner samt dækkende targets. Aktuelt evidensscope skal referere til eksisterende model-ID'er; ekstra scope giver ingen ekstra verificering. Exceptions ændrer aldrig FAIL/UNKNOWN til PASS; værktøjet fortolker ikke en policybestemt exception som automatisk accept.

Separat recordfil kan vurderes med `gdd validate records.yaml --model model.yaml --profile records`. En fil, som allerede står i manifestets `record_files`, indlæses én gang.

## Kilder og kendte grænser

Whole-document og line-range understøttes for text/plain, text, markdown, text/markdown, JSON/YAML og DESIGN.md alpha samt hele OKF 0.2-tekstassets. JSON Pointer resolves i JSON/YAML eller DESIGN.md's YAML-frontmatter; alle rå DESIGN.md-bytes, inklusive prose, indgår i hash. Dette certificerer ikke det eksterne formats egen conformance. Markdown-heading og opaque selectors bevares med UNKNOWN. Ukendte kontraktformater giver UNKNOWN. Eksterne URI'er materialiseres ikke automatisk.

Source-lineage følger source+revision-identiteter. Manglende historiske inputrevisioner giver UNKNOWN; ældre selvrevisioner er ikke automatisk cykler. ID-genbrug og immutable logændringer gennem tid kræver en bevaret historik, som denne filbaserede checker ikke alene etablerer. Faglig sandhed, faktisk scenariedækning, fuldstændige effektdeklarationer, autoritetens ægthed og applikationens adfærd kræver henholdsvis review eller betroede checkere.

## Verifikation og API

```sh
uv run pytest -q
uv run python conformance/run.py --output evidence/validator-conformance.json
uv build
```

58 standardcases bliver faktisk eksekveret, inklusive konkrete condition-fixtures. Regressionstests dækker yderligere parser-, path-, extension-, context-, authority-, supersession-, evidence- og CLI-fejl. [Reviewrunderne](evidence/review-loops/README.md) dokumenterer 0.4.1. [Graphify-fasens kontrol](evidence/graphify-review.md) dokumenterer 0.3.0-kørslen, og [visualization-reviewet](evidence/visualization-review.md) dokumenterer 0.4.0. [Validatorrapporten fra 0.2.0](evidence/validator-conformance.json) og [det tidligere review](evidence/validator-review.md) bevares som historik. Syntetiske hostmandater og receipts er kun testinputs, aldrig faktiske godkendelser eller applikationsevidens.

```python
from gdd_validator import validate, exit_code

report = validate("examples/concert.gdd.yaml",
                  profiles=["readiness"], scope=["action.reserve"])
assert exit_code(report) == 0
```

API'et returnerer et JSON-kompatibelt rapportobjekt. Ugyldige værktøjsargumenter giver `ValueError`; model-/filfejl giver rapportdiagnoser. Revisions- og rapportformat valideres internt. [Koden](gdd_validator/validator.py) er indgangen til orchestration; loading/model/readiness/planning/records indeholder de respektive kontroller.

Version 0.5.0 tilføjer [agent-workflowet](WORKFLOW.md) som separat værktøjslag. Runtime-valideringen eksekverer stadig ingen applikationstests; det afgrænsede [tilbudseksempel](examples/agent-native/README.md) har faktiske, eksternt udførte checks og eksplicit hostverificeret evidens. [Workflow-reviewet](evidence/workflow-review.md) dokumenterer denne version.
