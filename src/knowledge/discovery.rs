//! Search the readable prose of a captured wiki before graph references exist.
use super::{
    KnowledgeIndex, KnowledgeLink, KnowledgePage, entries, is_wiki_source,
    markdown::{Markdown, parser},
};
use crate::bundle::Bundle;
use pulldown_cmark::{Event, Tag, TagEnd};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, io};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KnowledgeHit {
    pub source: String,
    pub heading: Option<String>,
    pub ordinal: Option<usize>,
    pub excerpt: String,
    pub sha256: String,
    pub status: String,
}

impl KnowledgeHit {
    pub(super) fn links(&self, link: &KnowledgeLink) -> bool {
        self.source == link.source
            && match link.selector.as_str() {
                "whole-document" => link.kind == "graph-reference" || self.heading.is_none(),
                "markdown-heading" => {
                    self.heading == link.heading
                        && link
                            .ordinal
                            .is_none_or(|ordinal| Some(ordinal) == self.ordinal)
                }
                _ => false,
            }
    }
}

fn snippet(text: &str, needle: &str) -> Option<String> {
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let folded = normalized.to_lowercase();
    let hit = folded.find(needle)?;
    // Lowercasing can expand a Unicode character and change subsequent byte offsets.
    let mut folded_position = 0;
    let mut position = 0usize;
    for c in normalized.chars() {
        let bytes = c.to_lowercase().map(char::len_utf8).sum::<usize>();
        if folded_position + bytes > hit {
            break;
        }
        folded_position += bytes;
        position += 1;
    }
    let start = position.saturating_sub(60usize);
    let mut chars = normalized.chars().skip(start);
    let mut result = String::new();
    if start > 0 {
        result.push('…');
    }
    result.extend(chars.by_ref().take(280));
    if chars.next().is_some() {
        result.push('…');
    }
    Some(result)
}

/// Discover at most one hit per directly matching heading or page introduction.
/// Read the exact bytes captured by load_for_knowledge. A changed pin supplies no text.
pub fn discover_sections(
    bundle: &Bundle,
    query: &str,
    source: Option<&str>,
) -> io::Result<Vec<KnowledgeHit>> {
    let needle = query
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    let manifest = bundle.manifest().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidData, "Expected a GDD model manifest.")
    })?;
    let mut found_source = false;
    let mut matches = Vec::new();
    for (id, s) in entries(&manifest.value, "sources")
        .filter(|(id, s)| is_wiki_source(s) && source.is_none_or(|source| source == id.as_str()))
    {
        found_source = true;
        if needle.is_empty() {
            continue;
        }
        let (page, markdown) = KnowledgePage::capture(id, s, bundle);
        if let Some(doc) = markdown {
            search_document(&doc, &page, &needle, &mut matches);
        }
    }
    if source.is_some() && !found_source {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Unknown wiki source for discovery.",
        ));
    }
    Ok(matches)
}

fn search_document(
    doc: &Markdown<'_>,
    page: &KnowledgePage,
    needle: &str,
    hits: &mut Vec<KnowledgeHit>,
) {
    let mut headings = doc.sections.iter();
    let mut current = None;
    let mut text = String::new();
    let mut skip = false;
    let mut finish = |text: &str, section: Option<&super::markdown::Section>| {
        if let Some(excerpt) = snippet(text, needle) {
            hits.push(KnowledgeHit {
                source: page.id.clone(),
                heading: section.map(|s| s.heading.clone()),
                ordinal: section.map(|s| s.ordinal),
                excerpt,
                sha256: page.sha256.clone().expect("captured text has a digest"),
                status: page.status.clone(),
            });
        }
    };
    for event in parser(doc.text) {
        match event {
            Event::Start(Tag::MetadataBlock(_) | Tag::CodeBlock(_)) => {
                skip = true;
                text.push(' ');
            }
            Event::End(TagEnd::MetadataBlock(_) | TagEnd::CodeBlock) => skip = false,
            _ if skip => {}
            Event::Start(Tag::Heading { .. }) => {
                finish(&text, current);
                text.clear();
                current = headings.next();
            }
            Event::Text(s) | Event::Code(s) => text.push_str(&s),
            Event::SoftBreak
            | Event::HardBreak
            | Event::Rule
            | Event::End(TagEnd::Paragraph | TagEnd::Heading(_) | TagEnd::Item) => text.push(' '),
            _ => {}
        }
    }
    finish(&text, current);
}

impl KnowledgeIndex {
    /// Search declared Markdown sources in the same frozen bundle and preserve
    /// source metadata with every hit. A scoped index searches only linked excerpts.
    pub fn search_bundle(
        &mut self,
        bundle: &Bundle,
        query: &str,
        source: Option<&str>,
    ) -> io::Result<()> {
        if let Some(id) = source {
            *self = self.select(&[], Some(id))?;
        }
        if !self.allow_discovery {
            self.search(query);
            return Ok(());
        }
        let query = query.trim().to_lowercase();
        if query.is_empty() {
            return Ok(());
        }
        let pages: BTreeMap<_, _> = self
            .pages
            .iter()
            .map(|page| (page.id.as_str(), page.sha256.as_deref()))
            .collect();
        self.matches = discover_sections(bundle, &query, source)?
            .into_iter()
            .filter(|m| pages.get(m.source.as_str()) == Some(&Some(m.sha256.as_str())))
            .collect();
        self.filter_search(&query);
        Ok(())
    }
}
