# Final bounded review: changed input boundary + independent semantic claims.
exec(open('/tmp/gdd-loop6-protocol-probes.py').read().split("result('positive baseline'")[0])
from itertools import product
from gdd_validator import render_view
OUT=Path(tempfile.mkdtemp(prefix='gdd-loop10-protocol-'))
passed=[]
for n,(primary,project_id,profile,activity) in enumerate(product(['missing','unknown','deep-json','deep-yaml'],['loop6','other'],['model','records'],[None,'implementation'])):
 c=dataset('unknown-primary-'+str(n));p,r,h,w=c;w(r);secondary=p.parent/'secondary.json';secondary.write_text(json.dumps({'format':'gdd-records/0.2','project_id':project_id,'records':{}}))
 if primary=='missing':p.unlink()
 if primary=='unknown':p.write_text('{"format":"gdd/99.0","project":{"id":"loop6"}}')
 if primary=='deep-json':
  m=json.loads(p.read_text());m['nodes']['entity.x']['annotations']={'example.deep':'RAW'};p.write_text(json.dumps(m).replace('"RAW"','['*2000+'0'+']'*2000))
 if primary=='deep-yaml':p.write_text('format: gdd/0.2\nproject: {id: loop6, title: Probe, goal: Parser limit}\nnodes:\n  entity.x:\n    kind: entity\n    description: Value\n    fields: {}\n    annotations:\n      example.deep: '+('['*2000)+'0'+(']'*2000)+'\nedges: []\n')
 report=validate(secondary,model_path=p,profiles=[profile],activity=activity,scope=['entity.x'],host_context=h)
 assert aggregate(report['checks'],'model')=='UNKNOWN',(primary,project_id,profile,activity,report)
 assert exit_code(report)==2 and report['spec_revision'] is None
 assert not any(d['code']=='RECORD_INVALID' for chk in report['checks'] for d in chk['diagnostics'])
 if activity:assert aggregate(report['checks'],'authorization')=='UNRESOLVED'
 elif profile=='records':assert aggregate(report['checks'],'implementation')=='UNKNOWN'
 passed.append('unavailable primary '+str(n))
# Project mismatch is still decidable when the loaded identity is known.
for n,(partial,matching) in enumerate(product([False,True],[False,True])):
 c=dataset('known-primary-'+str(n));p,r,h,w=c;w(r)
 if partial:
  m=json.loads(p.read_text());m['includes']=['missing.fragment.json'];p.write_text(json.dumps(m))
 secondary=p.parent/'secondary.json';secondary.write_text(json.dumps({'format':'gdd-records/0.2','project_id':'loop6' if matching else 'other','records':{}}))
 report=validate(secondary,model_path=p,profiles=['records'],scope=['entity.x'],host_context=h)
 expected='FAIL' if not matching else 'UNKNOWN' if partial else 'PASS';assert aggregate(report['checks'],'model')==expected
 assert exit_code(report)==(1 if not matching else 2 if partial else 0);passed.append('known identity '+str(n))
# Viewer-profile failure/capacity does not promote or erase separate claims.
for mode in ['exact','missing','malformed','capacity']:
 for activity in [None,'implementation']:
  c=dataset('viewer-'+mode+'-'+str(activity));p,r,h,w=c;w(r);artifact=p.parent/'graph.html'
  if mode=='exact':render_view(p,artifact)
  if mode=='malformed':artifact.write_text('<html>Missing artifact payload</html>')
  if mode=='capacity':artifact.write_text('<script id="gdd-data" type="application/json">{"format":"gdd-view/0.2","nodes":'+('['*2000)+'0'+(']'*2000)+'}</script>')
  report=validate(p,profiles=['records','visualization'],scope=['entity.x'],activity=activity,host_context=h,view_path=artifact)
  assert aggregate(report['checks'],'model')=='PASS'
  assert aggregate(report['checks'],'integration')=={'exact':'PASS','missing':'UNKNOWN','malformed':'FAIL','capacity':'UNKNOWN'}[mode]
  assert aggregate(report['checks'],'authorization' if activity else 'implementation')==('GRANTED' if activity else 'PASS')
  assert exit_code(report)=={'exact':0,'missing':2,'malformed':1,'capacity':2}[mode];passed.append('separate visualization '+mode+' '+str(activity))
print('LOOP10 FRESH CHANGED-BOUNDARY CHECKS PASS:',len(passed));print('TEMP:',OUT)
