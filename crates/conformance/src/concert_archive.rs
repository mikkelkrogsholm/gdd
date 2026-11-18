//! Deterministic construction and verified recovery of concert-archive/1 bundles.

mod build;
mod members;

use anyhow::{Context, Result, bail, ensure};
use clap::{Args, Subcommand};
use flate2::read::MultiGzDecoder;
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{self, Read, Write};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const FORMAT: &str = "concert-archive/1";

#[derive(Args)]
pub struct Inputs {
    #[arg(long)]
    archive: PathBuf,
    #[arg(long)]
    index: PathBuf,
}

#[derive(Subcommand)]
pub enum Action {
    /// Build a deterministic archive and index from a committed Git tree.
    Build {
        #[arg(long, value_name = "DIR")]
        repository: PathBuf,
        #[arg(long, value_name = "REV")]
        revision: String,
        #[arg(long, default_value = "examples/concert-app", value_name = "PATH")]
        source_path: String,
        #[arg(long, value_name = "NEW_PATH")]
        archive: PathBuf,
        #[arg(long, value_name = "NEW_PATH")]
        index: PathBuf,
    },
    /// Verify the compressed bytes and the complete indexed Git file set.
    Check(Inputs),
    /// Restore verified regular files into a new directory.
    Restore {
        #[command(flatten)]
        inputs: Inputs,
        #[arg(long, value_name = "NEW_DIR")]
        output: PathBuf,
    },
}

pub fn command(action: Action) -> Result<ExitCode> {
    let receipt = match action {
        Action::Build {
            repository,
            revision,
            source_path,
            archive,
            index,
        } => build::build(&repository, &revision, &source_path, &archive, &index)?,
        Action::Check(inputs) => verified_files(&inputs.archive, &inputs.index)?.receipt,
        Action::Restore { inputs, output } => restore(&inputs.archive, &inputs.index, &output)?,
    };
    println!("{}", serde_json::to_string_pretty(&receipt)?);
    Ok(ExitCode::SUCCESS)
}

#[derive(Deserialize, Serialize)]
struct Index {
    format: String,
    source: Source,
    archive: Compressed,
    files: Vec<IndexedFile>,
}

#[derive(Deserialize, Serialize)]
struct Source {
    git_commit: String,
    #[serde(skip_deserializing)]
    git_tree: String,
    #[serde(skip_deserializing)]
    git_path: String,
}

#[derive(Deserialize, Serialize)]
struct Compressed {
    #[serde(skip_deserializing)]
    path: String,
    sha256: String,
    size: u64,
}

#[derive(Deserialize, Serialize)]
struct IndexedFile {
    original_path: String,
    retrieval_path: String,
    sha256: String,
    size: u64,
    git_mode: GitMode,
    git_blob: String,
}

#[derive(Deserialize, Serialize, Copy, Clone, Eq, PartialEq)]
#[serde(try_from = "String")]
enum GitMode {
    #[serde(rename = "100644")]
    Regular,
    #[serde(rename = "100755")]
    Executable,
}

impl TryFrom<String> for GitMode {
    type Error = anyhow::Error;

    fn try_from(mode: String) -> Result<Self> {
        match mode.as_str() {
            "100644" => Ok(Self::Regular),
            "100755" => Ok(Self::Executable),
            _ => bail!("Expected regular Git mode 100644 or 100755, got {mode:?}"),
        }
    }
}

impl GitMode {
    fn permissions(self) -> u32 {
        match self {
            Self::Regular => 0o644,
            Self::Executable => 0o755,
        }
    }
}

fn git_blob(data: &[u8]) -> String {
    let mut blob = Sha1::new();
    blob.update(format!("blob {}\0", data.len()));
    blob.update(data);
    format!("{:x}", blob.finalize())
}

#[derive(Serialize)]
pub struct Receipt {
    format: &'static str,
    verified: bool,
    git_commit: String,
    archive_sha256: String,
    files: usize,
    recovered_bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    output: Option<PathBuf>,
}

struct VerifiedArchive {
    receipt: Receipt,
    files: Vec<(IndexedFile, Vec<u8>)>,
}

fn check_path(path: &str) -> Result<()> {
    ensure!(
        !path.contains(['\\', '\0'])
            && !path.split('/').any(|part| matches!(part, "" | "." | "..")),
        "Archive path must be canonical and relative: {path:?}"
    );
    #[cfg(windows)]
    for part in path.split('/') {
        // Win32 device names and alternate data streams do not denote normal files.
        let stem = part
            .split('.')
            .next()
            .unwrap()
            .trim_end()
            .to_ascii_uppercase();
        let numbered_device = stem
            .strip_prefix("COM")
            .or_else(|| stem.strip_prefix("LPT"))
            .is_some_and(|n| {
                matches!(
                    n,
                    "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
                )
            });
        ensure!(
            !part.contains(':')
                && !part.ends_with([' ', '.'])
                && !matches!(
                    stem.as_str(),
                    "AUX" | "CON" | "NUL" | "PRN" | "CONIN$" | "CONOUT$"
                )
                && !numbered_device,
            "Unsupported Windows archive path: {path:?}"
        );
    }
    Ok(())
}

