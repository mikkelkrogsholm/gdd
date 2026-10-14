import hashlib
import json
from pathlib import Path
from gdd_validator.host import snapshot_code

root = Path.cwd()
def save(path, value):
 p=root/path
 if p.exists(): raise RuntimeError('Refusing overwrite: '+path)
 p.write_text(json.dumps(value,indent=2)+'\n')
raw = (root/'graphify-input.json').read_bytes()
recovered = (root/'evidence/recovered.graphify.json').read_bytes()
assert raw == recovered
obs=json.loads((root/'observations/observations.json').read_text())
assert len(obs['nodes']) == 3 and len(obs['edges']) == 5
save('evidence/raw-roundtrip.json',{'result':'PASS','byte_identical':True,'sha256':hashlib.sha256(raw).hexdigest(),'bytes':len(raw),'nodes':3,'edges':5,'identities':[x['identity'] for x in obs['nodes']+obs['edges']]})
(root/'evidence/reservation.before.py').write_bytes((root/'reservation.py').read_bytes())
save('evidence/initial-code-snapshot.json', snapshot_code(root,['reservation.py']))
project={'id':'project.reservation','title':'Reservation lifecycle','goal':'Preserve reserve(value: Reservation) -> Reservation while implementing an in-place draft, reserved, cancelled lifecycle.'}
ids=[x['id'] for x in obs['nodes']]
nodes=[{'observation':ids[0],'target':'component.reservation','kind':'component','description':'Own the local Reservation lifecycle and its public operations.','rationale':'The observed module is a coherent domain boundary; ownership is intended, not inferred authority.'},
 {'observation':ids[1],'target':'entity.reservation','kind':'entity','description':'A Reservation carries its lifecycle state.','rationale':'Interpret the observed class as domain data under the requested lifecycle.'},
 {'observation':ids[2],'target':'action.reserve','kind':'action','description':'Reserve the supplied Reservation in place and return its identity.','rationale':'Interpret the observed function as a public domain action; replace constructor behavior under human requirements.'}]
edges=[]
for i, target in [(0,'entity.reservation'),(1,'action.reserve')]:
 edges.append({'observation':obs['edges'][i]['id'],'id':'edge.'+target.replace('.','_')+'_owner','from':target,'to':'component.reservation','relation':'owned_by','rationale':'Explicit desired ownership corresponding to observed module containment.'})
edges.append({'observation':obs['edges'][2]['id'],'id':'edge.reserve_reads','from':'action.reserve','to':'entity.reservation','relation':'reads','rationale':'The intended action must inspect the supplied entity; a type reference alone does not prove the current read.'})
save('mapping.json',{'format':'gdd-graphify-mapping/0.2','project':project,'nodes':nodes,'edges':edges})
save('evidence/interpretation.json',{'status':'proposed','omissions':[{'observation':obs['edges'][3]['id'],'reason':'The return-type reference is preserved as observation; the authoritative return identity belongs in the action contract.'},{'observation':obs['edges'][4]['id'],'reason':'The constructor call is observed current behavior that conflicts with the requested same-object return. It must not become intended creates.'}],'source_claims':'Producer paths were checked against the actual local reservation.py baseline. Artifact hashes are not source-code hashes or authority.'})
