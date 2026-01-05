use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::PathBuf,
};

use clap::Args as ClapArgs;
use color_eyre::eyre::{WrapErr as _, eyre};
use iroha_crypto::PublicKey;

use crate::{Outcome, RunArgs, tui};

/// Embed one or more PoPs into a JSON genesis manifest (inlined under `topology` entries).
#[derive(ClapArgs, Debug, Clone)]
pub struct Args {
    /// Input genesis JSON file (RawGenesisTransaction)
    #[clap(long)]
    manifest: PathBuf,
    /// Output file path
    #[clap(long)]
    out: PathBuf,
    /// Peer PoP entries in the form `public_key=hex`
    #[clap(long = "peer-pop")]
    peer_pops: Vec<String>,
}

impl<T: std::io::Write> RunArgs<T> for Args {
    fn run(self, _writer: &mut std::io::BufWriter<T>) -> Outcome {
        tui::status("Embedding PoP entries into genesis manifest");
        let bytes = fs::read(&self.manifest).wrap_err("read manifest")?;
        let mut manifest: norito::json::Value =
            norito::json::from_slice(&bytes).wrap_err("parse genesis json")?;
        ensure_consensus_mode(&manifest)?;

        let mut pops: BTreeMap<PublicKey, Vec<u8>> = BTreeMap::new();
        for kv in &self.peer_pops {
            let (k, v) = kv
                .split_once('=')
                .ok_or_else(|| color_eyre::eyre::eyre!("invalid --peer-pop entry: {kv}"))?;
            let pk: PublicKey = k.parse().wrap_err("parse public_key")?;
            let pop = hex::decode(v.trim_start_matches("0x")).wrap_err("parse pop hex")?;
            if pop.is_empty() {
                return Err(eyre!("--peer-pop entry for {pk} is empty"));
            }
            if pops.insert(pk.clone(), pop).is_some() {
                return Err(eyre!("duplicate --peer-pop entry for peer {pk}"));
            }
        }

        let mut used_pops = BTreeSet::new();
        let txs = manifest
            .get_mut("transactions")
            .and_then(norito::json::Value::as_array_mut)
            .ok_or_else(|| eyre!("genesis manifest missing `transactions` array"))?;
        if txs.is_empty() {
            return Err(eyre!(
                "genesis manifest must include at least one transaction entry"
            ));
        }
        for (idx, tx) in txs.iter_mut().enumerate() {
            let Some(tx_obj) = tx.as_object_mut() else {
                return Err(eyre!("transaction at index {} must be a JSON object", idx));
            };
            let topology_value = match tx_obj.remove("topology") {
                Some(value) => value,
                None => continue,
            };
            let entries = topology_value
                .as_array()
                .cloned()
                .ok_or_else(|| eyre!("topology for tx {idx} must be an array"))?;
            if entries.is_empty() {
                tx_obj.insert("topology".into(), norito::json::Value::Array(Vec::new()));
                continue;
            }
            let mut out_entries = Vec::with_capacity(entries.len());
            for (entry_idx, entry) in entries.into_iter().enumerate() {
                let (peer_value, pk) = extract_peer(entry)
                    .map_err(|err| eyre!("transactions[{idx}].topology[{entry_idx}]: {err}"))?;
                let pop = pops
                    .get(&pk)
                    .ok_or_else(|| eyre!("missing --peer-pop entry for peer {}", pk))?;
                used_pops.insert(pk);
                let mut map = norito::json::native::Map::new();
                map.insert("peer".into(), peer_value);
                map.insert("pop_hex".into(), norito::json::Value::from(encode_hex(pop)));
                out_entries.push(norito::json::Value::Object(map));
            }
            tx_obj.insert("topology".into(), norito::json::Value::Array(out_entries));
        }
        if !pops.is_empty() {
            let unused: Vec<String> = pops
                .keys()
                .filter(|pk| !used_pops.contains(*pk))
                .map(ToString::to_string)
                .collect();
            if !unused.is_empty() {
                return Err(eyre!(
                    "peer-pop entries provided for peers not present in topology: {}",
                    unused.join(", ")
                ));
            }
        }

        let json = norito::json::to_json_pretty(&manifest).wrap_err("serialize genesis")?;
        fs::write(&self.out, json).wrap_err("write out")?;
        tui::success("Genesis manifest updated");
        Ok(())
    }
}

