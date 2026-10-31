//! Signed genesis and shared node settings for a network of externally held validators.
//!
//! Each operator creates its validator's keys on its own host (`kagami node init`) and hands
//! over only `PUBLIC_KEY@HOST:PORT=POP_HEX`. [`generate_network`] checks that roster, composes
//! genesis with the bundle genesis helpers it shares with [`crate::sora_network`], signs it with
//! a fresh genesis key and writes
//! [`NETWORK_CONFIG_FILE`]: the chain, the roster, `[genesis]` and the consensus-bound settings
//! the signed genesis binds. A node includes that file with `extends = "network.toml"`, so no
//! validator private key leaves its host. Unlike a developer network ([`crate::localnet`]) no
//! generator holds the validators' keys, and unlike a Sora network it has no Sora profile.
use crate::{
    custody,
    genesis::{
        ConsensusPolicy,
        profile::{PUBLIC_NEXUS_CHAIN_ID, PUBLIC_TAIRA_CHAIN_ID, reject_retired_public_chain_id},
        validate_consensus_mode,
    },
    localnet::consensus_mode_label,
    sora_network::{
        BootstrapRegistrations, GENESIS_EXPECTED_HASH_FILE, GENESIS_PRIVATE_KEY_FILE,
        GENESIS_PUBLIC_KEY_FILE, GENESIS_SEED, GenesisConsensusPolicies, GenesisValidator,
        GenesisWriteContext, LOCALNET_BLOCK_MAX_TRANSACTIONS, LOCALNET_NEXUS_DOMAIN,
        LOCALNET_PIPELINE_TIME_MS, LocalnetClientIdentity, LocalnetNposBootstrapContext,
        append_localnet_contract_permissions_for_client, append_localnet_native_lane_policy,
        append_localnet_npos_bootstrap, append_localnet_service_accounts,
        append_localnet_service_fee_bootstrap, append_peer_pop, apply_parameter_overrides,
        generate_genesis_key_pair, generate_raw_genesis, insert_localnet_signed_nexus_settings,
        localnet_bootstrap_staking_policy, localnet_dataspace_fault_tolerance,
        localnet_ephemeral_identity, localnet_gas_account_id, localnet_gas_custody_parameter,
        localnet_npos_stake_amount, localnet_root_fee_policy, localnet_staking_policy,
        parse_localnet_peer_config, resolve_localnet_chain_discriminant,
        resolve_localnet_da_proof_policies, validate_localnet_crypto_policy,
        write_and_validate_genesis_expected_hash, write_genesis, write_genesis_key_files,
        write_owner_only_localnet_file, write_private_key_sidecar,
    },
};
use color_eyre::eyre::{Result, WrapErr as _, ensure, eyre};
use iroha_crypto::{ExposedPrivateKey, Hash, HashOf, KeyPair};
use iroha_data_model::{
    account::address::ChainDiscriminantGuard, parameter::system::SumeragiConsensusMode, prelude::*,
};
use iroha_genesis::{RawGenesisTransaction, init_instruction_registry};
use iroha_model_base::{
    chain::ChainId,
    domain::DomainId,
    topology::{MAX_VALIDATORS_PER_HEIGHT, is_valid_committee_size},
};
use iroha_primitives::addr::SocketAddr;
use rand::{TryRng as _, rngs::SysRng};
use std::{
    collections::BTreeSet,
    io::{BufWriter, Write},
    num::NonZeroU16,
    path::PathBuf,
    str::FromStr,
};
use zeroize::Zeroizing;

