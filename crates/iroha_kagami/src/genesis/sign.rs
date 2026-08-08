use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::{BufWriter, Read, Write},
    path::{Path, PathBuf},
    sync::Arc,
};

use clap::Parser;
use color_eyre::eyre::{WrapErr, eyre};
use iroha_config::{
    base::toml::TomlSource,
    parameters::{actual, defaults},
};
use iroha_core::{
    block::ValidBlock,
    compliance::LaneComplianceEngine,
    governance::manifest::LaneManifestRegistry,
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::isi::Registrable as _,
    state::{State, World},
    sumeragi::{VotingBlock, network_topology::Topology},
};
use iroha_crypto::{Algorithm, ExposedPrivateKey, Hash, KeyPair, PrivateKey, PublicKey};
use iroha_data_model::{
    account::address::{AccountAddress, ChainDiscriminantGuard},
    asset::AssetDefinitionAlias,
    block::consensus_v2::{
        ConsensusMode as WireConsensusMode, MAX_VALIDATORS_PER_HEIGHT, is_valid_committee_size,
    },
    da::commitment::DaProofPolicyBundle,
    isi::RegisterPublicLaneValidator,
    parameter::system::SumeragiConsensusMode,
    prelude::*,
};
use iroha_genesis::{GenesisBlock, GenesisBuilder, GenesisTopologyEntry, RawGenesisTransaction};
use iroha_primitives::time::TimeSource;
use zeroize::Zeroize as _;

use super::{
    ConsensusPolicy, build_line_from_env, ensure_npos_parameters, generate::ConsensusModeArg,
    require_v2_wire_protocol_only, validate_consensus_mode_for_line,
};
use crate::{
    Outcome, RunArgs,
    genesis::{PUBLIC_XOR_ALIAS, public_xor_profile_for_chain_id},
    tui,
};

/// Sign the genesis block
#[derive(Clone, Debug, Parser)]
pub struct Args {
    /// Path to genesis json file
    genesis_file: PathBuf,
    /// Path to signed genesis output file in Norito format (stdout by default).
    #[clap(short, long, value_name("PATH"))]
    out_file: Option<PathBuf>,
    /// Persist the exact config-bound genesis manifest used to build the signed block.
    /// May point to `GENESIS_FILE` to replace the input only after binding succeeds.
    #[clap(long, value_name = "PATH")]
    bound_manifest_out: Option<PathBuf>,
    /// Write the exact signed consensus-header hash as one lowercase line.
    ///
    /// This also atomically publishes a sibling `*.identity.toml` containing
    /// the same exact value as both client `network_id` and
    /// `genesis.expected_hash`. Deployment tooling must consume that paired
    /// identity artifact rather than assembling the two trust domains
    /// independently.
    #[clap(long, value_name = "PATH")]
    expected_hash_out: Option<PathBuf>,
    /// Use this topology instead of specified in genesis.json.
    /// JSON-serialized vector of `PeerId`. For use in `iroha_swarm`.
    ///
    /// The final unique topology must be an exact Sumeragi v2 `3f + 1`
    /// committee in the range 4..=31.
    #[clap(short, long)]
    topology: Option<String>,
    /// Embed one or more PoPs into the same transaction as `--topology`.
    /// Repeatable flag: `--peer-pop <public_key=pop_hex>`
    #[clap(long = "peer-pop")]
    peer_pops: Vec<String>,
    /// Private key hex (multihash payload, not prefixed) that matches the genesis public key.
    #[clap(long, conflicts_with = "seed", value_name = "HEX")]
    private_key: Option<String>,
    /// Owner-held mode-0600 file containing one canonical private-key multihash.
    #[clap(
        long,
        conflicts_with_all = ["private_key", "seed"],
        value_name = "PATH"
    )]
    private_key_file: Option<PathBuf>,
    /// Public key that the selected private key must derive.
    ///
    /// Use this when the verifier key is distributed separately from the
    /// owner-held signing key, such as through container secrets.
    #[clap(long, value_name = "PUBLIC_KEY")]
    expected_public_key: Option<PublicKey>,
    /// A 32-byte secret genesis key-generation seed encoded as 64 hexadecimal characters.
    ///
    /// This is a testing convenience. Production operators should prefer an
    /// owner-held private-key file.
    #[clap(long = "seed-hex", conflicts_with = "private_key", value_name = "HEX")]
    seed: Option<String>,
    /// Deterministic genesis transaction creation-time base in Unix milliseconds.
    ///
    /// Omit this for a fresh wall-clock timestamp. Fixture generators should
    /// set it so repeated signing produces identical canonical wire bytes.
    #[clap(long, value_name = "MILLISECONDS")]
    creation_time_ms: Option<u64>,
    /// Algorithm of the genesis key (must match the genesis public key).
    #[clap(long, default_value = "ed25519", value_name = "ALGORITHM")]
    algorithm: Algorithm,
    /// Optional peer config TOML used to derive the DA proof-policy bundle embedded into genesis.
    #[clap(long, value_name = "PATH")]
    config: Option<PathBuf>,
    /// Select the consensus mode to stamp into the manifest (optional override).
    #[clap(long, value_enum, value_name = "MODE")]
    consensus_mode: Option<ConsensusModeArg>,
}

const DEFAULT_NPOS_BOOTSTRAP_DOMAIN: &str = "nexus.universal";
const DEFAULT_NPOS_BOOTSTRAP_STAKE_ASSET_NAME: &str = "xor";
const DEFAULT_NPOS_BOOTSTRAP_STAKE_AMOUNT: u64 = 10_000;
const GENESIS_EXPECTED_HASH_PLACEHOLDER: &str = "REPLACE_WITH_GENESIS_EXPECTED_HASH";
const MAX_DEPLOYMENT_IDENTITY_BYTES: u64 = 4 * 1024;

fn deployment_identity_path(expected_hash_path: &Path) -> PathBuf {
    expected_hash_path.with_extension("identity.toml")
}

fn publish_deployment_identity(path: &Path, expected_hash: &str) -> Outcome {
    let body = format!(
        "network_id = \"{expected_hash}\"\n\n[genesis]\nexpected_hash = \"{expected_hash}\"\n"
    );
    if body.len() as u64 > MAX_DEPLOYMENT_IDENTITY_BYTES {
        return Err(eyre!(
            "generated deployment identity exceeds the {MAX_DEPLOYMENT_IDENTITY_BYTES}-byte limit"
        ));
    }

    if let Ok(metadata) = fs::symlink_metadata(path) {
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(eyre!(
                "deployment identity output must be a regular file: {}",
                path.display()
            ));
        }
        if metadata.len() > MAX_DEPLOYMENT_IDENTITY_BYTES {
            return Err(eyre!(
                "existing deployment identity exceeds the {MAX_DEPLOYMENT_IDENTITY_BYTES}-byte limit: {}",
                path.display()
            ));
        }
        let mut existing = String::new();
        File::open(path)?
            .take(MAX_DEPLOYMENT_IDENTITY_BYTES + 1)
            .read_to_string(&mut existing)
            .wrap_err_with(|| format!("read existing deployment identity {}", path.display()))?;
        if existing == body {
            return Ok(());
        }
        return Err(eyre!(
            "refusing to replace a different deployment identity: {}",
            path.display()
        ));
    }

    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut temporary = tempfile::Builder::new()
        .prefix(".genesis-identity-")
        .tempfile_in(parent)
        .wrap_err_with(|| {
            format!(
                "create temporary deployment identity beside {}",
                path.display()
            )
        })?;
    temporary
        .write_all(body.as_bytes())
        .wrap_err("write temporary deployment identity")?;
    temporary
        .flush()
        .wrap_err("flush temporary deployment identity")?;
    temporary
        .as_file()
        .sync_all()
        .wrap_err("sync temporary deployment identity")?;
    match temporary.persist_noclobber(path) {
        Ok(_) => {}
        Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {
            let metadata = fs::symlink_metadata(path).wrap_err_with(|| {
                format!("inspect raced deployment identity {}", path.display())
            })?;
            if !metadata.is_file()
                || metadata.file_type().is_symlink()
                || metadata.len() > MAX_DEPLOYMENT_IDENTITY_BYTES
            {
                return Err(eyre!(
                    "raced deployment identity is not the expected regular bounded file: {}",
                    path.display()
                ));
            }
            let mut existing = String::new();
            File::open(path)?
                .take(MAX_DEPLOYMENT_IDENTITY_BYTES + 1)
                .read_to_string(&mut existing)
                .wrap_err_with(|| format!("read raced deployment identity {}", path.display()))?;
            if existing != body {
                return Err(eyre!(
                    "refusing raced deployment identity with different contents: {}",
                    path.display()
                ));
            }
        }
        Err(error) => {
            return Err(error.error).wrap_err_with(|| {
                format!(
                    "publish deployment identity atomically to {}",
                    path.display()
                )
            });
        }
    }
    #[cfg(unix)]
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .wrap_err_with(|| format!("sync deployment identity directory {}", parent.display()))?;
    Ok(())
}

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
                continue;
            }
            if let Some(register) = instruction
                .as_any()
                .downcast_ref::<iroha_data_model::isi::register::RegisterBox>()
            {
                match register {
                    iroha_data_model::isi::register::RegisterBox::Domain(register) => {
                        domains.insert(register.object.id.clone());
                    }
                    iroha_data_model::isi::register::RegisterBox::Account(register) => {
                        accounts.insert(register.object.id.clone());
                    }
                    iroha_data_model::isi::register::RegisterBox::AssetDefinition(register) => {
                        asset_defs.insert(register.object.id.clone());
                    }
                    _ => {}
                }
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
    let mut peers = Vec::new();
    for tx in manifest.transactions() {
        for entry in tx.topology() {
            peers.push(entry.peer.clone());
        }
    }
    peers
}

fn default_npos_bootstrap_stake_asset_id() -> AssetDefinitionId {
    AssetDefinitionId::derive_from_components(
        DomainId::parse_fully_qualified(DEFAULT_NPOS_BOOTSTRAP_DOMAIN)
            .expect("static stake asset domain must remain valid"),
        DEFAULT_NPOS_BOOTSTRAP_STAKE_ASSET_NAME
            .parse()
            .expect("static stake asset name must remain valid"),
    )
}

fn resolve_npos_bootstrap_stake_asset_id(
    manifest: &RawGenesisTransaction,
    configured: &str,
) -> Result<AssetDefinitionId, color_eyre::eyre::Error> {
    if let Ok(asset_id) = configured.parse::<AssetDefinitionId>() {
        return Ok(asset_id);
    }

    let alias = configured.parse::<AssetDefinitionAlias>().map_err(|err| {
        eyre!(
            "invalid nexus.staking.stake_asset_id `{configured}`: {err}; expected canonical asset definition id or alias"
        )
    })?;

    let Some(asset_definition_id) = resolve_asset_definition_alias(manifest, &alias)? else {
        return Err(eyre!(
            "nexus.staking.stake_asset_id alias `{configured}` is not bound in genesis manifest"
        ));
    };
    Ok(asset_definition_id)
}

fn resolve_asset_definition_alias(
    manifest: &RawGenesisTransaction,
    alias: &AssetDefinitionAlias,
) -> Result<Option<AssetDefinitionId>, color_eyre::eyre::Error> {
    let mut target = None;
    for instruction in manifest.instructions() {
        if let Some(bind) = instruction
            .as_any()
            .downcast_ref::<iroha_data_model::isi::asset_alias::SetAssetDefinitionAlias>(
        ) && bind.alias.as_ref() == Some(alias)
        {
            if let Some(existing) = &target
                && existing != &bind.asset_definition_id
            {
                return Err(eyre!(
                    "asset definition alias `{alias}` is bound to multiple asset definitions"
                ));
            }
            target = Some(bind.asset_definition_id.clone());
        }
    }
    Ok(target)
}

fn public_xor_profile_for_manifest(
    manifest: &RawGenesisTransaction,
) -> Option<crate::genesis::GenesisProfile> {
    public_xor_profile_for_chain_id(manifest.chain_id().as_str())
}

fn configured_npos_bootstrap_stake_asset_id(
    manifest: &RawGenesisTransaction,
    config: Option<&actual::Root>,
) -> Result<AssetDefinitionId, color_eyre::eyre::Error> {
    let public_profile = public_xor_profile_for_manifest(manifest);
    let stake_asset_id = if let Some(config) = config {
        resolve_npos_bootstrap_stake_asset_id(manifest, &config.nexus.staking.stake_asset_id)
            .map_err(|err| eyre!("failed to resolve nexus.staking.stake_asset_id: {err}"))?
    } else if public_profile.is_some() {
        let public_xor_alias: AssetDefinitionAlias = PUBLIC_XOR_ALIAS.parse()?;
        resolve_asset_definition_alias(manifest, &public_xor_alias)?.ok_or_else(|| {
            eyre!(
                "public NPoS bootstrap requires `{PUBLIC_XOR_ALIAS}` to be bound to a canonical XOR asset in genesis; regenerate with `kagami genesis generate --xor-asset-definition-id <BASE58>` or pass a config with an explicit canonical stake asset"
            )
        })?
    } else {
        default_npos_bootstrap_stake_asset_id()
    };

    if let Some(profile) = public_profile {
        let public_xor_alias: AssetDefinitionAlias = PUBLIC_XOR_ALIAS.parse()?;
        let public_xor_asset_id =
            resolve_asset_definition_alias(manifest, &public_xor_alias)?.ok_or_else(|| {
                eyre!(
                    "public NPoS bootstrap for {profile:?} requires `{PUBLIC_XOR_ALIAS}` to be bound to a canonical XOR asset in genesis"
                )
            })?;
        if profile == crate::genesis::GenesisProfile::Iroha3Taira
            && public_xor_asset_id.to_string() != crate::genesis::TAIRA_XOR_ASSET_DEFINITION_ID
        {
            return Err(eyre!(
                "public Taira NPoS bootstrap requires `{PUBLIC_XOR_ALIAS}` to bind to `{}`; found `{public_xor_asset_id}`",
                crate::genesis::TAIRA_XOR_ASSET_DEFINITION_ID
            ));
        }
        if public_xor_asset_id == default_npos_bootstrap_stake_asset_id() {
            return Err(eyre!(
                "public NPoS bootstrap for {profile:?} cannot use synthetic `{DEFAULT_NPOS_BOOTSTRAP_DOMAIN}/{DEFAULT_NPOS_BOOTSTRAP_STAKE_ASSET_NAME}`; bind `{PUBLIC_XOR_ALIAS}` to the real XOR asset or configure a canonical stake asset id"
            ));
        }
        if stake_asset_id != public_xor_asset_id {
            return Err(eyre!(
                "public NPoS bootstrap for {profile:?} resolved stake asset `{stake_asset_id}`, but `{PUBLIC_XOR_ALIAS}` is bound to `{public_xor_asset_id}`; public stake asset must match the canonical XOR binding"
            ));
        }
    }

    Ok(stake_asset_id)
}

fn configured_npos_bootstrap_escrow_account_id(
    manifest: &RawGenesisTransaction,
    config: Option<&actual::Root>,
) -> Result<AccountId, color_eyre::eyre::Error> {
    let literal = if let Some(config) = config {
        config.nexus.staking.stake_escrow_account_id.clone()
    } else {
        staged_default_nexus(manifest)?
            .staking
            .stake_escrow_account_id
    };
    AccountId::parse_encoded(&literal)
        .map(iroha_data_model::account::ParsedAccountId::into_account_id)
        .map_err(|error| {
            eyre!(
                "NPoS auto-bootstrap requires `nexus.staking.stake_escrow_account_id` to be a canonical account literal, found `{literal}`: {error}"
            )
        })
}