fn extract_peer(
    entry: norito::json::Value,
) -> color_eyre::Result<(norito::json::Value, PublicKey)> {
    use iroha_data_model::peer::PeerId;
    use norito::json::Value;

    let Value::Object(mut map) = entry else {
        return Err(eyre!(
            "topology entries must be objects with `peer` and optional `pop_hex`"
        ));
    };
    let peer_value = map
        .remove("peer")
        .ok_or_else(|| eyre!("topology entry missing `peer` field"))?;
    let peer_raw = peer_value
        .as_str()
        .ok_or_else(|| eyre!("topology entry `peer` must be a public-key string (PeerId)"))?;
    let peer_id: PeerId = peer_raw.parse().wrap_err("parse topology peer")?;
    if let Some(pop_value) = map.remove("pop_hex") {
        match pop_value {
            Value::Null => {}
            Value::String(raw) => {
                if raw.is_empty() {
                    return Err(eyre!("topology entry `pop_hex` must not be empty"));
                }
            }
            _ => {
                return Err(eyre!("topology entry `pop_hex` must be a string or null"));
            }
        }
    }
    if let Some((field, _)) = map.into_iter().next() {
        return Err(eyre!("unknown field `{field}` in topology entry"));
    }
    let public_key = peer_id.public_key().clone();
    let canonical = Value::String(public_key.to_string());
    Ok((canonical, public_key))
}

