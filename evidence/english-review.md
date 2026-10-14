# English repository release 0.5.1

All current repository prose and user-facing assets are English. GDD remains
format 0.2. Package 0.5.1 and renderer 0.1.4 identify the localized release.

The review covered the entire original 497-file tracked tree: 463 text files,
33 PNGs and one starter ZIP. All 45 original Markdown documents were reviewed;
43 required translation and two were already English. Four read-only agents
prepared faithful translations and independently audited source/data, links and
binary content. Root alone modified the checkout.

| Content | Verification |
| --- | --- |
| Standard, guides, scratchpad and historical reviews | Complete English translations; links, IDs, requirement strength, code/commands and historical counts retained. |
| Models, CSVs, logs and fixtures | English human-readable values; active hashes, snapshots and planned record revisions updated. |
| Viewer | English controls, inspector, folded-community labels and `lang="en"`; derived HTML regenerated. |
| Historical PNG paths | 33 fresh English reconstructions with original/replacement hash provenance. |
| Starter ZIP | All 14 members read and confirmed English; exact original bytes retained. |
| Source and links | Independent full-tree vocabulary audit, all five viewer assets read, and 249 original local Markdown links/anchors verified. |
| Unicode fixtures and names | Proper names, IDs and intentional parser/identity payloads retain their technical purpose; English is not an ASCII restriction. |

The original historical bytes remain available at
[commit 2ced75a](https://github.com/mikkelkrogsholm/gdd/tree/2ced75a5cdd4e5521282c5e9c596da7c2dc83aac).
Old dated execution claims, versions and digests were not rebound to the English
files. Translated historical text and reconstructed screenshots are explicitly
identified in [the evidence policy](README.md),
[text provenance](english-presentation-provenance.json) and
[screenshot provenance](english-screenshot-provenance.json).

| Current check | Result | Evidence |
| --- | --- | --- |
| Full pytest suite | 1,328 PASS | [JUnit](english-pytest.xml) |
| Standard conformance | 58/58 | [JSON](english-core-conformance.json) |
| Graphify conformance | 18/18 | [JSON](english-graphify-conformance.json) |
| Viewer artifact conformance | 16/16 | [JSON](english-visualization-conformance.json) |
| Workflow conformance | 20/20 | [JSON](english-workflow-conformance.json) |
| English browser behavior | 23/23; no script errors or external requests | [JSON](english-browser-release/browser-conformance.json) |
| Installed wheel outside checkout | 18 CLI steps; 38 runtime/assets/schema files byte-identical | [JSON](english-wheel-smoke.json), [script](english-wheel-smoke.source.py) |

The already-English native-agent model, its immutable run events and application
evidence are unchanged. The Graphify raw/index/producer assets also retain their
original bytes. Reproducible checks continue to detect the deliberately failing
half-up application baseline and accept the actual integrated floor result.

[Machine-readable language and integrity audit](english-audit.json) records the
current file coverage, hashes, language exceptions and unchanged byte-bearing
assets. [Distribution verification](english-distribution.json) binds the final
source and wheel archives to the reviewed source.
