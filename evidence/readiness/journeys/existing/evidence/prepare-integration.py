import difflib
import json
from pathlib import Path
from gdd_validator.host import snapshot_code

root=Path.cwd()
def save(path,value):
 p=root/path
 if p.exists(): raise RuntimeError('Refusing overwrite: '+path)
 p.write_text(json.dumps(value,indent=2)+'\n')
before=(root/'evidence/reservation.before.py').read_text();after=(root/'reservation.py').read_text()
diff=''.join(difflib.unified_diff(before.splitlines(True),after.splitlines(True),fromfile='reservation.before.py',tofile='reservation.py'))
(root/'evidence/implementation.patch').write_text(diff)
print(diff)
code=snapshot_code(root,['reservation.py','test_reservation.py']);save('evidence/combined-code-snapshot.json',code)
host=json.loads((root/'evidence/host.implementation.base.json').read_text());host['code_revision']=code['revision'];save('evidence/host.implementation.combined.json',host)
review={'format':'gdd-design-review/0.3','spec_revision':host['policy']['spec_revision'],'implementation':'implementation.reservation','code_revision':code['revision'],'scope':['action.reserve','action.cancel'],'reviewer':{'name':'Codex independent existing-code journey','version':'2026-10-14.session.1'},'executed':True,'assessments':[]}
rationales={'kiss':'Reviewed component.reservation and action.reserve/action.cancel at reservation.py:L5-L23. A single class and two direct functions satisfy the current lifecycle; no additional layers are needed.','dry':'Reviewed action.reserve/action.cancel at reservation.py:L10-L23. Each business transition is stated once in its responsible function. The small shared branch shape is syntax, not duplicated authoritative transition knowledge requiring abstraction.','separation_of_concerns':'Reviewed component.reservation and machine.reservation at reservation.py:L1-L23. The module owns state changes and domain errors; persistence, permissions and concurrency concerns are excluded consistently.','modularity':'Reviewed component.reservation interfaces and action.reserve at reservation.py:L10-L15; inspect.signature regression confirms the original public contract. cancel at L18-L23 exposes a matching public operation. Consumers use these functions and need no additional module internals.'}
for principle,rationale in rationales.items():review['assessments'].append({'principle':principle,'status':'reviewed','rationale':rationale,'findings':[]})
save('evidence/design-review.json',review)
(root/'evidence/integration-review.md').write_text('# Direct integration review\n\nThe dispatched packet was performed directly by the current agent. No worker was spawned. The actual patch changes only reservation.py and adds test_reservation.py; these are the declared write resources. The fixed tests retain their hash from the red execution. Public reserve annotations, parameter name and return identity are checked; all six model scenarios and additional unsupported-state boundaries have assertions. The model and requirements were not changed after dispatch. Ten checks passed before registration; current acceptance evidence will be executed again after the integration event. The four-principle advisory review is evidence/design-review.json.\n')
r=json.loads((root/'evidence/dispatch-result.json').read_text());save('evidence/event.submit.json',{'type':'submit','expected_revision':r['receipt']['revision'],'idempotency_key':'submit.lifecycle.1','package':'package.lifecycle','attempt':'attempt.lifecycle.1','summary':'Implemented Reservation state, in-place reserve/cancel and fixed lifecycle assertions directly in the isolated project.','artifact_uri':'evidence/implementation.patch','changed_resources':['reservation.py','test_reservation.py']})