fn append_npos_bootstrap(
    builder: GenesisBuilder,
    registrations: &mut BootstrapRegistrations,
    topology: &[PeerId],
    escrow_account_id: &AccountId,
    stake_asset_id: &AssetDefinitionId,
) -> Result<GenesisBuilder, color_eyre::eyre::Error> {
    if topology.is_empty() {
        return Ok(builder);
    }

    let default_stake_asset_id = default_npos_bootstrap_stake_asset_id();

    let mut builder = builder.next_transaction();
    if stake_asset_id == &default_stake_asset_id {
        let nexus_domain = DomainId::parse_fully_qualified(DEFAULT_NPOS_BOOTSTRAP_DOMAIN)?;
        if !registrations.domains.contains(&nexus_domain) {
            builder =
                builder.append_instruction(Register::domain(Domain::new(nexus_domain.clone())));
            registrations.domains.insert(nexus_domain);
        }
    }
    if !registrations.accounts.contains(escrow_account_id) {
        builder =
            builder.append_instruction(Register::account(Account::new(escrow_account_id.clone())));
        registrations.accounts.insert(escrow_account_id.clone());
    }
    if !registrations.asset_defs.contains(stake_asset_id) {
        let definition = AssetDefinition::new(
            stake_asset_id.clone(),
            "NPOS Stake".to_owned(),
            NumericSpec::default(),
            iroha_data_model::asset::AssetBalancePolicy::Global,
            None,
        )
        .with_metadata(Metadata::default());
        builder = builder.append_instruction(Register::asset_definition(definition));
        registrations.asset_defs.insert(stake_asset_id.clone());
    }

    for peer in topology {
        let validator_id = AccountId::new(peer.public_key().clone());
        if !registrations.accounts.contains(&validator_id) {
            builder =
                builder.append_instruction(Register::account(Account::new(validator_id.clone())));
            registrations.accounts.insert(validator_id.clone());
        }
        builder = builder.append_instruction(Mint::asset_quantity(
            DEFAULT_NPOS_BOOTSTRAP_STAKE_AMOUNT,
            AssetId::new(stake_asset_id.clone(), validator_id.clone()),
        ));
        builder = builder.append_instruction(RegisterPublicLaneValidator {
            lane_id: LaneId::SINGLE,
            validator: validator_id.clone(),
            peer_id: peer.clone(),
            stake_account: validator_id.clone(),
            initial_stake: iroha_primitives::numeric::Quantity::from(
                DEFAULT_NPOS_BOOTSTRAP_STAKE_AMOUNT,
            ),
            metadata: Metadata::default(),
        });
        builder = builder.append_instruction(ActivatePublicLaneValidator {
            lane_id: LaneId::SINGLE,
            validator: validator_id,
        });
    }

    Ok(builder)
}

fn load_peer_config(config_path: &Path) -> Result<actual::Root, color_eyre::eyre::Error> {
    let mut source = TomlSource::from_file(config_path).map_err(|err| {
        eyre!(
            "failed to read peer config at {}: {err}",
            config_path.display()
        )
    })?;
    // Checked-in signing profiles are deliberately not runnable before their exact signed block
    // exists. The signing path needs the remaining consensus-policy projection to construct that
    // block, so replace only the explicit non-hash sentinel in this in-memory copy. The normal
    // node configuration parser never performs this substitution and therefore fails closed.
    if let Some(expected_hash) = source
        .table_mut()
        .get_mut("genesis")
        .and_then(toml::Value::as_table_mut)
        .and_then(|genesis| genesis.get_mut("expected_hash"))
        && expected_hash.as_str() == Some(GENESIS_EXPECTED_HASH_PLACEHOLDER)
    {
        let hash_body =
            Hash::new(b"Kagami unresolved genesis hash used only for policy derivation")
                .to_string()
                .to_ascii_uppercase();
        *expected_hash = toml::Value::String(norito::literal::format("hash", hash_body.as_str()));
    }
    actual::Root::from_toml_source(source).map_err(|err| {
        eyre!(
            "failed to parse peer config at {}: {err:?}",
            config_path.display()
        )
    })
}

fn ensure_peer_config_matches_manifest(
    config: &actual::Root,
    manifest: &RawGenesisTransaction,
) -> Result<(), color_eyre::eyre::Error> {
    if config.common.chain != *manifest.chain_id() {
        return Err(eyre!(
            "peer config chain `{}` does not match genesis manifest chain `{}`",
            config.common.chain,
            manifest.chain_id()
        ));
    }
    let configured_discriminant = *config.common.chain_discriminant.value();
    if configured_discriminant != manifest.chain_discriminant() {
        return Err(eyre!(
            "peer config chain discriminant {configured_discriminant} does not match genesis manifest chain discriminant {}",
            manifest.chain_discriminant()
        ));
    }
    Ok(())
}

fn build_signed_genesis(
    genesis: RawGenesisTransaction,
    genesis_key_pair: &KeyPair,
    da_proof_policies: Option<DaProofPolicyBundle>,
    confidential_policy_hash: [u8; 32],
    creation_time_ms: Option<u64>,
) -> Result<GenesisBlock, color_eyre::eyre::Error> {
    match creation_time_ms {
        Some(creation_time_ms) => genesis
            .build_and_sign_with_da_proof_policies_and_confidential_policy_hash_at(
                genesis_key_pair,
                da_proof_policies,
                Some(confidential_policy_hash),
                creation_time_ms,
            )
            .map_err(Into::into),
        None => genesis
            .build_and_sign_with_da_proof_policies_and_confidential_policy_hash(
                genesis_key_pair,
                da_proof_policies,
                Some(confidential_policy_hash),
            )
            .map_err(Into::into),
    }
}

/// Bind the staged consensus context and sign the exact resulting manifest.
///
/// Callers which publish both the manifest and signed block must persist the
/// returned manifest so prepared-bundle admission can compare every
/// instruction, including the derived consensus commitment.
pub(crate) fn bind_and_sign_staged_sumeragi_v2_context(
    genesis: RawGenesisTransaction,
    genesis_key_pair: &KeyPair,
    config: Option<&actual::Root>,
    da_proof_policies: Option<DaProofPolicyBundle>,
    confidential_policy_hash: [u8; 32],
    creation_time_ms: Option<u64>,
) -> Result<(RawGenesisTransaction, GenesisBlock), color_eyre::eyre::Error> {
    let mut parameters = genesis.sumeragi_v2_context_parameters();
    let (nexus_amx_context_hash, execution_policy_hash) = staged_sumeragi_v2_context_hashes(
        &genesis,
        genesis_key_pair,
        config,
        da_proof_policies.as_ref(),
        confidential_policy_hash,
        creation_time_ms,
    )?;
    parameters.nexus_amx_context_hash = nexus_amx_context_hash.into();
    parameters.execution_policy_hash = execution_policy_hash.into();

    let bound_manifest = genesis
        .with_sumeragi_v2_context_parameters(parameters)
        .with_consensus_meta();
    let block = build_signed_genesis(
        bound_manifest.clone(),
        genesis_key_pair,
        da_proof_policies,
        confidential_policy_hash,
        creation_time_ms,
    )?;
    Ok((bound_manifest, block))
}

/// Stage a raw genesis transaction and return its exact Nexus/AMX consensus
/// and execution-policy commitments without committing state or touching
/// persistent node storage.
fn staged_sumeragi_v2_context_hashes(
    genesis: &RawGenesisTransaction,
    genesis_key_pair: &KeyPair,
    config: Option<&actual::Root>,
    da_proof_policies: Option<&DaProofPolicyBundle>,
    confidential_policy_hash: [u8; 32],
    creation_time_ms: Option<u64>,
) -> Result<(iroha_crypto::Hash, iroha_crypto::Hash), color_eyre::eyre::Error> {
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .name("kagami-genesis-staging".to_owned())
            .stack_size(16 * 1024 * 1024)
            .spawn_scoped(scope, move || {
                staged_sumeragi_v2_context_hashes_on_bounded_stack(
                    genesis,
                    genesis_key_pair,
                    config,
                    da_proof_policies,
                    confidential_policy_hash,
                    creation_time_ms,
                )
            })
            .wrap_err("spawn bounded genesis staging thread")?
            .join()
            .map_err(|_| eyre!("bounded genesis staging thread panicked"))?
    })
}

/// Re-stage an already authenticated signed genesis body against one effective
/// validator configuration.
///
/// Prepared-bundle admission uses this path so every runtime config must
/// reproduce the exact Nexus/AMX and execution-policy commitments signed into
/// genesis without requiring or reloading the retired genesis private key.
pub(crate) fn staged_signed_sumeragi_v2_context_hashes(
    genesis: &RawGenesisTransaction,
    signed: &SignedBlock,
    config: &actual::Root,
) -> Result<(iroha_crypto::Hash, iroha_crypto::Hash), color_eyre::eyre::Error> {
    let provisional = GenesisBlock(signed.clone());
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .name("kagami-prepared-genesis-staging".to_owned())
            .stack_size(16 * 1024 * 1024)
            .spawn_scoped(scope, move || {
                staged_sumeragi_v2_context_hashes_from_provisional_on_bounded_stack(
                    genesis,
                    Some(config),
                    provisional,
                )
            })
            .wrap_err("spawn bounded prepared-genesis staging thread")?
            .join()
            .map_err(|_| eyre!("bounded prepared-genesis staging thread panicked"))?
    })
}

fn staged_sumeragi_v2_context_hashes_on_bounded_stack(
    genesis: &RawGenesisTransaction,
    genesis_key_pair: &KeyPair,
    config: Option<&actual::Root>,
    da_proof_policies: Option<&DaProofPolicyBundle>,
    confidential_policy_hash: [u8; 32],
    creation_time_ms: Option<u64>,
) -> Result<(iroha_crypto::Hash, iroha_crypto::Hash), color_eyre::eyre::Error> {
    // This worker is a new thread, so it does not inherit the caller's
    // thread-local I105 discriminant.
    let _chain_discriminant = staged_genesis_chain_discriminant(genesis);
    let provisional = build_signed_genesis(
        genesis.clone().with_consensus_meta(),
        genesis_key_pair,
        da_proof_policies.cloned(),
        confidential_policy_hash,
        creation_time_ms,
    )?;
    staged_sumeragi_v2_context_hashes_from_provisional_on_bounded_stack(
        genesis,
        config,
        provisional,
    )
}

fn staged_sumeragi_v2_context_hashes_from_provisional_on_bounded_stack(
    genesis: &RawGenesisTransaction,
    config: Option<&actual::Root>,
    provisional: GenesisBlock,
) -> Result<(iroha_crypto::Hash, iroha_crypto::Hash), color_eyre::eyre::Error> {
    let _chain_discriminant = staged_genesis_chain_discriminant(genesis);
    let consensus_mode = match genesis.consensus_mode() {
        SumeragiConsensusMode::Permissioned => WireConsensusMode::Permissioned,
        SumeragiConsensusMode::Npos => WireConsensusMode::Npos,
    };
    let authority = provisional
        .0
        .external_transactions()
        .next()
        .and_then(|transaction| transaction.authority().try_signatory())
        .cloned()
        .map(AccountId::new)
        .ok_or_else(|| {
            eyre!("prepared genesis authority must be one canonical single-key account")
        })?;
    let mut world = World::with(
        [Domain::new(iroha_genesis::GENESIS_DOMAIN_ID.clone()).build(&authority)],
        [Account::new(authority.clone()).build(&authority)],
        [],
    );
    let default_nexus;
    let dataspace_catalog = if let Some(config) = config {
        &config.nexus.dataspace_catalog
    } else {
        default_nexus = actual::Nexus::default();
        &default_nexus.dataspace_catalog
    };
    // Match fresh-node and `irohad --check-config` semantics exactly: genesis aliases are
    // pre-seeded before the block executes so declarative EnsureAlias instructions repair
    // derived state without charging or depending on policy activation order.
    iroha_core::sns::seed_genesis_alias_bootstrap(&mut world, &provisional.0, dataspace_catalog);
    let kura = match config {
        Some(config) => Kura::new_temporary_with_configured_lane_catalog(
            &config.kura,
            &config.nexus.lane_config,
            &config.nexus.configured_lane_catalog,
        )
        .map_err(|error| eyre!("initialize isolated Kura for staged genesis: {error}"))?,
        None => Kura::blank_kura_for_testing(),
    };
    let mut state = State::try_new_with_chain_with_default_telemetry(
        world,
        Arc::clone(&kura),
        LiveQueryStore::start_test(),
        genesis.chain_id().clone(),
    )
    .map_err(|error| eyre!("initialize isolated State for staged genesis: {error}"))?;
    configure_staged_genesis_state(&mut state, genesis, config)?;
    install_staged_nexus_policies(&mut state, genesis, config)?;

    let voters = iroha_core::sumeragi::signed_genesis_voting_peers(&provisional)
        .map_err(|error| eyre!("invalid signed Sumeragi v2 genesis roster: {error}"))?;
    if voters.is_empty() {
        return Err(eyre!(
            "Sumeragi v2 genesis roster is empty; inject BLS topology entries and PoPs before signing"
        ));
    }
    let topology = Topology::new(voters);
    let mut voting_block: Option<VotingBlock> = None;
    let (_valid, staged) = ValidBlock::validate_signed_genesis_keep_voting_block(
        provisional.0,
        &topology,
        genesis.chain_id(),
        &authority,
        &TimeSource::new_system(),
        &state,
        &mut voting_block,
        consensus_mode,
    )
    .unpack(|_| {})
    .map_err(|(block, error)| {
        let transaction_errors = (0..block.external_transactions().count())
            .filter_map(|index| {
                block
                    .error(index)
                    .map(|reason| format!("transaction[{index}]: {reason:?}"))
            })
            .collect::<Vec<_>>();
        if transaction_errors.is_empty() {
            eyre!("staged genesis execution failed: {error}")
        } else {
            eyre!(
                "staged genesis execution failed: {error}; {}",
                transaction_errors.join("; ")
            )
        }
    })?;
    let nexus_amx_context_hash =
        iroha_core::sumeragi::staged_genesis_nexus_amx_context_hash(&staged);
    let execution_policy_hash = iroha_core::sumeragi::staged_genesis_execution_policy_hash(&staged)
        .map_err(|error| eyre!("derive staged genesis execution policy: {error}"))?;
    drop(staged);
    Ok((nexus_amx_context_hash, execution_policy_hash))
}

fn staged_genesis_chain_discriminant(genesis: &RawGenesisTransaction) -> ChainDiscriminantGuard {
    ChainDiscriminantGuard::enter(genesis.chain_discriminant())
}

fn staged_lane_manifest_registry(
    genesis: &RawGenesisTransaction,
    nexus: &actual::Nexus,
) -> Result<LaneManifestRegistry, color_eyre::eyre::Error> {
    // Genesis construction can enter additional I105 scopes. Reassert the manifest
    // discriminant at the exact filesystem-parse boundary so validator accounts
    // cannot fall back to the process-global SORA prefix.
    let _chain_discriminant = staged_genesis_chain_discriminant(genesis);
    let registry =
        LaneManifestRegistry::from_config(&nexus.lane_catalog, &nexus.governance, &nexus.registry);
    registry
        .validate_active_coverage()
        .map_err(|error| eyre!("invalid lane manifest registry for staged genesis: {error}"))?;
    Ok(registry)
}

fn staged_genesis_pipeline(mut pipeline: actual::Pipeline) -> actual::Pipeline {
    // Keep offline genesis execution on the guarded staging worker so nested
    // account parsing cannot fall back to the process-global discriminant.
    pipeline.workers = 1;
    pipeline
}

fn staged_default_account_literal(
    literal: &str,
    target_discriminant: u16,
    label: &'static str,
) -> Result<String, color_eyre::eyre::Error> {
    let address = AccountAddress::from_i105_for_discriminant(
        literal,
        Some(defaults::common::chain_discriminant()),
    )
    .map_err(|error| eyre!("invalid built-in {label}: {error}"))?;
    address
        .to_i105_for_discriminant(target_discriminant)
        .map_err(|error| eyre!("re-encode built-in {label} for staged genesis: {error}"))
}

fn staged_default_nexus(
    genesis: &RawGenesisTransaction,
) -> Result<actual::Nexus, color_eyre::eyre::Error> {
    let discriminant = genesis.chain_discriminant();
    let mut nexus = actual::Nexus::default();
    nexus.staking.stake_escrow_account_id = staged_default_account_literal(
        &nexus.staking.stake_escrow_account_id,
        discriminant,
        "nexus.staking.stake_escrow_account_id",
    )?;
    nexus.staking.slash_sink_account_id = staged_default_account_literal(
        &nexus.staking.slash_sink_account_id,
        discriminant,
        "nexus.staking.slash_sink_account_id",
    )?;
    nexus.fees.fee_sink_account_id = staged_default_account_literal(
        &nexus.fees.fee_sink_account_id,
        discriminant,
        "nexus.fees.fee_sink_account_id",
    )?;
    if let Some(authority) = nexus.relay_worker.authority_account_id.as_mut() {
        *authority = staged_default_account_literal(
            authority,
            discriminant,
            "nexus.relay_worker.authority_account_id",
        )?;
    }
    Ok(nexus)
}

