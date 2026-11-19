//! Translate local HTTP requests into public identity and reservation operations.

mod error;
mod request;

use gdd_concert::{Principal, Reservations};
use ntex::http::{
    HeaderMap, Method, Request, Response, ResponseBuilder, ResponseError, StatusCode, header,
};
use serde_json::json;

use crate::identity::{LoginError, Sessions};
pub use error::Error;
use request::{JsonObject, check_local, expect_principal, token};

pub struct App {
    sessions: Sessions,
    reservations: Reservations,
    hosts: [String; 2],
}

pub fn invalid_framing() -> Response {
    Error::invalid_framing().error_response()
}

fn json_response(status: StatusCode) -> ResponseBuilder {
    let mut response = Response::build(status);
    response
        .content_type("application/json; charset=utf-8")
        .header(header::CACHE_CONTROL, "no-store")
        .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
        .header(header::REFERRER_POLICY, "same-origin")
        .header(header::CONTENT_SECURITY_POLICY, "default-src 'self'; script-src 'self'; style-src 'self'; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'self'");
    response
}

impl App {
    pub fn new(reservations: Reservations, port: u16) -> Self {
        Self {
            sessions: Sessions::default(),
            reservations,
            hosts: [format!("127.0.0.1:{port}"), format!("localhost:{port}")],
        }
    }

    pub fn validate_head(&self, request: &Request) -> Result<Option<usize>, Error> {
        check_local(request.headers(), &self.hosts)?;
        match *request.method() {
            Method::GET => Ok(None),
            Method::POST => JsonObject::length(request.headers()).map(Some),
            _ => Err(Error::NotFound),
        }
    }

    pub async fn handle(&self, mut request: Request) -> Result<Response, Error> {
        let result = match self.validate_head(&request)? {
            None => self.get(&request).await,
            Some(length) => {
                let body = JsonObject::read(&mut request, length).await?;
                self.post(&request, &body).await
            }
        };
        // Admission errors close the connection because its body may be unread.
        // Fully admitted requests may reuse the connection after a domain error.
        Ok(result.unwrap_or_else(|error| error.response()))
    }

    async fn get(&self, request: &Request) -> Result<Response, Error> {
        let headers = request.headers();
        let mut ok = json_response(StatusCode::OK);
        match request.path() {
            "/api/session" => {
                Ok(ok.json(&json!({"principal": self.sessions.profile(token(headers))})))
            }
            "/api/concerts" => Ok(ok.json(&self.reservations.catalog().await?)),
            "/api/reservations" => {
                Ok(ok.json(&self.reservations.list_own(self.principal(headers)?).await?))
            }
            _ => Err(Error::NotFound),
        }
    }

    async fn post(&self, request: &Request, body: &JsonObject) -> Result<Response, Error> {
        let headers = request.headers();
        match request.path() {
            "/api/login" => self.login(headers, body),
            "/api/logout" => self.logout(headers),
            "/api/reservations" => {
                let reserved = self
                    .reservations
                    .reserve(
                        self.principal(headers)?,
                        body.string("concert_id").unwrap_or_default(),
                        body.string("request_key").unwrap_or_default(),
                    )
                    .await?;
                Ok(json_response(StatusCode::OK).json(&reserved))
            }
            path => {
                let id = cancellation_id(path).ok_or(Error::NotFound)?;
                let cancelled = self
                    .reservations
                    .cancel(self.principal(headers)?, id)
                    .await?;
                Ok(json_response(StatusCode::OK).json(&cancelled))
            }
        }
    }

    fn principal(&self, headers: &HeaderMap) -> Result<Principal<'static>, Error> {
        let id = self.sessions.profile(token(headers)).map(|p| p.id);
        let principal = Principal::from_session(id).map_err(|_| Error::Unauthenticated)?;
        expect_principal(headers, id)?;
        Ok(principal)
    }

    fn login(&self, headers: &HeaderMap, body: &JsonObject) -> Result<Response, Error> {
        let (token, profile) = self
            .sessions
            .login(
                token(headers),
                body.string("username"),
                body.string("password"),
            )
            .map_err(|error| match error {
                LoginError::InvalidCredentials => Error::InvalidCredentials,
                LoginError::RandomnessUnavailable => Error::Internal,
            })?;
        let cookie = format!("concert_session={token}; HttpOnly; SameSite=Strict; Path=/");
        Ok(json_response(StatusCode::OK)
            .header(header::SET_COOKIE, cookie)
            .json(&json!({"principal": profile})))
    }

    fn logout(&self, headers: &HeaderMap) -> Result<Response, Error> {
        let token = token(headers);
        expect_principal(headers, self.sessions.profile(token).map(|p| p.id))?;
        self.sessions.logout(token);
        Ok(json_response(StatusCode::OK)
            .header(
                header::SET_COOKIE,
                "concert_session=; Max-Age=0; HttpOnly; SameSite=Strict; Path=/",
            )
            .json(&json!({"principal": null})))
    }
}

// The public identifier alphabet applies to the original, unescaped path.
fn cancellation_id(path: &str) -> Option<&str> {
    path.strip_prefix("/api/reservations/")?
        .strip_suffix("/cancel")
        .filter(|id| {
            !id.is_empty()
                && id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        })
}
