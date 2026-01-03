use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::{BufWriter, Write},
    path::PathBuf,
};

use clap::Parser;
use color_eyre::eyre::{WrapErr, eyre};
use iroha_crypto::{Algorithm, KeyPair, PrivateKey};
use iroha_data_model::{
    parameter::{SumeragiParameter, system::SumeragiConsensusMode},
    prelude::*,
};
use iroha_genesis::{GenesisTopologyEntry, RawGenesisTransaction};

use super::{ensure_npos_parameters, generate::ConsensusModeArg};
use crate::{Outcome, RunArgs, tui};

/// Sign the genesis block
#[derive(Clone, Debug, Parser)]
pub struct Args {
    /// Path to genesis json file
    genesis_file: PathBuf,
    /// Path to signed genesis output file in Norito format (stdout by default).
    #[clap(short, long, value_name("PATH"))]
    out_file: Option<PathBuf>,
    /// Use this topology instead of specified in genesis.json.
    /// JSON-serialized vector of `PeerId`. For use in `iroha_swarm`.
    #[clap(short, long)]
    topology: Option<String>,
    /// Embed one or more PoPs into the same transaction as `--topology`.
    /// Repeatable flag: `--peer-pop <public_key=pop_hex>`
    #[clap(long = "peer-pop")]
    peer_pops: Vec<String>,
    /// Private key hex (multihash payload, not prefixed) that matches the genesis public key.
    #[clap(long, conflicts_with = "seed", value_name = "HEX")]
    private_key: Option<String>,
    /// Seed string to derive the genesis key (testing convenience).
    #[clap(long, conflicts_with = "private_key", value_name = "SEED")]
    seed: Option<String>,
    /// Algorithm of the genesis key (must match the genesis public key).
    #[clap(long, default_value = "ed25519", value_name = "ALGORITHM")]
    algorithm: Algorithm,
    /// Select the consensus mode to stamp into the manifest (optional override).
    #[clap(long, value_enum, value_name = "MODE")]
    consensus_mode: Option<ConsensusModeArg>,
    /// Optional future consensus mode to stage behind `--mode-activation-height`.
    #[clap(long, value_enum, value_name = "MODE")]
    next_consensus_mode: Option<ConsensusModeArg>,
    /// Optional: set the block height at which `next_mode` should activate (requires `--next-consensus-mode`).
    #[clap(long, value_name = "HEIGHT")]
    mode_activation_height: Option<u64>,
}

impl<T: Write> RunArgs<T> for Args {
    fn run(self, writer: &mut BufWriter<T>) -> Outcome {
        tui::status("Signing genesis manifest");
        match (self.next_consensus_mode, self.mode_activation_height) {
            (Some(_), None) => {
                return Err(eyre!(
                    "`--next-consensus-mode` requires `--mode-activation-height` to stage a cutover"
                ));
            }
            (None, Some(_)) => {
                return Err(eyre!(
                    "`--mode-activation-height` requires `--next-consensus-mode` to stage a cutover"
                ));
            }
            _ => {}
        }
        if let Some(height) = self.mode_activation_height
            && height == 0
        {
            return Err(eyre!(
                "`--mode-activation-height` must be greater than zero"
            ));
        }
        let consensus_mode = self.consensus_mode.map_or(
            SumeragiConsensusMode::Permissioned,
            SumeragiConsensusMode::from,
        );
        let next_consensus_mode = self.next_consensus_mode.map(SumeragiConsensusMode::from);

        let genesis = RawGenesisTransaction::from_path(&self.genesis_file)?
            .with_consensus_mode(consensus_mode);
        if matches!(next_consensus_mode, Some(SumeragiConsensusMode::Npos)) {
            ensure_npos_parameters(&genesis)?;
        }
        let mut builder = genesis.into_builder();

        if self.topology.is_none() && !self.peer_pops.is_empty() {
            return Err(eyre!(
                "--peer-pop requires --topology to align PoPs with peers"
            ));
        }

        if let Some(topology) = self.topology {
            let topology: Vec<PeerId> =
                norito::json::from_str(&topology).wrap_err("parse --topology JSON")?;
            // Put topology into a dedicated transaction so it remains separate
            // from other genesis instructions.
            let entries = build_topology_entries(&topology, &self.peer_pops)?;
            builder = builder.next_transaction().set_topology(entries);
        }
        if let Some(mode) = next_consensus_mode {
            builder =
                builder.append_parameter(Parameter::Sumeragi(SumeragiParameter::NextMode(mode)));
        }
        if let (Some(height), Some(_)) = (self.mode_activation_height, self.next_consensus_mode) {
            builder = builder.append_parameter(Parameter::Sumeragi(
                SumeragiParameter::ModeActivationHeight(height),
            ));
        }
        let genesis_key_pair = load_genesis_key(
            self.private_key.as_deref(),
            self.seed.as_deref(),
            self.algorithm,
        )?;
        let genesis_block = builder
            .build_raw()
            .with_consensus_mode(consensus_mode)
            .with_consensus_meta()
            .build_and_sign(&genesis_key_pair)?;

        eprintln!("Genesis public key: {}", genesis_key_pair.public_key());

        let mut writer: Box<dyn Write> = match self.out_file {
            None => Box::new(writer),
            Some(path) => Box::new(BufWriter::new(File::create(path)?)),
        };
        let framed = genesis_block
            .0
            .encode_wire()
            .wrap_err("frame genesis block with Norito header")?;
        writer.write_all(&framed)?;
        tui::success("Genesis block signed");

        Ok(())
    }
}

