//! Generate canned Kagami profile bundles (genesis + PoPs + snippets) for Iroha 3 profiles.

use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use blake2::{Blake2b512, digest::Digest};
use iroha_crypto::{Algorithm, ExposedPrivateKey, Hash, KeyPair};
use iroha_data_model::{
    account::AccountId,
    asset::AssetDefinitionId,
    block::{consensus_v2::is_valid_committee_size, decode_framed_signed_block},
    isi::SetParameter,
    parameter::{
        Parameter,
        system::{ConsensusHandshakeMetadata, SumeragiConsensusMode, consensus_metadata},
    },
    peer::PeerId,
    transaction::{Executable, TransactionDomain},
};
use iroha_genesis::{GenesisTopologyEntry, RawGenesisTransaction};
use iroha_primitives::addr::{SocketAddr, SocketAddrV4};
use norito::json;
use sha2::Sha256;

use crate::workspace_root;

#[derive(Debug, Clone)]
pub(crate) struct KagamiProfileOptions {
    pub output: PathBuf,
    pub profiles: Vec<String>,
    pub kagami_override: Option<PathBuf>,
    pub nexus_xor_asset_definition_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ProfileSpec {
    slug: &'static str,
    profile_flag: &'static str,
    chain_id: &'static str,
    chain_discriminant: Option<u16>,
    min_peers: usize,
    requires_seed: bool,
}

impl ProfileSpec {
    fn vrf_seed_hex(&self) -> String {
        hex::encode_upper(Hash::new(self.chain_id).as_ref())
    }
}

#[derive(Debug, Clone)]
struct PeerMaterial {
    peer_id: PeerId,
    address: String,
    public_key: String,
    private_key: String,
    soranet_transport_public_key: String,
    soranet_transport_private_key: String,
    streaming_public_key: String,
    streaming_private_key: String,
    pop: Vec<u8>,
    pop_hex: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PrivateKeyRendering {
    InlineStaging,
    RuntimeFiles,
}

fn published_private_key_rendering(spec: &ProfileSpec) -> PrivateKeyRendering {
    if spec.slug == "iroha3-dev" {
        PrivateKeyRendering::InlineStaging
    } else {
        PrivateKeyRendering::RuntimeFiles
    }
}

struct StagedGenesis {
    manifest: RawGenesisTransaction,
    signed_wire: Vec<u8>,
    expected_hash: String,
}

type AnyResult<T> = Result<T, Box<dyn Error>>;
const DEFAULT_TORII_MAX_CONTENT_LEN: u64 =
    iroha_config::parameters::defaults::torii::MAX_CONTENT_LEN.0;
const PROFILE_GENESIS_CREATION_TIME_MS: u64 = 1_700_000_000_000;
const GENESIS_EXPECTED_HASH_PLACEHOLDER: &str = "REPLACE_WITH_GENESIS_EXPECTED_HASH";
const NEXUS_XOR_ASSET_DEFINITION_ID_REQUIRED: &str =
    "iroha3-nexus profile generation requires --nexus-xor-asset-definition-id <BASE58>";

fn format_toml_integer_u64(value: u64) -> String {
    let digits = value.to_string();
    let mut reversed = String::with_capacity(digits.len() + digits.len() / 3);
    for (idx, ch) in digits.chars().rev().enumerate() {
        if idx != 0 && idx % 3 == 0 {
            reversed.push('_');
        }
        reversed.push(ch);
    }
    reversed.chars().rev().collect()
}

fn account_literal_for_chain_discriminant(
    account_id: &iroha_data_model::account::AccountId,
    chain_discriminant: u16,
) -> String {
    account_id
        .to_i105_for_discriminant(chain_discriminant)
        .expect("known governance account id must render for the requested chain discriminant")
}

fn account_literal_string_for_chain_discriminant(raw: &str, chain_discriminant: u16) -> String {
    let account_id = iroha_data_model::account::AccountId::parse_encoded(raw)
        .map(iroha_data_model::account::ParsedAccountId::into_account_id)
        .expect("known account literal must parse");
    account_literal_for_chain_discriminant(&account_id, chain_discriminant)
}

pub(crate) fn generate(options: KagamiProfileOptions) -> AnyResult<()> {
    let specs = resolve_requested_profiles(&options.profiles)?;
    preflight_required_profile_inputs(&specs, options.nexus_xor_asset_definition_id.as_deref())?;
    let kagami_bin = resolve_kagami_path(options.kagami_override.as_deref())?;
    fs::create_dir_all(&options.output)?;

    for spec in specs {
        write_profile_bundle(
            &spec,
            &kagami_bin,
            &options.output,
            options.nexus_xor_asset_definition_id.as_deref(),
        )?;
    }

    Ok(())
}

fn preflight_required_profile_inputs(
    specs: &[ProfileSpec],
    nexus_xor_asset_definition_id: Option<&str>,
) -> AnyResult<()> {
    if specs.iter().any(|spec| spec.profile_flag == "iroha3-nexus") {
        let Some(asset_definition_id) = nexus_xor_asset_definition_id else {
            return Err(NEXUS_XOR_ASSET_DEFINITION_ID_REQUIRED.into());
        };
        AssetDefinitionId::parse_address_literal(asset_definition_id).map_err(|err| {
            format!(
                "invalid --nexus-xor-asset-definition-id `{asset_definition_id}`: {err}; \
                 expected a canonical unprefixed Base58 asset definition id"
            )
        })?;
    }
    Ok(())
}

fn resolve_requested_profiles(names: &[String]) -> AnyResult<Vec<ProfileSpec>> {
    if names.is_empty() {
        return Ok(PROFILES.to_vec());
    }
    let mut out = Vec::new();
    for name in names {
        if name == "all" {
            return Ok(PROFILES.to_vec());
        }
        let Some(spec) = PROFILES.iter().find(|spec| spec.slug == name.as_str()) else {
            return Err(format!(
                "unknown profile `{name}` (expected one of all,{})",
                profile_slug_list()
            )
            .into());
        };
        out.push(*spec);
    }
    Ok(out)
}

fn write_profile_bundle(
    spec: &ProfileSpec,
    kagami_bin: &Path,
    output_root: &Path,
    nexus_xor_asset_definition_id: Option<&str>,
) -> AnyResult<()> {
    fs::create_dir_all(output_root)?;
    let staging = tempfile::Builder::new()
        .prefix(&format!(".{}-staging-", spec.slug))
        .tempdir_in(output_root)?;
    let bundle_root = staging.path().to_path_buf();

    let genesis_key =
        deterministic_keypair(&format!("{}-genesis-key", spec.slug), Algorithm::Ed25519)?;
    let genesis_json = generate_genesis(
        spec,
        kagami_bin,
        genesis_key.public_key(),
        &bundle_root,
        nexus_xor_asset_definition_id,
    )?;
    let peers = build_peers(spec)?;
    let patched_genesis = inject_topology(genesis_json, &peers)?;
    let genesis_path = bundle_root.join("genesis.json");
    write_json(&genesis_path, &patched_genesis)?;

    write_peer_configs(
        spec,
        &peers,
        genesis_key.public_key(),
        &bundle_root,
        GENESIS_EXPECTED_HASH_PLACEHOLDER,
        PrivateKeyRendering::InlineStaging,
    )?;
    let config_path = bundle_root.join(peer_config_file_name(0));
    let staged_genesis =
        bind_staged_context(spec, kagami_bin, &genesis_path, &config_path, &genesis_key)?;
    write_json(&genesis_path, &staged_genesis.manifest)?;
    fs::write(
        bundle_root.join("genesis.signed.nrt"),
        staged_genesis.signed_wire,
    )?;
    fs::write(
        bundle_root.join("genesis.public_key"),
        format!("{}\n", genesis_key.public_key()),
    )?;
    fs::write(
        bundle_root.join("genesis.expected_hash"),
        format!("{}\n", staged_genesis.expected_hash),
    )?;
    write_peer_configs(
        spec,
        &peers,
        genesis_key.public_key(),
        &bundle_root,
        &staged_genesis.expected_hash,
        published_private_key_rendering(spec),
    )?;

    let vrf_seed_hex = if spec.requires_seed {
        Some(spec.vrf_seed_hex())
    } else {
        None
    };
    let verify_out = run_verify(spec, kagami_bin, &genesis_path, vrf_seed_hex.as_deref())?;
    fs::write(bundle_root.join("verify.txt"), verify_out)?;

    if spec.slug == "iroha3-taira" {
        fs::write(
            bundle_root.join("sorafs_sites.json"),
            b"{\n  \"version\": 1,\n  \"sites\": []\n}\n",
        )?;
    }

    let compose = render_docker_compose(spec, &peers);
    fs::write(bundle_root.join("docker-compose.yml"), compose)?;

    let readme = render_readme(
        spec,
        &peers,
        genesis_key.public_key(),
        vrf_seed_hex.as_deref(),
        nexus_xor_asset_definition_id,
    );
    fs::write(bundle_root.join("README.md"), readme)?;

    publish_profile_bundle(staging, &output_root.join(spec.slug))?;
    Ok(())
}

fn publish_profile_bundle(staging: tempfile::TempDir, destination: &Path) -> AnyResult<()> {
    if !destination.exists() {
        let staged_path = staging.keep();
        fs::rename(&staged_path, destination)?;
        return Ok(());
    }
    let name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("profile destination filename is not UTF-8")?;
    let backup = destination.with_file_name(format!(".{name}.backup-{}", std::process::id()));
    if backup.exists() {
        return Err(format!(
            "refusing to replace {} while recovery backup {} exists",
            destination.display(),
            backup.display()
        )
        .into());
    }
    fs::rename(destination, &backup)?;
    let staged_path = staging.keep();
    if let Err(publish_error) = fs::rename(&staged_path, destination) {
        let restore = fs::rename(&backup, destination);
        let _ = fs::remove_dir_all(&staged_path);
        return match restore {
            Ok(()) => Err(format!(
                "failed to publish validated profile bundle {}: {publish_error}; restored previous bundle",
                destination.display()
            )
            .into()),
            Err(restore_error) => Err(format!(
                "failed to publish validated profile bundle {}: {publish_error}; previous bundle remains at {} because restoration failed: {restore_error}",
                destination.display(),
                backup.display()
            )
            .into()),
        };
    }
    fs::remove_dir_all(&backup).map_err(|err| {
        format!(
            "published {} but failed to remove recovery backup {}: {err}",
            destination.display(),
            backup.display()
        )
        .into()
    })
}

fn generate_genesis(
    spec: &ProfileSpec,
    kagami_bin: &Path,
    genesis_public_key: &iroha_crypto::PublicKey,
    workdir: &Path,
    nexus_xor_asset_definition_id: Option<&str>,
) -> AnyResult<RawGenesisTransaction> {
    let mut command = Command::new(kagami_bin);
    command.args([
        "genesis",
        "generate",
        "--profile",
        spec.profile_flag,
        "--ivm-dir",
        ".",
        "--genesis-public-key",
        &genesis_public_key.to_string(),
        "--consensus-mode",
        "npos",
    ]);

    if spec.requires_seed {
        command.args(["--vrf-seed-hex", &spec.vrf_seed_hex()]);
    }
    if spec.profile_flag == "iroha3-nexus" {
        let Some(asset_definition_id) = nexus_xor_asset_definition_id else {
            return Err(NEXUS_XOR_ASSET_DEFINITION_ID_REQUIRED.into());
        };
        command.args(["--xor-asset-definition-id", asset_definition_id]);
    }

    let output = command
        .current_dir(workdir)
        .output()
        .map_err(|err| format!("failed to run kagami: {err}"))?;
    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "kagami genesis generate failed (status {:?}):\nstdout:\n{}\nstderr:\n{}",
            output.status.code(),
            stdout,
            stderr
        )
        .into());
    }

    json::from_slice(&output.stdout)
        .map_err(|err| format!("failed to parse genesis JSON: {err}").into())
}

