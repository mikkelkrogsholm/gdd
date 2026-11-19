use std::{fs, io::ErrorKind, path::Path, time::Duration};

use turso::transaction::TransactionBehavior;
use turso::{Builder, Connection, Database, Row};
use uuid::Uuid;

use crate::{Concert, DomainError, Reservation, ReservationStatus, State};

impl From<turso::Error> for DomainError {
    fn from(_: turso::Error) -> Self {
        Self::PersistenceUnavailable
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Store(Database);

/// Queries share the connection and snapshot of their enclosing transaction.
pub(crate) struct Queries<'a>(&'a Connection);

impl Store {
    async fn open_file(path: &Path, read_only: bool) -> Result<Self, DomainError> {
        // Use a filesystem path even when a relative filename resembles a database URI.
        let path = std::path::absolute(path).map_err(|_| DomainError::PersistenceUnavailable)?;
        let path = path.to_str().ok_or(DomainError::PersistenceUnavailable)?;
        Ok(Self(
            Builder::new_local(path)
                .read_only(read_only)
                .build()
                .await?,
        ))
    }

    pub(crate) async fn open(path: &Path) -> Result<Self, DomainError> {
        Self::open_file(path, false).await
    }

    pub(crate) async fn checkpoint(&self) -> Result<(), DomainError> {
        let connection = self.0.connect()?;
        let result = connection
            .prepare("PRAGMA wal_checkpoint(TRUNCATE)")
            .await?
            .query_row(())
            .await?;
        let busy = result.get::<i64>(0)?;
        if busy != 0 || result.get::<i64>(1)? != result.get::<i64>(2)? {
            return Err(DomainError::PersistenceUnavailable);
        }
        Ok(())
    }

    pub(crate) async fn existing_max_active(path: &Path) -> Result<i64, DomainError> {
        match fs::metadata(path) {
            Ok(metadata) if metadata.len() == 0 => return Ok(0),
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(0),
            Err(_) => return Err(DomainError::PersistenceUnavailable),
            Ok(_) => {}
        }
        Self::open_file(path, true)
            .await?
            .read(async |tx| Ok(tx.max_active().await?))
            .await
    }

    pub(crate) async fn read<T>(
        &self,
        work: impl AsyncFnOnce(Queries<'_>) -> Result<T, DomainError>,
    ) -> Result<T, DomainError> {
        self.transaction(TransactionBehavior::Deferred, work).await
    }

    pub(crate) async fn write<T>(
        &self,
        work: impl AsyncFnOnce(Queries<'_>) -> Result<T, DomainError>,
    ) -> Result<T, DomainError> {
        self.transaction(TransactionBehavior::Immediate, work).await
    }

    async fn transaction<T>(
        &self,
        behavior: TransactionBehavior,
        work: impl AsyncFnOnce(Queries<'_>) -> Result<T, DomainError>,
    ) -> Result<T, DomainError> {
        let mut connection = self.0.connect()?;
        connection.busy_timeout(Duration::from_secs(5))?;
        connection.execute("PRAGMA foreign_keys = ON", ()).await?;
        let tx = connection.transaction_with_behavior(behavior).await?;
        match work(Queries(&tx)).await {
            Ok(value) => {
                tx.commit().await?;
                Ok(value)
            }
            Err(error) => {
                tx.rollback().await?;
                Err(error)
            }
        }
    }
}

impl Queries<'_> {
    pub(crate) async fn initialize(&self) -> turso::Result<()> {
        self.0
            .execute(
                "CREATE TABLE IF NOT EXISTS concerts (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                date TEXT NOT NULL,
                venue TEXT NOT NULL,
                capacity INTEGER NOT NULL CHECK (capacity >= 0)
            )",
                (),
            )
            .await?;
        self.0
            .execute(
                "CREATE TABLE IF NOT EXISTS reservations (
                id TEXT PRIMARY KEY,
                concert_id TEXT NOT NULL REFERENCES concerts(id),
                principal_id TEXT NOT NULL,
                request_key TEXT NOT NULL,
                state TEXT NOT NULL CHECK (state IN ('reserved', 'cancelled')),
                UNIQUE (principal_id, request_key)
            )",
                (),
            )
            .await?;
        self.0
            .execute(
                "INSERT OR IGNORE INTO concerts (id, title, date, venue, capacity) VALUES
                ('midnight', 'Midnight Sessions', '2026-11-14', 'Harbor Hall', 8),
                ('northern', 'Northern Lights', '2026-11-21', 'North Stage', 3),
                ('matinee', 'Sunday Matinee', '2026-11-29', 'Studio 03', 2)",
                (),
            )
            .await?;
        Ok(())
    }

