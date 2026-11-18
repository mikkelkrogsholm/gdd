use super::{diagnostic, glob};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::Path;

pub(super) const IGNORED: [&str; 6] = [
    ".git",
    ".nox",
    ".tox",
    ".venv",
    "__pycache__",
    "node_modules",
];

#[derive(Default, PartialEq)]
pub(super) struct Inventory {
    pub sources: BTreeMap<String, Vec<u8>>,
    pub omitted: Vec<String>,
    pub diagnostics: Vec<Value>,
}

#[cfg(unix)]
fn identity(file: &File) -> io::Result<(u64, u64)> {
    use std::os::unix::fs::MetadataExt;
    let m = file.metadata()?;
    Ok((m.dev(), m.ino()))
}

#[cfg(windows)]
fn identity(file: &File) -> io::Result<(u64, u64)> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle,
    };
    let mut info = std::mem::MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::uninit();
    // SAFETY: The file descriptor is live and the output has the required size.
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), info.as_mut_ptr()) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let info = unsafe { info.assume_init() };
    Ok((
        u64::from(info.dwVolumeSerialNumber),
        u64::from(info.nFileIndexHigh) << 32 | u64::from(info.nFileIndexLow),
    ))
}

#[cfg(not(any(windows, unix)))]
fn identity(_: &File) -> io::Result<(u64, u64)> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "Physical file identity is unavailable.",
    ))
}

impl Inventory {
    fn unavailable(&mut self, name: Option<&str>, message: &str) {
        self.diagnostics
            .push(diagnostic("CODE_SOURCE_UNAVAILABLE", message, name, &[]));
    }
}

pub(super) fn scan(root: &Path, exclusions: &[String]) -> Inventory {
    let mut result = Inventory::default();
    let mut identities = BTreeMap::new();
    let mut todo = vec![String::new()];
    while let Some(dir) = todo.pop() {
        let path = root.join(&dir);
        // A directory discovered in one step can be replaced before traversal.
        match fs::canonicalize(&path) {
            Ok(resolved) if resolved == path => {}
            Ok(_) => {
                result.unavailable(Some(&dir), "Source directory changed before traversal.");
                continue;
            }
            Err(error) => {
                result.unavailable(None, &format!("{}: {error}", path.display()));
                continue;
            }
        }
        let entries = match fs::read_dir(&path) {
            Ok(entries) => entries,
            Err(e) => {
                result.unavailable(None, &format!("{}: {e}", path.display()));
                continue;
            }
        };
        let mut dirs = BTreeMap::new();
        let mut files = BTreeMap::new();
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(e) => {
                    result.unavailable(None, &e.to_string());
                    continue;
                }
            };
            let os_name = entry.file_name();
            let ft = match entry.file_type() {
                Ok(ft) => ft,
                Err(e) => {
                    result.unavailable(Some(&dir), &e.to_string());
                    continue;
                }
            };
            let directory = ft.is_dir() || ft.is_symlink() && entry.path().is_dir();
            if !directory && !os_name.as_encoded_bytes().ends_with(b".py") {
                continue;
            }
            let Ok(base) = os_name.into_string() else {
                result.unavailable(Some(&dir), "Source path is not valid Unicode.");
                continue;
            };
            let name = if dir.is_empty() {
                base.to_owned()
            } else {
                format!("{dir}/{base}")
            };
            if directory {
                dirs.insert(name, (base, ft.is_symlink()));
            } else {
                files.insert(name, ft.is_file());
            }
        }
        let mut next = Vec::new();
        for (name, (base, link)) in dirs {
            let folder = format!("{name}/");
            if IGNORED.contains(&base.as_str())
                || exclusions
                    .iter()
                    .any(|p| glob::matches(&name, p) || glob::matches(&folder, p))
            {
                result.omitted.push(folder);
            } else if link {
                result.unavailable(Some(&name), "Source directories cannot be symlinks.");
                result.omitted.push(folder);
            } else {
                next.push(name);
            }
        }
        todo.extend(next.into_iter().rev());
        for (name, regular) in files {
            if exclusions.iter().any(|p| glob::matches(&name, p)) {
                result.omitted.push(name);
                continue;
            }
            if !regular {
                result.unavailable(
                    Some(&name),
                    "Python source must be a regular file without symlinks.",
                );
                continue;
            }
            let captured = (|| -> io::Result<_> {
                let mut file = crate::bundle::open_regular(root, Path::new(&name))?;
                let physical = identity(&file)?;
                if let Some(first) = identities.get(&physical) {
                    result.sources.remove(first);
                    result.omitted.extend([String::clone(first), name.clone()]);
                    result.diagnostics.push(diagnostic(
                        "CODE_BINDING_AMBIGUOUS",
                        "Multiple source paths identify the same physical file.",
                        Some(&name),
                        &[],
                    ));
                } else {
                    identities.insert(physical, name.clone());
                    let mut bytes = Vec::new();
                    file.read_to_end(&mut bytes)?;
                    result.sources.insert(name.clone(), bytes);
                }
                Ok(())
            })();
            if let Err(e) = captured {
                result.unavailable(Some(&name), &format!("{}: {e}", root.join(&name).display()));
            }
        }
    }
    result.omitted.sort();
    result
}
