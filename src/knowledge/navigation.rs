//! Portable Markdown navigation between declared wiki pages and stable graph records.
use super::{KnowledgeIndex, field};
use crate::{
    bundle::{self, Bundle},
    input::Value,
    model::get,
};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write as _,
    fs::{self, File},
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn prose(text: &str) -> String {
    let mut out = String::new();
    for c in text
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
    {
        match c {
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '&' => out.push_str("&amp;"),
            '\\' | '[' | ']' | '*' | '_' | '`' | '!' | '#' | '~' | '|' | '$' => {
                out.push('\\');
                out.push(c);
            }
            c if !c.is_control() => out.push(c),
            _ => {}
        }
    }
    out
}

fn local_url(path: &str) -> String {
    let mut url = String::from("./");
    for byte in path.bytes() {
        if byte.is_ascii_alphanumeric() || b"/-._~".contains(&byte) {
            url.push(char::from(byte));
        } else {
            write!(url, "%{byte:02X}").unwrap();
        }
    }
    url
}

fn heading(body: &mut String, kind: &str, id: &str) -> io::Result<()> {
    if !crate::valid_id(id) {
        return Err(invalid(
            "A graph navigation anchor requires a valid GDD ID.",
        ));
    }
    writeln!(body, "\n<a name=\"gdd-{kind}-{id}\"></a>\n\n### `{id}`\n").unwrap();
    Ok(())
}

fn graph_link(kind: &str, id: &str) -> String {
    if crate::valid_id(id) {
        format!("[`{id}`](#gdd-{kind}-{id})")
    } else {
        prose(id)
    }
}

/// Portable fragments are restricted to unique headings made of ASCII words.
#[derive(Default)]
struct SectionAnchors {
    counts: BTreeMap<String, usize>,
    fragments: BTreeMap<String, String>,
}

