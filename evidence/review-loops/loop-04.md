# Loop 4 — revisionsintegritet og native inputgrænser

Status: verificeret efter rettelser. Reviewerne skiftede igen områder; Graphify blev gennemgået af den tidligere viewerreviewer og viewer af den tidligere integrationsreviewer. Alle frameworkområder blev gennemgået med almindeligt review og adversarial prober. Seks fundgrupper, R041–R046, er registreret i [loops.json](loops.json).

R041: En `%TAG !! ...`-direktiv kunne få parserens tokenkontrol til at tro, at et brugerdefineret resolved tag var built-in. YAML-kontrol skal bruge det faktisk resolved tag på den komponerede node, stadig med JSON-scalars og forbud mod anchors/aliases/custom tags.

R042: Et lokalt integer-interval uden heltal behøver ikke gøre en samlet object-/arraytype ubeboelig. En object-property kan være valgfri, et array kan være tomt, eller en nullable parent kan være null. Den semantiske kontrol skal derfor bruge den kontekst, hvor en konkret ikke-null værdi er påkrævet. Strukturelle constraints og referencer kontrolleres fortsat overalt.

R043: Korrekt global model-FAIL var ikke nok til at holde ugyldige brugte records ude af positive dimensioner. Reproduktionerne dækkede self/cyclic supersession, der ophævede et aktuelt afslag; binding-targets med ukendt current ID; model-ID-kollision på brugt check/evidence/binding/approval; samt ukendt ekstra approvals-scope. Rettelsen afgrænses til de records, som faktisk bruges. En uvedkommende modeldefekt må ikke automatisk tilsidesætte et gyldigt hostmandat eller uafhængig valid evidens. Malformed retirement kan ikke ophæve aktuelle negative receipts.

R044: Viewer-API'et accepterede en streng som scope og opdelte den i bogstaver. Med gyldige ID'er a, b og ab gav `scope="ab"` PASS for a/b og udelod ab. Scope skal være list[str] eller None; forkerte typer afvises med ValueError før output oprettes. Negative og positive kontroller findes i [viewer før rettelse](loop-04-viewer-before.txt) og test_review_viewer_loop4.py.

R045/R046: Desktop-panelets minimum kunne være for lavt ved 851×600, så reset løb ud over panelet og footer. En gyldig projekttitel på 500 tegn kunne også løbe ud over den faste headerhøjde. Begge blev reproduceret i installeret Chrome 155 og bundled Chromium 151. Normal desktop/tablet/mobile, filtre, ID-partitioner, retning, exact text, CSP og modeldata blev ellers genkontrolleret uden nye konkrete viewerfund.

Graphify-reviewet var rent: 184 eksisterende adapter-/CLI-regressioner bestod. Friske uafhængige prober dækkede numeric/string identity, dublerede producer-edge-ID'er, vendt retning, legacyversioner, tamper, dybde 256/257, numerisk overflow, proposalclaims, outputeksklusivitet og relativ locator gennem stier med mellemrum.

R043-kontrollen er samlet i record_integrity.py og genbrugt af readiness, autorisation og implementation. Seks ekstra regressionscases beskytter brugte readiness-review- og spørgsmålsreceipts mod ukendt scope eller ugyldige checkreferencer. Uvedkommende recordfejl er fortsat separate.

Integreret resultat: **691 pytest-tests**, **58/58** standard-, **18/18** adapter-, **16/16** renderer-cases og **23/23** offline browserchecks består. Ingen script-/konsolfejl eller eksterne requests. Alle seks fundgrupper er rettet. Renderer er 0.1.3; demo er regenereret. En frisk loop 5 er startet.
