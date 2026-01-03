use std::{collections::BTreeMap, fs, path::PathBuf};

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

        let mut pops: BTreeMap<PublicKey, Vec<u8>> = BTreeMap::new();
        for kv in &self.peer_pops {
            let (k, v) = kv
                .split_once('=')
                .ok_or_else(|| color_eyre::eyre::eyre!("invalid --peer-pop entry: {kv}"))?;
            let pk: PublicKey = k.parse().wrap_err("parse public_key")?;
            let pop = hex::decode(v.trim_start_matches("0x")).wrap_err("parse pop hex")?;
            pops.insert(pk, pop);
        }

        if let Some(txs) = manifest
            .get_mut("transactions")
            .and_then(norito::json::Value::as_array_mut)
        {
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
                    let mut map = norito::json::native::Map::new();
                    map.insert("peer".into(), peer_value);
                    map.insert("pop_hex".into(), norito::json::Value::from(encode_hex(pop)));
                    out_entries.push(norito::json::Value::Object(map));
                }
                tx_obj.insert("topology".into(), norito::json::Value::Array(out_entries));
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

    let peer_value = match entry {
        Value::Object(mut map) => map.remove("peer").map_or(Value::Object(map), |peer| peer),
        other => other,
    };
    let peer_id = match &peer_value {
        Value::Object(map) => {
            if let Some(pk) = map.get("public_key").and_then(Value::as_str) {
                pk.parse::<PublicKey>()
                    .map(PeerId::from)
                    .wrap_err("parse public_key in topology entry")?
            } else {
                norito::json::value::from_value(peer_value.clone())
                    .wrap_err("decode peer in topology entry")?
            }
        }
        _ => norito::json::value::from_value(peer_value.clone())
            .wrap_err("decode peer in topology entry")?,
    };
    Ok((peer_value, peer_id.public_key().clone()))
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
    fn embeds_pops_into_topology_entries() {
        let kp = iroha_crypto::KeyPair::random();
        let pk = kp.public_key();
        let manifest = format!(
            r#"{{
            "chain": "0",
            "executor": null,
            "ivm_dir": ".",
            "transactions": [{{
                "topology": [{{"public_key": "{pk}", "address": "127.0.0.1:8080"}}]
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
}
