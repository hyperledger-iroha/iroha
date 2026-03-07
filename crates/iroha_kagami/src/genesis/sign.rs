use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::{BufWriter, Write},
    path::{Path, PathBuf},
};

use clap::Parser;
use color_eyre::eyre::{WrapErr, eyre};
use iroha_crypto::{Algorithm, KeyPair, PrivateKey};
use iroha_data_model::{
    isi::RegisterPublicLaneValidator,
    parameter::{SumeragiParameter, system::SumeragiConsensusMode},
    prelude::*,
};
use iroha_genesis::{GenesisBuilder, GenesisTopologyEntry, RawGenesisTransaction};

use super::{
    ConsensusPolicy, build_line_from_env, ensure_npos_parameters, generate::ConsensusModeArg,
    validate_consensus_mode_for_line,
};
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

const DEFAULT_NPOS_BOOTSTRAP_DOMAIN: &str = "nexus";
const DEFAULT_NPOS_BOOTSTRAP_IVM_DOMAIN: &str = "ivm";
const DEFAULT_NPOS_BOOTSTRAP_STAKE_ASSET_ID: &str = "xor#nexus";
const DEFAULT_NPOS_BOOTSTRAP_STAKE_AMOUNT: u64 = 10_000;

struct BootstrapRegistrations {
    domains: BTreeSet<DomainId>,
    accounts: BTreeSet<AccountId>,
    asset_defs: BTreeSet<AssetDefinitionId>,
}

impl BootstrapRegistrations {
    fn from_manifest(manifest: &RawGenesisTransaction) -> Self {
        let mut domains = BTreeSet::new();
        let mut accounts = BTreeSet::new();
        let mut asset_defs = BTreeSet::new();
        for instruction in manifest.instructions() {
            if let Some(register) = instruction.as_any().downcast_ref::<Register<Domain>>() {
                domains.insert(register.object.id.clone());
                continue;
            }
            if let Some(register) = instruction.as_any().downcast_ref::<Register<Account>>() {
                accounts.insert(register.object.id.clone());
                continue;
            }
            if let Some(register) = instruction
                .as_any()
                .downcast_ref::<Register<AssetDefinition>>()
            {
                asset_defs.insert(register.object.id.clone());
            }
        }
        Self {
            domains,
            accounts,
            asset_defs,
        }
    }
}

fn manifest_has_npos_bootstrap(manifest: &RawGenesisTransaction) -> bool {
    manifest.instructions().any(|instruction| {
        instruction
            .as_any()
            .downcast_ref::<RegisterPublicLaneValidator>()
            .is_some()
            || instruction
                .as_any()
                .downcast_ref::<ActivatePublicLaneValidator>()
                .is_some()
    })
}

fn collect_topology_peers(manifest: &RawGenesisTransaction) -> Vec<PeerId> {
    let mut seen = BTreeSet::new();
    let mut peers = Vec::new();
    for tx in manifest.transactions() {
        for entry in tx.topology() {
            if seen.insert(entry.peer.clone()) {
                peers.push(entry.peer.clone());
            }
        }
    }
    peers
}

