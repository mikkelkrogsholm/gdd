//! Intercept completed reservation redirects between the real server and the browser.

use std::sync::{Arc, Mutex};

use ntex::{
    SharedCfg,
    client::Client,
    http::{Method, StatusCode, header},
    util::Bytes,
    web::{self, HttpRequest, HttpResponse, test::TestServer},
};
use serde::Deserialize;

#[derive(Deserialize)]
struct ReserveForm {
    request_key: String,
}

pub enum Redirect {
    Deliver,
    Disconnect,
    ServerError,
}

struct Reservations {
    redirect: Redirect,
    submissions: Vec<(StatusCode, String)>,
}

pub struct ReservationProxy {
    server: TestServer,
    reservations: Arc<Mutex<Reservations>>,
}

impl ReservationProxy {
    pub async fn start(upstream: String, redirect: Redirect) -> Self {
        let reservations = Arc::new(Mutex::new(Reservations {
            redirect,
            submissions: Vec::new(),
        }));
        let trace = Arc::clone(&reservations);
        let server = web::test::server(async move || {
            let client = Client::builder()
                .disable_redirects()
                .build(SharedCfg::default())
                .await
                .unwrap();
            let upstream = upstream.clone();
            let trace = Arc::clone(&trace);
            web::App::new().default_service(web::route().to(
                async move |req: HttpRequest, body: Bytes| {
                    forward(&client, &upstream, &trace, req, body).await
                },
            ))
        })
        .await;
        Self {
            server,
            reservations,
        }
    }

    pub fn site(&self) -> String {
        format!("http://{}", self.server.addr())
    }

    pub fn set_redirect(&self, redirect: Redirect) {
        self.reservations.lock().unwrap().redirect = redirect;
    }

    pub fn assert_submissions(&self, key: &str) {
        let mut trace = self.reservations.lock().unwrap();
        assert!(
            !trace.submissions.is_empty(),
            "no reservation reached the server"
        );
        for (status, posted_key) in trace.submissions.drain(..) {
            assert_eq!(status, StatusCode::SEE_OTHER);
            assert_eq!(posted_key, key);
        }
    }
}

async fn forward(
    client: &Client,
    upstream: &str,
    trace: &Mutex<Reservations>,
    req: HttpRequest,
    body: Bytes,
) -> HttpResponse {
    let mut request = client.request(req.method().clone(), format!("{upstream}{}", req.uri()));
    // The server requires the Host and Origin to match its own listener.
    for name in [header::COOKIE, header::CONTENT_TYPE] {
        if let Some(value) = req.headers().get(&name) {
            request = request.header(name, value.clone());
        }
    }
    if let Some(origin) = req.headers().get(header::ORIGIN) {
        if origin.as_bytes() != format!("http://{}", req.app_config().local_addr()).as_bytes() {
            return HttpResponse::Forbidden().finish();
        }
        request = request.header(header::ORIGIN, upstream);
    }
    let response = request
        .send_body(body.clone())
        .await
        .expect("forward browser request");
    let content = response.body().await.expect("complete upstream response");
    if req.method() == Method::POST && req.path() == "/forms/reserve" {
        let form: ReserveForm = serde_urlencoded::from_bytes(&body).unwrap();
        let mut trace = trace.lock().unwrap();
        trace
            .submissions
            .push((response.status(), form.request_key));
        if response.status() == StatusCode::SEE_OTHER {
            match trace.redirect {
                Redirect::Deliver => {}
                Redirect::Disconnect => req.io().unwrap().terminate(),
                Redirect::ServerError => {
                    return HttpResponse::InternalServerError()
                        .content_type("text/plain")
                        .body("Internal server error");
                }
            }
        }
    }
    let mut reply = HttpResponse::build(response.status());
    for (name, value) in response.headers() {
        if !matches!(
            *name,
            header::CONTENT_LENGTH | header::TRANSFER_ENCODING | header::CONNECTION
        ) {
            reply.header(name.clone(), value.clone());
        }
    }
    reply.body(content)
}
