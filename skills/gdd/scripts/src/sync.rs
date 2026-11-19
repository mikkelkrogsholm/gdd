use crate::{CATALOG, COPIES, Entry, FORMAT, INDEX, digest, files, record_path};
use anyhow::{Context, Result, ensure};
use percent_encoding::percent_decode_str;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use walkdir::WalkDir;

const PRODUCT: &str = "extensions/product-0.1.json";

const CORE: &[&str] = &[
    "AGENTS.md",
    "GDD.md",
    "SPEC.md",
    "WORKFLOW.md",
    "VALIDATOR.md",
    "GRAPHIFY.md",
    "VIEWER.md",
    "HOST.md",
    "agents/gdd-lead.md",
    "agents/gdd-worker.md",
    "docs/inspection.md",
    "docs/model-diff.md",
    "docs/PERSPECTIVES.md",
    PRODUCT,
];

#[derive(Default, Deserialize, Serialize)]
struct Origin {
    repository: Option<String>,
    git_base_revision: Option<String>,
    #[serde(default)]
    revision_basis: String,
}

#[derive(Serialize)]
struct Citation {
    from: String,
    href: String,
    source_target: String,
    role: &'static str,
}

#[derive(Serialize)]
struct Index {
    format: &'static str,
    origin: Origin,
    files: Vec<Entry>,
    repository_only_links: Vec<Citation>,
}

fn previous_origin(skill: &Path) -> Result<Origin> {
    #[derive(Deserialize)]
    struct Previous {
        #[serde(default)]
        origin: Origin,
        #[serde(default)]
        files: Vec<PreviousEntry>,
    }
    #[derive(Deserialize)]
    struct PreviousEntry {
        path: String,
    }
    let path = files::output(skill, INDEX)?;
    if !path.try_exists()? {
        return Ok(Origin::default());
    }
    let previous: Previous = serde_json::from_slice(&files::read(&path)?)
        .context("Invalid previous skill asset index")?;
    let mut seen = BTreeSet::new();
    for entry in &previous.files {
        record_path(&mut seen, &entry.path)?;
    }
    Ok(previous.origin)
}

fn contained(root: &Path, path: &Path) -> Result<PathBuf> {
    let resolved = path
        .canonicalize()
        .with_context(|| path.display().to_string())?;
    ensure!(
        resolved.starts_with(root),
        "Reference escapes the source repository: {}",
        path.display()
    );
    Ok(resolved)
}

fn source_paths(root: &Path) -> Result<BTreeSet<String>> {
    let mut names: BTreeSet<_> = CORE.iter().map(|name| name.to_string()).collect();
    for directory in ["standard", "schemas"] {
        let directory = root.join(directory);
        ensure!(
            contained(root, &directory)?.is_dir(),
            "Missing source directory: {}",
            directory.display()
        );
        for entry in WalkDir::new(directory).min_depth(1) {
            let entry = entry?;
            let is_file = if entry.file_type().is_symlink() {
                contained(root, entry.path())?.is_file()
            } else {
                entry.file_type().is_file()
            };
            if is_file {
                names.insert(files::relative(root, entry.path())?);
            }
        }
    }
    Ok(names)
}

// Match the repository generator's Markdown link and URL-splitting rules.
// Keep the cited href unchanged in the index, and decode only for file lookup.
fn local_url_path(href: &str) -> Option<String> {
    let url = href
        .trim_start_matches(|c: char| c <= ' ')
        .replace(['\t', '\r', '\n'], "");
    let path = url.split(['?', '#']).next()?;
    if let Some((scheme, _)) = path.split_once(':')
        && scheme
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphabetic)
        && scheme
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"+-.".contains(&c))
    {
        return None;
    }
    let path = match path.strip_prefix("//") {
        Some(authority) => &authority[authority.find('/')?..],
        None => path,
    };
    (!path.is_empty()).then(|| percent_decode_str(path).decode_utf8_lossy().into_owned())
}

pub(super) fn local_links(text: &str) -> impl Iterator<Item = (&str, String)> {
    static LINKS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\]\(([^)]+)\)").unwrap());
    LINKS.captures_iter(text).filter_map(|capture| {
        let href = capture.get(1)?.as_str().trim().trim_matches(['<', '>']);
        Some((href, local_url_path(href)?))
    })
}

fn citations(
    root: &Path,
    source: &str,
    data: &[u8],
    names: &BTreeSet<String>,
) -> Result<Vec<Citation>> {
    let mut result = Vec::new();
    let mut seen = BTreeSet::new();
    let text = std::str::from_utf8(data)?
        .replace("\r\n", "\n")
        .replace('\r', "\n");
    for (href, path) in local_links(&text) {
        if !seen.insert(href) {
            continue;
        }
        let target = root.join(source).parent().unwrap().join(path);
        let resolved = contained(root, &target)
            .with_context(|| format!("Source reference: {source}: {href}"))?;
        ensure!(
            resolved.is_file(),
            "Missing source reference: {source}: {href}"
        );
        let relative = files::relative(root, &resolved)?;
        if !names.contains(&relative) {
            result.push(Citation {
                from: source.to_owned(),
                href: href.to_owned(),
                source_target: relative,
                role: "optional-repository-reference",
            });
        }
    }
    Ok(result)
}

