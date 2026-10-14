from pathlib import Path
exec(Path(__file__).with_name('loop-06-protocol-probes.source.py').read_text().split("result('positive baseline'")[0])
for profile,activity,scope in [(['model'],None,['entity.x']),(['records'],None,['entity.x']),(['records'],'implementation',['entity.x'])]:
 c=dataset('digest-'+str(profile)+'-'+str(activity));p,r,h,w=c;r['evidence.y']['check_revision']='sha256:'+'a'*64;w(r);report=validate(p,profiles=profile,scope=scope,activity=activity,host_context=h)
 print({'profile':profile,'activity':activity,'model':aggregate(report['checks'],'model'),'implementation':aggregate(report['checks'],'implementation'),'authorization':aggregate(report['checks'],'authorization'),'exit':exit_code(report),'invalid_record_ids':sorted(invalid_record_ids(load_bundle(p))),'diagnostics':[d['code'] for chk in report['checks'] for d in chk['diagnostics']]})
print('Directory:',OUT)
