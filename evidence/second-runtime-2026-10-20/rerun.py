"""Prepare outside-checkout Claude Code trials; opt in to actual runtime calls."""
from pathlib import Path
import argparse
import hashlib
import json
import os
import shutil
import signal
import subprocess
import sys
import time
import zipfile

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--wheel', required=True, type=Path)
parser.add_argument('--output', required=True, type=Path)
parser.add_argument('--run', action='store_true', help='Run both bounded Claude journeys')
args = parser.parse_args()
wheel = args.wheel.resolve(strict=True)
root = args.output.resolve()
if root.exists():
    parser.error('--output must not exist; preserve earlier trial results')
if any((parent / 'gdd_validator').is_dir() for parent in (root, *root.parents)):
    parser.error('--output must be outside a GDD checkout')
root.mkdir(parents=True)
subprocess.run([sys.executable, '-m', 'venv', str(root / 'venv')], check=True)
python = root / 'venv/bin/python'
subprocess.run([str(python), '-m', 'pip', 'install', str(wheel)], check=True)
skill_files = {}
with zipfile.ZipFile(wheel) as archive:
    prefix = 'gdd_validator/skill/'
    for name in ('fresh', 'existing'):
        project = root / name
        project.mkdir()
        (project / 'receipts').mkdir()
        for item in archive.infolist():
            if not item.filename.startswith(prefix) or item.is_dir():
                continue
            relative = Path(item.filename[len(prefix):])
            if relative.is_absolute() or '..' in relative.parts:
                raise ValueError('Unsafe wheel skill path')
            data = archive.read(item)
            destination = project / '.claude/skills/gdd' / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_bytes(data)
            skill_files[str(relative)] = hashlib.sha256(data).hexdigest()
        brief = Path(__file__).with_name('briefs').joinpath(name + '.md').read_text()
        (project / 'BRIEF.md').write_text(brief)
        (project / 'prompt.txt').write_text('/gdd\n\n' + brief)
inventory = root / 'existing/inventory'
inventory.mkdir()
(inventory / '__init__.py').write_text('')
(inventory / 'domain.py').write_text(
    'def reserve(stock, quantity):\n'
    '    if type(stock) is not int or type(quantity) is not int or stock < 0 or quantity <= 0 or quantity > stock:\n'
    '        raise ValueError("Invalid reservation")\n'
    '    return stock - quantity\n')
(inventory / 'service.py').write_text(
    'from .domain import reserve as _reserve\n\ndef reserve(stock, quantity):\n    return _reserve(stock, quantity)\n')
(root / 'installation.json').write_text(json.dumps({
    'wheel_sha256': hashlib.sha256(wheel.read_bytes()).hexdigest(),
    'wheel': str(wheel), 'skill_files': skill_files,
    'python': subprocess.check_output([str(python), '--version'], text=True).strip(),
    'runtime': subprocess.check_output(['claude', '--version'], text=True).strip(),
    'journeys_requested': args.run,
}, indent=2) + '\n')
if not args.run:
    print('Prepared two isolated projects; no runtime journey executed:', root)
    sys.exit(0)
tools = 'Bash,Read,Write,Edit,Glob,Grep,Skill'
argv = [
    shutil.which('claude'), '-p', '--output-format', 'stream-json', '--verbose',
    '--no-session-persistence', '--setting-sources', 'project',
    '--settings', '{"disableAllHooks":true}', '--strict-mcp-config',
    '--mcp-config', '{"mcpServers":{}}', '--tools', tools, '--allowedTools', tools,
    '--disallowedTools', 'Agent,Task', '--permission-mode', 'dontAsk',
    '--max-turns', '45', '--append-system-prompt',
    'This is a bounded installed-skill runtime evaluation. Work only within the '
    'current project. No source checkouts, other projects, network, global '
    'configuration changes, delegation, or nested Claude/Codex CLIs. Read the '
    'project-installed gdd skill.',
]
env = os.environ.copy()
env.pop('PYTHONPATH', None)
env['PATH'] = str(root / 'venv/bin') + os.pathsep + env['PATH']
failed = False
for name in ('fresh', 'existing'):
    project = root / name
    started = time.time()
    with (project / 'receipts/runtime.jsonl').open('w') as out, (project / 'receipts/stderr.txt').open('w') as err:
        process = subprocess.Popen(argv, cwd=project, env=env, stdin=subprocess.PIPE,
                                   stdout=out, stderr=err, text=True, start_new_session=True)
        timed_out = False
        try:
            process.communicate((project / 'prompt.txt').read_text(), timeout=600)
        except subprocess.TimeoutExpired:
            timed_out = True
            os.killpg(process.pid, signal.SIGTERM)
            try:
                process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                process.wait()
    receipt = {'argv': argv, 'cwd': str(project), 'timeout_seconds': 600,
               'elapsed_seconds': round(time.time() - started, 2),
               'exit_code': process.returncode, 'timed_out': timed_out}
    (project / 'receipts/invocation.json').write_text(json.dumps(receipt, indent=2) + '\n')
    print(name, 'exit', process.returncode, 'timeout', timed_out)
    failed |= process.returncode != 0 or timed_out
sys.exit(1 if failed else 0)
