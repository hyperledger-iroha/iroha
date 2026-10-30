//! Developer localnets: disposable networks for building and testing against Iroha.
//!
//! A localnet carries what local development needs and nothing else: an exact `3f + 1`
//! validator committee, one funded client account that may deploy contracts and register
//! accounts, the XOR fee asset, and peer configurations limited to node essentials plus a few
//! developer conveniences. It never takes a public chain identity, a Sora network profile or
//! Taira policy; Nexus, Taira and the private dataspace presets come from
//! [`crate::sora_network`]. No faucet, account onboarding, fee sponsorship, alias setup,
//! VPN operator or peer telemetry polling is generated.
//!
//! [`generate_localnet`] writes a bundle with `start.sh` and `stop.sh` launchers for
//! `kagami localnet generate`. [`prepare_localnet_at`] prepares the same network for the
//! managed `kagami localnet up` engine, which owns the validator processes itself. Storage is
//! an opt-in layer of managed networks: [`LocalnetServiceProfile::StreamTokenAuthorities`]
//! (`kagami localnet up --with-storage`) adds the [`service_authorities`] (three storage
//! providers and their network services) to the same generation.

mod bundle;
mod config;
mod genesis;
pub mod service_authorities;
#[cfg(test)]
mod tests;

pub use service_authorities::LocalnetServiceProfile;

use crate::{
    custody,
    genesis::profile::{
        PUBLIC_NEXUS_CHAIN_ID, PUBLIC_TAIRA_CHAIN_ID, TAIRA_XOR_ASSET_DEFINITION_ID,
    },
    host::CanonicalHost,
};
use color_eyre::eyre::{Result, WrapErr as _, ensure, eyre};
use iroha::config::HttpTransportPolicy;
use iroha_crypto::{ExposedPrivateKey, KeyPair, PublicKey};
use iroha_data_model::{
    account::address::ChainDiscriminantGuard, parameter::system::SumeragiConsensusMode, prelude::*,
};
use iroha_model_base::{
    chain::ChainId,
    domain::DomainId,
    topology::{MAX_VALIDATORS_PER_HEIGHT, is_valid_committee_size},
};
use std::{
    collections::BTreeSet,
    io::{BufWriter, Write},
    num::NonZeroU16,
    path::{Path, PathBuf},
};
use zeroize::{Zeroize as _, Zeroizing};

/// Default chain label for disposable developer networks.
pub const DEFAULT_CHAIN_ID: &str = "00000000-0000-0000-0000-000000000000";
/// Default host interface for P2P and Torii listeners.
pub const DEFAULT_BIND_HOST: &str = "0.0.0.0";
/// Default host advertised to peers and clients.
pub const DEFAULT_PUBLIC_HOST: &str = "127.0.0.1";
/// Public genesis verifier key emitted for runtime configuration.
pub const GENESIS_PUBLIC_KEY_FILE: &str = "genesis.public_key";
/// Exact signed-genesis consensus-header hash emitted for runtime configuration.
pub const GENESIS_EXPECTED_HASH_FILE: &str = "genesis.expected_hash";
/// Owner-only genesis signing key emitted for offline custody.
pub const GENESIS_PRIVATE_KEY_FILE: &str = "genesis.private_key";
/// Name of the optional sample asset definition registered by `--sample-asset`.
pub const LOCALNET_SAMPLE_ASSET_NAME: &str = "sample";