fn ensure_consensus_mode(manifest: &norito::json::Value) -> color_eyre::Result<()> {
    use iroha_data_model::parameter::system::SumeragiConsensusMode;

    let Some(raw) = manifest.get("consensus_mode") else {
        return Err(eyre!("genesis manifest missing `consensus_mode`"));
    };
    let _: SumeragiConsensusMode =
        norito::json::value::from_value(raw.clone()).wrap_err("decode `consensus_mode`")?;
    Ok(())
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use std::io::BufWriter;

    use tempfile::NamedTempFile;

    use super::*;

    #[test]
    fn rejects_non_object_transactions() {
        let manifest = r#"{
            "chain": "0",
            "executor": null,
            "ivm_dir": ".",
            "consensus_mode": "Permissioned",
            "transactions": ["not-a-map"]
        }"#;

        let input = NamedTempFile::new().expect("create manifest tmp file");
        let output = NamedTempFile::new().expect("create output tmp file");
        fs::write(input.path(), manifest).expect("write manifest");

        let args = Args {
            manifest: input.path().to_path_buf(),
            out: output.path().to_path_buf(),
            peer_pops: Vec::new(),
        };

        let mut sink = BufWriter::new(Vec::<u8>::new());
        let err = args
            .run(&mut sink)
            .expect_err("should reject non-object tx");
        assert!(
            err.to_string().contains("transaction at index 0"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn rejects_missing_consensus_mode() {
        let manifest = r#"{
            "chain": "0",
            "executor": null,
            "ivm_dir": ".",
            "transactions": [
                {}
            ]
        }"#;

        let input = NamedTempFile::new().expect("create manifest tmp file");
        let output = NamedTempFile::new().expect("create output tmp file");
        fs::write(input.path(), manifest).expect("write manifest");

        let args = Args {
            manifest: input.path().to_path_buf(),
            out: output.path().to_path_buf(),
            peer_pops: Vec::new(),
        };

        let mut sink = BufWriter::new(Vec::<u8>::new());
        let err = args
            .run(&mut sink)
            .expect_err("missing consensus_mode should fail");
        assert!(
            err.to_string().contains("consensus_mode"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn embeds_pops_into_topology_entries() {
        let kp = iroha_crypto::KeyPair::random();
        let pk = kp.public_key();
        let manifest = format!(
            r#"{{
            "chain": "0",
            "executor": null,
            "ivm_dir": ".",
            "consensus_mode": "Permissioned",
            "transactions": [{{
                "topology": [{{"peer": "{pk}"}}]
            }}]
        }}"#
        );

        let input = NamedTempFile::new().expect("create manifest tmp file");
        let output = NamedTempFile::new().expect("create output tmp file");
        fs::write(input.path(), manifest).expect("write manifest");

        let args = Args {
            manifest: input.path().to_path_buf(),
            out: output.path().to_path_buf(),
            peer_pops: vec![format!("{pk}=00")],
        };

        let mut sink = BufWriter::new(Vec::<u8>::new());
        args.run(&mut sink).expect("embed pop should succeed");

        let out_json: norito::json::Value =
            norito::json::from_str(&fs::read_to_string(output.path()).expect("read output"))
                .expect("parse output");
        let topo_entry = out_json["transactions"][0]["topology"][0]
            .as_object()
            .expect("topology entry");
        assert_eq!(
            topo_entry
                .get("pop_hex")
                .and_then(norito::json::Value::as_str)
                .unwrap_or_default(),
            "00"
        );
        assert!(
            topo_entry.contains_key("peer"),
            "topology entry should wrap peer"
        );
    }

    #[test]
    fn rejects_noncanonical_peer_value() {
        let kp = iroha_crypto::KeyPair::random();
        let pk = kp.public_key();
        let manifest = format!(
            r#"{{
            "chain": "0",
            "executor": null,
            "ivm_dir": ".",
            "consensus_mode": "Permissioned",
            "transactions": [{{
                "topology": [{{"peer": {{"public_key": "{pk}", "address": "127.0.0.1:8080"}}}}]
            }}]
        }}"#
        );

        let input = NamedTempFile::new().expect("create manifest tmp file");
        let output = NamedTempFile::new().expect("create output tmp file");
        fs::write(input.path(), manifest).expect("write manifest");

        let args = Args {
            manifest: input.path().to_path_buf(),
            out: output.path().to_path_buf(),
            peer_pops: vec![format!("{pk}=00")],
        };

        let mut sink = BufWriter::new(Vec::<u8>::new());
        let err = args
            .run(&mut sink)
            .expect_err("non-canonical topology should fail");
        assert!(
            err.to_string()
                .contains("topology entry `peer` must be a public-key string"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn rejects_duplicate_peer_pops() {
        let kp = iroha_crypto::KeyPair::random();
        let pk = kp.public_key();
        let manifest = format!(
            r#"{{
            "chain": "0",
            "executor": null,
            "ivm_dir": ".",
            "consensus_mode": "Permissioned",
            "transactions": [{{
                "topology": [{{"peer": "{pk}"}}]
            }}]
        }}"#
        );

        let input = NamedTempFile::new().expect("create manifest tmp file");
        let output = NamedTempFile::new().expect("create output tmp file");
        fs::write(input.path(), manifest).expect("write manifest");

        let args = Args {
            manifest: input.path().to_path_buf(),
            out: output.path().to_path_buf(),
            peer_pops: vec![format!("{pk}=00"), format!("{pk}=01")],
        };

        let mut sink = BufWriter::new(Vec::<u8>::new());
        let err = args
            .run(&mut sink)
            .expect_err("duplicate peer pops should fail");
        assert!(
            err.to_string().contains("duplicate --peer-pop"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn rejects_unused_peer_pops() {
        let peer_a = iroha_crypto::KeyPair::random();
        let peer_b = iroha_crypto::KeyPair::random();
        let pk_a = peer_a.public_key();
        let pk_b = peer_b.public_key();
        let manifest = format!(
            r#"{{
            "chain": "0",
            "executor": null,
            "ivm_dir": ".",
            "consensus_mode": "Permissioned",
            "transactions": [{{
                "topology": [{{"peer": "{pk_a}"}}]
            }}]
        }}"#
        );

        let input = NamedTempFile::new().expect("create manifest tmp file");
        let output = NamedTempFile::new().expect("create output tmp file");
        fs::write(input.path(), manifest).expect("write manifest");

        let args = Args {
            manifest: input.path().to_path_buf(),
            out: output.path().to_path_buf(),
            peer_pops: vec![format!("{pk_a}=00"), format!("{pk_b}=01")],
        };

        let mut sink = BufWriter::new(Vec::<u8>::new());
        let err = args
            .run(&mut sink)
            .expect_err("unused peer pops should fail");
        assert!(
            err.to_string()
                .contains("peer-pop entries provided for peers not present in topology"),
            "unexpected error: {err}"
        );
    }
}