fn staged_default_pipeline(
    genesis: &RawGenesisTransaction,
) -> Result<actual::Pipeline, color_eyre::eyre::Error> {
    let mut pipeline = actual::Pipeline::default();
    pipeline.gas.tech_account_id = staged_default_account_literal(
        &pipeline.gas.tech_account_id,
        genesis.chain_discriminant(),
        "pipeline.gas.tech_account_id",
    )?;
    Ok(staged_genesis_pipeline(pipeline))
}

fn configure_staged_genesis_state(
    state: &mut State,
    genesis: &RawGenesisTransaction,
    config: Option<&actual::Root>,
) -> Result<(), color_eyre::eyre::Error> {
    if let Some(config) = config {
        state.set_pipeline(staged_genesis_pipeline(config.pipeline.clone()));
        state.set_oracle(config.oracle.clone());
        state.set_fraud_monitoring(config.fraud_monitoring.clone());
        state.set_gov(config.gov.clone());
        state.content = config.content.clone();
        state.set_settlement(config.settlement.clone());
        state.set_kagemusha_release_catalog(
            iroha_core::smartcontracts::isi::offline::KagemushaReleaseCatalogV4::from_offline_config(
                &config.settlement.offline,
            )
            .map_err(|error| {
                eyre!("invalid Kagemusha release catalog for staged genesis: {error}")
            })?,
        );
        state
            .set_zk(config.zk.clone())
            .map_err(|error| eyre!("invalid ZK config for staged genesis: {error}"))?;
        state
            .prepare_configured_primary_geometry_anchor(&config.nexus.configured_lane_catalog)
            .map_err(|error| eyre!("invalid primary Nexus geometry for staged genesis: {error}"))?;
        state
            .restore_kura_lane_segments_before_startup_replay()
            .map_err(|error| eyre!("restore staged genesis primary Nexus geometry: {error}"))?;
        state
            .set_nexus_from_config(config.nexus.clone())
            .map_err(|error| eyre!("invalid Nexus config for staged genesis: {error}"))?;
        state.set_crypto(config.crypto.clone());
    } else {
        state.set_pipeline(staged_default_pipeline(genesis)?);
        state
            .set_nexus(staged_default_nexus(genesis)?)
            .map_err(|error| eyre!("invalid default Nexus config: {error}"))?;
        state.set_crypto(actual::Crypto::default());
    }
    Ok(())
}

fn install_staged_nexus_policies(
    state: &mut State,
    genesis: &RawGenesisTransaction,
    config: Option<&actual::Root>,
) -> Result<(), color_eyre::eyre::Error> {
    let nexus = state.nexus_snapshot();
    let lane_compliance = match config {
        Some(config) if config.nexus.compliance.enabled => {
            let policy_dir = config.nexus.compliance.policy_dir.as_ref().ok_or_else(|| {
                eyre!("lane compliance is enabled but no policy_dir is configured")
            })?;
            let engine = LaneComplianceEngine::from_directory(
                policy_dir,
                config.nexus.compliance.audit_only,
            )
            .map_err(|error| eyre!("load staged genesis lane compliance policies: {error}"))?;
            engine
                .validate_active_catalog(&nexus.lane_catalog)
                .map_err(|error| {
                    eyre!("validate staged genesis lane compliance policies: {error}")
                })?;
            Some(Arc::new(engine))
        }
        _ => None,
    };
    state.install_lane_compliance_engine(lane_compliance);

    let lane_manifests = if nexus.enabled {
        staged_lane_manifest_registry(genesis, &nexus)?
    } else {
        LaneManifestRegistry::empty().rebind(&nexus.lane_catalog, &nexus.governance)
    };
    state.install_lane_manifests(&Arc::new(lane_manifests));
    Ok(())
}

fn should_auto_bootstrap_npos_validators(config: Option<&actual::Root>) -> bool {
    let Some(config) = config else {
        return true;
    };

    matches!(
        config
            .nexus
            .staking
            .validator_mode(LaneId::SINGLE, &config.nexus.lane_catalog),
        actual::LaneValidatorMode::StakeElected
    )
}

impl<T: Write> RunArgs<T> for Args {
    #[allow(clippy::too_many_lines)]
    fn run(self, writer: &mut BufWriter<T>) -> Outcome {
        tui::status("Signing genesis manifest");
        if let Some(path) = self.out_file.as_deref() {
            reject_legacy_scale_out_file(path)?;
        }
        if let (Some(signed), Some(bound)) =
            (self.out_file.as_deref(), self.bound_manifest_out.as_deref())
            && signed == bound
        {
            return Err(eyre!(
                "signed genesis output and bound manifest output must use different paths"
            ));
        }
        if let Some(expected_hash) = self.expected_hash_out.as_deref() {
            let deployment_identity = deployment_identity_path(expected_hash);
            if deployment_identity == expected_hash {
                return Err(eyre!(
                    "genesis expected-hash output must not use the reserved `.identity.toml` deployment-identity path"
                ));
            }
            for (label, path) in [
                ("signed genesis output", self.out_file.as_deref()),
                ("bound manifest output", self.bound_manifest_out.as_deref()),
                ("input genesis manifest", Some(self.genesis_file.as_path())),
            ] {
                if path == Some(expected_hash) {
                    return Err(eyre!(
                        "genesis expected-hash output and {label} must use different paths"
                    ));
                }
                if path == Some(deployment_identity.as_path()) {
                    return Err(eyre!(
                        "paired deployment-identity output and {label} must use different paths"
                    ));
                }
            }
        }
        let build_line = build_line_from_env();
        let consensus_mode_override = self.consensus_mode.map(SumeragiConsensusMode::from);

        let mut genesis = RawGenesisTransaction::from_path(&self.genesis_file)?;
        // Keep every same-thread rebuild, config parse, and bound-manifest
        // serialization on the manifest's network. Staged execution re-enters
        // this discriminant on its worker thread.
        let _chain_discriminant = staged_genesis_chain_discriminant(&genesis);
        // Parse the peer configuration exactly once. Every policy projection
        // below borrows this immutable snapshot, so replacing the source file
        // concurrently cannot create a mixed genesis generation.
        let peer_config = self.config.as_deref().map(load_peer_config).transpose()?;
        if let Some(config) = peer_config.as_ref() {
            ensure_peer_config_matches_manifest(config, &genesis)?;
        }
        let manifest_consensus_mode = genesis.consensus_mode();
        require_v2_wire_protocol_only(&genesis)?;
        let consensus_mode = consensus_mode_override.unwrap_or(manifest_consensus_mode);
        if build_line.is_iroha3() {
            validate_consensus_mode_for_line(build_line, consensus_mode, ConsensusPolicy::Any)?;
        }
        if self.topology.is_some() {
            genesis = genesis.clear_topology();
        }
        if matches!(consensus_mode, SumeragiConsensusMode::Npos) {
            ensure_npos_parameters(&genesis)?;
        }
        let topology_override = if let Some(raw) = self.topology.as_deref() {
            Some(norito::json::from_str::<Vec<PeerId>>(raw).wrap_err("parse --topology JSON")?)
        } else {
            None
        };
        let final_topology = topology_override
            .clone()
            .unwrap_or_else(|| collect_topology_peers(&genesis));
        ensure_valid_genesis_committee(&final_topology)?;
        let uses_npos = matches!(consensus_mode, SumeragiConsensusMode::Npos);
        let auto_bootstrap_npos = should_auto_bootstrap_npos_validators(peer_config.as_ref());
        let topology_peers = if uses_npos {
            final_topology
        } else {
            Vec::new()
        };
        let needs_npos_bootstrap = uses_npos
            && auto_bootstrap_npos
            && !manifest_has_npos_bootstrap(&genesis)
            && !topology_peers.is_empty();
        let mut bootstrap_registrations = if needs_npos_bootstrap {
            BootstrapRegistrations::from_manifest(&genesis)
        } else {
            BootstrapRegistrations {
                domains: BTreeSet::new(),
                accounts: BTreeSet::new(),
                asset_defs: BTreeSet::new(),
            }
        };
        let bootstrap_stake_asset_id = if needs_npos_bootstrap {
            configured_npos_bootstrap_stake_asset_id(&genesis, peer_config.as_ref())?
        } else {
            default_npos_bootstrap_stake_asset_id()
        };
        if self.topology.is_none() && !self.peer_pops.is_empty() {
            return Err(eyre!(
                "--peer-pop requires --topology to align PoPs with peers"
            ));
        }
        let genesis_key_pair = load_genesis_key(
            self.private_key.as_deref(),
            self.private_key_file.as_deref(),
            self.seed.as_deref(),
            self.algorithm,
        )?;
        ensure_expected_public_key(&genesis_key_pair, self.expected_public_key.as_ref())?;
        let da_proof_policies = resolve_da_proof_policies(peer_config.as_ref());
        let confidential_policy_hash = resolve_confidential_policy_hash(peer_config.as_ref());
        if let Some(config) = peer_config.as_ref()
            && config.genesis.public_key != *genesis_key_pair.public_key()
        {
            return Err(eyre!(
                "genesis signing key does not match the public key pinned by --config"
            ));
        }
        let bootstrap_escrow_account_id = if needs_npos_bootstrap {
            Some(configured_npos_bootstrap_escrow_account_id(
                &genesis,
                peer_config.as_ref(),
            )?)
        } else {
            None
        };
        let direct_sign_safe = topology_override.is_none() && !needs_npos_bootstrap;
        let prepared_genesis = if direct_sign_safe {
            genesis.with_consensus_mode(consensus_mode)
        } else {
            let chain_discriminant = genesis.chain_discriminant();
            let mut builder = genesis.into_builder();

            if let Some(topology) = topology_override.as_ref() {
                // Put topology into a dedicated transaction so it remains separate
                // from other genesis instructions.
                let entries = build_topology_entries(topology, &self.peer_pops)?;
                builder = builder.next_transaction().set_topology(entries);
            }
            if needs_npos_bootstrap {
                let escrow_account_id = bootstrap_escrow_account_id
                    .as_ref()
                    .expect("NPoS bootstrap escrow resolved above");
                builder = append_npos_bootstrap(
                    builder,
                    &mut bootstrap_registrations,
                    &topology_peers,
                    escrow_account_id,
                    &bootstrap_stake_asset_id,
                )?;
            }

            builder
                .build_raw()
                .with_chain_discriminant(chain_discriminant)
                .with_consensus_mode(consensus_mode)
                .with_consensus_meta()
        };
        ensure_valid_genesis_committee(&collect_topology_peers(&prepared_genesis))?;
        let (bound_manifest, genesis_block) = bind_and_sign_staged_sumeragi_v2_context(
            prepared_genesis,
            &genesis_key_pair,
            peer_config.as_ref(),
            da_proof_policies,
            confidential_policy_hash,
            self.creation_time_ms,
        )?;

        let framed = genesis_block
            .0
            .encode_wire()
            .wrap_err("frame genesis block with Norito header")?;
        let bound_manifest_json = self
            .bound_manifest_out
            .as_ref()
            .map(|_| {
                norito::json::to_vec_pretty(&bound_manifest)
                    .wrap_err("encode config-bound genesis manifest")
            })
            .transpose()?;

        let genesis_expected_hash = genesis_block.0.hash();
        eprintln!("Genesis public key: {}", genesis_key_pair.public_key());
        eprintln!("Genesis expected hash: {genesis_expected_hash}");

        let mut writer: Box<dyn Write> = match self.out_file {
            None => Box::new(writer),
            Some(path) => Box::new(BufWriter::new(File::create(path)?)),
        };
        writer.write_all(&framed)?;
        writer.flush()?;

        if let (Some(path), Some(json)) = (
            self.bound_manifest_out.as_deref(),
            bound_manifest_json.as_deref(),
        ) {
            fs::write(path, json).wrap_err_with(|| {
                format!("write config-bound genesis manifest to {}", path.display())
            })?;
        }
        if let Some(path) = self.expected_hash_out.as_deref() {
            fs::write(path, format!("{genesis_expected_hash}\n"))
                .wrap_err_with(|| format!("write genesis expected hash to {}", path.display()))?;
            let identity_path = deployment_identity_path(path);
            publish_deployment_identity(&identity_path, &genesis_expected_hash.to_string())?;
        }
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
    private_key_file: Option<&Path>,
    seed: Option<&str>,
    algorithm: Algorithm,
) -> Result<KeyPair, color_eyre::eyre::Error> {
    match (private_key_hex, private_key_file, seed) {
        (Some(hex), None, None) => {
            let sk = PrivateKey::from_hex(algorithm, hex).wrap_err("decode genesis private key")?;
            KeyPair::from_private_key(sk).wrap_err("derive genesis key pair from private key")
        }
        (None, Some(path), None) => load_genesis_key_file(path, algorithm),
        (None, None, Some(seed)) => {
            let seed = crate::crypto::parse_keygen_seed_hex(seed)?;
            KeyPair::try_from_seed(seed, algorithm).wrap_err("derive seeded genesis key pair")
        }
        (None, None, None) => Err(eyre!(
            "genesis signing requires a private key; pass --private-key-file, --private-key, or --seed-hex"
        )),
        _ => unreachable!("clap enforces key-source conflicts"),
    }
}

fn ensure_expected_public_key(
    key_pair: &KeyPair,
    expected_public_key: Option<&PublicKey>,
) -> Result<(), color_eyre::eyre::Error> {
    if let Some(expected_public_key) = expected_public_key
        && key_pair.public_key() != expected_public_key
    {
        return Err(eyre!(
            "genesis signing key does not match --expected-public-key"
        ));
    }
    Ok(())
}

fn load_genesis_key_file(
    path: &Path,
    algorithm: Algorithm,
) -> Result<KeyPair, color_eyre::eyre::Error> {
    let mut raw = zeroize::Zeroizing::new(crate::secure_fs::read_private_file(path)?);
    let text =
        std::str::from_utf8(raw.as_slice()).wrap_err("genesis private-key file is not UTF-8")?;
    let canonical = text.strip_suffix('\n').ok_or_else(|| {
        eyre!("genesis private-key file must contain one canonical key and a final newline")
    })?;
    if canonical.is_empty()
        || canonical.chars().any(char::is_whitespace)
        || format!("{canonical}\n").as_bytes() != raw.as_slice()
    {
        return Err(eyre!(
            "genesis private-key file is not one canonical key record"
        ));
    }
    let exposed = canonical
        .parse::<ExposedPrivateKey>()
        .wrap_err("decode canonical genesis private-key file")?;
    if exposed.to_string() != canonical || exposed.0.algorithm() != algorithm {
        return Err(eyre!(
            "genesis private-key file encoding or algorithm is not canonical"
        ));
    }
    raw.zeroize();
    KeyPair::from_private_key(exposed.0).wrap_err("derive genesis key pair from private-key file")
}

fn build_topology_entries(
    topology: &[PeerId],
    peer_pops: &[String],
) -> Result<Vec<GenesisTopologyEntry>, color_eyre::eyre::Error> {
    use iroha_crypto::PublicKey;

    ensure_valid_genesis_committee(topology)?;

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

fn ensure_valid_genesis_committee(topology: &[PeerId]) -> Result<(), color_eyre::eyre::Error> {
    let unique = topology.iter().collect::<BTreeSet<_>>();
    if unique.len() != topology.len() {
        return Err(eyre!(
            "genesis topology contains duplicate voting peer identities"
        ));
    }
    if !is_valid_committee_size(unique.len()) {
        return Err(eyre!(
            "genesis topology must contain an exact Sumeragi v2 `3f + 1` validator committee \
             in the supported range 4..={MAX_VALIDATORS_PER_HEIGHT} (saw {})",
            unique.len()
        ));
    }
    Ok(())
}

fn resolve_da_proof_policies(config: Option<&actual::Root>) -> Option<DaProofPolicyBundle> {
    config.map(|config| iroha_core::da::proof_policy_bundle(&config.nexus.lane_config))
}

fn resolve_confidential_policy_hash(config: Option<&actual::Root>) -> [u8; 32] {
    config.map_or_else(
        iroha_core::state::default_genesis_confidential_policy_hash,
        |config| iroha_core::state::compute_genesis_confidential_policy_hash(&config.zk),
    )
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
        str::FromStr,
    };

    use super::*;
    use iroha_crypto::{KeyPair as CryptoKeyPair, bls_normal_pop_prove};
    use iroha_data_model::{
        ChainId,
        asset::AssetDefinitionAlias,
        block::{SignedBlock, decode_framed_signed_block},
        isi::{
            SetParameter, asset_alias::SetAssetDefinitionAlias, mint_burn::MintBox,
            register::RegisterBox, staking::RegisterPublicLaneValidator,
        },
        nexus::{LaneCatalog, LaneConfig},
        parameter::{
            Parameter,
            system::{
                SumeragiConsensusMode, SumeragiNposParameters, SumeragiParameter,
                consensus_metadata,
            },
        },
        transaction::Executable,
    };
    use iroha_genesis::{GenesisBuilder, GenesisTopologyEntry};

    fn checked_in_config(path: &std::path::Path) -> actual::Root {
        if let Ok(config) = load_peer_config(path) {
            return config;
        }

        // Some deployment templates deliberately contain unresolved
        // runtime-secret bindings outside this projection. Reparse only the
        // Nexus and Pipeline tables consumed by the Nexus/AMX commitment,
        // while retaining the source chain/discriminant, through the
        // production config parser. No Nexus or Pipeline value is synthesized
        // or decoded by this test helper.
        let source = fs::read_to_string(path).expect("read checked-in config");
        let header = || {
            source
                .lines()
                .take_while(|line| !line.trim_start().starts_with('['))
        };
        let chain = header()
            .find(|line| line.trim_start().starts_with("chain ="))
            .expect("checked-in config has chain");
        let chain_discriminant = header()
            .find(|line| line.trim_start().starts_with("chain_discriminant ="))
            .unwrap_or("");
        let mut projected = format!(
            r#"{chain}
{chain_discriminant}
public_key = "ea01309060D021340617E9554CCBC2CF3CC3DB922A9BA323ABDF7C271FCC6EF69BE7A8DEBCA7D9E96C0F0089ABA22CDAADE4A2"
private_key = "8926201CA347641228C3B79AA43839DEDC85FA51C0E8B9B6A00F6B0D6B0423E902973F"
soranet_transport_public_key = "ed0120D9F6AEF1813164294D1D9C0662FEB9C7F7861B4DFFE385680331093DA4ABD10B"
soranet_transport_private_key = "802620134C4527B3852AE2218A8F079B301C651EAD8C7567B96BD7A9BE8DB366E46B89"
trusted_peers_pop = [
  {{ public_key = "ea01309060D021340617E9554CCBC2CF3CC3DB922A9BA323ABDF7C271FCC6EF69BE7A8DEBCA7D9E96C0F0089ABA22CDAADE4A2", pop_hex = "8515da750f81182aaba5c22fc9f03a01e81ed85e4495a2ca6b29a71c0c8549537e31e79cddf6ff285b9e22d0d9dc17ce0f46e7d0cf78b2ef9feab50c849a1ea8e1e4f07e966f6113faa8a999317545d9f111b8e08a7273913710b43a20b19c08" }}
]

[network]
address = "addr:127.0.0.1:1337#8F78"
public_address = "addr:127.0.0.1:1337#8F78"

[torii]
address = "addr:127.0.0.1:8080#8942"

[genesis]
public_key = "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
expected_hash = "hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"

[streaming]
identity_public_key = "ed01208BA62848CF767D72E7F7F4B9D2D7BA07FEE33760F79ABE5597A51520E292A0CB"
identity_private_key = "8026208F4C15E5D664DA3F13778801D23D4E89B76E94C1B94B389544168B6CB894F84F"
"#
        );
        let mut include = false;
        for line in source.lines() {
            let trimmed = line.trim_start();
            if trimmed.starts_with('[') {
                let section = trimmed.trim_start_matches('[');
                include = section.starts_with("nexus") || section.starts_with("pipeline");
            }
            if include {
                projected.push_str(line);
                projected.push('\n');
            }
        }
        let table = toml::Table::from_str(&projected).unwrap_or_else(|error| {
            panic!(
                "failed to parse consensus projection from {}: {error}",
                path.display()
            )
        });
        actual::Root::from_toml_source(TomlSource::inline(table)).unwrap_or_else(|error| {
            panic!(
                "failed to validate consensus projection from {}: {error}",
                path.display()
            )
        })
    }

    type ConsensusHandshakeMetaTest =
        iroha_data_model::parameter::system::ConsensusHandshakeMetadata;

    fn consensus_handshake_meta(block: &SignedBlock) -> ConsensusHandshakeMetaTest {
        for transaction in block.external_transactions() {
            if let Executable::Instructions(batch) = transaction.instructions() {
                for instruction in batch {
                    if let Some(set_parameter) = instruction.as_any().downcast_ref::<SetParameter>()
                        && let Parameter::Custom(custom) = set_parameter.inner()
                        && custom.id() == &consensus_metadata::handshake_meta_id()
                    {
                        return custom
                            .payload()
                            .try_into_any()
                            .expect("decode signed consensus handshake metadata");
                    }
                }
            }
        }
        panic!("signed genesis omitted consensus handshake metadata");
    }

    fn assert_genesis_signatures_verify(block: &SignedBlock, genesis_key_pair: &KeyPair) {
        let transactions = block.external_transactions().collect::<Vec<_>>();
        assert!(
            !transactions.is_empty(),
            "signed genesis must contain external transactions"
        );
        for transaction in transactions {
            transaction
                .verify_signature()
                .expect("genesis transaction signature must verify");
        }

        let signatures = block.signatures().collect::<Vec<_>>();
        assert!(
            !signatures.is_empty(),
            "signed genesis must have a block signature"
        );
        for signature in signatures {
            signature
                .signature()
                .verify_hash(genesis_key_pair.public_key(), block.hash())
                .expect("genesis block signature must verify");
        }
    }

    fn sign_checked_in_profile(
        root: &std::path::Path,
        genesis_path: &str,
        config_path: &str,
    ) -> ConsensusHandshakeMetaTest {
        sign_checked_in_profile_with_optional_config(root, genesis_path, Some(config_path))
    }

    fn sign_checked_in_profile_with_optional_config(
        root: &std::path::Path,
        genesis_path: &str,
        config_path: Option<&str>,
    ) -> ConsensusHandshakeMetaTest {
        const SAMPLE_GENESIS_PRIVATE_KEY: &str =
            "82B3BDE54AEBECA4146257DA0DE8D59D8E46D5FE34887DCD8072866792FCB3AD";
        const DEV_GENESIS_SEED: &str =
            "435e15ae8cca7d8ce54313e1ed60b98bd3be7832bbaa6e33b358068b61f6ea97";
        const NEXUS_GENESIS_SEED: &str =
            "ac74a176f589853d5a7488ae8c04caee8b99b408a98c1d2971b3d525e9f0e91c";
        let (private_key, seed) = match genesis_path {
            "defaults/kagami/iroha3-dev/genesis.json" => (None, Some(DEV_GENESIS_SEED.to_owned())),
            "defaults/kagami/iroha3-nexus/genesis.json" => {
                (None, Some(NEXUS_GENESIS_SEED.to_owned()))
            }
            "defaults/genesis.json"
            | "defaults/kagami/iroha3-taira/genesis.json"
            | "defaults/nexus/genesis.json"
            | "configs/soranexus/nexus/genesis.json" => {
                (Some(SAMPLE_GENESIS_PRIVATE_KEY.to_owned()), None)
            }
            _ => (Some(test_private_key_hex()), None),
        };
        let args = Args {
            genesis_file: root.join(genesis_path),
            out_file: None,
            bound_manifest_out: None,
            expected_hash_out: None,
            topology: None,
            peer_pops: Vec::new(),
            private_key,
            private_key_file: None,
            expected_public_key: None,
            seed,
            creation_time_ms: None,
            algorithm: Algorithm::Ed25519,
            config: config_path.map(|path| root.join(path)),
            consensus_mode: None,
        };
        let mut writer = BufWriter::new(Vec::new());
        args.run(&mut writer)
            .unwrap_or_else(|error| panic!("failed to sign {genesis_path}: {error:#}"));
        let bytes = writer.into_inner().expect("flush signed genesis buffer");
        let block = decode_framed_signed_block(&bytes)
            .unwrap_or_else(|error| panic!("failed to decode signed {genesis_path}: {error}"));

        consensus_handshake_meta(&block)
    }

    #[test]
    fn checked_in_genesis_templates_are_current_and_parse() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        for path in [
            "defaults/genesis.json",
            "defaults/kagami/iroha3-dev/genesis.json",
            "defaults/kagami/iroha3-nexus/genesis.json",
            "defaults/kagami/iroha3-taira/genesis.json",
            "defaults/nexus/genesis.json",
            "configs/soranexus/nexus/genesis.json",
            "configs/soranexus/taira/genesis.json",
        ] {
            let manifest = RawGenesisTransaction::from_path(root.join(path))
                .unwrap_or_else(|error| panic!("checked-in {path} must parse: {error:#}"));
            assert_eq!(manifest.wire_protocol_version(), 4, "{path}");
            ensure_npos_parameters(&manifest).unwrap_or_else(|error| {
                panic!("checked-in {path} has invalid NPoS policy: {error}")
            });
            let context = manifest.sumeragi_v2_context_parameters();
            assert_ne!(context.nexus_amx_context_hash, [0; 32], "{path}");
            assert_ne!(context.execution_policy_hash, [0; 32], "{path}");
            let refreshed = manifest.clone().with_consensus_meta();
            assert_eq!(
                manifest.consensus_fingerprint(),
                refreshed.consensus_fingerprint(),
                "{path} has a stale consensus fingerprint"
            );
        }
    }

