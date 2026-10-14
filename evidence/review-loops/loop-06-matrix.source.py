exec(open('/tmp/gdd-loop6-protocol-probes.py').read().split("result('positive baseline'")[0])
checks=[]
def assert_dim(label,c,dimension,expected,activity=None,scope=None):
 report=run(c,activity,scope);actual=aggregate(report['checks'],dimension);assert actual==expected,(label,actual,expected,report);checks.append(label)

def sync(c):
 c[1]['evidence.x']['check_revision']=digest(c[1]['check.x'])

assert_dim('valid implementation',dataset('baseline'), 'implementation','PASS')
assert_dim('valid explicit approval',dataset('auth-baseline'),'authorization','GRANTED','implementation')
for field,value in [('spec_revision','sha256:'+'a'*64),('implementation','implementation.other'),('code_revision','code.other'),('check_revision','sha256:'+'a'*64),('executed',False),('scope',['rule.y']),('environment',{}),('check','check.y')]:
 c=dataset('evidence-'+field);c[1]['evidence.x'][field]=value
 assert_dim('evidence mismatch '+field,c,'implementation','UNKNOWN')
for field,value in [('supported_methods',[]),('trusted_evidence',[]),('verified_bindings',[]),('implementation','implementation.other'),('code_revision','code.other'),('environment',{}),('negative_controls',[])]:
 c=dataset('host-'+field);c[2][field]=value
 assert_dim('host mismatch '+field,c,'implementation','UNKNOWN')
for kind,field,value in [('check','scope',['rule.x','entity.deleted']),('binding','targets',['rule.x','entity.deleted']),('binding','code_revision','code.other'),('binding','implementation','implementation.other'),('binding','status','planned'),('binding','role','design'),('check','method','other')]:
 c=dataset('record-'+kind+'-'+field);c[1][kind+'.x'][field]=value
 if kind=='check':sync(c)
 assert_dim('record mismatch '+kind+'.'+field,c,'implementation','UNKNOWN')
for kind in ['check','evidence','binding']:
 for defect in ['self','two-cycle','wrong-kind','missing','model-ref']:
  c=dataset(kind+'-supersedes-'+defect);r=c[1];ident=kind+'.x'
  r[ident]['supersedes']={'self':ident,'two-cycle':kind+'.other','wrong-kind':('evidence.y' if kind!='evidence' else 'check.y'),'missing':kind+'.deleted','model-ref':'entity.x'}[defect]
  if defect=='two-cycle':r[kind+'.other']={**deepcopy(r[ident]),'supersedes':ident}
  if kind=='check':sync(c)
  assert_dim('invalid used supersession '+kind+' '+defect,c,'implementation','UNKNOWN')
# Invalid successor cannot retire verified FAIL. Explicit selection keeps the valid observation unambiguous.
for defect in ['wrong-digest','unknown-scope','self-cycle','wrong-code','untrusted']:
 c=dataset('fail-retirement-'+defect);r,h=c[1:3];r['evidence.x']['result']='FAIL';new={**deepcopy(r['evidence.x']),'result':'PASS','supersedes':'evidence.x'};r['evidence.other']=new;h['trusted_evidence'].append('evidence.other');h['evidence_selection']={'check.x':'evidence.x'}
 if defect=='wrong-digest':new['check_revision']='sha256:'+'a'*64
 if defect=='unknown-scope':new['scope'].append('entity.deleted')
 if defect=='self-cycle':new['supersedes']='evidence.other'
 if defect=='wrong-code':new['code_revision']='code.other'
 if defect=='untrusted':h['trusted_evidence'].remove('evidence.other')
 assert_dim('invalid successor preserves FAIL '+defect,c,'implementation','FAIL')