/// Shared node configuration that every validator includes with `extends`.
pub const NETWORK_CONFIG_FILE: &str = "network.toml";
/// Signed genesis block that every validator boots from.
pub const SIGNED_GENESIS_FILE: &str = "genesis.signed.nrt";
/// Torii endpoint of the operator client when none is requested: `kagami node init`'s default.
pub const DEFAULT_NETWORK_TORII_URL: &str = "http://127.0.0.1:8080/";
/// Config-bound genesis manifest, kept for review.
const GENESIS_MANIFEST_FILE: &str = "genesis.json";
/// Authenticated policy preimage used by node configuration diagnostics.
const NEXUS_CONTEXT_FILE: &str = "nexus-amx-context.v1.bin";
/// Owner-only key material that never leaves the creator.
const SECRETS_DIRECTORY: &str = "secrets";
/// Signing key of the operator account, referenced by the client configuration.
const OPERATOR_KEY_FILE: &str = "operator.key";
const CLIENT_CONFIG_FILE: &str = "client.toml";
/// Listeners of the throwaway reference node; validating its configuration never binds them.
const REFERENCE_P2P_ADDRESS: &str = "127.0.0.1:1337";
const REFERENCE_TORII_ADDRESS: &str = "127.0.0.1:8080";

/// One genesis validator as `kagami node init` prints it: `PUBLIC_KEY@HOST:PORT=POP_HEX`.
///
/// Parsing accepts only a BLS-normal key with a valid proof of possession and an address
/// other peers can dial, so a malformed roster entry fails before any output exists.
#[derive(Debug, Clone)]
pub struct NetworkValidator {
    public_key: iroha_crypto::PublicKey,
    pop: Vec<u8>,
    address: SocketAddr,
}

impl NetworkValidator {
    /// BLS-normal consensus key, which is also the validator's peer identity.
    pub fn public_key(&self) -> &iroha_crypto::PublicKey {
        &self.public_key
    }

    /// P2P address the other validators dial.
    pub fn address(&self) -> &SocketAddr {
        &self.address
    }

    /// The `public_key@host:port` roster entry of `trusted_peers`.
    fn trusted_peer(&self) -> String {
        format!("{}@{}", self.public_key, self.address)
    }

    fn pop_hex(&self) -> String {
        hex::encode(&self.pop)
    }
}

impl GenesisValidator for NetworkValidator {
    fn consensus_key(&self) -> &iroha_crypto::PublicKey {
        &self.public_key
    }
    fn pop(&self) -> &[u8] {
        &self.pop
    }
    fn account_id(&self, taira: bool) -> AccountId {
        debug_assert!(!taira, "external validators never join the Taira chain");
        AccountId::new(self.public_key.clone())
    }
}

impl FromStr for NetworkValidator {
    type Err = color_eyre::Report;

    fn from_str(value: &str) -> Result<Self> {
        let (peer, pop_hex) = value.rsplit_once('=').ok_or_else(|| {
            eyre!("validator `{value}` must have the form PUBLIC_KEY@HOST:PORT=POP_HEX")
        })?;
        ensure!(
            peer.contains('@'),
            "validator `{value}` must name its P2P address as PUBLIC_KEY@HOST:PORT"
        );
        let peer = peer
            .parse::<iroha_model_base::peer::Peer>()
            .map_err(|error| eyre!("validator `{value}`: {error}"))?;
        let public_key = peer.id().public_key().clone();
        ensure!(
            public_key.try_algorithm().ok() == Some(iroha_crypto::Algorithm::BlsNormal),
            "validator {public_key} must use a bls_normal consensus key"
        );
        let address = peer
            .address()
            .to_string()
            .parse::<iroha_config::parameters::user::ConfigSocketAddr>()
            .map_err(|error| eyre!("validator {public_key} address: {error}"))?
            .into_inner();
        ensure!(
            address.port() != 0,
            "validator {public_key} must name a nonzero P2P port"
        );
        let unspecified = address
            .to_string()
            .parse::<std::net::SocketAddr>()
            .is_ok_and(|address| address.ip().is_unspecified());
        ensure!(
            !unspecified,
            "validator {public_key} address {address} is a wildcard; use the address other peers dial"
        );
        let pop = hex::decode(pop_hex.strip_prefix("0x").unwrap_or(pop_hex))
            .map_err(|_| eyre!("validator {public_key} proof of possession is not hex"))?;
        iroha_crypto::bls_normal_pop_verify(&public_key, &pop).map_err(|_| {
            eyre!("validator {public_key} proof of possession does not verify for its key")
        })?;
        Ok(Self {
            public_key,
            pop,
            address,
        })
    }
}

