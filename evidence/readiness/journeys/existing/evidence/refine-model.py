import hashlib
import json
from pathlib import Path

root=Path.cwd()
def save(path,value):
 p=root/path
 if p.exists(): raise RuntimeError('Refusing overwrite: '+path)
 p.write_text(json.dumps(value,indent=2)+'\n')
brief='''# Accepted Reservation requirements

Human task: build a Reservation lifecycle preserving the public
reserve(value: Reservation) -> Reservation signature. Reservation defaults to
draft. reserve changes draft to reserved and returns the same object. Repeated
reserve on reserved is harmless. cancel changes reserved to cancelled; repeated
cancel is harmless. reserve of cancelled rejects with InvalidTransition.

The task authorizes modeling, implementation, verification, and acceptance in
this isolated project. The observed Graphify graph and current implementation
are provenance, not accepted requirements.

Bounded agent decisions: expose cancel(value: Reservation) -> Reservation as a
module function returning the same object. Reject cancel of draft and unsupported
state values with InvalidTransition, preserving state. Use in-memory, sequential
calls only; no persistence, identity permissions, or thread-safety guarantee.
'''
(root/'requirements.md').write_text(brief)
m=json.loads((root/'candidate.json').read_text())
m['format']='gdd/0.3'
m['project']['scope']='Sequential in-memory Reservation lifecycle in reservation.py and its regression tests.'
m['project']['assumptions']=['Callers supply a Reservation object.','No concurrent-access or persistence guarantee is requested.']
scope=['action.reserve','action.cancel']
m['project']['workflow']={'mode':'autonomous','mandate':'The human assignment authorizes this isolated Reservation modeling, implementation, verification and acceptance journey. The agent may resolve routine implementation details within that scope.','gates':{a:{'requirement':'standing','authorities':['actor.independent_agent'],'scope':scope} for a in ['model_changes','implementation','acceptance']}}
m['record_files']=['records.json']
m['sources']['source.requirements']={'role':'input','title':'Accepted human requirements and bounded agent decisions','uri':'requirements.md','format':'markdown','normative':True,'revision':{'sha256':hashlib.sha256(brief.encode()).hexdigest()}}
ref={'source':'source.requirements','purpose':'contract','selector':{'kind':'whole-document'}}
n=m['nodes']
for node in n.values():
 node['maturity']='specified'
 node['source_refs'].append(ref)
n['component.reservation']['boundary']={'responsibility':'Own Reservation state and the reserve/cancel transitions.','rationale':'One domain module keeps state decisions and public operations together for this small lifecycle.','excludes':['Persistence','Authorization identities','Concurrent access guarantees']}
n['component.reservation']['interfaces']=scope
n['entity.reservation']['fields']={'state':{'type':'enum','values':['draft','reserved','cancelled']}}

def risk(relevant,rationale,scenarios=[]):return {'relevant':relevant,'rationale':rationale,'scenarios':scenarios}
def contract(action):
 reserve=action=='reserve'
 return {'input':{'value':{'type':'reference','target':'entity.reservation'}},'context':{},'output':{'type':'reference','target':'entity.reservation'},'errors':{'InvalidTransition':('Cancelled or unsupported state is rejected without mutation.' if reserve else 'Draft or unsupported state is rejected without mutation.')},'preconditions':['value is a Reservation object.'],'postconditions':['The returned object is the exact supplied object.',('Draft becomes reserved; already reserved remains reserved.' if reserve else 'Reserved becomes cancelled; already cancelled remains cancelled.')],'external_effects':[],'effects_complete':True,'dependencies_complete':True,'risks':{'authorization':risk(False,'There are no actor identities or authorization effects in the local sequential API.'),'idempotency':risk(True,'Repeated calls in the completed state must be harmless.',['scenario.'+action+'_repeat']),'boundaries':risk(True,'Invalid lifecycle edges must reject without changing the object.',['scenario.'+action+'_reject']),'concurrency':risk(False,'This bounded in-memory API makes no concurrent-access guarantee.'),'atomicity':risk(True,'Failure must leave the supplied state unchanged.',['scenario.'+action+'_reject'])},'error_cases':{'InvalidTransition':['scenario.'+action+'_reject']}}
