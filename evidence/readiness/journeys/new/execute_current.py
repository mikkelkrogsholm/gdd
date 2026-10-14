from journey_tools import ROOT,write
from gdd_validator.host import execute_check,snapshot_code
from pathlib import Path
import json,sys
phase=sys.argv[1]
files=list(json.loads((ROOT/f'evidence/{phase}-code.json').read_text())['files'])
cli=ROOT/'src/parcel/cli.py'
original=cli.read_bytes()
try:
    cli.write_bytes(original+b'\n# Deliberate negative control: a real forbidden component import.\nfrom . import tariffs\n')
    write(f'evidence/controls/{phase}/cli-forbidden.py',cli.read_text())
    bad=execute_check(ROOT,[sys.executable,'-B','tests/check_architecture.py','src'],files,output=ROOT/f'evidence/{phase}-architecture-injected.run.json')
    print('injected architecture',bad['result'],bad['stdout'])
    assert bad['result']=='FAIL'
finally:cli.write_bytes(original)
assert snapshot_code(ROOT,files)['revision']==json.loads((ROOT/f'evidence/{phase}-code.json').read_text())['revision']
green=execute_check(ROOT,[sys.executable,'-B','tests/check_architecture.py','src'],files,output=ROOT/f'evidence/{phase}-architecture-repaired.run.json')
print('repaired architecture',green['result'],green['stdout'])
assert green['result']=='PASS'
if phase=='a':
    tariffs=ROOT/'src/parcel/tariffs.py';original=tariffs.read_bytes()
    try:
        tariffs.write_bytes(original.replace(b'return 100',b'return 101'))
        write('evidence/controls/a/tariffs-bad-rate.py',tariffs.read_text())
        bad=execute_check(ROOT,[sys.executable,'-B','tests/test_a.py','src'],files,output=ROOT/'evidence/a-behavior-bad-rate.run.json')
        print('bad domestic rate',bad['result']);assert bad['result']=='FAIL'
    finally:tariffs.write_bytes(original)
current=execute_check(ROOT,[sys.executable,'-B',f'tests/test_{phase}.py','src'],files,output=ROOT/f'evidence/{phase}-behavior-current.run.json')
print('current behavior',current['result']);assert current['result']=='PASS'
if phase=='a':
    missing=ROOT/'evidence/missing-code';missing.mkdir()
    unknown=execute_check(ROOT,[sys.executable,'-B','tests/check_architecture.py',str(missing)],['tests/check_architecture.py'],output=ROOT/'evidence/architecture-missing-code.run.json')
    print('missing actual code',unknown['result'],unknown['stdout']);assert unknown['result']=='UNKNOWN'
