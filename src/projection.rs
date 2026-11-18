//! Admission of one captured model for read-only projections.

use crate::bundle::{self, Bundle};
use crate::report::{Check, CheckResult};
use crate::validation::{self, Profile};
use std::{fmt, io, path::Path};

#[derive(Debug)]
pub struct ProjectionError {
    message: String,
    pub exit_code: u8,
}
impl fmt::Display for ProjectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for ProjectionError {}
impl From<ProjectionError> for io::Error {
    fn from(value: ProjectionError) -> Self {
        io::Error::other(value)
    }
}

fn require_pass(checks: &[Check], path: &Path) -> Result<(), ProjectionError> {
    let mut details = Vec::new();
    let mut failed = false;
    for check in checks.iter().filter(|c| c.result != CheckResult::Pass) {
        failed |= check.result == CheckResult::Fail;
        if check.diagnostics.is_empty() {
            details.push(format!("{}: {}", check.check_id, check.reason));
        }
        for d in &check.diagnostics {
            details.push(format!(
                "{} at {}{}: {}",
                d.code,
                d.location
                    .file
                    .as_deref()
                    .unwrap_or_else(|| path.to_str().unwrap_or("<input>")),
                &d.location.pointer,
                d.message
            ));
        }
    }
    if details.is_empty() {
        return Ok(());
    }
    Err(ProjectionError {
        exit_code: if failed { 1 } else { 2 },
        message: format!(
            "Projection requires a resolved, valid model bundle. {}",
            details.join("\n")
        ),
    })
}

/// Accept representation, loading and core model checks independently of the
/// historical applicability of operational records.
pub fn load_projection_bundle(path: &Path) -> Result<Bundle, ProjectionError> {
    let bundle = bundle::load_model_only(path);
    let representation = validation::validate(&bundle, Profile::Representation);
    require_pass(&representation.checks, path)?;
    let manifest = bundle.manifest().ok_or_else(|| ProjectionError {
        exit_code: 2,
        message: "Projection requires a supported GDD 0.2 or 0.3 manifest.".into(),
    })?;
    require_pass(&crate::model::projection_checks(&bundle, manifest), path)?;
    if representation.spec_revision.is_none() {
        return Err(ProjectionError {
            exit_code: 2,
            message: "Projection requires a resolved model snapshot.".into(),
        });
    }
    Ok(bundle)
}
