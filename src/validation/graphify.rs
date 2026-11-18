//! Compose Graphify observations and portable view verification with a model report.

use std::{io, path::Path};

use crate::bundle::{self, Bundle};
use crate::graphify::{ImportReport, same_projection, verify_graphify};
use crate::input::Value;
use crate::perspectives::PerspectiveBundle;
use crate::report::{Check, CheckResult, Diagnostic, Report};
use crate::view::{ViewInput, verification_check, verification_issue};
use serde_json::Value as JsonValue;

use super::{Profile, ValidationOptions, assess, present, to_json};

/// Extra file inputs for a root validation with Graphify observations.
/// A separate path selects model-bound integration. In standalone mode the
/// positional input supplies the observations. Raw files use the given layout
/// and producer claims, and bundles check those against their recorded claims.
#[derive(Debug, Default)]
pub struct GraphifyValidationOptions<'a> {
    pub path: Option<&'a Path>,
    pub profile: Option<&'a str>,
    pub producer_version: Option<&'a str>,
    pub import_audit: Option<&'a Value>,
}

fn standalone(options: &ValidationOptions<'_>) -> bool {
    options.activity.is_none()
        && !options.profiles.is_empty()
        && options
            .profiles
            .iter()
            .all(|p| *p == Profile::ImportGraphify)
}

/// Assess file inputs, including standalone or model-bound Graphify evidence.
/// Validate all input envelopes and their common scope before capturing files.
pub fn validate_paths_with_graphify(
    input: &Path,
    model: Option<&Path>,
    options: ValidationOptions<'_>,
    graphify: GraphifyValidationOptions<'_>,
    explanation: Option<&mut dyn io::Write>,
) -> io::Result<Report> {
    validate_paths_with_integrations(input, model, options, graphify, None, explanation)
}

/// Assess optional Graphify observations and a portable view against one model capture.
pub fn validate_paths_with_integrations(
    input: &Path,
    model: Option<&Path>,
    options: ValidationOptions<'_>,
    graphify: GraphifyValidationOptions<'_>,
    view: Option<&Path>,
    explanation: Option<&mut dyn io::Write>,
) -> io::Result<Report> {
    let audit = present(graphify.import_audit).map(to_json).transpose()?;
    let scope = options.scope_with_audit(audit.as_ref())?;
    let standalone = model.is_none() && graphify.path.is_none() && standalone(&options);
    let bundle = (!standalone).then(|| {
        let capture_scope = (options.profiles.contains(&Profile::Readiness)
            && present(options.parallel_plan).is_none())
        .then(|| scope.iter().copied().map(str::to_owned).collect::<Vec<_>>());
        bundle::capture(
            model.unwrap_or(input),
            model.map(|_| input),
            capture_scope.as_deref(),
        )
    });
    let observations = if options.profiles.contains(&Profile::ImportGraphify) {
        graphify
            .path
            .or(standalone.then_some(input))
            .map(|path| verify_graphify(path, graphify.profile, graphify.producer_version))
            .transpose()?
    } else {
        None
    };
    assess_inputs(
        bundle.as_ref(),
        observations.map(|report| report.checks),
        options,
        &scope,
        audit.as_ref(),
        view.map(ViewInput::Path),
        explanation,
    )
}

/// Assess a model capture and a previously verified observation report without
/// reopening either input. Pass no bundle for a standalone import.graphify
/// assessment. Pass a bundle and no observations to report missing evidence.
pub fn validate_captured(
    bundle: Option<&Bundle>,
    observations: Option<&ImportReport>,
    options: ValidationOptions<'_>,
    import_audit: Option<&Value>,
    explanation: Option<&mut dyn io::Write>,
) -> io::Result<Report> {
    validate_captured_with_view(
        bundle,
        observations,
        options,
        import_audit,
        None,
        explanation,
    )
}

/// Assess captured model, Graphify, and portable view inputs without reopening files.
pub fn validate_captured_with_view(
    bundle: Option<&Bundle>,
    observations: Option<&ImportReport>,
    options: ValidationOptions<'_>,
    import_audit: Option<&Value>,
    view: Option<&[u8]>,
    explanation: Option<&mut dyn io::Write>,
) -> io::Result<Report> {
    let audit = present(import_audit).map(to_json).transpose()?;
    let scope = options.scope_with_audit(audit.as_ref())?;
    let checks = observations
        .filter(|_| options.profiles.contains(&Profile::ImportGraphify))
        .map(|report| report.checks.clone());
    assess_inputs(
        bundle,
        checks,
        options,
        &scope,
        audit.as_ref(),
        view.map(ViewInput::Bytes),
        explanation,
    )
}

fn assess_inputs(
    bundle: Option<&Bundle>,
    observations: Option<Vec<Check>>,
    options: ValidationOptions<'_>,
    scope: &[&str],
    audit: Option<&JsonValue>,
    view: Option<ViewInput<'_>>,
    explanation: Option<&mut dyn io::Write>,
) -> io::Result<Report> {
    let Some(bundle) = bundle else {
        let Some(checks) = observations.filter(|_| standalone(&options)) else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "A standalone Graphify assessment requires the import.graphify profile and an observation report.",
            ));
        };
        if explanation.is_some() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "--explain requires --profile readiness.",
            ));
        }
        let mut report = Report::standalone_graphify(checks);
        check_loss(&mut report, audit, "Raw reconstruction differs.");
        return Ok(report);
    };
    let mut report = assess(bundle, options, scope, explanation)?;
    if report.profiles.contains(&Profile::ImportGraphify) {
        check_loss(
            &mut report,
            audit,
            "Reconstructed original differs, including order, parallel records, direction or metadata.",
        );
        if let Some(checks) = observations {
            report.checks.extend(checks);
        } else {
            report.checks.push(integration_issue(
                "integration.graphify",
                CheckResult::Unknown,
                "Supply a Graphify observations bundle or raw artifact with an explicit profile.",
                "FORMAT_UNSUPPORTED",
                "No Graphify input was supplied for the model integration.",
                &report.scope,
            ));
        }
    }
    if report.profiles.contains(&Profile::Visualization) {
        let check = if bundle.manifest().is_some()
            && report.spec_revision.is_some()
            && report.result("model") == CheckResult::Pass
        {
            match PerspectiveBundle::from_validated(bundle) {
                Ok(capture) => verification_check(&capture, view),
                Err(error) => verification_issue(
                    CheckResult::Unknown,
                    "FORMAT_UNSUPPORTED",
                    error.to_string(),
                ),
            }
        } else {
            verification_issue(
                CheckResult::Unknown,
                "REVISION_UNRESOLVED",
                "A valid supported GDD model is required for artifact verification.",
            )
        };
        report.checks.push(check);
    }
    Ok(report)
}

fn check_loss(report: &mut Report, audit: Option<&JsonValue>, reason: &str) {
    if let Some(audit) = audit
        && !same_projection(&audit["original"], &audit["reconstructed"])
    {
        report.checks.push(integration_issue(
            "integration.loss",
            CheckResult::Fail,
            reason,
            "IMPORT_LOSS_DETECTED",
            "Raw original cannot be reconstructed exactly.",
            &report.scope,
        ));
    }
}

fn integration_issue(
    id: &str,
    result: CheckResult,
    reason: &str,
    code: &'static str,
    message: &str,
    scope: &[String],
) -> Check {
    let mut check = Check::new(id, "integration", result, reason);
    check.scope = scope.to_vec();
    check
        .diagnostics
        .push(Diagnostic::new(code, result, message, None, ""));
    check
}
