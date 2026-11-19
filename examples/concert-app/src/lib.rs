//! File-backed reservations for the Local Sessions concert example.

mod reservations;
mod storage;

pub use reservations::{ACTIVE_LIMIT, Principal, Reservations};
use serde::Serialize;

/// The domain's public error codes. The HTTP adapter maps these to status codes.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum DomainError {
    Unauthenticated,
    InvalidInput,
    IdempotencyConflict,
    ConcertNotFound,
    ActiveReservationLimitReached,
    SoldOut,
    ReservationNotFound,
    PolicyMigrationRequired,
    PersistenceUnavailable,
}

impl DomainError {
    pub fn message(self) -> &'static str {
        match self {
            Self::Unauthenticated => "Sign in to continue.",
            Self::InvalidInput => "Provide a valid identifier and request key.",
            Self::IdempotencyConflict => "This request key belongs to another concert.",
            Self::ConcertNotFound => "Concert not found.",
            Self::ActiveReservationLimitReached => {
                "Your active reservation limit has been reached."
            }
            Self::SoldOut => "No seats are available.",
            Self::ReservationNotFound => "Reservation not found.",
            Self::PolicyMigrationRequired => {
                "Existing reservations require explicit reconciliation before this policy can be activated."
            }
            Self::PersistenceUnavailable => {
                "Reservations are temporarily unavailable. Please retry."
            }
        }
    }
}

impl std::fmt::Display for DomainError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.message())
    }
}

impl std::error::Error for DomainError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum State {
    Reserved,
    Cancelled,
}

/// Successful reserve, replay or cancellation response.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ReservationStatus {
    pub id: String,
    pub state: State,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Reservation {
    pub id: String,
    pub concert_id: String,
    pub state: State,
}

impl Reservation {
    fn status(self) -> ReservationStatus {
        ReservationStatus {
            id: self.id,
            state: self.state,
        }
    }
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct History {
    pub reservations: Vec<Reservation>,
    pub active_count: usize,
    pub active_limit: Option<i64>,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct Concert {
    pub id: String,
    pub title: String,
    pub date: String,
    pub venue: String,
    pub capacity: i64,
    pub available: i64,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct Catalog {
    pub concerts: Vec<Concert>,
}
