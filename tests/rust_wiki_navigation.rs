use gdd::knowledge::navigation::publish_navigation;
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};

fn project(dir: &Path) -> (PathBuf, Value) {
    fs::create_dir(dir.join("wiki")).unwrap();
    fs::write(
        dir.join("wiki/team notes.md"),
        "# Review\n\n[Pricing](../gdd-graph.md#gdd-node-action.price)\n",
    )
    .unwrap();
    let model = json!({
        "format":"gdd/0.2",
        "project":{"id":"project.billing","title":"Billing <script>","goal":"Describe billing."},
        "nodes":{
            "component.billing":{"kind":"component","description":"Calculates charges."},
            "action.price":{"kind":"action","description":"Keep [unsafe](https://invalid.example/) as prose.",
                "source_refs":[{"source":"source.team","purpose":"provenance","selector":{"kind":"markdown-heading","value":"Review"}}]}
        },
        "questions":{"question.rule":{"status":"open","targets":["action.price"],"blocks":["implementation"],"statement":"Which standard applies?"}},
        "edges":[{"id":"edge.owner","from":"action.price","to":"component.billing","relation":"owned_by"}],
        "sources":{"source.team":{"role":"knowledge","title":"Team [notes]","format":"text/markdown","normative":false,"uri":"wiki/team notes.md"}}
    });
    let path = dir.join("gdd.json");
    fs::write(&path, serde_json::to_vec(&model).unwrap()).unwrap();
    (path, model)
}

#[test]
fn native_markdown_links_survive_refresh_and_bind_the_generated_project() {
    let temp = tempfile::tempdir().unwrap();
    let (model, mut data) = project(temp.path());
    let out = publish_navigation(&model).unwrap();
    let bytes = fs::read_to_string(&out).unwrap();
    for subsection in [
        "node-action.price",
        "node-component.billing",
        "edge-edge.owner",
        "question-question.rule",
    ] {
        assert!(bytes.contains(&format!("<a name=\"gdd-{subsection}\"></a>")));
    }
    assert!(bytes.contains("[Team \\[notes\\] · Review"), "{bytes}");
    assert!(bytes.contains("(<./wiki/team%20notes.md#review>)"));
    assert_eq!(
        bytes.matches("(<./wiki/team%20notes.md#review>)").count(),
        1
    );
    assert!(bytes.contains("unpinned"));
    assert!(bytes.contains("Billing &lt;script&gt;"));
    assert!(!bytes.contains("[unsafe](https"));
    let before = fs::metadata(&out).unwrap().modified().unwrap();
    assert_eq!(publish_navigation(&model).unwrap(), out);
    assert_eq!(fs::metadata(&out).unwrap().modified().unwrap(), before);
    data["project"]["title"] = json!("Updated billing");
    fs::write(&model, serde_json::to_vec(&data).unwrap()).unwrap();
    publish_navigation(&model).unwrap();
    let refreshed = fs::read_to_string(&out).unwrap();
    assert!(refreshed.contains("# Updated billing"));
    data["project"]["id"] = json!("project.other");
    fs::write(&model, serde_json::to_vec(&data).unwrap()).unwrap();
    assert!(publish_navigation(&model).is_err());
    assert_eq!(fs::read_to_string(&out).unwrap(), refreshed);
}

#[test]
fn foreign_files_and_changed_generated_text_are_preserved() {
    let temp = tempfile::tempdir().unwrap();
    let (model, _) = project(temp.path());
    let out = temp.path().join("gdd-graph.md");
    fs::write(&out, "# Human-authored\n").unwrap();
    assert!(publish_navigation(&model).is_err());
    assert_eq!(fs::read_to_string(&out).unwrap(), "# Human-authored\n");
    fs::remove_file(&out).unwrap();
    publish_navigation(&model).unwrap();
    let changed = fs::read_to_string(&out).unwrap() + "Human annotation\n";
    fs::write(&out, &changed).unwrap();
    assert!(publish_navigation(&model).is_err());
    assert_eq!(fs::read_to_string(&out).unwrap(), changed);
}

#[cfg(unix)]
#[test]
fn navigation_never_replaces_or_writes_through_a_symlink() {
    let temp = tempfile::tempdir().unwrap();
    let (model, _) = project(temp.path());
    let out = publish_navigation(&model).unwrap();
    let other = temp.path().join("other.md");
    fs::rename(&out, &other).unwrap();
    std::os::unix::fs::symlink(&other, &out).unwrap();
    let original = fs::read(&other).unwrap();
    assert!(publish_navigation(&model).is_err());
    assert!(out.is_symlink());
    assert_eq!(fs::read(&other).unwrap(), original);
}

#[test]
fn duplicate_and_complex_headings_link_to_the_page_with_the_exact_heading_label() {
    let temp = tempfile::tempdir().unwrap();
    let (model, _) = project(temp.path());
    fs::write(temp.path().join("wiki/team notes.md"), "# Review\n[Core](../gdd-graph.md#gdd-node-component.billing)\n# Review\n[Core](../gdd-graph.md#gdd-node-component.billing)\n# Whose rule?\n[Core](../gdd-graph.md#gdd-node-component.billing)\n").unwrap();
    let text = fs::read_to_string(publish_navigation(&model).unwrap()).unwrap();
    assert!(!text.contains("%20notes.md#"));
    assert!(text.contains("Review (occurrence 1)](<./wiki/team%20notes.md>)"));
    assert!(text.contains("Review (occurrence 2)](<./wiki/team%20notes.md>)"));
    assert!(text.contains("Whose rule?](<./wiki/team%20notes.md>)"));
}
