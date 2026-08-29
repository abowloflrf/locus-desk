use std::{error::Error, future::IntoFuture, io};

use locus_desk::{
    commands::{self, Command},
    config::Config,
    db, jobs,
    state::AppState,
};
use tokio::net::TcpListener;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    match commands::parse_environment()? {
        Command::Serve => serve().await,
        Command::Help => {
            print!("{}", commands::HELP);
            Ok(())
        }
        Command::Version => {
            println!("{}", locus_desk::version::display());
            Ok(())
        }
        command => commands::execute(command).await,
    }
}

async fn serve() -> Result<(), Box<dyn Error>> {
    let config = Config::from_env()?;
    let listener = TcpListener::bind(config.bind()).await?;
    let environment = config.environment();
    let bind = config.bind();
    let data_dir = config.data_dir().to_owned();
    let state = AppState::initialize(config).await?;
    let schema_version = db::schema_version(state.pool()).await?;

    info!(
        environment = %environment,
        bind = %bind,
        data_dir = %data_dir.display(),
        version = env!("CARGO_PKG_VERSION"),
        git_commit = locus_desk::version::GIT_COMMIT,
        schema_version,
        "starting Locus Desk"
    );

    let (worker_shutdown, worker_shutdown_rx) = tokio::sync::watch::channel(false);
    let mut worker = tokio::spawn(jobs::run_worker(state.clone(), worker_shutdown_rx));
    let server = axum::serve(listener, locus_desk::app(state))
        .with_graceful_shutdown(shutdown_signal())
        .into_future();
    tokio::pin!(server);
    let (server_result, worker_finished) = tokio::select! {
        result = &mut server => (result, false),
        result = &mut worker => {
            let message = match result {
                Ok(()) => "content worker exited unexpectedly".to_owned(),
                Err(error) => format!("content worker failed: {error}"),
            };
            (Err(io::Error::other(message)), true)
        }
    };
    let _ = worker_shutdown.send(true);
    if !worker_finished
        && tokio::time::timeout(std::time::Duration::from_secs(5), &mut worker)
            .await
            .is_err()
    {
        worker.abort();
        let _ = worker.await;
    }
    server_result?;

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl-C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
}
