#[path = "support/reservations.rs"]
mod support;

use std::{fs, sync::Arc};

use gdd_concert::{DomainError as Error, Principal, ReservationStatus, Reservations, State};
use serde_json::{json, to_value};
use support::{files_and_bytes, race, signed_in};
use tempfile::tempdir;
use tokio::{sync::Barrier, task::JoinSet};

#[tokio::test]
async fn catalog_and_private_responses_preserve_the_wire_contract() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("local sessions 🎵.sqlite");
    let service = Reservations::open(&path).await.unwrap();
    let catalog = service.catalog().await.unwrap();
    assert_eq!(
        to_value(catalog).unwrap(),
        json!({"concerts":[
            {"id":"midnight","title":"Midnight Sessions","date":"2026-11-14","venue":"Harbor Hall","capacity":8,"available":8},
            {"id":"northern","title":"Northern Lights","date":"2026-11-21","venue":"North Stage","capacity":3,"available":3},
            {"id":"matinee","title":"Sunday Matinee","date":"2026-11-29","venue":"Studio 03","capacity":2,"available":2}
        ]})
    );
    let alice = signed_in("alice");
    let bob = signed_in("bob");
    let first = service.reserve(alice, "midnight", "same").await.unwrap();
    let second = service.reserve(alice, "northern", "later").await.unwrap();
    let other = service.reserve(bob, "midnight", "same").await.unwrap();
    assert_ne!(first.id, other.id);
    let uuid = uuid::Uuid::parse_str(&first.id).unwrap();
    assert_eq!(uuid.get_version_num(), 4);
    assert_eq!(uuid.simple().to_string(), first.id);
    assert_eq!(
        to_value(&first).unwrap(),
        json!({"id":first.id, "state":"reserved"})
    );

    let missing = service.cancel(bob, "missing").await.unwrap_err();
    assert_eq!(missing, Error::ReservationNotFound);
    assert_eq!(service.cancel(bob, &first.id).await.unwrap_err(), missing);
    let cancelled = service.cancel(alice, &first.id).await.unwrap();
    assert_eq!(cancelled.state, State::Cancelled);
    assert_eq!(service.cancel(bob, &first.id).await.unwrap_err(), missing);
    assert_eq!(service.cancel(alice, &first.id).await.unwrap(), cancelled);
    assert_eq!(
        service.reserve(alice, "midnight", "same").await.unwrap(),
        cancelled
    );
    assert_eq!(
        to_value(service.list_own(alice).await.unwrap()).unwrap(),
        json!({
            "reservations":[{"id":second.id,"concert_id":"northern","state":"reserved"},
                            {"id":first.id,"concert_id":"midnight","state":"cancelled"}],
            "active_count":1, "active_limit":2
        })
    );
    assert_eq!(
        service.list_own(bob).await.unwrap().reservations[0].id,
        other.id
    );
    assert_eq!(service.catalog().await.unwrap().concerts[0].available, 7);
    drop(service);
    let reopened = Reservations::open(path).await.unwrap();
    assert_eq!(
        reopened.reserve(alice, "midnight", "same").await.unwrap(),
        cancelled
    );
    assert_eq!(
        reopened.reserve(alice, "northern", "later").await.unwrap(),
        second
    );
}

