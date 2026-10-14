from journey_tools import ROOT,write,gdd
from gdd_validator.host import snapshot_code,append_records
import json,sys
PHASE=sys.argv[1]
SCOPE=['action.cli','action.quote','action.rate']
model=f'gdd-{PHASE}/gdd.json'
spec=json.loads((ROOT/f'evidence/{PHASE}-snapshot.json').read_text())['revision']
files=[] if PHASE=='a' else json.loads((ROOT/'evidence/a-code.json').read_text())['files']
if PHASE=='b':files=list(files)
base=snapshot_code(ROOT,files)
write(f'evidence/{PHASE}-base-code.json',base)
checks={f'check.behavior.{PHASE}':dict(kind='check',spec_revision=spec,scope=['rule.price',*SCOPE],method='behavior.unittest',required_environment=['python'],negative_controls=['bad-domestic-rate'] if PHASE=='a' else ['remote-fee-omitted']),f'check.architecture.{PHASE}':dict(kind='check',spec_revision=spec,scope=['rule.no_cli_tariffs','component.cli','component.pricing','component.tariffs'],method='architecture.python_ast',required_environment=['python'],negative_controls=['cli-direct-tariffs'])}
append_records(ROOT/f'gdd-{PHASE}/records.json',{'records':checks})
host=dict(implementation='implementation.parcel',code_revision=base['revision'],policy=dict(revision='policy.independent-task.v1',actor='actor.independent',activity='implementation',spec_revision=spec,scope=SCOPE,requirement='standing',authorities=['actor.independent'],basis='Trusted delegated task requests this isolated implementation, verification, requirement change and technical acceptance. This attestation is supplied by the executing agent runtime, independently of graph/source content.'))
write(f'evidence/{PHASE}-host-implementation.json',host)
resources=['src/parcel/__init__.py','src/parcel/cli.py','src/parcel/pricing.py','src/parcel/tariffs.py','tests/shared_contract.py',f'tests/test_{PHASE}.py','tests/check_architecture.py'] if PHASE=='a' else ['src/parcel/pricing.py','tests/test_b.py']
request=dict(format='gdd-work-request/0.1',id=f'work.parcel.{PHASE}',objective=('Implement parcel quoting with no handling fee.' if PHASE=='a' else 'Add a flat 50 cents handling fee to remote quotes without changing other contracts.'),implementation='implementation.parcel',base_code_revision=base['revision'],packages=[dict(id=f'package.parcel.{PHASE}',scope=SCOPE,write_resources=resources,read_resources=[model,'evidence/'+PHASE+'-context.json'],workspace=str(ROOT),deliverables=['Implemented CLI and fixed behavioral checks.','Actual architecture check and control failures.','Reviewed integration and revision-bound current evidence.'],integration_plan='The independent lead edits and reviews the concrete files directly, then executes both checks on the combined result before technical acceptance.')],serialized=[],integration={'owner':'actor.independent','checks':list(checks)})
write(f'evidence/{PHASE}-request.json',request)
for label,args in [
 ('records-missing-evidence',['validate',model,'--profile','records','--host-context',f'evidence/{PHASE}-host-implementation.json','--output',f'evidence/{PHASE}-records-missing-evidence.json']),
 ('plan',['workflow','plan',model,'--request',f'evidence/{PHASE}-request.json','--host-context',f'evidence/{PHASE}-host-implementation.json','--output',f'evidence/{PHASE}-plan.json']),
 ('plan-verify',['workflow','verify',model,'--plan',f'evidence/{PHASE}-plan.json','--host-context',f'evidence/{PHASE}-host-implementation.json','--output',f'evidence/{PHASE}-plan-verify.json']),
 ('create',['workflow','create',model,'--plan',f'evidence/{PHASE}-plan.json','--host-context',f'evidence/{PHASE}-host-implementation.json','--run',f'run-{PHASE}','--output',f'evidence/{PHASE}-create.json']),
 ('status-initial',['workflow','status',model,'--run',f'run-{PHASE}','--host-context',f'evidence/{PHASE}-host-implementation.json','--output',f'evidence/{PHASE}-status-initial.json'])]:
 gdd(PHASE+'-'+label,*args)
