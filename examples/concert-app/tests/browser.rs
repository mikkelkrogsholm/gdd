#![cfg(any(unix, windows))]

#[path = "support/response_loss.rs"]
mod response_loss;
#[path = "support/server.rs"]
mod server;
#[path = "support/webdriver.rs"]
mod webdriver;

use std::{fs, path::Path};

use response_loss::{Redirect, ReservationProxy};
use serde_json::json;
use server::Server;
use webdriver::Browser;

const MIDNIGHT: &str = "22295aa924aa4f62bcf528cfc046cad7";
const NORTHERN: &str = "2d3f2bc9fed44168b9a47eb2e163061e";
const MATINEE: &str = "6122fbd2484a4c55a5c84026a1df2f24";
const RESERVE: &str = "[data-concert-id='midnight'] button";

fn sign_in(browser: &Browser, user: &str, password: &str) {
    browser.type_text("#username", user);
    browser.type_text("#password", password);
    browser.click("#sign-in");
}

// Match each rendered card and API record by reference, independent of display order.
fn assert_desk(
    browser: &Browser,
    site: &str,
    user: &str,
    expected: &[(&str, &str, &str)],
    disabled: usize,
) {
    assert_eq!(
        browser.text("#session-status"),
        format!("Signed in as {user}")
    );
    let active = expected
        .iter()
        .filter(|(_, _, state)| *state == "reserved")
        .count();
    let history = browser.api_history(site);
    assert_eq!(history["active_count"], active);
    assert_eq!(history["active_limit"], 2);
    assert_eq!(
        browser.text("#reservation-summary"),
        format!("{active} of 2 active reservations.")
    );
    let records = history["reservations"].as_array().unwrap();
    assert_eq!(records.len(), expected.len());
    assert_eq!(browser.count("[data-reservation-id]"), expected.len());
    for (id, concert, state) in expected {
        assert!(
            records.contains(&json!({"id":id, "concert_id":concert, "state":state})),
            "{history}"
        );
        let card = format!("[data-reservation-id='{id}']");
        assert_eq!(
            browser.text(&format!("{card} .reservation-reference")),
            format!("Reference {id}")
        );
        assert_eq!(
            browser
                .text(&format!("{card} .reservation-state"))
                .to_lowercase(),
            *state
        );
    }
    assert_eq!(browser.count("#concerts button:disabled"), disabled);
}

fn assert_stale(browser: &Browser) {
    assert!(
        browser
            .text("#error-message")
            .contains("Your session changed in another tab")
    );
    assert_eq!(browser.text("#session-status"), "You're signed out");
    assert_eq!(browser.count("#sign-in-form"), 1);
    assert_eq!(
        browser.count("#reservations-section, [data-reservation-id], #sign-out"),
        0
    );
}

async fn assert_database(database: &Path, expected: &[(&str, &str, &str)]) {
    let db = turso::Builder::new_local(database.to_str().unwrap())
        .build()
        .await
        .unwrap();
    let conn = db.connect().unwrap();
    let mut rows = conn
        .query(
            "SELECT principal_id, id, concert_id, state FROM reservations",
            (),
        )
        .await
        .unwrap();
    let mut count = 0;
    while let Some(row) = rows.next().await.unwrap() {
        assert_eq!(row.get::<String>(0).unwrap(), "alice");
        let id: String = row.get(1).unwrap();
        let concert: String = row.get(2).unwrap();
        let state: String = row.get(3).unwrap();
        assert!(
            expected.contains(&(id.as_str(), concert.as_str(), state.as_str())),
            "unexpected persisted reservation {id} {concert} {state}"
        );
        count += 1;
    }
    assert_eq!(count, expected.len());
}

