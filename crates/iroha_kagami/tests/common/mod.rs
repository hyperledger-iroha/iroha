//! Shared fixtures for Kagami integration tests.

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use color_eyre::eyre::{Result, WrapErr, ensure, eyre};
use iroha_data_model::{id::ChainId, peer::PeerId};
use iroha_genesis::{GenesisBuilder, RawGenesisTransaction};

/// Output of `kagami genesis pop --json`.
#[derive(Clone, Debug)]
pub struct PopFixture {
    pub peer_id: PeerId,
    pub pop_hex: String,
}

/// Run `kagami genesis pop` with the provided seed.
pub fn generate_pop(seed: &str) -> Result<PopFixture> {
    let output = Command::new(env!("CARGO_BIN_EXE_kagami"))
        .args([
            "genesis",
            "pop",
            "--algorithm",
            "bls_normal",
            "--seed",
            seed,
            "--json",
        ])
        .output()
        .wrap_err_with(|| format!("failed to run `kagami genesis pop` for seed `{seed}`"))?;

    ensure!(
        output.status.success(),
        "kagami pop for seed `{seed}` failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let value: norito::json::Value = norito::json::from_slice(&output.stdout)
        .wrap_err("parse `kagami genesis pop` JSON output")?;
    let pk = value["public_key"]
        .as_str()
        .ok_or_else(|| eyre!("`public_key` missing in pop output"))?;
    let pop_hex = value["pop_hex"]
        .as_str()
        .ok_or_else(|| eyre!("`pop_hex` missing in pop output"))?
        .to_owned();
    let peer_id: PeerId = pk
        .parse()
        .wrap_err("failed to parse peer public key into PeerId")?;

    Ok(PopFixture { peer_id, pop_hex })
}

/// Build a minimal raw genesis manifest with the provided topology.
pub fn minimal_manifest_with_topology(
    chain: &str,
    ivm_dir: impl Into<PathBuf>,
    topology: &[PeerId],
) -> RawGenesisTransaction {
    let chain_id = ChainId::from(chain.to_owned());

    GenesisBuilder::new_without_executor(chain_id, ivm_dir)
        .set_topology(topology.to_vec())
        .set_topology_pop(Vec::new())
        .build_raw()
}

/// Serialize a raw genesis manifest to JSON and write it to `path`.
pub fn write_raw_genesis_to(path: &Path, manifest: &RawGenesisTransaction) -> Result<()> {
    let bytes = norito::json::to_vec(manifest).wrap_err("serialize genesis manifest to JSON")?;
    fs::write(path, bytes)
        .wrap_err_with(|| format!("write genesis manifest to {}", path.display()))?;
    Ok(())
}
