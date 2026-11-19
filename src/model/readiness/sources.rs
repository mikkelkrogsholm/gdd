use super::{Readiness, flag, get, string, text};
use crate::input::{self, InputError, Value};

impl Readiness<'_, '_> {
    pub(super) fn source(&mut self, id: &str, reference: &Value) {
        let source_id = string(reference, "source");
        let Some(source) =
            get(&self.model.manifest.value, "sources").and_then(|s| get(s, source_id))
        else {
            return;
        };
        let format = string(source, "format");
        if !matches!(
            format,
            "text"
                | "text/plain"
                | "markdown"
                | "text/markdown"
                | "json"
                | "application/json"
                | "yaml"
                | "application/yaml"
                | "design-md/alpha"
                | "okf/0.2"
        ) {
            self.unknown(
                id,
                "SOURCE_UNAVAILABLE",
                format!("Unsupported contract source format: {format}."),
            );
        }
        let hash = get(source, "revision")
            .and_then(|r| get(r, "sha256"))
            .and_then(text);
        if !flag(source, "normative") || hash.is_none() {
            self.fail(
                id,
                "CONTRACT_SOURCE_UNPINNED",
                format!("Contract source {source_id} is not normative and pinned."),
                &[],
            );
        }
        let asset = match self.model.bundle.contract_asset(source_id) {
            Ok(asset) => asset,
            Err(error) => {
                self.unknown(
                    id,
                    "SOURCE_UNAVAILABLE",
                    format!(
                        "Contract source unavailable: {}.",
                        error.trim_end_matches('.')
                    ),
                );
                return;
            }
        };
        if !self.model.bundle.snapshot().is_some_and(|s| {
            s.assets
                .iter()
                .any(|a| a.path == asset.path && Some(a.sha256.as_str()) == hash)
        }) {
            self.fail(
                id,
                "CONTRACT_SOURCE_UNPINNED",
                format!("Contract source {source_id} is not in a verified snapshot."),
                &[],
            );
        }
        if let Err(error) = resolve(
            format,
            &asset.bytes,
            get(reference, "selector"),
            crate::knowledge::is_wiki_source(source),
        ) {
            let code = if error == InputError::UnsupportedDepth {
                "FORMAT_UNSUPPORTED"
            } else {
                "SOURCE_UNAVAILABLE"
            };
            self.unknown(id, code, format!("Contract source unavailable: {error}"));
        }
    }
}

fn resolve(
    format: &str,
    bytes: &[u8],
    selector: Option<&Value>,
    wiki: bool,
) -> Result<(), InputError> {
    let invalid = |message: &str| InputError::Invalid(message.into());
    let source = std::str::from_utf8(bytes).map_err(|e| InputError::Invalid(e.to_string()))?;
    let mut structured = match format {
        "json" | "application/json" => Some(input::parse_json(source)?),
        "yaml" | "application/yaml" => Some(input::parse(bytes)?),
        _ => None,
    };
    let Some(selector) = selector else {
        return Ok(());
    };
    let value = get(selector, "value").and_then(text).unwrap_or("");
    match string(selector, "kind") {
        "opaque" => return Err(invalid("Opaque selector semantics are unsupported.")),
        "markdown-heading" if wiki => {
            let ordinal = match get(selector, "ordinal") {
                None => None,
                Some(Value::Number(n)) => Some(
                    n.parse::<usize>()
                        .ok()
                        .filter(|i| *i > 0)
                        .ok_or_else(|| invalid("Invalid heading ordinal."))?,
                ),
                Some(_) => return Err(invalid("Invalid heading ordinal.")),
            };
            crate::knowledge::markdown::Markdown::parse(source)
                .heading(value, ordinal)
                .map_err(|(_, reason)| invalid(reason))?;
        }
        "markdown-heading" => {
            return Err(invalid(
                "Markdown-heading selectors are preserved but not resolved by this version.",
            ));
        }
        "line-range" => {
            let end = value
                .split_once("-L")
                .and_then(|(_, end)| end.parse::<usize>().ok());
            if !super::super::is_line_range(value)
                || end.is_none_or(|end| end > input::source_lines(source).count())
            {
                return Err(invalid("Line range outside source."));
            }
        }
        "json-pointer" => {
            if format == "design-md/alpha" {
                let mut lines = input::source_lines(source);
                let mut frontmatter = Vec::new();
                if lines.next() != Some("---") {
                    return Err(invalid("DESIGN.md has no parseable YAML frontmatter."));
                }
                loop {
                    match lines.next() {
                        Some("---") => break,
                        Some(line) => frontmatter.push(line),
                        None => {
                            return Err(invalid("DESIGN.md has no parseable YAML frontmatter."));
                        }
                    }
                }
                structured = Some(input::parse(frontmatter.join("\n").as_bytes())?);
            }
            let mut current = structured.as_ref().ok_or_else(|| {
                invalid("JSON Pointer requires JSON/YAML tokens or DESIGN.md frontmatter.")
            })?;
            for token in value.split('/').skip(1) {
                let part = token.replace("~1", "/").replace("~0", "~");
                current = match current {
                    Value::Object(values) => values.get(&part),
                    Value::Array(values)
                        if part == "0"
                            || (!part.starts_with('0')
                                && part.bytes().all(|b| b.is_ascii_digit())) =>
                    {
                        part.parse::<usize>().ok().and_then(|i| values.get(i))
                    }
                    _ => None,
                }
                .ok_or_else(|| invalid("JSON Pointer does not resolve."))?;
            }
        }
        _ => {}
    }
    Ok(())
}
