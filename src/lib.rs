//! GDD document input, project initialization, and portable snapshot revisions.

pub mod bundle;
pub mod graphify;
pub mod input;
mod model;
pub use model::canonical::digest as canonical_digest;
pub use model::workflow::run as workflow_run;
pub use model::{
    boundaries, code_check, context, design_review, host, impact, model_diff, perspectives,
    workflow,
};
pub mod projection;
pub mod python_imports;
pub mod report;
pub mod status;
pub mod validation;
pub mod view;

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{self, Write};
use std::path::Path;

const MODEL: &str = include_str!("../templates/project/gdd.json");
const RECORDS: &str = include_str!("../templates/project/records.json");
const SOURCES: &[u8] = include_bytes!("../templates/project/SOURCES.md");
const FILES: [&str; 3] = ["SOURCES.md", "gdd.json", "records.json"];

pub const DEFAULT_PROJECT_ID: &str = "project.new";
pub const DEFAULT_TITLE: &str = "New GDD project";

/// Receipt for the embedded, independently verified starter draft.
#[derive(Debug, Serialize)]
pub struct InitReceipt {
    pub format: &'static str,
    pub path: String,
    pub manifest: String,
    pub project_id: String,
    pub files: [&'static str; 3],
    pub spec_revision: String,
    pub model: &'static str,
    pub readiness: &'static str,
}

/// Publish a new draft in an existing directory, without replacing its final name.
pub fn init_project(path: &Path, project_id: &str, title: &str) -> io::Result<InitReceipt> {
    if !valid_id(project_id) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Project ID must be a GDD ID string.",
        ));
    }
    // Python str.strip also recognizes the four ASCII information separators.
    if title
        .trim_matches(|ch: char| ch.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&ch))
        .is_empty()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Project title must be a nonempty string.",
        ));
    }
    if path.as_os_str().as_encoded_bytes().contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Embedded NUL in project path.",
        ));
    }

    let (model, records) = draft_documents(project_id, title)?;
    let spec_revision = snapshot_revision(&[("gdd.json", &sha256(&model))])?;

    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    match absolute.symlink_metadata() {
        Ok(_) => return Err(already_exists(&absolute)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let (Some(parent), Some(name)) = (absolute.parent(), absolute.file_name()) else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Project path must name a new directory.",
        ));
    };
    let parent = parent.canonicalize()?;
    let output = parent.join(name);
    let receipt = InitReceipt {
        format: "gdd-init/0.1",
        path: receipt_path(&output)?,
        manifest: receipt_path(&output.join("gdd.json"))?,
        project_id: project_id.into(),
        files: FILES,
        spec_revision,
        model: "PASS",
        readiness: "FAIL",
    };

    let mut staging = tempfile::Builder::new();
    staging.prefix(".gdd-init-");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        staging.permissions(std::fs::Permissions::from_mode(0o700));
    }
    let staging = staging.tempdir_in(parent)?;
    for (name, bytes) in FILES.into_iter().zip([SOURCES, &model, &records]) {
        File::create_new(staging.path().join(name))?.write_all(bytes)?;
    }
    match rename_exclusive(staging.path(), &output) {
        Ok(()) => {
            // The staging path no longer belongs to this invocation.
            let _ = staging.keep();
            Ok(receipt)
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Err(already_exists(&output)),
        Err(error) => Err(error),
    }
}

fn valid_id(id: &str) -> bool {
    // See schemas/gdd.schema.json project.id and schemas/records.schema.json project_id.
    id.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
        && id.split(['.', '-']).all(|segment| {
            !segment.is_empty()
                && segment
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        })
}

fn draft_documents(project_id: &str, title: &str) -> io::Result<(Vec<u8>, Vec<u8>)> {
    // These fingerprints restrict the receipt to the model-valid, unready draft
    // checked by the Python validator. Recheck both profiles after a template
    // change before updating these fingerprints.
    if sha256(MODEL.as_bytes())
        != "dbe52bd46cedc53c9ef75547e45efec6e049e7bdfba415c9b474fc1511e36471"
        || sha256(RECORDS.as_bytes())
            != "7e9aa33c455e514b1a6f5b8bd4bbd28f50c9ee66a349325e85ccd04545fa669b"
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Starter templates differ from the verified GDD draft.",
        ));
    }

    let mut model: Value = serde_json::from_str(MODEL)?;
    model["project"]["id"] = project_id.into();
    model["project"]["title"] = title.into();

    let mut records: Value = serde_json::from_str(RECORDS)?;
    records["project_id"] = project_id.into();

    let mut model = serde_json::to_vec_pretty(&model)?;
    model.push(b'\n');
    let mut records = serde_json::to_vec_pretty(&records)?;
    records.push(b'\n');
    Ok((model, records))
}

