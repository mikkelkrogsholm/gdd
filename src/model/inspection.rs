//! Representation checks shared by captured model inspection commands.
use std::collections::{BTreeMap, BTreeSet};

use super::{entries, string};
use crate::bundle::{Bundle, Document, FormatIssue};
use crate::report::{Check, CheckResult, Diagnostic};
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
        .chain(
            bundle
                .unpinned_extensions
                .values()
                .filter_map(|v| v.as_ref().err()),
        )
        .filter_map(|d| Some((&d.location.file, d.format_issue?)))
        .collect();
    let mut unread = bundle.unread_extensions.clone();
    if let Some(root) = root.filter(|d| d.kind.is_manifest()) {
        for (id, decl) in entries(&root.value, "extensions") {
            let Some(observed) = bundle.unpinned_extensions.get(string(decl, "namespace")) else {
                continue;
            };
            let check = match observed {
                Ok(doc) => validation::representation_check(doc).expect("extension schema"),
                Err(issue) => Check::from(issue),
            };
            if check.result != CheckResult::Pass {
                unread.insert(id.clone());
            }
            let filename = match observed {
                Ok(doc) => &doc.asset.path,
                Err(issue) => &issue.location.file,
            };
            if let Some(i) = checks.iter().position(|c| {
                c.diagnostics.iter().any(|d| {
                    d.code == "EXTENSION_DATA_INVALID" && d.location.file.as_ref() == Some(filename)
                })
            }) {
                if check.result == CheckResult::Pass {
                    checks.insert(i, check);
                } else {
                    checks[i] = check;
                }
            } else {
                checks.push(check);
            }
        }
    }
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
                d.location
                    .file
                    .as_ref()
                    .is_some_and(|file| rejected.contains(file))
            })
    });
    for check in &mut checks {
        for d in &mut check.diagnostics {
            if d.code == "RECORD_INVALID" && d.location.pointer == "/project_id" {
                d.location.pointer.clear();
            }
            if d.code == "SCHEMA_INVALID"
                && d.location.pointer == "/format"
                && check.check_id == "check.schema_invalid"
            {
                match d
                    .location
                    .file
                    .as_ref()
                    .and_then(|path| format_issues.get(path))
                {
                    Some(FormatIssue::Expected(kind)) => {
                        check.check_id = "representation.schema".into();
                        check.reason = format!("Expected {}.", kind.format());
                    }
                    Some(FormatIssue::Missing) => {
                        check.check_id = "representation.format".into();
                        check.reason = "Document requires a string format identifier.".into();
                        d.message = "Missing or invalid format identifier.".into();
                    }
                    None => {}
                }
            }
        }
    }
    if let Some(manifest) = root.filter(|d| d.kind.is_manifest()) {
        let extensions: BTreeMap<_, _> = bundle.extensions().collect();
        for (id, decl) in entries(&manifest.value, "extensions") {
            let invalid = extensions
                .get(string(decl, "namespace"))
                .is_some_and(|d| rejected.contains(&d.asset.path));
            if invalid || unread.contains(id) {
                let reason = format!("Unavailable extension {id}.");
                let mut check = Check::new(
                    "check.extension_unsupported",
                    "model",
                    CheckResult::Unknown,
                    &reason,
                );
                check.diagnostics.push(Diagnostic::new(
                    "EXTENSION_UNSUPPORTED",
                    CheckResult::Unknown,
                    reason,
                    Some(string(decl, "path")),
                    "",
                ));
                checks.push(check);
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
