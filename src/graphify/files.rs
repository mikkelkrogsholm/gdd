//! Capture, verify and exclusively publish observation bundles.

use super::{
    GraphifyOptions, IMPORTER, ImportReport, RAW, VERSION, compact_observations, decode,
    import_report, observations_schema, observe_graphify,
};
use crate::bundle::{read_regular, resolve_local};
use crate::model::canonical::comparison_number;
use crate::report::CheckResult::{Fail, Pass, Unknown};
use crate::report::{Check, CheckResult, Diagnostic};
use json_event_parser::{JsonEvent, SliceJsonParser};
use serde::Serialize;
use serde_json::Value;
use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};

const MANIFEST: &str = "observations.json";
const FORMAT: &str = "gdd-observations/0.2";
// The index wraps producer values and historical full indexes can be deeper
// than a current raw projection. Bound the size of any recursively owned tree.
const MAX_MANIFEST_DEPTH: usize = 1024;

/// Captured evidence for one assessment. An unreadable or unsupported manifest
/// has no index. An unavailable original has no raw bytes.
#[derive(Debug)]
pub struct LoadedObservations {
    pub index: Option<Value>,
    pub raw: Option<Vec<u8>>,
    pub report: ImportReport,
}

struct Issue(CheckResult, &'static str, String);

impl Issue {
    fn check(self, name: &str) -> Check {
        let Self(result, code, reason) = self;
        let diagnostic = Diagnostic::new(code, result, &reason, None, "");
        let mut check = Check::new(format!("graphify.{name}"), "integration", result, reason);
        check.diagnostics.push(diagnostic);
        check
    }

    fn report(self, name: &str) -> ImportReport {
        import_report(vec![self.check(name)])
    }

    fn unavailable(self, name: &str, index: Option<Value>) -> LoadedObservations {
        LoadedObservations {
            index,
            raw: None,
            report: self.report(name),
        }
    }
}

fn capture(path: &Path, root: Option<&Path>) -> Result<(PathBuf, Vec<u8>), Issue> {
    let path = resolve_local(path)
        .map_err(|e| Issue(e.outcome.into(), e.code, e.message.into_string()))?;
    let root = root.or_else(|| path.parent()).unwrap_or(&path);
    let relative = path.strip_prefix(root).map_err(|_| {
        Issue(
            Fail,
            "ASSET_PATH_INVALID",
            format!("Asset escapes bundle root: {}", path.display()),
        )
    })?;
    let bytes = read_regular(root, relative).map_err(|e| {
        Issue(
            Unknown,
            "SOURCE_UNAVAILABLE",
            format!("Cannot read {}: {e}", path.display()),
        )
    })?;
    Ok((path, bytes))
}

fn file_uri(path: &Path) -> io::Result<String> {
    #[cfg(windows)]
    let windows_path = crate::receipt_path(path)?.replace('\\', "/");
    #[cfg(windows)]
    let (mut uri, bytes) = if let Some(unc) = windows_path.strip_prefix("//") {
        (String::from("file://"), unc.as_bytes())
    } else {
        (
            format!("file:///{}", &windows_path[..2]),
            &windows_path.as_bytes()[2..],
        )
    };
    #[cfg(not(windows))]
    let (mut uri, bytes) = (String::from("file://"), path.as_os_str().as_encoded_bytes());
    const HEX: &[u8] = b"0123456789ABCDEF";
    for &b in bytes {
        if b.is_ascii_alphanumeric() || b"/-._~".contains(&b) {
            uri.push(char::from(b));
        } else {
            uri.push('%');
            uri.push(char::from(HEX[(b >> 4) as usize]));
            uri.push(char::from(HEX[(b & 15) as usize]));
        }
    }
    Ok(uri)
}

fn decode_manifest(bytes: &[u8]) -> Result<Value, Issue> {
    match decode::decode(bytes, MAX_MANIFEST_DEPTH) {
        Ok((value, false)) => Ok(value),
        Ok((_, true)) => Err(Issue(
            Unknown,
            "FORMAT_UNSUPPORTED",
            format!("Observations index exceeds the {MAX_MANIFEST_DEPTH}-container reading limit."),
        )),
        Err(reason) => Err(Issue(Fail, "SCHEMA_INVALID", reason)),
    }
}

/// Compare sorted-object JSON semantics, including the distinct canonical
/// spellings for integer 1, decimal 1.0, positive zero and negative decimal zero.
pub(crate) fn same_projection(left: &Value, right: &Value) -> bool {
    let mut pairs = vec![(left, right)];
    while let Some((left, right)) = pairs.pop() {
        match (left, right) {
            (Value::Object(a), Value::Object(b)) if a.len() == b.len() => {
                for (key, value) in a {
                    let Some(other) = b.get(key) else {
                        return false;
                    };
                    pairs.push((value, other));
                }
            }
            (Value::Array(a), Value::Array(b)) if a.len() == b.len() => {
                pairs.extend(a.iter().zip(b));
            }
            (Value::Number(a), Value::Number(b)) => {
                if a != b && comparison_number(a.as_str()) != comparison_number(b.as_str()) {
                    return false;
                }
            }
            (Value::Null, Value::Null) => {}
            (Value::Bool(a), Value::Bool(b)) if a == b => {}
            (Value::String(a), Value::String(b)) if a == b => {}
            _ => return false,
        }
    }
    true
}

// Only the historical Python compact format needs this comparison. Fresh
// observations use the exact numbers from the JSON event parser throughout.
fn legacy_number_supported(token: &str) -> bool {
    if !token.contains(['.', 'e', 'E']) {
        // Default Python integer string conversion limit.
        return token.trim_start_matches('-').len() <= 4300;
    }
    let Some(rounded) = token
        .parse::<f64>()
        .ok()
        .and_then(serde_json::Number::from_f64)
    else {
        return false;
    };
    let exact = comparison_number(token);
    if exact != comparison_number(rounded.as_str()) {
        return false;
    }
    if matches!(exact.as_str(), "0.0" | "-0.0")
        && let Some((mantissa, exp)) = token.split_once(['e', 'E'])
    {
        // Decimal decoded the exponent even for zero. Its 64-bit context
        // admits stored exponents from MIN_ETINY through MAX_EMAX.
        let fraction = mantissa.split_once('.').map_or(0, |(_, f)| f.len());
        return exp
            .parse::<i128>()
            .ok()
            .and_then(|e| e.checked_sub(fraction as i128))
            .is_some_and(|e| (-1_999_999_999_999_999_997..=999_999_999_999_999_999).contains(&e));
    }
    true
}

fn legacy_numeric_limit(raw: &[u8]) -> bool {
    let mut parser = SliceJsonParser::new(raw);
    loop {
        match parser.parse_next() {
            Ok(JsonEvent::Number(token)) if !legacy_number_supported(&token) => return true,
            Ok(JsonEvent::Eof) | Err(_) => return false,
            _ => {}
        }
    }
}

fn assess_index(index: &Value, raw: &[u8]) -> io::Result<ImportReport> {
    let importer = &index["importer"];
    let claims = &index["input_claims"];
    let (mut expected, mut report) = observe_graphify(
        raw,
        GraphifyOptions {
            profile: importer["profile"].as_str(),
            producer_version: claims["producer_version"].as_str(),
            code_revision: claims["code_revision"].as_str(),
            source_id: index["source_id"].as_str().unwrap(),
            locator: claims["locator"].as_str().unwrap(),
        },
    )?;
    expected["importer"]["version"] = importer["version"].clone();
    // JSON Schema also allows integral decimals for byte_count. Integrity
    // measures the length; typed canonical comparison separately checks spelling.
    let count = index["artifact"]["byte_count"].as_number().unwrap();
    let count_matches = count.as_u64() == Some(raw.len() as u64) || {
        let decimal = comparison_number(count.as_str());
        decimal == comparison_number(&format!("{}.0", raw.len()))
            || raw.is_empty() && matches!(decimal.as_str(), "0" | "-0.0")
    };
    if index["artifact"]["revision"] == expected["artifact"]["revision"] && count_matches {
        report.result(
            "integrity",
            Pass,
            "Original bytes match the recorded SHA-256 and length.",
            None,
        );
    } else {
        report.checks.push(
            Issue(
                Fail,
                "IMPORT_LOSS_DETECTED",
                "Original bytes no longer match their recorded artifact revision/length.".into(),
            )
            .check("integrity"),
        );
    }
    let same = same_projection(&expected, index);
    let current = importer["version"] == VERSION;
    let deep = report
        .checks
        .iter()
        .any(|c| c.check_id == "graphify.depth" && c.result == Unknown);
    let issue = if deep && (same || !current) {
        Some(Issue(Unknown, "FORMAT_UNSUPPORTED", "Raw JSON exceeds supported projection depth; producer records are not verified, including historical full indexes.".into()))
    } else if !same
        && same_projection(&compact_observations(&expected), index)
        && (current || legacy_numeric_limit(raw))
    {
        Some(Issue(Unknown, "FORMAT_UNSUPPORTED", "Only the compact byte-identity envelope is present; producer records are not asserted.".into()))
    } else if !same {
        Some(Issue(Fail, "IMPORT_LOSS_DETECTED", "Raw-derived index differs from a fresh projection of the original snapshot. Caller assertions remain unverified.".into()))
    } else {
        None
    };
    if let Some(issue) = issue {
        report.checks.push(issue.check("projection"));
    } else {
        report.result("projection", Pass, "Raw-derived records, pointers, classifications and producer claims match a fresh projection. Caller assertions are inputs to this projection, not authenticated metadata.", None);
    }
    Ok(report)
}

fn load_index(manifest: &Path, decoded: Result<Value, Issue>) -> io::Result<LoadedObservations> {
    let index = match decoded {
        Ok(index) if index["format"] == FORMAT => index,
        Ok(_) => {
            return Ok(Issue(
                Unknown,
                "FORMAT_UNSUPPORTED",
                "Unsupported observations format.".into(),
            )
            .unavailable("manifest", None));
        }
        Err(issue) => return Ok(issue.unavailable("manifest", None)),
    };
    if let Err(error) = observations_schema().validate(&index) {
        return Ok(Issue(
            Fail,
            "SCHEMA_INVALID",
            format!(
                "Observation index at '{}' violates schema constraint '{}'.",
                error.instance_path(),
                error.schema_path()
            ),
        )
        .unavailable("manifest", None));
    }
    if index["importer"]["name"] != IMPORTER
        || !matches!(
            index["importer"]["version"].as_str(),
            Some("0.1.0" | "0.1.1" | VERSION)
        )
    {
        return Ok(Issue(
            Unknown,
            "FORMAT_UNSUPPORTED",
            "Unsupported importer version; original file is retained.".into(),
        )
        .unavailable("manifest", None));
    }
    let root = manifest.parent().expect("captured manifest names a file");
    // The schema requires precisely RAW. Resolve its actual target and enforce
    // containment even when a valid-looking local name is a symlink.
    let raw = match capture(&root.join(RAW), Some(root)) {
        Ok((_, bytes)) => bytes,
        Err(issue) => return Ok(issue.unavailable("raw", Some(index))),
    };
    let report = assess_index(&index, &raw)?;
    Ok(LoadedObservations {
        index: Some(index),
        raw: Some(raw),
        report,
    })
}

/// Read a directory's observations.json or an explicitly named index.
pub fn load_observations(path: &Path) -> io::Result<LoadedObservations> {
    let manifest = if path.is_dir() {
        path.join(MANIFEST)
    } else {
        path.to_owned()
    };
    match capture(&manifest, None) {
        Ok((path, bytes)) => load_index(&path, decode_manifest(&bytes)),
        Err(issue) => Ok(issue.unavailable("manifest", None)),
    }
}

fn manifest_report(
    loaded: LoadedObservations,
    profile: Option<&str>,
    producer_version: Option<&str>,
) -> ImportReport {
    let LoadedObservations {
        index, mut report, ..
    } = loaded;
    if let Some(index) = index {
        if profile.is_some_and(|p| index["importer"]["profile"] != p) {
            report.checks.push(
                Issue(
                    Unknown,
                    "FORMAT_UNSUPPORTED",
                    "Requested profile differs from the bundle's recorded profile.".into(),
                )
                .check("profile"),
            );
        }
        if producer_version.is_some_and(|v| {
            v != "0.9.75"
                || index["input_claims"]["producer_version"]
                    .as_str()
                    .is_some_and(|recorded| v != recorded)
        }) {
            report.checks.push(Issue(Unknown, "FORMAT_UNSUPPORTED", "Requested producer version is unsupported or conflicts with the recorded claim.".into()).check("version"));
        }
    }
    report
}

/// Verify a bundle or interpret a captured raw file with the selected profile.
pub fn verify_graphify(
    path: &Path,
    profile: Option<&str>,
    producer_version: Option<&str>,
) -> io::Result<ImportReport> {
    let directory = path.is_dir();
    let required_manifest =
        directory || (path.file_name().is_some_and(|n| n == MANIFEST) && profile.is_none());
    let manifest = if directory {
        path.join(MANIFEST)
    } else {
        path.to_owned()
    };
    let (path, bytes) = match capture(&manifest, None) {
        Ok(captured) => captured,
        Err(issue) => return Ok(issue.report(if required_manifest { "manifest" } else { "raw" })),
    };
    let decoded = decode_manifest(&bytes);
    if required_manifest || decoded.as_ref().is_ok_and(|v| v["format"] == FORMAT) {
        return Ok(manifest_report(
            load_index(&path, decoded)?,
            profile,
            producer_version,
        ));
    }
    drop(decoded);
    let (_, report) = observe_graphify(
        &bytes,
        GraphifyOptions {
            profile,
            producer_version,
            locator: &file_uri(&path)?,
            ..Default::default()
        },
    )?;
    Ok(report)
}

fn output_parent(path: &Path) -> &Path {
    path.parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
}

fn require_unused(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(_) => Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("Output already exists: {}", path.display()),
        )),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn write_json(path: &Path, value: &impl Serialize) -> io::Result<()> {
    let mut writer = BufWriter::new(File::create_new(path)?);
    serde_json::to_writer_pretty(&mut writer, value)?;
    writeln!(writer)?;
    writer.flush()
}