# Valid successor retires old failure.
c=dataset('valid-retirement');r,h=c[1:3];r['evidence.x']['result']='FAIL';r['evidence.other']={**deepcopy(r['evidence.x']),'result':'PASS','supersedes':'evidence.x'};h['trusted_evidence'].append('evidence.other');assert_dim('valid successor retires FAIL',c,'implementation','PASS')
# Independent good proof and failure survive malformed unrelated used records.
for defect in ['cycle','missing-target','bad-digest']:
 c=dataset('isolation-'+defect);r=c[1];r['evidence.y']['result']='FAIL'
 if defect=='cycle':r['evidence.x']['supersedes']='evidence.x'
 if defect=='missing-target':r['evidence.x']['scope'].append('entity.deleted')
 if defect=='bad-digest':r['evidence.x']['check_revision']='sha256:'+'a'*64
 report=run(c,scope=['entity.x','entity.y']);values={tuple(x['scope']):x['result'] for x in report['checks'] if x['dimension']=='implementation'};assert values=={('rule.x',):'UNKNOWN',('rule.y',):'FAIL'},(defect,values);checks.append('independent FAIL '+defect)
# Authority trust and full scope controls.
for field,value in [('spec_revision','sha256:'+'a'*64),('policy_revision','policy.other'),('activity','model_changes'),('topic','readiness_review'),('scope',['entity.y']),('outcome','proposed')]:
 c=dataset('approval-'+field);c[1]['decision.x'][field]=value;assert_dim('approval mismatch '+field,c,'authorization','UNRESOLVED','implementation')
for kind in ['untrusted','actor','missing-target','self-cycle','record-model-collision']:
 c=dataset('approval-'+kind);r,h=c[1:3]
 if kind=='untrusted':h['trusted_decisions']=[]
 if kind=='actor':r['decision.x']['actor']['id']='actor.untrusted'
 if kind=='missing-target':r['decision.x']['scope'].append('entity.deleted')
 if kind=='self-cycle':r['decision.x']['supersedes']='decision.x'
 if kind=='record-model-collision':r['entity.x']=r.pop('decision.x');h['trusted_decisions']=['entity.x']
 assert_dim('approval invalid '+kind,c,'authorization','UNRESOLVED','implementation')
for requirement in ['standing','none']:
 c=dataset('standing-'+requirement);r,h=c[1:3];h['policy']['requirement']=requirement;h['trusted_decisions']=[]
 assert_dim('standing controls '+requirement,c,'authorization','GRANTED','implementation')
 r['decision.y']={**deepcopy(r['decision.x']),'scope':['entity.y'],'outcome':'rejected','supersedes':'decision.y'};h['trusted_decisions']=['decision.y']
 assert_dim('unrelated invalid rejection does not veto standing '+requirement,c,'authorization','GRANTED','implementation')
 r['decision.y']['scope']=['entity.x'];assert_dim('invalid explicit overlapping rejection retained '+requirement,c,'authorization','DENIED','implementation')
# Model-ID reference guard on retirement from LOOP5.
for target in ['entity.x','rule.x','edge.x']:
 c=dataset('supersedes-collision-'+target);r,h=c[1:3];r[target]={**deepcopy(r['decision.x']),'outcome':'rejected'};r['decision.x']['supersedes']=target;h['trusted_decisions'].append(target);h['policy']['requirement']='standing';assert_dim('colliding supersedes cannot erase rejection '+target,c,'authorization','DENIED','implementation')
# Default/global scope and partial rejection behavior, retirement before request coverage.
for request in [['entity.x'],['entity.x','entity.y'],[]]:
 c=dataset('narrow-retirement-'+str(len(request)));r,h=c[1:3];r['decision.reject']={**deepcopy(r['decision.x']),'outcome':'rejected'};r['decision.x']['supersedes']='decision.reject';h['trusted_decisions'].append('decision.reject');h['policy']['requirement']='standing';c[3](r);report=validate(c[0],profiles=['records'],scope=request,activity='implementation',host_context=h);assert aggregate(report['checks'],'authorization')=='GRANTED';checks.append('narrow retirement '+str(request))
# Report and API envelope checks.
for dimension,good,bad in [('model','PASS','GRANTED'),('authorization','GRANTED','PASS'),('implementation','FAIL','DENIED')]:
 assert aggregate([{'dimension':dimension,'result':good}],dimension)==good;checks.append('aggregate '+dimension)
 try:aggregate([{'dimension':dimension,'result':bad}],dimension)
 except ValueError:checks.append('invalid result '+dimension)
 else:raise AssertionError('bad report vocabulary accepted')
assert exit_code({'checks':[]})==2;assert aggregate([],'implementation')=='UNKNOWN';checks.append('nonvacuous report')
print('FRESH CHECKS PASS:',len(checks));print('TEMP:',OUT)
