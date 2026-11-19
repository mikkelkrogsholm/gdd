//! Explicit trusted-caller operations for code snapshots and immutable evidence.

#[cfg(any(feature = "conformance", test))]
#[doc(hidden)]
pub mod process;

use super::{get, inspection, semantics, string, strings};
use crate::bundle::{self, Bundle, DocumentKind, open_regular};
use crate::input;
use crate::report::CheckResult;
use crate::validation::{embedded_schema, to_json};
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

/// File hashes have no prefix. The revision hashes the canonical JSON file map.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CodeSnapshot {
    pub format: &'static str,
    pub files: BTreeMap<String, String>,
    pub revision: String,
}

fn check_resource(name: &str) -> io::Result<()> {
    if name.contains(['\\', '\0']) || name.split('/').any(|p| matches!(p, "" | "." | "..")) {
        return Err(invalid(
            "Resources must be normalized relative POSIX paths.",
        ));
    }
    #[cfg(windows)]
    if name.contains(':') {
        return Err(invalid("Resources cannot name Windows drives or streams."));
    }
    Ok(())
}

/// Hash the selected regular files beneath an existing root, including an empty
/// selection. Resource names use normalized POSIX paths and cannot follow links.
pub fn snapshot_code(root: &Path, files: &[&str]) -> io::Result<CodeSnapshot> {
    let root = root.canonicalize()?;
    if !root.is_dir() {
        return Err(invalid(
            "Select an existing directory as the snapshot root.",
        ));
    }
    let mut names = BTreeSet::new();
    for &name in files {
        check_resource(name)?;
        if !names.insert(name) {
            return Err(invalid("A code snapshot cannot repeat a resource."));
        }
    }
    let mut hashes = BTreeMap::new();
    for name in names {
        let mut file = open_regular(&root, Path::new(name))?;
        let mut hasher = Sha256::new();
        io::copy(&mut file, &mut hasher)?;
        hashes.insert(name.to_owned(), format!("{:x}", hasher.finalize()));
    }
    Ok(CodeSnapshot {
        format: "gdd-code-snapshot/0.1",
        revision: crate::canonical_digest(&json!(hashes)),
        files: hashes,
    })
}

/// The caller independently verifies execution, coverage and every named control.
#[derive(Debug)]
pub struct Attestation<'a> {
    pub root: &'a Path,
    pub implementation: &'a str,
    pub binding_id: &'a str,
    pub evidence_id: &'a str,
    pub test_resource: &'a str,
    pub checker: &'a Value,
    pub environment: &'a BTreeMap<String, String>,
    pub verified_negative_controls: &'a [String],
}

struct CheckPlan<'a> {
    record: &'a input::Value,
    project_id: &'a str,
    spec_revision: String,
}

fn current_plan<'a>(
    bundle: &'a Bundle,
    check_id: &str,
    options: &Attestation<'_>,
) -> io::Result<CheckPlan<'a>> {
    let inspected = inspection::inspect(bundle);
    let model_error = || invalid("Evidence authoring requires a resolved valid model.");
    let manifest = inspected
        .root
        .filter(|d| d.kind.is_manifest())
        .ok_or_else(model_error)?;
    let revision = inspected.revision.ok_or_else(model_error)?;
    if inspected
        .checks
        .iter()
        .any(|c| c.result != CheckResult::Pass)
    {
        return Err(model_error());
    }
    let model = semantics(bundle, manifest, &[]);
    if model.checks.iter().any(|c| c.result != CheckResult::Pass) {
        return Err(model_error());
    }
    let records: BTreeMap<_, _> = bundle.records().map(|r| (r.id, r.value)).collect();
    let check = records
        .get(check_id)
        .filter(|record| {
            string(record, "kind") == "check"
                && string(record, "spec_revision") == revision
                && !model
                    .invalid_records(&records, Some(&revision))
                    .contains(check_id)
        })
        .ok_or_else(|| invalid("Select a valid current immutable check plan."))?;
    let model_ids = model.model_id_counts();
    if options.binding_id == options.evidence_id
        || [options.binding_id, options.evidence_id]
            .iter()
            .any(|id| records.contains_key(id) || model_ids.contains_key(id))
    {
        return Err(invalid(
            "Use new distinct record IDs; historical IDs are immutable.",
        ));
    }
    Ok(CheckPlan {
        record: check,
        spec_revision: revision,
        project_id: string(
            get(&manifest.value, "project").expect("validated project"),
            "id",
        ),
    })
}

