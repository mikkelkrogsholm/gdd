from journey_tools import ROOT,write,gdd
import hashlib,json,sys

def sha(text):return hashlib.sha256(text.encode()).hexdigest()
BRIEF='''# Preserved task input

Build a small Python stdlib parcel-price CLI through GDD 0.3: positive integer
weight in grams; domestic tariff 100 cents per begun kg, remote tariff 150 per
begun kg; initially no handling fee. Intent boundaries: CLI calls pricing,
pricing uses tariffs; CLI directly importing tariffs is forbidden.
Human authorization covers modeling, implementation and verification within
this isolated project. Establish graph, contracts, readiness, graph visualization,
work plan, actual checks/evidence and acceptance with the installed workflow.
Then change requirement to add flat 50 cents handling fee on remote quotes.
Keep meaningful fixed B assertions: execute red against A and green against B.
Deliberately inject a real architecture violation into code, demonstrate FAIL,
repair and demonstrate PASS; absence of code/check evidence must remain UNKNOWN.
Preserve commands, stdout and genuine execution receipts in project/evidence
and write an English JOURNEY.md with outcomes and gaps/interventions.
'''
CONTRACT='''# Accepted parcel contract A

`python -m parcel.cli WEIGHT ZONE` accepts a positive integer weight in grams
and zone `domestic` or `remote`. It prints one integer cent amount followed by a
newline on stdout, then exits 0. Weight is a decimal integer string; whitespace,
signs and leading zeros are accepted when Python int parses a positive value.
Invalid/missing arguments exit 2 with an English error on stderr and no quote.
No upper weight limit is imposed. Quotes do not mutate state.

Begun kilograms are (weight_grams + 999) // 1000. Domestic quotes multiply this
by 100 cents; remote quotes multiply by 150 cents. There is no handling fee.
The pricing API rejects bool, non-integer or nonpositive weight with ValueError,
and an unknown zone with ValueError. The tariff API rejects an unknown zone.

CLI owns argument parsing and printing and calls pricing.quote. Pricing owns
validation and weight rounding and calls tariffs.rate. Tariffs owns zone rates.
CLI must not directly import tariffs. These three components need no plugins,
network, persistence, auth, shared mutable state, or background work.
'''

def source(title,uri,text,normative=False):return dict(role='input',title=title,uri=uri,format='text/markdown',normative=normative,revision={'sha256':sha(text)})

