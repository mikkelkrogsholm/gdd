"""Perspective input failures retain validation status and diagnostics."""
from hashlib import sha256
import importlib
import json
from pathlib import Path
import shutil
import subprocess
import sys

import pytest

from gdd_validator import perspectives, load_perspective_bundle, perspective_data
from gdd_validator.loading import InputError

ROOT = Path(__file__).resolve().parents[1]


def fixture(tmp_path):
    shutil.copytree(ROOT / 'examples/product', tmp_path / 'product')
    return tmp_path / 'product/greenfield.gdd.json'


def mutate(path, update):
    model = json.loads(path.read_text())
    update(model)
    path.write_text(json.dumps(model))


def run(path, output, command="perspectives"):
    return subprocess.run([sys.executable, '-m', 'gdd_validator', command,
                           str(path), '--output', str(output)], capture_output=True, text=True)


@pytest.mark.parametrize('command', ['perspectives', 'view'])
@pytest.mark.parametrize('case,code,diagnostic', [
    ('missing', 2, 'SOURCE_UNAVAILABLE'),
    ('directory', 2, 'SOURCE_UNAVAILABLE'),
    ('include', 2, 'SOURCE_UNAVAILABLE'),
    ('extension', 2, 'EXTENSION_UNSUPPORTED'),
    ('extension-format', 2, 'FORMAT_UNSUPPORTED'),
    ('extension-syntax', 1, 'YAML'),
    ('include-format', 2, 'FORMAT_UNSUPPORTED'),
    ('invalid-data', 1, 'EXTENSION_DATA_INVALID'),
    ('product-version', 2, 'Unsupported gdd.product semantics'),
    ('normative', 2, 'SOURCE_UNAVAILABLE'),
    ('format', 2, 'FORMAT_UNSUPPORTED'),
    ('syntax', 1, 'YAML'),
    ('schema', 1, 'SCHEMA_INVALID'),
    ('reference', 1, 'REFERENCE_UNRESOLVED'),
    ('hash', 1, 'EXTENSION_DATA_INVALID'),
    ('escape', 1, 'ASSET_PATH_INVALID'),
    ('mixed', 1, 'EXTENSION_DATA_INVALID'),
    ('depth', 2, 'FORMAT_UNSUPPORTED'),
])
def test_input_boundary(tmp_path, case, code, diagnostic, command):
    path = fixture(tmp_path)
    if case == 'missing': path.unlink()
    elif case == 'directory': path = path.parent
    elif case == 'include': mutate(path, lambda m: m.update(includes=['missing.json']))
    elif case == 'extension': (path.parent / 'extensions/product-0.1.json').unlink()
    elif case == 'extension-format': (path.parent / 'extensions/product-0.1.json').write_text('{"format":"gdd-extension/99"}')
    elif case == 'extension-syntax': (path.parent / 'extensions/product-0.1.json').write_text('{"format":')
    elif case == 'include-format':
        mutate(path, lambda m: m.update(includes=['future.json']))
        (path.parent / 'future.json').write_text('{"format":"gdd-fragment/99"}')
    elif case == 'product-version':
        asset = path.parent / 'extensions/product-0.1.json'
        definition = json.loads(asset.read_text())
        definition['version'] = '0.2.0'
        raw = json.dumps(definition).encode()
        asset.write_bytes(raw)
        mutate(path, lambda m: m['extensions']['extension.product'].update(version='0.2.0', sha256=sha256(raw).hexdigest()))
    elif case == 'invalid-data': mutate(path, lambda m: m['nodes']['step.reserve']['data'].update(user_action=''))
    elif case == 'normative':
        mutate(path, lambda m: m.update(sources={'source.contract': {
            'role': 'input', 'format': 'text/plain', 'title': 'Contract', 'uri': 'missing.txt',
            'normative': True, 'revision': {'sha256': '0' * 64}}}))
    elif case == 'format': path.write_text('{"format":"gdd/99"}')
    elif case == 'syntax': path.write_text('{"format":')
    elif case == 'schema': path.write_text('{"format":"gdd/0.3"}')
    elif case == 'reference': mutate(path, lambda m: m['edges'][0].update(to='node.absent'))
    elif case in {'hash', 'mixed'}:
        mutate(path, lambda m: m['extensions']['extension.product'].update(sha256='0' * 64))
        if case == 'mixed': mutate(path, lambda m: m.update(includes=['missing.json']))
    elif case == 'escape': mutate(path, lambda m: m.update(includes=['../outside.json']))
    elif case == 'depth': path.write_text('{"format":"gdd/0.3","nested":' + '[' * 260 + '0' + ']' * 260 + '}')
    output = tmp_path / 'projection.json'
    result = run(path, output, command)
    assert result.returncode == code, result.stderr
    assert diagnostic in result.stderr
    assert not result.stdout and not output.exists()
    assert 'Traceback' not in result.stderr
    if case == 'mixed': assert 'SOURCE_UNAVAILABLE' in result.stderr
    # Rejection also preserves an existing caller-owned output.
    output.write_text('preserved')
    repeated = run(path, output, command)
    assert repeated.returncode == code
    assert output.read_text() == 'preserved'


