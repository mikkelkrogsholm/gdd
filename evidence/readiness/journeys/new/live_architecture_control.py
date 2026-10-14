from journey_tools import ROOT,write
from gdd_validator.host import execute_check,snapshot_code
import json,sys
phase='b';files=list(json.loads((ROOT/'evidence/b-code.json').read_text())['files'])
cli=ROOT/'src/parcel/cli.py';original=cli.read_bytes()
try:
    mutated=original.replace(b'import argparse\n',b'import argparse\nfrom . import tariffs  # Deliberate forbidden import executes on module startup.\n')
    assert mutated!=original
    cli.write_bytes(mutated)
    write('evidence/controls/b/cli-live-forbidden.py',cli.read_text())
    ast=execute_check(ROOT,[sys.executable,'-B','tests/check_architecture.py','src'],files,output=ROOT/'evidence/b-architecture-live-injected.run.json')
    print('live injected architecture',ast['result'],ast['stdout']);assert ast['result']=='FAIL'
    smoke=execute_check(ROOT,[sys.executable,'-B','-c',"import sys; sys.path.insert(0,'src'); from parcel.cli import main; raise SystemExit(main(['1','remote']))"],files,output=ROOT/'evidence/b-cli-live-injected.run.json')
    print('live forbidden module CLI execution',smoke['result'],smoke['stdout']);assert smoke['stdout']=='200\n'
finally:cli.write_bytes(original)
assert snapshot_code(ROOT,files)['revision']==json.loads((ROOT/'evidence/b-code.json').read_text())['revision']
repaired=execute_check(ROOT,[sys.executable,'-B','tests/check_architecture.py','src'],files,output=ROOT/'evidence/b-architecture-live-repaired.run.json')
print('live repair architecture',repaired['result'],repaired['stdout']);assert repaired['result']=='PASS'