/// Deterministic key domain of the dedicated transport identity (`soranet_transport_*`).
const SORANET_TRANSPORT_SEED_DOMAIN: &[u8] = b"iroha:kagami:localnet:soranet-transport:v1|";
/// Deterministic key domain of the streaming identity.
const STREAMING_IDENTITY_SEED_DOMAIN: &[u8] = b"iroha:kagami:localnet:streaming-identity:v1|";
/// Seed suffix of the genesis signing key.
const GENESIS_SEED: &[u8] = b"genesis";
/// Seed label of the client (ledger) signer.
const CLIENT_SEED_LABEL: &[u8] = b"operator-root";
/// Seed label of the HTTP operator signer used for operator-only Torii reads.
const OPERATOR_SEED_LABEL: &[u8] = b"http-operator-root";
/// Smallest committee a localnet generates.
const MIN_PEERS: u16 = 4;
/// Signed block cadence when the caller does not override it.
const DEFAULT_BLOCK_CADENCE_MS: u64 = 1_000;
/// Runtime credential directory beside `client.toml`.
pub(crate) const RUNTIME_DIRECTORY: &str = "runtime";
/// HTTP operator signer file inside [`RUNTIME_DIRECTORY`].
pub(crate) const OPERATOR_SIGNER_KEY_FILE: &str = "operator-signer.key";
/// Placeholder client used by callers before the generated client identity exists.
const PLACEHOLDER_CLIENT_PUBLIC_KEY: &str =
    "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03";
/// Domain of the optional sample asset definition.
const SAMPLE_ASSET_DOMAIN: &str = "wonderland.universal";
/// Initial supply minted for each explicitly requested asset definition.
const REQUESTED_ASSET_INITIAL_QUANTITY: u64 = 1_000_000_000;

/// Asset definition plus optional minting target for sample generation.
#[derive(Debug, Clone)]
pub struct AssetSpec {
    /// Canonical asset definition ID (unprefixed Base58 address).
    pub id: String,
    /// Human-readable display name for the asset definition.
    pub name: String,
    /// Optional leased alias binding to attach after registration.
    pub alias: Option<String>,
    /// Account that should own the asset definition after genesis completes.
    pub owned_by: AccountId,
    /// Account that should receive the minted supply.
    pub mint_to: AccountId,
    /// Quantity to mint for this asset definition.
    pub quantity: u64,
}

/// Localnet performance profiles for 10k TPS / 1s finality runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalnetPerfProfile {
    /// 10k TPS / 1s finality baseline for permissioned mode.
    Throughput10kPermissioned,
    /// 10k TPS / 1s finality baseline for `NPoS` mode.
    Throughput10kNpos,
}

impl LocalnetPerfProfile {
    /// Consensus mode required by this throughput preset.
    pub fn consensus_mode(self) -> SumeragiConsensusMode {
        match self {
            Self::Throughput10kPermissioned => SumeragiConsensusMode::Permissioned,
            Self::Throughput10kNpos => SumeragiConsensusMode::Npos,
        }
    }
}

/// Stable command and configuration spelling of a consensus mode.
pub fn consensus_mode_label(mode: SumeragiConsensusMode) -> &'static str {
    match mode {
        SumeragiConsensusMode::Permissioned => "permissioned",
        SumeragiConsensusMode::Npos => "npos",
    }
}

/// Placeholder client account for asset specs built before generation.
///
/// Generation replaces it with the freshly generated client account.
pub fn localnet_client_account_id() -> AccountId {
    AccountId::new(
        PLACEHOLDER_CLIENT_PUBLIC_KEY
            .parse()
            .expect("placeholder client public key must parse"),
    )
}

/// Canonical optional sample-asset definition address.
pub fn localnet_sample_asset_literal() -> String {
    AssetDefinitionId::derive_from_components(
        DomainId::parse_fully_qualified(SAMPLE_ASSET_DOMAIN)
            .expect("static sample asset domain must remain valid"),
        LOCALNET_SAMPLE_ASSET_NAME
            .parse()
            .expect("static sample asset name must remain valid"),
    )
    .to_string()
}

/// Validate an explicitly requested definition and build its developer bootstrap specification.
///
/// # Errors
/// Returns an error when the supplied asset definition address is empty or invalid.
pub fn requested_localnet_asset_spec(asset_definition_id: &str) -> Result<AssetSpec> {
    let id = asset_definition_id.trim();
    ensure!(!id.is_empty(), "asset definition id must not be empty");
    AssetDefinitionId::parse_address_literal(id)
        .wrap_err_with(|| format!("invalid asset definition id `{id}`"))?;
    let client = localnet_client_account_id();
    Ok(AssetSpec {
        id: id.to_owned(),
        name: format!("Localnet asset {id}"),
        alias: None,
        owned_by: client.clone(),
        mint_to: client,
        quantity: REQUESTED_ASSET_INITIAL_QUANTITY,
    })
}