def test_non_manifest_format_is_unresolved(tmp_path):
    path = tmp_path / 'records.json'
    path.write_text(json.dumps({'format': 'gdd-records/0.2', 'project_id': 'example.test', 'records': {}}))
    result = run(path, tmp_path / 'out.json')
    assert result.returncode == 2, result.stderr
    assert 'supported GDD' in result.stderr


@pytest.mark.parametrize('command', ['perspectives', 'view'])
@pytest.mark.parametrize('with_failure', [False, True])
def test_extension_semantic_capacity_is_unknown(tmp_path, with_failure, command):
    path = fixture(tmp_path)
    extension = path.parent / 'extensions/product-0.1.json'
    definition = json.loads(extension.read_text())
    definition['node_kinds']['gdd.product.actor']['data_schema'] = {'$ref': '#'}
    raw = json.dumps(definition).encode()
    extension.write_bytes(raw)
    mutate(path, lambda m: m['extensions']['extension.product'].update(sha256=sha256(raw).hexdigest()))
    if with_failure:
        mutate(path, lambda m: m['edges'][0].update(to='node.absent'))
    result = run(path, tmp_path / 'out.json', command)
    assert result.returncode == (1 if with_failure else 2), result.stderr
    if with_failure: assert 'REFERENCE_UNRESOLVED' in result.stderr
    assert 'EXTENSION_UNSUPPORTED' in result.stderr
    assert 'Traceback' not in result.stderr


def test_projection_capacity_and_cli_serialization_are_unresolved(tmp_path, monkeypatch):
    path = fixture(tmp_path)
    bundle = load_perspective_bundle(path)
    module = importlib.import_module('gdd_validator.perspectives')
    def exhausted(*args, **kwargs):
        raise RecursionError('simulated consumer exhaustion')
    monkeypatch.setattr(module, 'deepcopy', exhausted)
    with pytest.raises(InputError) as error:
        perspective_data(bundle)
    assert error.value.code == 'FORMAT_UNSUPPORTED'
    code = """
import json, sys
from gdd_validator.cli import main
original = json.dumps
def exhausted(value, *args, **kwargs):
    if isinstance(value, dict) and value.get('format') == 'gdd-perspectives/0.1':
        raise RecursionError('simulated output exhaustion')
    return original(value, *args, **kwargs)
json.dumps = exhausted
sys.argv = ['gdd', 'perspectives', sys.argv[1], '--output', sys.argv[2]]
raise SystemExit(main())
"""
    output = tmp_path / 'out.json'
    result = subprocess.run([sys.executable, '-c', code, str(path), str(output)], capture_output=True, text=True)
    assert result.returncode == 2 and not result.stdout and not output.exists()
    assert 'nesting capacity' in result.stderr and 'Traceback' not in result.stderr


@pytest.mark.parametrize('depth,expected', [(250, 0), (260, 2)])
def test_real_valid_annotation_nesting(tmp_path, depth, expected):
    path = tmp_path / 'deep.json'
    model = {'format': 'gdd/0.3', 'project': {'id': 'review.deep', 'title': 'Deep', 'goal': 'Inspect'},
             'nodes': {'entity.item': {'kind': 'entity', 'description': 'Nested data',
                                      'annotations': {'example.deep': 'NESTED'}}}, 'edges': []}
    path.write_text(json.dumps(model).replace('"NESTED"', '[' * depth + '0' + ']' * depth))
    output = tmp_path / 'projection.json'
    result = run(path, output)
    assert result.returncode == expected, result.stderr
    assert 'Traceback' not in result.stderr
    if expected == 0:
        assert json.loads(output.read_text())['views']['data']['nodes']['entity.item']
    else:
        assert 'FORMAT_UNSUPPORTED' in result.stderr and not output.exists()


def test_malformed_caller_path_has_no_partial_output(tmp_path):
    with pytest.raises(ValueError):
        perspectives('invalid\0path.json')
    output = tmp_path / 'projection.json'
    code = """
import sys
from gdd_validator.cli import main
sys.argv = ['gdd', 'perspectives', 'invalid' + chr(0) + 'path.json', '--output', sys.argv[1]]
raise SystemExit(main())
"""
    result = subprocess.run([sys.executable, '-c', code, str(output)], capture_output=True, text=True)
    assert result.returncode == 1 and not result.stdout and not output.exists()
    assert 'Traceback' not in result.stderr


@pytest.mark.parametrize('source', ['examples/extension.gdd.yaml', 'examples/product/partial.gdd.json'])
def test_viewer_legacy_extension_and_partial_controls(tmp_path, source):
    output = tmp_path / 'view.html'
    result = run(ROOT / source, output, 'view')
    assert result.returncode == 0, result.stderr
    assert not result.stderr
    assert json.loads(result.stdout)['format'] == 'gdd-report/0.2'
    assert '<html' in output.read_text()