fn load_genesis_key(
    private_key_hex: Option<&str>,
    seed: Option<&str>,
    algorithm: Algorithm,
) -> Result<KeyPair, color_eyre::eyre::Error> {
    match (private_key_hex, seed) {
        (Some(hex), None) => {
            let sk = PrivateKey::from_hex(algorithm, hex).wrap_err("decode genesis private key")?;
            Ok(KeyPair::from(sk))
        }
        (None, Some(seed)) => Ok(KeyPair::from_seed(seed.as_bytes().to_vec(), algorithm)),
        (None, None) => Err(eyre!(
            "genesis signing requires a private key; pass --private-key or --seed"
        )),
        (Some(_), Some(_)) => unreachable!("clap enforces conflicts"),
    }
}

fn build_topology_entries(
    topology: &[PeerId],
    peer_pops: &[String],
) -> Result<Vec<GenesisTopologyEntry>, color_eyre::eyre::Error> {
    use iroha_crypto::PublicKey;

    if peer_pops.is_empty() {
        return Err(eyre!(
            "topology provided without PoPs; supply --peer-pop for every peer"
        ));
    }

    let topo_set: BTreeSet<PublicKey> = topology
        .iter()
        .map(|pid| pid.public_key().clone())
        .collect();

    let mut map: BTreeMap<PublicKey, Vec<u8>> = BTreeMap::new();
    for kv in peer_pops {
        let (k, v) = kv
            .split_once('=')
            .ok_or_else(|| eyre!("invalid --peer-pop entry: {kv}"))?;
        let pk: PublicKey = k.parse()?;
        if !topo_set.contains(&pk) {
            return Err(eyre!(
                "peer-pop provided for {pk} but that peer is not present in --topology"
            ));
        }
        if map.insert(pk.clone(), decode_hex(v)?).is_some() {
            return Err(eyre!("duplicate --peer-pop entry for {pk}"));
        }
    }

    let missing: Vec<_> = topo_set
        .iter()
        .filter(|pk| !map.contains_key(*pk))
        .cloned()
        .collect();
    if !missing.is_empty() {
        let joined = missing
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        return Err(eyre!(
            "missing --peer-pop entries for topology peers: {joined}"
        ));
    }

    Ok(topology
        .iter()
        .map(|peer| {
            let pk = peer.public_key();
            GenesisTopologyEntry::new(
                peer.clone(),
                map.get(pk)
                    .cloned()
                    .expect("topology keys validated against pop map"),
            )
        })
        .collect())
}

fn decode_hex(s: &str) -> Result<Vec<u8>, color_eyre::eyre::Error> {
    let s = s.trim_start_matches("0x");
    if !s.len().is_multiple_of(2) {
        return Err(color_eyre::eyre::eyre!("odd hex length"));
    }
    let mut out = Vec::with_capacity(s.len() / 2);
    let b = s.as_bytes();
    for i in (0..b.len()).step_by(2) {
        let h = from_hex_nibble(b[i]).ok_or_else(|| color_eyre::eyre::eyre!("bad hex"))?;
        let l = from_hex_nibble(b[i + 1]).ok_or_else(|| color_eyre::eyre::eyre!("bad hex"))?;
        out.push((h << 4) | l);
    }
    Ok(out)
}

