# GDD agent-workflow 0.1

Workflowet bruger GDD 0.2 til at forberede kodearbejde og holde styr på dets
integration. Det kan bruges af eksisterende agenter hos enhver leverandør. CLI
og Python-API starter ingen LLM og udfører ingen kommandoer fra grafen.

En lead-agent vælger et afgrænset mål, implementation, kodebase, pakker og
filområder. Værktøjet afleder kontekst, kontrakter, pseudokode, regler,
scenarier, source-lineage, designreferencer, extensionsemantik og nødvendige
checkplaner. Det kontrollerer readiness for hver pakke og parallelitet separat.
En community er en gruppering; den er ikke bevis for uafhængighed.

## Kom i gang

```sh
gdd workflow plan model.gdd.yaml --request request.json --output plan.json
gdd workflow verify model.gdd.yaml --plan plan.json
gdd workflow create model.gdd.yaml --plan plan.json --run run
gdd workflow status model.gdd.yaml --run run
gdd workflow event model.gdd.yaml --run run --event dispatch.json \
  --host-context host.json --output dispatch-result.json
```

`plan --output` gemmer selve planen og viser rapporten/planen på stdout. De
andre kommandoer gemmer deres samlede JSON-resultat. Eksisterende destinationer
overskrives aldrig. `create` opretter en ny run-mappe atomisk; den accepterer en
uparat draft, men afviser en ændret eller historisk pakke. `status` er læsning.

En request følger [workflow.schema.json](schemas/workflow.schema.json):

```json
{
  "format": "gdd-work-request/0.1",
  "id": "work.feature",
  "objective": "Implementér den valgte funktion og dens tests",
  "implementation": "implementation.app",
  "base_code_revision": "host-verified-code-revision",
  "packages": [{
    "id": "package.feature",
    "scope": ["action.feature"],
    "write_resources": ["src/feature.py", "tests/test_feature.py"],
    "read_resources": ["src/contracts.py"],
    "workspace": "../workspaces/feature",
    "deliverables": ["Implementeret funktion og regressioner"],
    "integration_plan": "Review patch og kør fælles checks"
  }],
  "serialized": [],
  "integration": {"owner": "actor.lead", "checks": ["check.feature"]}
}
```

Filnavne og runtime vælges af agenten efter repositoryet. De afledes ikke af et
sprogneutralt action-ID. `scope` er implementeringsmandatet;
`implementation_targets` inkluderer dets refinement. `context.nodes` er det
større læsegrundlag og må ikke blive ekstra skriveejerskab. Pakken indeholder
fulde records og locators samt det oprindelige modelsnapshot. Lokale normative
assets læses fra modellen og kontrolleres mod snapshot; de eksekveres aldrig.

`serialized: [["package.a", "package.b"]]` betyder, at A skal være integreret,
før B udstedes. `calls` og `depends_on` beskriver produktkontrakter; de opfinder
ikke automatisk denne arbejdsrækkefølge. Uafhængige pakker kan udstedes fra samme
kodebase. Hosten etablerer faktisk isolation, tilladelser og passende read-only
adgang. Shared-checkout-underagenter leverer temp-patches; samtidige kodewriters
skal have isolerede worktrees. Logiske workspace-stier beviser ingen isolation.

## Hændelser og fremdrift

Alle events kræver `expected_revision` fra seneste status og en unik
`idempotency_key`. Eksempel på udstedelse:

```json
{
  "type": "dispatch",
  "expected_revision": "sha256:...",
  "idempotency_key": "dispatch.feature.1",
  "package": "package.feature",
  "attempt": "attempt.feature.1"
}
```

| Event | Forudsætning og effekt |
| --- | --- |
| `dispatch` | Aktuel pakke/readiness, implementeringsautorisation og hostbekræftet implementation/kodebase. Returnerer pakken; hosten transporterer den til agenten. |
| `submit` | Samme udstedte attempt. Gemmer `summary`, `artifact_uri`, `changed_resources` som workerpåstande. |
| `integrate` | Aktuelt submit, aktuel autorisation, review og hostbekræftet kode-/filresultat. Frigiver efterfølgere. |
| `retry` | Kun ufærdige attempts. Næste dispatch kræver et aldrig tidligere brugt attempt-token. |
| `cancel` | Stopper nye udviklingshandlinger. Bevarer kode, artifacts og sene submissions. |
| `resume` | Eksplicit genoptagelse med aktuelle gates og hostbekræftet kodebase. Genstarter ingen agent. |
| `accept` | Alle pakker integreret, aktuel implementation-evidens PASS, afklarede acceptance-spørgsmål og acceptautorisation GRANTED. Terminalt. |