impl SectionAnchors {
    fn from_text(text: &str) -> Self {
        let parsed = super::markdown::Markdown::parse(text);
        let mut anchors = Self::default();
        let mut candidates = BTreeMap::<String, usize>::new();
        for section in &parsed.sections {
            *anchors.counts.entry(section.heading.clone()).or_default() += 1;
            let leaf = section.heading.rsplit(" / ").next().unwrap();
            let slug: String = leaf
                .chars()
                .filter(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_'))
                .map(|c| {
                    if c == ' ' {
                        '-'
                    } else {
                        c.to_ascii_lowercase()
                    }
                })
                .collect();
            *candidates.entry(slug.clone()).or_default() += 1;
            let simple = leaf
                .split(' ')
                .all(|word| !word.is_empty() && word.bytes().all(|c| c.is_ascii_alphanumeric()));
            // Suffixes like "-1" can also be generated for duplicate headings.
            if simple
                && !slug
                    .rsplit_once('-')
                    .is_some_and(|(_, tail)| tail.bytes().all(|c| c.is_ascii_digit()))
            {
                anchors.fragments.insert(section.heading.clone(), slug);
            }
        }
        anchors.fragments.retain(|_, slug| candidates[slug] == 1);
        anchors
    }
}

type WikiReferences = BTreeMap<(String, String), BTreeMap<String, BTreeSet<String>>>;

fn references(bundle: &Bundle, index: &KnowledgeIndex) -> WikiReferences {
    let pages: BTreeMap<_, _> = index
        .pages
        .iter()
        .map(|page| {
            let asset = bundle.contract_asset(&page.id).ok();
            let anchors = asset
                .and_then(|a| std::str::from_utf8(&a.bytes).ok())
                .map(SectionAnchors::from_text)
                .unwrap_or_default();
            (
                page.id.as_str(),
                (&page.title, asset.map(|a| local_url(&a.path)), anchors),
            )
        })
        .collect();
    let mut references = WikiReferences::new();
    for link in &index.links {
        let Some((title, url, anchors)) = pages.get(link.source.as_str()) else {
            continue;
        };
        let mut label = prose(title);
        let mut destination = url.clone();
        if let Some(heading) = &link.heading {
            write!(label, " · {}", prose(heading)).unwrap();
            if let Some(n) = link.ordinal
                && (n > 1 || anchors.counts.get(heading).copied().unwrap_or(0) > 1)
            {
                write!(label, " (occurrence {n})").unwrap();
            }
            if matches!(link.status.as_str(), "pinned" | "unpinned")
                && let (Some(url), Some(fragment)) =
                    (&mut destination, anchors.fragments.get(heading))
            {
                write!(url, "#{fragment}").unwrap();
            }
        }
        let location = match destination {
            Some(url) => format!("[{label}](<{url}>)"),
            None => label,
        };
        let entry = references
            .entry((link.target_kind.clone(), link.target.clone()))
            .or_default()
            .entry(location)
            .or_default();
        entry.insert(format!(
            "{} ({})",
            prose(&link.purpose),
            prose(&link.status)
        ));
    }
    references
}

fn render(bundle: &Bundle, index: &KnowledgeIndex) -> io::Result<String> {
    if !crate::valid_id(&index.project_id) {
        return Err(invalid(
            "A graph navigation page requires a valid project ID.",
        ));
    }
    let manifest = &bundle.manifest().expect("index has manifest").value;
    let project = get(manifest, "project").unwrap_or(&Value::Null);
    let mut body = format!(
        "# {}\n\nGraph navigation for `{}`.\n",
        prose(field(project, "title").unwrap_or(&index.project_id)),
        index.project_id
    );
    let refs = references(bundle, index);
    let wiki_links = |body: &mut String, kind: &str, id: &str| {
        if let Some(entries) = refs.get(&(kind.into(), id.into())) {
            body.push('\n');
            for (location, roles) in entries {
                writeln!(
                    body,
                    "- {location} · {}",
                    roles
                        .iter()
                        .map(String::as_str)
                        .collect::<Vec<_>>()
                        .join(", ")
                )
                .unwrap();
            }
        }
    };
    body.push_str("\n## Nodes\n");
    for node in bundle.nodes() {
        heading(&mut body, "node", node.id)?;
        writeln!(
            body,
            "{} · {}",
            prose(field(node.value, "kind").unwrap_or("Node")),
            prose(field(node.value, "description").unwrap_or_default())
        )
        .unwrap();
        wiki_links(&mut body, "node", node.id);
    }
    body.push_str("\n## Edges\n");
    for edge in bundle.edges() {
        heading(&mut body, "edge", edge.id)?;
        writeln!(
            body,
            "{} → {} · {}",
            graph_link("node", field(edge.value, "from").unwrap_or_default()),
            graph_link("node", field(edge.value, "to").unwrap_or_default()),
            prose(field(edge.value, "relation").unwrap_or("relation"))
        )
        .unwrap();
        wiki_links(&mut body, "edge", edge.id);
    }
    body.push_str("\n## Questions\n");
    for question in &index.questions {
        heading(&mut body, "question", &question.id)?;
        writeln!(
            body,
            "{} · {}",
            prose(&question.status),
            prose(&question.statement)
        )
        .unwrap();
        wiki_links(&mut body, "question", &question.id);
    }
    Ok(body)
}

fn existing_hash(file: &mut File, prefix: &str) -> io::Result<String> {
    // Read just the marker before hashing; an unrelated file can be arbitrarily large.
    let suffix = " -->\n";
    let mut first = vec![0u8; prefix.len() + 64 + suffix.len()];
    let unexpected =
        || invalid("Existing gdd-graph.md is unowned, edited, or belongs to another project.");
    file.read_exact(&mut first).map_err(|_| unexpected())?;
    let expected = std::str::from_utf8(&first)
        .ok()
        .and_then(|line| line.strip_prefix(prefix))
        .and_then(|line| line.strip_suffix(suffix))
        .filter(|hash| hash.bytes().all(|c| c.is_ascii_hexdigit()))
        .ok_or_else(unexpected)?;
    let mut digest = Sha256::new();
    io::copy(file, &mut digest)?;
    if expected != format!("{:x}", digest.finalize()) {
        return Err(unexpected());
    }
    Ok(expected.into())
}

/// Publish or refresh gdd-graph.md alongside the selected model.
/// Wiki inputs are captured once; publication never writes to an authored page.
pub fn publish_navigation(model: &Path) -> io::Result<PathBuf> {
    let bundle = bundle::load_for_knowledge(model, &[]);
    let index = KnowledgeIndex::from_bundle(&bundle)?;
    let body = render(&bundle, &index)?;
    let hash = crate::sha256(body.as_bytes());
    let prefix = format!(
        "<!-- gdd-navigation/0.1 project={} sha256=",
        index.project_id
    );
    let root = bundle
        .path()
        .parent()
        .ok_or_else(|| invalid("Expected a model directory."))?;
    let dest = root.join("gdd-graph.md");
    let regular = || match fs::symlink_metadata(&dest) {
        Ok(meta) if meta.is_file() => Ok(true),
        Ok(_) => Err(invalid("gdd-graph.md must be a regular file.")),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(err) => Err(err),
    };
    let mut temp = tempfile::NamedTempFile::new_in(root)?;
    writeln!(temp, "{prefix}{hash} -->")?;
    temp.write_all(body.as_bytes())?;
    temp.as_file().sync_all()?;
    // Serialize publishers for the same model without introducing a lock file.
    // No model data is read or written while its advisory lock is held.
    let publisher = File::open(bundle.path())?;
    publisher.lock()?;
    if regular()? {
        let mut file = File::open(&dest)?;
        if existing_hash(&mut file, &prefix)? == hash {
            return Ok(dest);
        }
        if !regular()? {
            return Err(invalid("Graph navigation changed during publication."));
        }
        temp.persist(&dest)?;
    } else {
        temp.persist_noclobber(&dest)?;
    }
    Ok(dest)
}
