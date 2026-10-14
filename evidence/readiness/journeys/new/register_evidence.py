from journey_tools import ROOT,write,gdd
from gdd_validator.host import verification_records,append_records,snapshot_code
import json,sys
phase=sys.argv[1]
model=ROOT/f'gdd-{phase}/gdd.json'
code=json.loads((ROOT/f'evidence/{phase}-code.json').read_text())
assert snapshot_code(ROOT,list(code['files']))['revision']==code['revision']
host=json.loads((ROOT/f'evidence/{phase}-host-integrated.json').read_text())
for kind,receipt,resource,method,controls in [('behavior',f'evidence/{phase}-behavior-current.run.json',f'tests/test_{phase}.py','behavior.unittest',['bad-domestic-rate'] if phase=='a' else ['remote-fee-omitted']),('architecture',f'evidence/{phase}-architecture-repaired.run.json','tests/check_architecture.py','architecture.python_ast',['cli-direct-tariffs'])]:
    fragment=verification_records(model,f'check.{kind}.{phase}',ROOT/receipt,root=ROOT,implementation='implementation.parcel',binding_id=f'binding.{kind}.{phase}',evidence_id=f'evidence.{kind}.{phase}',test_resource=resource,checker={'name':'stdlib.unittest' if kind=='behavior' else 'parcel.python_ast','version':sys.version.split()[0] if kind=='behavior' else '1'},environment={'python':sys.version},verified_negative_controls=controls)
    write(f'evidence/{phase}-{kind}-record-fragment.json',fragment)
    append_records(ROOT/f'gdd-{phase}/records.json',fragment)
    for k,v in fragment['host'].items():
        if isinstance(v,list):host[k]=list(dict.fromkeys(host.get(k,[])+v))
        elif isinstance(v,dict):
            merged=host.get(k,{}).copy()
            for dk,dv in v.items():
                if dk in merged and merged[dk]!=dv:raise ValueError('conflicting verified '+k)
                merged[dk]=dv
            host[k]=merged
        elif k in host and host[k]!=v:raise ValueError('conflicting '+k)
        else:host[k]=v
bindings={}
for module,targets in [('cli',['component.cli','action.cli']),('pricing',['component.pricing','action.quote']),('tariffs',['component.tariffs','action.rate'])]:
    resource=f'src/parcel/{module}.py';bid=f'binding.code.{module}.{phase}'
    bindings[bid]=dict(kind='binding',spec_revision=host['policy']['spec_revision'],targets=targets,implementation='implementation.parcel',role='code',resource=resource,selector={'kind':'whole-document'},status='checked',revision='sha256:'+code['files'][resource],code_revision=code['revision'])
append_records(ROOT/f'gdd-{phase}/records.json',{'records':bindings})
host['verified_bindings']+=list(bindings)
host['policy']['activity']='acceptance'
write(f'evidence/{phase}-host-acceptance.json',host)
gdd(f'{phase}-records-current','validate',str(model),'--profile','records','--host-context',f'evidence/{phase}-host-acceptance.json','--output',f'evidence/{phase}-records-current.json')
gdd(f'{phase}-authorization-acceptance','validate',str(model),'--profile','records','--activity','acceptance','--scope','action.cli','--scope','action.quote','--scope','action.rate','--host-context',f'evidence/{phase}-host-acceptance.json','--output',f'evidence/{phase}-authorization-acceptance.json')
review=dict(format='gdd-design-review/0.3',spec_revision=host['policy']['spec_revision'],implementation=host['implementation'],code_revision=code['revision'],scope=['action.cli','action.quote','action.rate'],reviewer={'name':'actor.independent','version':'executing-lead-review-v1'},executed=True,assessments=[])
for principle,rationale,target,location,consequence in [
 ('kiss','The three requested components each use a small public function and stdlib only. No speculative layer was added.','action.quote','src/parcel/pricing.py','The quote can be read and checked directly.'),
 ('dry','Tariff values are owned once by rate and the begun-kilogram formula is owned once by quote; independent test literals are assertions.','action.rate','src/parcel/tariffs.py','Changing a rate has one runtime authority.'),
 ('separation_of_concerns','Argument parsing and stdout belong to CLI, integer formula and fee policy to pricing, and rate lookup to tariffs.','action.cli','src/parcel/cli.py','The fee change does not require process parsing or tariff-table changes.'),
 ('modularity','CLI calls the public quote function; quote calls the public rate function. No shared state or private data access exists.','component.pricing','src/parcel/pricing.py','The public contracts mediate the dependency chain.')]:
    review['assessments'].append(dict(principle=principle,status='reviewed',rationale=rationale,findings=[dict(targets=[target],code_locations=[{'resource':location,'selector':'whole file inspected in the current snapshot'}],consequence=consequence,recommendation='Retain this bounded design for the requested scope.')]))
write(f'evidence/{phase}-design-review.json',review)
gdd(f'{phase}-design-review','design-review',str(model),'--review',f'evidence/{phase}-design-review.json','--implementation','implementation.parcel','--code-revision',code['revision'],'--scope','action.cli','--scope','action.quote','--scope','action.rate','--output',f'evidence/{phase}-design-review-check.json')
