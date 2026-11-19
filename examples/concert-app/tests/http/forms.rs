use super::{
    Server,
    support::{HttpClient, Response},
};
use serde_json::json;
use std::collections::HashMap;
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

fn concert_card<'a>(html: &'a str, concert: &str) -> &'a str {
    html.split_once(&format!("data-concert-id=\"{concert}\""))
        .unwrap()
        .1
        .split_once("</article>")
        .unwrap()
        .0
}

fn reserve_fields(html: &str, concert: &str) -> String {
    fields(concert_card(html, concert), "/forms/reserve")
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
    let original = reloaded_original.body.as_str().unwrap();
    assert_eq!(reserve_fields(original, "northern"), second);
    assert!(concert_card(original, "northern").contains("<button>Check reservation</button>"));
    assert!(concert_card(original, "matinee").contains("<button disabled>Allowance full</button>"));
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
fn sold_out_desk_replays_only_the_matching_principal_concert_and_key() {
    let dir = tempdir().unwrap();
    let server = Server::start(&dir.path().join("db"));
    let (alice, login) = sign_in(&server, "alice", "");
    let original = follow(&server, &login, &alice);
    let matinee = reserve_fields(original.body.as_str().unwrap(), "matinee");
    let created = submit(&server, "/forms/reserve", &matinee, &alice);
    let fresh = follow(&server, &created, &alice);
    let fresh_key = |concert| {
        let fields = reserve_fields(fresh.body.as_str().unwrap(), concert);
        serde_urlencoded::from_str::<HashMap<String, String>>(&fields).unwrap()["request_key"]
            .clone()
    };

    // Bob takes the other seat with the exact key on Alice's fresh matinee form.
    let bob = server.login("bob", None);
    let reserve = |cookie, concert, key| {
        server.post(
            "/api/reservations",
            json!({"concert_id":concert, "request_key":key}),
            &[("Cookie", cookie)],
        )
    };
    assert_eq!(reserve(&bob, "matinee", fresh_key("matinee")).status, 200);
    let original = follow(&server, &login, &alice);
    let html = original.body.as_str().unwrap();
    assert!(html.contains("1 of 2 active reservations"));
    let card = concert_card(html, "matinee");
    assert!(card.contains("Sold out · 2 seats"));
    assert!(card.contains("<button>Check reservation</button>"));

    // Alice binds her fresh northern key to a different concert and fills her allowance.
    assert_eq!(
        reserve(&alice, "midnight", fresh_key("northern")).status,
        200
    );
    let fresh = follow(&server, &created, &alice);
    let html = fresh.body.as_str().unwrap();
    assert!(concert_card(html, "matinee").contains("<button disabled>Sold out</button>"));
    for concert in ["midnight", "northern"] {
        assert!(concert_card(html, concert).contains("<button disabled>Allowance full</button>"));
    }
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
