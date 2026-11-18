//! Representation checks shared by captured model inspection commands.
use std::collections::{BTreeMap, BTreeSet};

use crate::bundle::{Bundle, Document};
use crate::report::{Check, CheckResult};
use crate::validation::{self, Profile};

pub(crate) struct Inspection<'a> {
    pub root: Option<&'a Document>,
    pub revision: Option<String>,
    pub checks: Vec<Check>,
    pub rejected: BTreeSet<String>,
}

pub(crate) fn inspect(bundle: &Bundle) -> Inspection<'_> {
    let report = validation::validate(bundle, Profile::Representation);
    let mut checks = report.checks;
    let root = bundle.root_document().filter(|_| {
        checks
            .first()
            .is_some_and(|c| c.result == CheckResult::Pass)
    });
    if root.is_none() && bundle.manifest().is_some() {
        // A rejected manifest cannot declare additional inputs.
        checks.truncate(1);
    }
    let format_issues: BTreeMap<_, _> = bundle
        .diagnostics()
        .iter()
        .filter_map(|d| Some((&d.location.file, d.format_issue?)))
        .collect();
    let rejected: BTreeSet<_> = checks
        .iter()
        .filter(|c| c.check_id == "representation.schema" && c.result != CheckResult::Pass)
        .flat_map(|c| &c.diagnostics)
        .filter_map(|d| d.location.file.clone())
        .collect();
    // General-purpose captures can index fields before the document fails its
    // schema. That document cannot contribute additional identity assertions.
    checks.retain(|c| {
        c.check_id == "representation.schema"
            || !c.diagnostics.iter().any(|d| {
                // Only diagnostics produced by indexing a rejected document are redundant.
                matches!(d.code, "SCHEMA_INVALID" | "RECORD_INVALID" | "ID_COLLISION")
                    && d.location
                        .file
                        .as_ref()
                        .is_some_and(|file| rejected.contains(file))
            })
    });
    for check in &mut checks {
        if check.check_id == "check.schema_invalid"
            && let Some(format) = check
                .diagnostics
                .first()
                .filter(|d| d.location.pointer == "/format")
                .and_then(|d| d.location.file.as_ref())
                .and_then(|path| format_issues.get(path))
        {
            check.classify_format(*format);
        }
        for d in &mut check.diagnostics {
            if d.code == "RECORD_INVALID" && d.location.pointer == "/project_id" {
                d.location.pointer.clear();
            }
        }
    }
    Inspection {
        root,
        revision: report.spec_revision,
        checks,
        rejected,
    }
}
