import json
import sys
from pathlib import Path
from gdd_validator.host import verification_records, append_records, snapshot_code

root=Path.cwd()
controls=json.loads((root/'evidence/negative-controls.json').read_text())
assert all(c['detected'] for c in controls)
current=snapshot_code(root,['reservation.py','test_reservation.py'])
receipt=json.loads((root/'evidence/lifecycle.integrated.run.json').read_text())
assert receipt['result']=='PASS' and receipt['executed'] and receipt['code_revision']==current['revision']
assert receipt['snapshot']['files']['test_reservation.py']==json.loads((root/'evidence/lifecycle.red.run.json').read_text())['snapshot']['files']['test_reservation.py']
fragment=verification_records(root/'model.json','check.lifecycle',root/'evidence/lifecycle.integrated.run.json',root=root,implementation='implementation.reservation',binding_id='binding.lifecycle.tests.v1',evidence_id='evidence.lifecycle.integrated.v1',test_resource='test_reservation.py',checker={'name':'stdlib.unittest','version':sys.version.split()[0]},environment={'python':sys.version},verified_negative_controls=[c['id'] for c in controls])
code_binding={'kind':'binding','description':'Actual implementation inspected in the isolated project.','spec_revision':fragment['records']['binding.lifecycle.tests.v1']['spec_revision'],'targets':['component.reservation','entity.reservation','action.reserve','action.cancel','machine.reservation','rule.lifecycle'],'implementation':'implementation.reservation','role':'code','resource':'reservation.py','selector':{'kind':'whole-document'},'status':'checked','revision':'sha256:'+current['files']['reservation.py'],'code_revision':current['revision']}
fragment['records']['binding.lifecycle.code.v1']=code_binding
fragment['host']['verified_bindings'].append('binding.lifecycle.code.v1')
append_records(root/'records.json',fragment)
(root/'evidence/verification-fragment.json').write_text(json.dumps(fragment,indent=2)+'\n')
host=json.loads((root/'evidence/host.implementation.combined.json').read_text());host.update(fragment['host']);host['implementation']='implementation.reservation';host['code_revision']=current['revision'];host['policy']['activity']='acceptance'
(root/'evidence/host.acceptance.json').write_text(json.dumps(host,indent=2)+'\n')
print(json.dumps(fragment['host'],indent=2))