fn append_npos_bootstrap(
    builder: GenesisBuilder,
    registrations: &mut BootstrapRegistrations,
    topology: &[PeerId],
    escrow_account_id: &AccountId,
) -> Result<GenesisBuilder, color_eyre::eyre::Error> {
    if topology.is_empty() {
        return Ok(builder);
    }

    let nexus_domain: DomainId = DEFAULT_NPOS_BOOTSTRAP_DOMAIN.parse()?;
    let ivm_domain: DomainId = escrow_account_id.domain().clone();
    let stake_asset_id: AssetDefinitionId = DEFAULT_NPOS_BOOTSTRAP_STAKE_ASSET_ID.parse()?;

    let mut builder = builder.next_transaction();
    if !registrations.domains.contains(&nexus_domain) {
        builder = builder.append_instruction(Register::domain(Domain::new(nexus_domain.clone())));
        registrations.domains.insert(nexus_domain.clone());
    }
    if !registrations.domains.contains(&ivm_domain) {
        builder = builder.append_instruction(Register::domain(Domain::new(ivm_domain.clone())));
        registrations.domains.insert(ivm_domain.clone());
    }
    if !registrations.accounts.contains(escrow_account_id) {
        builder =
            builder.append_instruction(Register::account(Account::new(escrow_account_id.clone())));
        registrations.accounts.insert(escrow_account_id.clone());
    }
    if !registrations.asset_defs.contains(&stake_asset_id) {
        let definition = AssetDefinition::new(stake_asset_id.clone(), NumericSpec::default())
            .with_metadata(Metadata::default());
        builder = builder.append_instruction(Register::asset_definition(definition));
        registrations.asset_defs.insert(stake_asset_id.clone());
    }

    for peer in topology {
        let validator_id = AccountId::new(nexus_domain.clone(), peer.public_key().clone());
        if !registrations.accounts.contains(&validator_id) {
            builder =
                builder.append_instruction(Register::account(Account::new(validator_id.clone())));
            registrations.accounts.insert(validator_id.clone());
        }
        builder = builder.append_instruction(Mint::asset_numeric(
            DEFAULT_NPOS_BOOTSTRAP_STAKE_AMOUNT,
            AssetId::new(stake_asset_id.clone(), validator_id.clone()),
        ));
        builder = builder.append_instruction(RegisterPublicLaneValidator {
            lane_id: LaneId::SINGLE,
            validator: validator_id.clone(),
            stake_account: validator_id.clone(),
            initial_stake: Numeric::from(DEFAULT_NPOS_BOOTSTRAP_STAKE_AMOUNT),
            metadata: Metadata::default(),
        });
        builder = builder.append_instruction(ActivatePublicLaneValidator {
            lane_id: LaneId::SINGLE,
            validator: validator_id,
        });
    }

    Ok(builder)
}

