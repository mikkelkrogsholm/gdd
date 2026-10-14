from pathlib import Path
from tempfile import TemporaryDirectory
from decimal import Decimal
from math import ceil, floor
from itertools import product
from copy import deepcopy
import json, runpy, sys

ROOT = Path('/Users/mikkelfreltoftkrogsholm/Projekter/gdd')
sys.path.insert(0, str(ROOT))
from gdd_validator import validate
from gdd_validator.loading import canonical, load_bundle, parse, InputError
from gdd_validator.model import validate_model
from gdd_validator.readiness import context_closure
from gdd_validator.report import aggregate

counts = {}
with TemporaryDirectory(prefix='gdd-loop9-model-') as directory:
    root = Path(directory)
    numbers = ['-1e300', '-0.3', '0', '0.1', '0.3', '1e23', '100000000000000000000000', '1e300']
    path = root / 'numeric.json'
    for kind, minimum, maximum in product(['number', 'integer'], numbers, numbers):
        model = {'format': 'gdd/0.2', 'project': {'id': 'review', 'title': 'Review', 'goal': 'Exact numeric oracle'}, 'nodes': {'entity.x': {'kind': 'entity', 'description': 'Value', 'fields': {'v': {'type': kind, 'minimum': 'MIN', 'maximum': 'MAX'}}}}, 'edges': []}
        path.write_text(json.dumps(model).replace('"MIN"', minimum).replace('"MAX"', maximum))
        lo, hi = Decimal(minimum), Decimal(maximum)
        expected = 'PASS' if lo <= hi and (kind == 'number' or ceil(lo) <= floor(hi)) else 'FAIL'
        assert aggregate(validate(path)['checks'], 'model') == expected
    counts['independent_decimal_core_oracle'] = 128

    helper = runpy.run_path(str(ROOT / 'tests/test_review_numbers_loop7.py'))['run_extension']
    integer = '100000000000000000000000'
    previous = '99999999999999999999999'
    conditional = {'if': {'type': 'integer'}, 'then': {'const': 'TOKEN'}, 'else': {'multipleOf': 0.1}}
    cases = [
        (conditional, '1e23', 'PASS'), (conditional, previous, 'FAIL'),
        (conditional, '0.3', 'PASS'), (conditional, '0.31', 'FAIL'),
        ({'type': 'array', 'contains': {'const': 'TOKEN'}, 'minContains': 2}, '[1e23,' + integer + ']', 'PASS'),
        ({'type': 'array', 'contains': {'const': 'TOKEN'}, 'maxContains': 1}, '[1e23,' + integer + ']', 'FAIL'),
        ({'not': {'const': 'TOKEN'}}, '1e23', 'FAIL'),
        ({'not': {'const': 'TOKEN'}}, 'true', 'PASS'),
    ]
    for constraint, value, expected in cases:
        report, path = helper(root, constraint, value, schema_tokens={'TOKEN': integer}, mode='dynamic')
        assert aggregate(report['checks'], 'model') == expected
        bundle = load_bundle(path)
        before = canonical(bundle.model), canonical(bundle.extensions), deepcopy(bundle.assets), deepcopy(bundle.snapshot)
        validate_model(bundle)
        assert before == (canonical(bundle.model), canonical(bundle.extensions), bundle.assets, bundle.snapshot)
        assert type(bundle.model['nodes']['node.x']['data']['integer']) is int
    counts['dynamic_ref_composite_exact_assertions_and_nonmutation'] = len(cases)

    model = {'format': 'gdd/0.2', 'project': {'id': 'review', 'title': 'Review', 'goal': 'Closure'}, 'nodes': {'component.p': {'kind': 'component', 'description': 'Parent', 'interfaces': []}, 'entity.child': {'kind': 'entity', 'description': 'Child', 'fields': {}}, 'entity.sibling': {'kind': 'entity', 'description': 'Sibling', 'fields': {}}, 'entity.peer': {'kind': 'entity', 'description': 'Peer', 'fields': {}}, 'scenario.support': {'kind': 'scenario', 'description': 'Support'}, 'community.group': {'kind': 'community', 'description': 'Group'}, 'rule.global': {'kind': 'rule', 'description': 'Global rule', 'rule': {'category': 'invariant', 'statement': 'Rule', 'scope': 'global', 'blocking': False, 'verification': ['review']}}}, 'edges': []}
    triples = [('entity.child', 'refines', 'component.p'), ('entity.sibling', 'refines', 'component.p'), ('scenario.support', 'refines', 'entity.child'), ('entity.child', 'member_of', 'community.group'), ('entity.child', 'member_of', 'community.group'), ('entity.peer', 'member_of', 'community.group')]
    model['edges'] = [{'id': f'edge.{i}', 'from': a, 'relation': r, 'to': b} for i, (a, r, b) in enumerate(triples)]
    path = root / 'closure.json'; path.write_text(json.dumps(model)); bundle = load_bundle(path)
    expected = {'entity.child', 'scenario.support', 'component.p', 'rule.global'}
    assert context_closure(bundle, ['entity.child']) == expected
    assert context_closure(bundle, ['component.p']) == expected | {'entity.sibling'}
    assert aggregate(validate(path, profiles=['readiness'], scope=['entity.child'])['checks'], 'readiness') == 'PASS'
    assert aggregate(validate(path, profiles=['readiness'], scope=['component.p'])['checks'], 'readiness') == 'PASS'
    assert len(bundle.model['edges']) == len(triples)
    counts['scoped_closure_readiness_parallel_multiedges'] = 5

    fixture = runpy.run_path(str(ROOT / 'tests/test_review_planning_loop6.py'))['case'].__wrapped__
    path, records, plan = fixture(root)
    records['check.shared'] = {**deepcopy(records['check.x']), 'scope': ['rule.x', 'rule.y']}
    (root / 'records.json').write_text(json.dumps({'format': 'gdd-records/0.2', 'project_id': 'review', 'records': records}))
    ids = list(records)
    required = {('rule.x', 'synthetic'), ('rule.x', 'synthetic.two'), ('rule.y', 'synthetic')}
    for mask in range(1 << len(ids)):
        selected = [ident for i, ident in enumerate(ids) if mask & (1 << i)]
        plan['integration']['checks'] = selected
        covered = {(scope, records[ident]['method']) for ident in selected for scope in records[ident]['scope']}
        report = validate(path, profiles=['readiness'], parallel_plan=plan)
        observed = next(c['result'] for c in report['checks'] if c['check_id'] == 'parallel.independence')
        assert observed == ('PASS' if required <= covered else 'UNKNOWN')
    counts['independent_parallel_required_pair_oracle'] = 16

    for form, raw in [('json', '[0]'), ('yaml', 'value: [0]')]:
        assert parse(raw) is not None
        deep = '[' * 2000 + '0' + ']' * 2000
        try:
            parse(deep if form == 'json' else 'value: ' + deep)
        except InputError as error:
            assert error.code == 'FORMAT_UNSUPPORTED'
        else:
            raise AssertionError('Parser capacity was not diagnosed')
    for raw in ['{"x":1,"x":2}', 'value: &v [0]\ncopy: *v', 'value: !custom 1']:
        try:
            parse(raw)
        except InputError as error:
            assert error.code != 'FORMAT_UNSUPPORTED'
        else:
            raise AssertionError('Forbidden feature accepted')
    counts['json_yaml_capacity_and_forbidden_controls'] = 7

summary = {'loop': 9, 'owner': 'model_review', 'result': 'CLEAN', 'independent_probe_counts': counts, 'independent_probe_total': sum(counts.values())}
Path('/tmp/gdd-loop9-model-probes.json').write_text(json.dumps(summary, indent=2) + '\n')
print(json.dumps(summary))