/// User-facing options for generating a developer localnet.
pub struct LocalnetOptions {
    /// Optional localnet performance profile (throughput presets).
    pub perf_profile: Option<LocalnetPerfProfile>,
    /// Number of peers to create (an exact `3f + 1` committee, minimum four).
    pub peers: NonZeroU16,
    /// Optional seed to make key generation reproducible.
    pub seed: Option<String>,
    /// Host interface to bind P2P and Torii listeners to (host/IP only, no port).
    pub bind_host: String,
    /// Host peers should gossip to and clients should dial (host/IP only, no port).
    pub public_host: String,
    /// Base Torii API port; each peer increments this by one.
    pub base_api_port: u16,
    /// Base P2P port; each peer increments this by one.
    pub base_p2p_port: u16,
    /// Output directory for configs, scripts, and genesis.
    pub out_dir: PathBuf,
    /// Additional accounts to pre-register beyond the client.
    pub extra_accounts: u16,
    /// Additional asset specs to register and optionally mint.
    pub assets: Vec<AssetSpec>,
    /// Optional signed-genesis block cadence override in milliseconds.
    /// If unset, localnet uses a one-second cadence.
    pub block_cadence_ms: Option<u64>,
    /// Consensus mode to commit in signed genesis.
    pub consensus_mode: SumeragiConsensusMode,
}

impl Drop for LocalnetOptions {
    fn drop(&mut self) {
        if let Some(seed) = self.seed.as_mut() {
            seed.zeroize();
        }
    }
}

/// Generate a developer localnet under the default developer chain identity.
///
/// # Errors
/// Returns validation, key generation, genesis staging, filesystem, or output errors before a
/// complete network can be generated.
pub fn generate_localnet<T: Write>(
    opts: &LocalnetOptions,
    writer: &mut BufWriter<T>,
) -> Result<()> {
    generate_localnet_with_chain(opts, writer, None, None)
}

/// Generate a developer localnet with an optional chain identity and account-address prefix.
///
/// # Errors
/// Rejects a public chain identity or any invalid option before creating output; otherwise
/// returns key generation, genesis staging, filesystem, or output errors before a complete
/// network can be generated.
pub fn generate_localnet_with_chain<T: Write>(
    opts: &LocalnetOptions,
    writer: &mut BufWriter<T>,
    chain_id: Option<&str>,
    chain_discriminant: Option<u16>,
) -> Result<()> {
    let request = Request::new(opts, chain_id, chain_discriminant, Mode::Bundle)?;
    let generated = generate(&request)?;
    bundle::write_summary(writer, &request, &generated)
}

/// Validate a developer chain identity.
///
/// # Errors
/// Rejects the public Taira and Nexus chain identities, which only Sora network bundles may
/// use, and every empty, padded, retired or noncanonical chain identity.
pub fn resolve_localnet_chain_id(configured: Option<&str>) -> Result<String> {
    let chain_id = configured.unwrap_or(DEFAULT_CHAIN_ID);
    ensure!(
        chain_id != PUBLIC_TAIRA_CHAIN_ID && chain_id != PUBLIC_NEXUS_CHAIN_ID,
        "localnets are developer networks and cannot use the public chain identity {chain_id}; generate public Sora network bundles with `kagami sora-network generate`"
    );
    ensure!(!chain_id.is_empty(), "`--chain-id` must not be empty");
    ensure!(
        chain_id.trim() == chain_id,
        "`--chain-id` must not contain leading or trailing whitespace"
    );
    crate::genesis::profile::reject_retired_public_chain_id(chain_id)?;
    chain_id
        .parse::<ChainId>()
        .wrap_err("`--chain-id` must be canonical")?;
    Ok(chain_id.to_owned())
}