    #[test]
    fn checked_in_unsigned_templates_use_canonical_nexus_amx_projection() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let fixtures = [
            (
                "defaults/kagami/iroha3-nexus/genesis.json",
                "defaults/kagami/iroha3-nexus/config.toml",
                true,
            ),
            (
                "defaults/nexus/genesis.json",
                "defaults/nexus/config.toml",
                false,
            ),
            (
                "configs/soranexus/nexus/genesis.json",
                "configs/soranexus/nexus/config.toml",
                false,
            ),
            (
                "configs/soranexus/taira/genesis.json",
                "configs/soranexus/taira/config.toml",
                false,
            ),
        ];
        for (genesis_path, config_path, has_sample_topology) in fixtures {
            let manifest = RawGenesisTransaction::from_path(root.join(genesis_path))
                .unwrap_or_else(|error| panic!("checked-in {genesis_path} must parse: {error:#}"));
            let has_topology = manifest
                .transactions()
                .iter()
                .any(|transaction| !transaction.topology().is_empty());
            assert_eq!(has_topology, has_sample_topology, "{genesis_path}");
            let config = checked_in_config(&root.join(config_path));
            let expected = actual::sumeragi_v2_nexus_amx_context_hash(
                &config.nexus,
                &config.pipeline,
                &[],
                &[],
            );
            assert_eq!(
                manifest
                    .sumeragi_v2_context_parameters()
                    .nexus_amx_context_hash,
                <[u8; 32]>::from(expected),
                "{genesis_path} must carry the config-only projection; a deployable signer rebinds it after the final roster and operator-supplied public XOR identity are present"
            );
        }
    }

    #[test]
    fn checked_in_taira_source_template_requires_runtime_signer_rendering() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let path = root.join("configs/soranexus/taira/config.toml");
        let source = fs::read_to_string(&path).expect("read Taira deployment template");
        assert!(source.contains("production_mode = true"));
        for placeholder in [
            "REPLACE_WITH_SORACLOUD_RUNTIME_SIGNER_HANDLE",
            "REPLACE_WITH_SORACLOUD_RUNTIME_SIGNER_AUTHORITY",
            "REPLACE_WITH_SORACLOUD_RUNTIME_SIGNER_ALGORITHM",
            "REPLACE_WITH_SORACLOUD_RUNTIME_SIGNER_PUBLIC_KEY_HEX",
            "REPLACE_WITH_SORACLOUD_RUNTIME_SIGNER_REVISION",
            "REPLACE_WITH_SORACLOUD_RUNTIME_SIGNER_POLICY_DIGEST_HEX",
        ] {
            assert!(
                source.contains(placeholder),
                "Taira source template must retain explicit `{placeholder}`"
            );
        }
        assert!(
            load_peer_config(&path).is_err(),
            "unrendered Taira production template must not be runnable"
        );
    }

    #[test]
    fn signing_profile_hash_placeholder_is_never_a_runtime_trust_root() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let path = root.join("defaults/kagami/iroha3-dev/config.toml");
        let runtime_source = TomlSource::from_file(&path).expect("read signing profile");

        assert!(
            actual::Root::from_toml_source(runtime_source).is_err(),
            "the unresolved signing profile must not normalize as a runnable node config"
        );
        load_peer_config(&path)
            .expect("the genesis signer may project policy through the explicit placeholder");
    }

    #[test]
    fn checked_in_profile_commitments_match_production_signing() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let fixtures = [
            (
                "defaults/kagami/iroha3-dev/genesis.json",
                "defaults/kagami/iroha3-dev/config.toml",
            ),
            (
                "defaults/kagami/iroha3-taira/genesis.json",
                "defaults/kagami/iroha3-taira/config.toml",
            ),
        ];
        for (genesis_path, config_path) in fixtures {
            let manifest = RawGenesisTransaction::from_path(root.join(genesis_path))
                .unwrap_or_else(|error| panic!("checked-in {genesis_path} must parse: {error:#}"));
            let signed = sign_checked_in_profile(&root, genesis_path, config_path);
            assert_eq!(signed.wire_protocol_version, 4, "{genesis_path}");
            assert_eq!(
                signed.sumeragi_v2,
                manifest.sumeragi_v2_context_parameters(),
                "{genesis_path} must carry the exact context produced by the production signing path"
            );
            assert_eq!(
                Some(signed.consensus_fingerprint),
                manifest.consensus_fingerprint(),
                "{genesis_path} fingerprint must cover the exact staged context"
            );
        }
    }

    #[test]
    #[ignore = "read-only maintainer utility for refreshing generated profile commitments"]
    fn print_checked_in_profile_commitments() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        // The operator-facing Taira template pins a deliberately undistributed genesis key, so
        // it can only be rebound by the deployment signer that owns that key.
        for (genesis_path, config_path) in [
            ("defaults/genesis.json", None),
            (
                "defaults/kagami/iroha3-dev/genesis.json",
                Some("defaults/kagami/iroha3-dev/config.toml"),
            ),
            (
                "defaults/kagami/iroha3-nexus/genesis.json",
                Some("defaults/kagami/iroha3-nexus/config.toml"),
            ),
            (
                "defaults/kagami/iroha3-taira/genesis.json",
                Some("defaults/kagami/iroha3-taira/config.toml"),
            ),
            (
                "defaults/nexus/genesis.json",
                Some("defaults/nexus/config.toml"),
            ),
            (
                "configs/soranexus/nexus/genesis.json",
                Some("configs/soranexus/nexus/config.toml"),
            ),
        ] {
            let signed =
                sign_checked_in_profile_with_optional_config(&root, genesis_path, config_path);
            eprintln!(
                "{genesis_path}: {} {} {}",
                hex::encode(signed.sumeragi_v2.nexus_amx_context_hash),
                hex::encode(signed.sumeragi_v2.execution_policy_hash),
                signed.consensus_fingerprint
            );
        }
    }

    fn checked_genesis_sign_keypair() -> CryptoKeyPair {
        CryptoKeyPair::try_random().expect("genesis sign fixture key generation should succeed")
    }

    fn checked_genesis_sign_keypair_with_algorithm(algorithm: Algorithm) -> CryptoKeyPair {
        CryptoKeyPair::try_random_with_algorithm(algorithm)
            .expect("genesis sign fixture key generation should succeed")
    }

    fn valid_test_topology(count: usize) -> (Vec<PeerId>, Vec<String>) {
        let materials = (0..count)
            .map(|_| {
                let key_pair = checked_genesis_sign_keypair_with_algorithm(Algorithm::BlsNormal);
                let pop = bls_normal_pop_prove(key_pair.private_key())
                    .expect("generate checked topology proof of possession");
                let peer = PeerId::new(key_pair.public_key().clone());
                let encoded_pop = format!("{}={}", peer.public_key(), hex::encode(pop));
                (peer, encoded_pop)
            })
            .collect::<Vec<_>>();
        materials.into_iter().unzip()
    }

    fn valid_test_topology_entries(count: usize) -> Vec<GenesisTopologyEntry> {
        (0..count)
            .map(|_| {
                let key_pair = checked_genesis_sign_keypair_with_algorithm(Algorithm::BlsNormal);
                let pop = bls_normal_pop_prove(key_pair.private_key())
                    .expect("generate checked topology proof of possession");
                GenesisTopologyEntry::new(PeerId::new(key_pair.public_key().clone()), pop)
            })
            .collect()
    }

    fn replace_manifest_wire_protocol_version(
        path: &std::path::Path,
        version: norito::json::Value,
    ) {
        let bytes = fs::read(path).expect("read genesis fixture");
        let mut value: norito::json::Value =
            norito::json::from_slice(&bytes).expect("parse genesis fixture JSON");
        value
            .as_object_mut()
            .expect("genesis fixture is an object")
            .insert("wire_protocol_version".to_owned(), version);
        fs::write(
            path,
            norito::json::to_json_pretty(&value).expect("serialize genesis fixture JSON"),
        )
        .expect("rewrite genesis fixture");
    }

    #[test]
    fn signing_rejects_retired_protocol_version_arrays() {
        for versions in [Vec::new(), vec![1], vec![1, 2], vec![2, 1], vec![2, 2]] {
            let genesis_file = minimal_genesis_file();
            replace_manifest_wire_protocol_version(
                &genesis_file,
                norito::json::value::to_value(&versions).expect("serialize protocol list"),
            );
            let args = Args {
                genesis_file,
                out_file: None,
                bound_manifest_out: None,
                expected_hash_out: None,
                topology: None,
                peer_pops: Vec::new(),
                private_key: Some(test_private_key_hex()),
                private_key_file: None,
                expected_public_key: None,
                seed: None,
                creation_time_ms: None,
                algorithm: Algorithm::Ed25519,
                config: None,
                consensus_mode: None,
            };
            let error = args
                .run(&mut BufWriter::new(Vec::new()))
                .expect_err("non-canonical protocol list must be rejected before signing");
            assert!(
                error
                    .to_string()
                    .contains("failed to deserialize raw genesis transaction"),
                "unexpected error for {versions:?}: {error}"
            );
        }
    }

    #[test]
    fn signing_rejects_protocol_downgrades_and_unknown_future_versions() {
        let current_args = Args {
            genesis_file: minimal_genesis_file(),
            out_file: None,
            bound_manifest_out: None,
            expected_hash_out: None,
            topology: None,
            peer_pops: Vec::new(),
            private_key: Some(test_private_key_hex()),
            private_key_file: None,
            expected_public_key: None,
            seed: None,
            creation_time_ms: None,
            algorithm: Algorithm::Ed25519,
            config: None,
            consensus_mode: None,
        };
        current_args
            .run(&mut BufWriter::new(Vec::new()))
            .expect("current scalar protocol version 4 must be accepted before signing");

        for version in [0_u32, 1, 2, u32::MAX] {
            let genesis_file = minimal_genesis_file();
            replace_manifest_wire_protocol_version(
                &genesis_file,
                norito::json::Value::Number(norito::json::Number::U64(u64::from(version))),
            );
            let args = Args {
                genesis_file,
                out_file: None,
                bound_manifest_out: None,
                expected_hash_out: None,
                topology: None,
                peer_pops: Vec::new(),
                private_key: Some(test_private_key_hex()),
                private_key_file: None,
                expected_public_key: None,
                seed: None,
                creation_time_ms: None,
                algorithm: Algorithm::Ed25519,
                config: None,
                consensus_mode: None,
            };
            let error = args.run(&mut BufWriter::new(Vec::new())).expect_err(
                "retired or unknown future scalar protocol version must be rejected before signing",
            );
            assert!(
                error
                    .to_string()
                    .contains("fresh genesis must advertise wire_protocol_version = 4"),
                "unexpected error for protocol version {version}: {error}"
            );
        }
    }

    #[test]
    fn explicit_creation_time_repeats_signed_wire_bytes() {
        let genesis_file = minimal_genesis_file();
        let private_key = test_private_key_hex();
        let sign = || {
            let args = Args {
                genesis_file: genesis_file.clone(),
                out_file: None,
                bound_manifest_out: None,
                expected_hash_out: None,
                topology: None,
                peer_pops: Vec::new(),
                private_key: Some(private_key.clone()),
                private_key_file: None,
                expected_public_key: None,
                seed: None,
                creation_time_ms: Some(1_700_000_000_000),
                algorithm: Algorithm::Ed25519,
                config: None,
                consensus_mode: None,
            };
            let mut writer = BufWriter::new(Vec::new());
            args.run(&mut writer).expect("sign at fixed creation time");
            writer.into_inner().expect("extract signed genesis bytes")
        };

        assert_eq!(sign(), sign());
    }

    #[test]
    fn genesis_sign_fixture_key_generation_preserves_algorithms() {
        assert_eq!(
            checked_genesis_sign_keypair().public_key().algorithm(),
            Algorithm::default()
        );
        for algorithm in [Algorithm::Ed25519, Algorithm::BlsNormal] {
            assert_eq!(
                checked_genesis_sign_keypair_with_algorithm(algorithm)
                    .public_key()
                    .algorithm(),
                algorithm
            );
        }
    }

    #[test]
    fn peer_pops_without_topology_is_rejected() {
        let args = Args {
            genesis_file: npos_genesis_file(),
            out_file: None,
            bound_manifest_out: None,
            expected_hash_out: None,
            topology: None,
            peer_pops: vec!["pk=00".to_string()],
            private_key: Some(test_private_key_hex()),
            private_key_file: None,
            expected_public_key: None,
            seed: None,
            creation_time_ms: None,
            algorithm: Algorithm::Ed25519,
            config: None,
            consensus_mode: None,
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
        let (topology, mut peer_pops) = valid_test_topology(4);
        let topology_json = norito::json::to_json(&topology).unwrap();
        let dup = peer_pops[0].clone();
        peer_pops.push(dup.clone());
        let args = Args {
            genesis_file: npos_genesis_file(),
            out_file: None,
            bound_manifest_out: None,
            expected_hash_out: None,
            topology: Some(topology_json),
            peer_pops,
            private_key: Some(test_private_key_hex()),
            private_key_file: None,
            expected_public_key: None,
            seed: None,
            creation_time_ms: None,
            algorithm: Algorithm::Ed25519,
            config: None,
            consensus_mode: None,
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
        let (topology, peer_pops) = valid_test_topology(4);
        let entries = build_topology_entries(&topology, &peer_pops).expect("valid pops");
        assert_eq!(
            entries[0].peer, topology[0],
            "entries should respect topology order"
        );
        assert_eq!(
            entries[1].peer, topology[1],
            "entries should respect topology order"
        );
    }

    #[test]
    fn signing_boundary_enforces_bounded_committee_geometry() {
        let (topology, _) = valid_test_topology(32);
        for count in [1_usize, 2, 3, 5, 32] {
            let error = ensure_valid_genesis_committee(&topology[..count])
                .expect_err("non-committee topology must fail");
            assert!(
                error.to_string().contains("exact Sumeragi v2 `3f + 1`"),
                "unexpected error for {count} peers: {error}"
            );
        }

        for count in [4_usize, 7] {
            ensure_valid_genesis_committee(&topology[..count])
                .unwrap_or_else(|error| panic!("{count}-peer topology failed: {error}"));
        }
    }

    #[test]
    fn out_file_scale_extension_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("genesis.scale");
        let args = Args {
            genesis_file: npos_genesis_file(),
            out_file: Some(path),
            bound_manifest_out: None,
            expected_hash_out: None,
            topology: None,
            peer_pops: Vec::new(),
            private_key: Some(test_private_key_hex()),
            private_key_file: None,
            expected_public_key: None,
            seed: None,
            creation_time_ms: None,
            algorithm: Algorithm::Ed25519,
            config: None,
            consensus_mode: None,
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
    #[expect(
        clippy::too_many_lines,
        reason = "the regression test audits the complete bound-manifest write/sign contract as one transaction"
    )]
    fn bound_manifest_output_matches_the_manifest_used_for_signing() {
        let temp = tempfile::tempdir().expect("bound manifest temp dir");
        let bound_manifest_path = temp.path().join("genesis.bound.json");
        let genesis_file = minimal_genesis_file();
        let seed = "41".repeat(32);
        let genesis_key_pair = KeyPair::try_from_seed(
            crate::crypto::parse_keygen_seed_hex(&seed).expect("decode fixture seed"),
            Algorithm::Ed25519,
        )
        .expect("derive genesis signing key");
        let unbound_manifest = RawGenesisTransaction::from_path(&genesis_file)
            .expect("parse unbound genesis manifest");
        let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let config_source =
            fs::read_to_string(workspace_root.join("defaults/kagami/iroha3-dev/config.toml"))
                .expect("read current peer config fixture");
        let mut config_table = config_source
            .parse::<toml::Table>()
            .expect("parse current peer config fixture");
        config_table
            .get_mut("genesis")
            .and_then(toml::Value::as_table_mut)
            .expect("peer config genesis table")
            .insert(
                "public_key".to_owned(),
                toml::Value::String(genesis_key_pair.public_key().to_string()),
            );
        let nexus = config_table
            .get_mut("nexus")
            .and_then(toml::Value::as_table_mut)
            .expect("peer config nexus table");
        nexus.insert("enabled".to_owned(), toml::Value::Boolean(false));
        nexus.insert("lane_count".to_owned(), toml::Value::Integer(1));
        config_table
            .entry("pipeline")
            .or_insert_with(|| toml::Value::Table(toml::Table::new()))
            .as_table_mut()
            .expect("peer config pipeline table")
            .insert(
                "amx_per_instruction_ns".to_owned(),
                toml::Value::Integer(51),
            );
        let (topology, peer_pops) = valid_test_topology(4);
        let config_path = temp.path().join("peer0.toml");
        fs::write(
            &config_path,
            toml::to_string_pretty(&config_table).expect("render disabled-Nexus peer config"),
        )
        .expect("write disabled-Nexus peer config");
        let parsed_config =
            load_peer_config(&config_path).expect("load disabled-Nexus peer config");
        assert!(!parsed_config.nexus.enabled);
        let args = Args {
            genesis_file,
            out_file: None,
            bound_manifest_out: Some(bound_manifest_path.clone()),
            expected_hash_out: None,
            topology: Some(norito::json::to_json(&topology).expect("serialize topology override")),
            peer_pops,
            private_key: None,
            private_key_file: None,
            expected_public_key: None,
            seed: Some(seed),
            creation_time_ms: None,
            algorithm: Algorithm::Ed25519,
            config: Some(config_path),
            consensus_mode: None,
        };

        let mut writer = BufWriter::new(Vec::new());
        args.run(&mut writer).expect("sign genesis");
        writer.flush().expect("flush signed genesis");
        let signed =
            decode_framed_signed_block(&writer.into_inner().expect("extract signed genesis bytes"))
                .expect("decode signed genesis");
        let bound_manifest = RawGenesisTransaction::from_path(&bound_manifest_path)
            .expect("parse config-bound manifest output");
        let signed_meta = consensus_handshake_meta(&signed);

        assert_eq!(
            bound_manifest.sumeragi_v2_context_parameters(),
            signed_meta.sumeragi_v2,
            "persisted manifest must carry the exact staged Nexus/AMX context signed into the block"
        );
        assert_eq!(
            bound_manifest.consensus_fingerprint(),
            Some(signed_meta.consensus_fingerprint),
            "persisted manifest fingerprint must match the signed handshake metadata"
        );
        assert_ne!(
            bound_manifest
                .sumeragi_v2_context_parameters()
                .nexus_amx_context_hash,
            unbound_manifest
                .sumeragi_v2_context_parameters()
                .nexus_amx_context_hash,
            "disabled-Nexus peer config and its AMX policy must replace the generator's unbound context commitment"
        );
        assert_ne!(
            bound_manifest.consensus_fingerprint(),
            unbound_manifest.consensus_fingerprint(),
            "rebinding the Nexus/AMX context must refresh the persisted fingerprint"
        );
        assert_genesis_signatures_verify(&signed, &genesis_key_pair);

        let rebuilt = bound_manifest
            .build_and_sign_with_confidential_policy_hash(
                &genesis_key_pair,
                Some(iroha_core::state::default_genesis_confidential_policy_hash()),
            )
            .expect("rebuild persisted bound manifest");
        let signed_instructions = signed
            .external_transactions()
            .map(|transaction| transaction.instructions().clone())
            .collect::<Vec<_>>();
        let rebuilt_instructions = rebuilt
            .0
            .external_transactions()
            .map(|transaction| transaction.instructions().clone())
            .collect::<Vec<_>>();
        assert_eq!(
            signed_instructions, rebuilt_instructions,
            "bound manifest output must reproduce the signed transaction semantics"
        );
    }

    #[test]
    fn failed_signing_does_not_clobber_bound_manifest_output() {
        let temp = tempfile::tempdir().expect("bound manifest temp dir");
        let bound_manifest_path = temp.path().join("genesis.bound.json");
        let sentinel = b"existing-bound-manifest";
        fs::write(&bound_manifest_path, sentinel).expect("write sentinel manifest");
        let args = Args {
            genesis_file: npos_genesis_file(),
            out_file: None,
            bound_manifest_out: Some(bound_manifest_path.clone()),
            expected_hash_out: None,
            topology: Some("not valid json".to_owned()),
            peer_pops: Vec::new(),
            private_key: Some(test_private_key_hex()),
            private_key_file: None,
            expected_public_key: None,
            seed: None,
            creation_time_ms: None,
            algorithm: Algorithm::Ed25519,
            config: None,
            consensus_mode: None,
        };

        let _ = args
            .run(&mut BufWriter::new(Vec::new()))
            .expect_err("invalid topology must fail before publishing outputs");
        assert_eq!(
            fs::read(&bound_manifest_path).expect("read sentinel manifest"),
            sentinel,
            "failed signing must leave an existing bound manifest untouched"
        );
    }

    #[test]
    fn signed_output_failure_does_not_publish_bound_manifest() {
        let temp = tempfile::tempdir().expect("output publication temp dir");
        let bound_manifest_path = temp.path().join("genesis.bound.json");
        let sentinel = b"existing-bound-manifest";
        fs::write(&bound_manifest_path, sentinel).expect("write sentinel manifest");
        let args = Args {
            genesis_file: minimal_genesis_file(),
            out_file: Some(temp.path().join("missing-parent/genesis.signed.nrt")),
            bound_manifest_out: Some(bound_manifest_path.clone()),
            expected_hash_out: None,
            topology: None,
            peer_pops: Vec::new(),
            private_key: Some(test_private_key_hex()),
            private_key_file: None,
            expected_public_key: None,
            seed: None,
            creation_time_ms: None,
            algorithm: Algorithm::Ed25519,
            config: None,
            consensus_mode: None,
        };

        let _ = args
            .run(&mut BufWriter::new(Vec::new()))
            .expect_err("uncreatable signed output must fail publication");
        assert_eq!(
            fs::read(&bound_manifest_path).expect("read sentinel manifest"),
            sentinel,
            "bound manifest must publish only after the signed block output succeeds"
        );
    }

    #[test]
    fn signed_and_bound_manifest_outputs_must_not_alias() {
        let temp = tempfile::tempdir().expect("output alias temp dir");
        let output_path = temp.path().join("genesis-output.nrt");
        let sentinel = b"existing-output";
        fs::write(&output_path, sentinel).expect("write output sentinel");
        let args = Args {
            genesis_file: minimal_genesis_file(),
            out_file: Some(output_path.clone()),
            bound_manifest_out: Some(output_path.clone()),
            expected_hash_out: None,
            topology: None,
            peer_pops: Vec::new(),
            private_key: Some(test_private_key_hex()),
            private_key_file: None,
            expected_public_key: None,
            seed: None,
            creation_time_ms: None,
            algorithm: Algorithm::Ed25519,
            config: None,
            consensus_mode: None,
        };

        let error = args
            .run(&mut BufWriter::new(Vec::new()))
            .expect_err("signed and manifest outputs must not alias");
        assert!(
            error.to_string().contains("must use different paths"),
            "unexpected output alias error: {error:#}"
        );
        assert_eq!(
            fs::read(&output_path).expect("read output sentinel"),
            sentinel,
            "output alias rejection must happen before either output is opened"
        );
    }

    #[test]
    fn expected_hash_output_matches_the_signed_consensus_header() {
        let temp = tempfile::tempdir().expect("expected hash output temp dir");
        let expected_hash_path = temp.path().join("genesis.expected_hash");
        let identity_path = deployment_identity_path(&expected_hash_path);
        let args = Args {
            genesis_file: minimal_genesis_file(),
            out_file: None,
            bound_manifest_out: None,
            expected_hash_out: Some(expected_hash_path.clone()),
            topology: None,
            peer_pops: Vec::new(),
            private_key: Some(test_private_key_hex()),
            private_key_file: None,
            expected_public_key: None,
            seed: None,
            creation_time_ms: None,
            algorithm: Algorithm::Ed25519,
            config: None,
            consensus_mode: None,
        };

        let mut writer = BufWriter::new(Vec::new());
        args.run(&mut writer)
            .expect("minimal genesis signing must succeed");
        let block = decode_framed_signed_block(
            &writer
                .into_inner()
                .expect("flush signed genesis output buffer"),
        )
        .expect("decode signed genesis output");

        assert_eq!(
            fs::read_to_string(expected_hash_path).expect("read expected hash output"),
            format!("{}\n", block.hash()),
        );
        assert_eq!(
            fs::read_to_string(identity_path).expect("read paired deployment identity"),
            format!(
                "network_id = \"{}\"\n\n[genesis]\nexpected_hash = \"{}\"\n",
                block.hash(),
                block.hash()
            ),
        );
    }

    #[test]
    fn deployment_identity_publication_is_idempotent_and_refuses_drift() {
        let temp = tempfile::tempdir().expect("deployment identity temp dir");
        let identity_path = temp.path().join("genesis.identity.toml");
        let expected_hash = Hash::new(b"deployment identity").to_string();
        let different_hash = Hash::new(b"different deployment identity").to_string();

        publish_deployment_identity(&identity_path, &expected_hash)
            .expect("publish deployment identity");
        let published = fs::read(&identity_path).expect("read deployment identity");
        publish_deployment_identity(&identity_path, &expected_hash)
            .expect("same deployment identity must be idempotent");
        publish_deployment_identity(&identity_path, &different_hash)
            .expect_err("different deployment identity must not replace the trust root");

        assert_eq!(
            fs::read(&identity_path).expect("reread deployment identity"),
            published,
        );
        assert!(
            fs::read_dir(temp.path())
                .expect("list deployment identity directory")
                .all(|entry| {
                    !entry
                        .expect("deployment identity directory entry")
                        .file_name()
                        .to_string_lossy()
                        .starts_with(".genesis-identity-")
                }),
            "atomic publication must not retain temporary files"
        );
    }

    #[test]
    fn load_genesis_key_accepts_seed_and_algorithm() {
        let seed = "42".repeat(32);
        let kp = load_genesis_key(None, None, Some(&seed), Algorithm::Secp256k1)
            .expect("seed path should work");
        assert_eq!(
            kp.public_key()
                .try_algorithm()
                .expect("fixture public key must be valid"),
            Algorithm::Secp256k1
        );
    }

    #[test]
    fn expected_public_key_must_match_selected_private_key() {
        let selected = KeyPair::try_from_seed(vec![7_u8; 32], Algorithm::Ed25519)
            .expect("selected fixture key");
        let different = KeyPair::try_from_seed(vec![8_u8; 32], Algorithm::Ed25519)
            .expect("different fixture key");

        ensure_expected_public_key(&selected, Some(selected.public_key()))
            .expect("matching public key must be accepted");
        let error = ensure_expected_public_key(&selected, Some(different.public_key()))
            .expect_err("mismatched public key must be rejected");
        assert!(
            error.to_string().contains("--expected-public-key"),
            "mismatch error should identify the failed invariant: {error:#}"
        );
    }

    #[test]
    fn run_returns_err_on_invalid_topology_json() {
        let args = Args {
            genesis_file: npos_genesis_file(),
            out_file: None,
            bound_manifest_out: None,
            expected_hash_out: None,
            topology: Some("not valid json".to_owned()),
            peer_pops: Vec::new(),
            private_key: Some(test_private_key_hex()),
            private_key_file: None,
            expected_public_key: None,
            seed: None,
            creation_time_ms: None,
            algorithm: Algorithm::Ed25519,
            config: None,
            consensus_mode: None,
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
            bound_manifest_out: None,
            expected_hash_out: None,
            topology: None,
            peer_pops: Vec::new(),
            private_key: None,
            private_key_file: None,
            expected_public_key: None,
            seed: None,
            creation_time_ms: None,
            algorithm: Algorithm::Ed25519,
            config: None,
            consensus_mode: None,
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
            bound_manifest_out: None,
            expected_hash_out: None,
            topology: None,
            peer_pops: Vec::new(),
            private_key: Some(test_private_key_hex()),
            private_key_file: None,
            expected_public_key: None,
            seed: None,
            creation_time_ms: None,
            algorithm: Algorithm::Ed25519,
            config: None,
            consensus_mode: None,
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
            bound_manifest_out: None,
            expected_hash_out: None,
            topology: None,
            peer_pops: Vec::new(),
            private_key: Some(test_private_key_hex()),
            private_key_file: None,
            expected_public_key: None,
            seed: None,
            creation_time_ms: None,
            algorithm: Algorithm::Ed25519,
            config: None,
            consensus_mode: Some(ConsensusModeArg::Permissioned),
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
        let (topology, peer_pops) = valid_test_topology(4);
        let topology_json = norito::json::to_json(&topology).unwrap();

        // Provide PoP only for the first peer to trigger the missing-pop validation.
        let args = Args {
            genesis_file,
            out_file: None,
            bound_manifest_out: None,
            expected_hash_out: None,
            topology: Some(topology_json),
            peer_pops: vec![peer_pops[0].clone()],
            private_key: Some(test_private_key_hex()),
            private_key_file: None,
            expected_public_key: None,
            seed: None,
            creation_time_ms: None,
            algorithm: Algorithm::Ed25519,
            config: None,
            consensus_mode: None,
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

        let existing_kp = checked_genesis_sign_keypair_with_algorithm(Algorithm::BlsNormal);
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
        .with_consensus_mode(SumeragiConsensusMode::Npos)
        .with_consensus_meta();
        let json = norito::json::to_json_pretty(&manifest).expect("serialize genesis manifest");
        fs::write(genesis_file.path(), json).expect("write genesis json");

        let (new_peers, peer_pops) = valid_test_topology(4);
        let topology_json = norito::json::to_json(&new_peers).unwrap();

        let args = Args {
            genesis_file: genesis_file.path().to_path_buf(),
            out_file: None,
            bound_manifest_out: None,
            expected_hash_out: None,
            topology: Some(topology_json),
            peer_pops,
            private_key: Some(test_private_key_hex()),
            private_key_file: None,
            expected_public_key: None,
            seed: None,
            creation_time_ms: None,
            algorithm: Algorithm::Ed25519,
            config: None,
            consensus_mode: None,
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
            registered_peers, new_peers,
            "expected topology override to replace existing entries"
        );
    }

    #[test]
    fn sign_without_manifest_mutations_preserves_direct_manifest_payload() {
        use std::num::NonZeroU64;

        use iroha_crypto::{Algorithm, KeyPair};
        use iroha_data_model::parameter::BlockParameter;

        let manifest = GenesisBuilder::new_without_executor(
            ChainId::from("sign-direct-manifest-regression"),
            ".",
        )
        .append_parameter(Parameter::Sumeragi(SumeragiParameter::MaxClockDriftMs(100)))
        .append_parameter(Parameter::Block(BlockParameter::MaxTransactions(
            NonZeroU64::new(512).expect("non-zero"),
        )))
        .next_transaction()
        .append_parameter(Parameter::Sumeragi(SumeragiParameter::MaxClockDriftMs(333)))
        .set_topology(valid_test_topology_entries(4))
        .build_raw()
        .with_consensus_mode(SumeragiConsensusMode::Permissioned)
        .with_consensus_meta();

        let genesis_file = tempfile::NamedTempFile::new().expect("create temp genesis file");
        let json = norito::json::to_json_pretty(&manifest).expect("serialize genesis manifest");
        fs::write(genesis_file.path(), json).expect("write genesis json");

        let seed = "43".repeat(32);
        let key_pair = KeyPair::try_from_seed(
            crate::crypto::parse_keygen_seed_hex(&seed).expect("decode fixture seed"),
            Algorithm::Ed25519,
        )
        .expect("derive checked genesis fixture key");
        let expected = manifest
            .clone()
            .build_and_sign_with_confidential_policy_hash(
                &key_pair,
                Some(iroha_core::state::default_genesis_confidential_policy_hash()),
            )
            .expect("direct manifest signing should succeed");

        let args = Args {
            genesis_file: genesis_file.path().to_path_buf(),
            out_file: None,
            bound_manifest_out: None,
            expected_hash_out: None,
            topology: None,
            peer_pops: vec![],
            private_key: None,
            private_key_file: None,
            expected_public_key: None,
            seed: Some(seed),
            creation_time_ms: None,
            algorithm: Algorithm::Ed25519,
            config: None,
            consensus_mode: None,
        };

        let mut writer = BufWriter::new(Vec::new());
        args.run(&mut writer).expect("sign should succeed");
        writer.flush().expect("flush output");
        let bytes = writer.into_inner().expect("extract buffer");
        let actual = decode_framed_signed_block(&bytes).expect("decode signed block");

        let actual_instructions: Vec<_> = actual
            .external_transactions()
            .map(|tx| tx.instructions().clone())
            .collect();
        let expected_instructions: Vec<_> = expected
            .0
            .external_transactions()
            .map(|tx| tx.instructions().clone())
            .collect();
        assert_eq!(
            actual_instructions, expected_instructions,
            "signing an unchanged manifest should preserve parsed transaction payloads"
        );
        assert_eq!(
            actual.da_proof_policies(),
            expected.0.da_proof_policies(),
            "signing an unchanged manifest should preserve DA proof policies"
        );
        assert_eq!(
            actual.header().confidential_features(),
            expected.0.header().confidential_features(),
            "signing an unchanged manifest should commit the default genesis confidential policy"
        );
    }

    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "the integration regression exercises one generated Nexus localnet through complete peer-config resigning"
    )]
    fn generated_nexus_localnet_can_be_resigned_with_its_peer_config() {
        let temp = tempfile::tempdir().expect("create localnet output dir");
        let seed = "localnet-resign-confidential-policy".to_owned();
        let options = crate::localnet::LocalnetOptions {
            build_line: iroha_version::BuildLine::Iroha3,
            sora_profile: Some(crate::localnet::SoraProfile::Nexus),
            perf_profile: None,
            peers: std::num::NonZeroU16::new(4).expect("non-zero peer count"),
            seed: Some(seed.clone()),
            bind_host: crate::localnet::DEFAULT_BIND_HOST.to_owned(),
            public_host: crate::localnet::DEFAULT_PUBLIC_HOST.to_owned(),
            base_api_port: 31_080,
            base_p2p_port: 31_337,
            out_dir: temp.path().to_path_buf(),
            extra_accounts: 0,
            assets: Vec::new(),
            block_cadence_ms: None,
            consensus_mode: SumeragiConsensusMode::Npos,
        };
        crate::localnet::generate_localnet(&options, &mut BufWriter::new(Vec::new()))
            .expect("generate Nexus localnet");

        let generated_bytes = fs::read(temp.path().join("genesis.signed.nrt"))
            .expect("read generated signed genesis");
        let config_path = temp.path().join("peer0.toml");
        let config = load_peer_config(&config_path).expect("load generated peer config");
        let expected_policy =
            iroha_core::state::compute_genesis_confidential_policy_hash(&config.zk);
        let (genesis_public_key, genesis_private_key) = crate::localnet::generate_genesis_key_pair(
            Some(seed.as_bytes()),
            crate::localnet::GENESIS_SEED,
        )
        .expect("derive generated localnet genesis key");
        let (_, genesis_private_key_bytes) = genesis_private_key.0.to_bytes();
        let genesis_key_pair = KeyPair::new(genesis_public_key, genesis_private_key.0.clone())
            .expect("reconstruct generated localnet genesis key pair");
        let mut invalid_compliance_config = config.clone();
        invalid_compliance_config.nexus.compliance.enabled = true;
        invalid_compliance_config.nexus.compliance.policy_dir = None;
        let invalid_compliance_error = bind_and_sign_staged_sumeragi_v2_context(
            RawGenesisTransaction::from_path(temp.path().join("genesis.json"))
                .expect("reload generated genesis manifest"),
            &genesis_key_pair,
            Some(&invalid_compliance_config),
            Some(iroha_core::da::proof_policy_bundle(
                &invalid_compliance_config.nexus.lane_config,
            )),
            iroha_core::state::compute_genesis_confidential_policy_hash(
                &invalid_compliance_config.zk,
            ),
            None,
        )
        .expect_err("compliance-enabled staging must require a policy directory");
        assert!(
            invalid_compliance_error
                .to_string()
                .contains("lane compliance is enabled but no policy_dir is configured"),
            "unexpected compliance staging error: {invalid_compliance_error:#}"
        );
        let args = Args {
            genesis_file: temp.path().join("genesis.json"),
            out_file: None,
            bound_manifest_out: None,
            expected_hash_out: None,
            topology: None,
            peer_pops: vec![],
            private_key: Some(hex::encode(genesis_private_key_bytes)),
            private_key_file: None,
            expected_public_key: None,
            seed: None,
            creation_time_ms: None,
            algorithm: Algorithm::Ed25519,
            config: Some(config_path),
            consensus_mode: None,
        };

        let mut writer = BufWriter::new(Vec::new());
        args.run(&mut writer)
            .expect("re-sign generated localnet genesis with its peer config");
        writer.flush().expect("flush signed genesis");
        let bytes = writer.into_inner().expect("extract signed genesis");
        let generated_block =
            decode_framed_signed_block(&generated_bytes).expect("decode generated signed genesis");
        let block = decode_framed_signed_block(&bytes).expect("decode re-signed genesis");

        // Each signing invocation intentionally stamps a fresh creation-time base, so wire bytes,
        // transaction signatures, and the block signature are expected to differ. Compare the
        // complete consensus-bearing semantics and verify both independently signed artifacts.
        let generated_instructions = generated_block
            .external_transactions()
            .map(|transaction| transaction.instructions().clone())
            .collect::<Vec<_>>();
        let resigned_instructions = block
            .external_transactions()
            .map(|transaction| transaction.instructions().clone())
            .collect::<Vec<_>>();
        assert_eq!(generated_instructions.len(), resigned_instructions.len());
        for (batch_index, (generated, resigned)) in generated_instructions
            .iter()
            .zip(&resigned_instructions)
            .enumerate()
        {
            if generated != resigned {
                let generated_json = norito::json::to_json(generated)
                    .expect("encode generated instruction batch diagnostics");
                let resigned_json = norito::json::to_json(resigned)
                    .expect("encode re-signed instruction batch diagnostics");
                panic!(
                    "localnet generation and matching-key re-sign differ at instruction batch {batch_index}:\ngenerated: {generated_json}\nre-signed: {resigned_json}"
                );
            }
        }
        assert_eq!(
            generated_block.external_transactions().count(),
            block.external_transactions().count(),
            "localnet generation and matching-key re-sign must preserve transaction count"
        );
        let generated_topology_entries = generated_block
            .external_transactions()
            .filter_map(|transaction| match transaction.instructions() {
                Executable::Instructions(batch) => Some(batch),
                _ => None,
            })
            .flatten()
            .filter(|instruction| {
                instruction
                    .as_any()
                    .downcast_ref::<RegisterBox>()
                    .is_some_and(|register| matches!(register, RegisterBox::Peer(_)))
            })
            .count();
        let resigned_topology_entries = block
            .external_transactions()
            .filter_map(|transaction| match transaction.instructions() {
                Executable::Instructions(batch) => Some(batch),
                _ => None,
            })
            .flatten()
            .filter(|instruction| {
                instruction
                    .as_any()
                    .downcast_ref::<RegisterBox>()
                    .is_some_and(|register| matches!(register, RegisterBox::Peer(_)))
            })
            .count();
        assert_eq!(generated_topology_entries, options.peers.get() as usize);
        assert_eq!(generated_topology_entries, resigned_topology_entries);
        assert_eq!(
            generated_block.da_proof_policies(),
            block.da_proof_policies()
        );
        assert_eq!(
            generated_block.header().confidential_features(),
            block.header().confidential_features()
        );
        let generated_consensus_meta = consensus_handshake_meta(&generated_block);
        let resigned_consensus_meta = consensus_handshake_meta(&block);
        assert_eq!(generated_consensus_meta, resigned_consensus_meta);
        assert_ne!(
            generated_consensus_meta.sumeragi_v2.nexus_amx_context_hash, [0; 32],
            "staged Nexus/AMX context commitment must not be empty"
        );
        assert_genesis_signatures_verify(&generated_block, &genesis_key_pair);
        assert_genesis_signatures_verify(&block, &genesis_key_pair);
        assert_eq!(
            block
                .header()
                .confidential_features()
                .expect("re-signed genesis has confidential digest")
                .zk_policy_hash,
            Some(expected_policy),
        );
    }

    #[test]
    fn sign_auto_bootstraps_npos_validators_for_topology() {
        let genesis_file = npos_genesis_file();
        let manifest =
            RawGenesisTransaction::from_path(&genesis_file).expect("parse NPoS genesis fixture");
        let _chain_discriminant = staged_genesis_chain_discriminant(&manifest);
        let expected_escrow = configured_npos_bootstrap_escrow_account_id(&manifest, None)
            .expect("resolve default staking escrow");
        let private_key_hex = test_private_key_hex();
        let genesis_key_pair =
            load_genesis_key(Some(&private_key_hex), None, None, Algorithm::Ed25519)
                .expect("load fixture genesis signer");
        let legacy_orphan = AccountId::new(
            KeyPair::try_from_seed(
                genesis_key_pair
                    .public_key()
                    .to_string()
                    .bytes()
                    .chain(b"npos-escrow-account".iter().copied())
                    .collect(),
                Algorithm::default(),
            )
            .expect("derive legacy orphan identity")
            .public_key()
            .clone(),
        );
        let (peers, peer_pops) = valid_test_topology(4);
        let topology_json = norito::json::to_json(&peers).unwrap();
        let args = Args {
            genesis_file,
            out_file: None,
            bound_manifest_out: None,
            expected_hash_out: None,
            topology: Some(topology_json),
            peer_pops,
            private_key: Some(private_key_hex),
            private_key_file: None,
            expected_public_key: None,
            seed: None,
            creation_time_ms: None,
            algorithm: Algorithm::Ed25519,
            config: None,
            consensus_mode: None,
        };

        let mut writer = BufWriter::new(Vec::new());
        args.run(&mut writer).expect("sign should succeed");
        writer.flush().expect("flush output");
        let bytes = writer.into_inner().expect("extract buffer");
        let block = decode_framed_signed_block(&bytes).expect("decode signed block");

        let mut validators = std::collections::BTreeSet::new();
        let mut minted_asset_ids = std::collections::BTreeSet::new();
        let mut registered_accounts = std::collections::BTreeSet::new();
        for tx in block.external_transactions() {
            if let Executable::Instructions(instructions) = tx.instructions() {
                for instr in instructions {
                    if let Some(register) = instr.as_any().downcast_ref::<Register<Account>>() {
                        registered_accounts.insert(register.object.id.clone());
                    }
                    if let Some(register) =
                        instr.as_any().downcast_ref::<RegisterPublicLaneValidator>()
                    {
                        validators.insert(register.validator.clone());
                    }
                    if let Some(mint) = instr.as_any().downcast_ref::<MintBox>()
                        && let MintBox::Asset(mint_asset) = mint
                    {
                        minted_asset_ids.insert(mint_asset.destination.definition().clone());
                    }
                }
            }
        }

        let expected = peers
            .iter()
            .map(|peer| AccountId::new(peer.public_key().clone()))
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(
            validators, expected,
            "expected NPoS bootstrap to register topology validators"
        );
        assert!(
            minted_asset_ids.contains(&default_npos_bootstrap_stake_asset_id()),
            "private NPoS bootstrap should keep using the synthetic local stake asset"
        );
        assert!(
            registered_accounts.contains(&expected_escrow),
            "auto-bootstrap must register the escrow account selected by nexus.staking"
        );
        assert!(
            !registered_accounts.contains(&legacy_orphan),
            "auto-bootstrap must not emit an orphan account derived from public genesis data"
        );
    }

    fn nexus_profile_with_staking_overrides(overrides: &str) -> PathBuf {
        use std::fmt::Write as _;

        let config =
            fs::read_to_string(nexus_profile_config_path()).expect("read nexus profile config");
        let mut config_without_staking = String::new();
        let mut skipping_staking = false;
        for line in config.lines() {
            if line == "[nexus.staking]" {
                skipping_staking = true;
                continue;
            }
            if skipping_staking && line.starts_with('[') {
                skipping_staking = false;
            }
            if !skipping_staking {
                writeln!(config_without_staking, "{line}").expect("copy config line");
            }
        }
        writeln!(config_without_staking, "\n[nexus.staking]\n{overrides}")
            .expect("append staking overrides");
        let mut temp = tempfile::Builder::new()
            .prefix("kagami-nexus-profile-")
            .suffix(".toml")
            .tempfile()
            .expect("create temp config");
        write!(temp, "{config_without_staking}").expect("write temp config");
        let (_file, path) = temp.keep().expect("persist temp config");
        path
    }

    fn nexus_profile_with_validator_modes(public_mode: &str, restricted_mode: &str) -> PathBuf {
        nexus_profile_with_staking_overrides(&format!(
            "public_validator_mode = \"{public_mode}\"\nrestricted_validator_mode = \"{restricted_mode}\""
        ))
    }

    fn nexus_profile_with_stake_asset_id(stake_asset_id: &str) -> PathBuf {
        nexus_profile_with_staking_overrides(&format!(
            "public_validator_mode = \"stake_elected\"\nrestricted_validator_mode = \"admin_managed\"\nstake_asset_id = \"{stake_asset_id}\""
        ))
    }

    #[test]
    fn sign_auto_bootstraps_using_configured_alias_backed_stake_asset() {
        let (peers, peer_pops) = valid_test_topology(4);
        let topology_json = norito::json::to_json(&peers).unwrap();
        let configured_asset_id: AssetDefinitionId = "6TEAJqbb8oEPmLncoNiMRbLEK6tw"
            .parse()
            .expect("valid canonical asset id");
        let args = Args {
            genesis_file: alias_backed_npos_genesis_file(),
            out_file: None,
            bound_manifest_out: None,
            expected_hash_out: None,
            topology: Some(topology_json),
            peer_pops,
            private_key: Some(test_private_key_hex()),
            private_key_file: None,
            expected_public_key: None,
            seed: None,
            creation_time_ms: None,
            algorithm: Algorithm::Ed25519,
            config: Some(nexus_profile_with_stake_asset_id("xor#universal")),
            consensus_mode: None,
        };

        let mut writer = BufWriter::new(Vec::new());
        args.run(&mut writer).expect("sign should succeed");
        writer.flush().expect("flush output");
        let bytes = writer.into_inner().expect("extract buffer");
        let block = decode_framed_signed_block(&bytes).expect("decode signed block");

        let mut minted_asset_ids = std::collections::BTreeSet::new();
        let mut registered_asset_ids = std::collections::BTreeSet::new();
        for tx in block.external_transactions() {
            if let Executable::Instructions(instructions) = tx.instructions() {
                for instr in instructions {
                    if let Some(mint) = instr.as_any().downcast_ref::<MintBox>()
                        && let MintBox::Asset(mint_asset) = mint
                    {
                        minted_asset_ids.insert(mint_asset.destination.definition().clone());
                    }
                    if let Some(register) =
                        instr.as_any().downcast_ref::<Register<AssetDefinition>>()
                    {
                        registered_asset_ids.insert(register.object.id.clone());
                    }
                }
            }
        }

        assert!(
            minted_asset_ids.contains(&configured_asset_id),
            "expected bootstrap mint to target configured stake asset"
        );
        assert!(
            !registered_asset_ids.contains(&default_npos_bootstrap_stake_asset_id()),
            "alias-backed stake asset should not force the legacy localnet bootstrap asset"
        );
    }

    #[test]
    fn public_taira_auto_bootstrap_uses_alias_bound_xor_without_config() {
        let (peers, peer_pops) = valid_test_topology(4);
        let topology_json = norito::json::to_json(&peers).unwrap();
        let configured_asset_id: AssetDefinitionId = crate::genesis::TAIRA_XOR_ASSET_DEFINITION_ID
            .parse()
            .expect("valid canonical asset id");
        let bound_manifest = tempfile::NamedTempFile::new().expect("create bound manifest file");
        let args = Args {
            genesis_file: public_taira_alias_backed_npos_genesis_file(),
            out_file: None,
            bound_manifest_out: Some(bound_manifest.path().to_path_buf()),
            expected_hash_out: None,
            topology: Some(topology_json),
            peer_pops,
            private_key: Some(test_private_key_hex()),
            private_key_file: None,
            expected_public_key: None,
            seed: None,
            creation_time_ms: None,
            algorithm: Algorithm::Ed25519,
            config: None,
            consensus_mode: None,
        };

        let mut writer = BufWriter::new(Vec::new());
        args.run(&mut writer).expect("sign should succeed");
        writer.flush().expect("flush output");
        let bytes = writer.into_inner().expect("extract buffer");
        let block = decode_framed_signed_block(&bytes).expect("decode signed block");
        let rebound = RawGenesisTransaction::from_path(bound_manifest.path())
            .expect("parse config-bound Taira manifest");
        assert_eq!(rebound.chain_id().as_str(), "iroha3-taira");
        assert_eq!(
            rebound.chain_discriminant(),
            crate::genesis::profile::TAIRA_CHAIN_DISCRIMINANT,
            "NPoS bootstrap rebuild must preserve Taira's I105 network prefix"
        );

        let mut minted_asset_ids = std::collections::BTreeSet::new();
        let mut registered_asset_ids = std::collections::BTreeSet::new();
        for tx in block.external_transactions() {
            if let Executable::Instructions(instructions) = tx.instructions() {
                for instr in instructions {
                    if let Some(mint) = instr.as_any().downcast_ref::<MintBox>()
                        && let MintBox::Asset(mint_asset) = mint
                    {
                        minted_asset_ids.insert(mint_asset.destination.definition().clone());
                    }
                    if let Some(register) =
                        instr.as_any().downcast_ref::<Register<AssetDefinition>>()
                    {
                        registered_asset_ids.insert(register.object.id.clone());
                    }
                }
            }
        }

        assert!(
            minted_asset_ids.contains(&configured_asset_id),
            "public Taira bootstrap should mint to canonical XOR"
        );
        assert!(
            !registered_asset_ids.contains(&default_npos_bootstrap_stake_asset_id()),
            "public Taira bootstrap must not register the synthetic NPoS stake asset"
        );
    }

    #[test]
    fn public_nexus_auto_bootstrap_requires_xor_alias_binding() {
        let (peers, peer_pops) = valid_test_topology(4);
        let topology_json = norito::json::to_json(&peers).unwrap();
        let args = Args {
            genesis_file: public_nexus_npos_genesis_file_without_xor_alias(),
            out_file: None,
            bound_manifest_out: None,
            expected_hash_out: None,
            topology: Some(topology_json),
            peer_pops,
            private_key: Some(test_private_key_hex()),
            private_key_file: None,
            expected_public_key: None,
            seed: None,
            creation_time_ms: None,
            algorithm: Algorithm::Ed25519,
            config: None,
            consensus_mode: None,
        };

        let mut writer = BufWriter::new(Vec::new());
        let err = args
            .run(&mut writer)
            .expect_err("public Nexus without XOR binding should fail");
        assert!(
            err.to_string().contains(crate::genesis::PUBLIC_XOR_ALIAS),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn public_taira_auto_bootstrap_rejects_configured_stake_asset_that_bypasses_xor_binding() {
        let (peers, peer_pops) = valid_test_topology(4);
        let topology_json = norito::json::to_json(&peers).unwrap();
        let args = Args {
            genesis_file: public_taira_alias_backed_npos_genesis_file(),
            out_file: None,
            bound_manifest_out: None,
            expected_hash_out: None,
            topology: Some(topology_json),
            peer_pops,
            private_key: Some(test_private_key_hex()),
            private_key_file: None,
            expected_public_key: None,
            seed: None,
            creation_time_ms: None,
            algorithm: Algorithm::Ed25519,
            config: Some(nexus_profile_with_stake_asset_id(
                "61CtjvNd9T3THAR65GsMVHr82Bjc",
            )),
            consensus_mode: None,
        };

        let mut writer = BufWriter::new(Vec::new());
        let err = args
            .run(&mut writer)
            .expect_err("public stake config must match XOR alias binding");
        assert!(
            err.to_string()
                .contains("must match the canonical XOR binding"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn public_taira_auto_bootstrap_rejects_conflicting_xor_alias_bindings() {
        let (peers, peer_pops) = valid_test_topology(4);
        let topology_json = norito::json::to_json(&peers).unwrap();
        let args = Args {
            genesis_file: public_taira_conflicting_xor_alias_npos_genesis_file(),
            out_file: None,
            bound_manifest_out: None,
            expected_hash_out: None,
            topology: Some(topology_json),
            peer_pops,
            private_key: Some(test_private_key_hex()),
            private_key_file: None,
            expected_public_key: None,
            seed: None,
            creation_time_ms: None,
            algorithm: Algorithm::Ed25519,
            config: None,
            consensus_mode: None,
        };

        let mut writer = BufWriter::new(Vec::new());
        let err = args
            .run(&mut writer)
            .expect_err("conflicting public XOR bindings should fail");
        assert!(
            err.to_string().contains("multiple asset definitions"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn sign_skips_npos_validator_bootstrap_for_admin_managed_lane() {
        let (peers, peer_pops) = valid_test_topology(4);
        let topology_json = norito::json::to_json(&peers).unwrap();
        let args = Args {
            genesis_file: npos_genesis_file(),
            out_file: None,
            bound_manifest_out: None,
            expected_hash_out: None,
            topology: Some(topology_json),
            peer_pops,
            private_key: Some(test_private_key_hex()),
            private_key_file: None,
            expected_public_key: None,
            seed: None,
            creation_time_ms: None,
            algorithm: Algorithm::Ed25519,
            config: Some(nexus_profile_with_validator_modes(
                "admin_managed",
                "admin_managed",
            )),
            consensus_mode: None,
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

        assert!(
            validators.is_empty(),
            "admin-managed lane configs must not auto-inject NPoS validator bootstrap"
        );
    }

    #[test]
    fn sign_links_genesis_account_into_ivm_without_reregistering_it() {
        let (peers, peer_pops) = valid_test_topology(4);
        let topology_json = norito::json::to_json(&peers).unwrap();
        let args = Args {
            genesis_file: npos_genesis_file(),
            out_file: None,
            bound_manifest_out: None,
            expected_hash_out: None,
            topology: Some(topology_json),
            peer_pops,
            private_key: Some(test_private_key_hex()),
            private_key_file: None,
            expected_public_key: None,
            seed: None,
            creation_time_ms: None,
            algorithm: Algorithm::Ed25519,
            config: None,
            consensus_mode: None,
        };

        let mut writer = BufWriter::new(Vec::new());
        args.run(&mut writer).expect("sign should succeed");
        writer.flush().expect("flush output");
        let bytes = writer.into_inner().expect("extract buffer");
        let block = decode_framed_signed_block(&bytes).expect("decode signed block");
        let genesis_account = block
            .external_transactions()
            .next()
            .expect("signed genesis transaction")
            .authority()
            .clone();
        let mut ivm_genesis_registrations = 0usize;
        for tx in block.external_transactions() {
            if let Executable::Instructions(instructions) = tx.instructions() {
                for instr in instructions {
                    if let Some(register) = instr.as_any().downcast_ref::<Register<Account>>()
                        && register.object.id == genesis_account
                    {
                        ivm_genesis_registrations += 1;
                    }
                }
            }
        }

        assert_eq!(
            ivm_genesis_registrations, 0,
            "expected NPoS bootstrap to avoid re-registering the genesis controller under ivm"
        );
    }

    #[test]
    fn npos_consensus_mode_requires_npos_parameters() {
        let args = Args {
            genesis_file: minimal_genesis_file(),
            out_file: None,
            bound_manifest_out: None,
            expected_hash_out: None,
            topology: None,
            peer_pops: Vec::new(),
            private_key: Some(test_private_key_hex()),
            private_key_file: None,
            expected_public_key: None,
            seed: None,
            creation_time_ms: None,
            algorithm: Algorithm::Ed25519,
            config: None,
            consensus_mode: Some(ConsensusModeArg::Npos),
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
            bound_manifest_out: None,
            expected_hash_out: None,
            topology: None,
            peer_pops: Vec::new(),
            private_key: Some(test_private_key_hex()),
            private_key_file: None,
            expected_public_key: None,
            seed: None,
            creation_time_ms: None,
            algorithm: Algorithm::Ed25519,
            config: None,
            consensus_mode: None,
        };

        let mut writer = BufWriter::new(Vec::new());
        args.run(&mut writer)
            .expect("NPoS genesis with parameters should sign");
    }

    fn nexus_profile_config_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|path| path.parent())
            .expect("workspace root")
            .join("defaults/nexus/config.toml")
    }

    #[test]
    fn peer_config_chain_must_match_manifest() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let manifest = RawGenesisTransaction::from_path(root.join("defaults/genesis.json"))
            .expect("parse genesis fixture");
        let mut config = checked_in_config(&root.join("defaults/nexus/config.toml"));
        ensure_peer_config_matches_manifest(&config, &manifest)
            .expect("baseline config and manifest match");

        config.common.chain = ChainId::from("concurrently-replaced-chain");
        let error = ensure_peer_config_matches_manifest(&config, &manifest)
            .expect_err("chain mismatch must fail before signing");
        assert!(
            error
                .to_string()
                .contains("does not match genesis manifest chain")
        );
    }

    #[test]
    fn peer_config_discriminant_must_match_manifest() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let manifest = RawGenesisTransaction::from_path(root.join("defaults/genesis.json"))
            .expect("parse genesis fixture");
        let mut config = checked_in_config(&root.join("defaults/nexus/config.toml"));
        *config.common.chain_discriminant.value_mut() = manifest
            .chain_discriminant()
            .checked_add(1)
            .expect("fixture discriminant can increase");
        let error = ensure_peer_config_matches_manifest(&config, &manifest)
            .expect_err("discriminant mismatch must fail before signing");
        assert!(error.to_string().contains("chain discriminant"));
    }

    #[test]
    fn sign_embeds_da_proof_policies_from_peer_config() {
        let args = Args {
            genesis_file: minimal_genesis_file(),
            out_file: None,
            bound_manifest_out: None,
            expected_hash_out: None,
            topology: None,
            peer_pops: Vec::new(),
            private_key: Some(test_private_key_hex()),
            private_key_file: None,
            expected_public_key: None,
            seed: None,
            creation_time_ms: None,
            algorithm: Algorithm::Ed25519,
            config: Some(nexus_profile_config_path()),
            consensus_mode: None,
        };

        let mut writer = BufWriter::new(Vec::new());
        args.run(&mut writer).expect("sign should succeed");
        writer.flush().expect("flush output");
        let bytes = writer.into_inner().expect("extract buffer");
        let block = decode_framed_signed_block(&bytes).expect("decode signed block");
        let bundle = block
            .da_proof_policies()
            .expect("expected genesis to embed configured DA proof policies");
        let aliases: Vec<_> = bundle
            .policies
            .iter()
            .map(|policy| policy.alias.as_str())
            .collect();

        assert_eq!(aliases, vec!["core", "governance", "zk"]);
    }

    #[test]
    fn sign_accepts_permissioned_on_iroha3() {
        let args = Args {
            genesis_file: minimal_genesis_file(),
            out_file: None,
            bound_manifest_out: None,
            expected_hash_out: None,
            topology: None,
            peer_pops: Vec::new(),
            private_key: Some(test_private_key_hex()),
            private_key_file: None,
            expected_public_key: None,
            seed: None,
            creation_time_ms: None,
            algorithm: Algorithm::Ed25519,
            config: None,
            consensus_mode: None,
        };

        let mut writer = BufWriter::new(Vec::new());
        args.run(&mut writer)
            .expect("permissioned genesis should be allowed on Iroha3");
    }

    #[test]
    fn staged_manifest_registry_reenters_genesis_discriminant() {
        let discriminant = crate::genesis::profile::TAIRA_CHAIN_DISCRIMINANT;
        let genesis = RawGenesisTransaction::from_path(minimal_genesis_file())
            .expect("parse minimal genesis")
            .with_chain_discriminant(discriminant);
        let directory = tempfile::tempdir().expect("manifest directory");
        fs::write(
            directory.path().join("governance.manifest.json"),
            r#"{
                "lane": "governance",
                "governance": "parliament",
                "version": 1,
                "validators": [{
                    "validator": "testﾜヰ8ｽuimdh9FﾂｦUｸﾈbﾕﾆヱMUYｴGｷﾙｹﾐRヱbﾐｷwﾄ6ﾃdDLPQﾋW496uﾙﾜFpﾈtHd4Hﾙﾎ45M1L5",
                    "peer_id": "ea0130999C999F728B0829387F4E93732EE0479F911DE0CE1E9409C8CEA66CF99376F57DB2E709892648F222D9F4E90DB29B84"
                }],
                "quorum": 1
            }"#,
        )
        .expect("write Taira manifest");
        let lane_catalog = LaneCatalog::new(
            std::num::NonZeroU32::new(1).expect("non-zero lane count"),
            vec![LaneConfig {
                id: LaneId::SINGLE,
                alias: "governance".to_owned(),
                governance: Some("parliament".to_owned()),
                ..LaneConfig::default()
            }],
        )
        .expect("governance lane catalog");
        let mut governance = actual::GovernanceCatalog::default();
        governance
            .modules
            .insert("parliament".to_owned(), actual::GovernanceModule::default());
        let nexus = actual::Nexus {
            enabled: true,
            lane_catalog: lane_catalog.clone(),
            configured_lane_catalog: lane_catalog,
            governance,
            registry: actual::LaneRegistry {
                manifest_directory: Some(directory.path().to_path_buf()),
                cache_directory: Some(directory.path().to_path_buf()),
                ..actual::LaneRegistry::default()
            },
            ..actual::Nexus::default()
        };

        let _wrong_discriminant = ChainDiscriminantGuard::enter(discriminant.wrapping_add(1));
        let registry = staged_lane_manifest_registry(&genesis, &nexus)
            .expect("staged registry must re-enter the genesis discriminant");
        registry
            .validate_active_coverage()
            .expect("Taira governance manifest must remain active");
    }

    fn minimal_genesis_file() -> PathBuf {
        let genesis_file = tempfile::Builder::new()
            .prefix("kagami-genesis-test-")
            .tempfile()
            .expect("create temp genesis file");
        let manifest =
            GenesisBuilder::new_without_executor(ChainId::from("test-chain"), PathBuf::from("."))
                .set_topology(valid_test_topology_entries(4))
                .build_raw()
                .with_consensus_mode(SumeragiConsensusMode::Permissioned)
                .with_consensus_meta();
        let genesis_json = norito::json::to_json_pretty(&manifest).expect("serialize genesis");
        fs::write(genesis_file.path(), genesis_json).expect("write genesis json");
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
            "chain_discriminant": 1,
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
                .set_topology(valid_test_topology_entries(4))
                .build_raw()
                .with_consensus_mode(SumeragiConsensusMode::Npos)
                .with_consensus_meta();
        let json = norito::json::to_json_pretty(&manifest).expect("serialize genesis manifest");
        fs::write(genesis_file.path(), json).expect("write genesis json");
        let (_file, path) = genesis_file.keep().expect("persist temp genesis");
        path
    }

    fn alias_backed_npos_genesis_file() -> PathBuf {
        let genesis_file = tempfile::Builder::new()
            .prefix("kagami-npos-alias-genesis-")
            .tempfile()
            .expect("create temp genesis file");
        let asset_definition_id: AssetDefinitionId = "6TEAJqbb8oEPmLncoNiMRbLEK6tw"
            .parse()
            .expect("valid canonical asset id");
        let alias: AssetDefinitionAlias = "xor#universal".parse().expect("valid alias");
        let manifest = GenesisBuilder::new_without_executor(
            ChainId::from("npos-sign-alias"),
            PathBuf::from("."),
        )
        .append_instruction(Register::asset_definition(
            AssetDefinition::new(
                asset_definition_id.clone(),
                "xor".to_owned(),
                NumericSpec::default(),
                iroha_data_model::asset::AssetBalancePolicy::Global,
                None,
            )
            .with_metadata(Metadata::default()),
        ))
        .append_instruction(SetAssetDefinitionAlias::bind(
            asset_definition_id,
            alias,
            None,
        ))
        .append_parameter(Parameter::Custom(
            SumeragiNposParameters::default().into_custom_parameter(),
        ))
        .build_raw()
        .with_consensus_mode(SumeragiConsensusMode::Npos)
        .with_consensus_meta();
        let json = norito::json::to_json_pretty(&manifest).expect("serialize genesis manifest");
        fs::write(genesis_file.path(), json).expect("write genesis json");
        let (_file, path) = genesis_file.keep().expect("persist temp genesis");
        path
    }

    fn public_taira_alias_backed_npos_genesis_file() -> PathBuf {
        let genesis_file = tempfile::Builder::new()
            .prefix("kagami-public-taira-npos-alias-genesis-")
            .tempfile()
            .expect("create temp genesis file");
        let asset_definition_id: AssetDefinitionId = crate::genesis::TAIRA_XOR_ASSET_DEFINITION_ID
            .parse()
            .expect("valid canonical asset id");
        let alias: AssetDefinitionAlias = crate::genesis::PUBLIC_XOR_ALIAS
            .parse()
            .expect("valid alias");
        let manifest =
            GenesisBuilder::new_without_executor(ChainId::from("iroha3-taira"), PathBuf::from("."))
                .append_instruction(Register::asset_definition(
                    AssetDefinition::new(
                        asset_definition_id.clone(),
                        "xor".to_owned(),
                        NumericSpec::default(),
                        iroha_data_model::asset::AssetBalancePolicy::Global,
                        None,
                    )
                    .with_metadata(Metadata::default()),
                ))
                .append_instruction(SetAssetDefinitionAlias::bind(
                    asset_definition_id,
                    alias,
                    None,
                ))
                .append_parameter(Parameter::Custom(
                    SumeragiNposParameters::default().into_custom_parameter(),
                ))
                .build_raw()
                .with_consensus_mode(SumeragiConsensusMode::Npos)
                .with_chain_discriminant(crate::genesis::profile::TAIRA_CHAIN_DISCRIMINANT)
                .with_consensus_meta();
        let json = norito::json::to_json_pretty(&manifest).expect("serialize genesis manifest");
        fs::write(genesis_file.path(), json).expect("write genesis json");
        let (_file, path) = genesis_file.keep().expect("persist temp genesis");
        path
    }

    fn public_nexus_npos_genesis_file_without_xor_alias() -> PathBuf {
        let genesis_file = tempfile::Builder::new()
            .prefix("kagami-public-nexus-npos-genesis-")
            .tempfile()
            .expect("create temp genesis file");
        let manifest =
            GenesisBuilder::new_without_executor(ChainId::from("iroha3-nexus"), PathBuf::from("."))
                .append_parameter(Parameter::Custom(
                    SumeragiNposParameters::default().into_custom_parameter(),
                ))
                .build_raw()
                .with_consensus_mode(SumeragiConsensusMode::Npos)
                .with_chain_discriminant(crate::genesis::profile::NEXUS_CHAIN_DISCRIMINANT)
                .with_consensus_meta();
        let json = norito::json::to_json_pretty(&manifest).expect("serialize genesis manifest");
        fs::write(genesis_file.path(), json).expect("write genesis json");
        let (_file, path) = genesis_file.keep().expect("persist temp genesis");
        path
    }

    fn public_taira_conflicting_xor_alias_npos_genesis_file() -> PathBuf {
        let genesis_file = tempfile::Builder::new()
            .prefix("kagami-public-taira-conflicting-xor-genesis-")
            .tempfile()
            .expect("create temp genesis file");
        let canonical_xor: AssetDefinitionId = crate::genesis::TAIRA_XOR_ASSET_DEFINITION_ID
            .parse()
            .expect("valid canonical asset id");
        let wrong_xor: AssetDefinitionId = "61CtjvNd9T3THAR65GsMVHr82Bjc"
            .parse()
            .expect("valid canonical asset id");
        let alias: AssetDefinitionAlias = crate::genesis::PUBLIC_XOR_ALIAS
            .parse()
            .expect("valid alias");
        let manifest =
            GenesisBuilder::new_without_executor(ChainId::from("iroha3-taira"), PathBuf::from("."))
                .append_instruction(Register::asset_definition(
                    AssetDefinition::new(
                        canonical_xor.clone(),
                        "xor".to_owned(),
                        NumericSpec::default(),
                        iroha_data_model::asset::AssetBalancePolicy::Global,
                        None,
                    )
                    .with_metadata(Metadata::default()),
                ))
                .append_instruction(Register::asset_definition(
                    AssetDefinition::new(
                        wrong_xor.clone(),
                        "xor-shadow".to_owned(),
                        NumericSpec::default(),
                        iroha_data_model::asset::AssetBalancePolicy::Global,
                        None,
                    )
                    .with_metadata(Metadata::default()),
                ))
                .append_instruction(SetAssetDefinitionAlias::bind(
                    canonical_xor,
                    alias.clone(),
                    None,
                ))
                .append_instruction(SetAssetDefinitionAlias::bind(wrong_xor, alias, None))
                .append_parameter(Parameter::Custom(
                    SumeragiNposParameters::default().into_custom_parameter(),
                ))
                .build_raw()
                .with_consensus_mode(SumeragiConsensusMode::Npos)
                .with_chain_discriminant(crate::genesis::profile::TAIRA_CHAIN_DISCRIMINANT)
                .with_consensus_meta();
        let json = norito::json::to_json_pretty(&manifest).expect("serialize genesis manifest");
        fs::write(genesis_file.path(), json).expect("write genesis json");
        let (_file, path) = genesis_file.keep().expect("persist temp genesis");
        path
    }

    #[cfg(unix)]
    #[test]
    fn private_key_file_round_trips_owner_only_canonical_material() {
        use std::os::unix::fs::PermissionsExt as _;

        let temp = tempfile::tempdir().expect("private key temp dir");
        let key_pair = checked_genesis_sign_keypair_with_algorithm(Algorithm::Ed25519);
        let canonical = ExposedPrivateKey(key_pair.private_key().clone()).to_string();
        let path = temp.path().join("genesis.private_key");
        let raw = zeroize::Zeroizing::new(format!("{canonical}\n").into_bytes());
        crate::secure_fs::write_private_file_atomic(&path, raw.as_slice())
            .expect("write canonical private key");

        assert_eq!(
            fs::metadata(&path)
                .expect("private key metadata")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        let loaded =
            load_genesis_key_file(&path, Algorithm::Ed25519).expect("load canonical private key");
        assert_eq!(loaded.public_key(), key_pair.public_key());
    }

    #[cfg(unix)]
    #[test]
    fn private_key_file_rejects_unsafe_mode_symlink_hardlink_and_whitespace() {
        use std::os::unix::fs::{PermissionsExt as _, symlink};

        let temp = tempfile::tempdir().expect("private key temp dir");
        let key_pair = checked_genesis_sign_keypair_with_algorithm(Algorithm::Ed25519);
        let canonical = ExposedPrivateKey(key_pair.private_key().clone()).to_string();
        let unsafe_mode = temp.path().join("unsafe-mode.key");
        fs::write(&unsafe_mode, format!("{canonical}\n")).expect("write unsafe key");
        fs::set_permissions(&unsafe_mode, fs::Permissions::from_mode(0o644))
            .expect("set unsafe mode");
        assert!(load_genesis_key_file(&unsafe_mode, Algorithm::Ed25519).is_err());

        fs::set_permissions(&unsafe_mode, fs::Permissions::from_mode(0o600))
            .expect("set safe mode");
        let symlink_path = temp.path().join("symlink.key");
        symlink(&unsafe_mode, &symlink_path).expect("create symlink");
        assert!(load_genesis_key_file(&symlink_path, Algorithm::Ed25519).is_err());

        let hardlink_path = temp.path().join("hardlink.key");
        fs::hard_link(&unsafe_mode, &hardlink_path).expect("create hardlink");
        assert!(load_genesis_key_file(&unsafe_mode, Algorithm::Ed25519).is_err());
        fs::remove_file(&hardlink_path).expect("remove hardlink");

        let whitespace = temp.path().join("whitespace.key");
        crate::secure_fs::write_private_file_atomic(
            &whitespace,
            format!(" {canonical}\n").as_bytes(),
        )
        .expect("write whitespace key");
        assert!(load_genesis_key_file(&whitespace, Algorithm::Ed25519).is_err());
    }

    fn test_private_key_hex() -> String {
        let kp = checked_genesis_sign_keypair_with_algorithm(Algorithm::Ed25519);
        let (_alg, bytes) = kp.private_key().to_bytes();
        hex::encode(bytes)
    }
}