impl<T: Write> RunArgs<T> for Args {
    #[allow(clippy::too_many_lines)]
    fn run(self, writer: &mut BufWriter<T>) -> Outcome {
        tui::status("Signing genesis manifest");
        if let Some(path) = self.out_file.as_deref() {
            reject_legacy_scale_out_file(path)?;
        }
        let build_line = build_line_from_env();
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
        let consensus_mode_override = self.consensus_mode.map(SumeragiConsensusMode::from);
        let next_consensus_mode = self.next_consensus_mode.map(SumeragiConsensusMode::from);

        let mut genesis = RawGenesisTransaction::from_path(&self.genesis_file)?;
        let manifest_consensus_mode = genesis.consensus_mode().ok_or_else(|| {
            eyre!(
                "genesis manifest missing consensus_mode; regenerate with `kagami genesis generate --consensus-mode <mode>`"
            )
        })?;
        let consensus_mode = consensus_mode_override.unwrap_or(manifest_consensus_mode);
        let params = genesis.effective_parameters();
        let staged_next_mode = params.sumeragi().next_mode();
        let staged_activation_height = params.sumeragi().mode_activation_height();
        if build_line.is_iroha3() {
            validate_consensus_mode_for_line(
                build_line,
                consensus_mode,
                next_consensus_mode,
                ConsensusPolicy::Any,
            )?;
            if staged_next_mode.is_some() || staged_activation_height.is_some() {
                return Err(eyre!(
                    "Iroha3 does not support staged consensus cutovers; remove `next_mode` and `mode_activation_height` from genesis"
                ));
            }
        }
        if self.topology.is_some() {
            genesis = genesis.clear_topology();
        }
        if matches!(consensus_mode, SumeragiConsensusMode::Npos)
            || matches!(next_consensus_mode, Some(SumeragiConsensusMode::Npos))
        {
            ensure_npos_parameters(&genesis)?;
        }
        let topology_override = if let Some(raw) = self.topology.as_deref() {
            Some(norito::json::from_str::<Vec<PeerId>>(raw).wrap_err("parse --topology JSON")?)
        } else {
            None
        };
        let uses_npos = matches!(consensus_mode, SumeragiConsensusMode::Npos)
            || matches!(next_consensus_mode, Some(SumeragiConsensusMode::Npos));
        let topology_peers = if uses_npos {
            topology_override
                .clone()
                .unwrap_or_else(|| collect_topology_peers(&genesis))
        } else {
            Vec::new()
        };
        let needs_npos_bootstrap =
            uses_npos && !manifest_has_npos_bootstrap(&genesis) && !topology_peers.is_empty();
        let mut bootstrap_registrations = if needs_npos_bootstrap {
            BootstrapRegistrations::from_manifest(&genesis)
        } else {
            BootstrapRegistrations {
                domains: BTreeSet::new(),
                accounts: BTreeSet::new(),
                asset_defs: BTreeSet::new(),
            }
        };
        let mut builder = genesis.into_builder();

        if self.topology.is_none() && !self.peer_pops.is_empty() {
            return Err(eyre!(
                "--peer-pop requires --topology to align PoPs with peers"
            ));
        }

        if let Some(topology) = topology_override.as_ref() {
            // Put topology into a dedicated transaction so it remains separate
            // from other genesis instructions.
            let entries = build_topology_entries(topology, &self.peer_pops)?;
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
        if needs_npos_bootstrap {
            let ivm_domain: DomainId = DEFAULT_NPOS_BOOTSTRAP_IVM_DOMAIN.parse()?;
            let escrow_account_id =
                AccountId::new(ivm_domain, genesis_key_pair.public_key().clone());
            builder = append_npos_bootstrap(
                builder,
                &mut bootstrap_registrations,
                &topology_peers,
                &escrow_account_id,
            )?;
        }
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

fn reject_legacy_scale_out_file(path: &Path) -> Result<(), color_eyre::eyre::Error> {
    let Some(ext) = path.extension().and_then(|ext| ext.to_str()) else {
        return Ok(());
    };
    if !ext.eq_ignore_ascii_case("scale") {
        return Ok(());
    }

    Err(eyre!(
        "refusing to write `{}`: `.scale` is a legacy extension; kagami writes Norito wire format, use `.nrt` (e.g. genesis.signed.nrt)",
        path.display()
    ))
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
        block::decode_framed_signed_block,
        isi::staking::RegisterPublicLaneValidator,
        parameter::{
            Parameter,
            system::{SumeragiConsensusMode, SumeragiNposParameters, SumeragiParameter},
        },
        transaction::Executable,
    };
    use iroha_genesis::{GenesisBuilder, GenesisTopologyEntry};

    use super::*;

    #[test]
    fn peer_pops_without_topology_is_rejected() {
        let args = Args {
            genesis_file: npos_genesis_file(),
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
            genesis_file: npos_genesis_file(),
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
            entries[0].peer, peer_a,
            "entries should respect topology order"
        );
        assert_eq!(
            entries[1].peer, peer_b,
            "entries should respect topology order"
        );
    }

    #[test]
    fn out_file_scale_extension_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("genesis.scale");
        let args = Args {
            genesis_file: npos_genesis_file(),
            out_file: Some(path),
            topology: None,
            peer_pops: Vec::new(),
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
            .expect_err("writing a .scale out_file should be rejected");
        assert!(
            err.to_string().contains("legacy extension"),
            "unexpected error: {err}"
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
        let args = Args {
            genesis_file: npos_genesis_file(),
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
        let args = Args {
            genesis_file: npos_genesis_file(),
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
    fn sign_requires_consensus_mode_in_manifest() {
        let args = Args {
            genesis_file: legacy_genesis_file_missing_consensus_mode(),
            out_file: None,
            topology: None,
            peer_pops: Vec::new(),
            private_key: Some(test_private_key_hex()),
            seed: None,
            algorithm: Algorithm::Ed25519,
            consensus_mode: None,
            next_consensus_mode: None,
            mode_activation_height: None,
        };

        let mut writer = BufWriter::new(Vec::new());
        let err = args
            .run(&mut writer)
            .expect_err("missing consensus_mode should be rejected");
        assert!(
            err.to_string().contains("consensus_mode"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn sign_rejects_missing_consensus_mode_even_with_override() {
        let args = Args {
            genesis_file: legacy_genesis_file_missing_consensus_mode(),
            out_file: None,
            topology: None,
            peer_pops: Vec::new(),
            private_key: Some(test_private_key_hex()),
            seed: None,
            algorithm: Algorithm::Ed25519,
            consensus_mode: Some(ConsensusModeArg::Permissioned),
            next_consensus_mode: None,
            mode_activation_height: None,
        };

        let mut writer = BufWriter::new(Vec::new());
        let err = args
            .run(&mut writer)
            .expect_err("missing consensus_mode should be rejected");
        assert!(
            err.to_string().contains("consensus_mode"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn missing_pops_fail_when_topology_provided() {
        let genesis_file = npos_genesis_file();
        let peer_a = PeerId::new(CryptoKeyPair::random().public_key().clone());
        let peer_b = PeerId::new(CryptoKeyPair::random().public_key().clone());
        let topology_json = norito::json::to_json(&vec![peer_a.clone(), peer_b]).unwrap();

        // Provide PoP only for peer_a to trigger the missing-pop validation.
        let args = Args {
            genesis_file,
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
    fn topology_override_replaces_existing_entries() {
        use iroha_data_model::isi::register::RegisterBox;

        let existing_kp = CryptoKeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let existing_peer = PeerId::new(existing_kp.public_key().clone());
        let existing_pop = iroha_crypto::bls_normal_pop_prove(existing_kp.private_key())
            .expect("generate BLS PoP");
        let genesis_file = tempfile::NamedTempFile::new().expect("create temp genesis file");
        let manifest = GenesisBuilder::new_without_executor(
            ChainId::from("topology-override"),
            PathBuf::from("."),
        )
        .append_parameter(Parameter::Custom(
            SumeragiNposParameters::default().into_custom_parameter(),
        ))
        .set_topology(vec![GenesisTopologyEntry::new(existing_peer, existing_pop)])
        .build_raw()
        .with_consensus_mode(SumeragiConsensusMode::Npos);
        let json = norito::json::to_json_pretty(&manifest).expect("serialize genesis manifest");
        fs::write(genesis_file.path(), json).expect("write genesis json");

        let new_kp = CryptoKeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let new_peer = PeerId::new(new_kp.public_key().clone());
        let topology_json = norito::json::to_json(&vec![new_peer.clone()]).unwrap();

        let args = Args {
            genesis_file: genesis_file.path().to_path_buf(),
            out_file: None,
            topology: Some(topology_json),
            peer_pops: vec![format!("{}=01", new_peer.public_key())],
            private_key: Some(test_private_key_hex()),
            seed: None,
            algorithm: Algorithm::Ed25519,
            consensus_mode: None,
            next_consensus_mode: None,
            mode_activation_height: None,
        };

        let mut writer = BufWriter::new(Vec::new());
        args.run(&mut writer).expect("sign should succeed");
        writer.flush().expect("flush output");
        let bytes = writer.into_inner().expect("extract buffer");
        let block = decode_framed_signed_block(&bytes).expect("decode signed block");

        let mut registered_peers = Vec::new();
        for tx in block.external_transactions() {
            if let Executable::Instructions(instructions) = tx.instructions() {
                for instr in instructions {
                    if let Some(RegisterBox::Peer(register)) =
                        instr.as_any().downcast_ref::<RegisterBox>()
                    {
                        registered_peers.push(register.peer.clone());
                    }
                }
            }
        }

        assert_eq!(
            registered_peers,
            vec![new_peer],
            "expected topology override to replace existing entries"
        );
    }

    #[test]
    fn sign_auto_bootstraps_npos_validators_for_topology() {
        let peer = PeerId::new(
            CryptoKeyPair::random_with_algorithm(Algorithm::BlsNormal)
                .public_key()
                .clone(),
        );
        let topology_json = norito::json::to_json(&vec![peer.clone()]).unwrap();
        let args = Args {
            genesis_file: npos_genesis_file(),
            out_file: None,
            topology: Some(topology_json),
            peer_pops: vec![format!("{}=00", peer.public_key())],
            private_key: Some(test_private_key_hex()),
            seed: None,
            algorithm: Algorithm::Ed25519,
            consensus_mode: None,
            next_consensus_mode: None,
            mode_activation_height: None,
        };

        let mut writer = BufWriter::new(Vec::new());
        args.run(&mut writer).expect("sign should succeed");
        writer.flush().expect("flush output");
        let bytes = writer.into_inner().expect("extract buffer");
        let block = decode_framed_signed_block(&bytes).expect("decode signed block");

        let mut validators = std::collections::BTreeSet::new();
        for tx in block.external_transactions() {
            if let Executable::Instructions(instructions) = tx.instructions() {
                for instr in instructions {
                    if let Some(register) =
                        instr.as_any().downcast_ref::<RegisterPublicLaneValidator>()
                    {
                        validators.insert(register.validator.clone());
                    }
                }
            }
        }

        let nexus_domain: DomainId = DEFAULT_NPOS_BOOTSTRAP_DOMAIN
            .parse()
            .expect("parse default NPoS domain");
        let mut expected = std::collections::BTreeSet::new();
        expected.insert(AccountId::new(nexus_domain, peer.public_key().clone()));
        assert_eq!(
            validators, expected,
            "expected NPoS bootstrap to register topology validators"
        );
    }

    #[test]
    fn mode_activation_requires_next_mode_flag() {
        let args = Args {
            genesis_file: npos_genesis_file(),
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
    fn next_consensus_mode_rejected_on_iroha3() {
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
            .expect_err("Iroha3 should reject staged consensus cutovers");
        assert!(
            err.to_string().contains("staged consensus cutovers"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn npos_consensus_mode_requires_npos_parameters() {
        let args = Args {
            genesis_file: minimal_genesis_file(),
            out_file: None,
            topology: None,
            peer_pops: Vec::new(),
            private_key: Some(test_private_key_hex()),
            seed: None,
            algorithm: Algorithm::Ed25519,
            consensus_mode: Some(ConsensusModeArg::Npos),
            next_consensus_mode: None,
            mode_activation_height: None,
        };

        let mut writer = BufWriter::new(Vec::new());
        let err = args
            .run(&mut writer)
            .expect_err("NPoS consensus should require NPoS parameters");
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
            next_consensus_mode: None,
            mode_activation_height: None,
        };

        let mut writer = BufWriter::new(Vec::new());
        args.run(&mut writer)
            .expect("NPoS genesis with parameters should sign");
    }

    #[test]
    fn sign_accepts_permissioned_on_iroha3() {
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
            mode_activation_height: None,
        };

        let mut writer = BufWriter::new(Vec::new());
        args.run(&mut writer)
            .expect("permissioned genesis should be allowed on Iroha3");
    }

    #[test]
    fn sign_rejects_staged_cutover_on_iroha3() {
        let args = Args {
            genesis_file: staged_npos_genesis_file(),
            out_file: None,
            topology: None,
            peer_pops: Vec::new(),
            private_key: Some(test_private_key_hex()),
            seed: None,
            algorithm: Algorithm::Ed25519,
            consensus_mode: None,
            next_consensus_mode: None,
            mode_activation_height: None,
        };

        let mut writer = BufWriter::new(Vec::new());
        let err = args
            .run(&mut writer)
            .expect_err("staged cutover should be rejected on Iroha3");
        assert!(
            err.to_string().contains("staged consensus cutovers"),
            "unexpected error: {err}"
        );
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
            "consensus_mode": "Permissioned",
            "transactions": [
                {}
            ]
        }"#;
        write!(genesis_file, "{genesis_json}").expect("write genesis json");
        let (_file, path) = genesis_file.keep().expect("persist temp genesis");
        path
    }

    fn legacy_genesis_file_missing_consensus_mode() -> PathBuf {
        let mut genesis_file = tempfile::Builder::new()
            .prefix("kagami-genesis-legacy-")
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
                .build_raw()
                .with_consensus_mode(SumeragiConsensusMode::Npos);
        let json = norito::json::to_json_pretty(&manifest).expect("serialize genesis manifest");
        fs::write(genesis_file.path(), json).expect("write genesis json");
        let (_file, path) = genesis_file.keep().expect("persist temp genesis");
        path
    }

    fn staged_npos_genesis_file() -> PathBuf {
        let genesis_file = tempfile::Builder::new()
            .prefix("kagami-npos-staged-")
            .tempfile()
            .expect("create temp genesis file");
        let manifest =
            GenesisBuilder::new_without_executor(ChainId::from("npos-staged"), PathBuf::from("."))
                .append_parameter(Parameter::Custom(
                    SumeragiNposParameters::default().into_custom_parameter(),
                ))
                .append_parameter(Parameter::Sumeragi(SumeragiParameter::NextMode(
                    SumeragiConsensusMode::Npos,
                )))
                .append_parameter(Parameter::Sumeragi(
                    SumeragiParameter::ModeActivationHeight(5),
                ))
                .build_raw()
                .with_consensus_mode(SumeragiConsensusMode::Npos);
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
