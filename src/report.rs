//! Checks and reports for the representation and model validation profiles.

use std::collections::BTreeSet;

use serde::Serialize;

use crate::bundle::{self, Location, Outcome};
use crate::valid_id;
use crate::validation::Profile;

/// Verdicts in increasing order of precedence for aggregation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum CheckResult {
    Pass,
    Unknown,
    Fail,
}

impl From<Outcome> for CheckResult {
    fn from(outcome: Outcome) -> Self {
        match outcome {
            Outcome::Fail => Self::Fail,
            Outcome::Unknown => Self::Unknown,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Diagnostic {
    pub code: &'static str,
    pub severity: &'static str,
    pub message: String,
    pub location: DiagnosticLocation,
    pub related_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
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
                CheckResult::Fail => "error",
                CheckResult::Unknown => "warning",
                CheckResult::Pass => "info",
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

#[derive(Debug, Serialize)]
pub struct Check {
    pub check_id: String,
    pub dimension: &'static str,
    pub result: CheckResult,
    pub scope: Vec<String>,
    pub reason: String,
    pub diagnostics: Vec<Diagnostic>,
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
        }
    }
}

impl From<&bundle::Diagnostic> for Check {
    fn from(issue: &bundle::Diagnostic) -> Self {
        let result = issue.outcome.into();
        let Location { file, pointer } = &issue.location;
        let mut check = Self::new(
            format!("check.{}", issue.code.to_ascii_lowercase()),
            issue.dimension,
            result,
            issue.message.as_ref(),
        );
        check.diagnostics.push(
            Diagnostic::new(
                issue.code,
                result,
                issue.message.as_ref(),
                Some(file),
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

    pub fn result(&self, dimension: &str) -> CheckResult {
        self.checks
            .iter()
            .filter(|check| check.dimension == dimension)
            .map(|check| check.result)
            .max()
            .unwrap_or(CheckResult::Unknown)
    }

    pub fn exit_code(&self) -> u8 {
        match self.checks.iter().map(|check| check.result).max() {
            Some(CheckResult::Pass) => 0,
            Some(CheckResult::Fail) => 1,
            Some(CheckResult::Unknown) | None => 2,
        }
    }
}
