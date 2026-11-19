use super::support::{Response, Server};
use tempfile::tempdir;

fn submit(server: &Server, action: &str, body: &str, cookie: &str) -> Response {
    server.raw(
        "POST",
        action,
        &[
            ("Content-Type", "application/x-www-form-urlencoded"),
            ("Content-Length", &body.len().to_string()),
            ("Cookie", cookie),
        ],
        body.as_bytes(),
    )
}

fn follow(server: &Server, response: &Response, cookie: &str) -> Response {
    assert_eq!(response.status, 303);
    server.get(&response.headers["location"], &[("Cookie", cookie)])
}

fn sign_in(server: &Server, name: &str, previous: &str) -> (String, Response) {
    let response = submit(
        server,
        "/forms/login",
        &format!("username={name}&password={name}-demo"),
        previous,
    );
    assert_eq!(response.status, 303);
    let cookie = response.headers["set-cookie"]
        .strip_suffix("; HttpOnly; SameSite=Strict; Path=/")
        .unwrap();
    (cookie.to_owned(), response)
}

// Read the values the actual server rendered, including the session expectation
// and the saved request key, so a missing field breaks the normal form flow.
fn fields(html: &str, action: &str) -> String {
    let form = html
        .split_once(&format!("method=\"post\" action=\"{action}\">"))
        .unwrap()
        .1
        .split_once("</form>")
        .unwrap()
        .0;
    let inputs: Vec<_> = form
        .split("<input type=\"hidden\" name=\"")
        .skip(1)
        .map(|input| {
            let (name, rest) = input.split_once("\" value=\"").unwrap();
            (name, rest.split_once('"').unwrap().0)
        })
        .collect();
    serde_urlencoded::to_string(inputs).unwrap()
}

fn reserve_fields(html: &str, concert: &str) -> String {
    let card = html
        .split_once(&format!("data-concert-id=\"{concert}\""))
        .unwrap()
        .1
        .split_once("</article>")
        .unwrap()
        .0;
    fields(card, "/forms/reserve")
}

#[test]
fn program_login_reservation_retries_and_cancellation_use_rendered_forms() {
    let dir = tempdir().unwrap();
    let server = Server::start(&dir.path().join("db"));
    let program = server.get("/", &[]);
    assert_eq!(program.status, 200);
    assert!(program.headers["content-security-policy"].contains("script-src 'none'"));
    let html = program.body.as_str().unwrap();
    assert!(html.contains("Midnight Sessions"));
    assert!(html.contains("Sign in to reserve"));
    assert!(!html.contains("<script"));
    assert!(!html.contains("Your reservations</h2>"));
    let css = server.get("/style.css", &[]);
    assert_eq!(
        css.body.as_str().unwrap(),
        include_str!("../../concert_app/static/style.css")
    );
    let invalid = submit(&server, "/forms/login", "username=alice&password=wrong", "");
    assert_eq!(invalid.status, 401);
    assert!(
        invalid
            .body
            .as_str()
            .unwrap()
            .contains("credentials did not match")
    );

    let (cookie, login) = sign_in(&server, "alice", "");
    let page = follow(&server, &login, &cookie);
    let html = page.body.as_str().unwrap();
    assert!(html.contains("Signed in as Alice"));
    assert!(html.contains("0 of 2"));
    let first = reserve_fields(html, "midnight");
    let second = reserve_fields(html, "northern");
    let third = reserve_fields(html, "matinee");
    assert_ne!(first, second);
    let post = |body: &str| submit(&server, "/forms/reserve", body, &cookie);
    let created = post(&first);
    let loaded = follow(&server, &created, &cookie);
    let loaded = loaded.body.as_str().unwrap();
    assert!(loaded.contains("Your seat is reserved."));
    assert_ne!(first, reserve_fields(loaded, "midnight"));
    let cancel_first = fields(loaded, "/forms/cancel");

    // Discard the second redirect and replay the submitted page after commit.
    assert_eq!(post(&second).status, 303);
    let reloaded_original = follow(&server, &login, &cookie);
    assert_eq!(
        reserve_fields(reloaded_original.body.as_str().unwrap(), "northern"),
        second
    );
    let replay = follow(&server, &post(&second), &cookie);
    let replay = replay.body.as_str().unwrap();
    assert!(replay.contains("2 of 2"));
    assert_eq!(replay.matches("data-reservation-id=").count(), 2);
    assert!(replay.contains("<button disabled>Allowance full</button>"));

    let limited = post(&third);
    assert_eq!(limited.status, 409);
    let limited = limited.body.as_str().unwrap();
    assert!(limited.contains("Your active reservation limit has been reached."));
    assert_eq!(fields(limited, "/forms/reserve"), third);
    let original_desk = login.headers["location"].split_once('?').unwrap().0;
    assert!(limited.contains(&format!(
        r#"<a href="{original_desk}">Return to the concert program"#
    )));
    let cancelled = submit(&server, "/forms/cancel", &cancel_first, &cookie);
    assert!(
        follow(&server, &cancelled, &cookie)
            .body
            .as_str()
            .unwrap()
            .contains("1 of 2")
    );
    let replay = follow(&server, &post(&first), &cookie);
    assert!(
        replay
            .body
            .as_str()
            .unwrap()
            .contains("Your reservation is cancelled.")
    );
    assert_eq!(post(&third).status, 303);
    let out = submit(
        &server,
        "/forms/logout",
        &fields(html, "/forms/logout"),
        &cookie,
    );
    assert!(out.headers["set-cookie"].contains("Max-Age=0"));
    assert!(
        follow(&server, &out, "")
            .body
            .as_str()
            .unwrap()
            .contains("You're signed out")
    );
}