fn from_hex_nibble(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        io::{BufWriter, Write},
        path::PathBuf,
    };

    use iroha_crypto::KeyPair as CryptoKeyPair;
    use iroha_data_model::{
        ChainId,
        parameter::{Parameter, system::SumeragiNposParameters},
    };
    use iroha_genesis::GenesisBuilder;

    use super::*;

    #[test]
    fn peer_pops_without_topology_is_rejected() {
        let args = Args {
            genesis_file: minimal_genesis_file(),
            out_file: None,
            topology: None,
            peer_pops: vec!["pk=00".to_string()],
            private_key: Some(test_private_key_hex()),
            seed: None,
            algorithm: Algorithm::Ed25519,
            consensus_mode: None,
            next_consensus_mode: None,
            mode_activation_height: None,
        };

        let mut sink = BufWriter::new(Vec::new());
        let err = args
            .run(&mut sink)
            .expect_err("peer-pop without topology should fail");
        assert!(
            err.to_string().contains("requires --topology"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn duplicate_peer_pops_are_rejected() {
        let peer = PeerId::new(CryptoKeyPair::random().public_key().clone());
        let topology_json = norito::json::to_json(&vec![peer.clone()]).unwrap();
        let pk = peer.public_key();
        let dup = format!("{pk}=00");
        let args = Args {
            genesis_file: minimal_genesis_file(),
            out_file: None,
            topology: Some(topology_json),
            peer_pops: vec![dup.clone(), dup],
            private_key: Some(test_private_key_hex()),
            seed: None,
            algorithm: Algorithm::Ed25519,
            consensus_mode: None,
            next_consensus_mode: None,
            mode_activation_height: None,
        };

        let mut sink = BufWriter::new(Vec::new());
        let err = args.run(&mut sink).expect_err("duplicate pop should fail");
        assert!(
            err.to_string().contains("duplicate --peer-pop"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn topology_entries_order_matches_topology() {
        let peer_a = PeerId::new(CryptoKeyPair::random().public_key().clone());
        let peer_b = PeerId::new(CryptoKeyPair::random().public_key().clone());
        let topology = vec![peer_a.clone(), peer_b.clone()];
        let entries = build_topology_entries(
            &topology,
            &[
                format!("{}=01", peer_a.public_key()),
                format!("{}=02", peer_b.public_key()),
            ],
        )
        .expect("valid pops");
        assert_eq!(
            entries[0].peer,
            peer_a,
            "entries should respect topology order"
        );
        assert_eq!(
            entries[1].peer,
            peer_b,
            "entries should respect topology order"
        );
    }

    #[test]
    fn load_genesis_key_accepts_seed_and_algorithm() {
        let kp = load_genesis_key(None, Some("seed-123"), Algorithm::Secp256k1)
            .expect("seed path should work");
        assert_eq!(kp.public_key().algorithm(), Algorithm::Secp256k1);
    }

    #[test]
    fn run_returns_err_on_invalid_topology_json() {
        let mut genesis_file = tempfile::NamedTempFile::new().expect("create temp genesis file");
        let genesis_json = r#"{
            "chain": "test-chain",
            "executor": null,
            "ivm_dir": ".",
            "transactions": [
                {}
            ]
        }"#;
        write!(genesis_file, "{genesis_json}").expect("write genesis json");

        let args = Args {
            genesis_file: genesis_file.path().to_path_buf(),
            out_file: None,
            topology: Some("not valid json".to_owned()),
            peer_pops: Vec::new(),
            private_key: Some(test_private_key_hex()),
            seed: None,
            algorithm: Algorithm::Ed25519,
            consensus_mode: None,
            next_consensus_mode: None,
            mode_activation_height: None,
        };

        let mut writer = BufWriter::new(Vec::new());
        let result = args.run(&mut writer);

        assert!(result.is_err());
    }

    #[test]
    fn run_requires_key_material() {
        let mut genesis_file = tempfile::NamedTempFile::new().expect("create temp genesis file");
        let genesis_json = r#"{
            "chain": "test-chain",
            "executor": null,
            "ivm_dir": ".",
            "transactions": [
                {}
            ]
        }"#;
        write!(genesis_file, "{genesis_json}").expect("write genesis json");

        let args = Args {
            genesis_file: genesis_file.path().to_path_buf(),
            out_file: None,
            topology: None,
            peer_pops: Vec::new(),
            private_key: None,
            seed: None,
            algorithm: Algorithm::Ed25519,
            consensus_mode: None,
            next_consensus_mode: None,
            mode_activation_height: None,
        };

        let mut writer = BufWriter::new(Vec::new());
        let result = args.run(&mut writer);
        assert!(result.is_err(), "signing should require key material");
    }

    #[test]
    fn missing_pops_fail_when_topology_provided() {
        let mut genesis_file = tempfile::NamedTempFile::new().expect("create temp genesis file");
        let genesis_json = r#"{
            "chain": "test-chain",
            "executor": null,
            "ivm_dir": ".",
            "transactions": [
                {}
            ]
        }"#;
        write!(genesis_file, "{genesis_json}").expect("write genesis json");

        let peer_a = PeerId::new(CryptoKeyPair::random().public_key().clone());
        let peer_b = PeerId::new(CryptoKeyPair::random().public_key().clone());
        let topology_json = norito::json::to_json(&vec![peer_a.clone(), peer_b]).unwrap();

        // Provide PoP only for peer_a to trigger the missing-pop validation.
        let args = Args {
            genesis_file: genesis_file.path().to_path_buf(),
            out_file: None,
            topology: Some(topology_json),
            peer_pops: vec![format!(
                "{}={}",
                peer_a.public_key(),
                "00" // minimal hex payload for test
            )],
            private_key: Some(test_private_key_hex()),
            seed: None,
            algorithm: Algorithm::Ed25519,
            consensus_mode: None,
            next_consensus_mode: None,
            mode_activation_height: None,
        };

        let mut writer = BufWriter::new(Vec::new());
        let result = args.run(&mut writer);
        assert!(
            result.is_err(),
            "signing should fail when topology peers lack PoPs"
        );
    }

    #[test]
    fn mode_activation_requires_next_mode_flag() {
        let args = Args {
            genesis_file: minimal_genesis_file(),
            out_file: None,
            topology: None,
            peer_pops: Vec::new(),
            private_key: Some(test_private_key_hex()),
            seed: None,
            algorithm: Algorithm::Ed25519,
            consensus_mode: None,
            next_consensus_mode: None,
            mode_activation_height: Some(5),
        };

        let mut writer = BufWriter::new(Vec::new());
        let err = args
            .run(&mut writer)
            .expect_err("activation without mode should fail");
        assert!(
            err.to_string().contains("next-consensus-mode"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn npos_sign_requires_npos_parameters() {
        let args = Args {
            genesis_file: minimal_genesis_file(),
            out_file: None,
            topology: None,
            peer_pops: Vec::new(),
            private_key: Some(test_private_key_hex()),
            seed: None,
            algorithm: Algorithm::Ed25519,
            consensus_mode: None,
            next_consensus_mode: Some(ConsensusModeArg::Npos),
            mode_activation_height: Some(10),
        };

        let mut writer = BufWriter::new(Vec::new());
        let err = args
            .run(&mut writer)
            .expect_err("NPoS signing without parameters should fail");
        assert!(
            err.to_string().contains("sumeragi_npos_parameters"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn npos_sign_accepts_manifest_with_npos_parameters() {
        let args = Args {
            genesis_file: npos_genesis_file(),
            out_file: None,
            topology: None,
            peer_pops: Vec::new(),
            private_key: Some(test_private_key_hex()),
            seed: None,
            algorithm: Algorithm::Ed25519,
            consensus_mode: None,
            next_consensus_mode: Some(ConsensusModeArg::Npos),
            mode_activation_height: Some(4),
        };

        let mut writer = BufWriter::new(Vec::new());
        args.run(&mut writer)
            .expect("NPoS genesis with parameters should sign");
    }

    fn minimal_genesis_file() -> PathBuf {
        let mut genesis_file = tempfile::Builder::new()
            .prefix("kagami-genesis-test-")
            .tempfile()
            .expect("create temp genesis file");
        let genesis_json = r#"{
            "chain": "test-chain",
            "executor": null,
            "ivm_dir": ".",
            "transactions": [
                {}
            ]
        }"#;
        write!(genesis_file, "{genesis_json}").expect("write genesis json");
        let (_file, path) = genesis_file.keep().expect("persist temp genesis");
        path
    }

    fn npos_genesis_file() -> PathBuf {
        let genesis_file = tempfile::Builder::new()
            .prefix("kagami-npos-genesis-")
            .tempfile()
            .expect("create temp genesis file");
        let manifest =
            GenesisBuilder::new_without_executor(ChainId::from("npos-sign"), PathBuf::from("."))
                .append_parameter(Parameter::Custom(
                    SumeragiNposParameters::default().into_custom_parameter(),
                ))
                .build_raw();
        let json = norito::json::to_json_pretty(&manifest).expect("serialize genesis manifest");
        fs::write(genesis_file.path(), json).expect("write genesis json");
        let (_file, path) = genesis_file.keep().expect("persist temp genesis");
        path
    }

    fn test_private_key_hex() -> String {
        let kp = CryptoKeyPair::random_with_algorithm(Algorithm::Ed25519);
        let (_alg, bytes) = kp.private_key().to_bytes();
        hex::encode(bytes)
    }
}
