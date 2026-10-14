# Repository readiness: method, CLI and skill

Local delivery completed October 14, 2026, against the accepted
[repository plan](../REPOSITORY-PLAN.md). Package 0.7.0 is a local release
candidate under the maintainer-selected [MIT license](../LICENSE). GDD remains
draft 0.3 with explicit 0.2 compatibility; renderer 0.2.0 is unchanged.
No registry publication or successful remote CI run is claimed.

Maintainer update, October 14, 2026: GitHub CI was rejected as too heavy and
the workflow was removed. Verification now runs locally. References to the
initial CI configuration below describe the delivery at commit `8b65d14`;
its configuration and pin-check receipts remain historical facts, not pending
remote work. The recorded execution results are unchanged.

## Delivered entry points

- [README](../README.md) explains the method, separate assessments and tested
  quickstart. [INSTALL](../INSTALL.md) covers fresh source/wheel installation,
  standalone skill extraction and optional browser dependencies.
- `gdd init` publishes a valid 0.3 draft exclusively, with an open scope question,
  no operational mandate and no execution records. Readiness remains FAIL.
- [The GDD skill](../skills/gdd/SKILL.md) provides new-project, existing-code and
  change routes. Its required guidance and schemas are synchronized exact copies,
  and the folder can be extracted from the installed wheel independently.
- [HOST](../HOST.md) documents opt-in code snapshots, actual argv execution,
  checked-binding evidence and immutable record append. The caller establishes
  authorization, adequate coverage, performed controls and receipt authenticity.
- [Contribution guidance](../CONTRIBUTING.md), [changelog](../CHANGELOG.md),
  packaging metadata and the initial CI configuration completed maintenance
  entry points at `8b65d14`. The subsequent maintainer decision replaces that
  configuration with local verification. Code, standard and viewer versions
  remain separate.

Four bounded implementation workers used isolated source copies with disjoint
ownership. The main agent integrated and verified their artifacts. Three fresh
native Codex agents then used separately installed wheels and standalone skill
folders; they could write only their own isolated projects. They had no parent
checkout, concert-specific tools or parent answers to protocol fields. Exact
installation/skill hashes and runtime identity limits are recorded in
[trial installations](readiness/trial-installations.json).

## Observed independent journeys

| Trial | Actual observed outcome | Preserved artifacts |
| --- | --- | --- |
| New parcel-price CLI | A and B scoped readiness, plans, integration and current acceptance; fixed B assertions FAIL on A and PASS on B; actual forbidden CLI-to-tariffs import FAIL, exact repair PASS; missing source/evidence UNKNOWN | [Journey](readiness/journeys/new/JOURNEY.md), [byte index](readiness/journeys/new.index.json) |
| Existing Reservation/Graphify | Three AST nodes and five edges preserved; supported and unsupported raw exports byte-identical; explicit 0.2 proposal to accepted 0.3 intent; 10 fixed tests red then green; two actual defect controls detected; current acceptance | [Journey](readiness/journeys/existing/JOURNEY.md), [byte index](readiness/journeys/existing.index.json) |
| Fresh existing-code retrial | Updated mapping/migration guidance used without parent protocol intervention; nine final tests PASS and three isolated defect controls FAIL; current scoped acceptance with historical UNKNOWN retained | [Journey](readiness/journeys/existing-final/JOURNEY.md), [result](readiness/journeys/existing-final/RESULT.json), [byte index](readiness/journeys/existing-final.index.json) |

Input briefs, models, sources, observations, mappings, graph views, plans, run
journals, code, tests, raw commands and actual receipts are copied byte-for-byte
under `readiness/journeys/`. Their execution paths and identities remain those of
the original isolated runs; copying them is not a new execution. Temporary Python
caches are excluded. These are audit captures, not additional product features or
portable example hosts. The runnable reference remains the curated concert app.

The parcel B test file has identical red/green SHA-256
`24f2c5dbf220cb1c4efae9aca9b84ff25c129c6b382b03e5929065496bb6766c`.
Its fixed assertions cover rounding boundaries, domestic invariance and exactly
one 50-cent remote fee. The architecture checker parses actual Python imports;
the injected dependency can preserve price output while still violating the
boundary. Its missing/dynamic-input profile remains UNKNOWN.

The main agent subsequently inspected actual assertions and code and reran the
current tests, architecture checker and run status. Both original acceptances
were current; the retrial's current acceptance also remained true while its
status correctly exited 2 for unrelated historical UNKNOWN. See the separate
[original-trial rechecks](readiness/parent-journey-checks.json) and
[retrial rechecks](readiness/parent-final-existing-checks.json).

