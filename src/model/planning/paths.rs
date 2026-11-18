//! Logical write resources and declared workspace locations.

use super::{Model, blank_text};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn unsupported_path(value: &str) -> bool {
    blank_text(value) || value.contains(['\0', '*', '?', '[', '\\', ':'])
}

#[derive(Debug)]
pub(super) struct WriteResource {
    pub(super) path: String,
    pub(super) directory: bool,
}

impl WriteResource {
    pub(super) fn parse(value: &str) -> Option<Self> {
        if unsupported_path(value) || value.starts_with('/') {
            return None;
        }
        let mut parts = Vec::new();
        for part in value.split('/') {
            match part {
                "" | "." => {}
                ".." => {
                    parts.pop()?;
                }
                part => parts.push(part),
            }
        }
        let directory = matches!(value.rsplit('/').next(), Some("" | "." | ".."));
        if parts.is_empty() && !directory {
            return None;
        }
        let path = if parts.is_empty() {
            ".".to_owned()
        } else {
            parts.join("/")
        };
        Some(Self { path, directory })
    }

    pub(super) fn contains(&self, other: &Self) -> bool {
        self.path == other.path
            || self.directory
                && (self.path == "."
                    || other
                        .path
                        .strip_prefix(&self.path)
                        .is_some_and(|rest| rest.starts_with('/')))
    }
}

pub(super) fn workspace(model: &Model<'_>, value: Option<&str>) -> Option<PathBuf> {
    let value = value.filter(|value| !unsupported_path(value))?;
    let path = model.bundle.path().parent()?.join(Path::new(value));
    workspace_path(&path)
}

// Workspaces are declarations and may have missing prefixes. Follow each
// existing symlink before applying subsequent parent components. Cache link
// destinations separately from active links so reusing a link is not a cycle.
fn workspace_path(path: &Path) -> Option<PathBuf> {
    use std::ffi::OsString;
    use std::fs;
    use std::path::Component;

    if let Ok(existing) = fs::canonicalize(path) {
        return Some(existing);
    }
    enum Step {
        Part(OsString),
        Resolved(PathBuf),
    }
    let parts = |p: &Path| {
        p.iter()
            .rev()
            .map(|s| Step::Part(s.to_os_string()))
            .collect::<Vec<_>>()
    };
    let mut pending = parts(path);
    let mut links: BTreeMap<PathBuf, Option<PathBuf>> = BTreeMap::new();
    let mut resolved = PathBuf::new();
    let mut looped = false;
    while let Some(step) = pending.pop() {
        let part = match step {
            Step::Resolved(link) => {
                links.insert(link, Some(resolved.clone()));
                continue;
            }
            Step::Part(part) => part,
        };
        match Path::new(&part).components().next() {
            Some(Component::ParentDir) => {
                resolved.pop();
            }
            Some(Component::Normal(_)) => {
                resolved.push(&part);
                if looped {
                    continue;
                }
                if let Some(cached) = links.get(&resolved) {
                    if let Some(cached) = cached {
                        resolved.clone_from(cached);
                    } else {
                        looped = true;
                    }
                } else if fs::symlink_metadata(&resolved).is_ok_and(|m| m.is_symlink()) {
                    let target = fs::read_link(&resolved).ok()?;
                    links.insert(resolved.clone(), None);
                    pending.push(Step::Resolved(resolved.clone()));
                    resolved.pop();
                    if target.is_absolute() {
                        resolved.clear();
                    }
                    pending.extend(parts(&target));
                }
            }
            Some(Component::RootDir | Component::Prefix(_)) => resolved.push(part),
            _ => {}
        }
    }
    // A parent component following a loop can still name an ordinary workspace.
    if looped && fs::metadata(&resolved).is_err_and(|error| symlink_loop(&error)) {
        return None;
    }
    Some(resolved)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn symlink_loop(error: &std::io::Error) -> bool {
    error.raw_os_error() == Some(rustix::io::Errno::LOOP.raw_os_error())
}

#[cfg(windows)]
fn symlink_loop(error: &std::io::Error) -> bool {
    error.raw_os_error() == Some(1921)
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
fn symlink_loop(_: &std::io::Error) -> bool {
    true
}