    pub(crate) async fn max_active(&self) -> turso::Result<i64> {
        let mut tables = self
            .0
            .query(
                "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'reservations'",
                (),
            )
            .await?;
        if tables.next().await?.is_none() {
            return Ok(0);
        }
        drop(tables);
        let mut counts = self
            .0
            .query(
                "SELECT COUNT(*) FROM reservations WHERE state = 'reserved'
             GROUP BY principal_id ORDER BY COUNT(*) DESC LIMIT 1",
                (),
            )
            .await?;
        counts.next().await?.map_or(Ok(0), |r| r.get(0))
    }

    pub(crate) async fn concerts(&self) -> turso::Result<Vec<Concert>> {
        let mut rows = self
            .0
            .query(
                "SELECT c.id, c.title, c.date, c.venue, c.capacity,
                    c.capacity - (SELECT COUNT(*) FROM reservations r
                                  WHERE r.concert_id = c.id AND r.state = 'reserved')
             FROM concerts c ORDER BY c.date, c.id",
                (),
            )
            .await?;
        let mut concerts = Vec::new();
        while let Some(row) = rows.next().await? {
            concerts.push(Concert {
                id: row.get(0)?,
                title: row.get(1)?,
                date: row.get(2)?,
                venue: row.get(3)?,
                capacity: row.get(4)?,
                available: row.get(5)?,
            });
        }
        Ok(concerts)
    }

    pub(crate) async fn capacity(&self, concert_id: &str) -> turso::Result<Option<i64>> {
        self.0
            .query("SELECT capacity FROM concerts WHERE id = ?", [concert_id])
            .await?
            .next()
            .await?
            .map(|r| r.get(0))
            .transpose()
    }

    pub(crate) async fn active_for_principal(&self, principal_id: &str) -> turso::Result<i64> {
        self.count(
            "SELECT COUNT(*) FROM reservations WHERE state = 'reserved' AND principal_id = ?",
            principal_id,
        )
        .await
    }

    pub(crate) async fn active_for_concert(&self, concert_id: &str) -> turso::Result<i64> {
        self.count(
            "SELECT COUNT(*) FROM reservations WHERE state = 'reserved' AND concert_id = ?",
            concert_id,
        )
        .await
    }

    async fn count(&self, sql: &str, id: &str) -> turso::Result<i64> {
        self.0.prepare(sql).await?.query_row([id]).await?.get(0)
    }

    pub(crate) async fn find_by_key(
        &self,
        principal_id: &str,
        key: &str,
    ) -> turso::Result<Option<Reservation>> {
        self.0.query(
            "SELECT id, concert_id, state FROM reservations WHERE principal_id = ? AND request_key = ?",
            [principal_id, key],
        ).await?.next().await?.map(reservation).transpose()
    }

    pub(crate) async fn owned(
        &self,
        principal_id: &str,
        id: &str,
    ) -> turso::Result<Option<Reservation>> {
        self.0
            .query(
                "SELECT id, concert_id, state FROM reservations WHERE principal_id = ? AND id = ?",
                [principal_id, id],
            )
            .await?
            .next()
            .await?
            .map(reservation)
            .transpose()
    }

    pub(crate) async fn history(&self, principal_id: &str) -> turso::Result<Vec<Reservation>> {
        let mut rows = self.0.query(
            "SELECT id, concert_id, state FROM reservations WHERE principal_id = ? ORDER BY rowid DESC",
            [principal_id],
        ).await?;
        let mut history = Vec::new();
        while let Some(row) = rows.next().await? {
            history.push(reservation(row)?);
        }
        Ok(history)
    }

    pub(crate) async fn insert(
        &self,
        principal_id: &str,
        concert_id: &str,
        key: &str,
    ) -> turso::Result<ReservationStatus> {
        let id = Uuid::new_v4().simple().to_string();
        self.0
            .execute(
                "INSERT INTO reservations (id, concert_id, principal_id, request_key, state)
             VALUES (?, ?, ?, ?, 'reserved')",
                [id.as_str(), concert_id, principal_id, key],
            )
            .await?;
        self.status(principal_id, &id).await
    }

    pub(crate) async fn cancel(
        &self,
        principal_id: &str,
        id: &str,
    ) -> turso::Result<ReservationStatus> {
        self.0.execute(
            "UPDATE reservations SET state = 'cancelled' WHERE principal_id = ? AND id = ? AND state = 'reserved'",
            [principal_id, id],
        ).await?;
        self.status(principal_id, id).await
    }

    async fn status(&self, principal_id: &str, id: &str) -> turso::Result<ReservationStatus> {
        self.owned(principal_id, id)
            .await?
            .map(Reservation::status)
            .ok_or(turso::Error::QueryReturnedNoRows)
    }
}

fn reservation(row: Row) -> turso::Result<Reservation> {
    let state = match row.get::<String>(2)?.as_str() {
        "reserved" => State::Reserved,
        "cancelled" => State::Cancelled,
        _ => {
            return Err(turso::Error::ConversionFailure(
                "Unknown reservation state".into(),
            ));
        }
    };
    Ok(Reservation {
        id: row.get(0)?,
        concert_id: row.get(1)?,
        state,
    })
}

#[cfg(test)]
mod tests;