impl core::fmt::Display for NetworkValidator {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}={}", self.trusted_peer(), self.pop_hex())
    }
}

/// Request to create a network whose validators hold their own keys.
pub struct NetworkOptions {
    /// Every genesis validator; `trusted_peers` lists them in this order.
    pub validators: Vec<NetworkValidator>,
    /// Canonical chain identifier; a fresh random one when absent.
    pub chain_id: Option<String>,
    /// Account-address chain prefix; the node default when absent.
    pub chain_discriminant: Option<u16>,
    /// Consensus mode committed in genesis.
    pub consensus_mode: SumeragiConsensusMode,
    /// Torii API the operator client dials; [`DEFAULT_NETWORK_TORII_URL`] when absent.
    pub torii_url: Option<String>,
    /// Fresh output directory, created owner-only.
    pub out_dir: PathBuf,
}

/// Create a network for externally held validators.
///
/// Writes the signed genesis with its expected hash and public key, the shared node
/// configuration [`NETWORK_CONFIG_FILE`], the config-bound genesis manifest, and an operator
/// client whose account holds the network's XOR and administrative permissions. The genesis
/// and operator private keys stay in the owner-only `secrets/` directory.
///
/// # Errors
/// Returns roster, chain or endpoint validation errors before any output exists, and key
/// generation, genesis staging or filesystem errors while writing the fresh output directory.
#[allow(clippy::too_many_lines)]
pub fn generate_network<T: Write>(opts: &NetworkOptions, writer: &mut BufWriter<T>) -> Result<()> {
    init_instruction_registry();
    let peers = validate_network_roster(&opts.validators)?;
    let chain_id = resolve_network_chain_id(opts.chain_id.as_deref())?;
    let chain_discriminant =
        resolve_localnet_chain_discriminant(&chain_id, opts.chain_discriminant)?;
    validate_consensus_mode(opts.consensus_mode, ConsensusPolicy::Any)?;
    let torii_url = resolve_network_torii_url(opts.torii_url.as_deref())?;
    // No output path exists until every request-level invariant holds, so an invalid
    // request can be retried with the same directory.
    let out_dir = custody::prepare_empty_private_directory(&opts.out_dir)
        .wrap_err("prepare a fresh owner-only network directory")?;
    let secrets_dir = custody::prepare_empty_private_directory(&out_dir.join(SECRETS_DIRECTORY))
        .wrap_err("prepare the owner-only network secrets directory")?;
    // Scope every account literal of genesis, configs and the client to this chain.
    let _chain_discriminant = chain_discriminant.map(ChainDiscriminantGuard::enter);
    let (genesis_public_key, genesis_private) = generate_genesis_key_pair(None, GENESIS_SEED)
        .wrap_err("generate the network genesis key pair")?;
    let operator = localnet_ephemeral_identity(None, b"operator-root")
        .wrap_err("generate the network operator key pair")?;
    let genesis = compose_network_genesis(
        &opts.validators,
        peers,
        &chain_id,
        chain_discriminant,
        opts.consensus_mode,
        &genesis_public_key,
        &operator.account_id,
    )?;
    let shared = render_network_config(
        &opts.validators,
        peers,
        &chain_id,
        chain_discriminant,
        opts.consensus_mode,
        &genesis_public_key,
    );
    let shared_table = crate::secret_toml::parse_table(&shared, "generated network.toml")?;
    let reference = ReferenceNode::generate()?;
    // The signing context is the shared file plus a throwaway node identity: consensus-bound
    // policy comes only from the shared sections, which every validator includes unchanged.
    let signing_config = parse_localnet_peer_config(
        &reference.config(&shared_table, NetworkGenesisIdentity::Unpublished)?,
        None,
    )?;
    let genesis = append_localnet_native_lane_policy(genesis, &signing_config.nexus)?;
    let genesis = genesis
        .with_consensus_mode(opts.consensus_mode)
        .with_consensus_meta()?;
    let genesis_public_key_path = out_dir.join(GENESIS_PUBLIC_KEY_FILE);
    let genesis_private_key_path = secrets_dir.join(GENESIS_PRIVATE_KEY_FILE);
    write_genesis_key_files(
        &genesis_public_key_path,
        &genesis_private_key_path,
        &genesis_public_key,
        &genesis_private,
    )?;
    let genesis_json_path = out_dir.join(GENESIS_MANIFEST_FILE);
    let genesis_signed_path = out_dir.join(SIGNED_GENESIS_FILE);
    let genesis_expected_hash_path = out_dir.join(GENESIS_EXPECTED_HASH_FILE);
    let genesis_expected_hash = write_genesis(GenesisWriteContext {
        creation_time_ms: None,
        manifest: &genesis,
        public_key: &genesis_public_key,
        private_key: genesis_private,
        config: &signing_config,
        chain_discriminant,
        json_path: &genesis_json_path,
        signed_path: &genesis_signed_path,
        policies: GenesisConsensusPolicies {
            da_proof_policies: Some(resolve_localnet_da_proof_policies(&signing_config)),
            confidential_policy_hash: iroha_core::state::default_genesis_confidential_policy_hash(),
        },
    })?;
    write_and_validate_genesis_expected_hash(
        &genesis_expected_hash_path,
        &genesis_signed_path,
        genesis_expected_hash,
    )?;
    let network_config_path = out_dir.join(NETWORK_CONFIG_FILE);
    custody::write(&network_config_path, shared.as_bytes())
        .wrap_err("write the shared network configuration")?;
    // A validator reads the published files through the shared configuration; prove that this
    // exact file resolves to the identity just signed before handing it out.
    let runtime_config = parse_localnet_peer_config(
        &reference.config(&shared_table, NetworkGenesisIdentity::Published)?,
        Some(&network_config_path),
    )
    .wrap_err("generated network.toml failed node configuration validation")?;
    ensure!(
        runtime_config.genesis.expected_hash == genesis_expected_hash,
        "generated network.toml resolves genesis {}, expected {genesis_expected_hash}",
        runtime_config.genesis.expected_hash
    );
    drop(reference);
    let operator_key_path = secrets_dir.join(OPERATOR_KEY_FILE);
    write_private_key_sidecar(&operator_key_path, operator.private_key.as_str())?;
    let client_config_path = out_dir.join(CLIENT_CONFIG_FILE);
    write_owner_only_localnet_file(
        &client_config_path,
        render_network_client_config(&chain_id, chain_discriminant, &torii_url, &operator)
            .as_bytes(),
    )?;
    custody::validate_private_tree(&out_dir, &[])
        .wrap_err("validate the fresh network artifact tree")?;
    writeln!(writer, "out_dir: {}", out_dir.display())?;
    writeln!(writer, "chain_id: {chain_id}")?;
    writeln!(
        writer,
        "consensus_mode: {}",
        consensus_mode_label(opts.consensus_mode)
    )?;
    writeln!(writer, "validators: {}", peers.get())?;
    writeln!(writer, "network_config: {}", network_config_path.display())?;
    writeln!(writer, "genesis_signed: {}", genesis_signed_path.display())?;
    writeln!(
        writer,
        "genesis_expected_hash: {}",
        genesis_expected_hash_path.display()
    )?;
    writeln!(
        writer,
        "genesis_public_key: {}",
        genesis_public_key_path.display()
    )?;
    writeln!(writer, "genesis_manifest: {}", genesis_json_path.display())?;
    writeln!(
        writer,
        "genesis_private_key: {}",
        genesis_private_key_path.display()
    )?;
    writeln!(writer, "client_config: {}", client_config_path.display())?;
    writeln!(
        writer,
        "operator_account: {}",
        operator.account_literal(chain_discriminant)
    )?;
    writeln!(
        writer,
        "distribute: {NETWORK_CONFIG_FILE} {SIGNED_GENESIS_FILE} {GENESIS_EXPECTED_HASH_FILE} {NEXUS_CONTEXT_FILE}"
    )?;
    writeln!(
        writer,
        "next: copy those four files into every node directory; start each validator the \
         first time with `iroha3d --config <DIR>/config.toml --sumeragi-assert-fresh-key` \
         and without that flag afterwards"
    )?;
    Ok(())
}

