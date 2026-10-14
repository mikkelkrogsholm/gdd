import json
import sys
from pathlib import Path
from gdd_validator.host import execute_check

root=Path.cwd()
label=sys.argv[1]
r=execute_check(root,[sys.executable,'-B','-m','unittest','-v','test_reservation'],['reservation.py','test_reservation.py'],output=root/'evidence'/('lifecycle.'+label+'.run.json'))
print(json.dumps(r,indent=2))
raise SystemExit(0 if r['result']=='PASS' else 1 if r['result']=='FAIL' else 2)