fn inject_topology(
    manifest: RawGenesisTransaction,
    peers: &[PeerMaterial],
) -> AnyResult<RawGenesisTransaction> {
    let consensus_mode = manifest.consensus_mode();
    let topology: Vec<GenesisTopologyEntry> = peers
        .iter()
        .map(|peer| GenesisTopologyEntry::new(peer.peer_id.clone(), peer.pop.clone()))
        .collect();
    let manifest = manifest
        .into_builder()
        .set_topology(topology)
        .build_raw()
        .with_consensus_mode(consensus_mode)
        .with_consensus_meta();
    Ok(manifest)
}

fn write_json(path: &Path, value: &RawGenesisTransaction) -> AnyResult<()> {
    let mut rendered = json::to_json_pretty(value)?;
    rendered.push('\n');
    fs::write(path, rendered)?;
    Ok(())
}

fn bind_staged_context(
    spec: &ProfileSpec,
    kagami_bin: &Path,
    genesis_path: &Path,
    config_path: &Path,
    genesis_key: &KeyPair,
) -> AnyResult<StagedGenesis> {
    let workdir = genesis_path
        .parent()
        .ok_or_else(|| format!("{} genesis path has no parent", spec.slug))?;
    if config_path.parent() != Some(workdir) {
        return Err(format!(
            "{} genesis and config paths must share one staging directory",
            spec.slug
        )
        .into());
    }
    let genesis_file = genesis_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("{} genesis filename is not UTF-8", spec.slug))?;
    let config_file = config_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("{} config filename is not UTF-8", spec.slug))?;
    let private_key_hex = hex::encode(genesis_key.private_key().to_bytes().1);
    let expected_public_key = genesis_key.public_key().to_string();
    let creation_time_ms = PROFILE_GENESIS_CREATION_TIME_MS.to_string();
    let output = Command::new(kagami_bin)
        .args([
            "genesis",
            "sign",
            genesis_file,
            "--private-key",
            &private_key_hex,
            "--expected-public-key",
            &expected_public_key,
            "--creation-time-ms",
            &creation_time_ms,
            "--config",
            config_file,
            "--bound-manifest-out",
            genesis_file,
        ])
        .current_dir(workdir)
        .output()
        .map_err(|err| format!("failed to stage {} genesis: {err}", spec.slug))?;
    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "kagami genesis sign failed while staging {} (status {:?}):\nstdout:\n{}\nstderr:\n{}",
            spec.slug,
            output.status.code(),
            stdout,
            stderr
        )
        .into());
    }

    let signed_wire = output.stdout;
    let block = decode_framed_signed_block(&signed_wire)
        .map_err(|err| format!("failed to decode staged {} genesis: {err}", spec.slug))?;
    let canonical_wire = block
        .encode_wire()
        .map_err(|err| format!("failed to re-encode staged {} genesis: {err}", spec.slug))?;
    if canonical_wire != signed_wire {
        return Err(format!(
            "staged {} genesis is not canonical framed Norito",
            spec.slug
        )
        .into());
    }
    {
        let mut signatures = block.signatures();
        let signature = signatures
            .next()
            .ok_or_else(|| format!("staged {} genesis has no block signature", spec.slug))?;
        if signature.index() != 0 || signatures.next().is_some() {
            return Err(format!(
                "staged {} genesis must have exactly one block signature at index 0",
                spec.slug
            )
            .into());
        }
        signature
            .signature()
            .verify_hash(genesis_key.public_key(), block.hash())
            .map_err(|err| {
                format!(
                    "staged {} genesis block signature does not match its configured signer: {err}",
                    spec.slug
                )
            })?;
    }
    for transaction in block.external_transactions() {
        transaction.verify_signature().map_err(|err| {
            format!(
                "staged {} genesis contains an invalid transaction signature: {err}",
                spec.slug
            )
        })?;
    }
    let mut metadata = None;
    for transaction in block.external_transactions() {
        if let Executable::Instructions(batch) = transaction.instructions() {
            for instruction in batch {
                if let Some(set_parameter) = instruction.as_any().downcast_ref::<SetParameter>()
                    && let Parameter::Custom(custom) = set_parameter.inner()
                    && custom.id() == &consensus_metadata::handshake_meta_id()
                {
                    let decoded = custom
                        .payload()
                        .try_into_any::<ConsensusHandshakeMetadata>()
                        .map_err(|err| {
                            format!(
                                "failed to decode staged {} consensus metadata: {err}",
                                spec.slug
                            )
                        })?;
                    if metadata.replace(decoded).is_some() {
                        return Err(format!(
                            "staged {} genesis contains duplicate consensus metadata",
                            spec.slug
                        )
                        .into());
                    }
                }
            }
        }
    }
    let metadata = metadata
        .ok_or_else(|| format!("staged {} genesis omitted consensus metadata", spec.slug))?;
    if metadata.mode != SumeragiConsensusMode::Npos {
        return Err(format!(
            "staged {} genesis reported mode {}, expected NPoS",
            spec.slug, metadata.mode
        )
        .into());
    }
    if metadata.wire_protocol_version
        != u32::from(iroha_data_model::block::consensus_v2::PROTOCOL_VERSION)
    {
        return Err(format!(
            "staged {} genesis advertised {}, expected protocol v{}",
            spec.slug,
            metadata.wire_protocol_version,
            iroha_data_model::block::consensus_v2::PROTOCOL_VERSION
        )
        .into());
    }

    let bound_manifest_bytes = fs::read(genesis_path)
        .map_err(|err| format!("failed to read bound {} genesis: {err}", spec.slug))?;
    let bound_manifest: RawGenesisTransaction = json::from_slice(&bound_manifest_bytes)
        .map_err(|err| format!("failed to parse bound {} genesis: {err}", spec.slug))?;
    if bound_manifest.consensus_fingerprint() != Some(metadata.consensus_fingerprint) {
        return Err(format!(
            "staged {} consensus fingerprint did not cover the bound Nexus/AMX context",
            spec.slug
        )
        .into());
    }
    let expected_batches = bound_manifest
        .clone()
        .parse()
        .map_err(|err| format!("failed to expand bound {} genesis: {err}", spec.slug))?;
    let actual_transactions = block.external_transactions().collect::<Vec<_>>();
    if expected_batches.len() != actual_transactions.len() {
        return Err(format!(
            "staged {} signed transaction count differs from its bound manifest",
            spec.slug
        )
        .into());
    }
    let genesis_account = AccountId::new(genesis_key.public_key().clone());
    for (index, (expected_batch, transaction)) in expected_batches
        .iter()
        .zip(&actual_transactions)
        .enumerate()
    {
        if transaction.domain() != &TransactionDomain::Genesis
            || transaction.authority() != &genesis_account
        {
            return Err(format!(
                "staged {} transaction {index} has the wrong genesis domain or authority",
                spec.slug
            )
            .into());
        }
        let Executable::Instructions(actual_batch) = transaction.instructions() else {
            return Err(format!(
                "staged {} transaction {index} is not an instruction batch",
                spec.slug
            )
            .into());
        };
        let expected = expected_batch
            .iter()
            .map(iroha_data_model::Encode::encode)
            .collect::<Vec<_>>();
        let actual = actual_batch
            .iter()
            .map(iroha_data_model::Encode::encode)
            .collect::<Vec<_>>();
        if expected != actual {
            return Err(format!(
                "staged {} transaction {index} differs from its bound manifest",
                spec.slug
            )
            .into());
        }
    }
    iroha_core::validate_genesis_block(&block, &genesis_account)
        .map_err(|err| format!("staged {} genesis failed full validation: {err}", spec.slug))?;
    Ok(StagedGenesis {
        manifest: bound_manifest,
        signed_wire,
        expected_hash: block.hash().to_string(),
    })
}

