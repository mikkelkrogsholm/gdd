//! Bidirectional links between a model and its declared project knowledge.
mod discovery;
pub use discovery::{KnowledgeHit, discover_sections};
pub(crate) mod markdown;
pub mod navigation;

use crate::{
    bundle::{self, Bundle},
    input::Value,
    model::{get, items, strings, text},
};
use markdown::{Markdown, excerpt};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    io,
    path::Path,
    sync::Arc,
};

pub const FORMAT: &str = "gdd-knowledge/0.1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeIndex {
    pub format: String,
    pub project_id: String,
    pub spec_revision: Option<String>,
    pub pages: Vec<KnowledgePage>,
    pub links: Vec<KnowledgeLink>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub matches: Vec<KnowledgeHit>,
    #[serde(skip)]
    allow_discovery: bool,
    pub questions: Vec<KnowledgeQuestion>,
    // Query-only identities include valid graph records with no knowledge links.
    #[serde(skip)]
    identities: Arc<BTreeSet<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgePage {
    pub id: String,
    pub title: String,
    pub uri: String,
    pub normative: bool,
    #[serde(default)]
    pub derived_from: Vec<KnowledgeOrigin>,
    pub declared_sha256: Option<String>,
    pub sha256: Option<String>,
    pub status: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeOrigin {
    pub source: String,
    pub uri: Option<String>,
    pub sha256: Option<String>,
    pub producer_revision: Option<String>,
    pub observed_sha256: Option<String>,
    pub status: String,
}

impl KnowledgeOrigin {
    fn capture(origin: &Value, bundle: &Bundle) -> Self {
        let source = field(origin, "source").unwrap_or_default();
        let declaration = bundle
            .manifest()
            .and_then(|m| get(&m.value, "sources"))
            .and_then(|s| get(s, source));
        let sha256 = get(origin, "revision").and_then(|r| field(r, "sha256"));
        let observed = bundle
            .contract_asset(source)
            .ok()
            .map(|asset| asset.sha256.as_str())
            .or_else(|| bundle.source_mismatch(source));
        let status = match (declaration, sha256, observed) {
            (None, _, _) => "missing-source",
            (_, Some(expected), Some(actual)) if expected == actual => "bytes-match",
            (_, Some(_), Some(_)) => "different-bytes",
            _ => "unverified",
        };
        Self {
            source: source.into(),
            uri: declaration.and_then(|d| field(d, "uri")).map(str::to_owned),
            sha256: sha256.map(str::to_owned),
            producer_revision: get(origin, "revision")
                .and_then(|r| field(r, "producer_revision"))
                .map(str::to_owned),
            observed_sha256: observed.map(str::to_owned),
            status: status.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeLink {
    pub source: String,
    pub target: String,
    pub target_kind: String,
    pub kind: String,
    pub purpose: String,
    pub heading: Option<String>,
    pub ordinal: Option<usize>,
    pub selector: String,
    pub selector_value: Option<String>,
    pub excerpt: Option<String>,
    pub truncated: bool,
    pub status: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeQuestion {
    pub id: String,
    pub statement: String,
    pub targets: Vec<String>,
    pub blocks: Vec<String>,
    pub status: String,
    pub resolution: Option<String>,
}

fn field<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    get(v, key).and_then(text)
}
fn entries<'a>(v: &'a Value, key: &str) -> impl Iterator<Item = (&'a String, &'a Value)> {
    match get(v, key) {
        Some(Value::Object(o)) => Some(o),
        _ => None,
    }
    .into_iter()
    .flatten()
}

pub(crate) fn is_wiki_source(source: &Value) -> bool {
    field(source, "role") == Some("knowledge")
        && matches!(
            field(source, "format"),
            Some("markdown" | "text/markdown" | "okf/0.2")
        )
}

/// Load only declared local pages. Each index reflects a single frozen bundle.
pub fn knowledge_index(path: &Path) -> io::Result<KnowledgeIndex> {
    KnowledgeIndex::from_bundle(&bundle::load_for_knowledge(path, &[]))
}

impl KnowledgePage {
    fn capture<'a>(id: &str, source: &Value, bundle: &'a Bundle) -> (Self, Option<Markdown<'a>>) {
        let mut page = Self {
            id: id.into(),
            title: field(source, "title").unwrap_or(id).into(),
            uri: field(source, "uri").unwrap_or_default().into(),
            derived_from: items(source, "derived_from")
                .iter()
                .map(|o| KnowledgeOrigin::capture(o, bundle))
                .collect(),
            normative: get(source, "normative") == Some(&Value::Boolean(true)),
            declared_sha256: get(source, "revision")
                .and_then(|r| field(r, "sha256"))
                .map(str::to_owned),
            sha256: None,
            status: "unavailable".into(),
            detail: String::new(),
        };
        if let Some(actual) = bundle.source_mismatch(id) {
            page.sha256 = Some(actual.into());
            page.status = "stale".into();
            page.detail = "The page bytes differ from its declared SHA-256.".into();
            return (page, None);
        }
        let asset = match bundle.contract_asset(id) {
            Ok(a) => a,
            Err(detail) => {
                page.detail = detail.into();
                return (page, None);
            }
        };
        page.sha256 = Some(asset.sha256.clone());
        if let Some(pin) = &page.declared_sha256 {
            if pin != &asset.sha256 {
                page.status = "stale".into();
                page.detail = "The page bytes differ from its declared SHA-256.".into();
                return (page, None);
            }
            page.status = "pinned".into();
            page.detail = "The captured page matches the declared SHA-256.".into();
        } else {
            page.status = "unpinned".into();
            page.detail = "The page was captured without a declared SHA-256.".into();
        }
        match std::str::from_utf8(&asset.bytes) {
            Ok(s) => {
                let markdown = Markdown::parse_page(s, Some(&page.uri));
                (page, Some(markdown))
            }
            Err(_) => {
                page.status = "unsupported".into();
                page.detail = "The Markdown source must be UTF-8.".into();
                (page, None)
            }
        }
    }
}

impl KnowledgeLink {
    fn new(
        page: &KnowledgePage,
        target: &str,
        target_kind: &str,
        kind: &str,
        purpose: &str,
    ) -> Self {
        Self {
            source: page.id.clone(),
            target: target.into(),
            target_kind: target_kind.into(),
            kind: kind.into(),
            purpose: purpose.into(),
            heading: None,
            ordinal: None,
            selector: "whole-document".into(),
            selector_value: None,
            excerpt: None,
            truncated: false,
            status: page.status.clone(),
            detail: page.detail.clone(),
        }
    }

    fn problem(&mut self, status: &str, detail: &str) {
        self.status = status.into();
        self.detail = detail.into();
    }
    fn preview(&mut self, s: &str) {
        let (bytes, truncated) = excerpt(s);
        self.excerpt = Some(bytes);
        self.truncated = truncated;
    }

    fn from_reference(
        page: &KnowledgePage,
        doc: Option<&Markdown<'_>>,
        target: &str,
        kind: &str,
        reference: &Value,
    ) -> Self {
        let purpose = field(reference, "purpose").unwrap_or("provenance");
        let mut link = Self::new(page, target, kind, "graph-reference", purpose);
        if let Some(selector) = get(reference, "selector") {
            link.selector = field(selector, "kind").unwrap_or("").into();
            link.selector_value = field(selector, "value").map(str::to_owned);
            if link.selector == "markdown-heading" {
                link.heading = link.selector_value.clone();
                if let Some(raw) = get(selector, "ordinal") {
                    let ordinal = match raw {
                        Value::Number(n) => n.parse::<usize>().ok().filter(|n| *n > 0),
                        _ => None,
                    };
                    if ordinal.is_none() {
                        link.problem("broken", "A heading ordinal must be a positive integer.");
                        return link;
                    }
                    link.ordinal = ordinal;
                }
            }
        }
        let Some(doc) = doc else { return link };
        if purpose == "contract" && !page.normative {
            link.problem(
                "unpinned",
                "A contract needs a normative page and matching SHA-256.",
            );
        }
        let location = link.selector_value.as_deref().unwrap_or("");
        match link.selector.as_str() {
            "whole-document" => link.preview(doc.text),
            "markdown-heading" => match doc.heading(location, link.ordinal) {
                Ok(span) => link.preview(&doc.text[span]),
                Err((status, detail)) => link.problem(status, detail),
            },
            "line-range" if page.status != "pinned" => link.problem(
                "unpinned",
                "A line range requires a matching declared SHA-256.",
            ),
            "line-range" => match line_excerpt(doc.text, location) {
                Some(lines) => link.preview(&lines),
                None => link.problem("broken", "The line range does not resolve in this page."),
            },
            _ => link.problem(
                "unsupported",
                "This source selector cannot be resolved in Markdown.",
            ),
        }
        link
    }
}

// Match the line boundaries used by the readiness assessor.
fn line_excerpt(text: &str, range: &str) -> Option<String> {
    let (start, end) = range.strip_prefix('L')?.split_once("-L")?;
    if ![start, end]
        .iter()
        .all(|v| !v.is_empty() && !v.starts_with('0') && v.bytes().all(|c| c.is_ascii_digit()))
    {
        return None;
    }
    let (start, end) = (start.parse::<usize>().ok()?, end.parse::<usize>().ok()?);
    if end < start {
        return None;
    }
    let mut result = Vec::new();
    for (i, line) in crate::input::source_lines(text).enumerate().skip(start - 1) {
        result.push(line);
        if i + 1 == end {
            return Some(result.join("\n"));
        }
    }
    None
}

impl KnowledgeIndex {
    pub fn from_bundle(bundle: &Bundle) -> io::Result<Self> {
        let manifest = &bundle
            .manifest()
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "Expected a GDD model manifest.")
            })?
            .value;
        let project_id = get(manifest, "project")
            .and_then(|p| field(p, "id"))
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "The model requires a project ID.",
                )
            })?;
        let questions: Vec<_> = entries(manifest, "questions")
            .map(|(id, q)| KnowledgeQuestion {
                id: id.clone(),
                statement: field(q, "statement").unwrap_or_default().into(),
                targets: strings(q, "targets").map(str::to_owned).collect(),
                blocks: strings(q, "blocks").map(str::to_owned).collect(),
                status: field(q, "status").unwrap_or_default().into(),
                resolution: get(q, "resolution")
                    .and_then(|v| field(v, "decision"))
                    .map(str::to_owned),
            })
            .collect();
        let mut kinds: BTreeMap<&str, &str> = BTreeMap::new();
        let mut refs = BTreeMap::<&str, Vec<(&str, &str, &Value)>>::new();
        for (kind, record) in bundle
            .nodes()
            .map(|r| ("node", r))
            .chain(bundle.edges().map(|r| ("edge", r)))
        {
            kinds.insert(record.id, kind);
            for reference in items(record.value, "source_refs")
                .iter()
                .chain(items(record.value, "design_refs"))
            {
                if let Some(id) = field(reference, "source") {
                    refs.entry(id)
                        .or_default()
                        .push((record.id, kind, reference));
                }
            }
        }
        kinds.extend(questions.iter().map(|q| (q.id.as_str(), "question")));
        let mut pages = Vec::new();
        let mut links = Vec::new();
        for (id, source) in entries(manifest, "sources").filter(|(_, s)| is_wiki_source(s)) {
            let (page, markdown) = KnowledgePage::capture(id, source, bundle);
            for (target, kind, reference) in refs.remove(id.as_str()).unwrap_or_default() {
                links.push(KnowledgeLink::from_reference(
                    &page,
                    markdown.as_ref(),
                    target,
                    kind,
                    reference,
                ));
            }
            if let Some(doc) = markdown {
                for dest in &doc.links {
                    let mut link =
                        KnowledgeLink::new(&page, &dest.id, &dest.kind, "wiki-link", "provenance");
                    if let Some(section) = dest.section.map(|id| &doc.sections[id]) {
                        link.selector = "markdown-heading".into();
                        link.heading = Some(section.heading.clone());
                        link.selector_value = link.heading.clone();
                        link.ordinal = Some(section.ordinal);
                    }
                    match kinds.get(dest.id.as_str()) {
                        Some(kind) if *kind == dest.kind => link.preview(doc.excerpt_at(dest.section)),
                        _ => link.problem(
                            "broken",
                            "The Markdown link does not identify a graph record of the declared kind.",
                        ),
                    }
                    links.push(link);
                }
            }
            pages.push(page);
        }
        let identities = Arc::new(kinds.keys().map(|s| (*s).to_owned()).collect());
        Ok(Self {
            format: FORMAT.into(),
            project_id: project_id.into(),
            spec_revision: bundle.snapshot().map(|s| s.revision.clone()),
            pages,
            links,
            matches: Vec::new(),
            allow_discovery: true,
            questions,
            identities,
        })
    }

    /// Limit a report to graph records and/or a specific declared page, retaining
    /// questions targeted at those records and links authored against those questions.
    pub fn select(&self, ids: &[String], source: Option<&str>) -> io::Result<Self> {
        if let Some(id) = source
            && !self.pages.iter().any(|p| p.id == id)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("Unknown wiki source: {id}"),
            ));
        }
        let mut subjects: BTreeSet<_> = ids.iter().map(String::as_str).collect();
        for id in &subjects {
            if !self.identities.is_empty() && !self.identities.contains(*id) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("Unknown graph record: {id}"),
                ));
            }
        }
        for q in &self.questions {
            if q.targets.iter().any(|id| subjects.contains(id.as_str())) {
                subjects.insert(q.id.as_str());
            }
        }
        let links: Vec<_> = self
            .links
            .iter()
            .filter(|link| {
                source.is_none_or(|id| link.source == id)
                    && (ids.is_empty() || subjects.contains(link.target.as_str()))
            })
            .cloned()
            .collect();
        let linked_pages: BTreeSet<_> = links.iter().map(|l| l.source.as_str()).collect();
        let pages = self
            .pages
            .iter()
            .filter(|p| {
                source.is_none_or(|id| p.id == id)
                    && (ids.is_empty() || linked_pages.contains(p.id.as_str()))
            })
            .cloned()
            .collect();
        let linked_targets: BTreeSet<_> = links.iter().map(|l| l.target.as_str()).collect();
        let questions = self
            .questions
            .iter()
            .filter(|q| {
                (ids.is_empty() || subjects.contains(q.id.as_str()))
                    && (source.is_none()
                        || linked_targets.contains(q.id.as_str())
                        || q.targets
                            .iter()
                            .any(|t| linked_targets.contains(t.as_str())))
            })
            .cloned()
            .collect();
        Ok(Self {
            format: self.format.clone(),
            project_id: self.project_id.clone(),
            spec_revision: self.spec_revision.clone(),
            pages,
            links,
            questions,
            matches: Vec::new(),
            allow_discovery: self.allow_discovery && ids.is_empty(),
            identities: Arc::clone(&self.identities),
        })
    }

    /// Search the current selection without following links into another scope.
    pub fn search(&mut self, query: &str) {
        let query = query.trim().to_lowercase();
        if query.is_empty() {
            return;
        }
        self.matches.clear();
        self.filter_search(&query);
    }

    fn filter_search(&mut self, query: &str) {
        let hit = |s: &str| s.to_lowercase().contains(query);
        let pages: BTreeSet<_> = self
            .pages
            .iter()
            .filter(|p| [p.id.as_str(), &p.title, &p.uri].into_iter().any(hit))
            .map(|p| p.id.clone())
            .collect();
        let qs: BTreeSet<_> = self
            .questions
            .iter()
            .filter(|q| hit(&q.statement) || hit(&q.id))
            .map(|q| q.id.clone())
            .collect();
        let mut matches_by_page: BTreeMap<&str, Vec<&KnowledgeHit>> = BTreeMap::new();
        for matched in &self.matches {
            matches_by_page
                .entry(&matched.source)
                .or_default()
                .push(matched);
        }
        self.links.retain(|l| {
            pages.contains(&l.source)
                || matches_by_page
                    .get(l.source.as_str())
                    .is_some_and(|hits| hits.iter().any(|m| m.links(l)))
                || qs.contains(&l.target)
                || [
                    l.target.as_str(),
                    l.heading.as_deref().unwrap_or(""),
                    l.excerpt.as_deref().unwrap_or(""),
                ]
                .into_iter()
                .any(hit)
        });
        let matched_pages: BTreeSet<_> = self.links.iter().map(|l| l.source.as_str()).collect();
        let targets: BTreeSet<_> = self.links.iter().map(|l| l.target.as_str()).collect();
        let discovered: BTreeSet<_> = self.matches.iter().map(|m| m.source.as_str()).collect();
        self.pages.retain(|p| {
            pages.contains(&p.id)
                || matched_pages.contains(p.id.as_str())
                || discovered.contains(p.id.as_str())
        });
        self.questions.retain(|q| {
            qs.contains(&q.id)
                || targets.contains(q.id.as_str())
                || q.targets.iter().any(|t| targets.contains(t.as_str()))
        });
    }
}
