"""Installed command surfaces must project the same read-only inspection results."""

import json
from pathlib import Path
import subprocess
import sys

import pytest

from gdd_validator import analyze_impact, project_status
from gdd_validator.scaffold import init_project

ROOT = Path(__file__).resolve().parents[1]
READY = ROOT / "docs/tutorial/refined.gdd.json"


def cli(*arguments):
    return subprocess.run([sys.executable, "-m", "gdd_validator", *map(str, arguments)],
                          cwd=ROOT, capture_output=True, text=True, timeout=30)


@pytest.mark.parametrize('command,format_name', [('status', 'gdd-status/0.1'), ('impact', 'gdd-impact/0.1')])
def test_json_and_text_project_the_same_ready_scope(command, format_name):
    arguments = [command, READY, '--scope', 'action.add']
    before = READY.read_bytes()
    json_run = cli(*arguments)
    assert json_run.returncode == 0, json_run.stdout + json_run.stderr
    result = json.loads(json_run.stdout)
    assert result['format'] == format_name and result['exit_code'] == 0
    assert result['scope'] == ['action.add'] and result['spec_revision'].startswith('sha256:')
    text_run = cli(*arguments, '--format', 'text')
    assert text_run.returncode == json_run.returncode
    assert 'action.add' in text_run.stdout and text_run.stdout.endswith('\n')
    assert not text_run.stdout.lstrip().startswith('{')
    api = project_status(READY, scope=['action.add']) if command == 'status' else analyze_impact(READY, scope=['action.add'])
    assert api == result
    assert READY.read_bytes() == before


@pytest.mark.parametrize('command', ['status', 'impact'])
@pytest.mark.parametrize('format_name', ['json', 'text'])
def test_exclusive_output_preserves_existing_files(command, format_name, tmp_path):
    output = tmp_path / 'inspection.txt'
    execution = cli(command, READY, '--scope', 'action.add', '--format', format_name, '--output', output)
    assert execution.returncode == 0 and not execution.stdout, execution.stderr
    before = output.read_bytes()
    repeated = cli(command, READY, '--scope', 'action.add', '--format', format_name, '--output', output)
    assert repeated.returncode == 1 and output.read_bytes() == before


@pytest.mark.parametrize('command', ['status', 'impact'])
def test_dangling_output_symlink_is_not_followed(command, tmp_path):
    output, target = tmp_path / 'alias', tmp_path / 'missing'
    output.symlink_to(target)
    execution = cli(command, READY, '--scope', 'action.add', '--output', output)
    assert execution.returncode == 1 and output.is_symlink() and not target.exists()


def test_starter_status_exposes_readiness_without_mutating_project(tmp_path):
    project = tmp_path / 'project'
    init_project(project, project_id='project.status-cli', title='Status CLI')
    before = {path.name: path.read_bytes() for path in project.iterdir()}
    execution = cli('status', project / 'gdd.json', '--scope', 'action.main')
    assert execution.returncode == 1, execution.stdout + execution.stderr
    status = json.loads(execution.stdout)
    assert status['assessments']['readiness']['result'] == 'FAIL'
    questions = json.loads((project / 'gdd.json').read_text())['questions']
    assert questions and all(ident in execution.stdout for ident in questions)
    assert status['next_actions']
    assert {path.name: path.read_bytes() for path in project.iterdir()} == before


def test_impact_requires_explicit_scope_and_unknown_scope_is_visible():
    missing = cli('impact', READY)
    assert missing.returncode == 2 and '--scope' in missing.stderr
    unknown = cli('impact', READY, '--scope', 'action.absent')
    assert unknown.returncode in {1, 2}
    result = json.loads(unknown.stdout)
    assert result['diagnostics'] and result['coverage']['result'] != 'complete'


@pytest.mark.parametrize('command', ['status', 'impact'])
@pytest.mark.parametrize('format_name', ['json', 'text'])
def test_unavailable_model_is_a_controlled_inspection_result(command, format_name, tmp_path):
    execution = cli(command, tmp_path / 'missing.json', '--scope', 'action.absent', '--format', format_name)
    assert execution.returncode in {1, 2}, execution.stdout + execution.stderr
    assert execution.stdout and 'Traceback' not in execution.stderr
    if format_name == 'json':
        result = json.loads(execution.stdout)
        assert result['exit_code'] == execution.returncode
