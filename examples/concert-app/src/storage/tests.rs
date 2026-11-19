use super::*;
use crate::{Principal, Reservations};
use tempfile::tempdir;
use tokio::sync::oneshot;

async fn initialized(path: &Path) -> Store {
    let store = Store::open(path).await.unwrap();
    store
        .write(async |tx| Ok(tx.initialize().await?))
        .await
        .unwrap();
    store
}

#[tokio::test]
async fn sql_failure_after_real_insert_and_domain_failure_after_cancel_roll_back() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("reservations.db");
    let store = initialized(&path).await;
    let error = store
        .write(async |tx| {
            tx.insert("alice", "midnight", "retry").await?;
            tx.insert("alice", "absent", "bad-foreign-key").await?;
            Ok(())
        })
        .await
        .unwrap_err();
    assert_eq!(error, DomainError::PersistenceUnavailable);
    let first = store
        .write(async |tx| {
            assert!(tx.history("alice", None).await?.0.is_empty());
            Ok(tx.insert("alice", "northern", "retry").await?)
        })
        .await
        .unwrap();
    let failure: Result<(), _> = store
        .write(async |tx| {
            tx.cancel("alice", &first.id).await?;
            Err(DomainError::InvalidInput)
        })
        .await;
    assert_eq!(failure, Err(DomainError::InvalidInput));
    drop(store);
    let service = Reservations::open(&path).await.unwrap();
    let alice = Principal::from_session(Some("alice")).unwrap();
    assert_eq!(
        service.reserve(alice, "northern", "retry").await.unwrap(),
        first
    );
    assert_eq!(service.list_own(alice).await.unwrap().active_count, 1);
}

#[tokio::test]
async fn deferred_constraint_failure_at_commit_rolls_back_creation_and_key() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("reservations.db");
    let store = initialized(&path).await;
    let create_checks = "CREATE TABLE deferred_checks (
        concert TEXT REFERENCES concerts(id) DEFERRABLE INITIALLY DEFERRED)";
    store
        .0
        .connect()
        .unwrap()
        .execute(create_checks, ())
        .await
        .unwrap();
    let result = store
        .write(async |tx| {
            tx.insert("alice", "midnight", "retry").await?;
            // Both statements succeed, so only COMMIT can report the deferred failure.
            tx.0.execute("INSERT INTO deferred_checks VALUES ('absent')", ())
                .await
                .unwrap();
            Ok(())
        })
        .await;
    assert_eq!(result, Err(DomainError::PersistenceUnavailable));
    let first = store
        .write(async |tx| {
            assert!(tx.find_by_key("alice", "retry").await?.is_none());
            Ok(tx.insert("alice", "northern", "retry").await?)
        })
        .await
        .unwrap();
    assert_eq!(first.state, State::Reserved);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancelling_a_pending_future_releases_the_writer_and_rolls_back() {
    let dir = tempdir().unwrap();
    let store = initialized(&dir.path().join("reservations.db")).await;
    let (inserted, pending) = oneshot::channel();
    let writer = store.clone();
    let operation = tokio::spawn(async move {
        writer
            .write(async move |tx| {
                tx.insert("alice", "midnight", "retry").await?;
                inserted.send(()).unwrap();
                std::future::pending::<Result<(), DomainError>>().await
            })
            .await
    });
    pending.await.unwrap();
    operation.abort();
    assert!(operation.await.unwrap_err().is_cancelled());
    store
        .write(async |tx| {
            assert!(tx.find_by_key("alice", "retry").await?.is_none());
            tx.insert("alice", "midnight", "retry").await?;
            Ok(())
        })
        .await
        .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn new_writer_waits_for_held_writer_before_reading() {
    let dir = tempdir().unwrap();
    let store = initialized(&dir.path().join("reservations.db")).await;
    let mut first = store.0.connect().unwrap();
    let tx = first
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .await
        .unwrap();
    let first_row = Queries(&tx)
        .insert("alice", "midnight", "shared")
        .await
        .unwrap();
    let (started, waiting) = oneshot::channel();
    let mut writer = tokio::spawn(async move {
        started.send(()).unwrap();
        store
            .write(async |tx| Ok(tx.find_by_key("alice", "shared").await?))
            .await
    });
    waiting.await.unwrap();
    assert!(
        tokio::time::timeout(Duration::from_millis(50), &mut writer)
            .await
            .is_err()
    );
    tx.commit().await.unwrap();
    let seen = writer.await.unwrap().unwrap().unwrap();
    assert_eq!(seen.id, first_row.id);
}

#[tokio::test]
async fn startup_seeds_only_missing_concerts_and_keeps_date_then_id_order() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("reservations.db");
    let store = initialized(&path).await;
    store
        .write(async |tx| {
            tx.0.execute("UPDATE concerts SET capacity = 1 WHERE id = 'midnight'", ())
                .await?;
            tx.0.execute("DELETE FROM concerts WHERE id = 'matinee'", ())
                .await?;
            tx.0.execute(
                "INSERT INTO concerts VALUES ('a','Local show','2026-11-14','Stage',9)",
                (),
            )
            .await?;
            tx.insert("alice", "midnight", "one").await?;
            Ok(())
        })
        .await
        .unwrap();
    drop(store);
    let service = Reservations::open(&path).await.unwrap();
    let catalog = service.catalog().await.unwrap().concerts;
    assert_eq!(
        catalog.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
        ["a", "midnight", "northern", "matinee"]
    );
    assert_eq!(catalog[1].capacity, 1);
    assert_eq!(catalog[1].available, 0);
    assert_eq!(catalog[3].capacity, 2);
}

#[tokio::test]
async fn checkpoint_reports_a_busy_writer_and_can_be_retried_after_commit() {
    let dir = tempdir().unwrap();
    let store = initialized(&dir.path().join("concert.sqlite")).await;
    let mut connection = store.0.connect().unwrap();
    let writer = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .await
        .unwrap();
    Queries(&writer)
        .insert("alice", "midnight", "one")
        .await
        .unwrap();
    assert_eq!(
        store.checkpoint().await,
        Err(DomainError::PersistenceUnavailable)
    );
    writer.commit().await.unwrap();
    store.checkpoint().await.unwrap();
}
