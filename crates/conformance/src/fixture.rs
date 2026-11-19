//! Snapshot and write private conformance case inputs.

use anyhow::Result;
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

pub(super) fn write_json(path: &Path, value: &Value) -> Result<()> {
    Ok(fs::write(path, serde_json::to_vec_pretty(value)?)?)
}

/// Restore the verified historical concert project under a private scratch directory.
pub(super) fn restore_concert(conformance: &Path, scratch: &Path) -> Result<PathBuf> {
    let archives = conformance.join("../archive/experiments");
    let restored = scratch.canonicalize()?.join("restored");
    crate::concert_archive::restore(
        &archives.join("concert-7e27757.tar.gz"),
        &archives.join("concert-7e27757.index.json"),
        &restored,
    )?;
    Ok(restored.join("examples/concert-app"))
}

/// Collect the names and bytes of all regular files under a fixture directory.
pub(super) fn snapshot(root: &Path) -> Result<BTreeMap<PathBuf, Vec<u8>>> {
    let mut files = BTreeMap::new();
    let mut directories = vec![root.to_owned()];
    while let Some(directory) = directories.pop() {
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            let path = entry.path();
            let kind = entry.file_type()?;
            if kind.is_dir() {
                directories.push(path);
            } else if kind.is_file() {
                files.insert(path.strip_prefix(root)?.to_owned(), fs::read(path)?);
            }
        }
    }
    Ok(files)
}
