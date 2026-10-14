from journey_tools import ROOT,write,run
from gdd_validator.host import snapshot_code
import json,hashlib,sys,shutil

def read(path):return json.loads((ROOT/path).read_text())
a_spec=read('evidence/a-snapshot.json')['revision'];b_spec=read('evidence/b-snapshot.json')['revision']
a_code=read('evidence/a-code.json')['revision'];b_code=read('evidence/b-code.json')['revision'];red_code=read('evidence/b-red-code.json')['revision']
assert snapshot_code(ROOT,list(read('evidence/b-code.json')['files']))['revision']==b_code
for stage,snapshot in [('a','a-code'),('b-red','b-red-code')]:
    for path,sha in read('evidence/'+snapshot+'.json')['files'].items():
        assert hashlib.sha256((ROOT/'history'/stage/path).read_bytes()).hexdigest()==sha
assert read('evidence/b-status-final.json')['acceptance_current'] is True
for zone in ['domestic','remote']:
    run('final-cli-'+zone,[sys.executable,'-B','-c',f"import sys;sys.path.insert(0,'src');from parcel.cli import main;raise SystemExit(main(['1001','{zone}']))"])
write('README.md','''# Parcel price CLI

Use Python 3.11 or newer with no application dependencies:

```sh
PYTHONPATH=src python3 -B -m parcel.cli 1001 domestic
# 200
PYTHONPATH=src python3 -B -m parcel.cli 1001 remote
# 350
python3 -B tests/test_b.py src
python3 -B tests/check_architecture.py src
```

The current contract B adds one 50-cent remote handling fee. Amounts are integer
cents. Weight must be a positive integer in grams; invalid input exits 2.
The rate table remains 100 cents domestic and 150 cents remote per begun kg.

`tests/test_b.py` is the current fixed acceptance suite. `tests/test_a.py` is the
preserved historical A contract and is intentionally not selected for B. Run
it against `history/a/src` to reproduce A. Fixed B tests against
`history/b-red/src` reproduce the old-code defect. The explicit commands above
are the supported checks; generic test discovery is not configured.

See [JOURNEY.md](JOURNEY.md) for model, workflow, revisions, trust boundaries,
raw check receipts, negative controls and limitations. All artifacts are flat
files. The offline graph is [evidence/b-graph.html](evidence/b-graph.html).
''')
write('JOURNEY.md',f'''# Independent installed GDD journey

## Result

I completed the new-project path using only the installed GDD CLI/skill and my
own isolated sources and stdlib checkers. Contract A was technically accepted
with actual current evidence. Contract B was separately modeled, planned,
implemented, verified and accepted. The final B status reports
`acceptance_current: true`; the implementation and authorization dimensions
are independently PASS and GRANTED. No guide/API blocker prevented completion.
The skill does not install a CLI; this test used the separately supplied venv.

All work is under this isolated project. I did not read the parent checkout,
use its example tooling, consult parent implementation answers, or delegate.
I read the required global developer-team entry to respect the user's routing,
then handled the bounded task directly. The installed GDD skill's new-project
and change routes led to GDD.md, FORMAT, VALIDATION, VALIDATOR, WORKFLOW, the lead
guide, DESIGN-PRINCIPLES, AGENT-PROTOCOL, VIEWER, schemas, catalog and HOST.md.

## Inputs and model

`gdd-a/sources/brief.md` preserves the relevant task input. The brief is input,
not execution evidence or a self-granted mandate. Local contract files are
normative accepted intent, hashed from real bytes. English CLI syntax/errors
and explicit bool rejection are documented agent-resolved details within the
brief. There are no unresolved material questions.

`gdd init` produced a valid unready 0.3 scaffold, including records in the 0.2
envelope. I replaced the placeholders with three components and public action
contracts: CLI → quote → rate. Each component states responsibility, rationale
and exclusions. Actions specify inputs, outputs, error cases, postconditions,
effects and all five risk assessments. The graph includes normal, weight-boundary,
invalid-input and architecture scenarios. A precise blocking architecture rule
forbids direct CLI → tariffs imports; the other blocking rule requires the real
price/output behavior. Declared graph imports remain intended relationships.

Both manifests pass model validation and scoped structural readiness for
action.cli, action.quote and action.rate. Their contexts, boundary views,
byte snapshots and standalone HTML graphs are preserved. The renderer's exact
artifact verification passes. No browser-interaction test was claimed.

## Actual implementation and workflow

I used the public installed `gdd_validator.host` helpers documented in HOST.md:
snapshot_code, execute_check, verification_records and append_records. The
initial A code base was explicitly empty. A single package owned the three
application modules, package initializer and concrete tests. The task itself
provided the trusted bounded authority; separate host JSON attested it. No
source file or journal receipt was used to invent authority.

For A and B I prepared and verified a workplan, created a separate run, read
status, and advanced dispatch → submit → integrate → accept with fresh expected
revisions and unique event keys. I did the actual work directly. Dispatch
started no worker, and submit applied no patch. I inspected and integrated the
actual resources before registering the integration receipt. The package file
list matches the actual resulting patch. B changes only pricing.py and adds the
fixed B test file; CLI and tariffs retain their A bytes.

Current check/evidence records bind exact check hashes, model/code revisions,
real resources, checker versions, environment and raw execution receipts.
Host trust lists and explicit selections were merged from actually inspected
checks. Code bindings were independently checked against real selected-file
hashes. Four-principle performed reviews and their advisory envelope checks
are preserved; COMPLETE does not stand in for blocking acceptance.

## Red, green, architecture and unknown results

| Actual check | Outcome | Artifact |
| --- | --- | --- |
| A behavior, 6 tests including boundary subcases | PASS | evidence/a-behavior-current.run.json |
| A deliberately incorrect domestic rate | FAIL | evidence/a-behavior-bad-rate.run.json |
| Required check plans with no execution evidence | UNKNOWN | evidence/a-records-missing-evidence.json and evidence/b-records-missing-evidence.json |
| Missing actual source for architecture checker | UNKNOWN | evidence/architecture-missing-code.run.json |
| Fixed B assertions against unchanged A source | FAIL | evidence/b-fixed-assertions-against-a.run.json |
| GDD evaluation of real old-code B evidence | behavior FAIL, missing architecture UNKNOWN | evidence/b-records-real-red.json |
| Same fixed B assertions on B, 8 tests | PASS | evidence/b-behavior-current.run.json |
| Live forbidden CLI import at module startup | FAIL | evidence/b-architecture-live-injected.run.json |
| CLI actually executed with that forbidden import | quote output 200; architecture still FAIL | evidence/b-cli-live-injected.run.json |
| Forbidden import repaired with exact bytes restored | PASS | evidence/b-architecture-live-repaired.run.json |
| Both current B check records | PASS | evidence/b-records-current.json |
| Current B acceptance authority | GRANTED | evidence/b-authorization-acceptance.json |
| Final B workflow acceptance | current true | evidence/b-status-final.json |
| Historical A acceptance with current B code | current false; implementation UNKNOWN | evidence/a-status-stale-after-b.json |

Fixed B expected remote amounts were authored before changing source: 200 cents
through 1000 grams, 350 through 2000, 500 through 3000, and 650 at 3001 grams.
Domestic assertions remain unchanged. The test hash is
`{read('evidence/b-fixed-test-hash.json')['sha256']}`.
Red and green receipts contain that same test hash; expected results were never
edited to obtain PASS. `evidence/fixed-b-red-green.json` links both executions.
Historical A and red-code bytes are preserved under history/a and history/b-red.

The concrete AST checker parses actual files, reports actual imports and requires
CLI → pricing and pricing → tariffs. Direct `from . import tariffs` is a real
forbidden source relationship. Initial controls added the import below the main
guard; that still creates a static dependency and executes when the module is
imported. I added a stronger control at module startup and ran the real CLI to
make execution unambiguous. Both controls fail the checker. Exact original bytes
are restored after every mutation, and the final combined snapshot is unchanged.
The checker deliberately returns exit 2/UNKNOWN for missing code, syntax failure,
or detected dynamic imports outside its static profile. I claim coverage only
for this fixed static-import Python package, not arbitrary import mechanisms.

## Revisions and change record

| Identity | Revision |
| --- | --- |
| A model | `{a_spec}` |
| A accepted code | `{a_code}` |
| B model | `{b_spec}` |
| A source with fixed B test for red | `{red_code}` |
| B accepted current code | `{b_code}` |

`evidence/b-model-change.json` and decision.remote-fee.b identify the A base,
changed stable graph IDs, the accepted change source, rationale and affected
contract/scenarios. New plans and runs were created after model drift. A history
remains tied to its original revisions; it is not reused as B evidence.

## Gaps and interventions

The installed guide/API was sufficient, and no acceptance requirement was
weakened. The catalog clearly says repository examples/history are optional and
not bundled. A newcomer must author a concrete model and host adapter; there is
no bundled minimal full new-project example. HOST.md substantially helps, but a
direct link from the new/change route to HOST.md would make it easier to find.
These are documentation convenience improvements, not execution blockers.

I inspected public helper signatures and the actual snapshot result to learn
that snapshot_code returns a files mapping of paths to hashes; HOST.md describes
the content but does not show the exact return shape. I corrected my own adapter
before B preparation. Two earlier authoring errors were mine: a variable/function
name collision in the model generator, and an inline Python delimiter typo.
They occurred before the relevant GDD operations and were repaired locally.
They are not API defects and no failed tool route was repeated without a fix.

Negative controls temporarily mutate source in this isolated project under the
explicit task authorization for deliberate violations, then restore exact bytes.
B's workplan records its resulting patch ownership (pricing.py and test_b.py),
but does not separately enumerate transient verification mutations of CLI. A
production handoff should declare a control-copy area in its plan. No final
resource outside B patch ownership changed, and the current acceptance is bound
to the fully repaired snapshot.

Every GDD invocation after logger setup has its argv, cwd, timestamp, exit code,
full stdout and stderr in evidence/commands/*.json and commands.jsonl. Actual
application checks additionally have genuine gdd-host-check/0.1 receipts with
before/after code snapshots and runner/timing metadata. Initial guidance/schema
inspection and the two authoring errors remain in the agent transcript rather
than fabricated local execution receipts. Orchestration scripts are flat files
and preserve the exact choices. They are local test scaffolding, not product
features or a replacement for the installed CLI.

## Run the final deliverable

The README contains the current CLI and checker commands. The offline B graph,
current records, final status, fixed red/green index and all raw receipts are in
evidence. No deployment, messaging, future scheduling or additional work was
performed. Completion is limited to the requested isolated parcel-price CLI.
''')
summary={'format':'independent-journey-summary/1','root':str(ROOT),'result':'completed','current_contract':'B','model_revision':b_spec,'code_revision':b_code,'acceptance_current':True,'blocking_checks':{'behavior.unittest':'PASS','architecture.python_ast':'PASS'},'red_fixed_b':'FAIL','architecture_negative_control':'FAIL','missing_code_or_evidence':'UNKNOWN','guide_or_api_blocker':None,'artifacts':['JOURNEY.md','README.md','gdd-a/gdd.json','gdd-b/gdd.json','evidence/b-graph.html','evidence/b-status-final.json','evidence/fixed-b-red-green.json','evidence/commands.jsonl']}
write('evidence/summary.json',summary)
print(json.dumps(summary,indent=2))
