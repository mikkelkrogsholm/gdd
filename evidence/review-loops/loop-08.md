# Loop 8 — parserkapacitet

Status: verificeret efter rettelse af R055.

R055: Begge parsergrænser grupperede RecursionError sammen med kendte ugyldige JSON/YAML-features. Et gyldigt dokument med 2.000 nested arrays i fri annotationsdata gav derfor YAML_FEATURE_FORBIDDEN/model-FAIL, selv om årsagen alene er runtimeparserens kapacitet. Dokumentationens dybdegrænse og konservative resultatmodel kræver FORMAT_UNSUPPORTED/UNKNOWN. Rettelsen ændrer klassifikationen; ingen parserudvidelse eller ny vilkårlig dybdegrænse.

Modelområdet bestod 539 regressioner og 162 uafhængige Decimal-orakelcases for bounds og integerinhabitation, plus lokal ref-/equality-/non-mutation-kontrol. Graphify/CLI/filesystemområdet bestod 383 målrettede regressioner og friske integritets-, identitets-, byte-recovery-, importaudit- og eksklusive outputkontroller uden nye fund. R053’s oprindelige fem modelrepros blev genafprøvet med de korrekte matematiske resultater. Leadens førkontrol af gyldig JSON/YAML på dybde 10 og 2.000 er gemt separat.

Records/protokolområdet var rent: 248 friske assertions (inklusive 144-case authority-orakel) og 313 regressioner bestod. Viewerområdet var rent: 41 pytest, 23 browserchecks, 35 uafhængige browserkontroller, 13 API-/schemakontroller, 16 numeriske grænsekontroller og 16 renderer-cases bestod. Fire nye numeriske fidelity-cases samt faktisk Chrome-kontrol af inspector/søgning bekræftede bevaring af lagrede typer, kanonisk model, sourcebytes og HTML-værdier. Ingen browserfejl eller netværk.

R055 er rettet med separate RecursionError-grænser i JSON- og YAML-parseren. De 34 nye regressioner havde 17 FAIL/17 PASS før rettelsen og består nu; forbidden-feature- og syntaksfejl forbliver FAIL. Leadens samme positive/negative inputmatrix giver nu PASS/exit0 ved dybde 10 og UNKNOWN/exit2 ved dybde 2.000.

Integreret resultat: **1.106 pytest-tests**, **58/58** standard-, **18/18** adapter-, **16/16** renderer-cases og **23/23** offline browserchecks består. Ingen browserfejl eller eksterne requests. En frisk loop 9 starter efter sidste rettelse.