#[tokio::test]
async fn validate_blank_inputs_but_keep_the_exact_identifier_and_key() {
    for id in [None, Some(""), Some(" \t\n"), Some("\u{a0}\u{1c}")] {
        assert_eq!(Principal::from_session(id), Err(Error::Unauthenticated));
    }
    let dir = tempdir().unwrap();
    let service = Reservations::open(dir.path().join("sessions"))
        .await
        .unwrap();
    let alice = signed_in("alice");
    assert_eq!(
        service.reserve(alice, " ", "x").await,
        Err(Error::InvalidInput)
    );
    assert_eq!(
        service.reserve(alice, "missing", "\u{1f}").await,
        Err(Error::InvalidInput)
    );
    assert_eq!(service.cancel(alice, "\t").await, Err(Error::InvalidInput));
    assert_eq!(
        service.reserve(alice, " midnight", "x").await,
        Err(Error::ConcertNotFound)
    );
    let first = service.reserve(alice, "midnight", "key").await.unwrap();
    let second = service.reserve(alice, "midnight", " key ").await.unwrap();
    assert_ne!(first.id, second.id);
    let bob = signed_in("bob");
    let plain = service.reserve(bob, "midnight", "clé").await.unwrap();
    let embedded_nul = service
        .reserve(bob, "midnight", "clé\0suffix")
        .await
        .unwrap();
    assert_ne!(plain.id, embedded_nul.id);
    assert_eq!(
        service
            .reserve(bob, "midnight", "clé\0suffix")
            .await
            .unwrap(),
        embedded_nul
    );
    assert!(
        service
            .list_own(signed_in("alice "))
            .await
            .unwrap()
            .reservations
            .is_empty()
    );
}

#[tokio::test]
async fn replay_existence_global_allowance_and_capacity_have_stable_precedence() {
    let dir = tempdir().unwrap();
    let service = Reservations::open(dir.path().join("sessions"))
        .await
        .unwrap();
    let alice = signed_in("alice");
    let bob = signed_in("bob");
    let first = service.reserve(alice, "matinee", "one").await.unwrap();
    service.reserve(alice, "northern", "two").await.unwrap();
    service.reserve(bob, "matinee", "last").await.unwrap();
    assert_eq!(
        service.reserve(alice, "matinee", "one").await.unwrap(),
        first
    );
    assert_eq!(
        service.reserve(alice, " ", "one").await,
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
    assert_eq!(
        service.reserve(alice, "matinee", "retry").await,
        Err(Error::ActiveReservationLimitReached)
    );
    assert_eq!(
        service.reserve(bob, "matinee", "retry").await,
        Err(Error::SoldOut)
    );

    let cancelled = service.cancel(alice, &first.id).await.unwrap();
    service.reserve(bob, "matinee", "retry").await.unwrap();
    service.reserve(alice, "midnight", "retry").await.unwrap();
    assert_eq!(
        service.reserve(alice, "matinee", "one").await.unwrap(),
        cancelled
    );
    assert_eq!(
        service.reserve(alice, "missing", "one").await,
        Err(Error::IdempotencyConflict)
    );
    assert_eq!(service.list_own(alice).await.unwrap().active_count, 2);
    assert_eq!(service.list_own(bob).await.unwrap().active_count, 2);
}

#[tokio::test]
async fn opens_the_recorded_phase_a_schema_and_preserves_existing_keys_and_ids() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("prior.sqlite");
    fs::write(&path, include_bytes!("fixtures/recorded-phase-a.db")).unwrap();
    let service = Reservations::open(&path).await.unwrap();
    let alice = signed_in("alice");
    let history = service.list_own(alice).await.unwrap();
    assert_eq!(history.active_count, 2);
    assert_eq!(history.reservations.len(), 3);
    assert_eq!(
        history.reservations[0].id,
        "6122fbd2484a4c55a5c84026a1df2f24"
    );
    let cancelled = service
        .reserve(alice, "midnight", "73ce535d-f1dd-4556-a2ea-2813952e80f6")
        .await
        .unwrap();
    assert_eq!(cancelled.id, "22295aa924aa4f62bcf528cfc046cad7");
    assert_eq!(cancelled.state, State::Cancelled);
    let active = service
        .reserve(alice, "northern", "9707c2a7-aa9f-4bcd-a05c-6d80f579172e")
        .await
        .unwrap();
    assert_eq!(active.id, "2d3f2bc9fed44168b9a47eb2e163061e");
    assert_eq!(active.state, State::Reserved);
    assert_eq!(
        service.reserve(alice, "midnight", "new").await,
        Err(Error::ActiveReservationLimitReached)
    );
    service.cancel(alice, &active.id).await.unwrap();
    let new = service.reserve(alice, "midnight", "new").await.unwrap();
    drop(service);
    let reopened = Reservations::open(path).await.unwrap();
    assert_eq!(
        reopened.reserve(alice, "midnight", "new").await.unwrap(),
        new
    );
    assert_eq!(
        reopened.list_own(alice).await.unwrap().reservations.len(),
        4
    );
}

