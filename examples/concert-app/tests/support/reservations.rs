use std::{collections::BTreeMap, ffi::OsString, fs, path::Path, sync::Arc};

use gdd_concert::{DomainError as Error, Principal, ReservationStatus, Reservations};
use tokio::{sync::Barrier, task::JoinSet};

pub fn signed_in(id: &str) -> Principal<'_> {
    Principal::from_session(Some(id)).unwrap()
}

pub fn files_and_bytes(directory: &Path) -> BTreeMap<OsString, Vec<u8>> {
    fs::read_dir(directory)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            (entry.file_name(), fs::read(entry.path()).unwrap())
        })
        .collect()
}

pub async fn race(
    service: &Reservations,
    requests: &[(&'static str, &'static str, &'static str)],
) -> Vec<Result<ReservationStatus, Error>> {
    let start = Arc::new(Barrier::new(requests.len()));
    let mut tasks = JoinSet::new();
    for &(principal, concert, key) in requests {
        let service = service.clone();
        let start = start.clone();
        tasks.spawn(async move {
            start.wait().await;
            service.reserve(signed_in(principal), concert, key).await
        });
    }
    let mut results = Vec::new();
    while let Some(result) = tasks.join_next().await {
        results.push(result.unwrap());
    }
    results
}
