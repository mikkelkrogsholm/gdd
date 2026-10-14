# Bounded fresh matrix over documented supported envelopes and record semantics.
exec(open('/tmp/gdd-loop6-protocol-probes.py').read().split("result('positive baseline'")[0])
from itertools import product
from gdd_validator.readiness import context_closure
OUT=Path(tempfile.mkdtemp(prefix='gdd-loop8-protocol-'))
passed=[]
def inspect(label,c,dimension,expected,activity=None,scope=None):
 p,r,h,w=c;w(r);report=validate(p,profiles=['records'],scope=['entity.x'] if scope is None else scope,activity=activity,host_context=h);actual=aggregate(report['checks'],dimension);assert actual==expected,(label,actual,expected,report);passed.append(label);return report

# A direct independent oracle: rejection overlaps; approval must cover the full request.
scopes=[['entity.x'],['entity.y'],['entity.x','entity.y'],[]]
for n,(requirement,outcome,trusted,receipt_scope,request_scope) in enumerate(product(['explicit','standing','none'],['accepted','rejected','proposed'],[False,True],[['entity.x'],[]],scopes)):
 c=dataset('authority-'+str(n));r,h=c[1:3];r['decision.x'].update(outcome=outcome,scope=receipt_scope);h['policy']['requirement']=requirement;h['trusted_decisions']=['decision.x'] if trusted else []
 overlaps=not receipt_scope or not request_scope or bool(set(receipt_scope)&set(request_scope));covers=not receipt_scope or bool(request_scope) and set(request_scope)<=set(receipt_scope)
 expected='DENIED' if trusted and outcome=='rejected' and overlaps else 'GRANTED' if requirement!='explicit' or trusted and outcome=='accepted' and covers else 'UNRESOLVED'
 inspect('authority oracle '+str(n),c,'authorization',expected,'implementation',request_scope)

# Retirement varies trust, digest, revisions and explicit current selection.
for n,(variant,selected) in enumerate(product(['valid','untrusted','digest','code','scope','self-cycle'],[False,True])):
 c=dataset('evidence-retirement-'+str(n));r,h=c[1:3];r['evidence.x']['result']='FAIL';r['evidence.new']={**deepcopy(r['evidence.x']),'result':'PASS','supersedes':'evidence.x'};h['trusted_evidence'].append('evidence.new')
 if variant=='untrusted':h['trusted_evidence'].remove('evidence.new')
 if variant=='digest':r['evidence.new']['check_revision']='sha256:'+'b'*64
 if variant=='code':r['evidence.new']['code_revision']='code.other'
 if variant=='scope':r['evidence.new']['scope'].append('entity.deleted')
 if variant=='self-cycle':r['evidence.new']['supersedes']='evidence.new'
 if selected:h['evidence_selection']={'check.x':'evidence.x'}
 expected='UNKNOWN' if selected and variant=='valid' or not selected and variant!='valid' else 'PASS' if variant=='valid' else 'FAIL'
 inspect('retirement oracle '+variant+' '+str(selected),c,'implementation',expected)

# R054: every wrong immutable hash is visible, but selected good/failing work remains independent.
for n,(profile,evidence_result,wrong_hash) in enumerate(product(['model','records'],['PASS','FAIL','UNKNOWN'],[False,True])):
 c=dataset('hash-'+str(n));p,r,h,w=c;r['evidence.x']['result']=evidence_result
 if wrong_hash:r['evidence.y']['check_revision']='sha256:'+'c'*64
 w(r);report=validate(p,profiles=[profile],scope=['entity.x'],host_context=h);assert aggregate(report['checks'],'model')==('FAIL' if wrong_hash else 'PASS');passed.append('ledger digest '+str(n))
 if profile=='records':assert aggregate(report['checks'],'implementation')==evidence_result;passed.append('independent result '+str(n))
 assert any(d['code']=='CHECK_REVISION_MISMATCH' for x in report['checks'] for d in x['diagnostics'])==wrong_hash;passed.append('hash diagnostic '+str(n))

