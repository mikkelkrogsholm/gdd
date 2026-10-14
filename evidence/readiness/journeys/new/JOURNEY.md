# Independent installed GDD journey

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
`24f2c5dbf220cb1c4efae9aca9b84ff25c129c6b382b03e5929065496bb6766c`.
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
| A model | `sha256:aed571324f874297017497789d7c763aba9a7b3d73bfc06fa9667832b56ec9c4` |
| A accepted code | `sha256:69823548dad0e8e64e453c222fd526d4e6daaf03a598c86d7382d8e929506be7` |
| B model | `sha256:ef2bbd384bb74078fbda12442351c7ba74cf8c140ea150d39bf7f2ed24b7dc16` |
| A source with fixed B test for red | `sha256:55a41aeea12f79fffe9fe67d3d113ba999b0f2ddc4fedc73ff86d5a97cf7e3c0` |
| B accepted current code | `sha256:b0e8527e09cc538203aaa7c8c9c977ac5de6205d4bb0e1de188368b247e89970` |

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
