from pathlib import Path
import subprocess,os,json,time,signal,sys
name=sys.argv[1];p=Path('/tmp/gdd-second-runtime-20261020')/name
tools='Bash,Read,Write,Edit,Glob,Grep,Skill'
argv=['claude','-p','--output-format','stream-json','--verbose','--no-session-persistence','--setting-sources','project','--settings','{"disableAllHooks":true}','--strict-mcp-config','--mcp-config','{"mcpServers":{}}','--tools',tools,'--allowedTools',tools,'--disallowedTools','Agent,Task','--permission-mode','dontAsk','--max-turns','45','--append-system-prompt','This is a bounded installed-skill runtime evaluation. Do not access source checkouts, other projects, other agents, or the network. Work only within the current directory. No global configuration edits. Read the project-installed gdd skill. No delegation or nested Claude/Codex CLIs.']
env=os.environ.copy();env['PATH']='/tmp/gdd-second-runtime-20261020/venv/bin:'+env['PATH'];env.pop('PYTHONPATH',None)
start=time.time()
with (p/'receipts/runtime.jsonl').open('w') as out,(p/'receipts/stderr.txt').open('w') as err:
 proc=subprocess.Popen(argv,stdin=subprocess.PIPE,stdout=out,stderr=err,cwd=p,env=env,start_new_session=True,text=True)
 try:proc.communicate((p/'prompt.txt').read_text(),timeout=600);timed=False
 except subprocess.TimeoutExpired:os.killpg(proc.pid,signal.SIGTERM);proc.wait();timed=True
(p/'receipts/invocation.json').write_text(json.dumps({'argv':argv,'cwd':str(p),'timeout_seconds':600,'elapsed_seconds':round(time.time()-start,2),'exit_code':proc.returncode,'timed_out':timed,'path_prefix':'/tmp/gdd-second-runtime-20261020/venv/bin','pythonpath_present':False},indent=2)+'\n')
print(name,proc.returncode,'timeout',timed)