fn run_verify(
    spec: &ProfileSpec,
    kagami_bin: &Path,
    genesis_path: &Path,
    vrf_seed_hex: Option<&str>,
) -> AnyResult<String> {
    let workdir = genesis_path
        .parent()
        .ok_or_else(|| format!("{} genesis path has no parent", spec.slug))?;
    let genesis_file = genesis_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("{} genesis filename is not UTF-8", spec.slug))?;
    let mut command = Command::new(kagami_bin);
    command.args([
        "verify",
        "--profile",
        spec.profile_flag,
        "--genesis",
        genesis_file,
    ]);
    if let Some(seed) = vrf_seed_hex {
        command.args(["--vrf-seed-hex", seed]);
    }
    let output = command
        .current_dir(workdir)
        .output()
        .map_err(|err| format!("failed to run kagami verify: {err}"))?;
    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "kagami verify failed (status {:?}):\nstdout:\n{}\nstderr:\n{}",
            output.status.code(),
            stdout,
            stderr
        )
        .into());
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn render_config(
    spec: &ProfileSpec,
    peers: &[PeerMaterial],
    genesis_public_key: &iroha_crypto::PublicKey,
    genesis_expected_hash: &str,
) -> String {
    render_peer_config_with_private_keys(
        spec,
        peers,
        0,
        genesis_public_key,
        genesis_expected_hash,
        published_private_key_rendering(spec),
    )
}

fn render_peer_config(
    spec: &ProfileSpec,
    peers: &[PeerMaterial],
    peer_index: usize,
    genesis_public_key: &iroha_crypto::PublicKey,
    genesis_expected_hash: &str,
) -> String {
    render_peer_config_with_private_keys(
        spec,
        peers,
        peer_index,
        genesis_public_key,
        genesis_expected_hash,
        published_private_key_rendering(spec),
    )
}

