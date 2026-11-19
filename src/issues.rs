//! Stable work-package links. A tracker reports collaboration state independently of GDD evidence.

use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

mod github;
pub use github::GitHub;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssueLink {
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub github: Option<GitHubIssue>,
}

impl IssueLink {
    pub fn is_github(&self) -> bool {
        url::Url::parse(&self.url).is_ok_and(|url| url.host_str() == Some("github.com"))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitHubIssue {
    pub title: String,
    pub state: String,
    pub assignees: Vec<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredLink {
    project: String,
    package: String,
    url: String,
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

fn location(root: &Path, project: &str, package: &str) -> io::Result<PathBuf> {
    if !crate::valid_id(project) || !crate::valid_id(package) {
        return Err(invalid("Expected stable GDD project and package IDs."));
    }
    Ok(root.join(project).join(format!("{package}.json")))
}

fn https(value: &str) -> io::Result<url::Url> {
    let url = url::Url::parse(value).map_err(io::Error::other)?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || value
            .chars()
            .any(|ch| ch.is_control() || ch.is_whitespace() || matches!(ch, '<' | '>' | '\\'))
    {
        return Err(invalid(
            "Expected an HTTPS URL without credentials, spaces or markup.",
        ));
    }
    Ok(url)
}

fn from_url(value: &str) -> io::Result<IssueLink> {
    let url = https(value)?;
    if url.host_str() == Some("github.com") {
        github::issue_path(&url)?;
    }
    Ok(IssueLink {
        url: url.to_string(),
        github: None,
    })
}

/// A missing package link is normal. Malformed or misplaced links are reported.
pub fn lookup(root: &Path, project: &str, package: &str) -> io::Result<Option<IssueLink>> {
    let path = location(root, project, package)?;
    let bytes = match fs::read(&path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        other => other?,
    };
    let stored: StoredLink = serde_json::from_slice(&bytes)?;
    if stored.project != project || stored.package != package {
        return Err(invalid(
            "The issue link belongs to another project or work package.",
        ));
    }
    Ok(Some(from_url(&stored.url)?))
}

/// Publish one immutable link without clobbering a competing writer. Repeating it is harmless.
pub fn link(root: &Path, project: &str, package: &str, url: &str) -> io::Result<IssueLink> {
    let issue = from_url(url)?;
    let path = location(root, project, package)?;
    let dir = path.parent().expect("project directory");
    fs::create_dir_all(dir)?;
    let stored = StoredLink {
        project: project.into(),
        package: package.into(),
        url: issue.url.clone(),
    };
    let mut staged = tempfile::NamedTempFile::new_in(dir)?;
    serde_json::to_writer_pretty(&mut staged, &stored)?;
    writeln!(staged)?;
    staged.as_file().sync_all()?;
    match staged.persist_noclobber(path) {
        Ok(_) => Ok(issue),
        Err(error) if error.error.kind() == io::ErrorKind::AlreadyExists => {
            if lookup(root, project, package)?.as_ref() == Some(&issue) {
                Ok(issue)
            } else {
                Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "This work package already links to a different issue.",
                ))
            }
        }
        Err(error) => Err(error.error),
    }
}
