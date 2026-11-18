use super::{Position, PythonVersion};
use ruff_python_ast::{Suite, token::TokenKind};
use ruff_python_parser::{Mode, ParseErrorType, ParseOptions, parse_unchecked};
use ruff_text_size::Ranged;

pub(super) struct Source {
    pub body: Suite,
    line_starts: Vec<usize>,
}

impl Source {
    pub fn position(&self, offset: usize) -> Position {
        position(&self.line_starts, offset)
    }
}

fn position(starts: &[usize], offset: usize) -> Position {
    let line = starts.partition_point(|start| *start <= offset);
    Position {
        line,
        column: offset - starts[line - 1],
    }
}

pub(super) struct SourceError {
    pub position: Option<Position>,
    pub message: String,
}

pub(super) fn parse(
    file: &str,
    bytes: &[u8],
    version: PythonVersion,
) -> Result<Source, SourceError> {
    let basename = file.rsplit('/').next().unwrap_or(file);
    if bytes.contains(&0) {
        return Err(SourceError {
            position: None,
            message: "source code string cannot contain null bytes".into(),
        });
    }
    let source = super::encoding::decode(bytes, basename)?;
    if source.starts_with('\u{feff}') {
        return Err(SourceError {
            position: Some(Position { line: 1, column: 0 }),
            message: format!("invalid non-printable character U+FEFF ({basename}, line 1)"),
        });
    }
    let source = if source.contains('\r') {
        source.replace("\r\n", "\n").replace('\r', "\n")
    } else {
        source
    };
    if source.len() >= u32::MAX as usize {
        return Err(SourceError {
            position: None,
            message: "Source exceeds the parser's byte-offset range.".into(),
        });
    }
    let mut starts = vec![0];
    starts.extend(source.match_indices('\n').map(|(i, _)| i + 1));
    let parsed = parse_unchecked(
        &source,
        ParseOptions::from(Mode::Module).with_target_version(version.grammar()),
    )
    .try_into_module()
    .expect("module mode always returns a module");
    let failure = if let Some(error) = parsed.errors().first() {
        let mut offset = error.start().to_usize();
        let mut message = match &error.error {
            ParseErrorType::ExpectedToken {
                expected: TokenKind::Colon,
                ..
            } => "expected ':'".into(),
            ParseErrorType::UnexpectedIndentation => {
                offset = error.end().to_usize().saturating_sub(1).max(offset);
                "unexpected indent".into()
            }
            ParseErrorType::ExpectedExpression
            | ParseErrorType::ExpectedToken { .. }
            | ParseErrorType::EmptyImportNames
            | ParseErrorType::SimpleStatementsOnSameLine => "invalid syntax".into(),
            other => other.to_string(),
        };
        if let Some(closing @ (")" | "]" | "}")) =
            source.get(error.start().to_usize()..error.end().to_usize())
        {
            message = format!("unmatched '{closing}'");
        }
        // CPython reports the unmatched opener when it reaches end of input.
        if source[offset..].trim().is_empty() {
            let mut open = Vec::new();
            for token in parsed.tokens().iter() {
                match token.kind() {
                    TokenKind::Lpar | TokenKind::Lsqb | TokenKind::Lbrace => open.push(token),
                    TokenKind::Rpar | TokenKind::Rsqb | TokenKind::Rbrace => {
                        open.pop();
                    }
                    _ => {}
                }
            }
            if let Some(open) = open.last() {
                offset = open.start().to_usize();
                message = format!("'{}' was never closed", &source[open.range()]);
            }
        }
        Some((offset, message))
    } else {
        parsed
            .unsupported_syntax_errors()
            .first()
            .map(|e| (e.start().to_usize(), e.to_string()))
    };
    // CPython's tokenizer accepts at most 200 nested delimiters and 99 active
    // indentation levels. Ruff parses deeper trees; those sources cannot provide
    // observations for the selected CPython grammar.
    let mut delimiters = 0usize;
    let mut indents = 0usize;
    let token_error = parsed.tokens().iter().find_map(|t| {
        if t.kind() == TokenKind::Name
            && let Some(inner) = version.invalid_identifier_offset(&source[t.range()])
        {
            let offset = t.start().to_usize() + inner;
            let c = source[offset..].chars().next().unwrap();
            return Some((
                offset,
                format!(
                    "invalid identifier character U+{:04X} for Python {}",
                    c as u32,
                    version.grammar()
                ),
            ));
        }
        match t.kind() {
            TokenKind::Lpar | TokenKind::Lsqb | TokenKind::Lbrace => delimiters += 1,
            TokenKind::Rpar | TokenKind::Rsqb | TokenKind::Rbrace => {
                delimiters = delimiters.saturating_sub(1)
            }
            TokenKind::Indent => indents += 1,
            TokenKind::Dedent => indents = indents.saturating_sub(1),
            _ => {}
        }
        let message = if delimiters > 200 {
            "too many nested parentheses"
        } else if indents >= 100 {
            "too many levels of indentation"
        } else {
            return None;
        };
        Some((t.start().to_usize(), message.to_owned()))
    });
    let failure = failure
        .into_iter()
        .chain(token_error)
        .min_by_key(|(offset, _)| *offset);
    if let Some((offset, message)) = failure {
        let mut pos = position(&starts, offset);
        // SyntaxError.offset counts decoded characters; AST col_offset counts bytes.
        pos.column = source[starts[pos.line - 1]..offset].chars().count();
        return Err(SourceError {
            position: Some(pos),
            message: format!("{message} ({basename}, line {})", pos.line),
        });
    }
    Ok(Source {
        body: parsed.into_syntax().body,
        line_starts: starts,
    })
}
