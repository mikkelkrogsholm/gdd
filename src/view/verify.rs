//! Exact regeneration of passive view artifacts from the current model capture.

use super::{
    FORMAT, GENERATOR, PROFILE, VERSION, read_view_bytes, view_data, view_report, write_artifact,
};
use crate::bundle::Bundle;
use crate::input::{InputError, Value, parse_json};
use crate::perspectives::{PerspectiveBundle, load_perspective_bundle};
use crate::report::{Check, CheckResult, Diagnostic};
use serde_json::Value as Json;
use std::{borrow::Borrow, collections::BTreeMap, io, path::Path};

const CHECK: &str = "integration.visualization";
const MISMATCH: &str = "VIEW_ARTIFACT_MISMATCH";

pub(crate) enum ViewInput<'a> {
    Path(&'a Path),
    Bytes(&'a [u8]),
}

pub(crate) fn verification_issue(
    result: CheckResult,
    code: &'static str,
    message: impl Into<String>,
) -> Check {
    let mut check = Check::new(CHECK, "integration", result, message);
    check
        .diagnostics
        .push(Diagnostic::new(code, result, &check.reason, None, ""));
    check
}

fn mismatch(reason: impl Into<String>) -> Check {
    verification_issue(CheckResult::Fail, MISMATCH, reason)
}

fn unsupported(reason: impl Into<String>) -> Check {
    verification_issue(CheckResult::Unknown, "FORMAT_UNSUPPORTED", reason)
}

fn string_field<'a>(fields: &'a BTreeMap<String, Value>, key: &str) -> Option<&'a str> {
    match fields.get(key) {
        Some(Value::String(value)) if !value.is_empty() => Some(value),
        _ => None,
    }
}

fn verify_bytes(capture: &PerspectiveBundle<impl Borrow<Bundle>>, bytes: &[u8]) -> Check {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return mismatch("Expected a UTF-8 view artifact.");
    };
    // The released HTML shell encloses one passive JSON payload. Read its
    // selector as data so the native verifier can identify the historical profile.
    let html = text.starts_with("<!doctype html>");
    let payload = if html {
        const MARKER: &str = r#"<script id="gdd-data" type="application/json">"#;
        let Some((_, body)) = text.split_once(MARKER) else {
            return mismatch("Missing HTML view payload.");
        };
        let Some((payload, _)) = body.split_once("</script>") else {
            return mismatch("Incomplete HTML view payload.");
        };
        if body.contains(MARKER) || !body.trim_end().ends_with("</html>") {
            return mismatch("Incomplete or ambiguous HTML view envelope.");
        }
        payload
    } else {
        text
    };
    let data = match parse_json(payload) {
        Ok(Value::Object(data)) => data,
        Ok(_) => return mismatch("Expected a view object."),
        Err(error @ InputError::UnsupportedDepth) => return unsupported(error.to_string()),
        Err(error) => return mismatch(error.to_string()),
    };
    let Some(Value::Object(generator)) = data.get("generator") else {
        return mismatch("Missing or invalid view generator.");
    };
    let (Some(format), Some(name), Some(profile), Some(version)) = (
        string_field(&data, "format"),
        string_field(generator, "name"),
        string_field(generator, "profile"),
        string_field(generator, "version"),
    ) else {
        return mismatch("Missing or invalid view format or generator selector.");
    };
    if html
        && !(matches!(format, "gdd-view/0.2" | "gdd-view/0.3")
            && name == GENERATOR
            && profile == "gdd-viewer-html/0.1")
    {
        return mismatch("Unrecognized HTML view envelope.");
    }
    if (format, name, profile, version) != (FORMAT, GENERATOR, PROFILE, VERSION) {
        return unsupported("Unsupported view format, renderer profile or generator version.");
    }
    let revision = &capture
        .bundle()
        .snapshot()
        .expect("admitted snapshot")
        .revision;
    if string_field(&data, "spec_revision") != Some(revision) {
        return verification_issue(
            CheckResult::Fail,
            "VIEW_REVISION_MISMATCH",
            "View and current model revisions differ.",
        );
    }
    let Some(Value::Array(ids)) = data.get("scope") else {
        return mismatch("Expected a view scope array.");
    };
    let mut scope = Vec::with_capacity(ids.len());
    for id in ids {
        let Value::String(id) = id else {
            return mismatch("Expected string IDs in the view scope.");
        };
        scope.push(id.clone());
    }
    drop(data);
    let expected = match view_data(capture, &scope) {
        Ok(data) => data,
        Err(error)
            if error.get_ref().and_then(|e| e.downcast_ref::<InputError>())
                == Some(&InputError::UnsupportedDepth) =>
        {
            return unsupported(error.to_string());
        }
        Err(error) => return mismatch(error.to_string()),
    };
    let mut encoded = Vec::new();
    if let Err(error) = write_artifact(&mut encoded, &expected) {
        return mismatch(error.to_string());
    }
    if bytes != encoded {
        return mismatch("View bytes differ from the current model projection.");
    }
    let mut check = Check::new(
        CHECK,
        "integration",
        CheckResult::Pass,
        "Exact artifact matches the native renderer and current model snapshot.",
    );
    check.scope = scope;
    check
}

pub(crate) fn verification_check(
    capture: &PerspectiveBundle<impl Borrow<Bundle>>,
    input: Option<ViewInput<'_>>,
) -> Check {
    match input {
        None => verification_issue(
            CheckResult::Unknown,
            "VIEW_INPUT_MISSING",
            "Supply --view for artifact verification.",
        ),
        Some(ViewInput::Bytes(bytes)) => verify_bytes(capture, bytes),
        Some(ViewInput::Path(path)) => match read_view_bytes(path) {
            Ok(bytes) => verify_bytes(capture, &bytes),
            Err(error) => verification_issue(
                CheckResult::Unknown,
                "SOURCE_UNAVAILABLE",
                format!("Cannot read {}: {error}", path.display()),
            ),
        },
    }
}

/// Verify the exact artifact bytes after capturing the source model once.
pub fn verify_view(model: &Path, view: Option<&Path>) -> io::Result<Json> {
    let capture = load_perspective_bundle(model)?;
    Ok(view_report(
        &capture,
        verification_check(&capture, view.map(ViewInput::Path)),
    ))
}

/// Verify saved bytes against an admitted model without reading any files.
pub fn verify_view_captured(
    capture: &PerspectiveBundle<impl Borrow<Bundle>>,
    view: Option<&[u8]>,
) -> Json {
    view_report(
        capture,
        verification_check(capture, view.map(ViewInput::Bytes)),
    )
}
