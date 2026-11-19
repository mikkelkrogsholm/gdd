//! Native form navigation. Only the session cookie grants access to the desk.

use gdd_concert::{DomainError, Principal, State};
use ntex::http::{HeaderMap, Request, Response, ResponseBuilder, StatusCode, header};
use serde::Deserialize;
use uuid::Uuid;

use super::{App, EXPIRED_COOKIE, Error, request::token, response};
use crate::{identity::Profile, page};

const POLICY: &str = "default-src 'none'; script-src 'none'; style-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'self'";
const CSS: &str = include_str!("../../concert_app/static/style.css");

fn html(status: StatusCode) -> ResponseBuilder {
    response(status, "text/html; charset=utf-8", POLICY)
}

fn redirect(path: &str) -> ResponseBuilder {
    let mut result = html(StatusCode::SEE_OTHER);
    result.header(header::LOCATION, path);
    result
}

#[derive(Deserialize)]
struct Form {
    username: Option<String>,
    password: Option<String>,
    view: Option<String>,
    concert_id: Option<String>,
    request_key: Option<String>,
    reservation_id: Option<String>,
}

fn desk_path(profile: Profile, page_id: Uuid) -> String {
    format!("/desk/{}/{page_id}", profile.view)
}

fn new_desk(profile: Profile, notice: &str) -> String {
    let mut path = desk_path(profile, Uuid::new_v4());
    if !notice.is_empty() {
        path.push('?');
        path.push_str(notice);
    }
    path
}

fn notice(query: Option<&str>) -> &'static str {
    match query {
        Some("signed-in") => "You're signed in.",
        Some("signed-out") => "You're signed out.",
        Some("reserved") => "Your seat is reserved.",
        Some("cancelled") => "Your reservation is cancelled.",
        _ => "",
    }
}

fn signin_required(status: StatusCode, message: &str) -> Response {
    html(status).body(page::document(None, "/program", "", message, page::SIGN_IN))
}

impl App {
    fn presented(&self, headers: &HeaderMap, view: Option<Uuid>) -> Result<Profile, Error> {
        let profile = self
            .sessions
            .profile(token(headers))
            .ok_or(Error::Unauthenticated)?;
        if view != Some(profile.view) {
            return Err(Error::SessionChanged);
        }
        Ok(profile)
    }

    pub(super) async fn browser_get(&self, req: &Request) -> Result<Response, Error> {
        let path = req.path();
        if path == "/style.css" {
            return Ok(response(StatusCode::OK, "text/css; charset=utf-8", POLICY).body(CSS));
        }
        let desk = match path {
            "/" => {
                if let Some(profile) = self.sessions.profile(token(req.headers())) {
                    return Ok(redirect(&new_desk(profile, "")).finish());
                }
                None
            }
            "/program" => None,
            _ => {
                let (view, page_id) = path
                    .strip_prefix("/desk/")
                    .and_then(|s| s.split_once('/'))
                    .ok_or(Error::NotFound)?;
                let view = Uuid::parse_str(view).map_err(|_| Error::NotFound)?;
                let page_id = Uuid::parse_str(page_id).map_err(|_| Error::NotFound)?;
                let profile = match self.presented(req.headers(), Some(view)) {
                    Ok(profile) => profile,
                    Err(error) => return Ok(identity_error(error)),
                };
                Some((profile, page_id))
            }
        };
        let profile = desk.map(|(profile, _)| profile);
        let home = if profile.is_some() { path } else { "/program" };
        let program = async {
            let catalog = self.reservations.catalog().await?;
            let desk_data = if let Some((profile, page_id)) = desk {
                let principal = Principal::from_session(Some(profile.id))?;
                let key_prefix = format!("{page_id}:");
                let (history, replayable) = self
                    .reservations
                    .list_own_with_replays(principal, Some(&key_prefix))
                    .await?;
                Some(page::Desk {
                    profile,
                    key_prefix,
                    history,
                    replayable,
                })
            } else {
                None
            };
            Ok::<_, DomainError>(page::program(home, &catalog, desk_data.as_ref()))
        }
        .await;
        // A sign-in in another tab may revoke this session during a database read.
        if let Some(profile) = profile
            && let Err(error) = self.presented(req.headers(), Some(profile.view))
        {
            return Ok(identity_error(error));
        }
        match program {
            Ok(content) => Ok(html(StatusCode::OK).body(page::document(
                profile,
                home,
                notice(req.uri().query()),
                "",
                &content,
            ))),
            Err(error) => Ok(
                html(super::error::domain_status(error)).body(page::document(
                    profile,
                    home,
                    "",
                    error.message(),
                    &page::back_to_program(home),
                )),
            ),
        }
    }