/// Prepare a fresh managed developer network with storage in `directory`.
///
/// # Errors
/// Returns preparation failures, invalid generated client configuration, or filesystem errors.
pub fn prepare_localnet(
    name: &str,
    directory: &Path,
    ports: &crate::managed::LocalnetPorts,
) -> crate::managed::Result<crate::managed::PreparedLocalnet> {
    prepare_localnet_at(
        name,
        directory,
        ports,
        LocalnetServiceProfile::StreamTokenAuthorities,
        None,
    )
}

/// Prepare a fresh managed developer network in `directory` for `kagami localnet up`.
///
/// The managed engine stages the generation and publishes it under `publication_root`, so
/// every rendered path names that final location. The network has four loopback validators
/// with fresh keys; [`LocalnetServiceProfile::StreamTokenAuthorities`] adds the storage layer.
///
/// # Errors
/// Returns preparation failures, invalid generated client configuration, or filesystem errors.
pub(crate) fn prepare_localnet_at(
    name: &str,
    directory: &Path,
    ports: &crate::managed::LocalnetPorts,
    service_profile: LocalnetServiceProfile,
    publication_root: Option<&Path>,
) -> crate::managed::Result<crate::managed::PreparedLocalnet> {
    use crate::managed::{Error, ManagedContext, ManagedPeer, PreparedLocalnet};
    let opts = LocalnetOptions {
        perf_profile: None,
        peers: NonZeroU16::new(MANAGED_PEERS).expect("managed peer count is nonzero"),
        seed: None,
        bind_host: "127.0.0.1".into(),
        public_host: "127.0.0.1".into(),
        base_api_port: ports.base_api,
        base_p2p_port: ports.base_p2p,
        out_dir: directory.to_path_buf(),
        extra_accounts: 0,
        assets: Vec::new(),
        block_cadence_ms: None,
        consensus_mode: SumeragiConsensusMode::Permissioned,
    };
    Request::new(
        &opts,
        None,
        None,
        Mode::Managed {
            publication_root,
            service_profile,
        },
    )
    .and_then(|request| generate(&request))
    .map_err(|error| Error::Invalid(format!("localnet preparation failed: {error}")))?;
    let directory = directory.canonicalize()?;
    let publication_root = publication_root.map_or_else(|| directory.clone(), Path::to_path_buf);
    let client_config = directory.join("client.toml");
    let bytes = iroha_fs::read_private(&client_config, 1024 * 1024)?;
    let (config, _) =
        iroha::config::Config::load_bytes_with_musubi_publication(&client_config, &bytes)
            .map_err(|_| Error::Invalid("generated client configuration is invalid".into()))?;
    let _profile = ChainDiscriminantGuard::enter(config.account_chain_discriminant);
    let context = ManagedContext {
        name: name.into(),
        chain_id: config.chain.to_string(),
        network_id: config.network_id.to_string(),
        account_id: config.account.to_string(),
        torii_url: config.torii_api_url.to_string(),
        client_config: publication_root.join("client.toml"),
        dataspace_alias: "universal".into(),
        dataspace_id: 0,
    };
    let peers = (0..MANAGED_PEERS)
        .map(|index| ManagedPeer {
            config_path: publication_root.join(format!("peer{index}.toml")),
            torii_url: format!("http://127.0.0.1:{}/", ports.base_api + index),
            log_name: format!("peer{index}.log"),
        })
        .collect();
    Ok(PreparedLocalnet {
        service_profile,
        context,
        peers,
    })
}

/// Committee size of managed developer networks.
const MANAGED_PEERS: u16 = 4;

/// Where and how a generation is published.
#[derive(Clone, Copy)]
enum Mode<'a> {
    /// Operator bundle with `start.sh`/`stop.sh`, rendered in place.
    Bundle,
    /// Managed generation with its service layers, rendered for its final publication root
    /// (the generated directory itself when there is none).
    Managed {
        publication_root: Option<&'a Path>,
        service_profile: LocalnetServiceProfile,
    },
}

/// A validated generation request.
struct Request<'a> {
    opts: &'a LocalnetOptions,
    chain_id: String,
    chain_discriminant: Option<u16>,
    bind_host: CanonicalHost,
    public_host: CanonicalHost,
    block_cadence_ms: u64,
    mode: Mode<'a>,
}