Submit indeholder et artifact-URI og faktiske ændrede filnavne. Det kan hverken
indeholde host-trustfelter eller give implementation-PASS. Integrate skal have
`package`, `attempt`, `base_code_revision`, `code_revision`, `verified_resources`
og `review: {reviewer, verdict: "PASS", evidence_uri}`. Hosten bekræfter selv
patch, ændrede filer og review. Fil-listen skal svare til submission og være
inden for skriveområdet. Hosten leverer den resulterende `code_revision` i sin
separate host-context. Den kan ikke bevises ved en workers eget hashclaim.

To workers kan begynde ved C0. Efter første integration mod C0→C1 skal anden
integration ske mod C1→C2, selv om workerens oprindelige base var C0.

Run-mappen indeholder `run.json` og en hashkæde af eksklusivt publicerede
`events/000001.json` osv. En samtidig head-konflikt kan kun publicere én event.
Samme key og identiske request genfinder den oprindelige receipt før head-check;
den returnerer aktuelle gates og **ingen ny dispatchpakke**. En key med ændret
request afvises. `status` viser udstedte attempts, så lead kan afstemme faktisk
agentlevering efter en afbrydelse. Den starter aldrig automatisk en erstatning.

Logs er lokale auditclaims med tamper-detektion, ikke kryptografisk autentificeret
historik. Filsystemet og eventkald beskyttes af hosten. Selv en historisk
`accepted`-status vurderes igen: `acceptance_current` kræver aktuelle gates og
eksplicit host-context. Modellen, recordfiler og workerresultater er ikke kilder
til hostautoritet.

## Checks og ændringer

Checkplanerne skal dække hver blocking regel × verification-metode præcis én
gang. V1 understøtter denne GDD-semantik: ekstra checks uden required-pair-dækning
eller flere valg for samme par gør planen UNKNOWN. Accept låser evaluatorens
valg til planens check-ID'er; andre hostvalg afvises. Hosten udfører de faktiske
checks og registrerer eksisterende GDD bindings/evidence med præcis
check-, model-, implementation- og koderevision. Worker-tests fra C0 giver ikke
integrationsbevis for C2. Se [validatorens hostkontrakt](VALIDATOR.md).

Model-/assetændringer eller ændrede valgte checkplaner kræver en ny plan og et
nyt run. Historikken bevares. V1 har ingen automatisk rebasing, merge, modelredigering
eller invalidering af allerede integrerede jobs. Record-only historikdiagnoser
vises fortsat; aktuelle scoped gates vurderes separat og bruger ingen historisk
approval/evidence som aktuel. Manglende faktisk evidens giver UNKNOWN.

API: `prepare_workflow`, `verify_workflow`, `create_run`, `run_status` og
`advance_run` fra `gdd_validator`. Planbygning returnerer `{report, artifact}`;
events returnerer `applied`, `replayed`, rapport og ved commit en receipt. Afviste
gates publicerer ingen event. Ugyldige envelopes/head-konflikter giver
`ValueError`; CLI viser en kontrolleret fejl. Rapportkoder følger eksisterende
0=PASS/GRANTED, 1=FAIL/DENIED, 2=UNKNOWN/UNRESOLVED. En committed eller idempotent genfundet event returnerer 0; øvrige diagnoser bevares i rapporten. Se altid `applied`/`replayed` ved events.

## Agentinstruktioner og faktisk eksempel

Brug [lead-guiden](agents/gdd-lead.md) og [worker-guiden](agents/gdd-worker.md)
direkte i en eksisterende agentruntime. De indeholder intet leverandørspecifikt
API. [Tilbudseksemplet](examples/agent-native/README.md) viser parallelle native
agentrettelser fra half-up til grafens floor-kontrakt, faktisk rød/grøn testkørsel,
integration og revisionsbundet accept. Python er kun eksempelimplementationen.

Kør workflowets 20 navngivne conformance-forventninger med `uv run python conformance/run_workflow.py --output workflow-conformance.json`. Harnesset bruger dev-afhængigheden pytest; runtimepakken tilføjer ingen test- eller agentafhængighed.
