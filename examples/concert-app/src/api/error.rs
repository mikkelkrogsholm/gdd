use gdd_concert::DomainError;
use std::fmt;

use ntex::http::{ConnectionType, Response, ResponseError, StatusCode};
use serde::Serialize;
use serde_json::json;

#[derive(Debug)]
pub enum Error {
    Domain(DomainError),
    InvalidInput(&'static str),
    Forbidden(&'static str),
    Unauthenticated,
    SessionChanged,
    InvalidCredentials,
    NotFound,
    Internal,
}

impl From<DomainError> for Error {
    fn from(error: DomainError) -> Self {
        Self::Domain(error)
    }
}

fn response(status: StatusCode, code: impl Serialize, message: &str) -> Response {
    super::json_response(status).json(&json!({"error": {"code": code, "message": message}}))
}

pub(super) fn domain_status(error: DomainError) -> StatusCode {
    use StatusCode as S;
    match error {
        DomainError::InvalidInput => S::BAD_REQUEST,
        DomainError::Unauthenticated => S::UNAUTHORIZED,
        DomainError::ConcertNotFound | DomainError::ReservationNotFound => S::NOT_FOUND,
        DomainError::SoldOut
        | DomainError::IdempotencyConflict
        | DomainError::ActiveReservationLimitReached => S::CONFLICT,
        DomainError::PersistenceUnavailable => S::SERVICE_UNAVAILABLE,
        DomainError::PolicyMigrationRequired => S::INTERNAL_SERVER_ERROR,
    }
}

impl Error {
    pub fn invalid_framing() -> Self {
        Self::InvalidInput("Send a JSON object with a content length.")
    }

    pub fn response(&self) -> Response {
        use StatusCode as S;
        let (status, code, message) = match *self {
            Self::Domain(error) => {
                return response(domain_status(error), error, error.message());
            }
            Self::InvalidInput(message) => (S::BAD_REQUEST, "InvalidInput", message),
            Self::Forbidden(message) => (S::FORBIDDEN, "Forbidden", message),
            Self::Unauthenticated => (
                S::UNAUTHORIZED,
                "Unauthenticated",
                "Sign in to manage your reservations.",
            ),
            Self::SessionChanged => (
                S::CONFLICT,
                "SessionChanged",
                "The session changed in another tab. Sign in again.",
            ),
            Self::InvalidCredentials => (
                S::UNAUTHORIZED,
                "InvalidCredentials",
                "The demo credentials did not match.",
            ),
            Self::NotFound => (S::NOT_FOUND, "NotFound", "This route does not exist."),
            Self::Internal => (
                S::INTERNAL_SERVER_ERROR,
                "InternalError",
                "The request could not be completed.",
            ),
        };
        response(status, code, message)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for Error {}

impl ResponseError for Error {
    fn error_response(&self) -> Response {
        let mut response = self.response();
        response
            .head_mut()
            .set_connection_type(ConnectionType::Close);
        response
    }
}