# References must resolve to the required kind; identity collisions cannot be selected around.
for variant in ['missing-check','wrong-kind-check','evidence-record-ID-collision','check-record-ID-collision','binding-record-ID-collision','approval-record-ID-collision']:
 c=dataset('reference-'+variant);r,h=c[1:3]
 if variant=='missing-check':r['evidence.x']['check']='check.deleted'
 if variant=='wrong-kind-check':r['evidence.x']['check']='binding.x'
 if variant=='evidence-record-ID-collision':r['entity.x']=r.pop('evidence.x');h['trusted_evidence']=['entity.x','evidence.y'];h['evidence_selection']={'check.x':'entity.x'}
 if variant=='check-record-ID-collision':r['entity.x']=r.pop('check.x');r['evidence.x'].update(check='entity.x',check_revision=digest(r['entity.x']));h['check_selection']={'rule.x':{'probe':'entity.x'}}
 if variant=='binding-record-ID-collision':r['entity.x']=r.pop('binding.x');h['verified_bindings']=['entity.x','binding.y']
 if variant=='approval-record-ID-collision':r['entity.x']=r.pop('decision.x');h['trusted_decisions']=['entity.x']
 report=inspect('used identity '+variant,c,'authorization' if variant=='approval-record-ID-collision' else 'implementation','UNRESOLVED' if variant=='approval-record-ID-collision' else 'UNKNOWN','implementation' if variant=='approval-record-ID-collision' else None)
 assert aggregate(report['checks'],'model')=='FAIL';passed.append('identity defect visible '+variant)

# Binding role/status/context and explicit mappings are separate from raw evidence truth.
for variant,expected in [('direct-rule','PASS'),('entity-target','PASS'),('wrong-entity','UNKNOWN'),('planned','UNKNOWN'),('design-role','UNKNOWN'),('wrong-code','UNKNOWN'),('unverified','UNKNOWN'),('mixed-valid-invalid','PASS')]:
 c=dataset('bindings-'+variant);r,h=c[1:3]
 if variant=='entity-target':r['binding.x']['targets']=['entity.x']
 if variant=='wrong-entity':r['binding.x']['targets']=['entity.y']
 if variant=='planned':r['binding.x']['status']='planned'
 if variant=='design-role':r['binding.x']['role']='design'
 if variant=='wrong-code':r['binding.x']['code_revision']='code.old'
 if variant=='unverified':h['verified_bindings'].remove('binding.x')
 if variant=='mixed-valid-invalid':r['binding.bad']={**deepcopy(r['binding.x']),'targets':['entity.deleted']};h['verified_bindings'].append('binding.bad')
 inspect('mapping '+variant,c,'implementation',expected)

# Unicode and JSON representation order do not alter the prescribed immutable plan digest.
c=dataset('unicode');p,r,h,w=c;r['check.x']['description']='æøå — 𝄞\nCanonical check';r['evidence.x']['check_revision']=digest(r['check.x']);w(r);canonical_hash=r['evidence.x']['check_revision'];r['check.x']=dict(reversed(list(r['check.x'].items())));inspect('immutable hash is canonical',c,'implementation','PASS');assert digest(r['check.x'])==canonical_hash;passed.append('check hash key order')

# No implicit merger of active alternatives; explicit choices remain scoped.
for variant,expected in [('ambiguous','UNKNOWN'),('missing-selection','UNKNOWN'),('pass-selection','PASS'),('fail-selection','FAIL')]:
 c=dataset('selection-'+variant);r,h=c[1:3];r['evidence.alternative']={**deepcopy(r['evidence.x']),'result':'FAIL'};h['trusted_evidence'].append('evidence.alternative')
 if variant!='ambiguous':h['evidence_selection']={'check.x':{'missing-selection':'evidence.deleted','pass-selection':'evidence.x','fail-selection':'evidence.alternative'}[variant]}
 inspect('selection '+variant,c,'implementation',expected)

