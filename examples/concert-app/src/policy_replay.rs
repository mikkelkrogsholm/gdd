//! Replay the policy change through the loopback server against one database.

#[path = "../tests/http/support.rs"]
mod http;

use std::net::{Ipv4Addr, SocketAddr};

use gdd_concert::Reservations;
use http::HttpClient;
use serde_json::{Value, json};
use tokio::{net::TcpListener, sync::oneshot, task::spawn_blocking};

use crate::{api::App, server};

impl HttpClient for SocketAddr {
    fn address(&self) -> SocketAddr {
        *self
    }
}

async fn serve_phase<T: Send + 'static>(
    reservations: Reservations,
    exercise: impl FnOnce(SocketAddr) -> T + Send + 'static,
) -> T {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = App::new(reservations.clone(), address.port());
    let (done, shutdown) = oneshot::channel::<()>();
    let client = spawn_blocking(move || {
        // Dropping the sender also releases the server if the client panics.
        let _done = done;
        exercise(address)
    });
    let (served, result) = tokio::join!(
        server::serve(listener, app, async {
            let _ = shutdown.await;
        }),
        client
    );
    served.unwrap();
    reservations.checkpoint().await.unwrap();
    result.unwrap()
}

const HISTORY: &str = "/api/reservations";
const CONCERTS: [&str; 3] = ["midnight", "northern", "matinee"];

fn booking(concert: &str) -> Value {
    json!({"concert_id": concert, "request_key": format!("request-{concert}")})
}

fn assert_desk(server: SocketAddr, headers: &[(&str, &str)], summary: &str, button: &str) {
    let redirect = server.get("/", headers);
    assert_eq!(redirect.status, 303);
    let response = server.get(&redirect.headers["location"], headers);
    assert_eq!(response.status, 200);
    let page = response.body.as_str().unwrap();
    assert!(page.contains("Signed in as Alice"));
    assert!(page.contains(&format!("id=\"reservation-summary\">{summary}</p>")));
    assert_eq!(page.matches(button).count(), CONCERTS.len());
    println!("GET signed-in desk: {summary} {button}");
}

#[ntex::test]
async fn phase_a_to_b_over_loopback() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("reservations.db");
    let phase_a = Reservations::open_phase_a_for_replay(&path).await.unwrap();
    let ids = serve_phase(phase_a, |server| {
        let cookie = server.login("alice", None);
        println!("Phase A POST /api/login: Alice signed in");
        let headers = [("Cookie", cookie.as_str())];
        let ids = CONCERTS.map(|concert| {
            let created = server.post(HISTORY, booking(concert), &headers);
            assert_eq!(created.status, 200);
            assert_eq!(created.body["state"], "reserved");
            println!("Phase A POST request-{concert}: {}", created.body);
            created.body["id"].as_str().unwrap().to_owned()
        });
        let history = server.get(HISTORY, &headers);
        assert_eq!(history.status, 200);
        assert_eq!(history.body.get("active_limit"), Some(&Value::Null));
        assert_eq!(history.body["active_count"], 3);
        println!("Phase A GET {HISTORY}: {}", history.body);
        assert_desk(
            server,
            &headers,
            "3 active reservations.",
            "<button>Reserve one seat</button>",
        );
        let cancel = format!("{HISTORY}/{}/cancel", ids[2]);
        let cancelled = server.post(&cancel, json!({}), &headers);
        assert_eq!(cancelled.status, 200);
        assert_eq!(cancelled.body, json!({"id": ids[2], "state":"cancelled"}));
        println!("Phase A POST {cancel}: {}", cancelled.body);
        ids
    })
    .await;

    let phase_b = Reservations::open(&path).await.unwrap();
    serve_phase(phase_b, move |server| {
        let cookie = server.login("alice", None);
        println!("Phase B POST /api/login: Alice signed in");
        let headers = [("Cookie", cookie.as_str())];
        let history = server.get(HISTORY, &headers);
        assert_eq!(history.status, 200);
        assert_eq!(history.body["active_limit"], 2);
        assert_eq!(history.body["active_count"], 2);
        println!("Phase B GET {HISTORY}: {}", history.body);
        let rows = history.body["reservations"].as_array().unwrap();
        assert_eq!(rows.len(), 3);
        for ((id, concert), state) in
            ids.iter()
                .zip(CONCERTS)
                .zip(["reserved", "reserved", "cancelled"])
        {
            assert!(rows.contains(&json!({"id":id, "concert_id":concert, "state":state})));
            let replayed = server.post(HISTORY, booking(concert), &headers);
            assert_eq!(replayed.status, 200);
            assert_eq!(replayed.body, json!({"id":id, "state":state}));
            println!("Phase B replay request-{concert}: {}", replayed.body);
        }
        let rejected = server.post(
            HISTORY,
            json!({"concert_id":"matinee", "request_key":"fresh"}),
            &headers,
        );
        rejected.error(409, "ActiveReservationLimitReached");
        println!(
            "Phase B POST fresh key: {} {}",
            rejected.status, rejected.body
        );
        assert_desk(
            server,
            &headers,
            "2 of 2 active reservations.",
            "<button disabled>Allowance full</button>",
        );
    })
    .await;
}