impl<'a> Request<'a> {
    /// Validate every option before any output is created, so a refused request is retryable
    /// with the same output path.
    fn new(
        opts: &'a LocalnetOptions,
        chain_id: Option<&str>,
        chain_discriminant: Option<u16>,
        mode: Mode<'a>,
    ) -> Result<Self> {
        let chain_id = resolve_localnet_chain_id(chain_id)?;
        validate_asset_specs(&opts.assets)?;
        let peers = opts.peers.get();
        ensure!(
            peers >= MIN_PEERS,
            "`--peers` must be at least {MIN_PEERS} so generated localnets exercise a representative committee with mandatory RS16 data availability"
        );
        ensure!(
            usize::from(peers) <= MAX_VALIDATORS_PER_HEIGHT,
            "`--peers` ({peers}) exceeds the Sumeragi protocol maximum validator roster of {MAX_VALIDATORS_PER_HEIGHT}"
        );
        ensure!(
            is_valid_committee_size(usize::from(peers)),
            "`--peers` ({peers}) must form an exact Sumeragi 3f+1 validator committee in the supported range 4..={MAX_VALIDATORS_PER_HEIGHT}"
        );
        if let Some(profile) = opts.perf_profile {
            ensure!(
                opts.consensus_mode == profile.consensus_mode(),
                "`--perf-profile` {profile:?} requires `--consensus-mode {}`",
                consensus_mode_label(profile.consensus_mode())
            );
        }
        if let Some(cadence) = opts.block_cadence_ms {
            ensure!(
                cadence > 0,
                "`--block-cadence-ms` must be greater than zero"
            );
        }
        validate_port_ranges(opts.peers, opts.base_api_port, opts.base_p2p_port)?;
        let bind_host = CanonicalHost::parse(&opts.bind_host, "--bind-host")?;
        let public_host = CanonicalHost::parse(&opts.public_host, "--public-host")?;
        if let Mode::Managed {
            service_profile: LocalnetServiceProfile::StreamTokenAuthorities,
            ..
        } = mode
        {
            // Storage providers bind the original committee and loopback Torii endpoints.
            ensure!(
                peers == MANAGED_PEERS
                    && opts.consensus_mode == SumeragiConsensusMode::Permissioned
                    && opts.perf_profile.is_none()
                    && public_host.url_host() == "127.0.0.1",
                "the storage layer requires the managed four-validator permissioned loopback profile"
            );
        }
        crate::shell::quote_path(&opts.out_dir)
            .wrap_err("validate requested localnet output path for shell handoff commands")?;
        Ok(Self {
            opts,
            chain_id,
            chain_discriminant,
            bind_host,
            public_host,
            block_cadence_ms: opts.block_cadence_ms.unwrap_or(DEFAULT_BLOCK_CADENCE_MS),
            mode,
        })
    }

    fn managed(&self) -> bool {
        matches!(self.mode, Mode::Managed { .. })
    }

    /// Service layers of this generation; operator bundles carry none.
    fn service_profile(&self) -> LocalnetServiceProfile {
        match self.mode {
            Mode::Bundle => LocalnetServiceProfile::Standard,
            Mode::Managed {
                service_profile, ..
            } => service_profile,
        }
    }

    /// Root the rendered configurations name: the publication root for managed generations.
    fn render_root<'b>(&'b self, out_dir: &'b Path) -> &'b Path {
        match self.mode {
            Mode::Bundle => out_dir,
            Mode::Managed {
                publication_root, ..
            } => publication_root.unwrap_or(out_dir),
        }
    }
}

/// Every validator identity and its ports.
struct Peer {
    public_key: PublicKey,
    private_key: ExposedPrivateKey,
    pop: Vec<u8>,
    transport_public_key: PublicKey,
    transport_private_key: ExposedPrivateKey,
    streaming_public_key: PublicKey,
    streaming_private_key: ExposedPrivateKey,
    api_port: u16,
    p2p_port: u16,
}