# Secondary ledger/model matching is controlled, and repeated declared input is not loaded twice.
for wrong_project in [False,True]:
 c=dataset('secondary-'+str(wrong_project));p,r,h,w=c;w(r)
 second=p.parent/'secondary.json';second.write_text(json.dumps({'format':'gdd-records/0.2','project_id':'other' if wrong_project else 'loop6','records':{'decision.extra':{**deepcopy(r['decision.x']),'scope':['entity.y']}}}))
 report=validate(second,model_path=p,profiles=['records'],scope=['entity.x'],host_context=h);assert aggregate(report['checks'],'model')==('FAIL' if wrong_project else 'PASS');assert aggregate(report['checks'],'implementation')==('UNKNOWN' if wrong_project else 'PASS');passed.append('secondary project '+str(wrong_project))
c=dataset('declared-secondary');p,r,h,w=c;w(r);report=validate(p.parent/'records.json',model_path=p,profiles=['records'],scope=['entity.x'],host_context=h);assert aggregate(report['checks'],'implementation')=='PASS';assert not any(d['code']=='ID_COLLISION' for x in report['checks'] for d in x['diagnostics']);passed.append('declared input once')

# Effective scopes from each supported envelope propagate consistently to authority.
c=dataset('envelopes');p,r,h,w=c;w(r);b=load_bundle(p);pack={'format':'gdd-context/0.2','spec_revision':b.revision,'scope':['entity.y'],'nodes':sorted(context_closure(b,['entity.y'])),'edges':[e['id'] for e in b.model['edges']],'sources':[],'questions':[],'extensions':[]};plan={'format':'gdd-parallel-plan/0.2','packages':[{'id':'package.'+n,'scope':['entity.'+n],'write_resources':[n+'.py'],'base_revision':b.revision,'integration_plan':'Synthetic integrate'} for n in ['x','y']],'serialized':[]}
for label,envelope,expected_scope in [('context',{'package':pack},['entity.y']),('parallel',{'parallel_plan':plan},['entity.x','entity.y'])]:
 for explicit in [None,[],expected_scope]:
  report=validate(p,profiles=['records'],scope=explicit,activity='implementation',host_context=h,**envelope);assert report['scope']==expected_scope;assert aggregate(report['checks'],'authorization')=='UNRESOLVED';passed.append('envelope '+label+' '+str(explicit))
for envelope in [{'package':pack,'scope':['entity.x']},{'parallel_plan':plan,'scope':['entity.x']},{'package':pack,'parallel_plan':plan}]:
 try:validate(p,**envelope)
 except ValueError:passed.append('conflicting scope rejected')
 else:raise AssertionError('envelope conflict accepted')

# Result vocabularies, empty checks and precedence retain conservative report semantics.
for dimension in ['model','readiness','implementation','integration','authorization']:
 yes,no,unknown=('GRANTED','DENIED','UNRESOLVED') if dimension=='authorization' else ('PASS','FAIL','UNKNOWN')
 for values,expected in [([],unknown),([yes],yes),([yes,unknown],unknown),([yes,no,unknown],no)]:
  assert aggregate([{'dimension':dimension,'result':x} for x in values],dimension)==expected;passed.append('aggregate '+dimension+' '+str(values))
assert exit_code({'checks':[]})==2;passed.append('empty exit2')
for dimension,bad in [('model','VERIFIED'),('implementation','GRANTED'),('authorization','PASS')]:
 try:exit_code({'checks':[{'dimension':dimension,'result':bad}]})
 except ValueError:passed.append('invalid vocabulary rejected '+dimension)
 else:raise AssertionError('invalid vocabulary accepted')
print('LOOP8 FRESH PROTOCOL CHECKS PASS:',len(passed));print('TEMP:',OUT)
