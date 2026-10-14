# Migration fra gdd/0.1

Dette kapitel er informativt. 0.2 er en eksplicit formatrevision, ikke en transparent ændring af den gamle kontrakt.

## Bevaret grundlag

Den tidligere [SPEC](../archive/gdd-0.1/SPEC.md) og [model](../archive/gdd-0.1/concert.gdd.yaml) er bevaret byte-for-byte. Rodens `concert.gdd.yaml` er fortsat 0.1. Det nye eksempel ligger i [examples/concert.gdd.yaml](../examples/concert.gdd.yaml).

Stable IDs, oprindelig adfærdstekst, globale sikkerhedsprincipper og eksisterende scenarier bevares. Gamle linterresultater genbruges ikke som 0.2-resultater.

## Strukturel mapping

| 0.1 | 0.2 |
| --- | --- |
| nodes med component/entity/action | nodes med samme ID og kind |
| rules-registry | rule-nodes med samme ID, payload i rule |
| scenarios-registry | scenario-nodes med samme ID, payload i scenario |
| machines-registry | machine-nodes med samme ID; transitions beholder ID |
| owner-felt | eksplicit owned_by-edge med nyt edge-ID |
| stored_in-felt | eksplicit stored_in-edge |
| rule.targets | constrains-edges |
| scenario.actions/verifies | exercises-/verifies-edges |
| machine.entity | state_of-edge |
| action.specification | behavior; oprindelig tekst bevares |
| action input/context/returns/errors | abstraherede contractfelter; output er en konkret datakontrakt |
| bindings | separate planned binding-records |
| verification_policy | standardens resultater, checkplaner og projektets konfigurerbare workflowpolicy |

0.1 imports/calls-edges bevarer deres ID'er og retning. Nye core-relationer erstatter de tidligere indlejrede ejerskabs-/dækningsreferencer; de gamle og nye repræsentationer vedligeholdes ikke parallelt.

## Eksplicitte tilføjelser i eksemplet

Abstrakte typer gør principalens trusted origin og outputfelter synlige. Actions får erklærede effekter, riskvurderinger, error_cases og nye relevante scenarier. Det er specifikationspræciseringer, ikke bevis for behavior. Cancel får et selvstændigt normalforløb og et concurrency-scenarie. Den tidligere idempotente og atomare adfærd er bevaret.

Frontend/community-noder er nye skitser. Et planlagt DESIGN.md-referencefelt giver ikke et eksisterende designasset. Communities giver ikke parallelitetsbevis.

Den tidligere strenge acceptprofil kan konfigureres i et konkret projekt med required checks og gates. 0.2 pålægger ikke alle projekter en universel menneskelig godkendelsesmode. Blocking rules kan kun accepteres med PASS eller en synlig policytilladt exception; resultaterne ændres ikke.

Den medfølgende `check_spec.py` i zip-filen er en 0.1-linter og må ikke kaldes en 0.2-validator. En fremtidig migration skal validere begge formater med deres egne kontrakter og rapportere tab, uklarheder og ændrede krav.
