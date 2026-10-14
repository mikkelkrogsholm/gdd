from journey_tools import ROOT,write,run
from gdd_validator.host import snapshot_code,execute_check
import sys,json,hashlib
p=ROOT/'src/parcel/pricing.py'
original=p.read_text()
assert 'return begun_kg * rate(zone)' in original
p.write_text(original.replace('return begun_kg * rate(zone)','handling_fee = 50 if zone == "remote" else 0\n    return begun_kg * rate(zone) + handling_fee'))
fixed=json.loads((ROOT/'evidence/b-fixed-test-hash.json').read_text())
assert hashlib.sha256((ROOT/'tests/test_b.py').read_bytes()).hexdigest()==fixed['sha256']
files=list(json.loads((ROOT/'evidence/b-red-code.json').read_text())['files'])
code=snapshot_code(ROOT,files);write('evidence/b-code.json',code)
base=json.loads((ROOT/'evidence/b-base-code.json').read_text())
changed=[p for p,h in code['files'].items() if base['files'].get(p)!=h]
assert changed==['src/parcel/pricing.py','tests/test_b.py'],changed
for name,command in [('behavior',[sys.executable,'-B','tests/test_b.py','src']),('architecture',[sys.executable,'-B','tests/check_architecture.py','src'])]:
 r=execute_check(ROOT,command,files,output=ROOT/f'evidence/b-preintegration-{name}.run.json');print(name,r['result']);assert r['result']=='PASS'
host=json.loads((ROOT/'evidence/b-host-implementation.json').read_text());host['code_revision']=code['revision'];write('evidence/b-host-integrated.json',host)
write('evidence/b-integration-review.md',f'''# Integration review B

Model revision: {host['policy']['spec_revision']}
Base code revision: {base['revision']}
Result code revision: {code['revision']}
Changed actual resources: src/parcel/pricing.py, tests/test_b.py.
Reviewer: actor.independent, executing lead agent.

The requested change adds one flat 50-cent fee only when zone is remote. Begun
kilogram rounding and rate lookup retain one owner and unchanged values. CLI and
tariffs bytes match the accepted A snapshot. Test B bytes match the fixed hash
recorded before the real red execution; expected amounts were never weakened.
Both preintegration commands actually pass on B; the prior red receipt remains
FAIL on the prior code and is never used as current PASS evidence.

KISS: the fee uses a conditional integer in quote, without adding another layer.
DRY: rate lookup and rounding still have one owner; the flat fee appears once in
runtime policy (src/parcel/pricing.py quote).
Separation of concerns: the fee belongs to full quote policy and changes neither
CLI parsing/output nor tariffs (action.quote, component.pricing).
Modularity: the existing public quote/rate contracts still mediate calls; the
fee change needs no tariff or CLI knowledge (src/parcel/cli.py main and
src/parcel/tariffs.py rate). No new advisory recommendation is needed.

This performed review covers actual combined sources. Postintegration executions
and revision-bound evidence are required before technical acceptance.
''')
run('submit-b',[sys.executable,'-B','workflow_event.py','b','submit','submit','evidence/b-host-integrated.json'])
run('integrate-b',[sys.executable,'-B','workflow_event.py','b','integrate','integrate','evidence/b-host-integrated.json'])
