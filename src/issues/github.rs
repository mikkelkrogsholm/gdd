use super::{GitHubIssue, IssueLink, https, invalid};
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::json;
use std::{io, time::Duration};
use ureq::{Agent, RequestBuilder};
use url::Url;

pub(super) fn issue_path(url: &Url) -> io::Result<String> {
    let parts: Vec<_> = url
        .path()
        .trim_end_matches('/')
        .split('/')
        .skip(1)
        .collect();
    if let [owner, repo, "issues", number] = parts.as_slice()
        && url.host_str() == Some("github.com")
        && url.port().is_none()
        && url.query().is_none()
        && url.fragment().is_none()
        && [owner, repo].iter().all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
        })
        && number.bytes().all(|c| c.is_ascii_digit())
        && let Ok(n) = number.parse::<u64>()
        && n > 0
    {
        return Ok(format!("/repos/{owner}/{repo}/issues/{n}"));
    }
    Err(invalid(
        "Expected https://github.com/OWNER/REPO/issues/NUMBER.",
    ))
}

#[derive(Deserialize)]
struct User {
    login: String,
}

#[derive(Deserialize)]
struct RemoteIssue {
    title: String,
    state: String,
    assignees: Vec<User>,
    pull_request: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct Comment {
    id: u64,
    body: String,
    user: User,
}

/// GitHub.com issue adapter. Authentication is sent only to the fixed API origin.
pub struct GitHub {
    agent: Agent,
    base: String,
    token: Option<String>,
}

impl GitHub {
    pub fn new(token: Option<String>) -> Self {
        Self {
            agent: Agent::config_builder()
                .timeout_global(Some(Duration::from_secs(20)))
                .max_redirects(0)
                .build()
                .into(),
            base: "https://api.github.com".into(),
            token,
        }
    }

    fn headers<B>(&self, request: RequestBuilder<B>) -> RequestBuilder<B> {
        let request = request
            .header("Accept", "application/vnd.github+json")
            .header("User-Agent", "gdd-issue-adapter");
        match &self.token {
            Some(token) => request.header("Authorization", format!("Bearer {token}")),
            None => request,
        }
    }

    fn response(
        result: Result<ureq::http::Response<ureq::Body>, ureq::Error>,
    ) -> io::Result<ureq::http::Response<ureq::Body>> {
        let response = result.map_err(io::Error::other)?;
        if !response.status().is_success() {
            return Err(io::Error::other(format!(
                "GitHub returned {}.",
                response.status()
            )));
        }
        Ok(response)
    }

    fn get<T: DeserializeOwned>(&self, path: &str) -> io::Result<T> {
        Self::response(
            self.headers(self.agent.get(format!("{}{path}", self.base)))
                .call(),
        )?
        .body_mut()
        .read_json()
        .map_err(io::Error::other)
    }

    /// Populate GitHub collaboration data without changing package or evidence state.
    pub fn inspect(&self, link: &IssueLink) -> io::Result<GitHubIssue> {
        let remote: RemoteIssue = self.get(&issue_path(&https(&link.url)?)?)?;
        if remote.pull_request.is_some() {
            return Err(invalid(
                "This GitHub number identifies a pull request. Select an issue.",
            ));
        }
        Ok(GitHubIssue {
            title: remote.title,
            state: remote.state,
            assignees: remote.assignees.into_iter().map(|u| u.login).collect(),
        })
    }

    /// Maintain one progress comment belonging to the authenticated publisher.
    /// Other participants' comments and the issue's state are left intact.
    pub fn publish_linked(
        &self,
        root: &std::path::Path,
        overview: &crate::work_overview::WorkOverview,
        package: &crate::work_overview::WorkPackage,
        result_url: &str,
    ) -> io::Result<&'static str> {
        let path = super::location(root, &overview.project_id, &package.id)?;
        let lock = std::fs::File::open(path)?;
        lock.lock()?;
        let link = super::lookup(root, &overview.project_id, &package.id)?
            .ok_or_else(|| invalid("Link the package to a GitHub issue first."))?;
        self.publish(&link, overview, package, result_url)
    }

    fn publish(
        &self,
        link: &IssueLink,
        overview: &crate::work_overview::WorkOverview,
        package: &crate::work_overview::WorkPackage,
        result_url: &str,
    ) -> io::Result<&'static str> {
        if self.token.is_none() {
            return Err(invalid("Publishing requires GITHUB_TOKEN or GH_TOKEN."));
        }
        let result_url = https(result_url)?;
        self.inspect(link)?;
        let path = issue_path(&https(&link.url)?)?;
        let marker = format!("<!-- gdd-work {} {} -->", overview.project_id, package.id);
        let verdict = package
            .verification
            .map(|v| v.to_string())
            .unwrap_or_else(|| "UNKNOWN".into());
        let revision: String = overview
            .code_revision
            .chars()
            .map(|c| if c.is_control() || c == '`' { ' ' } else { c })
            .collect();
        let body = format!(
            "{marker}\nGDD package `{}` is **{}**. Implementation checks: **{verdict}**.\n\nCode revision: `{revision}`\n\n[Work result](<{result_url}>)",
            package.id, package.phase
        );
        let publisher: User = self.get("/user")?;
        let mut page = 1;
        loop {
            let comments: Vec<Comment> =
                self.get(&format!("{path}/comments?per_page=100&page={page}"))?;
            if let Some(comment) = comments.iter().find(|c| {
                c.user.login.eq_ignore_ascii_case(&publisher.login)
                    && c.body.lines().next() == Some(&marker)
            }) {
                if comment.body == body {
                    return Ok("unchanged");
                }
                let (repo, _) = path.rsplit_once("/issues/").expect("verified issue path");
                Self::response(
                    self.headers(self.agent.patch(format!(
                        "{}{repo}/issues/comments/{}",
                        self.base, comment.id
                    )))
                    .send_json(json!({"body": body})),
                )?;
                return Ok("updated");
            }
            if comments.len() < 100 {
                break;
            }
            page += 1;
        }
        Self::response(
            self.headers(self.agent.post(format!("{}{path}/comments", self.base)))
                .send_json(json!({"body": body})),
        )?;
        Ok("created")
    }
}

#[cfg(test)]
#[path = "github_tests.rs"]
mod tests;
