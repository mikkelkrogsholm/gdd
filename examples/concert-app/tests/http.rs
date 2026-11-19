#![cfg(any(unix, windows))]

#[path = "http/forms.rs"]
mod forms;
#[path = "support/server.rs"]
mod server;
#[path = "http/support.rs"]
mod support;

use std::{
    net::{Ipv4Addr, SocketAddr},
    process::Command,
};

use serde_json::json;
use server::Server;
use support::HttpClient;
use tempfile::tempdir;

impl HttpClient for Server {
    fn address(&self) -> SocketAddr {
        (Ipv4Addr::LOCALHOST, self.port).into()
    }
}

#[test]
fn admission_enforces_the_listener_origin_media_type_and_json_limit() {
    let dir = tempdir().unwrap();
    let server = Server::start(&dir.path().join("db"));
    let host = format!("localhost:{}", server.port);
    let origin = format!("http://{host}");
    let local = server.get(
        "/api/concerts?now=1",
        &[("Host", &host), ("Origin", &origin)],
    );
    assert_eq!(local.status, 200);
    assert_eq!(local.body["concerts"].as_array().unwrap().len(), 3);
    assert_eq!(local.headers["cache-control"], "no-store");
    assert_eq!(local.headers["x-content-type-options"], "nosniff");
    assert_eq!(local.headers["referrer-policy"], "same-origin");
    assert!(local.headers["content-security-policy"].contains("frame-ancestors 'none'"));
    assert!(!local.headers.contains_key("access-control-allow-origin"));
    for fields in [
        vec![("Host", "localhost:0")],
        vec![("Host", &host), ("Host", &host)],
        vec![("Origin", "null")],
        vec![("Origin", &origin)], // Same listener, different host from the request.
        vec![("Host", &host), ("Origin", &origin), ("Origin", &origin)],
    ] {
        server
            .post(
                "/api/login",
                json!({"username":"alice","password":"alice-demo"}),
                &fields,
            )
            .error(403, "Forbidden");
    }
    for body in [
        b"[]".as_slice(),
        b"42",
        b"{broken",
        b"{\"x\":NaN}",
        b"{\"x\":Infinity}",
        b"\xff",
    ] {
        let length = body.len().to_string();
        server
            .raw(
                "POST",
                "/api/login",
                &[
                    ("Content-Type", "application/json"),
                    ("Content-Length", &length),
                ],
                body,
            )
            .error(400, "InvalidInput");
    }
    let mut exact = b"{}".to_vec();
    exact.resize(16 * 1024, b' ');
    for n in [16 * 1024, 16 * 1024 + 1] {
        exact.resize(n, b' ');
        let response = server.raw(
            "POST",
            "/api/login",
            &[
                ("Content-Type", "Application/JSON; charset=utf-8"),
                ("Content-Length", &n.to_string()),
            ],
            &exact,
        );
        if n == 16 * 1024 {
            response.error(401, "InvalidCredentials");
        } else {
            response.error(400, "InvalidInput");
        }
    }
    for fields in [
        vec![("Content-Type", "application/json")],
        vec![("Content-Length", "2")],
        vec![("Content-Length", "2"), ("Content-Type", "text/plain")],
        vec![
            ("Content-Length", "2"),
            ("Content-Type", "application/json"),
            ("Content-Type", "application/json"),
        ],
    ] {
        server
            .raw("POST", "/api/unknown", &fields, b"{}")
            .error(400, "InvalidInput");
    }
    server
        .raw(
            "POST",
            "/api/login",
            &[
                ("Content-Type", "application/json"),
                ("Transfer-Encoding", "chunked"),
            ],
            b"2\r\n{}\r\n0\r\n\r\n",
        )
        .error(400, "InvalidInput");
    for path in ["/api/unknown", "/api/reservations/bad%20id/cancel"] {
        server.get(path, &[]).error(404, "NotFound");
        server.post(path, json!({}), &[]).error(404, "NotFound");
    }
}

#[test]
fn repeated_content_length_returns_the_public_admission_error() {
    let dir = tempdir().unwrap();
    let server = Server::start(&dir.path().join("db"));
    // The POST helper supplies the matching length of the JSON object.
    let rejected = server.post("/api/login", json!({}), &[("Content-Length", "2")]);
    rejected.error(400, "InvalidInput");
    assert_eq!(
        rejected.body["error"]["message"],
        "Send a JSON object with a content length."
    );
    assert_eq!(rejected.headers["cache-control"], "no-store");
    assert_eq!(rejected.headers["x-content-type-options"], "nosniff");
    assert_eq!(rejected.headers["referrer-policy"], "same-origin");
    assert!(rejected.headers["content-security-policy"].contains("frame-ancestors 'none'"));
}