fn parent_directory(path: &Path) -> io::Result<PathBuf> {
    path.parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
        .canonicalize()
}

fn document(file: &mut File) -> io::Result<Value> {
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    let value = input::parse(&bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    Ok(to_json(&value)?)
}

// The 0.1 Python reader compares exit_code numerically, including bools.
// Keep decimal and exponent inputs exact when testing for zero or one.
fn receipt_exit_code(value: &Value) -> Option<u8> {
    match value {
        Value::Bool(status) => Some(u8::from(*status)),
        Value::Number(number) => {
            match super::canonical::comparison_number(number.as_str()).as_str() {
                "0" | "0.0" | "-0.0" => Some(0),
                "1" | "1.0" => Some(1),
                _ => None,
            }
        }
        _ => None,
    }
}

fn checked_receipt(root: &Path, path: &Path) -> io::Result<(&'static str, CodeSnapshot)> {
    // A caller-selected receipt may have an alias; its supplied locator is
    // retained in evidence. A resolved destination must still be a regular file.
    let resolved = path.canonicalize()?;
    let (Some(parent), Some(name)) = (resolved.parent(), resolved.file_name()) else {
        return Err(invalid("Select a regular receipt file."));
    };
    let receipt = document(&mut open_regular(parent, Path::new(name))?)?;
    let outcome = match (
        receipt["format"].as_str(),
        receipt["executed"].as_bool(),
        receipt["result"].as_str(),
        receipt_exit_code(&receipt["exit_code"]),
    ) {
        (Some("gdd-host-check/0.1"), Some(true), Some("PASS"), Some(0)) => "PASS",
        (Some("gdd-host-check/0.1"), Some(true), Some("FAIL"), Some(1)) => "FAIL",
        _ => {
            return Err(invalid(
                "A completed actual PASS/FAIL host check is required.",
            ));
        }
    };
    let stale = || invalid("The selected code snapshot is stale or unresolved.");
    let files = receipt["snapshot"]["files"].as_object().ok_or_else(stale)?;
    let names: Vec<_> = files.keys().map(String::as_str).collect();
    let current = snapshot_code(root, &names)?;
    if receipt["snapshot"] != json!(current)
        || receipt["code_revision"] != current.revision
        || receipt["code_revision_after"] != current.revision
    {
        return Err(stale());
    }
    Ok((outcome, current))
}

fn resource_revision(snapshot: &CodeSnapshot, resource: &str) -> io::Result<String> {
    let resource = resource.trim_end_matches('/');
    check_resource(resource)?;
    if let Some(hash) = snapshot.files.get(resource) {
        return Ok(format!("sha256:{hash}"));
    }
    let directory = format!("{resource}/");
    let selected: BTreeMap<_, _> = snapshot
        .files
        .iter()
        .filter(|(name, _)| name.starts_with(&directory))
        .collect();
    if selected.is_empty() {
        return Err(invalid(
            "The test resource must be included in the checked code snapshot.",
        ));
    }
    Ok(crate::canonical_digest(&json!(selected)))
}

fn validate_ledger(document: &Value) -> io::Result<()> {
    embedded_schema(DocumentKind::Records)
        .expect("embedded records schema")
        .validate(document)
        .map_err(|e| invalid(&format!("Invalid records document: {e}")))
}

/// Author a binding and evidence pair for a current immutable check, together
/// with the caller's explicit host-context fragment. Accepts existing
/// `gdd-host-check/0.1` receipts. The caller is responsible for their authenticity.
pub fn verification_records(
    model: &Path,
    check_id: &str,
    receipt_path: &Path,
    options: &Attestation<'_>,
) -> io::Result<Value> {
    let bundle = bundle::load_for_inspection(model);
    let plan = current_plan(&bundle, check_id, options)?;
    let (result, snapshot) = checked_receipt(options.root, receipt_path)?;
    let mut environment = BTreeMap::new();
    for name in strings(plan.record, "required_environment") {
        let value = options
            .environment
            .get(name)
            .ok_or_else(|| invalid("Supply the actual declared checker environment."))?;
        environment.insert(name, value);
    }
    let controls: BTreeSet<_> = options
        .verified_negative_controls
        .iter()
        .map(String::as_str)
        .collect();
    if controls.contains("")
        || strings(plan.record, "negative_controls").any(|s| !controls.contains(s))
    {
        return Err(invalid(
            "The host must explicitly attest every performed negative control required by the plan.",
        ));
    }
    let check = to_json(plan.record)?;
    let binding = json!({
        "kind": "binding", "spec_revision": plan.spec_revision, "targets": check["scope"],
        "implementation": options.implementation, "role": "test", "resource": options.test_resource,
        "selector": {"kind": "whole-document"}, "status": "checked",
        "revision": resource_revision(&snapshot, options.test_resource)?, "code_revision": snapshot.revision,
    });
    let evidence = json!({
        "kind": "evidence", "check": check_id, "check_revision": crate::canonical_digest(&check),
        "implementation": options.implementation, "spec_revision": plan.spec_revision,
        "code_revision": snapshot.revision, "checker": options.checker, "scope": check["scope"],
        "result": result, "executed": true,
        "reason": "Actual host-selected execution; coverage and controls explicitly attested by its caller.",
        "environment": environment, "evidence_uri": crate::receipt_path(receipt_path)?,
    });
    let records = json!({ (options.binding_id): binding, (options.evidence_id): evidence });
    validate_ledger(
        &json!({"format": "gdd-records/0.2", "project_id": plan.project_id, "records": records}),
    )?;
    Ok(json!({"records": records, "host": {
        "implementation": options.implementation, "code_revision": snapshot.revision,
        "verified_bindings": [options.binding_id], "trusted_evidence": [options.evidence_id],
        "supported_methods": [check["method"]], "environment": options.environment,
        "negative_controls": options.verified_negative_controls, "evidence_selection": {(check_id): options.evidence_id},
    }}))
}

/// Append schema-valid new record IDs under an exclusive sibling `.lock`.
/// The complete validated ledger replaces the original atomically.
pub fn append_records(path: &Path, fragment: &Value) -> io::Result<()> {
    if !fs::symlink_metadata(path)?.is_file() {
        return Err(invalid("Select an existing regular records document."));
    }
    let additions = fragment["records"]
        .as_object()
        .filter(|a| !a.is_empty())
        .ok_or_else(|| invalid("Supply a nonempty records fragment."))?;
    let parent = parent_directory(path)?;
    let name = path
        .file_name()
        .ok_or_else(|| invalid("Select a records filename."))?;
    let mut lock_name = name.to_owned();
    lock_name.push(".lock");
    let lock = parent.join(lock_name);
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    drop(options.open(&lock)?);
    let lock = tempfile::TempPath::try_from_path(lock)?;
    let mut file = open_regular(&parent, Path::new(name))?;
    let permissions = file.metadata()?.permissions();
    #[cfg(unix)]
    let permissions = {
        use std::os::unix::fs::PermissionsExt;
        fs::Permissions::from_mode(permissions.mode() & 0o777)
    };
    let mut ledger = document(&mut file)?;
    drop(file);
    validate_ledger(&ledger)?;
    let records = ledger["records"].as_object_mut().expect("validated ledger");
    if additions.keys().any(|id| records.contains_key(id)) {
        return Err(invalid(
            "Existing record IDs are immutable; choose new IDs.",
        ));
    }
    records.extend(additions.clone());
    validate_ledger(&ledger)?;
    input::check_output_depth(&ledger).map_err(io::Error::other)?;

    let mut staging = tempfile::NamedTempFile::new_in(&parent)?;
    {
        let mut writer = BufWriter::new(staging.as_file_mut());
        serde_json::to_writer_pretty(&mut writer, &ledger)?;
        writeln!(writer)?;
        writer.flush()?;
    }
    staging.as_file().set_permissions(permissions)?;
    staging.into_temp_path().persist(parent.join(name))?;
    lock.close()
}