    pub(super) async fn browser_post(
        &self,
        req: &Request,
        bytes: &[u8],
    ) -> Result<Response, Error> {
        let Ok(form) = serde_urlencoded::from_bytes::<Form>(bytes) else {
            return Ok(signin_required(
                StatusCode::BAD_REQUEST,
                "This form could not be read. Open the program and try again.",
            ));
        };
        let headers = req.headers();
        if req.path() == "/forms/login" {
            return match self.sign_in(headers, form.username.as_deref(), form.password.as_deref()) {
                Ok((cookie, profile)) => Ok(redirect(&new_desk(profile, "signed-in"))
                    .header(header::SET_COOKIE, cookie)
                    .finish()),
                Err(Error::InvalidCredentials) => Ok(signin_required(
                    StatusCode::UNAUTHORIZED,
                    "The demo credentials did not match. Try again.",
                )),
                Err(_) => Ok(signin_required(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Sign-in is temporarily unavailable. Please try again.",
                )),
            };
        }
        let profile = match self.presented(
            headers,
            form.view.as_deref().and_then(|s| Uuid::parse_str(s).ok()),
        ) {
            Ok(profile) => profile,
            Err(Error::Unauthenticated) if req.path() == "/forms/logout" => {
                // A retry of a completed sign-out has nothing left to revoke.
                return Ok(redirect("/program?signed-out").finish());
            }
            Err(error) => return Ok(identity_error(error)),
        };
        let principal = Principal::from_session(Some(profile.id))?;
        let concert = form.concert_id.as_deref().unwrap_or_default();
        let key = form.request_key.as_deref().unwrap_or_default();
        let result = match req.path() {
            "/forms/logout" => {
                self.sessions.logout(token(headers));
                return Ok(redirect("/program?signed-out")
                    .header(header::SET_COOKIE, EXPIRED_COOKIE)
                    .finish());
            }
            "/forms/reserve" => self.reservations.reserve(principal, concert, key).await,
            "/forms/cancel" => {
                self.reservations
                    .cancel(
                        principal,
                        form.reservation_id.as_deref().unwrap_or_default(),
                    )
                    .await
            }
            _ => return Err(Error::NotFound),
        };
        if let Err(error) = self.presented(headers, Some(profile.view)) {
            return Ok(identity_error(error));
        }
        match result {
            Ok(reservation) => {
                let state = match reservation.state {
                    State::Reserved => "reserved",
                    State::Cancelled => "cancelled",
                };
                Ok(redirect(&new_desk(profile, state)).finish())
            }
            Err(error) => {
                // Returning to the program after an uncertain reservation
                // should offer the same request as retrying the form directly.
                let page_id = key
                    .split_once(':')
                    .and_then(|(page, _)| Uuid::parse_str(page).ok())
                    .unwrap_or_else(Uuid::new_v4);
                let home = desk_path(profile, page_id);
                let mut recovery = String::new();
                if req.path() == "/forms/reserve" && !key.is_empty() && !concert.is_empty() {
                    if error == DomainError::PersistenceUnavailable {
                        recovery.push_str(
                            "<p>Retry this request to check whether your seat was reserved.</p>",
                        );
                    }
                    recovery.push_str(&page::reserve_form(
                        profile,
                        concert,
                        key,
                        "Retry reservation",
                        false,
                    ));
                }
                recovery.push_str(&page::back_to_program(&home));
                Ok(
                    html(super::error::domain_status(error)).body(page::document(
                        Some(profile),
                        &home,
                        "",
                        error.message(),
                        &recovery,
                    )),
                )
            }
        }
    }
}

fn identity_error(error: Error) -> Response {
    let (status, message) = match error {
        Error::SessionChanged => (
            StatusCode::CONFLICT,
            "Your session changed in another tab. The other session was preserved. Sign in again to use this desk.",
        ),
        _ => (
            StatusCode::UNAUTHORIZED,
            "Your session has ended. Sign in again to use this desk.",
        ),
    };
    signin_required(status, message)
}
