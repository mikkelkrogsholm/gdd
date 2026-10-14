# Loop 6 — entydig identitet, tal og integrationsplaner

Status: verificeret efter rettelser af R050–R052. Lead integrerede alle ændringer; workers arbejdede kun i tempkopier.

R050: Model-ID’er er globalt unikke, men set-baseret medlemskab mistede multiplicitet. Et current check/evidence/binding kunne bruge et rule-ID, som også identificerede en source, og stadig give implementation-PASS. En tilsvarende tvetydig node i requestscope kunne give authorization-GRANTED. Brugte referencer skal resolve entydigt. Rettelsen afgrænses til konkret brug og bevarer uafhængige negative resultater og gyldige disjunkte scopes.

R051: Decimaler som minimum=1e-4000/maximum=0 eller minimum=0.10000000000000001/maximum=0.1 mister den deklarerede modstrid ved float-konvertering. Input uden tabsfri decimal round-trip skal være synligt unsupported/UNKNOWN, frem for en positiv vurdering af ændret betydning. Almindelige repræsenterbare værdier og reelle supported modstridstests bevares.

R052: Integrationsplanen kontrollerede kun, at regel-ID’er var dækket, så en blocking rule med to krævede verification-metoder kunne nøjes med én. Dækning skal gælde (regel, metode)-par og aktuelle gyldige checkreferencer. Planen er fortsat en deklaration; udført implementationsevidens kræves ikke til denne kontrol.

Friske read-only reviews: viewerens 35 uafhængige browserkontroller og 13 API-/schemakontroller bestod, ligesom 23/23 browser-, 16/16 renderer- og 41 regressionstests. Graphify/CLI/filesystem bestod 199 målrettede regressioner, adversarial Unicode-/UTF-8-raw-recovery, konkurrerende eksklusiv publicering, historiske bundles og Unicode-proposalpaths. Recordsområdet bestod 120 nye kontroller og 219 eksisterende regressioner, ud over R050-reproduktionen. Importsmodellen blev sammenholdt med et uafhængigt Warshall-orakel på alle 512 rettede grafer med tre vertices, direkte/transitivt: 9.216 forbid-checks matchede.

Integreret resultat: **940 pytest-tests**, **58/58** standard-, **18/18** adapter-, **16/16** renderer-cases og **23/23** offline browserchecks består. Ingen browserfejl eller eksterne requests. R050 tilføjede 64 regressioner (41 fejlede på baseline; 17 bestod; seks nye helpertests blev fravalgt før helperen fandtes). R051/R052 tilføjede 99 regressioner (52 fejlede før rettelsen, 47 bestod). Baselineoutputs er bevidst negative reproduktioner; loop-06-tests.txt er det aktuelle samlede resultat. En frisk loop 7 starter efter sidste rettelse.
