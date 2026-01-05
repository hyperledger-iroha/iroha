//! Developer entrypoint for the SoraFS gateway trustless profile.
//!
//! The binary serves a single manifest from the local SoraFS storage backend,
//! exposing the trustless CAR/proof endpoints against live pinned data.

use std::{net::SocketAddr, path::PathBuf};

use clap::Parser;
use eyre::{Result, WrapErr};
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

    /// Hex-encoded 32-byte provider identifier advertised by this gateway.
    #[arg(long, value_name = "HEX")]
    provider_id: String,

    /// Filesystem path to the Ed25519 signing key used for proofs and stream tokens.
    #[arg(long, value_name = "PATH")]
    signing_key: PathBuf,

    /// Storage data directory containing pinned manifests/chunks.
    #[arg(long, default_value = "./sorafs_storage")]
    data_dir: PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let provider_id_bytes =
        hex::decode(args.provider_id.trim()).wrap_err("provider id must be hex-encoded")?;
    let provider_id: [u8; 32] = provider_id_bytes
        .as_slice()
        .try_into()
        .map_err(|_| eyre::eyre!("provider id must be 32 bytes"))?;
    let config = StorageConfig::builder()
        .enabled(true)
        .data_dir(args.data_dir.clone())
        .stream_token_signing_key_path(Some(args.signing_key.clone()))
        .build();
    let node = sorafs_node::NodeHandle::new(config);
    let dataset =
        GatewayDataset::load_from_storage_with_provider(&node, &args.manifest_digest, provider_id)?;
    let state = GatewayState::new(dataset);
    let router = gateway::router(state);

    let listener = tokio::net::TcpListener::bind(args.bind).await?;
    println!(
        "sorafs_gateway listening on http://{} (manifest: {}, provider: {}, storage: {})",
        listener.local_addr()?,
        args.manifest_digest,
        args.provider_id,
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
