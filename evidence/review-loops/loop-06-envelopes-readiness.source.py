exec(open('/tmp/gdd-loop6-protocol-probes.py').read().split("result('positive baseline'")[0])
from hashlib import sha256
from gdd_validator.readiness import context_closure
checks=[]
def write_model(path,m,r):
 path.write_text(json.dumps(m));(path.parent/'records.json').write_text(json.dumps({'format':'gdd-records/0.2','project_id':m['project']['id'],'records':r}))
def screen(name,question=False):
 d=OUT/name;d.mkdir();raw=b'# Design\nReadable layout\n';(d/'design.md').write_bytes(raw)
 m={'format':'gdd/0.2','project':{'id':'loop6','title':'Loop6','goal':'Receipt probe'},'nodes':{'screen.x':{'kind':'screen','description':'Screen','interaction':{'states':['normal'],'behavior':'Show','accessibility':['Keyboard'],'responsive':'Fluid'},'design_refs':[{'source':'source.design','purpose':'contract'}]},'scenario.x':{'kind':'scenario','description':'Visible screen','scenario':{'category':'normal','given':'Data','when':'Open','then':'Visible'}}},'edges':[{'id':'edge.exercise','from':'scenario.x','relation':'exercises','to':'screen.x'}],'sources':{'source.design':{'role':'design','title':'Design','uri':'design.md','format':'text/markdown','normative':True,'revision':{'sha256':sha256(raw).hexdigest()}}},'record_files':['records.json']}
 if question:m['questions']={'question.x':{'statement':'Resolve requirement','targets':['screen.x'],'blocks':['implementation'],'status':'resolved','resolution':{'decision':'decision.answer'}}}
 p=d/'model.json';write_model(p,m,{});rev=load_bundle(p).revision
 receipt={'kind':'decision','topic':'readiness_review','scope':[],'spec_revision':rev,'policy_revision':'policy.x','actor':{'id':'actor.x','kind':'human'},'outcome':'accepted','rationale':'Synthetic review'}
 r={'decision.review':receipt}
 if question:r['decision.answer']={**deepcopy(receipt),'topic':'model_change','scope':['screen.x']}
 h={'trusted_decisions':list(r)};write_model(p,m,r);return p,m,r,h
for question in [False,True]:
 for defect in ['valid','untrusted','stale','unknown-scope','missing-check','self-cycle','model-supersedes','wrong-outcome']:
  p,m,r,h=screen(str(question)+'-'+defect,question);ident='decision.answer' if question else 'decision.review'
  if defect=='untrusted':h['trusted_decisions'].remove(ident)
  if defect=='stale':r[ident]['spec_revision']='sha256:'+'a'*64
  if defect=='unknown-scope':r[ident]['scope']=['screen.x','entity.deleted']
  if defect=='missing-check':r[ident]['checks']=['check.deleted']
  if defect=='self-cycle':r[ident]['supersedes']=ident
  if defect=='model-supersedes':r['source.design']=deepcopy(r[ident]);r[ident]['supersedes']='source.design'
  if defect=='wrong-outcome':r[ident]['outcome']='rejected'
  write_model(p,m,r);report=validate(p,profiles=['readiness'],scope=['screen.x'],host_context=h);res=aggregate(report['checks'],'readiness');assert (res=='PASS')==(defect=='valid'),(question,defect,res,report);checks.append(('receipt',question,defect))
# Context envelope is effective for implementation and authorization; it cannot override contradictory top-level scope.
c=dataset('context');p,r,h,write=c;write(r);b=load_bundle(p);pack={'format':'gdd-context/0.2','spec_revision':b.revision,'scope':['entity.x'],'nodes':sorted(context_closure(b,['entity.x'])),'edges':[e['id'] for e in b.model['edges']],'sources':[],'questions':[],'extensions':[]}
for explicit in [None,[],['entity.x']]:
 report=validate(p,profiles=['records'],package=pack,scope=explicit,host_context=h);assert report['scope']==['entity.x'];assert aggregate(report['checks'],'implementation')=='PASS';checks.append(('context effective',explicit))
 report=validate(p,profiles=['records'],package=pack,scope=explicit,host_context=h,activity='implementation');assert aggregate(report['checks'],'authorization')=='GRANTED';checks.append(('context authority',explicit))
plan={'format':'gdd-parallel-plan/0.2','packages':[{'id':'package.'+n,'scope':['entity.'+n],'write_resources':[n+'.py'],'base_revision':b.revision,'integration_plan':'Synthetic integrate'} for n in ['x','y']],'serialized':[]}
for request in [None,[],['entity.y','entity.x']]:
 report=validate(p,profiles=['records'],parallel_plan=plan,scope=request,host_context=h);assert set(report['scope'])=={'entity.x','entity.y'};assert aggregate(report['checks'],'implementation')=='PASS';checks.append(('parallel effective',request))
 report=validate(p,profiles=['records'],parallel_plan=plan,scope=request,host_context=h,activity='implementation');assert aggregate(report['checks'],'authorization')=='UNRESOLVED';checks.append(('parallel no broadened approval',request))
for options in [{'package':pack,'scope':['entity.y']},{'parallel_plan':plan,'scope':['entity.x']},{'package':pack,'parallel_plan':plan}]:
 try:validate(p,**options)
 except ValueError:checks.append(('envelope conflict',str(options)))
 else:raise AssertionError('conflict accepted')
print('FRESH ENVELOPE/READINESS CHECKS PASS:',len(checks));print('TEMP:',OUT)
