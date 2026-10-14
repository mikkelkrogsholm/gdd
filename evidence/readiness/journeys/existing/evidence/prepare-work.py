import json
import sys
from pathlib import Path
from gdd_validator.host import append_records

root=Path.cwd()
def save(path,value):
 p=root/path
 if p.exists(): raise RuntimeError('Refusing overwrite: '+path)
 p.write_text(json.dumps(value,indent=2)+'\n')
m=json.loads((root/'model.json').read_text());spec=json.loads((root/'evidence/model.snapshot.v2.json').read_text())['revision'];base=json.loads((root/'evidence/initial-code-snapshot.json').read_text())['revision'];scope=['action.reserve','action.cancel']
records={'decision.refinement':{'kind':'decision','topic':'model_change','scope':list(m['nodes']),'spec_revision':spec,'policy_revision':'policy.human-assignment.v1','actor':{'id':'actor.independent_agent','kind':'agent'},'outcome':'accepted','rationale':'Apply the explicitly requested lifecycle under the human task mandate. The retained candidate is an unaccepted gdd/0.2 sketch; the accepted gdd/0.3 refinement adds boundaries, full contracts, machines and six scenarios. See evidence/model-change.json for the base and changed IDs.'},'check.lifecycle':{'kind':'check','description':'Execute independent unittest assertions for all lifecycle guarantees and public reserve signature.','spec_revision':spec,'scope':['rule.lifecycle','action.reserve','action.cancel','entity.reservation','machine.reservation'],'method':'unittest','required_environment':['python'],'negative_controls':['constructor-return','cancelled-reopen']}}
append_records(root/'records.json', {'records': records})
host={'implementation':'implementation.reservation','code_revision':base,'policy':{'revision':'policy.human-assignment.v1','actor':'actor.independent_agent','activity':'implementation','spec_revision':spec,'scope':scope,'requirement':'standing','authorities':['actor.independent_agent'],'basis':'Trusted human assignment supplied to this agent: lifecycle modeling, implementation, verification and acceptance in the isolated project. The same mandate permits direct bounded implementation without delegation.'},'trusted_decisions':['decision.refinement']}
save('evidence/host.implementation.base.json',host)
save('request.json',{'format':'gdd-work-request/0.1','id':'work.reservation','objective':'Implement the fixed Reservation lifecycle without changing the reserve signature.','implementation':'implementation.reservation','base_code_revision':base,'packages':[{'id':'package.lifecycle','scope':scope,'write_resources':['reservation.py','test_reservation.py'],'read_resources':['requirements.md','model.json','observations/observations.json'],'workspace':str(root),'deliverables':['Reservation state, reserve and cancel operations','Behavior and signature regression assertions'],'integration_plan':'Inspect the direct patch, confirm file ownership and public signature, review all four principles, then execute the fixed assertions and isolated negative controls on the combined result.'}],'serialized':[],'integration':{'owner':'actor.independent_agent','checks':['check.lifecycle']}})
print(json.dumps({'spec_revision':spec,'base_code_revision':base,'scope':scope}))
