//! Rebuild the preserved PAX and gzip byte representation from Git objects.

use super::{
    Compressed, FORMAT, GitMode, Index, IndexedFile, Receipt, Source, check_path, git_blob,
    verified_files,
};
use anyhow::{Context, Result, bail, ensure};
use std::collections::{BTreeMap, HashSet};
use std::fmt::Write as _;
use std::fs;
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use tar::{Builder, EntryType, Header};

fn git(repository: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repository)
        .arg("--literal-pathspecs")
        .args(args)
        .output()
        .context("Run Git to read the committed tree")?;
    ensure!(
        output.status.success(),
        "Git failed: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
    Ok(output.stdout)
}

fn revision_id(repository: &Path, expression: &str) -> Result<String> {
    Ok(String::from_utf8(git(
        repository,
        &["rev-parse", "--verify", "--end-of-options", expression],
    )?)?
    .trim()
    .to_owned())
}

struct GitFile {
    mode: GitMode,
    blob: String,
    bytes: Vec<u8>,
}

fn committed_files(
    repository: &Path,
    commit: &str,
    path: &str,
) -> Result<BTreeMap<String, GitFile>> {
    let listing = git(repository, &["ls-tree", "-rz", commit, "--", path])?;
    let mut expected = BTreeMap::new();
    for line in std::str::from_utf8(&listing)?.split_terminator('\0') {
        let (identity, name) = line.split_once('\t').context("Invalid Git tree entry")?;
        check_path(name)?;
        let mut fields = identity.split(' ');
        let mode = GitMode::try_from(fields.next().context("Missing Git mode")?.to_owned())
            .with_context(|| format!("Unsupported Git file: {name:?}"))?;
        ensure!(
            fields.next() == Some("blob"),
            "Expected a Git blob: {name:?}"
        );
        let blob = fields.next().context("Missing Git blob identity")?;
        ensure!(
            fields.next().is_none() && expected.insert(name, (mode, blob)).is_none(),
            "Invalid or duplicate Git tree entry: {name:?}"
        );
    }
    ensure!(!expected.is_empty(), "The selected Git subtree is empty");

    // Git archive applies committed export attributes. Reject omissions and
    // substitutions by comparing its full file set and bytes to ls-tree.
    let raw = git(
        repository,
        &[
            "-c",
            "tar.umask=0000",
            "archive",
            "--format=tar",
            commit,
            "--",
            path,
        ],
    )?;
    let mut files = BTreeMap::new();
    super::members::regular_files(&mut raw.as_slice(), true, |name, archive_mode, _, entry| {
        let (mode, blob) = expected
            .remove(name.as_str())
            .with_context(|| format!("Unexpected Git archive member: {name:?}"))?;
        ensure!(
            (archive_mode & 0o100 != 0) == (mode == GitMode::Executable),
            "Git file mode mismatch: {name:?}"
        );
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes)?;
        ensure!(
            git_blob(&bytes) == blob,
            "Git blob bytes did not match: {name:?}"
        );
        files.insert(
            name,
            GitFile {
                mode,
                blob: blob.to_owned(),
                bytes,
            },
        );
        Ok(())
    })?;
    ensure!(
        expected.is_empty(),
        "Git archive did not recover the complete tree"
    );
    Ok(files)
}

fn append_member(
    tar: &mut Builder<Vec<u8>>,
    name: &str,
    kind: EntryType,
    mode: u32,
    header_size: u64,
    bytes: &[u8],
) -> Result<()> {
    let mut header = Header::new_ustar();
    for (slot, ch) in header.as_old_mut().name.iter_mut().zip(name.chars()) {
        *slot = if ch.is_ascii() { ch as u8 } else { b'?' };
    }
    header.set_entry_type(kind);
    header.set_mode(mode);
    header.set_uid(0);
    header.set_gid(0);
    header.set_size(header_size);
    header.set_cksum();
    // The historical USTAR checksum uses six octal digits, NUL, space.
    let checksum = &mut header.as_old_mut().cksum;
    checksum.copy_within(1..8, 0);
    checksum[7] = b' ';
    tar.append(&header, bytes)?;
    Ok(())
}

fn pax_record(records: &mut String, key: &str, value: &str) {
    // The decimal record length includes its own digits and trailing newline.
    let rest = key.len() + value.len() + 3;
    let mut length = rest + 1;
    while length != rest + length.ilog10() as usize + 1 {
        length = rest + length.ilog10() as usize + 1;
    }
    writeln!(records, "{length} {key}={value}").unwrap();
}

fn compressed_files(files: &BTreeMap<String, GitFile>) -> Result<Vec<u8>> {
    let mut tar = Builder::new(Vec::new());
    for (name, file) in files {
        let mut records = String::new();
        if !name.is_ascii() || name.len() > 100 {
            pax_record(&mut records, "path", name);
        }
        let mut size = file.bytes.len() as u64;
        if size >= 8u64.pow(11) {
            pax_record(&mut records, "size", &size.to_string());
            size = 0;
        }
        if !records.is_empty() {
            append_member(
                &mut tar,
                "././@PaxHeader",
                EntryType::XHeader,
                0,
                records.len() as u64,
                records.as_bytes(),
            )?;
        }
        append_member(
            &mut tar,
            name,
            EntryType::Regular,
            file.mode.permissions(),
            size,
            &file.bytes,
        )?;
    }
    let mut raw = tar.into_inner()?;
    raw.resize(raw.len().next_multiple_of(10240), 0);

    // The original gzip stream has no filename, zero mtime, XFL=2 and OS=255.
    let mut gzip = vec![0x1f, 0x8b, 8, 0, 0, 0, 0, 0, 2, 255];
    gzip.append(&mut zlib_bitexact_rs::deflate_raw(&raw));
    let mut crc = flate2::Crc::new();
    crc.update(&raw);
    gzip.extend_from_slice(&crc.sum().to_le_bytes());
    gzip.extend_from_slice(&(raw.len() as u32).to_le_bytes());
    Ok(gzip)
}

