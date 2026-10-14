from pathlib import Path
from copy import deepcopy
import json,tempfile
from gdd_validator import validate
from gdd_validator.loading import load_bundle,digest
from gdd_validator.report import aggregate,exit_code
from gdd_validator.record_integrity import invalid_record_ids

OUT=Path(tempfile.mkdtemp(prefix='gdd-loop6-protocol-'))
base={'format':'gdd/0.2','project':{'id':'loop6','title':'Loop6','goal':'Synthetic current record isolation'},'nodes':{},'edges':[],'record_files':['records.json']}
for n in ('x','y'):
 base['nodes']['entity.'+n]={'kind':'entity','description':'Value','fields':{}}
 base['nodes']['rule.'+n]={'kind':'rule','description':'Required check','rule':{'category':'invariant','statement':'Required behavior','scope':'targeted','blocking':True,'verification':['probe']}}
 base['edges'].append({'id':'edge.'+n,'from':'rule.'+n,'relation':'constrains','to':'entity.'+n})

def dataset(name,model_change=lambda m:None):
 directory=OUT/name;directory.mkdir();m=deepcopy(base);model_change(m)
 p=directory/'model.json';p.write_text(json.dumps(m))
 def write(records):
  (directory/'records.json').write_text(json.dumps({'format':'gdd-records/0.2','project_id':'loop6','records':records}))
 write({});revision=load_bundle(p).revision;assert revision
 records={}
 for n in ('x','y'):
  plan={'kind':'check','spec_revision':revision,'scope':['rule.'+n],'method':'probe','required_environment':['actual'],'negative_controls':['injected']}
  records['check.'+n]=plan
  records['evidence.'+n]={'kind':'evidence','check':'check.'+n,'check_revision':digest(plan),'implementation':'implementation.x','spec_revision':revision,'code_revision':'code.x','checker':{'name':'synthetic','version':'1'},'scope':['rule.'+n],'result':'PASS','executed':True,'reason':'Synthetic applicability observation; no app execution','environment':{'actual':'fixture'},'evidence_uri':'fixture://loop6'}
  records['binding.'+n]={'kind':'binding','spec_revision':revision,'targets':['rule.'+n],'implementation':'implementation.x','role':'test','resource':'test.py','selector':{'kind':'whole-document'},'status':'checked','revision':'resource.x','code_revision':'code.x'}
 records['decision.x']={'kind':'decision','topic':'approval','activity':'implementation','spec_revision':revision,'policy_revision':'policy.x','scope':['entity.x'],'actor':{'id':'actor.x','kind':'human'},'outcome':'accepted','rationale':'Synthetic current approval'}
 h={'implementation':'implementation.x','code_revision':'code.x','supported_methods':['probe'],'trusted_evidence':['evidence.x','evidence.y'],'verified_bindings':['binding.x','binding.y'],'environment':{'actual':'fixture'},'negative_controls':['injected'],'trusted_decisions':['decision.x'],'policy':{'revision':'policy.x','actor':'actor.x','activity':'implementation','spec_revision':revision,'scope':[],'requirement':'explicit','authorities':['actor.x'],'basis':'Synthetic mandate'}}
 return p,records,h,write

def run(c,activity=None,scope=None):
 p,r,h,write=c;write(r);return validate(p,profiles=['records'],scope=scope or ['entity.x'],activity=activity,host_context=h)
def result(name,c,activity=None):
 report=run(c,activity);print(name,{'model':aggregate(report['checks'],'model'),'implementation':aggregate(report['checks'],'implementation'),'authorization':aggregate(report['checks'],'authorization'),'exit':exit_code(report),'invalid_record_ids':sorted(invalid_record_ids(load_bundle(c[0])))})

result('positive baseline',dataset('positive'))
for target in ['rule.x','entity.x']:
 def collide(m,target=target):
  m['sources']={target:{'role':'input','title':'Colliding source identity','uri':'missing.md','format':'text/markdown','normative':False}}
 c=dataset('model-source-collision-'+target,collide)
 result('model source/node collision '+target,c)
 result('model source/node collision authority '+target,c,'implementation')
print('Directory:',OUT)