/// Require an exact Sumeragi `3f + 1` committee of distinct keys at distinct addresses.
fn validate_network_roster(validators: &[NetworkValidator]) -> Result<NonZeroU16> {
    let count = validators.len();
    ensure!(
        is_valid_committee_size(count),
        "the roster has {count} validators; a network needs an exact Sumeragi 3f+1 \
         committee (4, 7, 10, ... up to {MAX_VALIDATORS_PER_HEIGHT}), one `--validator` each"
    );
    let mut keys = BTreeSet::new();
    let mut addresses = BTreeSet::new();
    for validator in validators {
        ensure!(
            keys.insert(validator.public_key.clone()),
            "validator {} is listed more than once",
            validator.public_key
        );
        ensure!(
            addresses.insert(validator.address.to_string().to_ascii_lowercase()),
            "two validators share the P2P address {}",
            validator.address
        );
    }
    let count = u16::try_from(count).expect("a valid committee fits u16");
    Ok(NonZeroU16::new(count).expect("a valid committee is nonempty"))
}

/// Accept an explicit canonical chain identifier or create a fresh one; public SORA chain
/// identities belong to their own networks.
fn resolve_network_chain_id(configured: Option<&str>) -> Result<String> {
    let chain_id = match configured {
        Some(chain_id) => chain_id.to_owned(),
        None => fresh_network_chain_id()?,
    };
    reject_retired_public_chain_id(&chain_id)?;
    ensure!(
        chain_id != PUBLIC_TAIRA_CHAIN_ID && chain_id != PUBLIC_NEXUS_CHAIN_ID,
        "`--chain-id` {chain_id} belongs to a public SORA network; a new network needs its own"
    );
    chain_id
        .parse::<ChainId>()
        .wrap_err("`--chain-id` must be a canonical chain id")?;
    Ok(chain_id)
}