#[tokio::test]
#[ignore = "requires local Chromium and a compatible ChromeDriver"]
async fn recorded_phase_a_history_and_shared_cookie_forms_in_chromium() {
    let dir = tempfile::tempdir().unwrap();
    let database = dir.path().join("concert.sqlite");
    fs::write(&database, include_bytes!("fixtures/recorded-phase-a.db")).unwrap();
    let server = Server::start(&database);
    let site = format!("http://127.0.0.1:{}", server.port);

    let browser = Browser::start();
    browser.open(&site);
    assert_eq!(browser.text("#session-status"), "You're signed out");
    assert_eq!(
        browser.text("[data-concert-id='midnight'] h3"),
        "Midnight Sessions"
    );
    assert_eq!(browser.count("#concerts button:disabled"), 3);
    assert_eq!(browser.count("#reservations-section, script"), 0);

    sign_in(&browser, "alice", "incorrect");
    assert!(
        browser
            .text("#error-message")
            .contains("credentials did not match")
    );
    assert_eq!(browser.count("#reservations-section"), 0);
    sign_in(&browser, "alice", "alice-demo");
    let mut expected = vec![
        (MIDNIGHT, "midnight", "cancelled"),
        (NORTHERN, "northern", "reserved"),
        (MATINEE, "matinee", "reserved"),
    ];
    assert_desk(&browser, &site, "Alice", &expected, 3);
    assert_eq!(browser.text(RESERVE), "Allowance full");
    browser.click(&format!("[data-reservation-id='{NORTHERN}'] button"));
    expected[1] = (NORTHERN, "northern", "cancelled");
    assert_desk(&browser, &site, "Alice", &expected, 0);
    assert_eq!(browser.text(RESERVE), "Reserve one seat");

    // The current Alice URL includes the cancellation notice. Refreshing its history
    // submits a new GET to the desk after another tab has changed the shared cookie.
    let stale_read = browser.get("/window");
    let stale_reserve = browser.new_tab(&site);
    let stale_logout = browser.new_tab(&site);
    let current = browser.new_tab(&site);
    browser.click(RESERVE);
    assert_eq!(browser.text("#status-message"), "Your seat is reserved.");
    let history = browser.api_history(&site);
    let new_seat = history["reservations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|record| record["concert_id"] == "midnight" && record["state"] == "reserved")
        .expect("new active Midnight Sessions seat");
    let new_id = new_seat["id"].as_str().unwrap();
    assert!(![MIDNIGHT, NORTHERN, MATINEE].contains(&new_id));
    expected.push((new_id, "midnight", "reserved"));
    assert_desk(&browser, &site, "Alice", &expected, 3);

    browser.click("#sign-out");
    sign_in(&browser, "bob", "bob-demo");
    assert_desk(&browser, &site, "Bob", &[], 0);
    for (tab, action) in [
        (&stale_read, "#refresh-reservations"),
        (&stale_reserve, RESERVE),
        (&stale_logout, "#sign-out"),
    ] {
        browser.switch(tab);
        browser.click(action);
        assert_stale(&browser);
        browser.switch(&current);
        browser.open(&site);
        assert_desk(&browser, &site, "Bob", &[], 0);
    }
    browser.click("#sign-out");
    assert_eq!(browser.text("#session-status"), "You're signed out");
    assert_eq!(browser.count("#reservations-section"), 0);
    // Headless Chromium enforces a minimum desktop window width. Override its
    // content viewport to exercise the public program at 320 CSS pixels.
    browser.cdp(
        "Emulation.setDeviceMetricsOverride",
        json!({"width": 320, "height": 710, "deviceScaleFactor": 1, "mobile": false}),
    );
    browser.open(&site);
    assert_eq!(
        browser.text("[data-concert-id='midnight'] h3"),
        "Midnight Sessions"
    );
    for selector in ["body", "#sign-in-form", "[data-concert-id='midnight']"] {
        let rect = browser.rect(selector);
        let left = rect["x"].as_f64().unwrap();
        let width = rect["width"].as_f64().unwrap();
        assert!(
            left >= 0.0 && width > 0.0 && left + width <= 320.0,
            "{selector} outside narrow viewport: {rect}"
        );
    }
    drop(browser);

    #[cfg(unix)]
    {
        let mut server = server;
        server.signal("-TERM");
        server.wait_for_clean_exit();
    }
    #[cfg(windows)]
    drop(server);
    assert_database(&database, &expected).await;
}

#[ntex::test]
#[ignore = "requires local Chromium and a compatible ChromeDriver"]
async fn recover_a_committed_reservation_after_a_lost_response_in_chromium() {
    const KEY: &str = "[data-concert-id='midnight'] input[name='request_key']";
    let dir = tempfile::tempdir().unwrap();
    let server = Server::start(&dir.path().join("concert.sqlite"));
    let upstream = format!("http://127.0.0.1:{}", server.port);
    let proxy = ReservationProxy::start(upstream.clone(), Redirect::Disconnect).await;
    let site = proxy.site();
    let browser = Browser::start();
    browser.open(&site);
    sign_in(&browser, "alice", "alice-demo");
    assert_desk(&browser, &upstream, "Alice", &[], 0);
    let original = browser.get("/url").as_str().unwrap().to_owned();
    let key = browser.value(KEY);
    assert!(!key.is_empty());
    let cookie = browser.session_cookie();

    browser.click(RESERVE);
    proxy.assert_submissions(&key);
    // Chromium has finished the failed navigation, including any automatic retries.
    let frame = browser.cdp("Page.getFrameTree", json!({}));
    assert_eq!(
        frame["frameTree"]["frame"]["unreachableUrl"],
        format!("{site}/forms/reserve")
    );
    let history = browser.api_history_with_cookie(&upstream, &cookie);
    assert_eq!(history["active_count"], 1);
    let records = history["reservations"].as_array().unwrap();
    assert_eq!(records.len(), 1);
    let id = records[0]["id"].as_str().unwrap();
    assert!(!id.is_empty());
    assert_eq!(
        records[0],
        json!({"id": id, "concert_id": "midnight", "state": "reserved"})
    );

    let active = [(id, "midnight", "reserved")];
    let cancelled = [(id, "midnight", "cancelled")];

    browser.post("/back", json!({}));
    assert_eq!(browser.get("/url"), original);
    assert_eq!(browser.value(KEY), key);
    proxy.set_redirect(Redirect::Deliver);
    browser.click(RESERVE);
    proxy.assert_submissions(&key);
    assert_eq!(browser.text("#status-message"), "Your seat is reserved.");
    assert_desk(&browser, &upstream, "Alice", &active, 0);

    browser.click(&format!("[data-reservation-id='{id}'] button"));
    assert_desk(&browser, &upstream, "Alice", &cancelled, 0);
    browser.open(&original);
    assert_eq!(browser.value(KEY), key);
    browser.click(RESERVE);
    proxy.assert_submissions(&key);
    assert_eq!(
        browser.text("#status-message"),
        "Your reservation is cancelled."
    );
    assert_desk(&browser, &upstream, "Alice", &cancelled, 0);
}

fn assert_reflow(browser: &Browser, zoom: f64) {
    let metrics = browser.cdp("Page.getLayoutMetrics", json!({}));
    let visual = &metrics["cssVisualViewport"];
    assert_eq!(visual["zoom"].as_f64(), Some(zoom), "{metrics}");
    assert_eq!(visual["scale"].as_f64(), Some(1.0), "{metrics}");
    let width = metrics["cssLayoutViewport"]["clientWidth"]
        .as_f64()
        .unwrap();
    // Both windows provide 320 CSS pixels before space for a vertical scrollbar.
    assert!((300.0..=320.0).contains(&width), "{metrics}");
    assert!(
        metrics["cssContentSize"]["width"].as_f64().unwrap() <= width + 1.0,
        "{metrics}"
    );
    let controls = browser.visible_rects("button, input");
    assert!(!controls.is_empty(), "no visible controls");
    for rect in controls {
        let x = rect["x"].as_f64().unwrap();
        let w = rect["width"].as_f64().unwrap();
        let h = rect["height"].as_f64().unwrap();
        assert!(
            x >= 0.0 && w > 0.0 && x + w <= width + 1.0,
            "control overflows {width}px: {rect}"
        );
        assert!(h >= 44.0, "control below 44 CSS pixels: {rect}");
    }
}

fn assert_reduced_motion(browser: &Browser) {
    browser.cdp("DOM.enable", json!({}));
    browser.cdp("CSS.enable", json!({}));
    let document = browser.cdp("DOM.getDocument", json!({}));
    let body = browser.cdp(
        "DOM.querySelector",
        json!({
            "nodeId": document["root"]["nodeId"], "selector": "body"
        }),
    );
    let styles = browser.cdp(
        "CSS.getMatchedStylesForNode",
        json!({"nodeId": body["nodeId"]}),
    );
    let reduced = styles["matchedCSSRules"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|entry| entry["rule"]["media"].as_array())
        .flatten()
        .any(|media| media["text"] == "(prefers-reduced-motion: reduce)");
    assert!(
        reduced,
        "the body's styles did not match the reduced-motion rule"
    );
}

#[test]
#[ignore = "requires local Chromium and a compatible ChromeDriver"]
fn keyboard_sign_in_and_reflow_at_320_pixels_and_200_percent_zoom_in_chromium() {
    const TAB: &str = "\u{e004}";
    const ENTER: &str = "\u{e007}";
    let dir = tempfile::tempdir().unwrap();
    let database = dir.path().join("concert.sqlite");
    fs::write(&database, include_bytes!("fixtures/recorded-phase-a.db")).unwrap();
    let server = Server::start(&database);
    let site = format!("http://127.0.0.1:{}", server.port);

    for zoom in [1.0_f64, 2.0] {
        // Chrome stores per-host zoom as a level, with a factor of 1.2 per level.
        let browser = Browser::with_preferences(json!({
            "partition.per_host_zoom_levels": {"x": {"127.0.0.1": {"zoom_level": zoom.log(1.2)}}}
        }));
        if zoom == 1.0 {
            browser.cdp(
                "Emulation.setDeviceMetricsOverride",
                json!({"width": 320, "height": 900, "deviceScaleFactor": 1, "mobile": false}),
            );
        } else {
            let window = browser.post("/window/rect", json!({"width": 640, "height": 900}));
            assert_eq!(window["width"], 640);
        }
        browser.cdp(
            "Emulation.setEmulatedMedia",
            json!({"features": [
                {"name": "prefers-reduced-motion", "value": "reduce"}
            ]}),
        );
        browser.open(&site);
        assert_reduced_motion(&browser);
        assert_reflow(&browser, zoom);

        // Reach the form from the start of the document using only the keyboard.
        for _ in 0..browser.count("a[href], input, button, [tabindex]") {
            browser.keys(TAB);
            if browser.focused("#username") {
                break;
            }
        }
        assert!(browser.focused("#username"));
        assert_eq!(browser.text("label[for='username']"), "Username");
        browser.keys("alice");
        browser.keys(TAB);
        assert!(browser.focused("#password"));
        assert_eq!(browser.text("label[for='password']"), "Password");
        browser.keys("alice-demo");
        browser.keys(TAB);
        assert!(browser.focused("#sign-in"));
        browser.navigate_with(|| browser.keys(ENTER));
        assert_eq!(browser.text("#session-status"), "Signed in as Alice");
        assert_eq!(browser.count("#refresh-reservations:enabled"), 1);
        assert_eq!(browser.count("[role='status']"), 1);
        assert_eq!(browser.text("[role='status']"), "You're signed in.");
        assert_eq!(browser.count("[role='alert']"), 1);
        assert_reflow(&browser, zoom);
    }
}

#[ntex::test]
#[ignore = "requires local Chromium and a compatible ChromeDriver"]
async fn recover_the_last_seat_from_the_original_desk_after_http_500_in_chromium() {
    const BUTTON: &str = "[data-concert-id='matinee'] button";
    const KEY: &str = "[data-concert-id='matinee'] input[name='request_key']";
    let dir = tempfile::tempdir().unwrap();
    let server = Server::start(&dir.path().join("concert.sqlite"));
    let upstream = format!("http://127.0.0.1:{}", server.port);
    let proxy = ReservationProxy::start(upstream.clone(), Redirect::Deliver).await;
    let site = proxy.site();
    let browser = Browser::start();
    browser.open(&site);
    sign_in(&browser, "alice", "alice-demo");

    let first_key = browser.value(KEY);
    browser.click(BUTTON);
    proxy.assert_submissions(&first_key);
    let history = browser.api_history(&upstream);
    let first = history["reservations"][0]["id"].as_str().unwrap();
    assert_desk(
        &browser,
        &upstream,
        "Alice",
        &[(first, "matinee", "reserved")],
        0,
    );

    let original = browser.get("/url").as_str().unwrap().to_owned();
    let second_key = browser.value(KEY);
    assert_ne!(second_key, first_key);
    proxy.set_redirect(Redirect::ServerError);
    browser.click(BUTTON);
    proxy.assert_submissions(&second_key);
    assert_eq!(browser.text("body"), "Internal server error");

    // Check the real server before revisiting the program or retrying either form.
    let history = browser.api_history(&upstream);
    let records = history["reservations"].as_array().unwrap();
    assert_eq!(history["active_count"], 2);
    assert_eq!(records.len(), 2);
    assert!(records.contains(&json!({"id":first,"concert_id":"matinee","state":"reserved"})));
    let other = records.iter().find(|record| record["id"] != first).unwrap();
    let second = other["id"].as_str().unwrap();
    assert!(!second.is_empty());
    assert_eq!(
        *other,
        json!({"id":second,"concert_id":"matinee","state":"reserved"})
    );
    let active = [
        (first, "matinee", "reserved"),
        (second, "matinee", "reserved"),
    ];

    // A new GET must render the committed history and the same enabled form.
    browser.open(&original);
    assert_desk(&browser, &upstream, "Alice", &active, 2);
    assert_eq!(browser.value(KEY), second_key);
    assert_eq!(
        browser.text(&format!("{BUTTON}:enabled")),
        "Check reservation"
    );
    assert_eq!(browser.text(RESERVE), "Allowance full");
    assert!(
        browser
            .text("[data-concert-id='matinee'] .availability")
            .contains("Sold out")
    );
    proxy.set_redirect(Redirect::Deliver);
    browser.click(BUTTON);
    proxy.assert_submissions(&second_key);
    assert_eq!(browser.text("#status-message"), "Your seat is reserved.");

    // The redirect opens a fresh page, with a new key blocked by both limits.
    assert_desk(&browser, &upstream, "Alice", &active, 3);
    assert_ne!(browser.value(KEY), second_key);
    assert_eq!(browser.text(&format!("{BUTTON}:disabled")), "Sold out");

    browser.click(&format!("[data-reservation-id='{second}'] button"));
    let cancelled = [
        (first, "matinee", "reserved"),
        (second, "matinee", "cancelled"),
    ];
    assert_desk(&browser, &upstream, "Alice", &cancelled, 0);
    browser.open(&original);
    assert_eq!(browser.value(KEY), second_key);
    assert_eq!(
        browser.text(&format!("{BUTTON}:enabled")),
        "Check reservation"
    );
    browser.click(BUTTON);
    proxy.assert_submissions(&second_key);
    assert_eq!(
        browser.text("#status-message"),
        "Your reservation is cancelled."
    );
    assert_desk(&browser, &upstream, "Alice", &cancelled, 0);
}