n['action.reserve']['contract']=contract('reserve')
n['action.reserve']['behavior']='Read value.state. If draft, change it to reserved. If already reserved, do nothing. Otherwise raise InvalidTransition before any write. Return value.'
n['action.cancel']={'kind':'action','label':'cancel()','description':'Cancel a reserved Reservation in place; repeated cancellation is harmless.','maturity':'specified','source_refs':[ref],'contract':contract('cancel'),'behavior':'Read value.state. If reserved, change it to cancelled. If already cancelled, do nothing. Otherwise raise InvalidTransition before any write. Return value.'}
n['machine.reservation']={'kind':'machine','description':'Mutating lifecycle edges; successful repeated no-ops belong in the action contracts.','maturity':'specified','source_refs':[ref],'machine':{'states':['draft','reserved','cancelled'],'initial':'draft','terminal':['cancelled'],'transitions':[{'id':'transition.reserve','from':'draft','to':'reserved','trigger':'action.reserve','guard':'value.state is draft','effects':'Set value.state to reserved.'},{'id':'transition.cancel','from':'reserved','to':'cancelled','trigger':'action.cancel','guard':'value.state is reserved','effects':'Set value.state to cancelled.'}]}}
n['rule.lifecycle']={'kind':'rule','description':'Preserve public reserve signature, identity, default state, transitions, no-ops and rejection guarantees.','maturity':'specified','source_refs':[ref],'rule':{'category':'business','statement':'Reservation defaults to draft; reserve retains its signature and identity, draft -> reserved, repeat reserved no-op; cancel reserved -> cancelled, repeat cancelled no-op; reserve cancelled rejects InvalidTransition without mutation.','scope':'targeted','blocking':True,'verification':['unittest']}}
e=m['edges']
def edge(id,fr,to,rel):e.append({'id':id,'from':fr,'to':to,'relation':rel})
for target in ['entity.reservation','action.reserve','action.cancel','machine.reservation']:
 edge('edge.'+target.replace('.','_')+'_owner',target,'component.reservation','owned_by')
edge('edge.state_of','machine.reservation','entity.reservation','state_of')
edge('edge.reserve_changes','action.reserve','entity.reservation','changes')
edge('edge.cancel_reads','action.cancel','entity.reservation','reads')
edge('edge.cancel_changes','action.cancel','entity.reservation','changes')
for action in scope:edge('edge.rule_'+action.split('.')[1],'rule.lifecycle',action,'constrains')
scenarios=[('reserve_normal','normal','A new Reservation with draft state','reserve(value) is called','The same object is returned with state reserved.','reserve'),('reserve_repeat','boundary','A reserved Reservation','reserve(value) is called twice','Both results are the original object and state remains reserved.','reserve'),('reserve_reject','negative','A cancelled Reservation','reserve(value) is called','InvalidTransition is raised and the same object stays cancelled.','reserve'),('cancel_normal','normal','A reserved Reservation','cancel(value) is called','The same object is returned with state cancelled.','cancel'),('cancel_repeat','boundary','A cancelled Reservation','cancel(value) is called twice','The same object is returned and state remains cancelled.','cancel'),('cancel_reject','negative','A draft Reservation','cancel(value) is called','InvalidTransition is raised and state stays draft.','cancel')]
for name,category,given,when,then,action in scenarios:
 sid='scenario.'+name
 n[sid]={'kind':'scenario','description':then,'maturity':'specified','source_refs':[ref],'scenario':{'category':category,'given':given,'when':when,'then':then}}
 edge('edge.'+name+'_exercises',sid,'action.'+action,'exercises')
 edge('edge.'+name+'_verifies',sid,'rule.lifecycle','verifies')
m['questions']['question.graphify_contracts'].update({'status':'resolved','resolution':{'decision':'decision.refinement'}})
save('model.json',m)
save('records.json',{'format':'gdd-records/0.2','project_id':'project.reservation','records':{}})
save('evidence/model-change.json',{'base_manifest':'candidate.json','base_format':'gdd/0.2','result_manifest':'model.json','result_format':'gdd/0.3','changed_ids':list(n),'rationale':'Explicitly refine the unaccepted Graphify sketch under the human lifecycle mandate, add 0.3 boundaries and postconditions, and retain the original observation source as non-normative.','omissions':json.loads((root/'proposal.json').read_text())['omitted_observations'],'endpoint_intervention':'Containment observations cannot reverse to owned_by through propose; ownership is authored visibly in model refinement.'})
