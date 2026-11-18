use super::{COPIES, INDEX, files, sync};
use anyhow::{Context, Result, ensure};
use percent_encoding::percent_decode_str;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use walkdir::WalkDir;

fn installed_skill() -> PathBuf {
    // Cargo runs tests from the active package root, including when multiple
    // copies reuse compiled test binaries from a shared target directory.
    std::env::current_dir()
        .unwrap()
        .parent()
        .unwrap()
        .to_owned()
}

#[test]
fn bundled_files_match_the_shipped_index() -> Result<()> {
    let errors = super::verify(&installed_skill())?;
    ensure!(errors.is_empty(), "{}", errors.join("\n"));
    Ok(())
}

#[derive(Deserialize, PartialOrd, Ord, Eq, PartialEq, Debug)]
struct Citation {
    from: String,
    href: String,
}

fn check_navigation(skill: &Path) -> Result<()> {
    #[derive(Deserialize)]
    struct Navigation {
        repository_only_links: BTreeSet<Citation>,
    }
    let index: Navigation = serde_json::from_slice(&files::read(&skill.join(INDEX))?)?;
    let mut missing = BTreeSet::new();
    let build_output = skill.join("scripts/target");
    for entry in WalkDir::new(skill)
        .into_iter()
        .filter_entry(|entry| entry.path() != build_output)
    {
        let entry = entry?;
        let document = entry.path();
        if !entry.file_type().is_file() || document.extension().is_none_or(|ext| ext != "md") {
            continue;
        }
        let text = fs::read_to_string(document)?
            .replace("\r\n", "\n")
            .replace('\r', "\n");
        for (href, path) in sync::local_links(&text) {
            let mut target = document.parent().unwrap().to_owned();
            for component in Path::new(&path).components() {
                match component {
                    Component::Normal(name) => target.push(name),
                    Component::CurDir => (),
                    Component::ParentDir => {
                        target.pop();
                    }
                    _ => anyhow::bail!("Absolute reference in {}: {href}", document.display()),
                }
                ensure!(
                    target.starts_with(skill),
                    "Reference escapes skill: {}: {href}",
                    document.display()
                );
                if target.try_exists()? {
                    target = target.canonicalize()?;
                    ensure!(
                        target.starts_with(skill),
                        "Reference escapes skill: {}: {href}",
                        document.display()
                    );
                }
            }
            if !target.is_file() {
                let from = files::relative(&skill.join(COPIES), document).with_context(|| {
                    format!("Broken reference in {}: {href}", document.display())
                })?;
                let citation = Citation {
                    from,
                    href: href.into(),
                };
                ensure!(
                    index.repository_only_links.contains(&citation),
                    "Unrecorded citation: {citation:?}"
                );
                missing.insert(citation);
            }
        }
    }
    ensure!(
        index.repository_only_links == missing,
        "Index contains unused optional citations"
    );
    Ok(())
}

#[test]
fn all_installed_navigation_resolves_or_has_a_recorded_source_citation() -> Result<()> {
    check_navigation(&installed_skill().canonicalize()?)
}

#[test]
fn an_unrecorded_broken_skill_link_fails_navigation() -> Result<()> {
    let dir = tempfile::tempdir()?;
    fs::create_dir(dir.path().join("references"))?;
    fs::write(dir.path().join(INDEX), r#"{"repository_only_links":[]}"#)?;
    fs::write(dir.path().join("SKILL.md"), "[Absent](absent.md)")?;
    let error = check_navigation(&dir.path().canonicalize()?).unwrap_err();
    assert!(error.to_string().contains("absent.md"));
    Ok(())
}

fn check_schema_refs(documents: &[Value]) -> Result<()> {
    let mut schemas = BTreeMap::new();
    for schema in documents {
        if let Some(id) = schema["$id"].as_str() {
            ensure!(
                schemas.insert(id, schema).is_none(),
                "Duplicate schema ID: {id}"
            );
        }
    }
    for document in documents {
        let mut pending = vec![document];
        while let Some(value) = pending.pop() {
            match value {
                Value::Object(map) => {
                    if let Some(reference) = map.get("$ref") {
                        let reference = reference.as_str().context("Expected a string $ref")?;
                        let (id, fragment) = reference.split_once('#').unwrap_or((reference, ""));
                        let target = if id.is_empty() {
                            document
                        } else {
                            schemas
                                .get(id)
                                .with_context(|| format!("Missing bundled schema: {id}"))?
                        };
                        let pointer = percent_decode_str(fragment).decode_utf8()?;
                        ensure!(
                            target.pointer(&pointer).is_some(),
                            "Missing schema pointer: {reference}"
                        );
                    }
                    pending.extend(map.values());
                }
                Value::Array(items) => pending.extend(items),
                _ => (),
            }
        }
    }
    Ok(())
}

#[test]
fn all_bundled_schema_refs_resolve_locally() -> Result<()> {
    let mut schemas = Vec::new();
    for entry in fs::read_dir(installed_skill().join(COPIES).join("schemas"))? {
        let path = entry?.path();
        if path.extension().is_some_and(|ext| ext == "json") {
            schemas.push(serde_json::from_slice(&files::read(&path)?)?);
        }
    }
    ensure!(!schemas.is_empty(), "Missing bundled schemas");
    check_schema_refs(&schemas)
}

#[test]
fn schema_refs_resolve_escaped_pointer_tokens_and_report_missing_targets() {
    let definitions = json!({"$id":"urn:example", "$defs":{"é /~": [true]}});
    let valid = json!({"$ref": "urn:example#/$defs/%C3%A9%20~1~0/0"});
    check_schema_refs(&[definitions.clone(), valid]).unwrap();
    assert!(
        check_schema_refs(&[definitions, json!({"$ref": "urn:example#/$defs/missing"})]).is_err()
    );
    assert!(check_schema_refs(&[json!({"$ref": "urn:absent"})]).is_err());
}