/// An ordinary Ed25519 signer.
struct Identity {
    account_id: AccountId,
    public_key: PublicKey,
    private_key: Zeroizing<String>,
}

/// Facts about a completed generation used for the summary and managed context.
struct Generated {
    out_dir: PathBuf,
    client: Identity,
}

fn generate(request: &Request<'_>) -> Result<Generated> {
    iroha_genesis::init_instruction_registry();
    let opts = request.opts;
    // No output exists until every request-level invariant has been checked.
    let out_dir = custody::prepare_empty_private_directory(&opts.out_dir)
        .wrap_err("prepare fresh localnet private output directory")?;
    let _chain_discriminant = request
        .chain_discriminant
        .map(ChainDiscriminantGuard::enter);
    let seed = opts.seed.as_ref().map(String::as_bytes);
    let peers = build_peers(
        opts.peers.get(),
        seed,
        opts.base_api_port,
        opts.base_p2p_port,
    )?;
    let client = ephemeral_identity(seed, CLIENT_SEED_LABEL)?;
    let operator = ephemeral_identity(seed, OPERATOR_SEED_LABEL)?;
    ensure!(
        client.public_key != operator.public_key,
        "localnet ledger and HTTP operator signers must be distinct"
    );
    let (genesis_public_key, genesis_private_key) = genesis_key_pair(seed)?;
    bundle::write_scaffold(&out_dir, &operator)?;
    let authorities = service_authorities::generate(
        request.service_profile(),
        &out_dir,
        seed,
        &client,
        &operator,
    )?;
    let layout = config::Layout {
        request,
        peers: &peers,
        genesis_public_key: &genesis_public_key,
        operator_public_key: &operator.public_key,
        authorities: authorities.as_ref(),
    };
    layout.prepare_directories(&out_dir)?;
    let bootstrap = layout.parse_bootstrap(&out_dir)?;
    let genesis = genesis::write(genesis::Inputs {
        request,
        out_dir: &out_dir,
        peers: &peers,
        genesis_public_key: &genesis_public_key,
        genesis_private_key,
        client: &client.account_id,
        config: &bootstrap,
        authorities: authorities.as_ref(),
    })?;
    let peer_configs = layout.write_peer_configs(&out_dir, genesis)?;
    let publication = authorities
        .as_ref()
        .map(|authorities| {
            publish_storage_layer(
                authorities,
                &out_dir,
                genesis,
                &client.account_id,
                &peer_configs,
                opts.base_api_port,
            )
        })
        .transpose()?;
    drop(peer_configs);
    config::write_client_config(
        &out_dir,
        request,
        &client,
        if request.managed() {
            HttpTransportPolicy::DirectLoopback
        } else {
            HttpTransportPolicy::Ordinary
        },
        publication.as_ref().map(|publication| &publication.table),
    )?;
    if let Some(publication) = &publication {
        publication.initialize_namespace_parent(&out_dir)?;
    }
    if request.managed() {
        custody::validate_private_tree(&out_dir, &[])
            .wrap_err("validate managed localnet private artifact tree")?;
    } else {
        bundle::write_lifecycle(&out_dir, request)?;
    }
    Ok(Generated { out_dir, client })
}

/// Publish the storage layer once genesis is signed: the authority manifest, provider
/// attestation custody, the publication service journal, and the publication client image
/// derived from peer 0, which the client configuration embeds.
fn publish_storage_layer(
    authorities: &service_authorities::GeneratedAuthorities,
    out_dir: &Path,
    genesis: iroha_crypto::HashOf<iroha_data_model::block::BlockHeader>,
    client: &AccountId,
    peer_configs: &[iroha_config::parameters::actual::Root],
    base_api_port: u16,
) -> Result<service_authorities::publication_client::GeneratedPublicationClient> {
    authorities.publish(out_dir, genesis, client)?;
    let json_path = out_dir.join("genesis.json");
    let signed = iroha_fs::read_private(
        out_dir.join("genesis.signed.nrt"),
        iroha_genesis::SIGNED_GENESIS_MAX_BYTES_V1,
    )?;
    // Staging binds consensus commitments into the published manifest: project the exact
    // persisted bound bytes paired with the signed block.
    let bound_json = iroha_fs::read_private(
        &json_path,
        iroha_genesis::GENESIS_MANIFEST_JSON_MAX_BYTES_V1,
    )?;
    iroha_genesis::validate_genesis_manifest_json(&bound_json)?;
    let bound_manifest =
        iroha_genesis::RawGenesisTransaction::from_json_slice_at_path(&bound_json, &json_path)?;
    let selection = authorities.publication_client_selection(
        &bound_manifest,
        &signed,
        &peer_configs[0],
        out_dir,
        client,
        base_api_port,
    )?;
    for (index, config) in peer_configs.iter().enumerate().take(3) {
        authorities.initialize_native_attestation(
            out_dir,
            &config.torii.sorafs_storage.data_dir,
            genesis,
            index,
        )?;
    }
    authorities.initialize_publication(out_dir, genesis, client)?;
    Ok(selection)
}

