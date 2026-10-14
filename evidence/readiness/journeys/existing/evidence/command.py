import datetime
import json
import pathlib
import subprocess
import sys

root = pathlib.Path(__file__).resolve().parent.parent
label, *argv = sys.argv[1:]
path = root / 'evidence' / (label + '.command.json')
if path.exists():
    raise SystemExit('Refusing to replace command evidence')
started = datetime.datetime.now(datetime.timezone.utc).isoformat()
proc = subprocess.run(argv, cwd=root, capture_output=True, text=True)
record = {'argv': argv, 'cwd': str(root), 'started_at': started,
          'finished_at': datetime.datetime.now(datetime.timezone.utc).isoformat(),
          'exit_code': proc.returncode, 'stdout': proc.stdout, 'stderr': proc.stderr}
path.write_text(json.dumps(record, indent=2) + '\n')
print(json.dumps({'receipt': str(path), 'exit_code': proc.returncode}))
print(proc.stdout)
print(proc.stderr, file=sys.stderr)
raise SystemExit(proc.returncode)