/// Compute a `gdd-snapshot/0.2` revision from normalized local asset paths and
/// lowercase SHA-256 hashes. The caller determines which assets are normative.
pub fn snapshot_revision(assets: &[(&str, &str)]) -> io::Result<String> {
    let mut sorted = assets.to_vec();
    sorted.sort_unstable_by_key(|pair| pair.0);
    if sorted.is_empty()
        || sorted.iter().any(|(path, hash)| {
            path.split('/').any(|part| matches!(part, "" | "." | ".."))
                || path.contains(['\\', ':', '\0'])
                || hash.len() != 64
                || !hash
                    .bytes()
                    .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
        })
        || sorted.windows(2).any(|pair| pair[0].0 == pair[1].0)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Snapshot assets must have unique normalized paths and lowercase SHA-256 hashes.",
        ));
    }
    let compact = serde_json::to_vec(&sorted)?;
    Ok(format!("sha256:{}", sha256(&compact)))
}

/// Lowercase SHA-256 hash of the supplied asset bytes.
pub fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn already_exists(path: &Path) -> io::Error {
    io::Error::new(
        io::ErrorKind::AlreadyExists,
        format!("Project bundle already exists: {}", path.display()),
    )
}

fn receipt_path(path: &Path) -> io::Result<String> {
    let path = path.to_str().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "Project path must be valid Unicode.",
        )
    })?;
    #[cfg(windows)]
    {
        // Standard drive and UNC paths in the receipt match Python Path.resolve.
        if let Some(unc) = path.strip_prefix(r"\\?\UNC\") {
            return Ok(format!(r"\\{unc}"));
        }
        if let Some(drive) = path.strip_prefix(r"\\?\") {
            if drive.as_bytes().get(1) == Some(&b':') {
                return Ok(drive.into());
            }
        }
    }
    Ok(path.into())
}

/// Publish staged conformance output without replacing an existing destination.
#[cfg(feature = "conformance")]
pub fn publish_exclusive(source: &Path, destination: &Path) -> io::Result<()> {
    rename_exclusive(source, destination)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn rename_exclusive(source: &Path, destination: &Path) -> io::Result<()> {
    use rustix::fs::{CWD, RenameFlags, renameat_with};

    Ok(renameat_with(
        CWD,
        source,
        CWD,
        destination,
        RenameFlags::NOREPLACE,
    )?)
}

#[cfg(windows)]
fn rename_exclusive(source: &Path, destination: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::MoveFileExW;

    if [source, destination]
        .iter()
        .any(|path| path.as_os_str().encode_wide().any(|ch| ch == 0))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Embedded NUL in rename path.",
        ));
    }
    let source: Vec<_> = source.as_os_str().encode_wide().chain([0]).collect();
    let destination: Vec<_> = destination.as_os_str().encode_wide().chain([0]).collect();
    // SAFETY: Both arrays are NUL-terminated and remain live for the call.
    // Zero flags disables replacing the destination or copying across volumes.
    if unsafe { MoveFileExW(source.as_ptr(), destination.as_ptr(), 0) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
fn rename_exclusive(_source: &Path, _destination: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "Atomic exclusive rename is unavailable on this platform.",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_digest_matches_the_portable_profile() {
        // Expected digest computed with gdd_validator.loading.digest.
        let zero = "0".repeat(64);
        let one = "1".repeat(64);
        let two = "2".repeat(64);
        let three = "3".repeat(64);
        let four = "4".repeat(64);
        let assets = [
            ("é/🦀.md", one.as_str()),
            ("a", &zero),
            ("\u{10000}", &three),
            ("\u{e000}", &two),
            ("a/quote\"\n.md", &four),
        ];
        assert_eq!(
            snapshot_revision(&assets).unwrap(),
            "sha256:116122a4294ddcbb0c65f9b2b20acb4bbe501138bd66a0eaaf0149e02dc44681"
        );
        assert!(snapshot_revision(&[("a", &zero), ("a", &one)]).is_err());
        assert!(snapshot_revision(&[("a/../b", &zero)]).is_err());
        assert!(snapshot_revision(&[("a", "invalid hash")]).is_err());
        assert!(snapshot_revision(&[]).is_err());
    }

    #[cfg(any(target_os = "linux", target_os = "macos", windows))]
    #[test]
    fn exclusive_rename_refuses_a_competing_empty_directory() {
        let root = tempfile::tempdir().unwrap();
        let staging = root.path().join("staging");
        let output = root.path().join("output");
        std::fs::create_dir(&staging).unwrap();
        std::fs::write(staging.join("gdd.json"), "draft").unwrap();

        // Simulate the destination appearing after the initial existence check.
        assert!(!output.exists());
        std::fs::create_dir(&output).unwrap();
        assert!(rename_exclusive(&staging, &output).is_err());
        assert_eq!(output.read_dir().unwrap().count(), 0);
        assert!(staging.join("gdd.json").is_file());
    }
}
