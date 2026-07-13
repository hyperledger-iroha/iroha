use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    sync::Arc,
};

use clap::Parser;
use color_eyre::eyre::{WrapErr, eyre};
use iroha_config::{base::toml::TomlSource, parameters::actual};
use iroha_core::{
    block::ValidBlock,
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::isi::Registrable as _,
    state::{State, World},
    sumeragi::{VotingBlock, network_topology::Topology},
};
use iroha_crypto::{Algorithm, KeyPair, PrivateKey};
use iroha_data_model::{
    asset::AssetDefinitionAlias,
    da::commitment::DaProofPolicyBundle,
    isi::RegisterPublicLaneValidator,
    parameter::{SumeragiParameter, system::SumeragiConsensusMode},
    prelude::*,
};
use iroha_genesis::{GenesisBuilder, GenesisTopologyEntry, RawGenesisTransaction};
use iroha_primitives::time::TimeSource;

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
    /// Optional peer config TOML used to derive the DA proof-policy bundle embedded into genesis.
    #[clap(long, value_name = "PATH")]
    config: Option<PathBuf>,
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

const DEFAULT_NPOS_BOOTSTRAP_DOMAIN: &str = "nexus.universal";
const DEFAULT_NPOS_BOOTSTRAP_IVM_DOMAIN: &str = "ivm.universal";
const DEFAULT_NPOS_BOOTSTRAP_STAKE_ASSET_NAME: &str = "xor";
const DEFAULT_NPOS_BOOTSTRAP_STAKE_AMOUNT: u64 = 10_000;
const DEFAULT_NPOS_BOOTSTRAP_ESCROW_SEED: &[u8] = b"npos-escrow-account";

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

