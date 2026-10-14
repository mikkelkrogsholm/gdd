# Designbeslutninger for 0.2-udkastet

Informativt beslutningsgrundlag. Scratchpadens produktretning er fastholdt; følgende er konkrete tekniske valg inden for den retning.

1. **Ny formatversion.** Den gamle 0.1 blandede registries og indlejrede relationer. 0.2 gør alle otte modelbegreber til noder og giver relationer egne ID'er. Ændringen er synlig og migreret; 0.1-teksten er arkiveret.
2. **Én autoritativ relation.** Ejerskab og dækningsrelationer forfattes som edges. Der vedligeholdes ikke samtidig owner-/obeys-/members-felter, som kan divergere.
3. **Filer med lokale fragmenter.** JSON/YAML med sikre indlæsningsregler giver en enkel agentindgang og selektiv læsning. En grafdatabase er ikke nødvendig for standarden. JSON Schema er den maskinlæsbare repræsentationskontrakt; grafsemantik kræver yderligere checks.
4. **Separate operational records.** Receipts, bindings, checkplaner og evidens kan pege på en stabil modelrevision uden selvrefererende hashing. Records er uforanderlige; nye records kan supersede tidligere records.
5. **Fire forskellige vurderinger.** Modelform, strukturel parathed, autorisation og implementeringsevidens har forskellige resultater. Skitser er tilladte, mens manglende kontrakter kan blokere et valgt udviklingsudsnit.
6. **Afgrænset, deklarativ udvidelse.** Namespacede typer og relationer har dataschemas og eksplicit traverserings-/effektbetydning. Ukendt semantik giver UNKNOWN; vilkårlig kode indlæses ikke som extension.
7. **Revisionsbundne kontraktassets.** Faktisk anvendt DESIGN.md og andre kontraktkilder indgår i snapshot. Kildesyntese bevarer de inputrevisioner, som faktisk blev brugt.
8. **Grafen organiserer arbejde.** Communities er gruppering; undergrafer er dekomposition. Parallelitet vurderes gennem afklarede grænser, effekter, skriveansvar og integrationschecks, ikke blot adskilte clusters.
9. **Ingen implementeret motor i denne fase.** Standard, schemas og conformance-cases er leverancen. Validator, importadapters, visualisering, distribution og demonstrationsapp er efterfølgende implementationsarbejde.

Valgene er reviewbare draftbeslutninger. Der er ikke dokumenteret en produktivitetsgevinst, agentuniversalitet i praksis eller en unik opfindelse. Et senere værktøj skal demonstrere conformance og metodens nytte.
