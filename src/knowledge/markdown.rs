//! CommonMark headings and explicit graph links in authored prose.
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use std::{collections::BTreeMap, ops::Range};

#[derive(Debug)]
pub(super) struct Section {
    pub heading: String,
    pub ordinal: usize,
    pub span: Range<usize>,
    level: u8,
}

#[derive(Debug)]
pub(super) struct Destination {
    pub kind: String,
    pub id: String,
    pub section: Option<usize>,
}

pub(crate) struct Markdown<'a> {
    pub(super) text: &'a str,
    pub(super) sections: Vec<Section>,
    pub(super) links: Vec<Destination>,
}

pub(super) fn parser(text: &str) -> Parser<'_> {
    Parser::new_ext(
        text,
        Options::ENABLE_YAML_STYLE_METADATA_BLOCKS
            | Options::ENABLE_PLUSES_DELIMITED_METADATA_BLOCKS,
    )
}

impl<'a> Markdown<'a> {
    pub(crate) fn parse(text: &'a str) -> Self {
        Self::parse_page(text, None)
    }

    pub(super) fn parse_page(text: &'a str, page_uri: Option<&str>) -> Self {
        let mut sections: Vec<Section> = Vec::new();
        let mut active: Vec<usize> = Vec::new();
        let mut counts = BTreeMap::new();
        let mut current_heading = None;
        let mut destinations = Vec::new();
        for (event, span) in parser(text).into_offset_iter() {
            match event {
                Event::Start(Tag::Heading { level, .. }) => {
                    let level = level as u8;
                    while active.last().is_some_and(|i| sections[*i].level >= level) {
                        sections[active.pop().unwrap()].span.end = span.start;
                    }
                    current_heading = Some((level, span.start, String::new()));
                }
                Event::Text(label) | Event::Code(label) => {
                    if let Some((_, _, text)) = &mut current_heading {
                        text.push_str(&label);
                    }
                }
                Event::SoftBreak | Event::HardBreak => {
                    if let Some((_, _, text)) = &mut current_heading {
                        text.push(' ');
                    }
                }
                Event::End(TagEnd::Heading(_)) => {
                    if let Some((level, start, label)) = current_heading.take() {
                        let heading = active.last().map_or_else(
                            || label.trim().to_owned(),
                            |id| format!("{} / {}", sections[*id].heading, label.trim()),
                        );
                        let count = counts.entry(heading.clone()).or_insert(0);
                        *count += 1;
                        active.push(sections.len());
                        sections.push(Section {
                            heading,
                            ordinal: *count,
                            span: start..text.len(),
                            level,
                        });
                    }
                }
                Event::Start(Tag::Link { dest_url, .. }) => {
                    if let Some((kind, id)) = graph_destination(&dest_url, page_uri) {
                        destinations.push((span.start, kind.to_owned(), id.to_owned()));
                    }
                }
                _ => {}
            }
        }
        let links = destinations
            .into_iter()
            .map(|(offset, kind, id)| {
                let section = sections
                    .partition_point(|s| s.span.start <= offset)
                    .checked_sub(1);
                Destination { kind, id, section }
            })
            .collect();
        Self {
            text,
            sections,
            links,
        }
    }

    pub(crate) fn heading(
        &self,
        path: &str,
        ordinal: Option<usize>,
    ) -> Result<Range<usize>, (&'static str, &'static str)> {
        let mut candidates = self
            .sections
            .iter()
            .filter(|s| s.heading == path && ordinal.is_none_or(|i| i == s.ordinal));
        let first = candidates.next().ok_or((
            "broken",
            "The heading path or ordinal does not exist in this page.",
        ))?;
        if candidates.next().is_some() {
            return Err((
                "ambiguous",
                "More than one heading has this path. Supply a 1-based ordinal.",
            ));
        }
        Ok(first.span.clone())
    }

    pub(super) fn excerpt_at(&self, section: Option<usize>) -> &str {
        match section {
            Some(i) => &self.text[self.sections[i].span.clone()],
            None => {
                &self.text[..self
                    .sections
                    .first()
                    .map_or(self.text.len(), |s| s.span.start)]
            }
        }
    }
}

// Resolve authored relative links lexically. Exported graph pages can be generated
// later, so this does not check the filesystem or follow symlinks.
fn graph_destination<'a>(
    destination: &'a str,
    page_uri: Option<&str>,
) -> Option<(&'a str, &'a str)> {
    if let Some(target) = destination.strip_prefix("gdd:") {
        return Some(target.split_once('/').unwrap_or(("", target)));
    }
    let (relative, fragment) = destination.split_once('#')?;
    let record = fragment.strip_prefix("gdd-")?.split_once('-')?;
    if relative.is_empty() || relative.starts_with('/') || relative.contains([':', '\\', '?']) {
        return None;
    }
    let uri = page_uri?;
    let directory = uri.rsplit_once('/').map_or("", |(dir, _)| dir);
    let mut parts = Vec::new();
    for part in directory.split('/').chain(relative.split('/')) {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            _ => parts.push(part),
        }
    }
    (parts == ["gdd-graph.md"]).then_some(record)
}

pub(super) fn excerpt(text: &str) -> (String, bool) {
    const MAX_CHARS: usize = 4096;
    let text = text.trim_end();
    match text.char_indices().nth(MAX_CHARS) {
        Some((end, _)) => (text[..end].to_owned(), true),
        None => (text.to_owned(), false),
    }
}
