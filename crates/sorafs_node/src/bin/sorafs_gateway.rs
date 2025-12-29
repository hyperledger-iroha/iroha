//! Developer entrypoint for the SoraFS gateway trustless profile.
//!
//! The binary serves a single manifest from the local SoraFS storage backend,
//! exposing the trustless CAR/proof endpoints against live pinned data.

use std::{net::SocketAddr, path::PathBuf};

use clap::Parser;
use eyre::Result;
use sorafs_node::{
    config::StorageConfig,
    gateway::{self, GatewayDataset, GatewayState},
};
use tokio::signal;

#[derive(Debug, Parser)]
#[command(
    author,
    version,
    about = "SoraFS gateway server",
    propagate_version = true
)]
struct Args {
    /// Address to bind (e.g. 127.0.0.1:9070).
    #[arg(long, default_value = "127.0.0.1:9070")]
    bind: SocketAddr,

    /// Hex-encoded manifest digest (BLAKE3-256) to serve.
    #[arg(long, value_name = "HEX")]
    manifest_digest: String,

    /// Storage data directory containing pinned manifests/chunks.
    #[arg(long, default_value = "./sorafs_storage")]
    data_dir: PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let config = StorageConfig::builder()
        .enabled(true)
        .data_dir(args.data_dir.clone())
        .build();
    let node = sorafs_node::NodeHandle::new(config);
    let dataset = GatewayDataset::load_from_storage(&node, &args.manifest_digest)?;
    let state = GatewayState::new(dataset);
    let router = gateway::router(state);

    let listener = tokio::net::TcpListener::bind(args.bind).await?;
    println!(
        "sorafs_gateway listening on http://{} (manifest: {}, storage: {})",
        listener.local_addr()?,
        args.manifest_digest,
        args.data_dir.display()
    );

    axum::serve(listener, router.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(err) = signal::ctrl_c().await {
            eprintln!("failed to install Ctrl+C handler: {err}");
        }
    };

    #[cfg(unix)]
    let terminate = async {
        use tokio::signal::unix::{SignalKind, signal};

        match signal(SignalKind::terminate()) {
            Ok(mut sigterm) => {
                sigterm.recv().await;
            }
            Err(err) => eprintln!("failed to install SIGTERM handler: {err}"),
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            println!("shutdown requested via Ctrl+C");
        }
        _ = terminate => {
            #[cfg(unix)]
            println!("shutdown requested via SIGTERM");
        }
    }
}
