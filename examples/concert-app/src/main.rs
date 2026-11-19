mod api;
mod identity;
mod page;
mod server;

use std::{error::Error, net::Ipv4Addr, path::PathBuf, process::ExitCode};

use clap::Parser;
use gdd_concert::Reservations;
use tokio::net::TcpListener;

/// Run the Local Sessions concert program and JSON API on loopback.
#[derive(Parser)]
struct Arguments {
    #[arg(long)]
    database: PathBuf,
    #[arg(long, default_value_t = 8765)]
    port: u16,
    #[arg(long, default_value = "127.0.0.1", value_parser = ["127.0.0.1", "localhost"])]
    host: String,
}

#[ntex::main(signals = false)]
async fn main() -> ExitCode {
    match run(Arguments::parse()).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Local Sessions: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run(args: Arguments) -> Result<(), Box<dyn Error>> {
    // Both accepted host names refer to the IPv4 loopback interface.
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, args.port)).await?;
    let port = listener.local_addr()?.port();
    let shutdown = shutdown_signal()?;
    let reservations = Reservations::open(args.database).await?;
    let app = api::App::new(reservations.clone(), port);
    let served = server::serve(listener, app, shutdown).await;
    reservations.checkpoint().await.map_err(|error| {
        std::io::Error::other(format!(
            "Database checkpoint failed during shutdown: {error}"
        ))
    })?;
    served?;
    Ok(())
}

// Install handlers before announcing readiness, including on an ephemeral port.
#[cfg(unix)]
fn shutdown_signal() -> std::io::Result<impl Future<Output = ()>> {
    use tokio::signal::unix::{SignalKind, signal};
    let mut interrupt = signal(SignalKind::interrupt())?;
    let mut terminate = signal(SignalKind::terminate())?;
    Ok(async move {
        tokio::select! {
            _ = interrupt.recv() => (),
            _ = terminate.recv() => (),
        }
    })
}

#[cfg(windows)]
fn shutdown_signal() -> std::io::Result<impl Future<Output = ()>> {
    let mut interrupt = tokio::signal::windows::ctrl_c()?;
    Ok(async move {
        interrupt.recv().await;
    })
}