#[tokio::test]
async fn rejects_incompatible_python_database_without_changing_bytes_or_leaving_sidecars() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("prior.sqlite");
    fs::write(&path, include_bytes!("fixtures/over-limit.db")).unwrap();
    let before = files_and_bytes(dir.path());
    assert_eq!(
        Reservations::open(&path).await.unwrap_err(),
        Error::PolicyMigrationRequired
    );
    assert_eq!(files_and_bytes(dir.path()), before);
}

#[tokio::test]
async fn phase_a_to_b_requires_explicit_reconciliation() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("replay.sqlite");
    let service = Reservations::open_phase_a_for_replay(&path).await.unwrap();
    let alice = signed_in("alice");
    let first = service.reserve(alice, "midnight", "one").await.unwrap();
    service.reserve(alice, "northern", "two").await.unwrap();
    service.reserve(alice, "matinee", "three").await.unwrap();
    let history = service.list_own(alice).await.unwrap();
    assert_eq!(history.active_count, 3);
    assert_eq!(
        to_value(history).unwrap()["active_limit"],
        serde_json::Value::Null
    );
    drop(service);
    let before = files_and_bytes(dir.path());
    assert_eq!(
        Reservations::open(&path).await.unwrap_err(),
        Error::PolicyMigrationRequired
    );
    assert_eq!(files_and_bytes(dir.path()), before);
    let service = Reservations::open_phase_a_for_replay(&path).await.unwrap();
    let cancelled = service.cancel(alice, &first.id).await.unwrap();
    drop(service);
    let service = Reservations::open(&path).await.unwrap();
    let history = service.list_own(alice).await.unwrap();
    assert_eq!(history.active_limit, Some(2));
    assert_eq!(history.active_count, 2);
    assert_eq!(
        service.reserve(alice, "midnight", "one").await.unwrap(),
        cancelled
    );
}

#[tokio::test]
async fn corrupt_files_and_missing_parents_get_public_persistence_errors() {
    let dir = tempdir().unwrap();
    let corrupt = dir.path().join("invalid.sqlite");
    fs::write(&corrupt, vec![b'x'; 4096]).unwrap();
    for path in [corrupt, dir.path().join("missing").join("db")] {
        let error = Reservations::open(path).await.unwrap_err();
        assert_eq!(error, Error::PersistenceUnavailable);
        assert_eq!(
            error.message(),
            "Reservations are temporarily unavailable. Please retry."
        );
        assert_eq!(to_value(error).unwrap(), json!("PersistenceUnavailable"));
    }
}

