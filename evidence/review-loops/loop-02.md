# Loop 2 — rettelser og frisk verifikation

Status: verificeret. Fire parallelle read-only reviews og leadens krydskontrol fandt 11 nye fundgrupper, R020–R030 i [loops.json](loops.json). Alle er rettet med regressioner; ingen bekræftede fund er udskudt.

Autorisation beregner nu kompatibel, betroet supersession før requestens scope bruges til at vælge dækkende approvals. En smal godkendelse kan dermed ophæve et smalt afslag under et bredere standing-mandat, uden selv at autorisere bredere arbejde. Samme kompatibilitetsregel beskytter readiness-reviews og spørgsmålsbeslutninger mod urelaterede erstatninger. Ældre/ubetroede, forkert scoped eller blot foreslåede receipts kan ikke ophæve aktuelle beslutninger.

Mappeejerskab med terminal `.`/`..` normaliseres med undertræet; versioner bruger ASCII-cifre på tværs af schemaengines. Den native atomiske rename afviser NUL før platformskaldet. CLI-forslags source-URI beregnes fra manifestets og outputmappens faktiske placering, også gennem symlinks. Proposal-API'et kræver finite JSON-værdier og Unicode-scalars, som faktisk kan materialiseres i UTF-8.

Designreferencer kræver `purpose: contract` i begge modelschemas og løses altid som kontrakter ved readiness. Aktiv lokal `$ref`/`$dynamicRef` kan ikke længere skjule et andet schemadialect, remote refs, hooks eller ugyldige subschemas i ellers inerte felter. Ubrugte const-/annotationdata forbliver data. Dybde, som den anvendte schemaengine ikke kan afgøre, giver UNKNOWN; positive/negative understøttede schemas bevarer deres normale resultat. Meget store decimalværdier i line-range sammenlignes uden en begrænset heltalskonvertering.

Negative reproduktioner: [leadens første prober](loop-02-lead-before.json), [CLI-locator](loop-02-cli-locator-before.txt), [Graphify/schema før rettelser](loop-02-integration-before.txt) og [model/readiness før rettelser](loop-02-model-before.txt). De er bevidste fejloutputs, ikke slutstatus. Den første lead-testlog indeholdt desuden en forkert proposal-nøgle i locator-testen; den korrigerede negative locator-test er gemt særskilt. Under samlet browserintegration blev den eksisterende design-ref-fixture også rettet til contract-purpose, ligesom pytest-fixturen.

Integreret resultat: **456 pytest-tests består**, **58/58** standardcases, **18/18** Graphify-cases, **16/16** renderer-cases og **20/20** offline browserchecks matcher. Browserkørslen har ingen script-/konsolfejl eller eksterne requests. Se [tests](loop-02-tests.txt) og de øvrige konkrete outputs i ledgeren. Dette er frameworkkontrol med syntetiske receipts og evidence, ingen attestering af faktisk applikationsadfærd.

En frisk loop 3 gennemgår hele frameworket efter rettelserne. Model- og protokolreviewerne bytter område for at give ændringerne et andet perspektiv. Lead skriver alle checkoutændringer; workers har alene forberedt afgrænsede patches og prober i tempkopier.