/// Capture a raw file, record its resolved file URI and atomically publish an
/// exclusive bundle. The file URI replaces the in-memory options' locator.
pub fn import_graphify(
    path: &Path,
    output: &Path,
    options: GraphifyOptions<'_>,
) -> io::Result<ImportReport> {
    let (path, raw) = match capture(path, None) {
        Ok(captured) => captured,
        Err(issue) => return Ok(issue.report("raw")),
    };
    let (index, _) = observe_graphify(
        &raw,
        GraphifyOptions {
            locator: &file_uri(&path)?,
            ..options
        },
    )?;
    require_unused(output)?;
    let parent = output_parent(output);
    fs::create_dir_all(parent)?;
    let mut staging = tempfile::Builder::new();
    staging.prefix(".gdd-graphify-");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        staging.permissions(fs::Permissions::from_mode(0o700));
    }
    let staging = staging.tempdir_in(parent)?;
    fs::write(staging.path().join(RAW), raw)?;
    write_json(&staging.path().join(MANIFEST), &index)?;
    drop(index);
    let loaded = load_observations(staging.path())?;
    if !recoverable(&loaded) {
        return Err(io::Error::other(
            "Could not verify the staged observation bundle.",
        ));
    }
    write_json(&staging.path().join("import.report.json"), &loaded.report)?;
    fs::write(
        staging.path().join("README.md"),
        include_bytes!("../../examples/graphify/observations/README.md"),
    )?;
    crate::rename_exclusive(staging.path(), output)?;
    let _ = staging.keep();
    Ok(loaded.report)
}