fn index_bytes(index: &Index) -> Result<Vec<u8>> {
    // json.dump(indent=2) escaped non-ASCII scalars as UTF-16 code units.
    let json = serde_json::to_string_pretty(index)?;
    let mut ascii = String::with_capacity(json.len() + 1);
    for ch in json.chars() {
        if ch < '\u{7f}' {
            ascii.push(ch);
        } else {
            for unit in ch.encode_utf16(&mut [0; 2]) {
                write!(ascii, "\\u{unit:04x}").unwrap();
            }
        }
    }
    ascii.push('\n');
    Ok(ascii.into_bytes())
}

// Resolve aliases before creating missing parents, including cases where one
// requested output would otherwise become a directory for the other.
fn output_parent(path: &Path, missing: &mut HashSet<PathBuf>) -> Result<PathBuf> {
    let mut parent = PathBuf::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                parent.pop();
            }
            Component::Normal(name) => {
                parent.push(name);
                match parent.symlink_metadata() {
                    Ok(_) => {
                        parent = parent.canonicalize().with_context(|| {
                            format!("Resolve output directory {}", parent.display())
                        })?;
                        ensure!(
                            parent.is_dir(),
                            "Output parent is not a directory: {}",
                            parent.display()
                        );
                    }
                    Err(e) if e.kind() == io::ErrorKind::NotFound => {
                        missing.insert(parent.clone());
                    }
                    Err(e) => return Err(e.into()),
                }
            }
            _ => parent.push(component),
        }
    }
    Ok(parent)
}

fn output_path(path: &Path, missing: &mut HashSet<PathBuf>) -> Result<PathBuf> {
    let absolute = std::path::absolute(path)?;
    let name = absolute
        .file_name()
        .context("Output must name a new file")?;
    let path =
        output_parent(absolute.parent().context("Missing output parent")?, missing)?.join(name);
    match path.symlink_metadata() {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(path),
        Err(e) => Err(e.into()),
        Ok(_) => bail!("Output already exists: {}", path.display()),
    }
}

fn stage(path: &Path, bytes: &[u8]) -> Result<tempfile::NamedTempFile> {
    let parent = path.parent().context("Missing output parent")?;
    fs::create_dir_all(parent)?;
    let builder = &mut tempfile::Builder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        builder.permissions(fs::Permissions::from_mode(0o666));
    }
    let mut file = builder.tempfile_in(parent)?;
    file.write_all(bytes)?;
    file.flush()?;
    Ok(file)
}

pub(super) fn build(
    repository: &Path,
    revision: &str,
    source_path: &str,
    archive: &Path,
    index: &Path,
) -> Result<Receipt> {
    let mut missing_parents = HashSet::new();
    let archive_target = output_path(archive, &mut missing_parents)?;
    let index_target = output_path(index, &mut missing_parents)?;
    ensure!(
        archive_target != index_target
            && !missing_parents.contains(&archive_target)
            && !missing_parents.contains(&index_target),
        "Archive and index must name distinct files. A required directory cannot also be an output file."
    );
    check_path(source_path)?;
    let commit = revision_id(repository, &format!("{revision}^{{commit}}"))?;
    let tree = revision_id(repository, &format!("{commit}^{{tree}}"))?;
    let files = committed_files(repository, &commit, source_path)?;
    let gzip = compressed_files(&files)?;
    let document = Index {
        format: FORMAT.to_owned(),
        source: Source {
            git_commit: commit,
            git_tree: tree,
            git_path: source_path.to_owned(),
        },
        archive: Compressed {
            path: archive
                .file_name()
                .and_then(|n| n.to_str())
                .context("Archive filename must be UTF-8")?
                .to_owned(),
            sha256: gdd::sha256(&gzip),
            size: gzip.len() as u64,
        },
        files: files
            .into_iter()
            .map(|(name, file)| IndexedFile {
                original_path: name.clone(),
                retrieval_path: name,
                sha256: gdd::sha256(&file.bytes),
                size: file.bytes.len() as u64,
                git_mode: file.mode,
                git_blob: file.blob,
            })
            .collect(),
    };
    // Create the parent directories using the caller's spelling. A component
    // before `..` must exist for that spelling to work on POSIX filesystems.
    let staged_archive = stage(archive, &gzip)?;
    let staged_index = stage(index, &index_bytes(&document)?)?;
    let receipt = verified_files(staged_archive.path(), staged_index.path())?.receipt;

    // Neither publish can replace an occupied name. A failed second publish
    // rolls back the first, including when another builder wins the index.
    staged_archive.persist_noclobber(&archive_target)?;
    if let Err(error) = staged_index.persist_noclobber(&index_target) {
        fs::remove_file(&archive_target).with_context(|| {
            format!(
                "Could not roll back {} after {error}",
                archive_target.display()
            )
        })?;
        return Err(error.into());
    }
    Ok(receipt)
}

#[cfg(test)]
mod tests;
