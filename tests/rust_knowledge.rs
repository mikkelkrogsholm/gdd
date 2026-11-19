use gdd::{bundle, knowledge, sha256};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};
use tempfile::tempdir;

const PAGE: &str = "---\ntitle: '[Ignore](gdd:node/yaml.only)'\n---\n# Billing\n## Rounding\nSum first. [Calculate](gdd:node/action.main)\n\n```md\n## In a fence\n[Ignore](gdd:node/code.only)\n```\n\n## Rounding\nHistorical variant. [Decision](gdd:question/question.scope)\n\n## Other\nUnrelated work.\n";
fn model(root: &Path, pin: Option<&str>, normative: bool, selectors: Vec<Value>) -> PathBuf {
    let mut manifest: Value =
        serde_json::from_str(include_str!("../templates/project/gdd.json")).unwrap();
    manifest["record_files"] = json!([]);
    let mut source = json!({"role":"knowledge","title":"Team guide","uri":"guide.md","format":"text/markdown","normative":normative});
    if let Some(hash) = pin {
        source["revision"] = json!({"sha256":hash});
    }
    manifest["sources"]["source.guide"] = source;
    manifest["nodes"]["component.other"] =
        json!({"kind":"component","label":"Other","maturity":"sketch"});
    manifest["nodes"]["action.main"]["source_refs"]=json!(selectors.into_iter().map(|s|json!({"source":"source.guide","purpose":if normative {"contract"} else {"provenance"},"selector":s})).collect::<Vec<_>>());
    let path = root.join("model.json");
    fs::write(&path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    path
}
#[test]
fn heading_resolution_is_bidirectional_scoped_and_frozen() {
    let temp = tempdir().unwrap();
    fs::write(temp.path().join("guide.md"), PAGE).unwrap();
    let selector = json!({"kind":"markdown-heading","value":"Billing / Rounding","ordinal":1});
    let p = model(temp.path(), None, false, vec![selector]);
    let bundle = bundle::load_for_knowledge(&p, &[]);
    fs::write(temp.path().join("guide.md"), "changed after capture").unwrap();
    let index = knowledge::KnowledgeIndex::from_bundle(&bundle).unwrap();
    let page = &index.pages[0];
    assert_eq!(
        page.sha256.as_deref(),
        Some(sha256(PAGE.as_bytes()).as_str())
    );
    assert_eq!(page.status, "unpinned");
    assert_eq!(index.links.len(), 3);
    let section = &index.links[0];
    assert_eq!(section.ordinal, Some(1));
    assert!(section.excerpt.as_ref().unwrap().contains("Sum first."));
    assert!(
        !section
            .excerpt
            .as_ref()
            .unwrap()
            .contains("Historical variant.")
    );
    let by_source = index.select(&[], Some("source.guide")).unwrap();
    assert_eq!(by_source.questions[0].id, "question.scope");
    let by_action = index.select(&["action.main".into()], None).unwrap();
    assert_eq!(by_action.links.len(), 3);
    assert!(
        by_action
            .links
            .iter()
            .any(|l| l.target == "question.scope" && l.ordinal == Some(2))
    );
    let other = index.select(&["component.other".into()], None).unwrap();
    assert!(other.pages.is_empty() && other.links.is_empty() && other.questions.is_empty());
    let mut search = by_action;
    search.search("Historical variant");
    assert_eq!(search.links.len(), 1);
    assert_eq!(search.pages.len(), 1);
    assert!(index.select(&["action.missing".into()], None).is_err());
}

#[test]
fn checksum_drift_never_supplies_excerpts_or_reverse_links() {
    let temp = tempdir().unwrap();
    fs::write(temp.path().join("guide.md"), PAGE).unwrap();
    for normative in [true, false] {
        let path = model(
            temp.path(),
            Some(&"a".repeat(64)),
            normative,
            vec![json!({"kind":"whole-document"})],
        );
        let index = knowledge::knowledge_index(&path).unwrap();
        assert_eq!(index.pages[0].status, "stale");
        assert_eq!(index.pages[0].sha256, Some(sha256(PAGE.as_bytes())));
        assert_eq!(index.links.len(), 1);
        assert_eq!(index.links[0].status, "stale");
        assert!(index.links[0].excerpt.is_none());
    }
}

#[test]
fn unresolved_selectors_links_and_external_pages_are_reported() {
    let temp = tempdir().unwrap();
    let page = format!("{PAGE}\n[Missing](gdd:node/node.missing) [Wrong](gdd:edge/action.main)\n");
    fs::write(temp.path().join("guide.md"), &page).unwrap();
    let path = model(
        temp.path(),
        Some(&sha256(page.as_bytes())),
        true,
        vec![
            json!({"kind":"markdown-heading","value":"Billing / Rounding"}),
            json!({"kind":"markdown-heading","value":"No heading"}),
            json!({"kind":"line-range","value":"L5-L6"}),
        ],
    );
    let index = knowledge::knowledge_index(&path).unwrap();
    assert_eq!(index.links[0].status, "ambiguous");
    assert_eq!(index.links[1].status, "broken");
    assert_eq!(
        index.links[2].excerpt.as_deref(),
        Some("## Rounding\nSum first. [Calculate](gdd:node/action.main)")
    );
    assert_eq!(index.links[2].status, "pinned");
    assert!(
        index
            .links
            .iter()
            .filter(|l| l.kind == "wiki-link" && l.status == "broken")
            .count()
            == 2
    );
    let mut doc: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    doc["sources"]["source.guide"]["normative"] = json!(false);
    for uri in ["https://example.org/wiki", "../outside.md"] {
        doc["sources"]["source.guide"]["uri"] = json!(uri);
        fs::write(&path, doc.to_string()).unwrap();
        let index = knowledge::knowledge_index(&path).unwrap();
        assert_eq!(index.pages[0].status, "unavailable");
        assert!(index.links.iter().all(|link| link.excerpt.is_none()));
    }
}

#[test]
fn preview_stops_on_a_character_boundary_and_preserves_the_full_digest() {
    let temp = tempdir().unwrap();
    let page = format!("Notes\n=====\n{}\n", "é".repeat(4200));
    fs::write(temp.path().join("guide.md"), &page).unwrap();
    let hash = sha256(page.as_bytes());
    let path = model(
        temp.path(),
        Some(&hash),
        true,
        vec![json!({"kind":"markdown-heading","value":"Notes"})],
    );
    let index = knowledge::knowledge_index(&path).unwrap();
    assert_eq!(index.pages[0].sha256.as_deref(), Some(hash.as_str()));
    assert!(index.links[0].truncated);
    assert_eq!(
        index.links[0].excerpt.as_ref().unwrap().chars().count(),
        4096
    );
}

#[test]
fn cited_stakeholder_revision_is_independent_from_current_note_and_page_hashes() {
    let root = tempdir().unwrap();
    fs::write(root.path().join("guide.md"), PAGE).unwrap();
    let note = root.path().join("stakeholder.md");
    fs::write(&note, "Round each item.").unwrap();
    let note_hash = sha256(b"Round each item.");
    let path = model(root.path(), Some(&sha256(PAGE.as_bytes())), true, vec![]);
    let mut doc: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    doc["sources"]["input.notes"] = json!({"role":"input","title":"Workshop notes","uri":"stakeholder.md","format":"text/markdown","normative":false});
    doc["sources"]["source.guide"]["derived_from"] = json!([
        {"source":"input.notes","revision":{"sha256":note_hash,"producer_revision":"meeting-7"}},
        {"source":"input.notes"},
        {"source":"input.missing"}
    ]);
    fs::write(&path, doc.to_string()).unwrap();
    let first = knowledge::knowledge_index(&path).unwrap();
    assert_eq!(first.pages[0].derived_from[0].status, "bytes-match");
    assert_eq!(first.pages[0].derived_from[1].status, "unverified");
    assert_eq!(first.pages[0].derived_from[2].status, "missing-source");
    fs::write(&note, "Round once.").unwrap();
    let next = knowledge::knowledge_index(&path).unwrap();
    assert_eq!(next.pages[0].status, "pinned");
    let origin = &next.pages[0].derived_from[0];
    assert_eq!(origin.status, "different-bytes");
    assert_eq!(origin.uri.as_deref(), Some("stakeholder.md"));
    assert_eq!(origin.producer_revision.as_deref(), Some("meeting-7"));
    assert_eq!(origin.sha256.as_ref(), Some(&note_hash));
    assert_eq!(origin.observed_sha256, Some(sha256(b"Round once.")));
}

#[test]
fn a_relative_link_to_the_bundle_graph_export_works_in_the_wiki_and_index() {
    let root = tempdir().unwrap();
    fs::create_dir(root.path().join("wiki")).unwrap();
    let page = "# Guide\n[Yes](../gdd-graph.md#gdd-node-action.main)\n[Question](.././gdd-graph.md#gdd-question-question.scope)\n[No](gdd-graph.md#gdd-node-lookalike) [No](../../gdd-graph.md#gdd-node-escape) [No](https://example.org/gdd-graph.md#gdd-node-external) [No](/gdd-graph.md#gdd-node-absolute)\n";
    fs::write(root.path().join("wiki/guide.md"), page).unwrap();
    let path = model(root.path(), None, false, vec![]);
    let mut doc: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    doc["sources"]["source.guide"]["uri"] = json!("wiki/guide.md");
    fs::write(&path, doc.to_string()).unwrap();
    let index = knowledge::knowledge_index(&path).unwrap();
    assert_eq!(
        index
            .links
            .iter()
            .map(|l| l.target.as_str())
            .collect::<Vec<_>>(),
        ["action.main", "question.scope"]
    );
}

#[test]
fn readiness_resolves_a_pinned_knowledge_contract_and_rejects_ambiguous_sections() {
    use gdd::validation::{Profile, validate};
    let root = tempdir().unwrap();
    let path = root.path().join("model.json");
    fs::write(root.path().join("guide.md"), PAGE).unwrap();
    let mut declaration = json!({
        "format":"gdd/0.2","project":{"id":"project.test","title":"Guide","goal":"Read a contract."},
        "nodes":{"entity.total":{"kind":"entity","description":"Payment total","fields":{},"source_refs":[
            {"source":"source.guide","purpose":"contract","selector":{"kind":"markdown-heading","value":"Billing / Rounding","ordinal":1}}
        ]}},"edges":[],
        "sources":{"source.guide":{"role":"knowledge","title":"Guide","uri":"guide.md","format":"text/markdown","normative":true,"revision":{"sha256":sha256(PAGE.as_bytes())}}}
    });
    fs::write(&path, declaration.to_string()).unwrap();
    assert_eq!(
        validate(&bundle::load(&path), Profile::Readiness).exit_code(),
        0
    );
    declaration["nodes"]["entity.total"]["source_refs"][0]["selector"]
        .as_object_mut()
        .unwrap()
        .remove("ordinal");
    fs::write(&path, declaration.to_string()).unwrap();
    let report = validate(&bundle::load(&path), Profile::Readiness);
    assert!(
        report
            .checks
            .iter()
            .flat_map(|c| &c.diagnostics)
            .any(|d| d.code == "SOURCE_UNAVAILABLE" && d.message.contains("ordinal"))
    );
}

#[test]
fn discover_an_unlinked_section_and_introduction_without_leaking_other_work_into_a_packet() {
    let root = tempdir().unwrap();
    let page = format!(
        "---\nnote: ghost metadata\n---\nIntro terminology.\n# Payments\n## Rules\n{} İ … apply the Revision **Policy** after review.\n```md\n# Ghost heading\nignore ghost code\n```\n## Unrelated\n[Calculate](gdd:node/action.main).\n",
        "Background. ".repeat(530)
    );
    fs::write(root.path().join("guide.md"), &page).unwrap();
    let model = model(root.path(), Some(&sha256(page.as_bytes())), true, vec![]);
    let bundle = bundle::load_for_knowledge(&model, &[]);
    fs::remove_file(root.path().join("guide.md")).unwrap();
    let index = knowledge::KnowledgeIndex::from_bundle(&bundle).unwrap();
    let query = "revision policy";
    let mut search = index.clone();
    search.search_bundle(&bundle, query, None).unwrap();
    assert_eq!(search.matches.len(), 1);
    let hit = &search.matches[0];
    assert!(hit.excerpt.contains("Revision Policy"));
    assert!(hit.excerpt.chars().count() <= 282);
    assert_eq!(hit.heading.as_deref(), Some("Payments / Rules"));
    assert_eq!(hit.ordinal, Some(1));
    assert_eq!(hit.sha256, sha256(page.as_bytes()));
    assert_eq!(search.pages.len(), 1);
    assert!(search.links.is_empty());
    let intro =
        knowledge::discover_sections(&bundle, "intro terminology", Some("source.guide")).unwrap();
    assert_eq!(intro.len(), 1);
    assert_eq!(intro[0].heading, None);
    assert!(
        knowledge::discover_sections(&bundle, "ghost", None)
            .unwrap()
            .is_empty()
    );
    let mut scoped = index.select(&["action.main".into()], None).unwrap();
    scoped.search_bundle(&bundle, query, None).unwrap();
    assert!(scoped.matches.is_empty() && scoped.pages.is_empty() && scoped.links.is_empty());
}

#[test]
fn whole_wiki_discovery_withholds_the_old_and_current_text_on_pin_drift() {
    let root = tempdir().unwrap();
    fs::write(root.path().join("guide.md"), "Current uncited fact.").unwrap();
    let model = model(root.path(), Some(&sha256(b"Original text.")), false, vec![]);
    let bundle = bundle::load_for_knowledge(&model, &[]);
    let hits = knowledge::discover_sections(&bundle, "uncited", None).unwrap();
    assert!(hits.is_empty());
}
