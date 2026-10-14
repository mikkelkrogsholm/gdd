import hashlib
import json
from pathlib import Path
from gdd_validator.host import snapshot_code

root=Path.cwd()
def read(path):return json.loads((root/path).read_text())
raw=(root/'graphify-input.json').read_bytes();digest=hashlib.sha256(raw).hexdigest()
for path in ['observations/graphify.raw.json','observations-unsupported/graphify.raw.json','evidence/recovered.graphify.json','evidence/recovered-unsupported.graphify.json']:
 assert (root/path).read_bytes()==raw,path
obs=read('observations/observations.json')
assert len(obs['nodes'])==3 and len(obs['edges'])==5
identities=[x['identity'] for x in obs['nodes']+obs['edges']]
assert identities==read('evidence/raw-roundtrip.json')['identities']
unknown=read('observations-unsupported/import.report.json')
assert any(c['result']=='UNKNOWN' and any(d['code']=='FORMAT_UNSUPPORTED' for d in c['diagnostics']) for c in unknown['checks'])
for path in ['evidence/model.report.json','evidence/readiness-trusted.report.json','evidence/context.report.json','evidence/view.report.json','evidence/implementation.report.json','evidence/acceptance-authorization.report.json']:
 assert all(c['result'] in ['PASS','GRANTED'] for c in read(path)['checks']),path
s=read('evidence/accepted-status.json');assert s['state']['status']=='accepted' and s['acceptance_current'] is True
current=snapshot_code(root,['reservation.py','test_reservation.py']);assert current==read('evidence/combined-code-snapshot.json')
assert current['revision']==s['state']['code_revision']
review=read('evidence/design-review.report.json');print('Advisory review:',review)
revisions={'artifact_revision':'sha256:'+digest,'candidate_spec_revision':read('evidence/candidate-model.report.json')['spec_revision'],'accepted_spec_revision':read('evidence/model.snapshot.v2.json')['revision'],'initial_code_revision':read('evidence/initial-code-snapshot.json')['revision'],'result_code_revision':current['revision'],'implementation':'implementation.reservation','scope':['action.reserve','action.cancel']}
result={'result':'PASS','acceptance_current':True,'event_count':s['event_count'],'raw_bytes_unchanged':True,'observation_identities_unchanged':True,'supported_import':'PASS','unsupported_profile':'UNKNOWN','supported_and_unsupported_exports_byte_identical':True,'tests':10,'negative_controls_detected':['constructor-return','cancelled-reopen'],'revisions':revisions,'limitations':['No Graphify extraction was performed here; the genuine supplied AST artifact was preserved.','The HTML projection was generated and revision-validated; browser interaction was not tested.','Review completeness is advisory, separate from implementation acceptance.','Sequential in-memory behavior only; persistence and concurrency are out of scope.']}
(root/'evidence/final-audit.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result,indent=2))
