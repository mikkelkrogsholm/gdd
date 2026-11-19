use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use gdd_concert::DomainError;
use serde::Serialize;
use serde_json::json;

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
    (
        status,
        Json(json!({"error": {"code": code, "message": message}})),
    )
        .into_response()
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        use StatusCode as S;
        let (status, code, message) = match self {
            Self::Domain(error) => {
                let status = match error {
                    DomainError::InvalidInput => S::BAD_REQUEST,
                    DomainError::Unauthenticated => S::UNAUTHORIZED,
                    DomainError::ConcertNotFound | DomainError::ReservationNotFound => S::NOT_FOUND,
                    DomainError::SoldOut
                    | DomainError::IdempotencyConflict
                    | DomainError::ActiveReservationLimitReached => S::CONFLICT,
                    DomainError::PersistenceUnavailable => S::SERVICE_UNAVAILABLE,
                    DomainError::PolicyMigrationRequired => S::INTERNAL_SERVER_ERROR,
                };
                return response(status, error, error.message());
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