fn verified_files(archive: &Path, index: &Path) -> Result<VerifiedArchive> {
    let index = fs::read(index).with_context(|| format!("Read index {}", index.display()))?;
    let index: Index = serde_json::from_slice(&index).context("Invalid archive index")?;
    ensure!(index.format == FORMAT, "Unsupported archive index format");
    let raw = fs::read(archive).with_context(|| format!("Read archive {}", archive.display()))?;
    let archive_sha256 = gdd::sha256(&raw);
    ensure!(
        raw.len() as u64 == index.archive.size && archive_sha256 == index.archive.sha256,
        "Compressed archive digest or size mismatch"
    );

    let mut expected = BTreeMap::new();
    for entry in index.files {
        let path = &entry.original_path;
        check_path(path)?;
        ensure!(
            *path == entry.retrieval_path,
            "Retrieval path differs from {path:?}"
        );
        ensure!(
            !expected.contains_key(path),
            "Duplicate index path: {path:?}"
        );
        expected.insert(path.clone(), entry);
    }
    for path in expected.keys() {
        for (slash, _) in path.match_indices('/') {
            ensure!(
                !expected.contains_key(&path[..slash]),
                "File/directory conflict: {path:?}"
            );
        }
    }

    let mut decoder = MultiGzDecoder::new(raw.as_slice());
    let mut recovered_bytes = 0;
    let mut files = Vec::new();
    members::regular_files(&mut decoder, false, |path, mode, size, member| {
        check_path(&path)?;
        let entry = expected
            .remove(&path)
            .with_context(|| format!("Unexpected or duplicate archive member: {path:?}"))?;
        ensure!(
            entry.size == size && entry.git_mode.permissions() == mode,
            "Archive member metadata mismatch: {path:?}"
        );
        let mut data = Vec::new();
        member.read_to_end(&mut data)?;
        ensure!(
            data.len() as u64 == entry.size
                && gdd::sha256(&data) == entry.sha256
                && git_blob(&data) == entry.git_blob,
            "Recovered file digest or size mismatch: {:?}",
            entry.original_path
        );
        recovered_bytes += data.len() as u64;
        files.push((entry, data));
        Ok(())
    })?;
    // The tar iterator stops at its first end marker. Read through the gzip
    // trailer to verify its checksum and reject hidden trailing members.
    let mut padding = [0u8; 8192];
    loop {
        let count = decoder.read(&mut padding)?;
        if count == 0 {
            break;
        }
        ensure!(
            padding[..count].iter().all(|&byte| byte == 0),
            "Data after tar end marker"
        );
    }
    ensure!(expected.is_empty(), "Archive is missing indexed files");
    Ok(VerifiedArchive {
        receipt: Receipt {
            format: FORMAT,
            verified: true,
            git_commit: index.source.git_commit,
            archive_sha256,
            files: files.len(),
            recovered_bytes,
            output: None,
        },
        files,
    })
}

pub fn restore(archive: &Path, index: &Path, output: &Path) -> Result<Receipt> {
    verified_files(archive, index)?.restore(output)
}

impl VerifiedArchive {
    fn restore(self, output: &Path) -> Result<Receipt> {
        let output = std::env::current_dir()?.join(output);
        output
            .to_str()
            .context("Restore output must be representable in the JSON receipt")?;
        let parent = output
            .parent()
            .context("Restore output must name a new directory")?;
        for ancestor in parent.ancestors() {
            let metadata = ancestor
                .symlink_metadata()
                .with_context(|| format!("Inspect output parent {}", ancestor.display()))?;
            ensure!(
                metadata.is_dir(),
                "Restore output ancestors must be existing directories without symlinks"
            );
        }
        match output.symlink_metadata() {
            Ok(_) => bail!(
                "Restore output must not already exist: {}",
                output.display()
            ),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        let mut staging = tempfile::Builder::new();
        staging.prefix(".concert-restore-");
        #[cfg(unix)]
        staging.permissions(fs::Permissions::from_mode(0o700));
        let staging = staging.tempdir_in(parent)?;
        for (entry, data) in self.files {
            let target = staging.path().join(&entry.original_path);
            fs::create_dir_all(target.parent().unwrap())?;
            let mut file = File::create_new(&target)?;
            file.write_all(&data)?;
            #[cfg(unix)]
            {
                file.set_permissions(fs::Permissions::from_mode(entry.git_mode.permissions()))?;
            }
            drop(file);
            ensure!(
                gdd::sha256(&fs::read(&target)?) == entry.sha256,
                "Restored bytes did not match: {:?}",
                entry.original_path
            );
        }
        gdd::publish_exclusive(staging.path(), &output)?;
        let _ = staging.keep();
        Ok(Receipt {
            output: Some(output),
            ..self.receipt
        })
    }
}

#[cfg(test)]
mod tests;