def make_model(phase='a'):
    folder='gdd-'+phase
    write(folder+'/sources/brief.md',BRIEF)
    contract=CONTRACT if phase=='a' else CONTRACT.replace('contract A','contract B').replace('There is no handling fee.','Remote quotes add a flat 50 cents handling fee once per quote; domestic quotes add no fee.')
    write(folder+'/sources/contract.md',contract)
    refs=[{'source':'source.contract','purpose':'contract','selector':{'kind':'whole-document'}},{'source':'source.brief','purpose':'provenance'}]
    nodes={};edges=[]
    def node(id,kind,description,**kw): nodes[id]=dict(kind=kind,description=description,maturity='specified',source_refs=refs,**kw)
    def edge(frm,to,relation):edges.append(dict(id=f'edge.e{len(edges)+1}',**{'from':frm,'to':to,'relation':relation}))
    for cid,aid,responsibility,rationale,excludes in [
      ('component.cli','action.cli','Parse process arguments, call pricing, and print a quote or error.','Process input/output changes independently of the price formula.',['Rounding weights','Looking up tariffs','Calculating handling fees']),
      ('component.pricing','action.quote','Validate inputs and calculate the complete quote in integer cents.','The domain formula is reusable outside process argument parsing.',['Parsing process arguments','Printing output','Owning tariff values']),
      ('component.tariffs','action.rate','Provide the domestic and remote per-kilogram rates.','The table of rates has one owner and a public lookup contract.',['Parsing user input','Weight rounding','Handling-fee policy'])]:
        node(cid,'component',responsibility,boundary=dict(responsibility=responsibility,rationale=rationale,excludes=excludes),interfaces=[aid])
        edge(aid,cid,'owned_by')
    def risks(boundary_scenarios):
      return {'authorization':dict(relevant=False,rationale='Public local price calculation needs no identity or privileged effects.',scenarios=[]),'idempotency':dict(relevant=False,rationale='Calculation has no domain mutation; repeated quotes are independent.',scenarios=[]),'boundaries':dict(relevant=True,rationale='Validate positive integer weight and allowed zone; enforce exact kilogram boundaries.',scenarios=boundary_scenarios),'concurrency':dict(relevant=False,rationale='No shared mutable state, I/O transaction, or persisted quote exists.',scenarios=[]),'atomicity':dict(relevant=False,rationale='No multi-resource mutation or transaction is performed.',scenarios=[])}
    def action_contract(inp,out,errors,post,scenarios,error_cases,effects=[]):
      return dict(input=inp,context={},output=out,errors=errors,preconditions=[],postconditions=post,external_effects=effects,effects_complete=True,dependencies_complete=True,risks=risks(scenarios),error_cases=error_cases)
    node('action.cli','action','Run a command-line parcel quote.',contract=action_contract({'argv':{'type':'array','items':{'type':'string'}}},{'type':'integer'}, {},['On valid arguments print the quote and a newline and return exit code 0.','Invalid/missing arguments print no quote and return 2 with an English stderr message.'],['scenario.cli_invalid','scenario.weights'],{},['Write the quote to stdout or argument errors to stderr.']),behavior='Parse WEIGHT as int and ZONE as domestic or remote. Reject a nonpositive weight. Call action.quote once, print the integer cents, and return 0. For invalid arguments return 2 after an English stderr error.')
    formula='begun_kg * rate' if phase=='a' else 'begun_kg * rate + (50 if zone == remote else 0)'
    node('action.quote','action','Calculate the complete parcel quote.',contract=action_contract({'weight_grams':{'type':'integer','minimum':1},'zone':{'type':'enum','values':['domestic','remote']}},{'type':'integer','minimum':100},{'invalid_weight':'Raise ValueError for bool, noninteger, or nonpositive weight without a quote or effects.','invalid_zone':'Raise ValueError for an unknown zone without a quote or effects.'},['Return '+formula+' in integer cents.','No state is mutated.'],['scenario.invalid_weight','scenario.invalid_zone','scenario.weights'],{'invalid_weight':['scenario.invalid_weight'],'invalid_zone':['scenario.invalid_zone']}),behavior='Reject bool/non-integer/nonpositive weight with ValueError. Get action.rate for the zone. Compute begun_kg=(weight_grams+999)//1000. Return '+formula+'.')
    node('action.rate','action','Look up the per-begun-kilogram rate.',contract=action_contract({'zone':{'type':'enum','values':['domestic','remote']}},{'type':'integer'},{'invalid_zone':'Raise ValueError for an unknown zone without effects.'},['Return 100 for domestic and 150 for remote.'],['scenario.invalid_zone'],{'invalid_zone':['scenario.invalid_zone']}),behavior='Return 100 for domestic, 150 for remote, otherwise raise ValueError.')
    edge('action.cli','action.quote','calls');edge('action.quote','action.rate','calls');edge('component.cli','component.pricing','imports');edge('component.pricing','component.tariffs','imports')
    node('rule.price','rule','Preserve exact quoted amounts and rejection behavior.',rule=dict(category='behavior',statement='Quotes follow the accepted contract, begun-kilogram rounding, input rejection, and CLI exit/output guarantees.',scope='global',blocking=True,verification=['behavior.unittest']))
    node('rule.no_cli_tariffs','rule','Require pricing to mediate access to tariffs.',rule=dict(category='architecture',statement='CLI may import pricing; pricing imports tariffs. Direct CLI imports of tariffs are forbidden.',scope='targeted',blocking=True,verification=['architecture.python_ast'],constraint=dict(relation='imports',**{'from':'component.cli','to':'component.tariffs'},reach='direct',policy='forbid')))
    edge('rule.no_cli_tariffs','component.cli','constrains')
    scenarios=[('scenario.domestic','normal','Positive domestic parcel weighing 1 gram.','Quote from API and CLI.','Return/print 100 cents and exit 0.', ['action.quote','action.cli','action.rate']),('scenario.remote','normal','Positive remote parcel weighing 1 gram.','Quote from API and CLI.',f'Return/print {150 if phase=="a" else 200} cents and exit 0.', ['action.quote','action.cli','action.rate']),('scenario.weights','boundary','Weights are 1, 999, 1000, 1001, 2000, 2001 and a large positive integer.','Quote each zone at kilogram boundaries.','Only a begun kilogram changes the base price; '+('no fee is added.' if phase=='a' else 'remote handling is exactly 50 cents once.'),['action.quote','action.cli']),('scenario.invalid_weight','negative','Weight is 0, -1, 1.5, a string, None or bool.','Call quote.','Raise ValueError without effects.', ['action.quote']),('scenario.invalid_zone','negative','Zone is unknown or blank.','Call quote or rate.','Raise ValueError without effects.', ['action.quote','action.rate']),('scenario.cli_invalid','negative','Weight is malformed, nonpositive, absent, or the zone is unknown.','Run CLI.','Exit 2 with English stderr and no stdout quote.', ['action.cli']),('scenario.architecture','architecture','Actual CLI source contains a direct tariffs import.','Run the concrete Python AST checker.','Report FAIL and the forbidden relationship; after repair report PASS.', ['action.cli'])]
    for sid,category,given,when,then,actions in scenarios:
        node(sid,'scenario',then,scenario=dict(category=category,given=given,when=when,then=then))
        for aid in actions:edge(sid,aid,'exercises')
        edge(sid,'rule.no_cli_tariffs' if category=='architecture' else 'rule.price','verifies')
    mandate='The delegated independent test authorizes modeling, implementation and verification of this isolated parcel CLI, and explicitly requests its technical acceptance through the installed workflow; no deployment or external side effects are included.'
    m=dict(format='gdd/0.3',project=dict(id='project.parcel',title='Parcel price CLI '+phase.upper(),goal='Calculate a correct parcel quote from positive integer grams and zone.',scope='Three stdlib Python modules and fixed behavioral/architecture checks.',assumptions=['The exact CLI spelling and English errors are agent-resolved implementation choices within the brief.'],workflow=dict(mode='autonomous',mandate=mandate,gates={k:dict(requirement='standing',authorities=['actor.independent'],scope=['action.cli','action.quote','action.rate']) for k in ['model_changes','implementation','acceptance']})),nodes=nodes,edges=edges,sources={'source.brief':source('Original independent task','sources/brief.md',BRIEF),'source.contract':source('Accepted contract '+phase.upper(),'sources/contract.md',contract,True)},questions={},record_files=['records.json'])
    if phase=='b':
        change='# Accepted requirement change\n\nAdd a flat 50 cents handling fee to remote quotes. Domestic quotes and all input\nvalidation and architecture requirements remain unchanged. This change was\nexplicitly requested as part of the original independent test.\n'
        write(folder+'/sources/change.md',change);m['sources']['source.change']=source('Remote handling fee','sources/change.md',change)
        m['nodes']['action.quote']['source_refs'].append({'source':'source.change','purpose':'provenance'})
    write(folder+'/gdd.json',m)
    write(folder+'/records.json',dict(format='gdd-records/0.2',project_id='project.parcel',records={}))
    write(folder+'/SOURCES.md','# Source distinctions\n\nThe brief is preserved input. The local contract is accepted intent within the\nrequested task. CLI syntax, bool rejection, and error wording are explicitly\nagent-resolved implementation details; they do not weaken price requirements.\nNo external evidence or unresolved material questions were used.\n')
    write(folder+'/knowledge.md','# Project knowledge\n\nThe brief establishes rates and import boundaries. The graph is accepted intent,\nnot a claim about source imports. Runtime stdout and AST analysis will establish\nimplementation behavior only after actual executions. No code observations exist\nwhen this model is first authored.\n')
    return m

