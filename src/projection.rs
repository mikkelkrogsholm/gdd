//! Admission of one captured model for read-only projections.

use crate::bundle::{self, Bundle};
use crate::report::{Check, CheckResult};
use std::{fmt, io, path::Path};

#[derive(Debug)]
pub struct ProjectionError {
    message: String,
    pub exit_code: u8,
}
impl ProjectionError {
    pub(crate) fn unsupported(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            exit_code: 2,
        }
    }
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
    let bundle = bundle::load_for_inspection(path);
    validate_projection_bundle(&bundle)?;
    Ok(bundle)
}

pub(crate) fn validate_projection_bundle(bundle: &Bundle) -> Result<(), ProjectionError> {
    let path = bundle.path();
    if path.as_os_str().as_encoded_bytes().contains(&0) {
        return Err(ProjectionError {
            message: "Embedded NUL in model path.".into(),
            exit_code: 1,
        });
    }
    let inspected = crate::model::inspection::inspect(bundle);
    require_pass(&inspected.checks, path)?;
    let manifest = bundle.manifest().ok_or_else(|| {
        ProjectionError::unsupported("Projection requires a supported GDD 0.2 or 0.3 manifest.")
    })?;
    require_pass(&crate::model::projection_checks(bundle, manifest), path)?;
    if inspected.revision.is_none() {
        return Err(ProjectionError::unsupported(
            "Projection requires a resolved model snapshot.",
        ));
    }
    Ok(())
}
