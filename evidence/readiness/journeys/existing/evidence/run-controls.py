import json
import sys
from pathlib import Path
from gdd_validator.host import execute_check

root=Path.cwd();source=(root/'reservation.py').read_text();tests=(root/'test_reservation.py').read_bytes()
mutants={'constructor-return':source.replace('return value','return Reservation()',1),'cancelled-reopen':source.replace('    elif value.state != "reserved":\n','    elif value.state == "cancelled":\n        value.state = "reserved"\n    elif value.state != "reserved":\n',1)}
results=[]
for name,source in mutants.items():
 control=root/'evidence'/'controls'/name;control.mkdir(parents=True)
 (control/'reservation.py').write_text(source);(control/'test_reservation.py').write_bytes(tests)
 receipt=execute_check(control,[sys.executable,'-B','-m','unittest','-v','test_reservation'],['reservation.py','test_reservation.py'],output=control/'check.run.json')
 assert receipt['executed'] and receipt['result']=='FAIL',name
 results.append({'id':name,'result':receipt['result'],'exit_code':receipt['exit_code'],'receipt':str((control/'check.run.json').relative_to(root)),'detected':True})
 print(name,receipt['stderr'])
(root/'evidence/negative-controls.json').write_text(json.dumps(results,indent=2)+'\n')