#[test]
fn stale_desk_refresh_redirect_and_submitted_forms_preserve_the_current_session() {
    let dir = tempdir().unwrap();
    let server = Server::start(&dir.path().join("db"));
    let (alice, login) = sign_in(&server, "alice", "");
    let desk = follow(&server, &login, &alice);
    let html = desk.body.as_str().unwrap();
    let reserve = reserve_fields(html, "midnight");
    let created = submit(&server, "/forms/reserve", &reserve, &alice);
    let alice_desk = follow(&server, &created, &alice);
    let cancel = fields(alice_desk.body.as_str().unwrap(), "/forms/cancel");
    let logout = fields(html, "/forms/logout");
    // The first logout succeeds but its response is lost. Another tab signs in.
    assert_eq!(
        submit(&server, "/forms/logout", &logout, &alice).status,
        303
    );
    let (bob, bob_login) = sign_in(&server, "bob", &alice);
    let bob_page = follow(&server, &bob_login, &bob);
    let bob_reserve = reserve_fields(bob_page.body.as_str().unwrap(), "northern");
    let bob_created = submit(&server, "/forms/reserve", &bob_reserve, &bob);
    for stale in [
        follow(&server, &login, &bob),
        follow(&server, &created, &bob),
        submit(&server, "/forms/reserve", &reserve, &bob),
        submit(&server, "/forms/cancel", &cancel, &bob),
        submit(&server, "/forms/logout", &logout, &bob),
    ] {
        assert_eq!(stale.status, 409);
        let html = stale.body.as_str().unwrap();
        assert!(html.contains("Sign in again"));
        assert!(html.contains("id=\"sign-in-form\""));
        assert!(!html.contains("Signed in as"));
        assert!(!html.contains("data-reservation-id"));
        assert!(!stale.headers.contains_key("set-cookie"));
    }
    let retained = follow(&server, &bob_created, &bob);
    assert!(retained.body.as_str().unwrap().contains("Signed in as Bob"));
    assert!(retained.body.as_str().unwrap().contains("1 of 2"));
    let (alice_again, new_desk) = sign_in(&server, "alice", &bob);
    assert_eq!(
        submit(&server, "/forms/logout", &logout, &alice_again).status,
        409
    );
    let restored = follow(&server, &new_desk, &alice_again);
    assert!(restored.body.as_str().unwrap().contains("1 of 2"));
}