#[cfg(unix)]
#[tokio::test]
async fn unrepresentable_filename_cannot_be_redirected_to_a_lossy_path() {
    use std::{ffi::OsStr, os::unix::ffi::OsStrExt};
    let dir = tempdir().unwrap();
    let path = dir.path().join(OsStr::from_bytes(b"db\xff"));
    assert_eq!(
        Reservations::open(path).await.unwrap_err(),
        Error::PersistenceUnavailable
    );
    assert!(files_and_bytes(dir.path()).is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn parallel_same_key_replays_or_conflicts_without_double_booking() {
    let dir = tempdir().unwrap();
    let service = Reservations::open(dir.path().join("sessions"))
        .await
        .unwrap();
    let results = race(&service, &[("alice", "midnight", "key"); 8]).await;
    let first = results[0].as_ref().unwrap();
    assert!(results.iter().all(|r| r.as_ref() == Ok(first)));
    assert_eq!(
        service
            .list_own(signed_in("alice"))
            .await
            .unwrap()
            .active_count,
        1
    );
    service
        .reserve(signed_in("alice"), "northern", "second")
        .await
        .unwrap();
    let at_limit = race(&service, &[("alice", "midnight", "key"); 8]).await;
    assert!(at_limit.iter().all(|r| r.as_ref() == Ok(first)));
    let results = race(
        &service,
        &[("bob", "midnight", "key"), ("bob", "northern", "key")],
    )
    .await;
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    assert!(results.contains(&Err(Error::IdempotencyConflict)));
    assert_eq!(
        service
            .list_own(signed_in("bob"))
            .await
            .unwrap()
            .active_count,
        1
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn parallel_different_keys_compete_for_one_seat_and_one_global_allowance() {
    let dir = tempdir().unwrap();
    let service = Reservations::open(dir.path().join("sessions"))
        .await
        .unwrap();
    service
        .reserve(signed_in("alice"), "matinee", "first")
        .await
        .unwrap();
    let results = race(
        &service,
        &[
            ("bob", "matinee", "one"),
            ("carol", "matinee", "two"),
            ("dan", "matinee", "three"),
            ("eve", "matinee", "four"),
        ],
    )
    .await;
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|r| matches!(r, Err(Error::SoldOut)))
            .count(),
        3
    );
    assert_eq!(service.catalog().await.unwrap().concerts[2].available, 0);
    let results = race(
        &service,
        &[("alice", "midnight", "x"), ("alice", "northern", "y")],
    )
    .await;
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    assert!(results.contains(&Err(Error::ActiveReservationLimitReached)));
    assert_eq!(
        service
            .list_own(signed_in("alice"))
            .await
            .unwrap()
            .active_count,
        2
    );
}

#[tokio::test]
async fn graceful_checkpoint_makes_the_main_file_portable_with_its_history() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("concert.sqlite");
    let alice = signed_in("alice");
    let service = Reservations::open(&path).await.unwrap();
    let active = service.reserve(alice, "midnight", "active").await.unwrap();
    let old = service.reserve(alice, "northern", "old").await.unwrap();
    let cancelled = service.cancel(alice, &old.id).await.unwrap();
    service.checkpoint().await.unwrap();
    drop(service);

    let destination = tempdir().unwrap();
    let copy = destination.path().join("copied.sqlite");
    fs::copy(path, &copy).unwrap();
    let reopened = Reservations::open(copy).await.unwrap();
    assert_eq!(
        reopened.list_own(alice).await.unwrap().reservations.len(),
        2
    );
    assert_eq!(
        reopened.reserve(alice, "midnight", "active").await.unwrap(),
        active
    );
    assert_eq!(
        reopened.reserve(alice, "northern", "old").await.unwrap(),
        cancelled
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn parallel_cancellation_releases_one_seat_and_keeps_the_key_bound() {
    let dir = tempdir().unwrap();
    let service = Reservations::open(dir.path().join("sessions"))
        .await
        .unwrap();
    let alice = signed_in("alice");
    let created = service.reserve(alice, "matinee", "key").await.unwrap();
    let start = Arc::new(Barrier::new(4));
    let mut tasks = JoinSet::new();
    for _ in 0..4 {
        let service = service.clone();
        let id = created.id.clone();
        let start = start.clone();
        tasks.spawn(async move {
            start.wait().await;
            service.cancel(alice, &id).await
        });
    }
    let cancelled = ReservationStatus {
        id: created.id,
        state: State::Cancelled,
    };
    while let Some(outcome) = tasks.join_next().await {
        assert_eq!(outcome.unwrap().unwrap(), cancelled);
    }
    assert_eq!(
        service.reserve(alice, "matinee", "key").await.unwrap(),
        cancelled
    );
    assert_eq!(service.catalog().await.unwrap().concerts[2].available, 2);
}