fn bootstrap_escrow_account_id(
    genesis_public_key: &iroha_crypto::PublicKey,
) -> Result<AccountId, color_eyre::eyre::Error> {
    let escrow_key_pair = KeyPair::try_from_seed(
        genesis_public_key
            .to_string()
            .bytes()
            .chain(DEFAULT_NPOS_BOOTSTRAP_ESCROW_SEED.iter().copied())
            .collect(),
        iroha_crypto::Algorithm::default(),
    )
    .wrap_err("failed to derive NPoS bootstrap escrow account key pair")?;
    Ok(AccountId::new(escrow_key_pair.public_key().clone()))
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

fn default_npos_bootstrap_stake_asset_id() -> AssetDefinitionId {
    AssetDefinitionId::new(
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
    config_path: Option<&Path>,
) -> Result<AssetDefinitionId, color_eyre::eyre::Error> {
    let public_profile = public_xor_profile_for_manifest(manifest);
    let stake_asset_id = if let Some(config_path) = config_path {
        let config = load_peer_config(config_path)?;
        resolve_npos_bootstrap_stake_asset_id(manifest, &config.nexus.staking.stake_asset_id)
            .map_err(|err| {
                eyre!(
                    "failed to resolve nexus.staking.stake_asset_id from {}: {err}",
                    config_path.display(),
                )
            })?
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

fn append_npos_bootstrap(
    builder: GenesisBuilder,
    registrations: &mut BootstrapRegistrations,
    topology: &[PeerId],
    escrow_domain_id: &DomainId,
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
    if !registrations.domains.contains(escrow_domain_id) {
        builder =
            builder.append_instruction(Register::domain(Domain::new(escrow_domain_id.clone())));
        registrations.domains.insert(escrow_domain_id.clone());
    }
    if !registrations.accounts.contains(escrow_account_id) {
        builder =
            builder.append_instruction(Register::account(Account::new(escrow_account_id.clone())));
        registrations.accounts.insert(escrow_account_id.clone());
    }
    if !registrations.asset_defs.contains(stake_asset_id) {
        let definition = AssetDefinition::new(stake_asset_id.clone(), NumericSpec::default())
            .with_name("NPOS Stake".to_owned())
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

fn load_peer_config(config_path: &Path) -> Result<actual::Root, color_eyre::eyre::Error> {
    let source = TomlSource::from_file(config_path).map_err(|err| {
        eyre!(
            "failed to read peer config at {}: {err}",
            config_path.display()
        )
    })?;
    actual::Root::from_toml_source(source).map_err(|err| {
        eyre!(
            "failed to parse peer config at {}: {err}",
            config_path.display()
        )
    })
}

fn bind_staged_sumeragi_v2_context(
    genesis: RawGenesisTransaction,
    genesis_key_pair: &KeyPair,
    config: Option<&actual::Root>,
    da_proof_policies: Option<DaProofPolicyBundle>,
    confidential_policy_hash: [u8; 32],
) -> Result<iroha_genesis::GenesisBlock, color_eyre::eyre::Error> {
    let mut parameters = genesis
        .sumeragi_v2_context_parameters()
        .ok_or_else(|| eyre!("genesis manifest is missing required `sumeragi_v2` parameters"))?;
    parameters.nexus_amx_context_hash = staged_sumeragi_v2_context_hash(
        &genesis,
        genesis_key_pair,
        config,
        da_proof_policies.as_ref(),
        confidential_policy_hash,
    )?
    .into();

    genesis
        .with_sumeragi_v2_context_parameters(parameters)
        .with_consensus_meta()
        .build_and_sign_with_da_proof_policies_and_confidential_policy_hash(
            genesis_key_pair,
            da_proof_policies,
            Some(confidential_policy_hash),
        )
}

/// Stage a raw genesis transaction and return its exact Nexus/AMX consensus
/// commitment without committing state or writing Kura.
fn staged_sumeragi_v2_context_hash(
    genesis: &RawGenesisTransaction,
    genesis_key_pair: &KeyPair,
    config: Option<&actual::Root>,
    da_proof_policies: Option<&DaProofPolicyBundle>,
    confidential_policy_hash: [u8; 32],
) -> Result<iroha_crypto::Hash, color_eyre::eyre::Error> {
    let provisional = genesis
        .clone()
        .with_consensus_meta()
        .build_and_sign_with_da_proof_policies_and_confidential_policy_hash(
            genesis_key_pair,
            da_proof_policies.cloned(),
            Some(confidential_policy_hash),
        )?;

    let authority = AccountId::new(genesis_key_pair.public_key().clone());
    let world = World::with(
        [Domain::new(iroha_genesis::GENESIS_DOMAIN_ID.clone()).build(&authority)],
        [Account::new(authority.clone()).build(&authority)],
        [],
    );
    let kura = Kura::blank_kura_for_testing();
    let mut state = State::new_with_chain_for_testing(
        world,
        Arc::clone(&kura),
        LiveQueryStore::start_test(),
        genesis.chain_id().clone(),
    );
    if let Some(config) = config {
        state.set_pipeline(config.pipeline.clone());
        state
            .set_nexus(config.nexus.clone())
            .map_err(|error| eyre!("invalid Nexus config for staged genesis: {error}"))?;
        state.set_crypto(config.crypto.clone());
    } else {
        state.set_pipeline(actual::Pipeline::default());
        state
            .set_nexus(actual::Nexus::default())
            .map_err(|error| eyre!("invalid default Nexus config: {error}"))?;
        state.set_crypto(actual::Crypto::default());
    }

    let voters = iroha_core::sumeragi::signed_genesis_voting_peers(&provisional)
        .map_err(|error| eyre!("invalid signed Sumeragi v2 genesis roster: {error}"))?;
    if voters.is_empty() {
        return Err(eyre!(
            "Sumeragi v2 genesis roster is empty; inject BLS topology entries and PoPs before signing"
        ));
    }
    let topology = Topology::new(voters);
    let mut voting_block: Option<VotingBlock> = None;
    let (_valid, staged) = ValidBlock::validate_keep_voting_block(
        provisional.0,
        &topology,
        genesis.chain_id(),
        &authority,
        &TimeSource::new_system(),
        &state,
        &mut voting_block,
        false,
    )
    .unpack(|_| {})
    .map_err(|(_block, error)| eyre!("staged genesis execution failed: {error}"))?;
    let hash = iroha_core::sumeragi::staged_genesis_nexus_amx_context_hash(&staged);
    drop(staged);
    Ok(hash)
}

fn should_auto_bootstrap_npos_validators(
    config_path: Option<&Path>,
) -> Result<bool, color_eyre::eyre::Error> {
    let Some(config_path) = config_path else {
        return Ok(true);
    };

    let config = load_peer_config(config_path)?;
    Ok(matches!(
        config
            .nexus
            .staking
            .validator_mode(LaneId::SINGLE, &config.nexus.lane_catalog),
        actual::LaneValidatorMode::StakeElected
    ))
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
        require_v2_wire_protocol_only(&genesis)?;
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
        let auto_bootstrap_npos = should_auto_bootstrap_npos_validators(self.config.as_deref())?;
        let topology_peers = if uses_npos {
            topology_override
                .clone()
                .unwrap_or_else(|| collect_topology_peers(&genesis))
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
            configured_npos_bootstrap_stake_asset_id(&genesis, self.config.as_deref())?
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
            self.seed.as_deref(),
            self.algorithm,
        )?;
        let da_proof_policies = resolve_da_proof_policies(self.config.as_deref())?;
        let confidential_policy_hash = resolve_confidential_policy_hash(self.config.as_deref())?;
        let peer_config = self.config.as_deref().map(load_peer_config).transpose()?;
        let direct_sign_safe = topology_override.is_none()
            && next_consensus_mode.is_none()
            && self.mode_activation_height.is_none()
            && !needs_npos_bootstrap;
        let prepared_genesis = if direct_sign_safe {
            genesis.with_consensus_mode(consensus_mode)
        } else {
            let mut builder = genesis.into_builder();

            if let Some(topology) = topology_override.as_ref() {
                // Put topology into a dedicated transaction so it remains separate
                // from other genesis instructions.
                let entries = build_topology_entries(topology, &self.peer_pops)?;
                builder = builder.next_transaction().set_topology(entries);
            }
            if let Some(mode) = next_consensus_mode {
                builder = builder
                    .append_parameter(Parameter::Sumeragi(SumeragiParameter::NextMode(mode)));
            }
            if let (Some(height), Some(_)) = (self.mode_activation_height, self.next_consensus_mode)
            {
                builder = builder.append_parameter(Parameter::Sumeragi(
                    SumeragiParameter::ModeActivationHeight(height),
                ));
            }
            if needs_npos_bootstrap {
                let ivm_domain =
                    DomainId::parse_fully_qualified(DEFAULT_NPOS_BOOTSTRAP_IVM_DOMAIN)?;
                let escrow_account_id = bootstrap_escrow_account_id(genesis_key_pair.public_key())?;
                builder = append_npos_bootstrap(
                    builder,
                    &mut bootstrap_registrations,
                    &topology_peers,
                    &ivm_domain,
                    &escrow_account_id,
                    &bootstrap_stake_asset_id,
                )?;
            }

            builder
                .build_raw()
                .with_consensus_mode(consensus_mode)
                .with_consensus_meta()
        };
        let genesis_block = bind_staged_sumeragi_v2_context(
            prepared_genesis,
            &genesis_key_pair,
            peer_config.as_ref(),
            da_proof_policies,
            confidential_policy_hash,
        )?;

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
            KeyPair::from_private_key(sk).wrap_err("derive genesis key pair from private key")
        }
        (None, Some(seed)) => KeyPair::try_from_seed(seed.as_bytes().to_vec(), algorithm)
            .wrap_err("derive seeded genesis key pair"),
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

fn resolve_da_proof_policies(
    config_path: Option<&Path>,
) -> Result<Option<DaProofPolicyBundle>, color_eyre::eyre::Error> {
    let Some(config_path) = config_path else {
        return Ok(None);
    };

    let config = load_peer_config(config_path)?;

    Ok(Some(iroha_core::da::proof_policy_bundle(
        &config.nexus.lane_config,
    )))
}

fn resolve_confidential_policy_hash(
    config_path: Option<&Path>,
) -> Result<[u8; 32], color_eyre::eyre::Error> {
    let Some(config_path) = config_path else {
        return Ok(iroha_core::state::default_zk_consensus_policy_hash());
    };

    let config = load_peer_config(config_path)?;
    Ok(iroha_core::state::compute_genesis_zk_consensus_policy_hash(
        &config.zk,
    ))
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
    use iroha_crypto::KeyPair as CryptoKeyPair;
    use iroha_data_model::{
        ChainId,
        asset::AssetDefinitionAlias,
        block::{consensus_v2::SumeragiV2GenesisContextParameters, decode_framed_signed_block},
        isi::{
            SetParameter, asset_alias::SetAssetDefinitionAlias, mint_burn::MintBox,
            staking::RegisterPublicLaneValidator,
        },
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
    use norito::derive::JsonDeserialize;

    fn checked_in_config(path: &std::path::Path) -> actual::Root {
        if let Ok(config) = load_peer_config(path) {
            return config;
        }

        // Some archived/generated examples carry obsolete or runtime-secret
        // fields outside consensus. Reparse only the two tables consumed by
        // the v2 commitment, while retaining the source chain/discriminant,
        // through the production config parser. No Nexus or Pipeline value is
        // synthesized or decoded by this test helper.
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

    #[derive(Debug, Clone, JsonDeserialize, PartialEq, Eq)]
    struct ConsensusHandshakeMetaTest {
        mode: String,
        bls_domain: String,
        wire_proto_versions: Vec<u32>,
        consensus_fingerprint: String,
        sumeragi_v2: SumeragiV2GenesisContextParameters,
    }

    fn sign_checked_in_profile(
        root: &std::path::Path,
        genesis_path: &str,
        config_path: &str,
    ) -> ConsensusHandshakeMetaTest {
        let args = Args {
            genesis_file: root.join(genesis_path),
            out_file: None,
            topology: None,
            peer_pops: Vec::new(),
            private_key: Some(test_private_key_hex()),
            seed: None,
            algorithm: Algorithm::Ed25519,
            config: Some(root.join(config_path)),
            consensus_mode: None,
            next_consensus_mode: None,
            mode_activation_height: None,
        };
        let mut writer = BufWriter::new(Vec::new());
        args.run(&mut writer)
            .unwrap_or_else(|error| panic!("failed to sign {genesis_path}: {error:#}"));
        let bytes = writer.into_inner().expect("flush signed genesis buffer");
        let block = decode_framed_signed_block(&bytes)
            .unwrap_or_else(|error| panic!("failed to decode signed {genesis_path}: {error}"));

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
        panic!("signed {genesis_path} omitted consensus handshake metadata");
    }

    #[test]
    fn checked_in_genesis_templates_are_current_v2_and_parse() {
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
            assert_eq!(manifest.wire_proto_versions(), &[2], "{path}");
            let expected_domain = match manifest
                .consensus_mode()
                .expect("checked-in manifest has consensus mode")
            {
                SumeragiConsensusMode::Permissioned => {
                    iroha_data_model::block::consensus_v2::PERMISSIONED_BLS_DOMAIN
                }
                SumeragiConsensusMode::Npos => {
                    iroha_data_model::block::consensus_v2::NPOS_BLS_DOMAIN
                }
            };
            assert_eq!(manifest.bls_domain(), Some(expected_domain), "{path}");
            let context = manifest
                .sumeragi_v2_context_parameters()
                .unwrap_or_else(|| panic!("{path} omitted sumeragi_v2"));
            assert_ne!(context.nexus_amx_context_hash, [0; 32], "{path}");
            let refreshed = manifest.clone().with_consensus_meta();
            assert_eq!(
                manifest.consensus_fingerprint(),
                refreshed.consensus_fingerprint(),
                "{path} has a stale consensus fingerprint"
            );
        }
    }

    #[test]
    fn checked_in_unsigned_templates_use_canonical_config_projection() {
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
                    .expect("v2 context")
                    .nexus_amx_context_hash,
                <[u8; 32]>::from(expected),
                "{genesis_path} must carry the config-only projection; a deployable signer rebinds it after the final roster and operator-supplied public XOR identity are present"
            );
        }
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
            assert_eq!(signed.wire_proto_versions, vec![2], "{genesis_path}");
            assert_eq!(
                signed.sumeragi_v2,
                manifest
                    .sumeragi_v2_context_parameters()
                    .expect("checked-in profile has v2 context"),
                "{genesis_path} must carry the exact context produced by the production signing path"
            );
            assert_eq!(
                Some(signed.consensus_fingerprint.as_str()),
                manifest.consensus_fingerprint(),
                "{genesis_path} fingerprint must cover the exact staged context"
            );
        }
    }

    #[test]
    #[ignore = "read-only maintainer utility for refreshing generated profile commitments"]
    fn print_checked_in_profile_commitments() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        for (genesis_path, config_path) in [
            (
                "defaults/kagami/iroha3-dev/genesis.json",
                "defaults/kagami/iroha3-dev/config.toml",
            ),
            (
                "defaults/kagami/iroha3-taira/genesis.json",
                "defaults/kagami/iroha3-taira/config.toml",
            ),
        ] {
            let signed = sign_checked_in_profile(&root, genesis_path, config_path);
            eprintln!(
                "{genesis_path}: {} {}",
                hex::encode(signed.sumeragi_v2.nexus_amx_context_hash),
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

    fn replace_manifest_wire_protocols(path: &std::path::Path, versions: &[u32]) {
        let bytes = fs::read(path).expect("read genesis fixture");
        let mut value: norito::json::Value =
            norito::json::from_slice(&bytes).expect("parse genesis fixture JSON");
        value
            .as_object_mut()
            .expect("genesis fixture is an object")
            .insert(
                "wire_proto_versions".to_owned(),
                norito::json::value::to_value(&versions.to_vec()).expect("serialize protocol list"),
            );
        fs::write(
            path,
            norito::json::to_json_pretty(&value).expect("serialize genesis fixture JSON"),
        )
        .expect("rewrite genesis fixture");
    }

    #[test]
    fn signing_rejects_legacy_mixed_empty_and_duplicate_protocol_lists() {
        for versions in [Vec::new(), vec![1], vec![1, 2], vec![2, 1], vec![2, 2]] {
            let genesis_file = minimal_genesis_file();
            replace_manifest_wire_protocols(&genesis_file, &versions);
            let args = Args {
                genesis_file,
                out_file: None,
                topology: None,
                peer_pops: Vec::new(),
                private_key: Some(test_private_key_hex()),
                seed: None,
                algorithm: Algorithm::Ed25519,
                config: None,
                consensus_mode: None,
                next_consensus_mode: None,
                mode_activation_height: None,
            };
            let error = args
                .run(&mut BufWriter::new(Vec::new()))
                .expect_err("non-canonical protocol list must be rejected before signing");
            assert!(
                error
                    .to_string()
                    .contains("exactly wire_proto_versions = [2]"),
                "unexpected error for {versions:?}: {error}"
            );
        }
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
            topology: None,
            peer_pops: vec!["pk=00".to_string()],
            private_key: Some(test_private_key_hex()),
            seed: None,
            algorithm: Algorithm::Ed25519,
            config: None,
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
        let peer = PeerId::new(checked_genesis_sign_keypair().public_key().clone());
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
            config: None,
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
        let peer_a = PeerId::new(checked_genesis_sign_keypair().public_key().clone());
        let peer_b = PeerId::new(checked_genesis_sign_keypair().public_key().clone());
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
            config: None,
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
        assert_eq!(
            kp.public_key()
                .try_algorithm()
                .expect("fixture public key must be valid"),
            Algorithm::Secp256k1
        );
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
            config: None,
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
            config: None,
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
            config: None,
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
            config: None,
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
        let peer_a = PeerId::new(checked_genesis_sign_keypair().public_key().clone());
        let peer_b = PeerId::new(checked_genesis_sign_keypair().public_key().clone());
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
            config: None,
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

        let new_kp = checked_genesis_sign_keypair_with_algorithm(Algorithm::BlsNormal);
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
            config: None,
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
    fn sign_without_manifest_mutations_preserves_direct_manifest_payload() {
        use std::num::NonZeroU64;

        use iroha_crypto::{Algorithm, KeyPair};
        use iroha_data_model::parameter::BlockParameter;

        let manifest = GenesisBuilder::new_without_executor(
            ChainId::from("sign-direct-manifest-regression"),
            ".",
        )
        .append_parameter(Parameter::Sumeragi(SumeragiParameter::MinFinalityMs(100)))
        .append_parameter(Parameter::Sumeragi(SumeragiParameter::BlockTimeMs(100)))
        .append_parameter(Parameter::Sumeragi(SumeragiParameter::CommitTimeMs(100)))
        .append_parameter(Parameter::Block(BlockParameter::MaxTransactions(
            NonZeroU64::new(512).expect("non-zero"),
        )))
        .next_transaction()
        .append_parameter(Parameter::Sumeragi(SumeragiParameter::BlockTimeMs(333)))
        .append_parameter(Parameter::Sumeragi(SumeragiParameter::CommitTimeMs(667)))
        .build_raw()
        .with_consensus_mode(SumeragiConsensusMode::Permissioned)
        .with_consensus_meta();

        let genesis_file = tempfile::NamedTempFile::new().expect("create temp genesis file");
        let json = norito::json::to_json_pretty(&manifest).expect("serialize genesis manifest");
        fs::write(genesis_file.path(), json).expect("write genesis json");

        let seed = "sign-direct-manifest-regression";
        let key_pair = KeyPair::try_from_seed(seed.as_bytes().to_vec(), Algorithm::Ed25519)
            .expect("derive checked genesis fixture key");
        let expected = manifest
            .clone()
            .build_and_sign_with_confidential_policy_hash(
                &key_pair,
                Some(iroha_core::state::default_zk_consensus_policy_hash()),
            )
            .expect("direct manifest signing should succeed");

        let args = Args {
            genesis_file: genesis_file.path().to_path_buf(),
            out_file: None,
            topology: None,
            peer_pops: vec![],
            private_key: None,
            seed: Some(seed.to_owned()),
            algorithm: Algorithm::Ed25519,
            config: None,
            consensus_mode: None,
            next_consensus_mode: None,
            mode_activation_height: None,
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
            "signing an unchanged manifest should commit the default ZK policy hash"
        );
    }

    #[test]
    fn sign_auto_bootstraps_npos_validators_for_topology() {
        let peer = PeerId::new(
            checked_genesis_sign_keypair_with_algorithm(Algorithm::BlsNormal)
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
            config: None,
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
        let mut minted_asset_ids = std::collections::BTreeSet::new();
        for tx in block.external_transactions() {
            if let Executable::Instructions(instructions) = tx.instructions() {
                for instr in instructions {
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

        let mut expected = std::collections::BTreeSet::new();
        expected.insert(AccountId::new(peer.public_key().clone()));
        assert_eq!(
            validators, expected,
            "expected NPoS bootstrap to register topology validators"
        );
        assert!(
            minted_asset_ids.contains(&default_npos_bootstrap_stake_asset_id()),
            "private NPoS bootstrap should keep using the synthetic local stake asset"
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
        let peer = PeerId::new(
            checked_genesis_sign_keypair_with_algorithm(Algorithm::BlsNormal)
                .public_key()
                .clone(),
        );
        let topology_json = norito::json::to_json(&vec![peer.clone()]).unwrap();
        let configured_asset_id: AssetDefinitionId = "6TEAJqbb8oEPmLncoNiMRbLEK6tw"
            .parse()
            .expect("valid canonical asset id");
        let args = Args {
            genesis_file: alias_backed_npos_genesis_file(),
            out_file: None,
            topology: Some(topology_json),
            peer_pops: vec![format!("{}=00", peer.public_key())],
            private_key: Some(test_private_key_hex()),
            seed: None,
            algorithm: Algorithm::Ed25519,
            config: Some(nexus_profile_with_stake_asset_id("xor#universal")),
            consensus_mode: None,
            next_consensus_mode: None,
            mode_activation_height: None,
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
        let peer = PeerId::new(
            checked_genesis_sign_keypair_with_algorithm(Algorithm::BlsNormal)
                .public_key()
                .clone(),
        );
        let topology_json = norito::json::to_json(&vec![peer.clone()]).unwrap();
        let configured_asset_id: AssetDefinitionId = crate::genesis::TAIRA_XOR_ASSET_DEFINITION_ID
            .parse()
            .expect("valid canonical asset id");
        let args = Args {
            genesis_file: public_taira_alias_backed_npos_genesis_file(),
            out_file: None,
            topology: Some(topology_json),
            peer_pops: vec![format!("{}=00", peer.public_key())],
            private_key: Some(test_private_key_hex()),
            seed: None,
            algorithm: Algorithm::Ed25519,
            config: None,
            consensus_mode: None,
            next_consensus_mode: None,
            mode_activation_height: None,
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
            "public Taira bootstrap should mint to canonical XOR"
        );
        assert!(
            !registered_asset_ids.contains(&default_npos_bootstrap_stake_asset_id()),
            "public Taira bootstrap must not register the synthetic NPoS stake asset"
        );
    }

    #[test]
    fn public_nexus_auto_bootstrap_requires_xor_alias_binding() {
        let peer = PeerId::new(
            checked_genesis_sign_keypair_with_algorithm(Algorithm::BlsNormal)
                .public_key()
                .clone(),
        );
        let topology_json = norito::json::to_json(&vec![peer.clone()]).unwrap();
        let args = Args {
            genesis_file: public_nexus_npos_genesis_file_without_xor_alias(),
            out_file: None,
            topology: Some(topology_json),
            peer_pops: vec![format!("{}=00", peer.public_key())],
            private_key: Some(test_private_key_hex()),
            seed: None,
            algorithm: Algorithm::Ed25519,
            config: None,
            consensus_mode: None,
            next_consensus_mode: None,
            mode_activation_height: None,
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
        let peer = PeerId::new(
            checked_genesis_sign_keypair_with_algorithm(Algorithm::BlsNormal)
                .public_key()
                .clone(),
        );
        let topology_json = norito::json::to_json(&vec![peer.clone()]).unwrap();
        let args = Args {
            genesis_file: public_taira_alias_backed_npos_genesis_file(),
            out_file: None,
            topology: Some(topology_json),
            peer_pops: vec![format!("{}=00", peer.public_key())],
            private_key: Some(test_private_key_hex()),
            seed: None,
            algorithm: Algorithm::Ed25519,
            config: Some(nexus_profile_with_stake_asset_id(
                "61CtjvNd9T3THAR65GsMVHr82Bjc",
            )),
            consensus_mode: None,
            next_consensus_mode: None,
            mode_activation_height: None,
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
        let peer = PeerId::new(
            checked_genesis_sign_keypair_with_algorithm(Algorithm::BlsNormal)
                .public_key()
                .clone(),
        );
        let topology_json = norito::json::to_json(&vec![peer.clone()]).unwrap();
        let args = Args {
            genesis_file: public_taira_conflicting_xor_alias_npos_genesis_file(),
            out_file: None,
            topology: Some(topology_json),
            peer_pops: vec![format!("{}=00", peer.public_key())],
            private_key: Some(test_private_key_hex()),
            seed: None,
            algorithm: Algorithm::Ed25519,
            config: None,
            consensus_mode: None,
            next_consensus_mode: None,
            mode_activation_height: None,
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
        let peer = PeerId::new(
            checked_genesis_sign_keypair_with_algorithm(Algorithm::BlsNormal)
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
            config: Some(nexus_profile_with_validator_modes(
                "admin_managed",
                "admin_managed",
            )),
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

        assert!(
            validators.is_empty(),
            "admin-managed lane configs must not auto-inject NPoS validator bootstrap"
        );
    }

    #[test]
    fn sign_links_genesis_account_into_ivm_without_reregistering_it() {
        let peer = PeerId::new(
            checked_genesis_sign_keypair_with_algorithm(Algorithm::BlsNormal)
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
            config: None,
            consensus_mode: None,
            next_consensus_mode: None,
            mode_activation_height: None,
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
    fn mode_activation_requires_next_mode_flag() {
        let args = Args {
            genesis_file: npos_genesis_file(),
            out_file: None,
            topology: None,
            peer_pops: Vec::new(),
            private_key: Some(test_private_key_hex()),
            seed: None,
            algorithm: Algorithm::Ed25519,
            config: None,
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
            config: None,
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
            config: None,
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
            config: None,
            consensus_mode: None,
            next_consensus_mode: None,
            mode_activation_height: None,
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
    fn sign_embeds_da_proof_policies_from_peer_config() {
        let args = Args {
            genesis_file: minimal_genesis_file(),
            out_file: None,
            topology: None,
            peer_pops: Vec::new(),
            private_key: Some(test_private_key_hex()),
            seed: None,
            algorithm: Algorithm::Ed25519,
            config: Some(nexus_profile_config_path()),
            consensus_mode: None,
            next_consensus_mode: None,
            mode_activation_height: None,
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
            topology: None,
            peer_pops: Vec::new(),
            private_key: Some(test_private_key_hex()),
            seed: None,
            algorithm: Algorithm::Ed25519,
            config: None,
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
            config: None,
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
        let genesis_file = tempfile::Builder::new()
            .prefix("kagami-genesis-test-")
            .tempfile()
            .expect("create temp genesis file");
        let manifest =
            GenesisBuilder::new_without_executor(ChainId::from("test-chain"), PathBuf::from("."))
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
            AssetDefinition::new(asset_definition_id.clone(), NumericSpec::default())
                .with_name("xor".to_owned())
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
                    AssetDefinition::new(asset_definition_id.clone(), NumericSpec::default())
                        .with_name("xor".to_owned())
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
                    AssetDefinition::new(canonical_xor.clone(), NumericSpec::default())
                        .with_name("xor".to_owned())
                        .with_metadata(Metadata::default()),
                ))
                .append_instruction(Register::asset_definition(
                    AssetDefinition::new(wrong_xor.clone(), NumericSpec::default())
                        .with_name("xor-shadow".to_owned())
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
                .with_consensus_mode(SumeragiConsensusMode::Npos)
                .with_consensus_meta();
        let json = norito::json::to_json_pretty(&manifest).expect("serialize genesis manifest");
        fs::write(genesis_file.path(), json).expect("write genesis json");
        let (_file, path) = genesis_file.keep().expect("persist temp genesis");
        path
    }

    fn test_private_key_hex() -> String {
        let kp = checked_genesis_sign_keypair_with_algorithm(Algorithm::Ed25519);
        let (_alg, bytes) = kp.private_key().to_bytes();
        hex::encode(bytes)
    }
}
