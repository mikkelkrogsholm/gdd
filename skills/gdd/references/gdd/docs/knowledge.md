# Develop with a shared project wiki

The project wiki is where people and agents can look up stakeholder requests, domain vocabulary, engineering standards and the reasons for a decision. The graph names the behavior the team has agreed to build. A source reference connects the two at a specific section. Start with one page and one action.

## Start or adopt a wiki

For a new project, create a draft with a linked brief and a real scope question. Open the model in the native viewer and select its first action to see the brief, or open `project/wiki/project.md` and follow its links in an ordinary Markdown reader.

```sh
gdd init project --wiki --project-id project.example --title "Example project"
gdd wiki project/gdd.json --scope action.main --format text
gdd-viewer project/gdd.json
```

For an existing system, choose one user journey. Capture an observation about today's behavior and a stakeholder's request as separate inputs. Register the wiki page that explains them, identify who can settle disagreements, and link a graph question to the affected action. A useful first test exercises a case where the existing and proposed rules produce different results.

Teams using OpenWiki can continue to maintain their pages there. Register a reviewed Markdown page under the model directory, for example `openwiki/domain/invoices.md`; use a versioned local copy if the team's wiki lives elsewhere. Preserve the page's provenance and original revision. OpenWiki's repository mode researches code and tests; its standalone personal mode can ingest other sources. Decide which material to share with the team before bringing it into the project. The Rust tools operate on the declared local Markdown and can run offline. [OpenWiki documentation](https://github.com/langchain-ai/openwiki#connect-your-sources)

## Link a rule, a request and a question

The [invoice model](../examples/knowledge/model.gdd.json) has a published calculation rule and a proposed change to the display. Two lines of 15 tenths sum to 3 whole minor units; rounding each line first would produce 4. The current action applies the first rule. The stakeholder has asked about line display, and the consequence for the amount charged remains an open question. Follow the links from the [standard](../examples/knowledge/wiki/rounding.md) or the [workshop summary](../examples/knowledge/wiki/requests.md).

1. Give each maintained page a stable source ID with `role: knowledge`. Use `format: text/markdown`, `markdown` or `okf/0.2` as appropriate. The URI is relative to the model directory. Register the original input separately. For a synthesis, `derived_from` records the source ID and the revision actually used.
2. Add a `source_refs` entry to the affected graph record. Use `purpose: provenance` for observations, proposals and their rationale. For an adopted standard, set `purpose: contract`, mark the page normative, and pin the SHA-256 of its exact bytes. A source hash identifies text; the team's decision determines which behavior is intended.
3. Select an exact nested heading, for example `{"kind":"markdown-heading","value":"Invoice rounding / Calculation"}`. If that full path occurs more than once, add a 1-based `ordinal`. Narrow selectors keep unrelated policies out of an agent's brief.
4. From a wiki page, point back to a graph node, edge, or question. For example, use `../gdd-graph.md#gdd-node-action.total` as the destination of an ordinary Markdown link to open the invoice action. In the GDD inspector, a link with the destination `gdd:question/question.rounding` works without a generated page. Both forms are navigational provenance. The graph's reference carries any contract obligation.

Generate or refresh the browsable graph page alongside the model. An existing hand-written file is protected. Preserve project-owned prose in the source wiki pages.

```sh
gdd wiki examples/knowledge/model.gdd.json --publish-navigation
gdd wiki examples/knowledge/model.gdd.json --source source.standard --format text
gdd wiki examples/knowledge/model.gdd.json --scope action.total --search rounding --format text
gdd wiki examples/knowledge/model.gdd.json --check
```

The full JSON report lists page revisions, links, excerpts and questions. Without a graph scope, search also finds matching sections of registered pages that have no graph link yet. With a scope, search stays inside its linked material. For sources outside the model, search the team's wider wiki and register the relevant page. With `--check`, broken links, changed page pins and unpinned contract links exit 1; ambiguous, unavailable or unsupported material exits 2. An advisory page can be unpinned. An unresolved project question stays visible with its declared blocking activities.

## How an agent uses the knowledge

```sh
gdd workflow plan examples/knowledge/model.gdd.json \
  --request examples/knowledge/request.json --output invoice-plan.json
```

A wiki-aware workplan captures the relevant knowledge in each package that uses it. The package includes linked source IDs, declared and observed page revisions, relevant questions and pointers to section previews. Look up each link's `excerpt_ref` in `package.knowledge.excerpts`. Reciprocal links share one preview. Previews are limited to 4,096 characters. For longer rules, use the page URI, captured digest and selector to read the complete original before implementing.

Before asking a stakeholder, check the package, the cited standard and any existing decision. If the sources conflict, record one question against the affected nodes, identify which activity it blocks, and bring it to the person who owns that choice. After resolution, update the decision record, the wiki rationale, the graph contract and a distinguishing scenario as appropriate. Keep unrelated graph areas at their existing level of detail.

When a page changes, inspect its backlinks with `--source` to identify affected work. Review a changed normative rule with its owner and update the pin deliberately. Verify the plan against the current inputs before dispatch; a relevant change requires a fresh plan. The earlier run retains its captured context. Only executed checks against the implementation provide behavioral evidence.

To share a saved graph with its wiki excerpts, export the two captures together. A recipient can open them offline. The report contains the captured text, so select a scope suitable for that recipient.

```sh
gdd view examples/knowledge/model.gdd.json --output invoice.gddview
gdd wiki examples/knowledge/model.gdd.json --scope action.total --output invoice.knowledge.json
gdd-viewer invoice.gddview --wiki invoice.knowledge.json
```