## Findings incorporated

The first existing-code trial exposed two guidance omissions: edge mappings must
retain observed endpoint direction, and the preserved adapter proposal emits
`gdd/0.2`. The existing-code route now explains omitting a containment edge before
authoring intended ownership, and explicitly migrating accepted intent to 0.3.
The fresh retrial exercised that guidance.

Further self-recovered drafting errors and discovery effort remain disclosed in
the agent reports. HOST now shows the snapshot mapping shape, the record-fragment
envelope, exact question-decision coverage, delegated policy authority and the
mixed-history output distinction. New/change routes link to it directly. The
required host reference cannot silently disappear from a generated skill. No
expected behavioral assertion was weakened to obtain PASS.

Package checks found and repaired stale runtime report-version literals and a
wheel-skill copy command that could import the checkout. Reports now use the
package version; extraction uses isolated Python and is checked even when run
from the source directory. Initial raw reports still say tool 0.6.0 although their
wheel metadata/module was 0.7.0. The installation receipt records this correction;
old execution output and the first agent's repeated label are preserved.

After the retrial had already accepted, the parent pointed out the existing
documented historical-snapshot limitation for reporting accuracy. That review
changed no assertions, model, records, code or completed workflow effects.

## Verification

| Check | Actual local result |
| --- | --- |
| Core tests, Python 3.12 | 1,469 passed |
| Core tests, Python 3.11 | 1,469 passed |
| Standard conformance, both Python versions | 58/58 expectations matched |
| Graphify conformance, both versions | 18/18 expectations matched |
| Visualization conformance, both versions | 16/16 expectations matched |
| Workflow conformance, both versions | 20/20 named cases passed |
| Design conformance, both versions | 32 named cases / 52 test instances passed |
| Offline browser interactions | 25/25; no external requests or script errors |
| Final wheel outside the checkout | 20 CLI checks, actual legacy replay, all 91 exact runtime/reference files, MIT metadata/license and standalone extraction passed |
| Fresh source installation | Locked setup, CLI/model check, skill synchronization and current asset tests passed in a new copy/environment |
| Final guidance refinements | Required-reference and host checks passed; regenerated references and final wheel reverified |
| Concert portable replay | A/B original code/spec identities reproduced, current acceptance true, five/eight browser cases |
| Original concert archive | All 197 files / 7,443,413 recovered bytes match SHA-256 and Git blob identities; recovered original replay passed before curation |

[The core receipt](readiness/core-tests.json) retains actual JUnit identities and
counts. Conformance, installation, browser, replay and wheel outputs are indexed
in [the evidence byte index](readiness/index.json). Local checks ran on macOS
arm64 with Python 3.11/3.12; the fresh browser trial used Node 25.9.0,
Playwright 1.64.1 and managed Chromium 156.0.8078.4. The initial CI configuration targeted Ubuntu 24.04,
Python 3.11/3.12 and Node 24, with explicit browser installation and uploaded
reports. Its action commit pins were checked against the upstream tags. That
configuration was removed following the maintainer's decision; no remote run
is required or planned.

The full core runs preceded the final prose/reference refinements. The affected
skill/host tests, synchronization, fresh installation and exact final wheel
checks were rerun after those refinements. Normative standard/model semantics and
viewer runtime bytes were not changed by the readiness work.

## History and practical limits

The concert reference keeps runtime code, meaningful tests, models, necessary A
fixtures, replay and concise results. Its complete original subtree at `7e27757`
is in the [recovery archive](../archive/experiments/README.md). Original model/code
and normative asset bytes remain unchanged. Portable host scripts have new hashes
and new replay evidence; they do not replace historical checker bindings.

Historical snapshots are not loaded by the current validator. A ledger containing
records from older specification revisions can retain global model UNKNOWN while
current scoped readiness and acceptance pass. In that situation snapshot/context
CLI output is a report, and global view verification can remain UNKNOWN. The
retrial preserved those results and verified reading artifacts from its installed
workplan. Separate frozen revision bundles, as in the parcel A/B trial, provide a
clear current reading path. No old target was rebound or uncertainty promoted.

The journeys demonstrate reusable planning and bounded verified development with
one tested native agent runtime and Python implementations. They do not establish
universal language/runtime coverage, automatic KISS/DRY quality, arbitrary import
analysis, producer authenticity or adequacy of every authored contract. The
standard remains a draft, and actual execution/trust remain host responsibilities.