fn catalog(index: &Index) -> String {
    let rows: Vec<_> = index
        .files
        .iter()
        .map(|entry| {
            let name = &entry.source;
            let copy = entry.path.strip_prefix("references/gdd/").unwrap();
            format!("| `{name}` | [{copy}](gdd/{copy}) |")
        })
        .collect();
    let targets: BTreeSet<_> = index
        .repository_only_links
        .iter()
        .map(|link| &link.source_target)
        .collect();
    let targets: Vec<_> = targets
        .iter()
        .map(|target| format!("- `{target}`"))
        .collect();
    // Substitute each template line once, even when a source name contains braces.
    let mut catalog = String::new();
    for line in include_str!("../catalog.template").lines() {
        match line {
            "{source_rows}" => catalog.push_str(&rows.join("\n")),
            "{optional_targets}" => catalog.push_str(&targets.join("\n")),
            _ => catalog.push_str(line),
        }
        catalog.push('\n');
    }
    catalog
}

fn assets(root: &Path, origin: Origin) -> Result<BTreeMap<String, Vec<u8>>> {
    let names = source_paths(root)?;
    let mut index = Index {
        format: FORMAT,
        origin,
        files: Vec::new(),
        repository_only_links: Vec::new(),
    };
    let mut assets = BTreeMap::new();
    for name in &names {
        let source = contained(root, &root.join(name))?;
        ensure!(source.is_file(), "Missing source file: {name}");
        let data = files::read(&source)?;
        if name.ends_with(".md") {
            index
                .repository_only_links
                .extend(citations(root, name, &data, &names)?);
        }
        let mut copies = vec![name.clone()];
        if name == PRODUCT {
            // Keep the path shipped in earlier standalone skills.
            copies.push(format!("gdd_validator/{name}"));
        }
        for copy in copies {
            let path = format!("{COPIES}/{copy}");
            index.files.push(Entry {
                path: path.clone(),
                source: name.to_owned(),
                sha256: digest(&data),
                bytes: data.len() as u64,
            });
            assets.insert(path, data.clone());
        }
    }
    let mut bytes = serde_json::to_vec_pretty(&index)?;
    bytes.push(b'\n');
    assets.insert(INDEX.to_owned(), bytes);
    assets.insert(CATALOG.to_owned(), catalog(&index).into_bytes());
    Ok(assets)
}

pub fn sync(
    root: &Path,
    check: bool,
    repository: Option<String>,
    git_base_revision: Option<String>,
) -> Result<Vec<String>> {
    let root = root
        .canonicalize()
        .with_context(|| root.display().to_string())?;
    let skill = files::output(&root, "skills/gdd")?;
    let previous = previous_origin(&skill)?;
    let origin = Origin {
        repository: repository.or(previous.repository),
        git_base_revision: git_base_revision.or(previous.git_base_revision),
        revision_basis: "Working tree based on the recorded Git base; per-file SHA-256 identifies copied bytes, including later edits.".into(),
    };
    let expected = assets(&root, origin)?;
    let obsolete: Vec<_> = files::generated_files(&skill)?
        .into_iter()
        .filter(|path| !expected.contains_key(path))
        .collect();
    let mut differences = Vec::new();
    if check {
        differences.extend(
            obsolete
                .iter()
                .map(|name| format!("Obsolete generated reference: {name}")),
        );
    }
    let mut changed = Vec::new();
    let mut linked = Vec::new();
    for (name, bytes) in &expected {
        // generated_files already validated every existing path under COPIES,
        // including file symlinks. Atomic replacement never writes through them.
        let path = if name.starts_with("references/gdd/") {
            skill.join(name)
        } else {
            files::output(&skill, name)?
        };
        let exists = path.try_exists()?;
        if exists && files::read(&path)? == *bytes {
            if !check && path.is_symlink() {
                linked.push((path, bytes));
            }
            continue;
        }
        if check {
            let cause = if exists {
                "Generated asset differs"
            } else {
                "Missing generated asset"
            };
            differences.push(format!("{cause}: {name}"));
        } else {
            changed.push((path, bytes));
        }
    }
    if check {
        return Ok(differences);
    }
    if !changed.is_empty() || !obsolete.is_empty() {
        // Replacing or deleting another reference may change a link's bytes.
        changed.extend(linked);
    }
    for (path, bytes) in changed {
        files::write(&path, bytes)?;
    }
    for name in obsolete {
        let path = skill.join(name);
        fs::remove_file(&path).with_context(|| path.display().to_string())?;
    }
    Ok(Vec::new())
}
