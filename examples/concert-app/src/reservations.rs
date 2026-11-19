use std::path::Path;

use crate::storage::Store;
use crate::{Catalog, DomainError, History, ReservationStatus, State};

pub const ACTIVE_LIMIT: i64 = 2;

// Python's str.strip includes the four ASCII information separators.
fn is_blank(value: &str) -> bool {
    value
        .chars()
        .all(|c| c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c))
}

/// A principal resolved by the server's session store.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Principal<'a>(&'a str);

impl<'a> Principal<'a> {
    /// Pass the result of session lookup. An absent or blank ID requires sign-in.
    pub fn from_session(id: Option<&'a str>) -> Result<Self, DomainError> {
        id.filter(|value| !is_blank(value))
            .map(Self)
            .ok_or(DomainError::Unauthenticated)
    }
}

/// Clone the opened service to share the database among concurrent requests.
#[derive(Clone, Debug)]
pub struct Reservations {
    store: Store,
    active_limit: Option<i64>,
}

impl Reservations {
    /// Initialize missing tables and concerts and activate the current policy.
    pub async fn open(path: impl AsRef<Path>) -> Result<Self, DomainError> {
        Self::open_with_limit(path.as_ref(), Some(ACTIVE_LIMIT)).await
    }

    /// Open the same schema with the unlimited allowance for controlled phase-A replay.
    pub async fn open_phase_a_for_replay(path: impl AsRef<Path>) -> Result<Self, DomainError> {
        Self::open_with_limit(path.as_ref(), None).await
    }

    async fn open_with_limit(path: &Path, active_limit: Option<i64>) -> Result<Self, DomainError> {
        if let Some(limit) = active_limit
            && Store::existing_max_active(path).await? > limit
        {
            return Err(DomainError::PolicyMigrationRequired);
        }
        let store = Store::open(path).await?;
        store
            .write(async |tx| {
                // Recheck after acquiring the writer and before creating or seeding tables.
                if let Some(limit) = active_limit
                    && tx.max_active().await? > limit
                {
                    return Err(DomainError::PolicyMigrationRequired);
                }
                tx.initialize().await?;
                Ok(())
            })
            .await?;
        Ok(Self {
            store,
            active_limit,
        })
    }

    /// Flush committed WAL pages into the main database. For a single-file handoff,
    /// stop admitting requests, drain them, await this operation, then drop all handles.
    pub async fn checkpoint(&self) -> Result<(), DomainError> {
        self.store.checkpoint().await
    }

    pub async fn catalog(&self) -> Result<Catalog, DomainError> {
        self.store
            .read(async |tx| {
                Ok(Catalog {
                    concerts: tx.concerts().await?,
                })
            })
            .await
    }

    pub async fn list_own(&self, principal: Principal<'_>) -> Result<History, DomainError> {
        self.store
            .read(async |tx| {
                let reservations = tx.history(principal.0).await?;
                let active_count = reservations
                    .iter()
                    .filter(|r| r.state == State::Reserved)
                    .count();
                Ok(History {
                    reservations,
                    active_count,
                    active_limit: self.active_limit,
                })
            })
            .await
    }

    pub async fn reserve(
        &self,
        principal: Principal<'_>,
        concert_id: &str,
        request_key: &str,
    ) -> Result<ReservationStatus, DomainError> {
        if is_blank(concert_id) || is_blank(request_key) {
            return Err(DomainError::InvalidInput);
        }
        self.store
            .write(async |tx| {
                if let Some(previous) = tx.find_by_key(principal.0, request_key).await? {
                    if previous.concert_id != concert_id {
                        return Err(DomainError::IdempotencyConflict);
                    }
                    return Ok(previous.status());
                }
                let capacity = tx
                    .capacity(concert_id)
                    .await?
                    .ok_or(DomainError::ConcertNotFound)?;
                if let Some(limit) = self.active_limit
                    && tx.active_for_principal(principal.0).await? >= limit
                {
                    return Err(DomainError::ActiveReservationLimitReached);
                }
                if tx.active_for_concert(concert_id).await? >= capacity {
                    return Err(DomainError::SoldOut);
                }
                Ok(tx.insert(principal.0, concert_id, request_key).await?)
            })
            .await
    }

    pub async fn cancel(
        &self,
        principal: Principal<'_>,
        reservation_id: &str,
    ) -> Result<ReservationStatus, DomainError> {
        if is_blank(reservation_id) {
            return Err(DomainError::InvalidInput);
        }
        self.store
            .write(async |tx| {
                let reservation = tx
                    .owned(principal.0, reservation_id)
                    .await?
                    .ok_or(DomainError::ReservationNotFound)?;
                if reservation.state == State::Cancelled {
                    return Ok(reservation.status());
                }
                Ok(tx.cancel(principal.0, reservation_id).await?)
            })
            .await
    }
}