if __name__=='__main__':
    phase=sys.argv[1] if len(sys.argv)>1 else 'a'
    make_model(phase)
    gdd(phase+'-model','validate',f'gdd-{phase}/gdd.json','--profile','model','--output',f'evidence/{phase}-model.json')
    gdd(phase+'-readiness','validate',f'gdd-{phase}/gdd.json','--profile','readiness','--scope','action.cli','--scope','action.quote','--scope','action.rate','--output',f'evidence/{phase}-readiness.json')
    gdd(phase+'-snapshot','snapshot',f'gdd-{phase}/gdd.json','--output',f'evidence/{phase}-snapshot.json')
    gdd(phase+'-context','context',f'gdd-{phase}/gdd.json','--scope','action.cli','--scope','action.quote','--scope','action.rate','--output',f'evidence/{phase}-context.json')
    gdd(phase+'-view','view',f'gdd-{phase}/gdd.json','--output',f'evidence/{phase}-graph.html')
    gdd(phase+'-view-check','validate',f'gdd-{phase}/gdd.json','--profile','visualization','--view',f'evidence/{phase}-graph.html','--output',f'evidence/{phase}-view-check.json')
    gdd(phase+'-boundaries','boundaries',f'gdd-{phase}/gdd.json','--output',f'evidence/{phase}-boundaries.json')
