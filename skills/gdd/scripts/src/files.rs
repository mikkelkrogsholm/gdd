use crate::COPIES;
use anyhow::{Context, Result, bail, ensure};
use std::collections::BTreeSet;
use std::fs;
use std::io::{ErrorKind, Write};
use std::path::{Component, Path, PathBuf};
use walkdir::WalkDir;

pub fn read(path: &Path) -> Result<Vec<u8>> {
    ensure!(
        fs::metadata(path)
            .with_context(|| path.display().to_string())?
            .is_file(),
        "Expected a regular file: {}",
        path.display()
    );
    fs::read(path).with_context(|| format!("Cannot read {}", path.display()))
}

// Index paths always use portable forward slashes and have a unique spelling.
pub fn is_relative_name(name: &str) -> bool {
    !name.contains('\\')
        && name.split('/').all(|part| {
            !part.is_empty()
                && part != "."
                && part != ".."
                && matches!(
                    Path::new(part).components().next(),
                    Some(Component::Normal(_))
                )
        })
}

pub fn relative(root: &Path, path: &Path) -> Result<String> {
    let parts: Vec<_> = path
        .strip_prefix(root)?
        .iter()
        .map(|part| part.to_str().context("Reference path must be UTF-8"))
        .collect::<Result<_>>()?;
    let name = parts.join("/");
    ensure!(is_relative_name(&name), "Invalid reference path: {name}");
    Ok(name)
}

// Generated directory roots and metadata paths must not be redirected.
// Also check ancestors of paths that do not yet exist.
pub fn output(root: &Path, relative: &str) -> Result<PathBuf> {
    let mut path = root.to_owned();
    for part in relative.split('/') {
        path.push(part);
        match fs::symlink_metadata(&path) {
            Ok(meta) => ensure!(
                !meta.is_symlink(),
                "Symbolic link in generated path: {}",
                path.display()
            ),
            Err(err) if err.kind() == ErrorKind::NotFound => (),
            Err(err) => return Err(err).with_context(|| path.display().to_string()),
        }
    }
    Ok(path)
}

pub fn generated_files(skill: &Path) -> Result<BTreeSet<String>> {
    let dir = output(skill, COPIES)?;
    let mut names = BTreeSet::new();
    if !dir.try_exists()? {
        return Ok(names);
    }
    ensure!(
        dir.is_dir(),
        "Expected generated directory: {}",
        dir.display()
    );
    for entry in WalkDir::new(&dir).min_depth(1) {
        let entry = entry?;
        if entry.file_type().is_symlink() {
            let target = entry
                .path()
                .canonicalize()
                .with_context(|| entry.path().display().to_string())?;
            ensure!(
                target.starts_with(&dir) && target.is_file(),
                "Generated reference redirects outside its files: {}",
                entry.path().display()
            );
            names.insert(relative(skill, entry.path())?);
        } else if entry.file_type().is_file() {
            names.insert(relative(skill, entry.path())?);
        } else if !entry.file_type().is_dir() {
            bail!(
                "Unexpected file type in generated references: {}",
                entry.path().display()
            );
        }
    }
    Ok(names)
}

pub fn write(path: &Path, data: &[u8]) -> Result<()> {
    let dir = path.parent().context("Missing output directory")?;
    fs::create_dir_all(dir)?;
    let mut builder = tempfile::Builder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        builder.permissions(fs::Permissions::from_mode(0o666));
    }
    let mut file = builder.tempfile_in(dir)?;
    file.write_all(data)?;
    file.persist(path)
        .with_context(|| format!("Cannot replace {}", path.display()))?;
    Ok(())
}