fn recoverable(loaded: &LoadedObservations) -> bool {
    loaded.raw.is_some()
        && !loaded.report.checks.iter().any(|c| {
            c.result == Fail
                && matches!(
                    c.check_id.as_str(),
                    "graphify.manifest" | "graphify.integrity" | "graphify.projection"
                )
        })
}

/// Exclusively recover the captured original, retaining the producer verdict
/// even when its layout or semantics failed.
pub fn export_graphify(path: &Path, output: &Path) -> io::Result<ImportReport> {
    let loaded = load_observations(path)?;
    if !recoverable(&loaded) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Cannot export a corrupt/unavailable observation bundle.",
        ));
    }
    publish_bytes(output, loaded.raw.as_deref().unwrap())?;
    Ok(loaded.report)
}

fn publish_bytes(output: &Path, bytes: &[u8]) -> io::Result<()> {
    require_unused(output)?;
    let mut builder = tempfile::Builder::new();
    builder.prefix(".gdd-graphify-");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        builder.permissions(fs::Permissions::from_mode(0o666));
    }
    let mut staged = builder.tempfile_in(output_parent(output))?;
    staged.write_all(bytes)?;
    staged.flush()?;
    let mut temporary = staged.into_temp_path();
    crate::rename_exclusive(&temporary, output)?;
    temporary.disable_cleanup(true);
    Ok(())
}

