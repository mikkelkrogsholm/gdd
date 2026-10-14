from pathlib import Path
exec(Path(__file__).with_name('loop-06-protocol-probes.source.py').read_text().split("result('positive baseline'")[0])
for variant in ['missing-primary','unknown-primary-format','deep-primary']:
 c=dataset(variant);p,r,h,w=c;w(r)
 if variant=='missing-primary':p.unlink()
 elif variant=='unknown-primary-format':p.write_text(json.dumps({'format':'gdd/99.0','project':{'id':'loop6'}}))
 else:
  m=json.loads(p.read_text());m['nodes']['entity.x']['annotations']={'example.data':'RAW'};p.write_text(json.dumps(m).replace('"RAW"','['*2000+'0'+']'*2000))
 for activity in [None,'implementation']:
  report=validate(p.parent/'records.json',model_path=p,profiles=['records'],scope=['entity.x'],activity=activity,host_context=h)
  print({'variant':variant,'activity':activity,'model':aggregate(report['checks'],'model'),'implementation':aggregate(report['checks'],'implementation'),'authorization':aggregate(report['checks'],'authorization'),'exit':exit_code(report),'diagnostics':[(d['code'],d['message']) for chk in report['checks'] for d in chk['diagnostics']]})
print('Directory:',OUT)
