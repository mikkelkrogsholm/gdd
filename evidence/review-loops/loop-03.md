# Loop 3 — andet perspektiv og konkrete grænsetilfælde

Status: verificeret efter rettelser. De to reviewere for model og protokol byttede område. Graphify og viewer blev gennemgået igen, og lead undersøgte CLI, filstier, numerisk kapacitet og pakning. Ti fundgrupper, R031–R040, er registreret i [loops.json](loops.json).

Viewerens tablet-række kunne være lavere end sine paneler. Ved 740×360 og 850×600 dækkede inspector graf/filtre/listen. En indholdsbaseret minimumshøjde og en afgrænset objektliste retter overlappet; browsercasen kontrollerer fem størrelser samt faktiske scenarie-, foldnings- og resetklik. Rendereridentiteten er ændret til 0.1.2, og eksempelfilen er genereret igen.

Schema-gyldige modelassets med NUL i stien giver nu en rapportdiagnose før filesystemkald. En trusted readiness_review med projekt-scope kan dække en enkelt skærm. Standardens hele-projekt-readiness medtager custom-noder, hvis ukendte parathedssemantik tidligere kunne blive udeladt. Model-ID-kollisioner i det valgte udsnit bliver ikke længere fejlagtigt behandlet som uvedkommende recordfejl.

Revisionsmæssigt uforenelige verified bindings kan ikke ophæve en aktuel mapping. Aktuel evidence-scope skal referere til eksisterende model-ID'er; en ugyldig receipt giver model-FAIL og UNKNOWN for det berørte check. Historiske/null revisioner reboundes ikke. Uafhængig valid evidens bevarer sit resultat, og ekstra evidensscope beviser ikke ekstra verificering.

Graphify-inputs ved runtimegrænser kunne få PASS fra observation og derefter crashe under serialisering eller blive meldt som korrupt manifest ved genindlæsning. Parserdybde og finite JSON-tal uden for runtimekapaciteten blev også fejlagtigt meldt som ugyldig JSON. Disse begrænsninger håndteres som UNKNOWN med rå bytes til gendannelse. Et eksplicit raw-profile må desuden bruges på et raw-artefakt, selv om filen hedder observations.json.

Triage, uden bekræftet defect: Logiske skriveområder og deklareret workspace-isolation er ikke en fysisk checkout-/symlink-/worktreeaudit; værktøjet har ingen verificeret code-root. Tomme modeller er gyldige og har ingen implementationstargets at afvise. Et eksplicit host-denial kan nedlægge veto. Approval-expiry udenfor standardens exceptionkontrakt er ikke indført som ny runtimefunktion. En ugyldig direkte API-path kan give den dokumenterede ValueError; modelindhold skal derimod diagnosticeres i rapporten.

Negative reproduktioner er bevaret særskilt: [assetpaths](loop-03-asset-before.txt), [bindings/evidence](loop-03-records-before.txt), [tablet-layout](loop-03-landscape-before.json), [raw-filnavn](loop-03-graphify-locator-before.json) og [numerisk runtime](loop-03-numeric-runtime-before.json). De er ikke slutstatus. En patch-backup fra loop 1 er flyttet ud af det aktive schema-directory; wheelkontrollen afviser .orig/.rej-filer.

Integreret resultat: **542 pytest-tests består**; **58/58** standard-, **18/18** Graphify- og **16/16** renderer-cases matcher. **21/21** browserchecks består uden script-/konsolfejl eller netværk. Alle ti fundgrupper er rettet med regressioner. Loop 4 er startet som frisk review af den samlede løsning efter disse ændringer.
