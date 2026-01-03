//! Shared fixtures for Kagami integration tests.

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use color_eyre::eyre::{Result, WrapErr, ensure, eyre};
use iroha_data_model::peer::PeerId;

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
) -> norito::json::Value {
    let mut tx_map = norito::json::Map::new();
    let topo_entries = topology
        .iter()
        .map(|peer| norito::json::value::to_value(peer).expect("serialize peer id"))
        .collect();
    tx_map.insert("topology".to_string(), norito::json::Value::Array(topo_entries));

    let mut root = norito::json::Map::new();
    root.insert("chain".to_string(), norito::json::Value::String(chain.to_owned()));
    root.insert("executor".to_string(), norito::json::Value::Null);
    root.insert(
        "ivm_dir".to_string(),
        norito::json::Value::String(ivm_dir.into().display().to_string()),
    );
    root.insert(
        "transactions".to_string(),
        norito::json::Value::Array(vec![norito::json::Value::Object(tx_map)]),
    );
    norito::json::Value::Object(root)
}

/// Serialize a raw genesis manifest to JSON and write it to `path`.
pub fn write_raw_genesis_to(path: &Path, manifest: &norito::json::Value) -> Result<()> {
    let bytes = norito::json::to_vec(manifest).wrap_err("serialize genesis manifest to JSON")?;
    fs::write(path, bytes)
        .wrap_err_with(|| format!("write genesis manifest to {}", path.display()))?;
    Ok(())
}