fn render_peer_config_with_private_keys(
    spec: &ProfileSpec,
    peers: &[PeerMaterial],
    peer_index: usize,
    genesis_public_key: &iroha_crypto::PublicKey,
    genesis_expected_hash: &str,
    private_key_rendering: PrivateKeyRendering,
) -> String {
    let genesis_expected_hash = if genesis_expected_hash == GENESIS_EXPECTED_HASH_PLACEHOLDER {
        genesis_expected_hash.to_owned()
    } else {
        norito::literal::format("hash", &genesis_expected_hash.to_ascii_uppercase())
    };
    let genesis_identity_source = if genesis_expected_hash == GENESIS_EXPECTED_HASH_PLACEHOLDER
        && private_key_rendering == PrivateKeyRendering::RuntimeFiles
    {
        "expected_hash_file = \"/run/iroha/genesis.expected_hash\"".to_owned()
    } else {
        format!("expected_hash = \"{genesis_expected_hash}\"")
    };
    let node = peers
        .get(peer_index)
        .expect("peer config index must address signed topology");
    let (node_private_key, soranet_transport_private_key, streaming_private_key) =
        match private_key_rendering {
            PrivateKeyRendering::InlineStaging => (
                format!("private_key = \"{}\"", node.private_key),
                format!(
                    "soranet_transport_private_key = \"{}\"",
                    node.soranet_transport_private_key
                ),
                format!("identity_private_key = \"{}\"", node.streaming_private_key),
            ),
            PrivateKeyRendering::RuntimeFiles => {
                let prefix = format!("/run/secrets/iroha/{}-peer-{peer_index}", spec.slug);
                (
                    format!("private_key_file = \"{prefix}-validator-private-key\""),
                    format!(
                        "soranet_transport_private_key_file = \"{prefix}-soranet-private-key\""
                    ),
                    format!("identity_private_key_file = \"{prefix}-streaming-private-key\""),
                )
            }
        };
    let trusted_peers = peers
        .iter()
        .map(|peer| format!("  \"{}@{}\"", peer.public_key, peer.address))
        .collect::<Vec<_>>()
        .join(",\n");
    let trusted_peers_pop = peers
        .iter()
        .map(|peer| {
            format!(
                "  {{ public_key = \"{}\", pop_hex = \"{}\" }}",
                peer.public_key, peer.pop_hex
            )
        })
        .collect::<Vec<_>>()
        .join(",\n");
    let chain_discriminant = spec
        .chain_discriminant
        .map_or_else(String::new, |discriminant| {
            format!("chain_discriminant = {discriminant}\n")
        });
    let governance_overrides = spec
        .chain_discriminant
        .map_or_else(String::new, |discriminant| {
            let citizenship_escrow_account = account_literal_for_chain_discriminant(
                &iroha_config::parameters::defaults::governance::citizenship_escrow_account_id(),
                discriminant,
            );
            let bond_escrow_account = account_literal_for_chain_discriminant(
                &iroha_config::parameters::defaults::governance::bond_escrow_account_id(),
                discriminant,
            );
            let slash_receiver_account = account_literal_for_chain_discriminant(
                &iroha_config::parameters::defaults::governance::slash_receiver_account_id(),
                discriminant,
            );
            let telemetry_submitters =
                iroha_config::parameters::defaults::governance::sorafs_telemetry::submitters()
                    .into_iter()
                    .map(|literal| {
                        format!(
                            "\"{}\"",
                            account_literal_string_for_chain_discriminant(&literal, discriminant)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
            format!(
                r#"
[gov]
citizenship_escrow_account = "{citizenship_escrow_account}"
bond_escrow_account = "{bond_escrow_account}"
slash_receiver_account = "{slash_receiver_account}"
viral_incentive_pool_account = "{slash_receiver_account}"
viral_escrow_account = "{slash_receiver_account}"

[gov.sorafs_telemetry]
submitters = [{telemetry_submitters}]
"#,
            )
        });
    let torii_max_content_len = format_toml_integer_u64(DEFAULT_TORII_MAX_CONTENT_LEN);
    let sorafs_site_bindings = if spec.slug == "iroha3-taira" {
        r#"
[sorafs.gateway.site_bindings]
path = "/config/sorafs_sites.json"
max_bytes = 1048576
max_sites = 1024
"#
    } else {
        ""
    };
    let taira_nexus_overrides = if spec.slug == "iroha3-taira" {
        r#"
[nexus.fees]
fee_asset_id = "xor#universal"

[nexus.staking]
stake_asset_id = "xor#universal"
"#
    } else {
        ""
    };
    let taira_mcp_overrides = if spec.slug == "iroha3-taira" {
        r#"
[torii.mcp]
enabled = true
profile = "writer"
expose_operator_routes = false
allow_tool_prefixes = ["iroha."]
"#
    } else {
        ""
    };
    let max_transactions = if spec.slug == "iroha3-taira" {
        96
    } else {
        iroha_config::parameters::defaults::sumeragi::BLOCK_MAX_TRANSACTIONS.get()
    };
    let max_payload_bytes =
        iroha_config::parameters::defaults::sumeragi::BLOCK_MAX_PAYLOAD_BYTES.get();
    let authenticated_non_validator_sources =
        iroha_config::parameters::defaults::sumeragi::QUEUE_AUTHENTICATED_NON_VALIDATOR_SOURCE_CAPACITY
            .get();
    let body_source_bytes =
        iroha_config::parameters::defaults::sumeragi::QUEUE_BODY_SOURCE_BYTES.get();
    let source_partitions = peers
        .len()
        .checked_add(authenticated_non_validator_sources)
        .and_then(|count| count.checked_add(1))
        .expect("profile ingress source-partition count must fit usize");
    let body_bytes = source_partitions
        .checked_mul(body_source_bytes)
        .expect("profile aggregate body-ingress bytes must fit usize")
        .max(iroha_config::parameters::defaults::sumeragi::QUEUE_BODY_BYTES.get());
    let p2p_port = node
        .address
        .rsplit_once(':')
        .and_then(|(_, port)| port.parse::<u16>().ok())
        .expect("generated peer material must contain a valid IPv4 port");
    let network_address =
        SocketAddr::Ipv4(SocketAddrV4::from(([0, 0, 0, 0], p2p_port))).to_literal();
    let public_ip = node
        .address
        .rsplit_once(':')
        .and_then(|(ip, _)| ip.parse::<std::net::Ipv4Addr>().ok())
        .expect("generated peer material must contain a valid IPv4 address")
        .octets();
    let network_public_address =
        SocketAddr::Ipv4(SocketAddrV4::from((public_ip, p2p_port))).to_literal();
    let torii_port = 8080_u16
        .checked_add(u16::try_from(peer_index).expect("profile peer index must fit u16"))
        .expect("profile Torii port must fit u16");
    let torii_address =
        SocketAddr::Ipv4(SocketAddrV4::from(([0, 0, 0, 0], torii_port))).to_literal();
    format!(
        r#"# Sample config for {slug} (generated via cargo xtask kagami-profiles)
chain = "{chain}"
{chain_discriminant}public_key = "{node_pk}"
{node_private_key}
soranet_transport_public_key = "{soranet_transport_pk}"
{soranet_transport_private_key}

trusted_peers = [
{trusted_peers}
]
trusted_peers_pop = [
{trusted_peers_pop}
]

[sumeragi]
role = "validator"

[sumeragi.block]
max_transactions = {max_transactions}
max_payload_bytes = {max_payload_bytes}
proposal_queue_scan_multiplier = 4

[sumeragi.queues]
authenticated_non_validator_sources = {authenticated_non_validator_sources}
body_bytes = {body_bytes}
body_source_bytes = {body_source_bytes}

[network]
address = "{network_address}"
public_address = "{network_public_address}"

[torii]
address = "{torii_address}"
max_content_len = {torii_max_content_len}
{taira_mcp_overrides}

[streaming]
identity_public_key = "{stream_pub}"
{streaming_private_key}
{sorafs_site_bindings}

[nexus]
enabled = true
lane_count = 3
{taira_nexus_overrides}
{governance_overrides}

[genesis]
public_key = "{genesis_pk}"
file = "genesis.signed.nrt"
{genesis_identity_source}
"#,
        slug = spec.slug,
        chain = spec.chain_id,
        chain_discriminant = chain_discriminant,
        node_pk = node.public_key,
        node_private_key = node_private_key,
        soranet_transport_pk = node.soranet_transport_public_key,
        soranet_transport_private_key = soranet_transport_private_key,
        trusted_peers = trusted_peers,
        trusted_peers_pop = trusted_peers_pop,
        max_transactions = max_transactions,
        max_payload_bytes = max_payload_bytes,
        authenticated_non_validator_sources = authenticated_non_validator_sources,
        body_bytes = body_bytes,
        body_source_bytes = body_source_bytes,
        network_address = network_address,
        network_public_address = network_public_address,
        torii_address = torii_address,
        torii_max_content_len = torii_max_content_len,
        sorafs_site_bindings = sorafs_site_bindings,
        taira_nexus_overrides = taira_nexus_overrides,
        taira_mcp_overrides = taira_mcp_overrides,
        governance_overrides = governance_overrides,
        genesis_pk = genesis_public_key,
        genesis_identity_source = genesis_identity_source,
        stream_pub = node.streaming_public_key,
        streaming_private_key = streaming_private_key,
    )
}

fn write_peer_configs(
    spec: &ProfileSpec,
    peers: &[PeerMaterial],
    genesis_public_key: &iroha_crypto::PublicKey,
    bundle_root: &Path,
    genesis_expected_hash: &str,
    private_key_rendering: PrivateKeyRendering,
) -> AnyResult<()> {
    for peer_index in 0..peers.len() {
        let rendered = render_peer_config_with_private_keys(
            spec,
            peers,
            peer_index,
            genesis_public_key,
            genesis_expected_hash,
            private_key_rendering,
        );
        fs::write(
            bundle_root.join(peer_config_file_name(peer_index)),
            &rendered,
        )?;
        fs::write(bundle_root.join(format!("peer{peer_index}.toml")), rendered)?;
    }
    Ok(())
}

fn render_docker_compose(spec: &ProfileSpec, peers: &[PeerMaterial]) -> String {
    let runtime_secrets_volume =
        if published_private_key_rendering(spec) == PrivateKeyRendering::RuntimeFiles {
            "\n      - /run/secrets/iroha:/run/secrets/iroha:ro"
        } else {
            ""
        };
    let services = peers
        .iter()
        .enumerate()
        .map(|(peer_index, peer)| {
            let service = format!("iroha-{}-{peer_index}", spec.slug);
            let config_file = peer_config_file_name(peer_index);
            let command = r#"["iroha3d", "--sora", "--config", "/config/config.toml"]"#;
            let site_bindings_volume = if spec.slug == "iroha3-taira" {
                "\n      - ./sorafs_sites.json:/config/sorafs_sites.json:ro"
            } else {
                ""
            };
            let p2p_port = peer
                .address
                .rsplit_once(':')
                .map(|(_, port)| port)
                .expect("generated peer address must contain a port");
            let peer_ip = peer
                .address
                .rsplit_once(':')
                .map(|(ip, _)| ip)
                .expect("generated peer address must contain an IP");
            let torii_port = 8080_u16
                .checked_add(
                    u16::try_from(peer_index).expect("profile peer index must fit into u16"),
                )
                .expect("profile Torii port must fit into u16");
            format!(
                r#"  {service}:
    image: hyperledger/iroha:latest
    command: {command}
    volumes:
      - ./{config_file}:/config/config.toml:ro
      - ./genesis.json:/config/genesis.json:ro
      - ./genesis.signed.nrt:/config/genesis.signed.nrt:ro{site_bindings_volume}{runtime_secrets_volume}
    ports:
      - "{torii_port}:{torii_port}"
      - "{p2p_port}:{p2p_port}"
    networks:
      profile:
        ipv4_address: {peer_ip}
"#,
            )
        })
        .collect::<Vec<_>>()
        .join("");
    format!(
        r#"version: "3.9"
services:
{services}
networks:
  profile:
    ipam:
      config:
        - subnet: 172.28.0.0/24
"#,
    )
}

fn peer_config_file_name(peer_index: usize) -> String {
    if peer_index == 0 {
        "config.toml".to_owned()
    } else {
        format!("config-peer-{peer_index}.toml")
    }
}

fn render_readme(
    spec: &ProfileSpec,
    peers: &[PeerMaterial],
    genesis_public_key: &iroha_crypto::PublicKey,
    vrf_seed_hex: Option<&str>,
    nexus_xor_asset_definition_id: Option<&str>,
) -> String {
    let peer_rows = peers
        .iter()
        .enumerate()
        .map(|(idx, peer)| {
            format!(
                "- peer {idx}: public_key={pk} address={addr} pop_hex={pop}",
                idx = idx + 1,
                pk = peer.public_key,
                addr = peer.address,
                pop = peer.pop_hex
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let vrf_line = if let Some(seed) = vrf_seed_hex {
        format!("VRF seed (hex): {seed}")
    } else {
        "VRF seed: derived from chain id".to_string()
    };
    let verify_vrf_seed_arg = vrf_seed_hex
        .map(|seed| format!(" --vrf-seed-hex {seed}"))
        .unwrap_or_default();
    let chain_discriminant_line = spec.chain_discriminant.map_or_else(String::new, |value| {
        format!("- chain discriminant: {value}\n")
    });
    let site_bindings_file = if spec.slug == "iroha3-taira" {
        "- sorafs_sites.json — empty version-1 named-host binding document loaded, validated, and cached at Torii startup\n"
    } else {
        ""
    };
    let nexus_regeneration_arg = if spec.profile_flag == "iroha3-nexus" {
        format!(
            " --nexus-xor-asset-definition-id {}",
            nexus_xor_asset_definition_id.unwrap_or("<BASE58>")
        )
    } else {
        String::new()
    };
    let runtime_key_note = if published_private_key_rendering(spec)
        == PrivateKeyRendering::RuntimeFiles
    {
        "\nRuntime keys:\n- Validator, SoraNet transport, and streaming signing keys are not embedded. Provision the per-peer files named by each config under `/run/secrets/iroha` before starting a validator. The compose file mounts that host directory read-only and startup fails closed when a required file is absent.\n"
    } else {
        ""
    };

    format!(
        r#"# {slug} sample bundle

- chain id: {chain}
{chain_discriminant_line}
- {vrf_line}
- deterministic genesis creation-time base (ms): {creation_time_ms}
- genesis public key: {genesis_pk}
- peers:
{peer_rows}

Files:
- genesis.json — generated with `kagami genesis generate --profile {profile}`, patched with deterministic topology+PoPs, and rebound to the exact staged Nexus/AMX context through `kagami genesis sign`
- genesis.signed.nrt — canonical signed genesis wire artifact consumed by every validator
- genesis.public_key — canonical one-line verifier key for the signed genesis artifact
- genesis.expected_hash — canonical one-line independently provisioned signed-header hash
- verify.txt — stdout from `kagami verify --profile {profile} --genesis genesis.json{verify_vrf_seed_arg}`
- config.toml and config-peer-*.toml — compatibility names for the generated validator configs
- peer0.toml through peerN.toml — canonical prepared-bundle validator configs
{site_bindings_file}- docker-compose.yml — full validator committee mounting the shared genesis and per-peer configs
{runtime_key_note}

Regenerate:
- cargo xtask kagami-profiles --profile {profile}{nexus_regeneration_arg}
"#,
        slug = spec.slug,
        chain = spec.chain_id,
        chain_discriminant_line = chain_discriminant_line,
        vrf_line = vrf_line,
        creation_time_ms = PROFILE_GENESIS_CREATION_TIME_MS,
        genesis_pk = genesis_public_key,
        peer_rows = peer_rows,
        profile = spec.profile_flag,
        verify_vrf_seed_arg = verify_vrf_seed_arg,
        nexus_regeneration_arg = nexus_regeneration_arg,
        site_bindings_file = site_bindings_file,
        runtime_key_note = runtime_key_note,
    )
}

fn build_peers(spec: &ProfileSpec) -> AnyResult<Vec<PeerMaterial>> {
    if !is_valid_committee_size(spec.min_peers) {
        return Err(format!(
            "profile {} peer count {} is not an exact revision-4 `3f + 1` committee",
            spec.slug, spec.min_peers
        )
        .into());
    }
    (0..spec.min_peers)
        .map(|idx| {
            let seed = format!("{}-peer-{idx}", spec.slug);
            let kp = deterministic_keypair(&seed, Algorithm::BlsNormal)?;
            let streaming_kp = deterministic_keypair(
                &format!("{}-streaming-{idx}", spec.slug),
                Algorithm::Ed25519,
            )?;
            let soranet_transport_kp = deterministic_soranet_transport_keypair(spec, idx)?;
            let pop = iroha_crypto::bls_normal_pop_prove(kp.private_key()).map_err(|err| {
                format!("failed to generate deterministic BLS PoP for `{seed}`: {err}")
            })?;
            let peer_offset =
                u16::try_from(idx).map_err(|_| "profile peer index does not fit into u16")?;
            let port = 1337_u16
                .checked_add(peer_offset)
                .ok_or("profile P2P port does not fit into u16")?;
            let host_octet = 10_usize
                .checked_add(idx)
                .filter(|octet| *octet <= usize::from(u8::MAX))
                .ok_or("profile peer index does not fit into the compose subnet")?;
            let address = format!("172.28.0.{host_octet}:{port}");
            Ok(PeerMaterial {
                peer_id: PeerId::from(kp.public_key().clone()),
                address,
                public_key: kp.public_key().to_string(),
                private_key: ExposedPrivateKey(kp.private_key().clone()).to_string(),
                soranet_transport_public_key: soranet_transport_kp.public_key().to_string(),
                soranet_transport_private_key: ExposedPrivateKey(
                    soranet_transport_kp.private_key().clone(),
                )
                .to_string(),
                streaming_public_key: streaming_kp.public_key().to_string(),
                streaming_private_key: ExposedPrivateKey(streaming_kp.private_key().clone())
                    .to_string(),
                pop: pop.clone(),
                pop_hex: hex::encode(&pop),
            })
        })
        .collect()
}

fn deterministic_soranet_transport_keypair(
    spec: &ProfileSpec,
    peer_index: usize,
) -> AnyResult<KeyPair> {
    let seed_label = if peer_index == 0 {
        format!("kagami-{}-soranet-transport-v1", spec.slug)
    } else {
        format!(
            "kagami-{}-soranet-transport-v1-peer-{peer_index}",
            spec.slug
        )
    };
    let seed = Sha256::digest(seed_label.as_bytes()).to_vec();
    KeyPair::try_from_seed(seed, Algorithm::Ed25519).map_err(|err| {
        format!(
            "failed to derive deterministic Ed25519 SoraNet transport keypair for `{seed_label}`: {err}"
        )
        .into()
    })
}

fn deterministic_keypair(seed_label: &str, algorithm: Algorithm) -> AnyResult<KeyPair> {
    let mut hasher = Blake2b512::new();
    hasher.update(seed_label.as_bytes());
    let hash = hasher.finalize();
    let mut seed = Vec::with_capacity(32);
    seed.extend_from_slice(&hash[..32]);
    KeyPair::try_from_seed(seed, algorithm).map_err(|err| {
        format!(
            "failed to derive deterministic {} keypair for `{seed_label}`: {err}",
            algorithm.as_static_str()
        )
        .into()
    })
}

fn resolve_kagami_path(override_path: Option<&Path>) -> AnyResult<PathBuf> {
    if let Some(path) = override_path {
        if !path.exists() {
            return Err(format!("kagami override {} does not exist", path.display()).into());
        }
        if !path.is_file() {
            return Err(format!("kagami override {} is not a file", path.display()).into());
        }
        return path.canonicalize().map_err(|err| {
            format!("canonicalize kagami override {}: {err}", path.display()).into()
        });
    }

    let target_dir = cargo_target_dir();
    let release_candidate = target_dir
        .join("release")
        .join(format!("kagami{}", std::env::consts::EXE_SUFFIX));
    if release_candidate.exists() {
        return release_candidate.canonicalize().map_err(|err| {
            format!(
                "canonicalize release kagami binary {}: {err}",
                release_candidate.display()
            )
            .into()
        });
    }

    let debug_candidate = target_dir
        .join("debug")
        .join(format!("kagami{}", std::env::consts::EXE_SUFFIX));
    if debug_candidate.exists() {
        return debug_candidate.canonicalize().map_err(|err| {
            format!(
                "canonicalize debug kagami binary {}: {err}",
                debug_candidate.display()
            )
            .into()
        });
    }

    let status = Command::new("cargo")
        .args(["build", "-p", "iroha_kagami", "--release"])
        .status()?;
    if !status.success() {
        return Err(format!("cargo build -p iroha_kagami --release failed with {status:?}").into());
    }

    let release_candidate = target_dir
        .join("release")
        .join(format!("kagami{}", std::env::consts::EXE_SUFFIX));
    if release_candidate.exists() {
        release_candidate.canonicalize().map_err(|err| {
            format!(
                "canonicalize built kagami binary {}: {err}",
                release_candidate.display()
            )
            .into()
        })
    } else {
        Err(format!(
            "expected kagami binary at {} after build",
            release_candidate.display()
        )
        .into())
    }
}

fn cargo_target_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("CARGO_TARGET_DIR") {
        let path = PathBuf::from(dir);
        if path.is_absolute() {
            path
        } else {
            workspace_root().join(path)
        }
    } else {
        workspace_root().join("target")
    }
}

const PROFILES: &[ProfileSpec] = &[
    ProfileSpec {
        slug: "iroha3-dev",
        profile_flag: "iroha3-dev",
        chain_id: "iroha3-dev.local",
        chain_discriminant: None,
        min_peers: 4,
        requires_seed: false,
    },
    ProfileSpec {
        slug: "iroha3-taira",
        profile_flag: "iroha3-taira",
        chain_id: "iroha3-taira",
        chain_discriminant: Some(369),
        min_peers: 7,
        requires_seed: true,
    },
    ProfileSpec {
        slug: "iroha3-nexus",
        profile_flag: "iroha3-nexus",
        chain_id: "iroha3-nexus",
        chain_discriminant: Some(753),
        min_peers: 4,
        requires_seed: true,
    },
];

fn profile_slug_list() -> String {
    PROFILES
        .iter()
        .map(|spec| spec.slug)
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(test)]
mod tests {
    use iroha_config::{base::toml::TomlSource, parameters::actual};
    use iroha_crypto::Signature;
    use iroha_data_model::account::address::ChainDiscriminantGuard;
    use tempfile::tempdir;

    use super::*;

    fn stub_genesis() -> RawGenesisTransaction {
        json::from_str(
            r#"{
            "chain": "stub",
            "chain_discriminant": 753,
            "executor": null,
            "ivm_dir": ".",
            "consensus_mode": "Npos",
            "transactions": [ {} ]
        }"#,
        )
        .expect("stub genesis parses")
    }

    #[test]
    fn peers_are_deterministic_and_populated() {
        let peers = build_peers(&PROFILES[1]).expect("build deterministic peers");
        assert_eq!(peers.len(), PROFILES[1].min_peers);
        assert!(peers.iter().all(|p| !p.pop_hex.is_empty()));
        assert_eq!(
            peers[0].peer_id.public_key(),
            build_peers(&PROFILES[1]).expect("rebuild deterministic peers")[0]
                .peer_id
                .public_key()
        );
    }

    #[test]
    fn profile_peer_builder_rejects_non_committee_sizes() {
        for count in [1_usize, 2, 3, 5, 32] {
            let spec = ProfileSpec {
                min_peers: count,
                ..PROFILES[0]
            };
            let error = build_peers(&spec).expect_err("non-committee profile must fail");
            assert!(
                error.to_string().contains("exact revision-4 `3f + 1`"),
                "unexpected error for {count} peers: {error}"
            );
        }
    }

    #[test]
    fn topology_is_injected_into_genesis() {
        let peers = build_peers(&PROFILES[0]).expect("build deterministic peers");
        let patched = inject_topology(stub_genesis(), &peers).expect("inject topology");
        let txs = patched.transactions();
        assert_eq!(txs.len(), 1, "stub genesis should carry one transaction");
        let tx0 = &txs[0];
        assert_eq!(
            tx0.topology().len(),
            peers.len(),
            "topology should be populated inside the manifest"
        );
        let pop_count = tx0
            .topology()
            .iter()
            .filter(|entry| entry.pop_hex.as_deref().is_some_and(|hex| !hex.is_empty()))
            .count();
        assert_eq!(
            pop_count,
            peers.len(),
            "pop_hex should be embedded for every topology entry"
        );
        let value = json::to_value(&patched).expect("serialize patched genesis");
        let tx0 = value
            .get("transactions")
            .and_then(norito::json::Value::as_array)
            .and_then(|txs| txs.first())
            .and_then(norito::json::Value::as_object)
            .expect("first transaction present");
        let topo = tx0["topology"].as_array().expect("topology array present");
        assert_eq!(topo.len(), peers.len());
        let first = topo[0].as_object().expect("topology entry object");
        assert!(first.get("pop_hex").is_some(), "pop_hex embedded");
    }

    #[test]
    fn config_contains_expected_keys() {
        let peers = build_peers(&PROFILES[2]).expect("build deterministic peers");
        let genesis_key = deterministic_keypair("config-genesis", Algorithm::Ed25519)
            .expect("derive deterministic genesis key");
        let rendered = render_config(
            &PROFILES[2],
            &peers,
            genesis_key.public_key(),
            GENESIS_EXPECTED_HASH_PLACEHOLDER,
        );
        assert!(rendered.contains(PROFILES[2].chain_id));
        assert!(rendered.contains("chain_discriminant = 753"));
        assert!(rendered.contains("viral_incentive_pool_account"));
        assert!(rendered.contains(peers[0].public_key.as_str()));
        assert!(rendered.contains(&genesis_key.public_key().to_string()));
        assert!(rendered.contains(&peers[0].streaming_public_key));
        assert!(!rendered.contains(&peers[0].private_key));
        assert!(!rendered.contains(&peers[0].soranet_transport_private_key));
        assert!(!rendered.contains(&peers[0].streaming_private_key));
        assert!(rendered.contains(
            "private_key_file = \"/run/secrets/iroha/iroha3-nexus-peer-0-validator-private-key\""
        ));
        assert!(rendered.contains(
            "soranet_transport_private_key_file = \"/run/secrets/iroha/iroha3-nexus-peer-0-soranet-private-key\""
        ));
        assert!(rendered.contains(
            "identity_private_key_file = \"/run/secrets/iroha/iroha3-nexus-peer-0-streaming-private-key\""
        ));
        assert!(
            !rendered.contains("round_timeout_ms"),
            "round timing is derived from the signed genesis cadence"
        );
        assert!(
            !rendered.contains("[sumeragi.npos]"),
            "NPoS policy belongs in the signed genesis parameter snapshot"
        );
        assert!(
            !rendered.contains("protocol_version"),
            "wire protocol version is derived from the signed consensus metadata"
        );
        assert!(
            !rendered.contains("[sumeragi.da]"),
            "mandatory DA policy is derived from the signed consensus metadata"
        );
    }

    #[test]
    fn rendered_profile_configs_pass_actual_config_admission() {
        let expected_hash = Hash::new(b"xtask profile config admission").to_string();

        for profile in PROFILES {
            let peers = build_peers(profile).expect("build deterministic peers");
            let genesis_key = deterministic_keypair(
                &format!("config-{}-admission-genesis", profile.slug),
                Algorithm::Ed25519,
            )
            .expect("derive deterministic genesis key");
            let rendered = render_peer_config_with_private_keys(
                profile,
                &peers,
                0,
                genesis_key.public_key(),
                &expected_hash,
                PrivateKeyRendering::InlineStaging,
            );
            let table = rendered
                .parse::<toml::Table>()
                .expect("rendered profile config is valid TOML");
            let bundle = tempdir().expect("profile config admission directory");
            let path = bundle.path().join("peer0.toml");
            let _chain_discriminant = profile
                .chain_discriminant
                .map(ChainDiscriminantGuard::enter);

            actual::Root::from_toml_source(TomlSource::new(path, table)).unwrap_or_else(|error| {
                panic!(
                    "rendered profile {} must pass exact runtime config admission: {error:?}",
                    profile.slug
                )
            });
        }
    }

    #[test]
    fn published_profiles_keep_runtime_keys_outside_production_configs() {
        for profile in [&PROFILES[1], &PROFILES[2]] {
            let peers = build_peers(profile).expect("build deterministic peers");
            let genesis_key = deterministic_keypair(
                &format!("config-{}-public-only-genesis", profile.slug),
                Algorithm::Ed25519,
            )
            .expect("derive deterministic genesis key");
            let rendered = render_config(
                profile,
                &peers,
                genesis_key.public_key(),
                GENESIS_EXPECTED_HASH_PLACEHOLDER,
            );

            for peer in &peers {
                assert!(!rendered.contains(&peer.private_key));
                assert!(!rendered.contains(&peer.soranet_transport_private_key));
                assert!(!rendered.contains(&peer.streaming_private_key));
            }
            assert!(rendered.contains("private_key_file = \"/run/secrets/iroha/"));
            assert!(
                rendered.contains("soranet_transport_private_key_file = \"/run/secrets/iroha/")
            );
            assert!(rendered.contains("identity_private_key_file = \"/run/secrets/iroha/"));
            assert!(rendered.contains("expected_hash_file = \"/run/iroha/genesis.expected_hash\""));
            assert!(!rendered.contains(GENESIS_EXPECTED_HASH_PLACEHOLDER));
        }

        let profile = &PROFILES[0];
        let peers = build_peers(profile).expect("build deterministic dev peers");
        let genesis_key = deterministic_keypair("config-dev-inline-genesis", Algorithm::Ed25519)
            .expect("derive deterministic genesis key");
        let rendered = render_config(
            profile,
            &peers,
            genesis_key.public_key(),
            GENESIS_EXPECTED_HASH_PLACEHOLDER,
        );
        assert!(rendered.contains(&peers[0].private_key));
        assert!(rendered.contains(&peers[0].soranet_transport_private_key));
        assert!(rendered.contains(&peers[0].streaming_private_key));
        assert!(!rendered.contains("private_key_file"));
    }

    #[test]
    fn final_peer_configs_pin_the_exact_genesis_hash_and_prepared_names() {
        let peers = build_peers(&PROFILES[0]).expect("build deterministic peers");
        let genesis_key = deterministic_keypair("prepared-config-genesis", Algorithm::Ed25519)
            .expect("derive deterministic genesis key");
        let bundle = tempdir().expect("prepared config bundle");
        let expected_hash = Hash::new(b"prepared profile genesis").to_string();
        let expected_hash_literal =
            norito::literal::format("hash", &expected_hash.to_ascii_uppercase());

        write_peer_configs(
            &PROFILES[0],
            &peers,
            genesis_key.public_key(),
            bundle.path(),
            &expected_hash,
            PrivateKeyRendering::InlineStaging,
        )
        .expect("write prepared validator configs");

        for peer_index in 0..peers.len() {
            let compatibility =
                fs::read_to_string(bundle.path().join(peer_config_file_name(peer_index)))
                    .expect("read compatibility config");
            let prepared = fs::read_to_string(bundle.path().join(format!("peer{peer_index}.toml")))
                .expect("read prepared config");
            assert_eq!(compatibility, prepared);
            assert!(prepared.contains(&format!("expected_hash = \"{expected_hash_literal}\"")));
            assert!(!prepared.contains(GENESIS_EXPECTED_HASH_PLACEHOLDER));
        }
    }

    #[test]
    fn readme_carries_profile_metadata() {
        let peers = build_peers(&PROFILES[0]).expect("build deterministic peers");
        let genesis_key = deterministic_keypair("readme-genesis", Algorithm::Ed25519)
            .expect("derive deterministic genesis key");
        let readme = render_readme(&PROFILES[0], &peers, genesis_key.public_key(), None, None);
        assert!(readme.contains(PROFILES[0].slug));
        assert!(readme.contains("Regenerate"));
        assert!(readme.contains("genesis.public_key"));
        assert!(readme.contains("genesis.expected_hash"));
        assert!(readme.contains("peer0.toml through peerN.toml"));
        assert!(readme.contains("cargo xtask kagami-profiles --profile iroha3-dev\n"));
        assert!(!readme.contains("--nexus-xor-asset-definition-id"));
    }

    #[test]
    fn taira_readme_mentions_chain_discriminant() {
        let peers = build_peers(&PROFILES[1]).expect("build deterministic peers");
        let genesis_key = deterministic_keypair("readme-taira-genesis", Algorithm::Ed25519)
            .expect("derive deterministic genesis key");
        let readme = render_readme(
            &PROFILES[1],
            &peers,
            genesis_key.public_key(),
            Some("ABCD"),
            None,
        );
        assert!(readme.contains("- chain discriminant: 369"));
        assert!(readme.contains(
            "kagami verify --profile iroha3-taira --genesis genesis.json \
             --vrf-seed-hex ABCD"
        ));
        assert!(readme.contains("cargo xtask kagami-profiles --profile iroha3-taira\n"));
        assert!(!readme.contains("--nexus-xor-asset-definition-id"));
    }

    #[test]
    fn nexus_readme_regeneration_includes_asset_definition_id() {
        let peers = build_peers(&PROFILES[2]).expect("build deterministic peers");
        let genesis_key = deterministic_keypair("readme-nexus-genesis", Algorithm::Ed25519)
            .expect("derive deterministic genesis key");
        let readme = render_readme(
            &PROFILES[2],
            &peers,
            genesis_key.public_key(),
            Some("ABCD"),
            Some("xor-definition-id"),
        );
        assert!(readme.contains(
            "cargo xtask kagami-profiles --profile iroha3-nexus \
             --nexus-xor-asset-definition-id xor-definition-id\n"
        ));
    }

    #[test]
    fn nexus_requirement_is_preflighted_before_all_profile_mutation() {
        let temp = tempdir().expect("temp dir");
        let output = temp.path().join("profiles");
        let kagami = temp.path().join("kagami");
        fs::write(&kagami, b"unused test executable").expect("write dummy kagami file");

        for profile in PROFILES {
            let bundle = output.join(profile.slug);
            fs::create_dir_all(&bundle).expect("create existing profile bundle");
            fs::write(bundle.join("sentinel"), profile.slug).expect("write profile sentinel");
        }

        let error = generate(KagamiProfileOptions {
            output: output.clone(),
            profiles: vec!["all".to_owned()],
            kagami_override: Some(kagami),
            nexus_xor_asset_definition_id: None,
        })
        .expect_err("all-profile generation without the Nexus XOR id must fail");
        assert_eq!(error.to_string(), NEXUS_XOR_ASSET_DEFINITION_ID_REQUIRED);
        for profile in PROFILES {
            assert_eq!(
                fs::read_to_string(output.join(profile.slug).join("sentinel"))
                    .expect("pre-existing profile sentinel must remain"),
                profile.slug
            );
        }
    }

    #[test]
    fn validated_profile_publication_replaces_the_directory_without_mixing_files() {
        let temp = tempdir().expect("profile publication root");
        let destination = temp.path().join("iroha3-dev");
        fs::create_dir(&destination).expect("create previous bundle");
        fs::write(destination.join("old-only"), b"old").expect("write previous bundle");
        let staging = tempfile::Builder::new()
            .prefix(".profile-publication-test-")
            .tempdir_in(temp.path())
            .expect("create staged bundle");
        fs::write(staging.path().join("new-only"), b"new").expect("write staged bundle");

        publish_profile_bundle(staging, &destination).expect("publish validated bundle");

        assert_eq!(
            fs::read(destination.join("new-only")).expect("read published file"),
            b"new"
        );
        assert!(!destination.join("old-only").exists());
        assert!(
            fs::read_dir(temp.path())
                .expect("read publication root")
                .all(|entry| {
                    !entry
                        .expect("read publication entry")
                        .file_name()
                        .to_string_lossy()
                        .contains(".backup-")
                }),
            "successful publication must remove its recovery backup"
        );
    }

    #[test]
    fn invalid_nexus_identity_is_preflighted_before_profile_mutation() {
        let temp = tempdir().expect("temp dir");
        let output = temp.path().join("profiles");
        let bundle = output.join(PROFILES[2].slug);
        fs::create_dir_all(&bundle).expect("create existing Nexus bundle");
        fs::write(bundle.join("sentinel"), b"preserve").expect("write Nexus sentinel");

        let error = generate(KagamiProfileOptions {
            output: output.clone(),
            profiles: vec![PROFILES[2].slug.to_owned()],
            kagami_override: Some(temp.path().join("unused-kagami")),
            nexus_xor_asset_definition_id: Some("xor#universal".to_owned()),
        })
        .expect_err("invalid Nexus XOR identity must fail before output mutation");
        assert!(
            error
                .to_string()
                .contains("invalid --nexus-xor-asset-definition-id"),
            "unexpected preflight error: {error}"
        );
        assert_eq!(
            fs::read(bundle.join("sentinel")).expect("Nexus sentinel must remain"),
            b"preserve"
        );
    }

    #[test]
    fn deterministic_keypair_uses_checked_seed_expansion() {
        let keypair = deterministic_keypair("checked-seed-expansion", Algorithm::Ed25519)
            .expect("derive deterministic keypair");
        let signature = Signature::try_new(keypair.private_key(), b"kagami profile fixture")
            .expect("checked deterministic key signs fixture message");

        signature
            .verify(keypair.public_key(), b"kagami profile fixture")
            .expect("checked deterministic signature verifies");
    }

    #[test]
    fn relative_kagami_override_is_canonicalized_before_staged_chdir() {
        let current_dir = std::env::current_dir().expect("current directory");
        let temp = tempfile::tempdir_in(&current_dir).expect("temp dir under current directory");
        let kagami = temp.path().join("kagami-test-bin");
        fs::write(&kagami, b"test executable placeholder").expect("write kagami placeholder");
        let relative = kagami
            .strip_prefix(&current_dir)
            .expect("temporary kagami is below current directory");

        let resolved = resolve_kagami_path(Some(relative)).expect("resolve relative override");

        assert!(resolved.is_absolute());
        assert_eq!(
            resolved,
            kagami.canonicalize().expect("canonical temporary kagami")
        );
    }

    #[test]
    fn all_profile_configs_pin_default_torii_max_content_len() {
        let expected = format!(
            "max_content_len = {}",
            format_toml_integer_u64(DEFAULT_TORII_MAX_CONTENT_LEN)
        );
        for profile in PROFILES {
            let peers = build_peers(profile).expect("build deterministic peers");
            let seed = format!("config-{}-genesis", profile.slug);
            let genesis_key = deterministic_keypair(&seed, Algorithm::Ed25519)
                .expect("derive deterministic genesis key");
            let rendered = render_config(
                profile,
                &peers,
                genesis_key.public_key(),
                GENESIS_EXPECTED_HASH_PLACEHOLDER,
            );
            assert!(
                rendered.contains(&expected),
                "profile {} should pin the Torii body-cap default explicitly",
                profile.slug
            );
        }
    }

    #[test]
    fn all_profile_configs_use_canonical_socket_literals() {
        let expected_listen =
            SocketAddr::Ipv4(SocketAddrV4::from(([0, 0, 0, 0], 1337))).to_literal();
        let expected_public =
            SocketAddr::Ipv4(SocketAddrV4::from(([172, 28, 0, 10], 1337))).to_literal();
        let expected_torii =
            SocketAddr::Ipv4(SocketAddrV4::from(([0, 0, 0, 0], 8080))).to_literal();
        for profile in PROFILES {
            let peers = build_peers(profile).expect("build deterministic peers");
            let seed = format!("config-{}-address-genesis", profile.slug);
            let genesis_key = deterministic_keypair(&seed, Algorithm::Ed25519)
                .expect("derive deterministic genesis key");
            let rendered = render_config(
                profile,
                &peers,
                genesis_key.public_key(),
                GENESIS_EXPECTED_HASH_PLACEHOLDER,
            );
            assert!(
                rendered.contains(&format!("address = \"{expected_listen}\"")),
                "profile {} must render the canonical network listen literal",
                profile.slug
            );
            assert!(
                rendered.contains(&format!("public_address = \"{expected_public}\"")),
                "profile {} must render the canonical advertised network literal",
                profile.slug
            );
            assert!(
                rendered.contains(&format!("address = \"{expected_torii}\"")),
                "profile {} must render the canonical Torii listen literal",
                profile.slug
            );
        }
    }

    #[test]
    fn all_profile_configs_scale_body_ingress_for_the_complete_committee() {
        let authenticated_non_validator_sources =
            iroha_config::parameters::defaults::sumeragi::QUEUE_AUTHENTICATED_NON_VALIDATOR_SOURCE_CAPACITY
                .get();
        let body_source_bytes =
            iroha_config::parameters::defaults::sumeragi::QUEUE_BODY_SOURCE_BYTES.get();
        let default_body_bytes =
            iroha_config::parameters::defaults::sumeragi::QUEUE_BODY_BYTES.get();

        for profile in PROFILES {
            let peers = build_peers(profile).expect("build deterministic peers");
            let genesis_key = deterministic_keypair(
                &format!("config-{}-queue-genesis", profile.slug),
                Algorithm::Ed25519,
            )
            .expect("derive deterministic genesis key");
            let rendered = render_config(
                profile,
                &peers,
                genesis_key.public_key(),
                GENESIS_EXPECTED_HASH_PLACEHOLDER,
            );
            let expected_body_bytes = peers
                .len()
                .checked_add(authenticated_non_validator_sources)
                .and_then(|count| count.checked_add(1))
                .and_then(|count| count.checked_mul(body_source_bytes))
                .expect("test profile ingress geometry fits usize")
                .max(default_body_bytes);

            assert!(
                rendered.contains(&format!(
                    "[sumeragi.queues]\n\
                     authenticated_non_validator_sources = {authenticated_non_validator_sources}\n\
                     body_bytes = {expected_body_bytes}\n\
                     body_source_bytes = {body_source_bytes}\n"
                )),
                "profile {} must allocate one isolated byte partition per validator, authenticated non-validator source, and anonymous source",
                profile.slug
            );
        }
    }

    #[test]
    fn taira_config_pins_site_nexus_and_mcp_policy() {
        let peers = build_peers(&PROFILES[1]).expect("build deterministic peers");
        let genesis_key = deterministic_keypair("config-taira-quota-genesis", Algorithm::Ed25519)
            .expect("derive deterministic genesis key");
        let rendered = render_config(
            &PROFILES[1],
            &peers,
            genesis_key.public_key(),
            GENESIS_EXPECTED_HASH_PLACEHOLDER,
        );
        assert!(rendered.contains("[sorafs.gateway.site_bindings]"));
        assert!(rendered.contains("path = \"/config/sorafs_sites.json\""));
        assert!(rendered.contains("max_bytes = 1048576"));
        assert!(rendered.contains("max_sites = 1024"));
        assert!(rendered.contains("[sumeragi.block]\nmax_transactions = 96\n"));
        assert!(rendered.contains(
            "[torii.mcp]\nenabled = true\nprofile = \"writer\"\nexpose_operator_routes = false\nallow_tool_prefixes = [\"iroha.\"]\n",
        ));
        assert!(rendered.contains("[nexus.fees]\nfee_asset_id = \"xor#universal\"\n"));
        assert!(rendered.contains("[nexus.staking]\nstake_asset_id = \"xor#universal\"\n"));

        let dev_peers = build_peers(&PROFILES[0]).expect("build deterministic dev peers");
        let dev_genesis_key =
            deterministic_keypair("config-dev-policy-genesis", Algorithm::Ed25519)
                .expect("derive deterministic dev genesis key");
        let dev = render_config(
            &PROFILES[0],
            &dev_peers,
            dev_genesis_key.public_key(),
            GENESIS_EXPECTED_HASH_PLACEHOLDER,
        );
        assert!(!dev.contains("[torii.mcp]"));
        assert!(!dev.contains("[nexus.fees]"));
        assert!(!dev.contains("[nexus.staking]"));
    }

    #[test]
    fn profiles_do_not_emit_backend_offline_capability_switches() {
        for profile in PROFILES {
            let peers = build_peers(profile).expect("build deterministic generic peers");
            let seed = format!("config-{}-universal-offline-genesis", profile.slug);
            let genesis_key = deterministic_keypair(&seed, Algorithm::Ed25519)
                .expect("derive deterministic generic genesis key");
            let rendered = render_config(
                profile,
                &peers,
                genesis_key.public_key(),
                GENESIS_EXPECTED_HASH_PLACEHOLDER,
            );
            let config: toml::Value =
                toml::from_str(&rendered).expect("parse rendered generic config");

            assert!(
                config
                    .get("settlement")
                    .and_then(toml::Value::as_table)
                    .and_then(|settlement| settlement.get("offline"))
                    .is_none(),
                "profile {} must not model universal offline support as a backend opt-in",
                profile.slug
            );
            for retired in ["escrow_required", "escrow_accounts", "offline.enabled"] {
                assert!(
                    !rendered.contains(retired),
                    "profile {} emitted retired offline setting {retired}",
                    profile.slug
                );
            }
        }
    }

    #[test]
    fn taira_compose_mounts_config_backed_site_bindings_without_runtime_env() {
        let peers = build_peers(&PROFILES[1]).expect("build deterministic peers");
        let rendered = render_docker_compose(&PROFILES[1], &peers);
        assert!(
            rendered.contains("./sorafs_sites.json:/config/sorafs_sites.json:ro"),
            "Taira compose must mount the startup-configured binding document"
        );
        assert!(!rendered.contains("IROHA_SORAFS_SITE_BINDINGS_FILE"));
        assert_eq!(
            rendered
                .matches("/run/secrets/iroha:/run/secrets/iroha:ro")
                .count(),
            peers.len(),
            "each production validator must receive the runtime key directory read-only"
        );
        let dev_peers = build_peers(&PROFILES[0]).expect("build deterministic dev peers");
        let dev_compose = render_docker_compose(&PROFILES[0], &dev_peers);
        assert!(
            !dev_compose.contains("sorafs_sites.json"),
            "profiles without a configured binding document must not mount one"
        );
        assert!(!dev_compose.contains("/run/secrets/iroha"));

        let genesis_key = deterministic_keypair("readme-taira-sites", Algorithm::Ed25519)
            .expect("derive deterministic genesis key");
        let readme = render_readme(
            &PROFILES[1],
            &peers,
            genesis_key.public_key(),
            Some("ABCD"),
            None,
        );
        assert!(readme.contains("sorafs_sites.json"));
    }

    #[test]
    fn compose_launches_every_signed_topology_member_with_unique_config() {
        let peers = build_peers(&PROFILES[0]).expect("build deterministic dev peers");
        let rendered = render_docker_compose(&PROFILES[0], &peers);

        assert_eq!(
            rendered
                .matches("    image: hyperledger/iroha:latest")
                .count(),
            peers.len()
        );
        assert_eq!(
            rendered.matches("ipv4_address: 172.28.0.").count(),
            peers.len()
        );
        assert!(rendered.contains("./config.toml:/config/config.toml:ro"));
        assert_eq!(
            rendered
                .matches("./genesis.signed.nrt:/config/genesis.signed.nrt:ro")
                .count(),
            peers.len()
        );
        assert!(!rendered.contains("--genesis"));
        for peer_index in 1..peers.len() {
            assert!(rendered.contains(&format!(
                "./config-peer-{peer_index}.toml:/config/config.toml:ro"
            )));
        }
    }

    #[test]
    fn peer_configs_use_distinct_consensus_streaming_and_port_material() {
        let peers = build_peers(&PROFILES[0]).expect("build deterministic dev peers");
        let genesis_key = deterministic_keypair("distinct-peer-configs", Algorithm::Ed25519)
            .expect("derive deterministic genesis key");
        let mut transport_public_keys = std::collections::BTreeSet::new();

        for (peer_index, peer) in peers.iter().enumerate() {
            let rendered = render_peer_config(
                &PROFILES[0],
                &peers,
                peer_index,
                genesis_key.public_key(),
                GENESIS_EXPECTED_HASH_PLACEHOLDER,
            );
            let torii_port = 8080 + peer_index;
            assert!(rendered.contains(&format!("private_key = \"{}\"", peer.private_key)));
            assert!(rendered.contains(&format!(
                "soranet_transport_public_key = \"{}\"",
                peer.soranet_transport_public_key
            )));
            assert!(rendered.contains(&format!(
                "soranet_transport_private_key = \"{}\"",
                peer.soranet_transport_private_key
            )));
            assert!(rendered.contains(&peer.streaming_public_key));
            assert!(rendered.contains(&peer.streaming_private_key));
            assert_ne!(peer.soranet_transport_public_key, peer.public_key);
            assert_ne!(peer.soranet_transport_private_key, peer.private_key);
            assert_ne!(peer.soranet_transport_public_key, peer.streaming_public_key);
            assert_ne!(
                peer.soranet_transport_private_key,
                peer.streaming_private_key
            );
            let transport_public = peer
                .soranet_transport_public_key
                .parse::<iroha_crypto::PublicKey>()
                .expect("transport public key");
            let transport_private = peer
                .soranet_transport_private_key
                .parse::<ExposedPrivateKey>()
                .expect("transport private key");
            let transport_key_pair = KeyPair::new(transport_public.clone(), transport_private.0)
                .expect("transport public/private key pair must match");
            assert_eq!(transport_key_pair.algorithm(), Algorithm::Ed25519);
            assert!(
                transport_public_keys.insert(transport_public),
                "profile peers must not share a SoraNet transport identity"
            );
            assert!(rendered.contains(&format!("0.0.0.0:{torii_port}")));
            assert!(rendered.contains("file = \"genesis.signed.nrt\""));
            assert!(!rendered.contains("manifest_json"));
            for (other_index, other) in peers.iter().enumerate() {
                if peer_index != other_index {
                    assert!(
                        !rendered.contains(&format!("private_key = \"{}\"", other.private_key))
                    );
                    assert!(!rendered.contains(&other.soranet_transport_private_key));
                    assert!(!rendered.contains(&other.streaming_private_key));
                }
            }
        }
    }

    #[test]
    fn checked_in_profile_transport_identities_are_reproducible() {
        let expected = [
            (
                "ed01205A4FF1E3840273F79909F02BA854FE9394DE6FBAEF06B87397B059C16BAD6ADC",
                "802620BBEB9930B26B2CFB85EF7683349DAB2921927F0EA1F5E5BFF89C7E60A7D1700B",
            ),
            (
                "ed012080A47B672C44202B67EC8E81DFFA4D0B46AD2507113C8627F30599FB1CC83717",
                "802620F18FE388B674C8831AB6061413C8184D7BC03C5595B3AD671852B8FE1611240F",
            ),
            (
                "ed01201F60DE7C82F77FF1EA9AA2DFC60166A0DF904A771DCBFF36186EFAE8AC8324D3",
                "802620720B9507E31A382E02FF4523D0E22071E19D39974C9AE907492EEAE616F23E15",
            ),
        ];

        for (spec, (public_key, private_key)) in PROFILES.iter().zip(expected) {
            let peers = build_peers(spec).expect("build deterministic profile peers");
            assert_eq!(peers[0].soranet_transport_public_key, public_key);
            assert_eq!(peers[0].soranet_transport_private_key, private_key);
        }
    }

    #[test]
    fn verify_output_captured_on_success() {
        if std::env::var("XTASK_TEST_KAGAMI_BIN").is_err() {
            return;
        }
        let kagami_path = PathBuf::from(std::env::var("XTASK_TEST_KAGAMI_BIN").unwrap());
        let temp = tempdir().expect("temp dir");
        let genesis_path = temp.path().join("genesis.json");
        let mut rendered = json::to_json_pretty(&stub_genesis()).expect("render stub genesis");
        rendered.push('\n');
        fs::write(&genesis_path, rendered).expect("write stub genesis");
        let out = run_verify(&PROFILES[0], &kagami_path, &genesis_path, None);
        assert!(
            out.is_ok(),
            "verify should succeed when kagami binary is supplied: {out:?}"
        );
    }
}