#[test]
fn login_rotates_revokes_and_preserves_other_sessions_on_stale_actions() {
    let dir = tempdir().unwrap();
    let server = Server::start(&dir.path().join("db"));
    assert_eq!(
        server.get("/api/session", &[]).body,
        json!({"principal":null})
    );
    let alice = server.login("alice", None);
    let bob = server.login("bob", Some(&alice));
    assert_ne!(alice, bob);
    assert_eq!(
        server.get("/api/session", &[("Cookie", &alice)]).body,
        json!({"principal":null})
    );
    let bob_headers = [("Cookie", bob.as_str())];
    let stale = [("Cookie", bob.as_str()), ("X-Concert-Principal", "alice")];
    server
        .get("/api/reservations", &stale)
        .error(409, "SessionChanged");
    server
        .post(
            "/api/reservations",
            json!({"concert_id":"midnight","request_key":"stale"}),
            &stale,
        )
        .error(409, "SessionChanged");
    server
        .post("/api/reservations/missing/cancel", json!({}), &stale)
        .error(409, "SessionChanged");
    server
        .post("/api/logout", json!({}), &stale)
        .error(409, "SessionChanged");
    assert_eq!(
        server.get("/api/session", &bob_headers).body,
        json!({"principal":{"id":"bob","name":"Bob"}})
    );
    let duplicate = [
        ("Cookie", bob.as_str()),
        ("X-Concert-Principal", "bob"),
        ("X-Concert-Principal", "bob"),
    ];
    server
        .get("/api/reservations", &duplicate)
        .error(400, "InvalidInput");
    server
        .post("/api/logout", json!({}), &duplicate)
        .error(400, "InvalidInput");
    let unauthenticated = server.get("/api/reservations", &[("X-Concert-Principal", "bob")]);
    unauthenticated.error(401, "Unauthenticated");
    assert_eq!(
        unauthenticated.body["error"]["message"],
        "Sign in to manage your reservations."
    );
    assert_eq!(
        server
            .get("/api/session", &[("Cookie", &bob), ("Cookie", &bob)])
            .body,
        json!({"principal":null})
    );
    let alice = server.login("alice", None);
    let failed = server.post(
        "/api/login",
        json!({"username":["alice"],"password":"alice-demo"}),
        &[("Cookie", &alice)],
    );
    failed.error(401, "InvalidCredentials");
    assert!(!failed.headers.contains_key("set-cookie"));
    assert_eq!(
        server.get("/api/session", &[("Cookie", &alice)]).body,
        json!({"principal":null})
    );
    assert_eq!(
        server.get("/api/reservations", &bob_headers).body["active_count"],
        0
    );
    let out = server.post("/api/logout", json!({}), &bob_headers);
    assert_eq!(out.status, 200);
    assert_eq!(out.body, json!({"principal":null}));
    assert_eq!(
        out.headers["set-cookie"],
        "concert_session=; Max-Age=0; HttpOnly; SameSite=Strict; Path=/"
    );
    assert_eq!(server.post("/api/logout", json!({}), &stale).status, 200);
    server
        .get("/api/reservations", &bob_headers)
        .error(401, "Unauthenticated");
}