fn build_peers(count: u16, seed: Option<&[u8]>, base_api: u16, base_p2p: u16) -> Result<Vec<Peer>> {
    (0..count)
        .map(|nth| {
            let index = nth.to_be_bytes();
            let (public_key, private_key, pop) = generate_bls_key_pair(seed, &index)?;
            let (transport_public_key, transport_private_key) =
                ed25519_key_pair(seed, SORANET_TRANSPORT_SEED_DOMAIN, &index)?;
            let (streaming_public_key, streaming_private_key) =
                ed25519_key_pair(seed, STREAMING_IDENTITY_SEED_DOMAIN, &index)?;
            Ok(Peer {
                public_key,
                private_key,
                pop,
                transport_public_key,
                transport_private_key,
                streaming_public_key,
                streaming_private_key,
                api_port: base_api + nth,
                p2p_port: base_p2p + nth,
            })
        })
        .collect::<Result<Vec<_>>>()
        .wrap_err("failed to generate localnet peer keys")
}

/// A BLS key pair and its proof of possession, derived from `seed ‖ label` when seeded.
fn generate_bls_key_pair(
    seed: Option<&[u8]>,
    label: &[u8],
) -> Result<(PublicKey, ExposedPrivateKey, Vec<u8>)> {
    let key_pair = match seed {
        Some(seed) => KeyPair::try_from_seed(
            seed.iter().chain(label).copied().collect::<Vec<_>>(),
            iroha_crypto::Algorithm::BlsNormal,
        )?,
        None => KeyPair::try_random_with_algorithm(iroha_crypto::Algorithm::BlsNormal)?,
    };
    let pop = iroha_crypto::bls_normal_pop_prove(key_pair.private_key())?;
    let (public_key, private_key) = key_pair.into_parts();
    Ok((public_key, ExposedPrivateKey(private_key), pop))
}

fn ed25519_key_pair(
    seed: Option<&[u8]>,
    domain: &[u8],
    label: &[u8],
) -> Result<(PublicKey, ExposedPrivateKey)> {
    let key_pair = match seed {
        Some(seed) => KeyPair::try_from_seed(
            seed.iter()
                .chain(domain)
                .chain(label)
                .copied()
                .collect::<Vec<_>>(),
            iroha_crypto::Algorithm::Ed25519,
        )?,
        None => KeyPair::try_random_with_algorithm(iroha_crypto::Algorithm::Ed25519)?,
    };
    let (public_key, private_key) = key_pair.into_parts();
    Ok((public_key, ExposedPrivateKey(private_key)))
}

fn ephemeral_identity(seed: Option<&[u8]>, label: &[u8]) -> Result<Identity> {
    let (public_key, private_key) = ed25519_key_pair(seed, &[], label)?;
    Ok(Identity {
        account_id: AccountId::new(public_key.clone()),
        public_key,
        private_key: Zeroizing::new(private_key.to_string()),
    })
}

fn genesis_key_pair(seed: Option<&[u8]>) -> Result<(PublicKey, ExposedPrivateKey)> {
    ed25519_key_pair(seed, &[], GENESIS_SEED).wrap_err("failed to generate localnet genesis key")
}

