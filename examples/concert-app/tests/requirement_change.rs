//! The ten fixed phase-B requirements from the original two-phase concert delivery.

#[path = "support/reservations.rs"]
mod support;

use std::{env, fs, path::PathBuf};

use gdd_concert::{ACTIVE_LIMIT, DomainError as Error, ReservationStatus, Reservations, State};
use serde_json::{json, to_value};
use support::{files_and_bytes, race, signed_in};
use tempfile::{TempDir, tempdir};

fn path(dir: &TempDir) -> PathBuf {
    dir.path().join("requirements.sqlite")
}

async fn open(dir: &TempDir) -> Result<Reservations, Error> {
    match env::var("GDD_CONCERT_REQUIREMENT_PHASE").as_deref() {
        Ok("A") => Reservations::open_phase_a_for_replay(path(dir)).await,
        Ok("B") | Err(env::VarError::NotPresent) => Reservations::open(path(dir)).await,
        value => panic!("Expected GDD_CONCERT_REQUIREMENT_PHASE to be A or B, got {value:?}"),
    }
}

async fn fresh() -> (TempDir, Reservations) {
    let dir = tempdir().unwrap();
    let service = open(&dir).await.unwrap();
    (dir, service)
}

// Read persisted bindings and row counts after the service has released the file.
async fn count(dir: &TempDir, sql: &str) -> i64 {
    let db = turso::Builder::new_local(path(dir).to_str().unwrap())
        .read_only(true)
        .build()
        .await
        .unwrap();
    let connection = db.connect().unwrap();
    let mut statement = connection.prepare(sql).await.unwrap();
    statement.query_row(()).await.unwrap().get(0).unwrap()
}

#[tokio::test]
async fn global_limit_rejects_third_across_concerts_and_leaves_key_unbound() {
    let (dir, service) = fresh().await;
    let alice = signed_in("alice");
    service.reserve(alice, "midnight", "one").await.unwrap();
    service.reserve(alice, "northern", "two").await.unwrap();
    let third = service.reserve(alice, "matinee", "three").await;
    assert_eq!(
        third.map(|r| r.state),
        Err(Error::ActiveReservationLimitReached)
    );
    assert_eq!(service.list_own(alice).await.unwrap().active_count, 2);
    drop(service);
    assert_eq!(
        count(
            &dir,
            "SELECT count(*) FROM reservations
             WHERE principal_id='alice' AND request_key='three'"
        )
        .await,
        0
    );
    assert_eq!(
        count(
            &dir,
            "SELECT count(*) FROM reservations
             WHERE concert_id='matinee' AND state='reserved'"
        )
        .await,
        0
    );
}

#[tokio::test]
async fn principal_allowances_are_independent() {
    let (_dir, service) = fresh().await;
    let alice = signed_in("alice");
    let bob = signed_in("bob");
    service.reserve(alice, "midnight", "a1").await.unwrap();
    service.reserve(alice, "northern", "a2").await.unwrap();
    service.reserve(bob, "midnight", "b1").await.unwrap();
    service.reserve(bob, "northern", "b2").await.unwrap();
    assert_eq!(service.list_own(alice).await.unwrap().active_count, 2);
    assert_eq!(service.list_own(bob).await.unwrap().active_count, 2);
    let third = service.reserve(bob, "midnight", "b3").await;
    assert_eq!(
        third.map(|r| r.state),
        Err(Error::ActiveReservationLimitReached)
    );
}

#[tokio::test]
async fn cancellation_frees_allowance_and_failed_key_can_then_succeed() {
    let (_dir, service) = fresh().await;
    let alice = signed_in("alice");
    let first = service.reserve(alice, "midnight", "one").await.unwrap();
    service.reserve(alice, "northern", "two").await.unwrap();
    let attempt = service.reserve(alice, "matinee", "failed").await;
    assert_eq!(
        attempt.map(|r| r.state),
        Err(Error::ActiveReservationLimitReached)
    );
    service.cancel(alice, &first.id).await.unwrap();
    let created = service.reserve(alice, "matinee", "failed").await.unwrap();
    let history = service.list_own(alice).await.unwrap();
    assert_eq!(history.active_count, 2);
    assert_eq!(history.reservations.len(), 3);
    assert_eq!(history.active_limit, Some(2));
    assert_eq!(created.state, State::Reserved);
}

#[tokio::test]
async fn active_and_cancelled_successful_replay_precede_limit_and_capacity() {
    let (_dir, service) = fresh().await;
    let alice = signed_in("alice");
    let bob = signed_in("bob");
    let first = service.reserve(alice, "matinee", "one").await.unwrap();
    service.reserve(alice, "northern", "two").await.unwrap();
    service.reserve(bob, "matinee", "last-seat").await.unwrap();
    assert_eq!(
        service.reserve(bob, "matinee", "freed-seat").await,
        Err(Error::SoldOut)
    );
    assert_eq!(
        service.reserve(alice, "matinee", "one").await.unwrap(),
        first
    );
    let cancelled = service.cancel(alice, &first.id).await.unwrap();
    service.reserve(bob, "matinee", "freed-seat").await.unwrap();
    service.reserve(alice, "midnight", "three").await.unwrap();
    assert_eq!(
        service.reserve(alice, "matinee", "one").await.unwrap(),
        cancelled
    );
    assert_eq!(service.list_own(alice).await.unwrap().active_count, 2);
}

