from pathlib import Path
import json, subprocess, sys, datetime
ROOT = Path(__file__).resolve().parent
CLI = Path(sys.executable).with_name('gdd')
LOG = ROOT / 'evidence' / 'commands.jsonl'

def write(path, data):
    path = ROOT / path
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data, indent=2, ensure_ascii=False) + '\n' if not isinstance(data, str) else data)

def run(label, argv):
    (ROOT/'evidence').mkdir(exist_ok=True)
    p = subprocess.run([str(x) for x in argv], cwd=ROOT, text=True, capture_output=True)
    record={'label':label,'argv':[str(x) for x in argv],'cwd':str(ROOT),'timestamp':datetime.datetime.now(datetime.timezone.utc).isoformat(),'exit_code':p.returncode,'stdout':p.stdout,'stderr':p.stderr}
    write('evidence/commands/'+label+'.json',record)
    with LOG.open('a') as f: f.write(json.dumps(record,ensure_ascii=False)+'\n')
    print(f'{label}: exit {p.returncode}')
    print(p.stdout if len(p.stdout) < 2000 else p.stdout[:1000] + "\n[Full stdout preserved in command receipt.]\n")
    if p.stderr: print(p.stderr)
    return p

def gdd(label,*args): return run(label,[CLI,*args])

if __name__=='__main__':
    gdd('cli-help','--help')
    gdd('init-a','init','gdd-a','--project-id','project.parcel','--title','Parcel price CLI')
