//! Integration test for `kagami genesis embed-pop` populating `pop_hex` inside topology entries.
mod common;

use std::process::Command;

use color_eyre::{
    Result,
    eyre::{WrapErr, ensure},
};
use common::{PopFixture, generate_pop, minimal_manifest_with_topology, write_raw_genesis_to};

#[test]
fn pop_and_embed_populates_pop_hex_entries() -> Result<()> {
    let pop1 = generate_pop("seedA")?;
    let pop2 = generate_pop("seedB")?;
    let PopFixture {
        peer_id: peer1,
        pop_hex: pop_hex1,
    } = pop1;
    let PopFixture {
        peer_id: peer2,
        pop_hex: pop_hex2,
    } = pop2;

    // Build a minimal genesis manifest using shared fixtures
    let manifest = minimal_manifest_with_topology("chain", ".", &[peer1.clone(), peer2.clone()]);

    let dir = tempfile::tempdir().wrap_err("create temporary directory")?;
    let manifest_path = dir.path().join("genesis.json");
    let out_path = dir.path().join("genesis_out.json");
    write_raw_genesis_to(&manifest_path, &manifest)?;

    // Run embed-pop
    let arg1 = format!("{peer1}={pop_hex1}");
    let arg2 = format!("{peer2}={pop_hex2}");
    let status = Command::new(env!("CARGO_BIN_EXE_kagami"))
        .args([
            "genesis",
            "embed-pop",
            "--manifest",
            manifest_path.to_str().unwrap(),
            "--out",
            out_path.to_str().unwrap(),
            "--peer-pop",
            &arg1,
            "--peer-pop",
            &arg2,
        ])
        .status()
        .wrap_err("run `kagami genesis embed-pop`")?;
    ensure!(status.success(), "`kagami genesis embed-pop` failed");

    // Read back and check pop_hex populated
    let out_bytes = std::fs::read(&out_path).wrap_err("read embed-pop output")?;
    let mf2: norito::json::Value =
        norito::json::from_slice(&out_bytes).wrap_err("parse embed-pop JSON output")?;
    // Check structure without relying on private fields
    let txs = mf2["transactions"].as_array().expect("transactions array");
    assert_eq!(txs.len(), 1);
    let t0 = &txs[0];
    let topology = t0["topology"]
        .as_array()
        .expect("topology array should exist");
    assert_eq!(topology.len(), 2);

    let mut seen = std::collections::BTreeMap::new();
    for entry in topology {
        let obj = entry.as_object().expect("topology entry object");
        let peer_pk = match obj.get("peer") {
            Some(norito::json::Value::Object(map)) => map
                .get("public_key")
                .and_then(norito::json::Value::as_str)
                .expect("peer.public_key string")
                .to_string(),
            Some(norito::json::Value::String(s)) => s.clone(),
            _ => panic!("unexpected peer value in topology entry"),
        };
        let pop_hex = obj["pop_hex"].as_str().expect("pop_hex string").to_string();
        seen.insert(peer_pk, pop_hex);
    }

    let expected1 = pop_hex1.to_ascii_lowercase();
    let expected2 = pop_hex2.to_ascii_lowercase();
    assert_eq!(
        seen.get(&peer1.public_key.to_string())
            .map(|s| s.to_ascii_lowercase()),
        Some(expected1),
        "expected pop for peer1"
    );
    assert_eq!(
        seen.get(&peer2.public_key.to_string())
            .map(|s| s.to_ascii_lowercase()),
        Some(expected2),
        "expected pop for peer2"
    );
    Ok(())
}