#[tokio::test]
async fn precedence_is_input_then_replay_then_existence_then_limit_then_capacity() {
    let (_dir, service) = fresh().await;
    let alice = signed_in("alice");
    let bob = signed_in("bob");
    service.reserve(alice, "midnight", "one").await.unwrap();
    service.reserve(alice, "northern", "two").await.unwrap();
    assert_eq!(
        service.reserve(alice, "midnight", "").await,
        Err(Error::InvalidInput)
    );
    assert_eq!(
        service.reserve(alice, "missing", "one").await,
        Err(Error::IdempotencyConflict)
    );
    assert_eq!(
        service.reserve(alice, "missing", "new").await,
        Err(Error::ConcertNotFound)
    );
    service.reserve(bob, "matinee", "first-seat").await.unwrap();
    service
        .reserve(signed_in("carol"), "matinee", "last-seat")
        .await
        .unwrap();
    assert_eq!(
        service.reserve(alice, "matinee", "new").await,
        Err(Error::ActiveReservationLimitReached)
    );
    assert_eq!(
        service.reserve(bob, "matinee", "new").await,
        Err(Error::SoldOut)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn concurrent_new_keys_across_concerts_share_one_remaining_allowance() {
    let (dir, service) = fresh().await;
    let alice = signed_in("alice");
    service
        .reserve(alice, "midnight", "existing")
        .await
        .unwrap();
    let outcomes = race(
        &service,
        &[
            ("alice", "northern", "competing-one"),
            ("alice", "matinee", "competing-two"),
        ],
    )
    .await;
    let reserved = outcomes
        .iter()
        .filter(|outcome| outcome.as_ref().is_ok_and(|r| r.state == State::Reserved))
        .count();
    assert_eq!(reserved, 1);
    assert_eq!(
        outcomes
            .iter()
            .filter(|outcome| **outcome == Err(Error::ActiveReservationLimitReached))
            .count(),
        1
    );
    assert_eq!(service.list_own(alice).await.unwrap().active_count, 2);
    drop(service);
    let bound = count(
        &dir,
        "SELECT count(*) FROM reservations
         WHERE principal_id='alice' AND request_key IN ('competing-one','competing-two')",
    )
    .await;
    assert_eq!(bound, 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_replay_remains_one_reservation_at_limit() {
    let (_dir, service) = fresh().await;
    let alice = signed_in("alice");
    let first = service
        .reserve(alice, "midnight", "existing")
        .await
        .unwrap();
    service.reserve(alice, "northern", "second").await.unwrap();
    let outcomes = race(&service, &[("alice", "midnight", "existing"); 8]).await;
    assert_eq!(outcomes, vec![Ok(first); 8]);
    assert_eq!(service.list_own(alice).await.unwrap().active_count, 2);
}

#[tokio::test]
async fn server_exposes_single_current_policy_value() {
    let (_dir, service) = fresh().await;
    let history = service.list_own(signed_in("alice")).await.unwrap();
    assert_eq!(ACTIVE_LIMIT, 2);
    assert_eq!(
        to_value(history).unwrap().get("active_limit"),
        Some(&json!(ACTIVE_LIMIT))
    );
}

#[tokio::test]
async fn incompatible_existing_a_data_is_rejected_without_history_mutation() {
    let dir = tempdir().unwrap();
    fs::write(path(&dir), include_bytes!("fixtures/over-limit.db")).unwrap();
    let before = files_and_bytes(dir.path());
    assert_eq!(
        open(&dir).await.map(|_| ()),
        Err(Error::PolicyMigrationRequired)
    );
    assert_eq!(files_and_bytes(dir.path()), before);
    assert_eq!(
        count(
            &dir,
            "SELECT count(*) FROM reservations
             WHERE principal_id='alice' AND state='reserved'"
        )
        .await,
        3
    );
}

#[tokio::test]
async fn compatible_a_history_and_cancelled_keys_survive_activation() {
    let dir = tempdir().unwrap();
    fs::write(path(&dir), include_bytes!("fixtures/recorded-phase-a.db")).unwrap();
    let service = open(&dir).await.unwrap();
    let alice = signed_in("alice");
    let replayed = service
        .reserve(alice, "midnight", "73ce535d-f1dd-4556-a2ea-2813952e80f6")
        .await
        .unwrap();
    assert_eq!(
        replayed,
        ReservationStatus {
            id: "22295aa924aa4f62bcf528cfc046cad7".into(),
            state: State::Cancelled,
        }
    );
    let history = service.list_own(alice).await.unwrap();
    assert_eq!(history.active_count, 2);
    assert_eq!(history.reservations.len(), 3);
    assert_eq!(history.active_limit, Some(2));
}
