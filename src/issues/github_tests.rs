use super::*;
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::TcpListener,
    thread,
};

use crate::work_overview::{WorkOverview, work_overview};
use std::path::Path;
fn fixture() -> WorkOverview {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/agent-native");
    work_overview(&root.join("model.gdd.json"), &root.join("native-run"), None).unwrap()
}

#[test]
fn github_refresh_and_repeat_publish_preserve_remote_and_workflow_state() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let mut comments = Vec::new();
        // Read the issue, then create, repeat, and update a result.
        for expected in [
            "GET issue",
            "GET issue",
            "GET user",
            "GET comments",
            "POST comments",
            "GET issue",
            "GET user",
            "GET comments",
            "GET issue",
            "GET user",
            "GET comments",
            "PATCH comment",
        ] {
            let (mut stream, _) = listener.accept().unwrap();
            let mut input = BufReader::new(&stream);
            let mut request = String::new();
            input.read_line(&mut request).unwrap();
            let mut len = 0;
            let mut auth = false;
            for header in input.by_ref().lines() {
                let header = header.unwrap();
                if header.is_empty() {
                    break;
                }
                if let Some((name, value)) = header.split_once(':') {
                    if name.eq_ignore_ascii_case("content-length") {
                        len = value.trim().parse().unwrap();
                    }
                    if name.eq_ignore_ascii_case("authorization") {
                        auth = value.trim() == "Bearer local-fixture";
                    }
                }
            }
            assert!(auth);
            let mut data = vec![0; len];
            input.read_exact(&mut data).unwrap();
            let mut first = request.split_ascii_whitespace();
            let method = first.next().unwrap();
            let path = first.next().unwrap();
            let endpoint = match path {
                "/user" => "user",
                "/repos/org/repo/issues/17" => "issue",
                "/repos/org/repo/issues/17/comments"
                | "/repos/org/repo/issues/17/comments?per_page=100&page=1" => "comments",
                "/repos/org/repo/issues/comments/777" => "comment",
                _ => panic!("Unexpected endpoint {path}"),
            };
            assert_eq!(format!("{method} {endpoint}"), expected);
            let payload = match expected {
                "GET issue" => json!({"title":"Discount","state":"closed","assignees":[{"login":"maintainer"}]}),
                "GET user" => json!({"login":"publisher"}),
                "GET comments" => json!(comments),
                "POST comments" | "PATCH comment" => {
                    let body: serde_json::Value = serde_json::from_slice(&data).unwrap();
                    let body = body["body"].as_str().unwrap();
                    assert!(body.contains("UNKNOWN"));
                    assert!(body.contains("https://example.org/"));
                    comments = vec![json!({"id":777,"body":body,"user":{"login":"publisher"}})];
                    json!({"id":777})
                }
                _ => unreachable!()
            }.to_string();
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", payload.len(), payload).unwrap();
        }
    });
    let mut api = GitHub::new(Some("local-fixture".into()));
    api.base = format!("http://{address}");
    let overview = fixture();
    let root = tempfile::tempdir().unwrap();
    let package = &overview.packages[0];
    let link = crate::issues::link(
        root.path(),
        &overview.project_id,
        &package.id,
        "https://github.com/org/repo/issues/17",
    )
    .unwrap();
    let state = api.inspect(&link).unwrap();
    assert_eq!(state.state, "closed");
    assert_eq!(state.assignees, ["maintainer"]);
    assert!(package.verification.is_none());
    for (url, expected) in [("a", "created"), ("a", "unchanged"), ("b", "updated")] {
        assert_eq!(
            api.publish_linked(
                root.path(),
                &overview,
                package,
                &format!("https://example.org/{url}")
            )
            .unwrap(),
            expected
        );
    }
    server.join().unwrap();
}

#[test]
fn stable_project_package_links_survive_reruns_and_reject_conflicts() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let store = |url| crate::issues::link(root, "project.shop", "package.pay", url);
    let issue = store("https://tracker.example/work/34").unwrap();
    assert!(!issue.is_github());
    assert_eq!(store(&issue.url).unwrap(), issue);
    assert!(store("https://tracker.example/work/31").is_err());
    assert_eq!(
        crate::issues::lookup(root, "project.shop", "package.pay").unwrap(),
        Some(issue)
    );
    assert!(
        crate::issues::lookup(root, "project.other", "package.pay")
            .unwrap()
            .is_none()
    );
    assert!(crate::issues::lookup(root, "project.shop", "../outside").is_err());
    for url in [
        "file:///tmp/issues",
        "https://user:pwd@github.com/org/repo/issues/17",
        "https://github.com/org/repo/pull/14",
    ] {
        assert!(super::super::from_url(url).is_err());
    }
}
