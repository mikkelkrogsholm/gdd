from journey_tools import ROOT,write,gdd
import sys,json
phase,kind,label=sys.argv[1:4]
host_path=sys.argv[4] if len(sys.argv)>4 else f'evidence/{phase}-host-implementation.json'
status_path=f'evidence/{phase}-status-before-{label}.json'
p=gdd(f'{phase}-status-before-{label}','workflow','status',f'gdd-{phase}/gdd.json','--run',f'run-{phase}','--host-context',host_path,'--output',status_path)
status=json.loads((ROOT/status_path).read_text())
event=dict(type=kind,expected_revision=status['revision'],idempotency_key=f'{phase}.{label}')
if kind in ['dispatch','submit','integrate']:
 event.update(package=f'package.parcel.{phase}',attempt=f'attempt.parcel.{phase}.1')
if kind=='submit':
 request=json.loads((ROOT/f'evidence/{phase}-request.json').read_text())
 event.update(summary=f'Implemented and reviewed parcel contract {phase.upper()} and concrete checks within the single authorized package.',artifact_uri=f'evidence/{phase}-integration-review.md',changed_resources=request['packages'][0]['write_resources'])
if kind=='integrate':
 request=json.loads((ROOT/f'evidence/{phase}-request.json').read_text())
 host=json.loads((ROOT/host_path).read_text())
 base=json.loads((ROOT/f'evidence/{phase}-base-code.json').read_text())
 event.update(base_code_revision=base['revision'],code_revision=host['code_revision'],verified_resources=request['packages'][0]['write_resources'],review={'reviewer':'actor.independent','verdict':'PASS','evidence_uri':f'evidence/{phase}-integration-review.md'})
write(f'evidence/{phase}-event-{label}.json',event)
gdd(f'{phase}-event-{label}','workflow','event',f'gdd-{phase}/gdd.json','--run',f'run-{phase}','--event',f'evidence/{phase}-event-{label}.json','--host-context',host_path,'--output',f'evidence/{phase}-event-{label}-result.json')
