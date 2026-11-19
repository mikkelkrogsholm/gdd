//! Translate local HTTP requests into public identity and reservation operations.

mod error;
mod request;

use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{FromRequest, Request, State},
    http::{HeaderMap, HeaderValue, Method, Uri, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use gdd_concert::{Catalog, History, Principal, ReservationStatus, Reservations};
use serde_json::{Value, json};

use crate::identity::{LoginError, Sessions};
use error::Error;
use request::{JsonObject, check_local, expect_principal, token};

#[derive(Clone)]
struct App {
    sessions: Arc<Sessions>,
    reservations: Reservations,
}

pub fn router(reservations: Reservations, port: u16) -> Router {
    let hosts = [format!("127.0.0.1:{port}"), format!("localhost:{port}")];
    Router::new()
        .route("/api/session", get(session))
        .route("/api/login", post(login))
        .route("/api/logout", post(logout))
        .route("/api/concerts", get(catalog))
        .route("/api/reservations", get(history).post(reserve))
        .route("/api/reservations/{id}/cancel", post(cancel))
        .fallback(unknown)
        .method_not_allowed_fallback(unknown)
        .layer(middleware::from_fn_with_state(hosts, local_request))
        .with_state(App {
            sessions: Arc::default(),
            reservations,
        })
}

async fn local_request(State(hosts): State<[String; 2]>, request: Request, next: Next) -> Response {
    let mut response = match check_local(request.headers(), &hosts) {
        Err(error) => error.into_response(),
        Ok(()) if matches!(*request.method(), Method::GET | Method::POST) => {
            next.run(request).await
        }
        Ok(()) => Error::NotFound.into_response(),
    };
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json; charset=utf-8"),
    );
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("same-origin"),
    );
    headers.insert(header::CONTENT_SECURITY_POLICY, HeaderValue::from_static("default-src 'self'; script-src 'self'; style-src 'self'; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'self'"));
    response
}

async fn unknown(request: Request) -> Result<(), Error> {
    if request.method() == Method::POST {
        JsonObject::from_request(request, &()).await?;
    }
    Err(Error::NotFound)
}

fn principal(app: &App, headers: &HeaderMap) -> Result<Principal<'static>, Error> {
    let id = app.sessions.profile(token(headers)).map(|p| p.id);
    let principal = Principal::from_session(id).map_err(|_| Error::Unauthenticated)?;
    expect_principal(headers, id)?;
    Ok(principal)
}

async fn session(State(app): State<App>, headers: HeaderMap) -> Json<Value> {
    Json(json!({"principal": app.sessions.profile(token(&headers))}))
}

async fn login(
    State(app): State<App>,
    headers: HeaderMap,
    body: JsonObject,
) -> Result<Response, Error> {
    let (token, profile) = app
        .sessions
        .login(
            token(&headers),
            body.string("username"),
            body.string("password"),
        )
        .map_err(|error| match error {
            LoginError::InvalidCredentials => Error::InvalidCredentials,
            LoginError::RandomnessUnavailable => Error::Internal,
        })?;
    let cookie = format!("concert_session={token}; HttpOnly; SameSite=Strict; Path=/");
    Ok((
        [(header::SET_COOKIE, cookie)],
        Json(json!({"principal": profile})),
    )
        .into_response())
}

async fn logout(
    State(app): State<App>,
    headers: HeaderMap,
    _: JsonObject,
) -> Result<Response, Error> {
    let token = token(&headers);
    expect_principal(&headers, app.sessions.profile(token).map(|p| p.id))?;
    app.sessions.logout(token);
    Ok((
        [(
            header::SET_COOKIE,
            "concert_session=; Max-Age=0; HttpOnly; SameSite=Strict; Path=/",
        )],
        Json(json!({"principal": null})),
    )
        .into_response())
}

async fn catalog(State(app): State<App>) -> Result<Json<Catalog>, Error> {
    Ok(Json(app.reservations.catalog().await?))
}

async fn history(State(app): State<App>, headers: HeaderMap) -> Result<Json<History>, Error> {
    Ok(Json(
        app.reservations
            .list_own(principal(&app, &headers)?)
            .await?,
    ))
}

async fn reserve(
    State(app): State<App>,
    headers: HeaderMap,
    body: JsonObject,
) -> Result<Json<ReservationStatus>, Error> {
    Ok(Json(
        app.reservations
            .reserve(
                principal(&app, &headers)?,
                body.string("concert_id").unwrap_or_default(),
                body.string("request_key").unwrap_or_default(),
            )
            .await?,
    ))
}

async fn cancel(
    State(app): State<App>,
    headers: HeaderMap,
    uri: Uri,
    _: JsonObject,
) -> Result<Json<ReservationStatus>, Error> {
    // The public identifier alphabet applies to the original, unescaped path.
    let id = uri
        .path()
        .strip_prefix("/api/reservations/")
        .and_then(|s| s.strip_suffix("/cancel"))
        .filter(|id| {
            !id.is_empty()
                && id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        })
        .ok_or(Error::NotFound)?;
    Ok(Json(
        app.reservations
            .cancel(principal(&app, &headers)?, id)
            .await?,
    ))
}