/// A random UUID-formatted chain identifier, so separate networks never share one by default.
fn fresh_network_chain_id() -> Result<String> {
    let mut bytes = [0_u8; 16];
    SysRng
        .try_fill_bytes(&mut bytes)
        .wrap_err("obtain OS entropy for the chain id")?;
    // RFC 9562 version 4 and variant bits.
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex = hex::encode(bytes);
    Ok(format!(
        "{}-{}-{}-{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    ))
}

fn resolve_network_torii_url(configured: Option<&str>) -> Result<url::Url> {
    let raw = configured.unwrap_or(DEFAULT_NETWORK_TORII_URL);
    let url = url::Url::parse(raw).wrap_err_with(|| format!("`--torii-url` {raw} is not a URL"))?;
    ensure!(
        matches!(url.scheme(), "http" | "https") && url.host().is_some(),
        "`--torii-url` {raw} must be an http or https URL with a host"
    );
    Ok(url)
}

/// Compose genesis from the bundle helpers: default bootstrap state, the funded operator,
/// their block parameters and fee policy, the validator roster with its PoPs, and the
/// consensus-specific validator bootstrap.
fn compose_network_genesis(
    validators: &[NetworkValidator],
    peers: NonZeroU16,
    chain_id: &str,
    chain_discriminant: Option<u16>,
    consensus_mode: SumeragiConsensusMode,
    genesis_public_key: &iroha_crypto::PublicKey,
    operator_account_id: &AccountId,
) -> Result<RawGenesisTransaction> {
    let npos = consensus_mode == SumeragiConsensusMode::Npos;
    let genesis_account_id = AccountId::new(genesis_public_key.clone());
    let gas_account_id = localnet_gas_account_id(genesis_public_key);
    let mut genesis = generate_raw_genesis(
        genesis_public_key,
        consensus_mode,
        chain_id,
        localnet_staking_policy(genesis_public_key, None, false)?,
    )?;
    genesis =
        append_localnet_service_accounts(genesis, &[Account::new(operator_account_id.clone())])?;
    // The operator also takes the onboarding role, so it receives every service balance.
    genesis = append_localnet_service_fee_bootstrap(
        genesis,
        &genesis_account_id,
        operator_account_id,
        operator_account_id,
    )?;
    genesis = apply_parameter_overrides(
        genesis,
        peers,
        Some(LOCALNET_PIPELINE_TIME_MS),
        LOCALNET_BLOCK_MAX_TRANSACTIONS,
        consensus_mode,
    )?;
    genesis = genesis
        .into_builder()
        .append_parameter(localnet_gas_custody_parameter(&gas_account_id))
        .append_parameter(Parameter::Custom(
            localnet_root_fee_policy(npos || chain_discriminant.is_some(), &gas_account_id)
                .into_custom_parameter()?,
        ))
        .build_raw()?;
    genesis = append_localnet_contract_permissions_for_client(
        genesis,
        &genesis_account_id,
        operator_account_id,
    )?;
    genesis = append_peer_pop(genesis, validators)?;
    if npos {
        let stake_amount = localnet_npos_stake_amount(&genesis.effective_parameters()?, None)?;
        genesis = append_localnet_npos_bootstrap(
            genesis,
            &LocalnetNposBootstrapContext {
                peers: validators,
                gas_account_id: &gas_account_id,
                stake_amount: &stake_amount,
                sora_profile: None,
                genesis_account_id: &genesis_account_id,
                client_account_id: operator_account_id,
                onboarding_account_id: operator_account_id,
                taira: false,
            },
        )?;
    } else {
        genesis = append_permissioned_support_accounts(genesis, validators, &gas_account_id)?;
    }
    validate_localnet_crypto_policy(genesis)
}

/// Permissioned networks register the Nexus support accounts and every validator account, but
/// stake nothing: staking is NPoS-only (the network XOR identity lives in the signed `NPoS`
/// parameters), and the committee is the genesis roster (`RegisterPeerWithPop`).
fn append_permissioned_support_accounts(
    genesis: RawGenesisTransaction,
    validators: &[NetworkValidator],
    gas_account_id: &AccountId,
) -> Result<RawGenesisTransaction> {
    let policy = localnet_bootstrap_staking_policy(&genesis)?;
    let nexus_domain = DomainId::parse_fully_qualified(LOCALNET_NEXUS_DOMAIN)?;
    let mut registrations = BootstrapRegistrations::from_manifest(&genesis);
    let mut builder = genesis.into_builder().next_transaction();
    if !registrations.domains.contains(&nexus_domain) {
        builder = builder.append_instruction(Register::domain(Domain::new(nexus_domain)));
    }
    for account in [
        gas_account_id,
        &policy.stake_escrow_account_id,
        &policy.slash_sink_account_id,
    ] {
        if registrations.accounts.insert(account.clone()) {
            builder = builder.append_instruction(Register::account(Account::new(account.clone())));
        }
    }
    for validator in validators {
        let validator_id = validator.account_id(false);
        if registrations.accounts.insert(validator_id.clone()) {
            builder = builder.append_instruction(Register::account(Account::new(validator_id)));
        }
    }
    builder.build_raw()
}

/// TOML string literal for a value rendered into a generated file.
fn toml_string(value: &str) -> String {
    toml::Value::String(value.to_owned()).to_string()
}

/// Render the shared node configuration: chain, roster, `[genesis]` and the `[nexus]`
/// settings the signed genesis binds.
fn render_network_config(
    validators: &[NetworkValidator],
    peers: NonZeroU16,
    chain_id: &str,
    chain_discriminant: Option<u16>,
    consensus_mode: SumeragiConsensusMode,
    genesis_public_key: &iroha_crypto::PublicKey,
) -> String {
    use std::fmt::Write as _;
    let mut rendered = format!(
        "# Shared configuration of network {chain_id}, created by `kagami network create`.\n\
         #\n\
         # Every validator's config.toml includes this file with `extends = \"{NETWORK_CONFIG_FILE}\"`\n\
         # and keeps it beside {SIGNED_GENESIS_FILE} and {GENESIS_EXPECTED_HASH_FILE}. The signed\n\
         # genesis binds the [nexus] settings below: a node that drops or changes them fails at\n\
         # startup with a signed genesis policy mismatch. Create a new network to change the roster.\n\
         chain = {}\n",
        toml_string(chain_id)
    );
    if let Some(chain_discriminant) = chain_discriminant {
        writeln!(rendered, "chain_discriminant = {chain_discriminant}")
            .expect("writing to a String cannot fail");
    }
    rendered.push_str("trusted_peers = [\n");
    for validator in validators {
        writeln!(rendered, "  {},", toml_string(&validator.trusted_peer()))
            .expect("writing to a String cannot fail");
    }
    rendered.push_str("]\ntrusted_peers_pop = [\n");
    for validator in validators {
        writeln!(
            rendered,
            "  {{ public_key = {}, pop_hex = {} }},",
            toml_string(&validator.public_key.to_string()),
            toml_string(&validator.pop_hex())
        )
        .expect("writing to a String cannot fail");
    }
    write!(
        rendered,
        "]\n\n[genesis]\npublic_key = {}\nfile = {}\nexpected_hash_file = {}\n\n",
        toml_string(&genesis_public_key.to_string()),
        toml_string(SIGNED_GENESIS_FILE),
        toml_string(GENESIS_EXPECTED_HASH_FILE)
    )
    .expect("writing to a String cannot fail");
    let mut nexus = toml::Table::new();
    let dataspace_fault_tolerance = (consensus_mode == SumeragiConsensusMode::Npos)
        .then(|| localnet_dataspace_fault_tolerance(peers));
    insert_localnet_signed_nexus_settings(&mut nexus, None, false, dataspace_fault_tolerance, None);
    let bound = toml::Table::from_iter([("nexus".to_owned(), toml::Value::Table(nexus))]);
    rendered.push_str(&toml::to_string(&bound).expect("generated [nexus] settings encode as TOML"));
    rendered
}

/// Whether a validation config claims the genesis identity before or after it is published.
#[derive(Clone, Copy)]
enum NetworkGenesisIdentity {
    /// Signing runs before the expected hash exists; policy derivation never reads it.
    Unpublished,
    /// Exactly the shared file's `expected_hash_file`, as every validator reads it.
    Published,
}

/// A throwaway validator identity that completes the shared configuration into a node
/// configuration, so it can be parsed exactly as a validator parses its own.
struct ReferenceNode {
    validator: KeyPair,
    transport_private_key: Zeroizing<String>,
    streaming_private_key: Zeroizing<String>,
}

impl ReferenceNode {
    fn generate() -> Result<Self> {
        let validator = KeyPair::try_random_with_algorithm(iroha_crypto::Algorithm::BlsNormal)
            .wrap_err("generate the reference validator key")?;
        let transport = KeyPair::try_random_with_algorithm(iroha_crypto::Algorithm::Ed25519)
            .wrap_err("generate the reference transport key")?;
        let streaming = KeyPair::try_random_with_algorithm(iroha_crypto::Algorithm::Ed25519)
            .wrap_err("generate the reference streaming key")?;
        let encode = |key_pair: &KeyPair| -> Result<Zeroizing<String>> {
            ExposedPrivateKey(key_pair.private_key().clone())
                .try_to_multihash_string()
                .map(Zeroizing::new)
                .wrap_err("encode a reference private key")
        };
        Ok(Self {
            transport_private_key: encode(&transport)?,
            streaming_private_key: encode(&streaming)?,
            validator,
        })
    }

    /// Render the shared table with this node's identity added.
    fn config(
        &self,
        shared: &toml::Table,
        identity: NetworkGenesisIdentity,
    ) -> Result<Zeroizing<String>> {
        use toml::Value;
        let mut root = crate::secret_toml::Table::new(shared.clone());
        root.insert(
            "public_key".into(),
            Value::String(self.validator.public_key().to_string()),
        );
        root.insert(
            "private_key".into(),
            Value::String(
                ExposedPrivateKey(self.validator.private_key().clone())
                    .try_to_multihash_string()
                    .wrap_err("encode the reference validator key")?,
            ),
        );
        root.insert(
            "soranet_transport_private_key".into(),
            Value::String(self.transport_private_key.as_str().to_owned()),
        );
        let streaming = toml::Table::from_iter([(
            "identity_private_key".to_owned(),
            Value::String(self.streaming_private_key.as_str().to_owned()),
        )]);
        root.insert("streaming".into(), Value::Table(streaming));
        let network = toml::Table::from_iter(["address", "public_address"].map(|key| {
            (
                key.to_owned(),
                Value::String(REFERENCE_P2P_ADDRESS.to_owned()),
            )
        }));
        root.insert("network".into(), Value::Table(network));
        let torii = toml::Table::from_iter([(
            "address".to_owned(),
            Value::String(REFERENCE_TORII_ADDRESS.to_owned()),
        )]);
        root.insert("torii".into(), Value::Table(torii));
        if let NetworkGenesisIdentity::Unpublished = identity {
            let genesis = root
                .get_mut("genesis")
                .and_then(Value::as_table_mut)
                .ok_or_else(|| eyre!("generated network.toml has no [genesis] table"))?;
            genesis.remove("expected_hash_file");
            genesis.insert(
                "expected_hash".into(),
                Value::String(
                    NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
                        b"Kagami network policy-derivation placeholder",
                    )))
                    .to_string(),
                ),
            );
        }
        toml::to_string(&*root)
            .map(Zeroizing::new)
            .map_err(|_| eyre!("cannot encode the reference node configuration"))
    }
}

/// The operator client: the shared chain identity, a Torii endpoint and the operator account,
/// whose key stays in `secrets/`.
fn render_network_client_config(
    chain_id: &str,
    chain_discriminant: Option<u16>,
    torii_url: &url::Url,
    operator: &LocalnetClientIdentity,
) -> String {
    let chain_discriminant = chain_discriminant
        .unwrap_or_else(iroha_config::parameters::defaults::common::chain_discriminant);
    format!(
        "# Operator client of network {chain_id}, created by `kagami network create`.\n\
         # The operator account holds the network's XOR and its administrative permissions.\n\
         # Point torii_url at any validator's Torii API.\n\
         chain = {}\n\
         network_id_file = {}\n\
         torii_url = {}\n\
         \n\
         [account]\n\
         chain_discriminant = {chain_discriminant}\n\
         public_key = {}\n\
         private_key_file = {}\n",
        toml_string(chain_id),
        toml_string(GENESIS_EXPECTED_HASH_FILE),
        toml_string(torii_url.as_str()),
        toml_string(&operator.public_key.to_string()),
        toml_string(&format!("{SECRETS_DIRECTORY}/{OPERATOR_KEY_FILE}")),
    )
}

#[cfg(test)]
mod tests;