#[test]
fn reservations_keep_ownership_policy_and_replay_at_the_http_boundary() {
    let dir = tempdir().unwrap();
    let server = Server::start(&dir.path().join("db"));
    let alice = server.login("alice", None);
    let bob = server.login("bob", None);
    let alice = [("Cookie", alice.as_str())];
    let bob = [("Cookie", bob.as_str())];
    let reserve = |concert, key| json!({"concert_id":concert,"request_key":key,"owner_id":"bob","principal_id":"bob"});
    let first = reserve("midnight", "first");
    server
        .post("/api/reservations", first.clone(), &[])
        .error(401, "Unauthenticated");
    server
        .post(
            "/api/reservations",
            json!({"concert_id":"midnight","request_key":3}),
            &alice,
        )
        .error(400, "InvalidInput");
    let created = server.post("/api/reservations", first.clone(), &alice);
    assert_eq!(created.status, 200);
    let id = created.body["id"].as_str().unwrap();
    assert_eq!(created.body, json!({"id":id,"state":"reserved"}));
    assert_eq!(
        server.post("/api/reservations", first.clone(), &alice).body,
        created.body
    );
    assert_eq!(
        server
            .post("/api/reservations", reserve("northern", "second"), &alice)
            .status,
        200
    );
    server
        .post("/api/reservations", reserve("missing", "first"), &alice)
        .error(409, "IdempotencyConflict");
    server
        .post("/api/reservations", reserve("missing", "new"), &alice)
        .error(404, "ConcertNotFound");
    server
        .post("/api/reservations", reserve("matinee", "new"), &alice)
        .error(409, "ActiveReservationLimitReached");
    assert_eq!(
        server.get("/api/reservations", &bob).body,
        json!({"reservations":[],"active_limit":2,"active_count":0})
    );
    let cancel = format!("/api/reservations/{id}/cancel");
    let missing = server.post("/api/reservations/missing/cancel", json!({}), &bob);
    let foreign = server.post(&cancel, json!({"owner_id":"alice"}), &bob);
    foreign.error(404, "ReservationNotFound");
    assert_eq!(foreign.body, missing.body);
    let cancelled = server.post(&cancel, json!({}), &alice);
    assert_eq!(cancelled.body, json!({"id":id,"state":"cancelled"}));
    assert_eq!(
        server.post("/api/reservations", first, &alice).body,
        cancelled.body
    );
    assert_eq!(
        server
            .post("/api/reservations", reserve("matinee", "new"), &alice)
            .status,
        200
    );
    assert_eq!(
        server
            .post("/api/reservations", reserve("matinee", "first"), &bob)
            .status,
        200
    );
    server
        .post("/api/reservations", reserve("matinee", "new"), &bob)
        .error(409, "SoldOut");
    let history = server.get("/api/reservations", &alice).body;
    let rows = history["reservations"].as_array().unwrap();
    assert_eq!(rows.len(), 3);
    assert_eq!(history["active_count"], 2);
    assert_eq!(
        rows[2],
        json!({"id":id,"concert_id":"midnight","state":"cancelled"})
    );
    assert!(rows.iter().all(|row| row.as_object().unwrap().len() == 3));
}

#[cfg(unix)]
#[test]
fn shutdown_drains_an_admitted_reservation_before_checkpoint_and_restart() {
    use std::io::{BufRead, BufReader, Write};

    let dir = tempdir().unwrap();
    let path = dir.path().join("sessions.sqlite");
    let mut server = Server::start(&path);
    let cookie = server.login("alice", None);
    let mut stream = server.connect();
    let body = br#"{"concert_id":"matinee","request_key":"shutdown"}"#;
    write!(stream,"POST /api/reservations HTTP/1.1\r\nHost: localhost:{}\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\nExpect: 100-continue\r\nCookie: {cookie}\r\n\r\n",server.port,body.len()).unwrap();
    let mut reader = BufReader::new(stream);
    let mut status = String::new();
    reader.read_line(&mut status).unwrap();
    assert_eq!(status, "HTTP/1.1 100 Continue\r\n");
    status.clear();
    reader.read_line(&mut status).unwrap();
    assert_eq!(status, "\r\n");
    server.signal("-TERM");
    let mut stream = reader.into_inner();
    stream.write_all(body).unwrap();
    let reserved = support::Response::read(stream);
    assert_eq!(reserved.status, 200);
    server.wait_for_clean_exit();
    match std::fs::metadata(dir.path().join("sessions.sqlite-wal")) {
        Ok(metadata) => assert_eq!(metadata.len(), 0),
        Err(error) => assert_eq!(error.kind(), std::io::ErrorKind::NotFound),
    }

    let mut reopened = Server::start(&path);
    assert_eq!(
        reopened.get("/api/session", &[("Cookie", &cookie)]).body,
        json!({"principal":null})
    );
    let fresh = reopened.login("alice", None);
    let history = reopened
        .get("/api/reservations", &[("Cookie", &fresh)])
        .body;
    assert_eq!(
        history,
        json!({"active_count":1,"active_limit":2,"reservations":[
            {"id":reserved.body["id"],"concert_id":"matinee","state":"reserved"}
        ]})
    );
    reopened.signal("-INT");
    reopened.wait_for_clean_exit();
}

#[test]
fn cli_rejects_a_public_bind_and_incompatible_data_before_advertising_readiness() {
    let dir = tempdir().unwrap();
    let database = dir.path().join("db");
    let public = Command::new(env!("CARGO_BIN_EXE_gdd-concert"))
        .arg("--database")
        .arg(&database)
        .args(["--host", "0.0.0.0"])
        .output()
        .unwrap();
    assert!(!public.status.success());
    assert!(!database.exists());
    let original = include_bytes!("fixtures/over-limit.db");
    std::fs::write(&database, original).unwrap();
    let refused = Command::new(env!("CARGO_BIN_EXE_gdd-concert"))
        .arg("--database")
        .arg(&database)
        .args(["--port", "0"])
        .output()
        .unwrap();
    assert!(!refused.status.success());
    assert!(refused.stdout.is_empty());
    assert!(
        String::from_utf8(refused.stderr)
            .unwrap()
            .contains("explicit reconciliation")
    );
    assert_eq!(std::fs::read(database).unwrap(), original);
}
