//! Schema-shaped validation checks and dimension-specific verdicts.

use std::collections::BTreeSet;

use serde::Serialize;

use crate::bundle::{self, Location, Outcome};
use crate::valid_id;
use crate::validation::{Activity, Profile};

/// Verdicts in increasing order of precedence for aggregation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum CheckResult {
    Pass,
    Granted,
    Unknown,
    Unresolved,
    Fail,
    Denied,
}

impl std::fmt::Display for CheckResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Pass => "PASS",
            Self::Fail => "FAIL",
            Self::Unknown => "UNKNOWN",
            Self::Granted => "GRANTED",
            Self::Denied => "DENIED",
            Self::Unresolved => "UNRESOLVED",
        })
    }
}

impl From<Outcome> for CheckResult {
    fn from(outcome: Outcome) -> Self {
        match outcome {
            Outcome::Fail => Self::Fail,
            Outcome::Unknown => Self::Unknown,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Diagnostic {
    pub code: &'static str,
    pub severity: &'static str,
    pub message: String,
    pub location: DiagnosticLocation,
    pub related_ids: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct DiagnosticLocation {
    pub file: Option<String>,
    pub pointer: String,
}

impl Diagnostic {
    pub fn new(
        code: &'static str,
        result: CheckResult,
        message: impl Into<String>,
        file: Option<&str>,
        pointer: impl Into<String>,
    ) -> Self {
        Self {
            code,
            severity: match result {
                CheckResult::Fail | CheckResult::Denied => "error",
                CheckResult::Unknown | CheckResult::Unresolved => "warning",
                CheckResult::Pass | CheckResult::Granted => "info",
            },
            message: message.into(),
            location: DiagnosticLocation {
                file: file.map(str::to_owned),
                pointer: pointer.into(),
            },
            related_ids: Vec::new(),
        }
    }

    pub fn with_related_ids<'a>(mut self, ids: impl IntoIterator<Item = &'a str>) -> Self {
        let mut seen = BTreeSet::new();
        self.related_ids = ids
            .into_iter()
            .filter(|id| valid_id(id) && seen.insert(*id))
            .map(str::to_owned)
            .collect();
        self
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Check {
    pub check_id: String,
    pub dimension: &'static str,
    pub result: CheckResult,
    pub scope: Vec<String>,
    pub reason: String,
    pub diagnostics: Vec<Diagnostic>,
    #[serde(flatten)]
    pub details: Option<CheckDetails>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(untagged)]
pub enum CheckDetails {
    Authorization {
        activity: Activity,
        #[serde(skip_serializing_if = "Option::is_none")]
        policy_revision: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        authority_basis: Option<String>,
    },
    Implementation {
        implementation: String,
        code_revision: String,
        evidence_refs: Vec<String>,
    },
}

impl Check {
    pub fn new(
        id: impl Into<String>,
        dimension: &'static str,
        result: CheckResult,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            check_id: id.into(),
            dimension,
            result,
            scope: Vec::new(),
            reason: reason.into(),
            diagnostics: Vec::new(),
            details: None,
        }
    }
}

impl From<&bundle::Diagnostic> for Check {
    fn from(issue: &bundle::Diagnostic) -> Self {
        let result = issue.outcome.into();
        let Location { file, pointer } = &issue.location;
        let mut check = if issue.code == "FORMAT_UNSUPPORTED" && pointer == "/format" {
            Self::new(
                "representation.format",
                issue.dimension,
                result,
                "Format is not supported.",
            )
        } else {
            Self::new(
                format!("check.{}", issue.code.to_ascii_lowercase()),
                issue.dimension,
                result,
                issue.message.as_ref(),
            )
        };
        check.diagnostics.push(
            Diagnostic::new(
                issue.code,
                result,
                issue.message.as_ref(),
                (!file.is_empty()).then_some(file.as_str()),
                pointer,
            )
            .with_related_ids(issue.related_ids.iter().map(String::as_str)),
        );
        check
    }
}

#[derive(Debug, Serialize)]
struct Tool {
    name: &'static str,
    version: &'static str,
}

#[derive(Debug, Serialize)]
pub struct Report {
    format: &'static str,
    tool: Tool,
    pub spec_revision: Option<String>,
    pub profiles: Vec<Profile>,
    pub scope: Vec<String>,
    pub checks: Vec<Check>,
}

impl Report {
    pub(crate) fn new(profiles: Vec<Profile>, scope: Vec<String>) -> Self {
        Self {
            format: "gdd-report/0.2",
            tool: Tool {
                name: "gdd",
                version: env!("CARGO_PKG_VERSION"),
            },
            spec_revision: None,
            profiles,
            scope,
            checks: Vec::new(),
        }
    }

    pub(crate) fn standalone_graphify(checks: Vec<Check>) -> Self {
        Self {
            tool: Tool {
                name: crate::graphify::IMPORTER,
                version: crate::graphify::VERSION,
            },
            checks,
            ..Self::new(vec![Profile::ImportGraphify], Vec::new())
        }
    }

    pub fn result(&self, dimension: &str) -> CheckResult {
        self.checks
            .iter()
            .filter(|check| check.dimension == dimension)
            .map(|check| check.result)
            .max()
            .unwrap_or(if dimension == "authorization" {
                CheckResult::Unresolved
            } else {
                CheckResult::Unknown
            })
    }

    pub fn exit_code(&self) -> u8 {
        checks_exit_code(self.checks.iter())
    }
}

pub(crate) fn checks_exit_code<'a>(checks: impl Iterator<Item = &'a Check>) -> u8 {
    match checks.map(|check| check.result).max() {
        Some(CheckResult::Pass | CheckResult::Granted) => 0,
        Some(CheckResult::Fail | CheckResult::Denied) => 1,
        Some(CheckResult::Unknown | CheckResult::Unresolved) | None => 2,
    }
}

pub(crate) fn quoted(value: &str) -> String {
    let quote = if value.contains('\'') && !value.contains('"') {
        '"'
    } else {
        '\''
    };
    use std::fmt::Write;
    use unicode_general_category::{GeneralCategory as Category, get_general_category};

    let mut label = String::from(quote);
    for ch in value.chars() {
        match ch {
            '\n' => label.push_str("\\n"),
            '\t' => label.push_str("\\t"),
            '\r' => label.push_str("\\r"),
            ch if ch == quote || ch == '\\' => {
                label.push('\\');
                label.push(ch);
            }
            ch if ch != ' '
                && matches!(
                    get_general_category(ch),
                    Category::Control
                        | Category::Format
                        | Category::PrivateUse
                        | Category::Unassigned
                        | Category::LineSeparator
                        | Category::ParagraphSeparator
                        | Category::SpaceSeparator
                ) =>
            {
                let code = u32::from(ch);
                match code {
                    0..=0xff => write!(label, "\\x{code:02x}"),
                    0x100..=0xffff => write!(label, "\\u{code:04x}"),
                    _ => write!(label, "\\U{code:08x}"),
                }
                .expect("writing to a string cannot fail");
            }
            ch => label.push(ch),
        }
    }
    label.push(quote);
    label
}
