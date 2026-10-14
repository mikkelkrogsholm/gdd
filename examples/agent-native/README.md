# Fra GDD-graf til faktisk kode

Grafen kræver floor-afrunding med heltalsenheder og rabat før skat. Den gamle
kode i `baseline/` bruger half-up. To native agenter læste de faktisk udstedte
GDD-pakker og rettede hver sit modul i adskilte tempkopier. Lead reviewede og
integrerede dem. Derefter læste en agent quote-pakken og tilføjede en manglende
CLI-regression for store heltal. Shared checkout blev kun ændret af lead.

Faktisk resultat: **18 baseline-tests med 7 afrundingsfejl → 19 beståede tests**.
De oprindelige forventninger er bevaret. For `101, 5000, 5000` giver den faktiske
CLI nu rabat 50, netto 51, skat 25 og total 76. Modellen er sprogneutral; Python
er alene referenceimplementationen.

```sh
# Fra denne mappe; applikation og tests bruger kun standardbiblioteket.
python -B -m unittest discover -s tests -v
python -B -m quote_demo 101 5000 5000

# Fra repositoryets rod med gdd-validator installeret.
uv run python examples/agent-native/replay.py --output /tmp/min-gdd-demo
gdd workflow status examples/agent-native/model.gdd.json \
  --run examples/agent-native/native-run \
  --host-context examples/agent-native/host.acceptance.json
```

Replay-scriptet opretter et nyt run i en ny mappe og genbruger de arkiverede
native-agentrettelser. Det udfører faktisk baseline, modulchecks, samlet suite,
bindings/evidence og accept på ny. Det starter ingen ny LLM. Commands er valgt i
hostscriptet; ingen kommando eksekveres fra grafens tekst eller checklabels.

| Artifact | Betydning |
| --- | --- |
| [model.gdd.json](model.gdd.json), [REQUIREMENTS.md](REQUIREMENTS.md) | Accepterede floor-kontrakter, pseudokode, regler og scenarier. |
| [workflow.request.json](workflow.request.json), [workflow.plan.json](workflow.plan.json) | Tre eksplicitte arbejdspakker; discount/tax uafhængige, quote efter begge. |
| `native-dispatches/`, `worker-results/` | De faktisk udstedte pakker, native workerpatches og rå resultater. |
| `native-run/` | Ti events: tre dispatch, tre submit, tre integration, én accept. |
| [evidence/baseline.run.json](evidence/baseline.run.json), [evidence/final.run.json](evidence/final.run.json) | Faktisk rød/grøn-kørsel inklusive den rigtige CLI. |
| [evidence/status.json](evidence/status.json), [evidence/summary.json](evidence/summary.json) | Aktuel revisionsbundet accept og sammenfatning. |
| [model.records.plan.json](model.records.plan.json), [model.records.json](model.records.json) | Planlagte records samt nye, faktisk hostverificerede testbindings og evidence. |

`code.snapshot.json` identificerer baseline; `code.final.snapshot.json` det
integrerede resultat. Den lokale `demo-code-snapshot/0.1`-konvention hasher
kanonisk JSON af et sorteret filnavn→SHA-256-map for alle leverede Python-kode-
og testfiler. Den er ikke en Git-commit eller en ny GDD core-hashregel.
`model.snapshot.json` identificerer de separate normative model-/sourcebytes.

De medfølgende hostfiler er eksplicitte inputs til denne afgrænsede lokale demo.
Hosten vælger at stole på dem efter faktisk verifikation; en vilkårlig fil med
samme felter giver ingen autoritet. `acceptance_current` er kun true, når den
aktuelle host-context understøtter præcis dette resultat. Workerens egne PASS
blev ikke ophøjet til integrationsevidens. Workspaces i requesten er logiske
deklarationer; de faktiske native workers arbejdede i separate tempkopier.

Se [workflowkontrakten](../../WORKFLOW.md) og de leverandøruafhængige
[lead](../../agents/gdd-lead.md)/[worker](../../agents/gdd-worker.md)-guides.