fn validate_port_ranges(peers: NonZeroU16, base_api_port: u16, base_p2p_port: u16) -> Result<()> {
    ensure!(base_api_port > 0, "base_api_port must be > 0");
    ensure!(base_p2p_port > 0, "base_p2p_port must be > 0");
    let max_offset = u32::from(peers.get() - 1);
    let api = (
        u32::from(base_api_port),
        u32::from(base_api_port) + max_offset,
    );
    let p2p = (
        u32::from(base_p2p_port),
        u32::from(base_p2p_port) + max_offset,
    );
    ensure!(
        u16::try_from(api.1).is_ok(),
        "base_api_port {base_api_port} with {peers} peers exceeds u16 range"
    );
    ensure!(
        u16::try_from(p2p.1).is_ok(),
        "base_p2p_port {base_p2p_port} with {peers} peers exceeds u16 range"
    );
    ensure!(
        !(api.0 <= p2p.1 && p2p.0 <= api.1),
        "base_api_port {base_api_port} and base_p2p_port {base_p2p_port} overlap for {peers} peers"
    );
    Ok(())
}

fn validate_asset_specs(assets: &[AssetSpec]) -> Result<()> {
    let mut ids = BTreeSet::new();
    let mut aliases = BTreeSet::new();
    for (index, asset) in assets.iter().enumerate() {
        ensure!(
            !asset.name.trim().is_empty(),
            "localnet asset {} has an empty display name",
            index + 1
        );
        let id = AssetDefinitionId::parse_address_literal(&asset.id).wrap_err_with(|| {
            format!(
                "localnet asset {} has invalid asset definition id `{}`",
                index + 1,
                asset.id
            )
        })?;
        ensure!(
            ids.insert(id),
            "localnet asset definition id is duplicated: `{}`",
            asset.id
        );
        if let Some(alias) = asset.alias.as_deref() {
            let parsed = alias
                .parse::<iroha_data_model::asset::AssetDefinitionAlias>()
                .wrap_err_with(|| {
                    format!("localnet asset {} has invalid alias `{alias}`", index + 1)
                })?;
            ensure!(
                aliases.insert(parsed.to_string()),
                "localnet asset alias is duplicated: `{alias}`"
            );
        }
    }
    Ok(())
}

/// Asset specs with the placeholder client replaced by the generated client account.
fn assets_for_client(assets: &[AssetSpec], client: &AccountId) -> Vec<AssetSpec> {
    let placeholder = localnet_client_account_id();
    assets
        .iter()
        .map(|asset| {
            let mut asset = asset.clone();
            if asset.owned_by == placeholder {
                asset.owned_by = client.clone();
            }
            if asset.mint_to == placeholder {
                asset.mint_to = client.clone();
            }
            asset
        })
        .collect()
}

/// The XOR fee asset every localnet registers.
fn localnet_xor_asset_definition_id() -> AssetDefinitionId {
    AssetDefinitionId::parse_address_literal(TAIRA_XOR_ASSET_DEFINITION_ID)
        .expect("canonical XOR asset definition")
}

/// Render `account` in the chain's address prefix, or in the default rendering.
fn account_id_runtime_literal(account: &AccountId, chain_discriminant: Option<u16>) -> String {
    chain_discriminant.map_or_else(
        || account.to_string(),
        |discriminant| config::account_literal(account, discriminant),
    )
}

/// Write a signer key file: the multihash private key and a newline, owner-only.
fn write_private_key_sidecar(path: &Path, private_key: &str) -> Result<()> {
    let mut contents = Zeroizing::new(Vec::with_capacity(private_key.len() + 1));
    contents.extend_from_slice(private_key.as_bytes());
    contents.push(b'\n');
    custody::write_private_file_atomic(path, &contents)
        .wrap_err_with(|| format!("write private signer key {}", path.display()))
}

fn write_owner_only(path: &Path, contents: &[u8]) -> Result<()> {
    custody::write_private_file_atomic(path, contents)
        .wrap_err_with(|| format!("write owner-only localnet file {}", path.display()))
}
