from journey_tools import ROOT,write,run
from gdd_validator.host import append_records
import json,sys
base=json.loads((ROOT/'evidence/a-snapshot.json').read_text())['revision']
result=json.loads((ROOT/'evidence/b-snapshot.json').read_text())['revision']
a=json.loads((ROOT/'gdd-a/gdd.json').read_text());b=json.loads((ROOT/'gdd-b/gdd.json').read_text())
changed=[id for id in a['nodes'] if a['nodes'][id]!=b['nodes'][id]]
write('evidence/b-model-change.json',dict(base_spec_revision=base,result_spec_revision=result,changed_ids=changed+['source.contract'],added_ids=['source.change'],deleted_ids=[],rationale='The requested remote-only flat fee belongs in pricing.quote; tariff rates and CLI responsibilities stay fixed.',sources=['source.change','source.brief'],affected_contracts=['action.quote'],affected_scenarios=['scenario.remote','scenario.weights'],implementation_resources=['src/parcel/pricing.py'],fixed_verification_resource='tests/test_b.py'))
decision=dict(kind='decision',topic='model_change',scope=['action.quote','scenario.remote','scenario.weights'],spec_revision=result,policy_revision='policy.independent-task.v1',actor={'id':'actor.independent','kind':'agent'},outcome='accepted',rationale=f'Original task explicitly requests this remote handling-fee requirement. Base {base}; changed action.quote, scenario.remote, scenario.weights and source.contract; add source.change. Keep all fixed B expectations and both architecture boundaries. Full change description: evidence/b-model-change.json.')
append_records(ROOT/'gdd-b/records.json',{'records':{'decision.remote-fee.b':decision}})
run('prepare-work-b',[sys.executable,'-B','prepare_work.py','b'])
run('dispatch-b',[sys.executable,'-B','workflow_event.py','b','dispatch','dispatch'])