// Return a POSIX locator from the resolved output directory to the original.
fn relative_source(source: &Path, parent: &Path) -> io::Result<String> {
    for (levels, ancestor) in parent.ancestors().enumerate() {
        if let Ok(rest) = source.strip_prefix(ancestor) {
            let mut parts = vec![".."; levels];
            for part in rest.components() {
                parts.push(part.as_os_str().to_str().ok_or_else(|| {
                    io::Error::other("Proposal source path must be valid Unicode.")
                })?);
            }
            return Ok(parts.join("/"));
        }
    }
    Err(io::Error::other(
        "Cannot express the original relative to the proposal directory.",
    ))
}

/// Publish an explicit proposal exclusively and return the same envelope. Its
/// source URI is relative to the output directory, including through aliases.
pub fn propose_graphify_to_file(
    path: &Path,
    mapping: &Value,
    output: &Path,
    format: &str,
) -> io::Result<Value> {
    let manifest = if path.is_dir() {
        path.join(MANIFEST)
    } else {
        path.to_owned()
    };
    let manifest = resolve_local(&manifest).map_err(|e| io::Error::other(e.to_string()))?;
    let root = manifest
        .parent()
        .ok_or_else(|| io::Error::other("Expected an observations manifest."))?;
    let parent = output_parent(output).canonicalize()?;
    let name = output
        .file_name()
        .ok_or_else(|| io::Error::other("Expected a new proposal filename."))?;
    let uri = relative_source(&root.join(RAW), &parent)?;
    let proposal = super::propose_graphify(&manifest, mapping, &uri, format)?;
    let mut bytes = serde_json::to_vec_pretty(&proposal)?;
    bytes.push(b'\n');
    publish_bytes(&parent.join(name), &bytes)?;
    Ok(proposal)
}
