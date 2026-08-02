use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{BufWriter, Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};

use clap::Args as ClapArgs;
use color_eyre::eyre::{WrapErr as _, ensure, eyre};
use iroha_config::{base::toml::TomlSource, parameters::actual};
use iroha_crypto::{Hash, HashOf, PublicKey};
use iroha_data_model::{
    ChainId,
    account::address::ChainDiscriminantGuard,
    block::{
        BlockHeader, SignedBlock,
        consensus_v2::{
            ConsensusMode as WireConsensusMode, MAX_VALIDATORS_PER_HEIGHT, is_valid_committee_size,
        },
        decode_framed_signed_block,
    },
    isi::{SetParameter, register::RegisterBox},
    parameter::{
        Parameter,
        system::{ConsensusHandshakeMetadata, SumeragiConsensusMode, consensus_metadata},
    },
    prelude::AccountId,
    transaction::Executable,
};
use iroha_genesis::RawGenesisTransaction;
use iroha_swarm::{
    PeerOverride, PreparedBuildLine, PreparedGenesisArtifacts, PreparedRuntimeFile,
    PreparedSecretFile, PreparedValidator,
};
use iroha_version::BuildLine;

use crate::{
    Outcome, RunArgs,
    genesis::{
        ConsensusPolicy, build_line_from_env, ensure_npos_parameters,
        validate_consensus_mode_for_line,
    },
    tui,
};

/// Docker Compose configuration generator for Iroha.
#[allow(clippy::struct_excessive_bools)]
#[derive(ClapArgs, Debug, Clone)]
pub struct Args {
    /// Number of peer services in the configuration.
    ///
    /// Must be an exact Sumeragi v2 `3f + 1` committee in the range 4..=31.
    #[arg(long, short, value_name = "COUNT")]
    peers: std::num::NonZeroU16,
    /// Enable deterministic development mode with this UTF-8 validator seed.
    ///
    /// When omitted, `--config-dir` must be an authoritative prepared bundle containing
    /// `peerN.toml`, signed genesis, verifier-key, and exact-hash files. Production workflows
    /// should omit this option so Compose cannot generate identities that diverge from genesis.
    #[arg(long, short)]
    seed: Option<String>,
    /// Includes a healthcheck for every service in the configuration.
    ///
    /// Healthchecks use predefined settings.
    ///
    /// For more details on healthcheck configuration in Docker Compose files, see:
    /// <https://docs.docker.com/compose/compose-file/compose-file-v3/#healthcheck>
    #[arg(long, short = 'H')]
    healthcheck: bool,
    /// Authoritative prepared validator/genesis bundle, or development manifest directory.
    ///
    /// Normal mode requires `genesis.json`, `peer0.toml` through `peerN.toml`,
    /// `genesis.signed.nrt`, `genesis.public_key`, and `genesis.expected_hash`. Kagami validates
    /// their canonical wire, signer, semantic manifest binding, exact hash, validator roster,
    /// and PoPs together. With `--seed`, only `genesis.json` is read and runtime artifact paths
    /// are supplied explicitly through the generated manifest's `IROHA_GENESIS_*_FILE`
    /// variables.
    #[arg(long, short, value_name = "DIR")]
    config_dir: PathBuf,
    /// Optional TOML file describing peer names and port mappings.
    /// Only available with deterministic development `--seed` mode.
    ///
    /// The file must contain an array named `peers`, for example:
    ///
    /// ```toml
    /// [[peers]]
    /// name = "alpha"
    /// p2p_port = 2000
    /// api_port = 9000
    /// [[peers]]
    /// name = "beta"
    /// p2p_port = 2001
    /// api_port = 9001
    /// ```
    #[arg(long, value_name = "FILE", requires = "seed")]
    peer_config: Option<PathBuf>,
    /// Docker image used by the peer services.
    ///
    /// By default, the image is pulled from Docker Hub if not cached.
    /// Pass the `--build` option to build the image from a Dockerfile instead.
    ///
    /// **Note**: Swarm only guarantees that the Docker Compose configuration it generates
    /// is compatible with the same Git revision it is built from itself. Therefore, if the
    /// specified image is not compatible with the version of Swarm you are running,
    /// the generated configuration might not work.
    #[arg(long, short, value_name = "NAME")]
    image: String,
    /// Build the image from the Dockerfile in the specified directory.
    /// Do not rebuild if the image has been cached.
    ///
    /// The provided path is resolved relative to the current working directory.
    #[arg(long, short, value_name = "DIR")]
    build: Option<PathBuf>,
    /// Always pull or rebuild the image even if it is cached locally.
    #[arg(long)]
    no_cache: bool,
    /// Path to the target Compose configuration file.
    ///
    /// If the file exists, the app will prompt its overwriting. If the TTY is not
    /// interactive, the app will stop execution with a non-zero exit code.
    /// To overwrite the file anyway, pass the `--force` flag.
    #[arg(long, short, value_name = "FILE")]
    out_file: PathBuf,
    /// Print the generated configuration to stdout
    /// instead of writing it to the target file.
    ///
    /// Note that the target path still needs to be provided, as it is used to resolve paths.
    #[arg(long, short = 'P', conflicts_with = "force")]
    print: bool,
    /// Overwrite the target file if it already exists.
    #[arg(long, short = 'F')]
    force: bool,
    /// Do not include the banner with the generation notice in the file.
    ///
    /// The banner includes the seed to help with reproducibility.
    #[arg(long)]
    no_banner: bool,
}

impl Args {
    /// If this returns `Ok(true)`, then Swarm is allowed to proceed.
    fn user_allows_overwrite(&self) -> Result<bool, inquire::InquireError> {
        if self.out_file.exists() && !self.force {
            use owo_colors::OwoColorize;
            return inquire::Confirm::new(&format!(
                "File {} already exists. Overwrite it?",
                self.out_file.display().blue().bold()
            ))
            .with_help_message("Pass the `--force` flag to overwrite the file anyway.")
            .with_default(false)
            .prompt();
        }
        Ok(true)
    }
}

#[derive(Debug)]
struct PreparedBundle {
    chain: ChainId,
    validators: Vec<PreparedValidator>,
    signed_block: PathBuf,
    public_key: PathBuf,
    expected_hash: PathBuf,
}

struct ValidatedGenesis {
    block: iroha_data_model::block::SignedBlock,
    canonical_wire: Vec<u8>,
    public_key: PublicKey,
    expected_hash: HashOf<BlockHeader>,
    validator_pops: BTreeMap<PublicKey, Vec<u8>>,
}

struct AdmittedPreparedValidator {
    config: actual::Root,
    table: toml::Table,
    key_pair: iroha_crypto::KeyPair,
    pop: Vec<u8>,
}

struct PreparedRuntimePeer {
    service_name: String,
    p2p_port: u16,
    api_port: u16,
    public_key: PublicKey,
}

fn read_exact_record(path: &Path, label: &str) -> color_eyre::Result<String> {
    const MAX_EXACT_RECORD_BYTES: u64 = 64 * 1024;
    let record = read_runtime_file_bounded(path, label, MAX_EXACT_RECORD_BYTES)?;
    let record = String::from_utf8(record)
        .wrap_err_with(|| format!("read UTF-8 {label} record {}", path.display()))?;
    let payload = record.strip_suffix('\n').ok_or_else(|| {
        eyre!(
            "{label} record {} must end in exactly one newline",
            path.display()
        )
    })?;
    ensure!(
        !payload.is_empty()
            && payload.trim() == payload
            && !payload.chars().any(char::is_whitespace),
        "{label} record {} must contain exactly one non-empty canonical line",
        path.display()
    );
    Ok(payload.to_owned())
}

fn validate_prepared_genesis(
    signed_block: &Path,
    public_key_path: &Path,
    expected_hash_path: &Path,
) -> color_eyre::Result<ValidatedGenesis> {
    let public_record = read_exact_record(public_key_path, "genesis public-key")?;
    let public_key = public_record
        .parse::<PublicKey>()
        .wrap_err("parse prepared genesis public key")?;
    ensure!(
        public_key.to_string() == public_record,
        "prepared genesis public-key record is not canonical"
    );

    let expected_record = read_exact_record(expected_hash_path, "genesis expected-hash")?;
    let expected_hash = expected_record
        .parse::<HashOf<BlockHeader>>()
        .wrap_err("parse prepared exact genesis hash")?;
    ensure!(
        expected_hash.to_string() == expected_record,
        "prepared genesis expected-hash record is not canonical lowercase marked hex"
    );

    const MAX_SIGNED_GENESIS_BYTES: u64 = 512 * 1024 * 1024;
    let signed = read_runtime_file_bounded(
        signed_block,
        "signed genesis body",
        MAX_SIGNED_GENESIS_BYTES,
    )?;
    ensure!(
        !signed.is_empty(),
        "signed genesis body {} is empty",
        signed_block.display()
    );
    iroha_genesis::init_instruction_registry();
    let block = decode_framed_signed_block(&signed)
        .wrap_err("decode prepared canonical signed genesis body")?;
    let canonical = block
        .encode_wire()
        .wrap_err("re-encode prepared signed genesis body")?;
    ensure!(
        canonical == signed,
        "prepared signed genesis body is not canonical framed Norito"
    );
    ensure!(
        block.hash() == expected_hash,
        "prepared signed genesis body hashes to {}, expected {}",
        block.hash(),
        expected_hash
    );

    let first = block
        .external_transactions()
        .next()
        .ok_or_else(|| eyre!("prepared signed genesis contains no external transactions"))?;
    let embedded_signer = first.authority().try_signatory().ok_or_else(|| {
        eyre!("prepared genesis authority must be one canonical single-key account")
    })?;
    ensure!(
        embedded_signer == &public_key,
        "prepared genesis signer {embedded_signer} differs from verifier key {public_key}"
    );

    {
        let mut signatures = block.signatures();
        let signature = signatures
            .next()
            .ok_or_else(|| eyre!("prepared signed genesis has no block signature"))?;
        ensure!(
            signature.index() == 0 && signatures.next().is_none(),
            "prepared signed genesis must have exactly one block signature at index 0"
        );
        signature
            .signature()
            .verify_hash(&public_key, block.hash())
            .wrap_err("verify prepared genesis block signature")?;
    }
    for transaction in block.external_transactions() {
        transaction
            .verify_signature()
            .wrap_err("verify prepared genesis transaction signature")?;
    }

    let mut validator_pops = BTreeMap::new();
    for transaction in block.external_transactions() {
        let Executable::Instructions(instructions) = transaction.instructions() else {
            continue;
        };
        for instruction in instructions {
            let Some(RegisterBox::Peer(register)) =
                instruction.as_any().downcast_ref::<RegisterBox>()
            else {
                continue;
            };
            let public_key = register.peer.public_key().clone();
            ensure!(
                validator_pops
                    .insert(public_key.clone(), register.pop.clone())
                    .is_none(),
                "prepared genesis registers validator {public_key} more than once"
            );
        }
    }
    ensure!(
        !validator_pops.is_empty(),
        "prepared genesis contains no RegisterPeerWithPop validator roster"
    );

    Ok(ValidatedGenesis {
        block,
        canonical_wire: canonical,
        public_key,
        expected_hash,
        validator_pops,
    })
}

fn signed_genesis_consensus_metadata(
    block: &SignedBlock,
) -> color_eyre::Result<ConsensusHandshakeMetadata> {
    let mut metadata = None;
    for transaction in block.external_transactions() {
        let Executable::Instructions(instructions) = transaction.instructions() else {
            continue;
        };
        for instruction in instructions {
            let Some(set_parameter) = instruction.as_any().downcast_ref::<SetParameter>() else {
                continue;
            };
            let Parameter::Custom(custom) = set_parameter.inner() else {
                continue;
            };
            if custom.id() != &consensus_metadata::handshake_meta_id() {
                continue;
            }
            let decoded = custom
                .payload()
                .try_into_any::<ConsensusHandshakeMetadata>()
                .wrap_err("decode prepared signed genesis consensus metadata")?;
            ensure!(
                metadata.replace(decoded).is_none(),
                "prepared signed genesis contains more than one consensus metadata instruction"
            );
        }
    }
    metadata
        .ok_or_else(|| eyre!("prepared signed genesis contains no consensus metadata instruction"))
}

fn validate_prepared_manifest_binding(
    manifest: &RawGenesisTransaction,
    block: &SignedBlock,
    public_key: &PublicKey,
) -> color_eyre::Result<()> {
    let signed_metadata = signed_genesis_consensus_metadata(block)?;
    ensure!(
        signed_metadata.mode == manifest.consensus_mode(),
        "prepared genesis manifest consensus mode {} differs from signed body mode {}",
        manifest.consensus_mode(),
        signed_metadata.mode
    );
    ensure!(
        manifest.wire_protocol_version() == signed_metadata.wire_protocol_version,
        "prepared genesis manifest wire protocol version {} differs from signed body version {}",
        manifest.wire_protocol_version(),
        signed_metadata.wire_protocol_version
    );
    ensure!(
        manifest.consensus_fingerprint() == Some(signed_metadata.consensus_fingerprint),
        "prepared genesis manifest consensus fingerprint differs from signed body"
    );
    ensure!(
        manifest.sumeragi_v2_context_parameters() == signed_metadata.sumeragi_v2,
        "prepared genesis manifest Sumeragi v2 context differs from signed body"
    );

    let expected = manifest
        .clone()
        .with_consensus_meta()
        .parse()
        .wrap_err("expand prepared genesis manifest instructions")?;
    let actual = block.external_transactions().collect::<Vec<_>>();
    ensure!(
        expected.len() == actual.len(),
        "prepared signed genesis transaction count differs from genesis manifest"
    );
    let genesis_account = AccountId::new(public_key.clone());
    for (index, (expected_batch, transaction)) in expected.iter().zip(&actual).enumerate() {
        ensure!(
            transaction.chain() == manifest.chain_id()
                && transaction.authority() == &genesis_account,
            "prepared signed genesis transaction {index} has the wrong chain or root authority"
        );
        let Executable::Instructions(actual_batch) = transaction.instructions() else {
            return Err(eyre!(
                "prepared signed genesis transaction {index} is not an instruction batch"
            ));
        };
        let expected_semantic = expected_batch
            .iter()
            .map(iroha_data_model::Encode::encode)
            .collect::<Vec<_>>();
        let actual_semantic = actual_batch
            .iter()
            .map(iroha_data_model::Encode::encode)
            .collect::<Vec<_>>();
        ensure!(
            expected_semantic == actual_semantic,
            "prepared signed genesis transaction {index} differs from genesis manifest"
        );
    }
    Ok(())
}

fn prepared_peer_config_paths(
    config_dir: &Path,
    count: std::num::NonZeroU16,
) -> color_eyre::Result<Vec<PathBuf>> {
    let mut discovered = BTreeSet::new();
    for entry in fs::read_dir(config_dir)
        .wrap_err_with(|| format!("read prepared bundle directory {}", config_dir.display()))?
    {
        let entry = entry.wrap_err("read prepared bundle directory entry")?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let Some(index) = name
            .strip_prefix("peer")
            .and_then(|suffix| suffix.strip_suffix(".toml"))
        else {
            continue;
        };
        if index.is_empty() || !index.bytes().all(|byte| byte.is_ascii_digit()) {
            continue;
        }
        let index = index
            .parse::<u16>()
            .wrap_err_with(|| format!("prepared peer filename `{name}` has an invalid index"))?;
        ensure!(
            name == format!("peer{index}.toml"),
            "prepared peer filename `{name}` is not canonical"
        );
        ensure!(
            discovered.insert(index),
            "prepared bundle contains duplicate peer index {index}"
        );
    }
    let expected = (0..count.get()).collect::<BTreeSet<_>>();
    ensure!(
        discovered == expected,
        "prepared validator roster files are {:?}, expected {:?}",
        discovered,
        expected
    );
    Ok(expected
        .into_iter()
        .map(|index| config_dir.join(format!("peer{index}.toml")))
        .collect())
}

struct ParsedPreparedPeerConfig {
    actual: actual::Root,
    table: toml::Table,
}

fn parse_prepared_peer_config(path: &Path) -> color_eyre::Result<ParsedPreparedPeerConfig> {
    const MAX_PREPARED_CONFIG_BYTES: u64 = 8 * 1024 * 1024;
    let raw = read_runtime_file_bounded(path, "validator config", MAX_PREPARED_CONFIG_BYTES)?;
    let raw = String::from_utf8(raw)
        .wrap_err_with(|| format!("read UTF-8 prepared validator config {}", path.display()))?;
    let table = raw
        .parse::<toml::Table>()
        .wrap_err_with(|| format!("parse prepared validator config {}", path.display()))?;
    ensure!(
        !table.contains_key("extends"),
        "prepared validator config {} must be flattened and cannot use `extends`",
        path.display()
    );
    let chain_discriminant = table
        .get("chain_discriminant")
        .and_then(toml::Value::as_integer)
        .and_then(|value| u16::try_from(value).ok());
    let _chain_discriminant = chain_discriminant.map(ChainDiscriminantGuard::enter);
    // Keep parsing and projection bound to the exact bytes read above. Reopening
    // the path here would leave a swap interval between the admitted table and
    // the table from which validator identity/policy is derived.
    let source = TomlSource::new(path.to_path_buf(), table.clone());
    let actual = actual::Root::from_toml_source(source).map_err(|error| {
        eyre!(
            "prepared validator config {} is invalid: {error:?}",
            path.display()
        )
    })?;
    Ok(ParsedPreparedPeerConfig { actual, table })
}

fn ensure_toml_table<'a>(
    root: &'a mut toml::Table,
    path: &[&str],
) -> color_eyre::Result<&'a mut toml::Table> {
    let mut current = root;
    for segment in path {
        let value = current
            .entry((*segment).to_owned())
            .or_insert_with(|| toml::Value::Table(toml::Table::new()));
        current = value.as_table_mut().ok_or_else(|| {
            eyre!(
                "prepared runtime projection expected `{}` to be a TOML table",
                path.join(".")
            )
        })?;
    }
    Ok(current)
}

fn set_toml_string(
    root: &mut toml::Table,
    table_path: &[&str],
    key: &str,
    value: impl Into<String>,
) -> color_eyre::Result<()> {
    ensure_toml_table(root, table_path)?.insert(key.to_owned(), toml::Value::String(value.into()));
    Ok(())
}

fn remove_toml_key(
    root: &mut toml::Table,
    table_path: &[&str],
    key: &str,
) -> color_eyre::Result<()> {
    if table_path.is_empty() {
        root.remove(key);
        return Ok(());
    }
    let mut current = root;
    for segment in table_path {
        let Some(value) = current.get_mut(*segment) else {
            return Ok(());
        };
        let Some(table) = value.as_table_mut() else {
            return Err(eyre!(
                "prepared runtime projection expected `{}` to be a TOML table",
                table_path.join(".")
            ));
        };
        current = table;
    }
    current.remove(key);
    Ok(())
}

fn toml_contains(root: &toml::Table, table_path: &[&str], key: &str) -> bool {
    let mut current = root;
    for segment in table_path {
        let Some(table) = current.get(*segment).and_then(toml::Value::as_table) else {
            return false;
        };
        current = table;
    }
    current.contains_key(key)
}

fn config_requires_sora_profile(config: &actual::Root) -> bool {
    config.torii.sorafs_storage.enabled
        || config.torii.sorafs_discovery.discovery_enabled
        || config.torii.sorafs_repair.enabled
        || config.torii.sorafs_gc.enabled
        || config.nexus.uses_multilane_catalogs()
        || config.nexus.has_lane_overrides()
}

fn effective_runtime_config(mut config: actual::Root, table: &toml::Table) -> (actual::Root, bool) {
    let requires_sora_profile = config_requires_sora_profile(&config);
    if requires_sora_profile {
        // Match irohad's explicit-value preservation around `--sora`.
        let storage_explicit = toml_contains(table, &["sorafs", "storage"], "enabled");
        let discovery_explicit =
            toml_contains(table, &["sorafs", "discovery"], "discovery_enabled");
        let storage_enabled = config.torii.sorafs_storage.enabled;
        let discovery_enabled = config.torii.sorafs_discovery.discovery_enabled;
        config.apply_sora_profile();
        if storage_explicit {
            config.torii.sorafs_storage.enabled = storage_enabled;
        }
        if discovery_explicit {
            config.torii.sorafs_discovery.discovery_enabled = discovery_enabled;
        }
    }
    (config, requires_sora_profile)
}

fn compose_addr_literal(host: &str, port: u16) -> String {
    norito::literal::format("addr", &format!("{host}:{port}"))
}

fn rewrite_container_network(
    table: &mut toml::Table,
    index: usize,
    peers: &[PreparedRuntimePeer],
) -> color_eyre::Result<()> {
    let myself = peers
        .get(index)
        .ok_or_else(|| eyre!("prepared runtime peer index {index} is out of bounds"))?;
    set_toml_string(
        table,
        &["network"],
        "address",
        compose_addr_literal("0.0.0.0", myself.p2p_port),
    )?;
    set_toml_string(
        table,
        &["network"],
        "public_address",
        compose_addr_literal(&myself.service_name, myself.p2p_port),
    )?;
    set_toml_string(
        table,
        &["torii"],
        "address",
        compose_addr_literal("0.0.0.0", myself.api_port),
    )?;
    table.insert(
        "trusted_peers".to_owned(),
        toml::Value::Array(
            peers
                .iter()
                .map(|peer| {
                    toml::Value::String(format!(
                        "{}@{}",
                        peer.public_key,
                        compose_addr_literal(&peer.service_name, peer.p2p_port)
                    ))
                })
                .collect(),
        ),
    );
    ensure_toml_table(table, &["torii"])?.insert(
        "peer_telemetry_urls".to_owned(),
        toml::Value::Array(
            peers
                .iter()
                .map(|peer| {
                    toml::Value::String(format!("http://{}:{}/", peer.service_name, peer.api_port))
                })
                .collect(),
        ),
    );
    Ok(())
}

fn validate_prepared_network_projection(
    config: &actual::Root,
    path: &Path,
) -> color_eyre::Result<()> {
    let loopback_only = |cidrs: &[String]| {
        cidrs
            .iter()
            .all(|cidr| matches!(cidr.as_str(), "127.0.0.0/8" | "127.0.0.1/32" | "::1/128"))
    };
    let trusted = config.common.trusted_peers.value();
    let committee_size = trusted.others.len().saturating_add(1);
    let required_peer_connections = committee_size.saturating_sub(1);
    ensure!(
        matches!(config.network.relay_mode, actual::RelayMode::Disabled)
            && config.network.relay_hub_addresses.is_empty(),
        "prepared validator config {} uses relay topology that cannot be projected into the exact Compose committee",
        path.display()
    );
    ensure!(
        !config.network.scion.enabled
            && config.network.scion.listen_endpoint.is_none()
            && config.network.scion.routes.is_empty(),
        "prepared validator config {} uses SCION routes that cannot be projected into Compose service DNS",
        path.display()
    );
    ensure!(
        !config.network.tls_enabled
            && config.network.tls_listen_address.is_none()
            && !config.network.tls_inbound_only,
        "prepared validator config {} uses P2P TLS listeners that require an explicit container certificate/address projection",
        path.display()
    );
    ensure!(
        config.network.p2p_proxy.is_none() && !config.network.p2p_proxy_required,
        "prepared validator config {} uses a P2P proxy that would bypass the exact Compose committee topology",
        path.display()
    );
    ensure!(
        config.network.debug_packet_loss_inbound_percent == 0
            && config.network.debug_packet_loss_outbound_percent == 0,
        "prepared validator config {} enables debug packet loss, which can deterministically prevent committee progress",
        path.display()
    );
    ensure!(
        config.network.allow_cidrs.is_empty() && config.network.deny_cidrs.is_empty(),
        "prepared validator config {} uses P2P CIDR ACLs whose meaning changes on a Compose bridge",
        path.display()
    );
    let committee_keys = std::iter::once(&trusted.myself)
        .chain(trusted.others.iter())
        .map(|peer| peer.id().public_key())
        .collect::<BTreeSet<_>>();
    if config.network.allowlist_only {
        let allowed = config.network.allow_keys.iter().collect::<BTreeSet<_>>();
        ensure!(
            committee_keys.is_subset(&allowed),
            "prepared validator config {} P2P allowlist excludes a signed-genesis committee member",
            path.display()
        );
    }
    let denied = config.network.deny_keys.iter().collect::<BTreeSet<_>>();
    ensure!(
        committee_keys.is_disjoint(&denied),
        "prepared validator config {} P2P denylist contains a signed-genesis committee member",
        path.display()
    );
    if let Some(max_incoming) = config.network.max_incoming {
        ensure!(
            max_incoming.get() >= required_peer_connections,
            "prepared validator config {} max_incoming={} cannot admit the other {} committee members",
            path.display(),
            max_incoming,
            required_peer_connections
        );
    }
    if let Some(max_total) = config.network.max_total_connections {
        ensure!(
            max_total.get() >= required_peer_connections,
            "prepared validator config {} max_total_connections={} cannot connect to the other {} committee members",
            path.display(),
            max_total,
            required_peer_connections
        );
    }
    ensure!(
        !config.network.soranet_vpn.enabled,
        "prepared validator config {} enables SoraVPN, but generated Compose services have no tunnel device or helper topology",
        path.display()
    );
    ensure!(
        loopback_only(&config.torii.api_rate_limit_bypass_cidrs)
            && loopback_only(&config.torii.internal_api_trusted_cidrs)
            && loopback_only(&config.torii.preauth_allow_cidrs)
            && loopback_only(&config.torii.operator_auth.mtls_trusted_proxy_cidrs)
            && loopback_only(&config.torii.soranet_privacy_ingest.allow_cidrs)
            && loopback_only(&config.torii.transport.trusted_proxy_cidrs)
            && loopback_only(&config.torii.transport.norito_rpc.mtls_trusted_proxy_cidrs),
        "prepared validator config {} uses non-loopback source CIDRs or trusted-proxy CIDRs whose trust semantics change behind Compose NAT",
        path.display()
    );
    ensure!(
        !config.torii.push.enabled,
        "prepared validator config {} enables push delivery whose private provider keys and egress are not projected",
        path.display()
    );
    ensure!(
        !config.torii.zk_prover_enabled,
        "prepared validator config {} enables the background ZK prover without an immutable key-directory projection",
        path.display()
    );
    ensure!(
        !config.torii.sorafs_storage.governance_dag_service.enabled,
        "prepared validator config {} enables the Governance DAG service without exposing its independent listener",
        path.display()
    );
    Ok(())
}

#[cfg(unix)]
fn same_file_snapshot(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;

    left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.mode() == right.mode()
        && left.uid() == right.uid()
        && left.gid() == right.gid()
        && left.nlink() == right.nlink()
        && left.size() == right.size()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
}

#[cfg(not(unix))]
fn same_file_snapshot(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.is_file() == right.is_file()
        && left.is_dir() == right.is_dir()
        && left.len() == right.len()
        && left.modified().ok() == right.modified().ok()
}

fn read_runtime_file_bounded(
    path: &Path,
    label: &str,
    max_bytes: u64,
) -> color_eyre::Result<Vec<u8>> {
    let lexical = fs::symlink_metadata(path)
        .wrap_err_with(|| format!("inspect prepared {label} {}", path.display()))?;
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let mut file = options
        .open(path)
        .wrap_err_with(|| format!("open prepared {label} {}", path.display()))?;
    let before = file
        .metadata()
        .wrap_err_with(|| format!("inspect opened prepared {label} {}", path.display()))?;
    ensure!(
        !lexical.file_type().is_symlink()
            && before.is_file()
            && same_file_snapshot(&lexical, &before),
        "prepared {label} {} changed while opening or is not a regular file",
        path.display()
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        ensure!(
            before.uid() == rustix::process::geteuid().as_raw()
                && before.nlink() == 1
                && before.mode() & 0o022 == 0,
            "prepared {label} {} must be owner-held, single-link, and not group/world writable",
            path.display()
        );
    }
    ensure!(
        before.len() <= max_bytes,
        "prepared {label} {} is {} bytes, exceeding the {}-byte Compose projection cap",
        path.display(),
        before.len(),
        max_bytes
    );
    let mut raw = Vec::with_capacity(usize::try_from(before.len()).unwrap_or(0));
    Read::by_ref(&mut file)
        .take(max_bytes.saturating_add(1))
        .read_to_end(&mut raw)
        .wrap_err_with(|| format!("read prepared {label} {}", path.display()))?;
    ensure!(
        u64::try_from(raw.len()).unwrap_or(u64::MAX) <= max_bytes,
        "prepared {label} {} grew beyond the {max_bytes}-byte Compose projection cap",
        path.display()
    );
    let after = file
        .metadata()
        .wrap_err_with(|| format!("reinspect prepared {label} {}", path.display()))?;
    ensure!(
        same_file_snapshot(&before, &after) && u64::try_from(raw.len()).ok() == Some(before.len()),
        "prepared {label} {} changed while being read",
        path.display()
    );
    Ok(raw)
}

fn collect_runtime_directory(
    source: &Path,
    projection_root: &Path,
    namespace: &str,
    target_root: &str,
    label: &str,
) -> color_eyre::Result<(Vec<PreparedRuntimeFile>, PathBuf)> {
    const MAX_FILES: usize = 128;
    const MAX_ENTRIES: usize = 256;
    const MAX_DEPTH: usize = 8;
    const MAX_TOTAL_BYTES: u64 = 16 * 1024 * 1024;

    fn collect_entries(
        directory: &Path,
        relative_prefix: &str,
        depth: usize,
        label: &str,
        captured: &mut Vec<(String, Vec<u8>)>,
        entries_seen: &mut usize,
        total: &mut u64,
    ) -> color_eyre::Result<()> {
        ensure!(
            depth <= MAX_DEPTH,
            "prepared {label} directory {} exceeds the maximum nesting depth {MAX_DEPTH}",
            directory.display()
        );
        let before = fs::symlink_metadata(directory).wrap_err_with(|| {
            format!(
                "inspect prepared {label} directory {} before traversal",
                directory.display()
            )
        })?;
        ensure!(
            before.is_dir() && !before.file_type().is_symlink(),
            "prepared {label} directory {} is not a regular directory",
            directory.display()
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt as _;
            ensure!(
                before.uid() == rustix::process::geteuid().as_raw() && before.mode() & 0o022 == 0,
                "prepared {label} directory {} must be owner-held and not group/world writable",
                directory.display()
            );
        }
        let mut entries = fs::read_dir(directory)
            .wrap_err_with(|| format!("read prepared {label} directory {}", directory.display()))?
            .collect::<Result<Vec<_>, _>>()
            .wrap_err_with(|| {
                format!(
                    "enumerate prepared {label} directory {}",
                    directory.display()
                )
            })?;
        entries.sort_by_key(std::fs::DirEntry::file_name);
        ensure!(
            !entries.is_empty(),
            "prepared {label} directory {} is empty; empty runtime trees cannot be represented byte-exactly",
            directory.display()
        );
        for entry in entries {
            *entries_seen = entries_seen
                .checked_add(1)
                .ok_or_else(|| eyre!("prepared {label} entry count overflow"))?;
            ensure!(
                *entries_seen <= MAX_ENTRIES,
                "prepared {label} directory tree exceeds the {MAX_ENTRIES}-entry cap"
            );
            let file_type = entry
                .file_type()
                .wrap_err_with(|| format!("inspect prepared {label} entry"))?;
            ensure!(
                !file_type.is_symlink(),
                "prepared {label} entry {} is a symbolic link",
                entry.path().display()
            );
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| eyre!("prepared {label} filename is not UTF-8"))?;
            ensure!(
                !name.is_empty()
                    && name.bytes().all(|byte| {
                        byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-')
                    }),
                "prepared {label} filename `{name}` is not portable"
            );
            let relative = if relative_prefix.is_empty() {
                name
            } else {
                format!("{relative_prefix}/{name}")
            };
            if file_type.is_dir() {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::MetadataExt as _;
                    let metadata = fs::symlink_metadata(entry.path())
                        .wrap_err_with(|| format!("inspect prepared {label} directory entry"))?;
                    ensure!(
                        metadata.uid() == rustix::process::geteuid().as_raw(),
                        "prepared {label} directory {} must be owner-held",
                        entry.path().display()
                    );
                }
                collect_entries(
                    &entry.path(),
                    &relative,
                    depth + 1,
                    label,
                    captured,
                    entries_seen,
                    total,
                )?;
                continue;
            }
            ensure!(
                file_type.is_file(),
                "prepared {label} entry {} is not a regular file or directory",
                entry.path().display()
            );
            ensure!(
                captured.len() < MAX_FILES,
                "prepared {label} directory tree exceeds the {MAX_FILES}-file cap"
            );
            let content = read_runtime_file_bounded(&entry.path(), label, MAX_TOTAL_BYTES)?;
            *total = total
                .checked_add(u64::try_from(content.len()).unwrap_or(u64::MAX))
                .ok_or_else(|| eyre!("prepared {label} aggregate size overflow"))?;
            ensure!(
                *total <= MAX_TOTAL_BYTES,
                "prepared {label} files exceed the {MAX_TOTAL_BYTES}-byte aggregate cap"
            );
            captured.push((relative, content));
        }
        let after = fs::symlink_metadata(directory).wrap_err_with(|| {
            format!(
                "reinspect prepared {label} directory {} after traversal",
                directory.display()
            )
        })?;
        ensure!(
            same_file_snapshot(&before, &after),
            "prepared {label} directory {} changed while being captured",
            directory.display()
        );
        Ok(())
    }

    let lexical_source = fs::symlink_metadata(source).wrap_err_with(|| {
        format!(
            "inspect configured prepared {label} directory {}",
            source.display()
        )
    })?;
    ensure!(
        lexical_source.is_dir() && !lexical_source.file_type().is_symlink(),
        "configured prepared {label} directory {} must not be a symbolic link",
        source.display()
    );
    let source = fs::canonicalize(source).wrap_err_with(|| {
        format!(
            "canonicalize prepared {label} directory {}",
            source.display()
        )
    })?;
    let source_metadata = fs::symlink_metadata(&source)
        .wrap_err_with(|| format!("inspect prepared {label} directory {}", source.display()))?;
    ensure!(
        source_metadata.is_dir() && !source_metadata.file_type().is_symlink(),
        "prepared {label} directory {} is not a regular directory",
        source.display()
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        ensure!(
            source_metadata.uid() == rustix::process::geteuid().as_raw()
                && source_metadata.mode() & 0o022 == 0,
            "prepared {label} directory {} must be owner-held and not group/world writable",
            source.display()
        );
    }
    let mut total = 0_u64;
    let mut entries_seen = 0;
    let mut captured = Vec::new();
    collect_entries(
        &source,
        "",
        0,
        label,
        &mut captured,
        &mut entries_seen,
        &mut total,
    )?;
    let mut digest = blake3::Hasher::new();
    digest.update(b"iroha-prepared-runtime-directory-v1");
    for (name, content) in &captured {
        digest.update(
            &u64::try_from(name.len())
                .expect("captured runtime filename length fits u64")
                .to_le_bytes(),
        );
        digest.update(name.as_bytes());
        digest.update(
            &u64::try_from(content.len())
                .expect("captured runtime file length fits u64")
                .to_le_bytes(),
        );
        digest.update(content);
    }
    let validation_dir =
        projection_root.join(format!("{namespace}-{}", digest.finalize().to_hex()));
    ensure_container_projection_directory(projection_root)?;
    ensure_container_projection_directory(&validation_dir)?;
    let expected_files = captured
        .iter()
        .map(|(relative, _)| relative.clone())
        .collect::<BTreeSet<_>>();
    let mut expected_directories = BTreeSet::new();
    for relative in &expected_files {
        let mut parent = Path::new(relative).parent();
        while let Some(path) = parent {
            if path.as_os_str().is_empty() {
                break;
            }
            expected_directories.insert(path.to_string_lossy().replace('\\', "/"));
            parent = path.parent();
        }
    }
    let mut files = Vec::with_capacity(captured.len());
    for (relative, content) in captured {
        let relative_path = Path::new(&relative);
        let mut validation_parent = validation_dir.clone();
        if let Some(parent) = relative_path.parent() {
            for component in parent.components() {
                let std::path::Component::Normal(segment) = component else {
                    return Err(eyre!(
                        "prepared {label} relative path `{relative}` is not normalized"
                    ));
                };
                validation_parent.push(segment);
                ensure_container_projection_directory(&validation_parent)?;
            }
        }
        let name = relative_path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| eyre!("prepared {label} relative filename is not UTF-8"))?;
        materialize_read_only_file_at(&validation_parent, name, &content)?;
        files.push(PreparedRuntimeFile {
            target: format!("{target_root}/{relative}"),
            content,
        });
    }
    fn collect_projection_entries(
        root: &Path,
        directory: &Path,
        relative_prefix: &str,
        files: &mut BTreeSet<String>,
        directories: &mut BTreeSet<String>,
    ) -> color_eyre::Result<()> {
        let entries = fs::read_dir(directory)
            .wrap_err_with(|| format!("verify prepared runtime projection {}", root.display()))?;
        for entry in entries {
            let entry = entry.wrap_err("enumerate prepared runtime projection")?;
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| eyre!("prepared runtime projection filename is not UTF-8"))?;
            let relative = if relative_prefix.is_empty() {
                name
            } else {
                format!("{relative_prefix}/{name}")
            };
            let file_type = entry
                .file_type()
                .wrap_err("inspect prepared runtime projection entry")?;
            ensure!(
                !file_type.is_symlink(),
                "prepared runtime projection {} contains a symbolic link",
                entry.path().display()
            );
            if file_type.is_dir() {
                directories.insert(relative.clone());
                collect_projection_entries(root, &entry.path(), &relative, files, directories)?;
            } else {
                ensure!(
                    file_type.is_file(),
                    "prepared runtime projection {} contains a special file",
                    entry.path().display()
                );
                files.insert(relative);
            }
        }
        Ok(())
    }
    let mut projected_files = BTreeSet::new();
    let mut projected_directories = BTreeSet::new();
    collect_projection_entries(
        &validation_dir,
        &validation_dir,
        "",
        &mut projected_files,
        &mut projected_directories,
    )?;
    ensure!(
        projected_files == expected_files && projected_directories == expected_directories,
        "content-addressed prepared {label} projection {} contains stale or unexpected entries",
        validation_dir.display()
    );
    let validation_dir = fs::canonicalize(&validation_dir).wrap_err_with(|| {
        format!(
            "canonicalize captured prepared {label} directory {}",
            validation_dir.display()
        )
    })?;
    Ok((files, validation_dir))
}

fn execution_policy_projection(config: &actual::Root) -> [u8; 32] {
    actual::execution_policy_digest_v1(
        &config.pipeline,
        &config.oracle,
        &config.crypto,
        &config.fraud_monitoring,
        &config.gov,
        &config.content,
        &config.settlement,
        [0x11; 32],
        [0x22; 32],
        Some([0x44; 32]),
    )
}

fn validate_runtime_projection_policy(
    source: &actual::Root,
    projected: &actual::Root,
    metadata: &ConsensusHandshakeMetadata,
) -> color_eyre::Result<()> {
    ensure!(
        matches!(source.sumeragi.role, actual::NodeRole::Validator)
            && matches!(projected.sumeragi.role, actual::NodeRole::Validator),
        "prepared committee members must run with sumeragi.role = \"validator\""
    );
    ensure!(
        source.common.chain == projected.common.chain
            && source.common.key_pair.public_key() == projected.common.key_pair.public_key(),
        "container runtime projection changed validator chain or identity"
    );
    let trusted_keys = |config: &actual::Root| {
        std::iter::once(&config.common.trusted_peers.value().myself)
            .chain(config.common.trusted_peers.value().others.iter())
            .map(|peer| peer.id().public_key().clone())
            .collect::<BTreeSet<_>>()
    };
    ensure!(
        trusted_keys(source) == trusted_keys(projected)
            && source.common.trusted_peers.value().pops
                == projected.common.trusted_peers.value().pops,
        "container runtime projection changed the authenticated validator roster"
    );
    let mode = match metadata.mode {
        SumeragiConsensusMode::Permissioned => WireConsensusMode::Permissioned,
        SumeragiConsensusMode::Npos => WireConsensusMode::Npos,
    };
    let cadence = Duration::from_millis(metadata.block_cadence_ms.get());
    let source_v2 = source
        .sumeragi
        .v2_config(cadence, mode)
        .wrap_err("derive source prepared Sumeragi v2 configuration")?;
    let projected_v2 = projected
        .sumeragi
        .v2_config(cadence, mode)
        .wrap_err("derive projected prepared Sumeragi v2 configuration")?;
    ensure!(
        source_v2.fingerprint() == projected_v2.fingerprint(),
        "container runtime projection changed the Sumeragi v2 safety/liveness fingerprint"
    );
    ensure!(
        execution_policy_projection(source) == execution_policy_projection(projected),
        "container runtime projection changed deterministic execution policy"
    );
    let source_nexus =
        actual::sumeragi_v2_nexus_amx_context_hash(&source.nexus, &source.pipeline, &[], &[]);
    let projected_nexus =
        actual::sumeragi_v2_nexus_amx_context_hash(&projected.nexus, &projected.pipeline, &[], &[]);
    ensure!(
        source_nexus == projected_nexus,
        "container runtime projection changed Nexus/AMX consensus policy"
    );
    Ok(())
}

fn ensure_container_projection_directory(directory: &Path) -> color_eyre::Result<()> {
    match fs::symlink_metadata(directory) {
        Ok(metadata) => ensure!(
            metadata.is_dir() && !metadata.file_type().is_symlink(),
            "prepared runtime projection path {} is not a regular directory",
            directory.display()
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt as _;
                let mut builder = fs::DirBuilder::new();
                builder.mode(0o700);
                builder.create(directory).wrap_err_with(|| {
                    format!(
                        "create prepared runtime projection directory {}",
                        directory.display()
                    )
                })?;
            }
            #[cfg(not(unix))]
            fs::create_dir(directory).wrap_err_with(|| {
                format!(
                    "create prepared runtime projection directory {}",
                    directory.display()
                )
            })?;
        }
        Err(error) => {
            return Err(error).wrap_err_with(|| {
                format!(
                    "inspect prepared runtime projection directory {}",
                    directory.display()
                )
            });
        }
    }
    #[cfg(unix)]
    {
        use rustix::fs::{Mode, OFlags, fchmod, open};
        use std::os::unix::fs::MetadataExt as _;

        let lexical = fs::symlink_metadata(directory).wrap_err_with(|| {
            format!(
                "inspect prepared runtime projection directory {}",
                directory.display()
            )
        })?;
        let opened = fs::File::from(
            open(
                directory,
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(std::io::Error::from)
            .wrap_err_with(|| {
                format!(
                    "open prepared runtime projection directory {}",
                    directory.display()
                )
            })?,
        );
        let before = opened.metadata().wrap_err_with(|| {
            format!(
                "inspect opened prepared runtime projection directory {}",
                directory.display()
            )
        })?;
        ensure!(
            !lexical.file_type().is_symlink()
                && before.is_dir()
                && same_file_snapshot(&lexical, &before)
                && before.uid() == rustix::process::geteuid().as_raw(),
            "prepared runtime projection directory {} changed while opening or is not owner-held",
            directory.display()
        );
        fchmod(&opened, Mode::from_raw_mode(0o700))
            .map_err(std::io::Error::from)
            .wrap_err_with(|| {
                format!(
                    "protect prepared runtime projection directory {}",
                    directory.display()
                )
            })?;
        opened.sync_all().wrap_err_with(|| {
            format!(
                "sync prepared runtime projection directory {}",
                directory.display()
            )
        })?;
        let after = opened.metadata().wrap_err_with(|| {
            format!(
                "reinspect prepared runtime projection directory {}",
                directory.display()
            )
        })?;
        let linked = fs::symlink_metadata(directory).wrap_err_with(|| {
            format!(
                "reinspect linked prepared runtime projection directory {}",
                directory.display()
            )
        })?;
        ensure!(
            same_file_snapshot(&after, &linked) && after.mode() & 0o777 == 0o700,
            "prepared runtime projection directory {} changed while being protected",
            directory.display()
        );
    }
    Ok(())
}

fn materialize_read_only_file_at(
    projection_dir: &Path,
    name: &str,
    content: &[u8],
) -> color_eyre::Result<PathBuf> {
    ensure!(
        !name.is_empty()
            && name != "."
            && name != ".."
            && !name.contains('/')
            && !name.contains('\\')
            && name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-')),
        "prepared runtime projection filename `{name}` is not portable"
    );
    ensure_container_projection_directory(projection_dir)?;

    let path = projection_dir.join(name);
    match fs::symlink_metadata(&path) {
        Ok(lexical) => {
            let mut options = fs::OpenOptions::new();
            options.read(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt as _;
                options.custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_NONBLOCK);
            }
            let mut file = options
                .open(&path)
                .wrap_err_with(|| format!("open prepared runtime projection {}", path.display()))?;
            let before = file.metadata().wrap_err_with(|| {
                format!(
                    "inspect opened prepared runtime projection {}",
                    path.display()
                )
            })?;
            ensure!(
                !lexical.file_type().is_symlink()
                    && before.is_file()
                    && same_file_snapshot(&lexical, &before),
                "prepared runtime projection {} changed while opening or is not a regular file",
                path.display()
            );
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt as _;
                ensure!(
                    before.uid() == rustix::process::geteuid().as_raw() && before.nlink() == 1,
                    "prepared runtime projection {} must be owner-held single-link data",
                    path.display()
                );
            }
            ensure!(
                before.len() == u64::try_from(content.len()).unwrap_or(u64::MAX),
                "content-addressed prepared runtime projection {} has a different length",
                path.display()
            );
            let mut existing = Vec::with_capacity(content.len());
            file.read_to_end(&mut existing)
                .wrap_err_with(|| format!("read prepared runtime projection {}", path.display()))?;
            let after_read = file.metadata().wrap_err_with(|| {
                format!("reinspect prepared runtime projection {}", path.display())
            })?;
            ensure!(
                existing == content && same_file_snapshot(&before, &after_read),
                "content-addressed prepared runtime projection {} changed or has different bytes",
                path.display()
            );
            #[cfg(unix)]
            {
                use rustix::fs::{Mode, fchmod};
                use std::os::unix::fs::MetadataExt as _;
                fchmod(&file, Mode::from_raw_mode(0o400))
                    .map_err(std::io::Error::from)
                    .wrap_err_with(|| {
                        format!("protect prepared runtime projection {}", path.display())
                    })?;
                file.sync_all().wrap_err_with(|| {
                    format!("sync prepared runtime projection {}", path.display())
                })?;
                let protected = file.metadata().wrap_err_with(|| {
                    format!("reinspect protected runtime projection {}", path.display())
                })?;
                let linked = fs::symlink_metadata(&path).wrap_err_with(|| {
                    format!("reinspect linked runtime projection {}", path.display())
                })?;
                ensure!(
                    same_file_snapshot(&protected, &linked) && protected.mode() & 0o777 == 0o400,
                    "prepared runtime projection {} changed while being protected",
                    path.display()
                );
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let mut options = fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt as _;
                options
                    .mode(0o600)
                    .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW);
            }
            let mut file = options.open(&path).wrap_err_with(|| {
                format!("create prepared runtime projection {}", path.display())
            })?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt as _;
                let created = file.metadata().wrap_err_with(|| {
                    format!("inspect new prepared runtime projection {}", path.display())
                })?;
                ensure!(
                    created.is_file()
                        && created.uid() == rustix::process::geteuid().as_raw()
                        && created.nlink() == 1,
                    "new prepared runtime projection {} has unsafe custody",
                    path.display()
                );
            }
            file.write_all(content)
                .wrap_err("write prepared runtime projection")?;
            file.sync_all()
                .wrap_err("sync prepared runtime projection")?;
            #[cfg(unix)]
            {
                use rustix::fs::{Mode, fchmod};
                use std::os::unix::fs::MetadataExt as _;
                fchmod(&file, Mode::from_raw_mode(0o400))
                    .map_err(std::io::Error::from)
                    .wrap_err_with(|| {
                        format!("protect new prepared runtime projection {}", path.display())
                    })?;
                file.sync_all()
                    .wrap_err("sync protected prepared runtime projection")?;
                let protected = file.metadata().wrap_err_with(|| {
                    format!(
                        "reinspect new prepared runtime projection {}",
                        path.display()
                    )
                })?;
                let linked = fs::symlink_metadata(&path).wrap_err_with(|| {
                    format!("reinspect linked runtime projection {}", path.display())
                })?;
                ensure!(
                    same_file_snapshot(&protected, &linked) && protected.mode() & 0o777 == 0o400,
                    "new prepared runtime projection {} changed while being protected",
                    path.display()
                );
            }
        }
        Err(error) => {
            return Err(error).wrap_err_with(|| {
                format!("inspect prepared runtime projection {}", path.display())
            });
        }
    }
    let root = fs::canonicalize(projection_dir).wrap_err_with(|| {
        format!(
            "canonicalize prepared runtime projection directory {}",
            projection_dir.display()
        )
    })?;
    Ok(root.join(name))
}

fn materialize_container_readable_file(
    projection_root: &Path,
    namespace: &str,
    name: &str,
    content: &[u8],
) -> color_eyre::Result<PathBuf> {
    ensure!(
        !namespace.is_empty()
            && namespace
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')),
        "prepared runtime projection namespace `{namespace}` is not portable"
    );
    let digest = hex::encode(iroha_crypto::Hash::new(content).as_ref());
    ensure_container_projection_directory(projection_root)?;
    let projection_dir = projection_root.join(format!("{namespace}-{digest}"));
    ensure_container_projection_directory(&projection_dir)?;
    materialize_read_only_file_at(&projection_dir, name, content)
}

fn materialize_runtime_projection(
    projection_root: &Path,
    index: usize,
    content: &str,
) -> color_eyre::Result<PathBuf> {
    materialize_container_readable_file(
        projection_root,
        &format!("peer{index}"),
        &format!("peer{index}.toml"),
        content.as_bytes(),
    )
}

struct CapturedValidationPath {
    table_path: &'static [&'static str],
    key: &'static str,
    source: PathBuf,
}

#[allow(clippy::too_many_arguments)]
fn capture_prepared_runtime_file(
    table: &mut toml::Table,
    projection_root: &Path,
    source: &Path,
    namespace: &str,
    filename: &str,
    target: &str,
    label: &str,
    max_bytes: u64,
    table_path: &'static [&'static str],
    key: &'static str,
) -> color_eyre::Result<(PreparedRuntimeFile, CapturedValidationPath)> {
    let content = read_runtime_file_bounded(source, label, max_bytes)?;
    let captured =
        materialize_container_readable_file(projection_root, namespace, filename, &content)?;
    set_toml_string(table, table_path, key, target)?;
    Ok((
        PreparedRuntimeFile {
            target: target.to_owned(),
            content,
        },
        CapturedValidationPath {
            table_path,
            key,
            source: captured,
        },
    ))
}

fn ensure_fresh_state_directory(
    path: &Path,
    label: &str,
    config_path: &Path,
) -> color_eyre::Result<()> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(error).wrap_err_with(|| {
                format!(
                    "inspect {label} selected by prepared validator {}",
                    config_path.display()
                )
            });
        }
    };
    ensure!(
        metadata.is_dir() && !metadata.file_type().is_symlink(),
        "prepared validator {} {label} {} is not a regular directory",
        config_path.display(),
        path.display()
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        ensure!(
            metadata.uid() == rustix::process::geteuid().as_raw() && metadata.mode() & 0o022 == 0,
            "prepared validator {} {label} {} must be owner-held and not group/world writable",
            config_path.display(),
            path.display()
        );
    }
    ensure!(
        fs::read_dir(path)
            .wrap_err_with(|| format!("read prepared {label} {}", path.display()))?
            .next()
            .transpose()
            .wrap_err_with(|| format!("enumerate prepared {label} {}", path.display()))?
            .is_none(),
        "prepared validator {} {label} {} is non-empty; prepared Compose only admits fresh-genesis state and does not migrate live state",
        config_path.display(),
        path.display()
    );
    Ok(())
}

fn ensure_fresh_state_file(path: &Path, label: &str, config_path: &Path) -> color_eyre::Result<()> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(error).wrap_err_with(|| {
                format!(
                    "inspect {label} selected by prepared validator {}",
                    config_path.display()
                )
            });
        }
    };
    ensure!(
        metadata.is_file() && !metadata.file_type().is_symlink() && metadata.len() == 0,
        "prepared validator {} {label} {} contains existing state; prepared Compose only admits fresh-genesis state",
        config_path.display(),
        path.display()
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        ensure!(
            metadata.uid() == rustix::process::geteuid().as_raw()
                && metadata.nlink() == 1
                && metadata.mode() & 0o022 == 0,
            "prepared validator {} {label} {} must be owner-held, single-link, and not group/world writable",
            config_path.display(),
            path.display()
        );
    }
    Ok(())
}

fn ensure_fresh_prepared_state(
    config: &actual::Root,
    config_path: &Path,
) -> color_eyre::Result<()> {
    let config_root = config_path.parent().unwrap_or_else(|| Path::new("."));
    let resolve = |path: &Path| {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            config_root.join(path)
        }
    };
    let directory =
        |path: &Path, label: &str| ensure_fresh_state_directory(&resolve(path), label, config_path);
    let file =
        |path: &Path, label: &str| ensure_fresh_state_file(&resolve(path), label, config_path);

    directory(config.kura.store_dir.value(), "Kura store")?;
    directory(config.snapshot.store_dir.value(), "snapshot store")?;
    directory(&config.torii.data_dir, "Torii data")?;
    file(
        Path::new(
            config
                .network
                .soranet_handshake
                .pow
                .revocation_store_path
                .as_ref(),
        ),
        "SoraNet ticket-revocation store",
    )?;
    directory(
        &config.torii.da_ingest.replay_cache_store_dir,
        "DA replay-cache store",
    )?;
    directory(
        &config.torii.da_ingest.manifest_store_dir,
        "DA manifest store",
    )?;
    directory(&config.torii.sorafs_storage.data_dir, "SoraFS data store")?;
    if let Some(pop_credentials) = config.torii.sorafs_storage.pop_credentials.as_ref() {
        directory(&pop_credentials.issuer_state_dir, "SoraFS PoP issuer state")?;
        directory(&pop_credentials.wallet_state_dir, "SoraFS PoP wallet state")?;
    }
    if let Some(moderation) = config.torii.sorafs_storage.moderation_orchestrator.as_ref() {
        file(&moderation.checkpoint_path, "SoraFS moderation checkpoint")?;
    }
    if let Some(evidence_viewer) = config.torii.sorafs_storage.evidence_viewer.as_ref() {
        file(
            &evidence_viewer.checkpoint_path,
            "SoraFS evidence-viewer checkpoint",
        )?;
    }
    file(
        &config.torii.sorafs_discovery.replay_checkpoint_path,
        "SoraFS discovery replay checkpoint",
    )?;
    directory(&config.torii.sorafs_por.state_dir, "SoraFS PoR state")?;
    file(
        &config.torii.sorafs_por.vrf_state_path,
        "SoraFS PoR VRF state",
    )?;
    if let Some(path) = config.torii.sorafs_gc.state_dir.as_deref() {
        directory(path, "SoraFS GC state")?;
    }
    if let Some(compliance) = config.torii.sorafs_gateway.compliance.as_ref() {
        file(
            &compliance.checkpoint_path,
            "SoraFS gateway-compliance checkpoint",
        )?;
    }
    if let Some(issuer) = config.torii.privacy_bootle_lantern_issuer.as_ref() {
        directory(&issuer.state_dir, "privacy issuer state")?;
    }
    directory(
        &config.soracloud_runtime.state_dir,
        "Soracloud runtime state",
    )?;
    if let Some(path) = config.tiered_state.cold_store_root.as_deref() {
        directory(path, "tiered cold state")?;
    }
    if let Some(path) = config.tiered_state.da_store_root.as_deref() {
        directory(path, "tiered DA state")?;
    }
    directory(
        &config.streaming.session_store_dir,
        "streaming session state",
    )?;
    directory(
        &config.streaming.soranet.provision_spool_dir,
        "streaming SoraNet spool",
    )?;
    directory(
        &config.streaming.soravpn.provision_spool_dir,
        "streaming SoraVPN spool",
    )?;
    if let Some(path) = config.telemetry_integrity.state_dir.as_deref() {
        directory(path, "telemetry-integrity state")?;
    }
    if let Some(path) = config.dev_telemetry.out_file.as_ref() {
        file(path.value(), "development telemetry output")?;
    }
    if let Some(runtime) = config.torii.sorafs_storage.reputation_runtime.as_ref() {
        directory(&runtime.state_dir, "SoraFS reputation state")?;
        directory(
            &runtime.finalized_archive_root,
            "SoraFS finalized reputation archive",
        )?;
    }
    if let Some(runtime) = config.torii.sorafs_storage.hedging_billing_runtime.as_ref() {
        directory(&runtime.state_dir, "SoraFS hedging state")?;
    }
    if let Some(path) = config.torii.sorafs_storage.governance_dag_dir.as_deref() {
        directory(path, "SoraFS Governance DAG state/feed")?;
    }
    if let Some(path) = config
        .torii
        .sorafs_storage
        .governance_dag_service
        .state_dir
        .as_deref()
    {
        directory(path, "SoraFS Governance DAG service state")?;
    }
    if config.torii.iso_bridge.enabled {
        if let Some(path) = config.torii.iso_bridge.store_dir.as_deref() {
            directory(path, "ISO bridge state")?;
        }
        if let Some(path) = config.torii.iso_bridge.audit_export_dir.as_deref() {
            directory(path, "ISO bridge audit export")?;
        }
        if let Some(path) = config.torii.iso_bridge.reference_data.cache_dir.as_deref() {
            directory(path, "ISO reference-data cache")?;
        }
    }
    Ok(())
}

fn project_prepared_runtime_config(
    config_dir: &Path,
    projection_root: &Path,
    index: usize,
    mut table: toml::Table,
    source: &actual::Root,
    metadata: &ConsensusHandshakeMetadata,
    peers: &[PreparedRuntimePeer],
) -> color_eyre::Result<(
    PathBuf,
    [u8; 32],
    Vec<PreparedRuntimeFile>,
    Vec<PreparedSecretFile>,
    bool,
    actual::Root,
)> {
    const RANS_TABLE_MAX_BYTES: u64 = 4 * 1024 * 1024;
    const SITE_BINDINGS_MAX_BYTES: u64 = 4 * 1024 * 1024;
    const RANS_TARGET: &str = "/config/runtime/rans_seed0.toml";
    const LANE_MANIFEST_TARGET: &str = "/config/runtime/lane-manifests";
    const LANE_CACHE_TARGET: &str = "/config/runtime/lane-cache";
    const LANE_POLICY_TARGET: &str = "/config/runtime/lane-policies";
    const SORAFS_ADMISSION_TARGET: &str = "/config/runtime/sorafs-admission";
    const SORAFS_SALT_TARGET: &str = "/config/runtime/sorafs-salt-schedule";
    const KAGEMUSHA_ARTIFACT_TARGET: &str = "/config/runtime/kagemusha-artifacts";
    const SITE_BINDINGS_TARGET: &str = "/config/runtime/sorafs_sites.json";

    let source_table = table.clone();
    let (effective_source, source_requires_sora) =
        effective_runtime_config(source.clone(), &source_table);
    let source = &effective_source;
    ensure_fresh_prepared_state(source, &config_dir.join(format!("peer{index}.toml")))?;
    rewrite_container_network(&mut table, index, peers)?;
    set_toml_string(
        &mut table,
        &["network", "soranet_handshake", "pow"],
        "revocation_store_path",
        "/storage/network/soranet-ticket-revocations.norito",
    )?;

    let rans_content = read_runtime_file_bounded(
        &source.streaming.codec.rans_tables_path,
        "rANS table",
        RANS_TABLE_MAX_BYTES,
    )?;
    let captured_rans_source = materialize_container_readable_file(
        projection_root,
        "rans",
        "rans_seed0.toml",
        &rans_content,
    )?;
    let mut runtime_files = vec![PreparedRuntimeFile {
        target: RANS_TARGET.to_owned(),
        content: rans_content,
    }];
    let mut secret_files = Vec::new();
    let mut captured_validation_paths = Vec::new();

    set_toml_string(
        &mut table,
        &["genesis"],
        "file",
        "/genesis/genesis.signed.nrt",
    )?;
    remove_toml_key(&mut table, &["genesis"], "manifest_json")?;
    set_toml_string(&mut table, &["kura"], "store_dir", "/storage/kura")?;
    set_toml_string(&mut table, &["snapshot"], "store_dir", "/storage/snapshot")?;
    set_toml_string(&mut table, &["torii"], "data_dir", "/storage/torii")?;
    set_toml_string(
        &mut table,
        &["torii", "da_ingest"],
        "replay_cache_store_dir",
        "/storage/torii/da-replay-cache",
    )?;
    set_toml_string(
        &mut table,
        &["torii", "da_ingest"],
        "manifest_store_dir",
        "/storage/torii/da-manifests",
    )?;
    set_toml_string(
        &mut table,
        &["sorafs", "storage"],
        "data_dir",
        "/storage/sorafs",
    )?;
    let sorafs_storage = &source.torii.sorafs_storage;
    if sorafs_storage.pop_credentials.is_some() {
        set_toml_string(
            &mut table,
            &["sorafs", "storage", "pop_credentials"],
            "issuer_state_dir",
            "/storage/sorafs/pop-credentials/issuer",
        )?;
        set_toml_string(
            &mut table,
            &["sorafs", "storage", "pop_credentials"],
            "wallet_state_dir",
            "/storage/sorafs/pop-credentials/wallet",
        )?;
    }
    if sorafs_storage.moderation_orchestrator.is_some() {
        set_toml_string(
            &mut table,
            &["sorafs", "storage", "moderation_orchestrator"],
            "checkpoint_path",
            "/storage/sorafs/moderation/checkpoint.norito",
        )?;
    }
    if sorafs_storage.evidence_viewer.is_some() {
        set_toml_string(
            &mut table,
            &["sorafs", "storage", "evidence_viewer"],
            "checkpoint_path",
            "/storage/sorafs/evidence-viewer/checkpoint.norito",
        )?;
    }
    if sorafs_storage.reputation_runtime.is_some() {
        set_toml_string(
            &mut table,
            &["sorafs", "storage", "reputation_runtime"],
            "state_dir",
            "/storage/sorafs/reputation",
        )?;
    }
    if sorafs_storage.hedging_billing_runtime.is_some() {
        set_toml_string(
            &mut table,
            &["sorafs", "storage", "hedging_billing_runtime"],
            "state_dir",
            "/storage/sorafs/hedging-billing",
        )?;
    }
    if sorafs_storage.governance_dag_dir.is_some() {
        set_toml_string(
            &mut table,
            &["sorafs", "storage"],
            "governance_dag_dir",
            "/storage/sorafs/governance-dag",
        )?;
    }
    if sorafs_storage.governance_dag_service.state_dir.is_some() {
        set_toml_string(
            &mut table,
            &["sorafs", "storage", "governance_dag_service"],
            "state_dir",
            "/storage/sorafs/governance-dag-service",
        )?;
    }
    set_toml_string(
        &mut table,
        &["sorafs", "discovery"],
        "replay_checkpoint_path",
        "/storage/sorafs/discovery/replay-checkpoint.nrt",
    )?;
    set_toml_string(
        &mut table,
        &["sorafs", "por"],
        "state_dir",
        "/storage/sorafs/por",
    )?;
    if source.torii.sorafs_gc.state_dir.is_some() {
        set_toml_string(
            &mut table,
            &["sorafs", "gc"],
            "state_dir",
            "/storage/sorafs/gc",
        )?;
    }
    if source.torii.sorafs_gateway.compliance.is_some() {
        set_toml_string(
            &mut table,
            &["sorafs", "gateway", "compliance"],
            "checkpoint_path",
            "/storage/sorafs/gateway-compliance/checkpoint.norito",
        )?;
    }
    if source.torii.privacy_bootle_lantern_issuer.is_some() {
        set_toml_string(
            &mut table,
            &["torii", "privacy_bootle_lantern_issuer"],
            "state_dir",
            "/storage/torii/privacy-issuer",
        )?;
    }
    if source.torii.iso_bridge.enabled {
        if source.torii.iso_bridge.store_dir.is_some() {
            set_toml_string(
                &mut table,
                &["torii", "iso_bridge"],
                "store_dir",
                "/storage/torii/iso-bridge",
            )?;
        }
        if source.torii.iso_bridge.audit_export_dir.is_some() {
            set_toml_string(
                &mut table,
                &["torii", "iso_bridge"],
                "audit_export_dir",
                "/storage/torii/iso-bridge-audit",
            )?;
        }
        if source.torii.iso_bridge.reference_data.cache_dir.is_some() {
            set_toml_string(
                &mut table,
                &["torii", "iso_bridge", "reference_data"],
                "cache_dir",
                "/storage/torii/iso-reference-cache",
            )?;
        }
    }
    set_toml_string(
        &mut table,
        &["soracloud_runtime"],
        "state_dir",
        "/storage/soracloud_runtime",
    )?;
    if source.tiered_state.cold_store_root.is_some() {
        set_toml_string(
            &mut table,
            &["tiered_state"],
            "cold_store_root",
            "/storage/tiered_state",
        )?;
    } else {
        remove_toml_key(&mut table, &["tiered_state"], "cold_store_root")?;
    }
    if source.tiered_state.da_store_root.is_some() {
        set_toml_string(
            &mut table,
            &["tiered_state"],
            "da_store_root",
            "/storage/da_wsv_snapshots",
        )?;
    } else {
        remove_toml_key(&mut table, &["tiered_state"], "da_store_root")?;
    }
    if source.telemetry_integrity.state_dir.is_some() {
        set_toml_string(
            &mut table,
            &["telemetry_integrity"],
            "state_dir",
            "/storage/telemetry-integrity",
        )?;
    }
    if source.dev_telemetry.out_file.is_some() {
        set_toml_string(
            &mut table,
            &["dev_telemetry"],
            "out_file",
            "/storage/telemetry/development.log",
        )?;
    }
    set_toml_string(
        &mut table,
        &["streaming"],
        "session_store_dir",
        "/storage/streaming",
    )?;
    set_toml_string(
        &mut table,
        &["streaming", "codec"],
        "rans_tables_path",
        RANS_TARGET,
    )?;
    {
        let streaming = ensure_toml_table(&mut table, &["streaming"])?;
        let mut soranet = match streaming.get("soranet") {
            Some(toml::Value::Table(existing)) => existing.clone(),
            Some(_) => {
                return Err(eyre!(
                    "prepared runtime projection expected `streaming.soranet` to be a TOML table"
                ));
            }
            None => {
                let source = &source.streaming.soranet;
                let padding_budget_ms = source.padding_budget_ms.ok_or_else(|| {
                    eyre!(
                        "prepared runtime projection cannot encode an absent \
                         streaming.soranet.padding_budget_ms in TOML"
                    )
                })?;
                let mut complete = toml::Table::new();
                complete.insert("enabled".into(), toml::Value::Boolean(source.enabled));
                complete.insert(
                    "exit_multiaddr".into(),
                    toml::Value::String(source.exit_multiaddr.clone()),
                );
                complete.insert(
                    "padding_budget_ms".into(),
                    toml::Value::Integer(i64::from(padding_budget_ms)),
                );
                complete.insert(
                    "access_kind".into(),
                    toml::Value::String(source.access_kind.as_str().to_owned()),
                );
                complete.insert(
                    "channel_salt".into(),
                    toml::Value::String(source.channel_salt.clone()),
                );
                complete.insert(
                    "provision_spool_max_bytes".into(),
                    toml::Value::Integer(
                        i64::try_from(source.provision_spool_max_bytes.get()).wrap_err(
                            "streaming.soranet.provision_spool_max_bytes exceeds TOML integer range",
                        )?,
                    ),
                );
                complete.insert(
                    "provision_window_segments".into(),
                    toml::Value::Integer(
                        i64::try_from(source.provision_window_segments).wrap_err(
                            "streaming.soranet.provision_window_segments exceeds TOML integer range",
                        )?,
                    ),
                );
                complete.insert(
                    "provision_queue_capacity".into(),
                    toml::Value::Integer(i64::try_from(source.provision_queue_capacity).wrap_err(
                        "streaming.soranet.provision_queue_capacity exceeds TOML integer range",
                    )?),
                );
                complete
            }
        };
        soranet.insert(
            "provision_spool_dir".into(),
            toml::Value::String("/storage/streaming/soranet".to_owned()),
        );
        streaming.insert("soranet".into(), toml::Value::Table(soranet));

        let mut soravpn = match streaming.get("soravpn") {
            Some(toml::Value::Table(existing)) => existing.clone(),
            Some(_) => {
                return Err(eyre!(
                    "prepared runtime projection expected `streaming.soravpn` to be a TOML table"
                ));
            }
            None => {
                let mut complete = toml::Table::new();
                complete.insert(
                    "provision_spool_max_bytes".into(),
                    toml::Value::Integer(
                        i64::try_from(source.streaming.soravpn.provision_spool_max_bytes.get())
                            .wrap_err(
                                "streaming.soravpn.provision_spool_max_bytes exceeds TOML integer range",
                            )?,
                    ),
                );
                complete
            }
        };
        soravpn.insert(
            "provision_spool_dir".into(),
            toml::Value::String("/storage/streaming/soravpn".to_owned()),
        );
        streaming.insert("soravpn".into(), toml::Value::Table(soravpn));
    }
    let mut captured_onboarding_private_key = None;
    if let Some(onboarding) = source.torii.account_onboarding.as_ref() {
        let content = read_runtime_file_bounded(
            &onboarding.private_key_file,
            "account-onboarding private key",
            64 * 1024,
        )?;
        let captured = materialize_container_readable_file(
            projection_root,
            &format!("peer{index}-onboarding-secret"),
            "private.key",
            &content,
        )?;
        let target = format!("/run/secrets/iroha_peer{index}_onboarding_private_key");
        set_toml_string(
            &mut table,
            &["torii", "account_onboarding"],
            "private_key_file",
            &target,
        )?;
        secret_files.push(PreparedSecretFile {
            target,
            source_path: captured.clone(),
        });
        captured_onboarding_private_key = Some(captured);
    }
    let mut captured_faucet_private_key = None;
    if let Some(faucet) = source.torii.faucet.as_ref() {
        let content =
            read_runtime_file_bounded(&faucet.private_key_file, "faucet private key", 64 * 1024)?;
        let captured = materialize_container_readable_file(
            projection_root,
            &format!("peer{index}-faucet-secret"),
            "private.key",
            &content,
        )?;
        let target = format!("/run/secrets/iroha_peer{index}_faucet_private_key");
        set_toml_string(
            &mut table,
            &["torii", "faucet"],
            "private_key_file",
            &target,
        )?;
        secret_files.push(PreparedSecretFile {
            target,
            source_path: captured.clone(),
        });
        captured_faucet_private_key = Some(captured);
    }

    if let Some(path) = source
        .torii
        .tx_history
        .as_ref()
        .and_then(|history| history.mandatory_aliases_path.as_deref())
    {
        let (file, captured) = capture_prepared_runtime_file(
            &mut table,
            projection_root,
            path,
            "tx-history-aliases",
            "mandatory_aliases.norito",
            "/config/runtime/tx-history-mandatory-aliases.norito",
            "transaction-history mandatory-alias policy",
            16 * 1024 * 1024,
            &["torii", "tx_history"],
            "mandatory_aliases_path",
        )?;
        runtime_files.push(file);
        captured_validation_paths.push(captured);
    }
    if source.torii.sorafs_storage.moderation_screening_enabled {
        let path = source
            .torii
            .sorafs_storage
            .moderation_screening_authority_bundle_path
            .as_deref()
            .ok_or_else(|| {
                eyre!("prepared SoraFS moderation screening requires an authority-bundle path")
            })?;
        let (file, captured) = capture_prepared_runtime_file(
            &mut table,
            projection_root,
            path,
            "sorafs-moderation-authority",
            "authority_bundle.norito",
            "/config/runtime/sorafs-moderation-authority.norito",
            "SoraFS moderation authority bundle",
            16 * 1024 * 1024,
            &["sorafs", "storage"],
            "moderation_screening_authority_bundle_path",
        )?;
        runtime_files.push(file);
        captured_validation_paths.push(captured);
    }
    if let Some(path) = source
        .torii
        .sorafs_storage
        .reputation_trust_policy_path
        .as_deref()
    {
        let (file, captured) = capture_prepared_runtime_file(
            &mut table,
            projection_root,
            path,
            "sorafs-reputation-trust",
            "trust_policy.norito",
            "/config/runtime/sorafs-reputation-trust.norito",
            "SoraFS reputation trust policy",
            16 * 1024 * 1024,
            &["sorafs", "storage"],
            "reputation_trust_policy_path",
        )?;
        runtime_files.push(file);
        captured_validation_paths.push(captured);
    }
    if let Some(path) = source
        .torii
        .sorafs_storage
        .hedging_feed_trust_policy_path
        .as_deref()
    {
        let (file, captured) = capture_prepared_runtime_file(
            &mut table,
            projection_root,
            path,
            "sorafs-hedging-feed-trust",
            "trust_policy.norito",
            "/config/runtime/sorafs-hedging-feed-trust.norito",
            "SoraFS hedging feed trust policy",
            16 * 1024 * 1024,
            &["sorafs", "storage"],
            "hedging_feed_trust_policy_path",
        )?;
        runtime_files.push(file);
        captured_validation_paths.push(captured);
    }
    if let Some(runtime) = source.torii.sorafs_storage.hedging_billing_runtime.as_ref() {
        let (file, captured) = capture_prepared_runtime_file(
            &mut table,
            projection_root,
            &runtime.service_policy_path,
            "sorafs-hedging-service-policy",
            "service_policy.norito",
            "/config/runtime/sorafs-hedging-service-policy.norito",
            "SoraFS hedging service policy",
            16 * 1024 * 1024,
            &["sorafs", "storage", "hedging_billing_runtime"],
            "service_policy_path",
        )?;
        runtime_files.push(file);
        captured_validation_paths.push(captured);
    }
    if source.torii.iso_bridge.enabled {
        let reference_data = &source.torii.iso_bridge.reference_data;
        for (key, path, filename) in [
            (
                "isin_crosswalk_path",
                reference_data.isin_crosswalk_path.as_deref(),
                "isin_crosswalk.snapshot",
            ),
            (
                "bic_lei_path",
                reference_data.bic_lei_path.as_deref(),
                "bic_lei.snapshot",
            ),
            (
                "mic_directory_path",
                reference_data.mic_directory_path.as_deref(),
                "mic_directory.snapshot",
            ),
            (
                "csd_venue_path",
                reference_data.csd_venue_path.as_deref(),
                "csd_venue.snapshot",
            ),
            (
                "securities_account_path",
                reference_data.securities_account_path.as_deref(),
                "securities_account.snapshot",
            ),
            (
                "cash_leg_path",
                reference_data.cash_leg_path.as_deref(),
                "cash_leg.snapshot",
            ),
        ] {
            let Some(path) = path else {
                continue;
            };
            let target = format!("/config/runtime/iso-reference/{filename}");
            let namespace = format!("iso-reference-{}", key.replace('_', "-"));
            let (file, captured) = capture_prepared_runtime_file(
                &mut table,
                projection_root,
                path,
                &namespace,
                filename,
                &target,
                "ISO reference-data snapshot",
                64 * 1024 * 1024,
                &["torii", "iso_bridge", "reference_data"],
                key,
            )?;
            runtime_files.push(file);
            captured_validation_paths.push(captured);
        }
    }
    let offline = &source.settlement.offline;
    if let Some(path) = offline.kagemusha_release_policy_path.as_deref() {
        let (file, captured) = capture_prepared_runtime_file(
            &mut table,
            projection_root,
            path,
            "kagemusha-release-policy",
            "release_policy.norito",
            "/config/runtime/kagemusha-release-policy.norito",
            "Kagemusha release policy",
            64 * 1024 * 1024,
            &["settlement", "offline"],
            "kagemusha_release_policy_path",
        )?;
        runtime_files.push(file);
        captured_validation_paths.push(captured);
    }
    if let Some(path) = offline.kagemusha_catalog_qualification_seal_path.as_deref() {
        let (file, captured) = capture_prepared_runtime_file(
            &mut table,
            projection_root,
            path,
            "kagemusha-catalog-seal",
            "catalog_seal.norito",
            "/config/runtime/kagemusha-catalog-seal.norito",
            "Kagemusha catalog qualification seal",
            64 * 1024 * 1024,
            &["settlement", "offline"],
            "kagemusha_catalog_qualification_seal_path",
        )?;
        runtime_files.push(file);
        captured_validation_paths.push(captured);
    }

    let captured_manifest_directory = if source.nexus.enabled {
        if let Some(manifest_directory) = source.nexus.registry.manifest_directory.as_deref() {
            let (files, validation_directory) = collect_runtime_directory(
                manifest_directory,
                projection_root,
                "lane-manifests",
                LANE_MANIFEST_TARGET,
                "lane manifest",
            )?;
            runtime_files.extend(files);
            set_toml_string(
                &mut table,
                &["nexus", "registry"],
                "manifest_directory",
                LANE_MANIFEST_TARGET,
            )?;
            Some(validation_directory)
        } else {
            None
        }
    } else {
        remove_toml_key(&mut table, &["nexus", "registry"], "manifest_directory")?;
        None
    };
    let captured_manifest_cache_directory = if source.nexus.enabled {
        if let Some(cache_directory) = source.nexus.registry.cache_directory.as_deref() {
            let (files, validation_directory) = collect_runtime_directory(
                cache_directory,
                projection_root,
                "lane-cache",
                LANE_CACHE_TARGET,
                "lane manifest cache",
            )?;
            runtime_files.extend(files);
            set_toml_string(
                &mut table,
                &["nexus", "registry"],
                "cache_directory",
                LANE_CACHE_TARGET,
            )?;
            Some(validation_directory)
        } else {
            None
        }
    } else {
        remove_toml_key(&mut table, &["nexus", "registry"], "cache_directory")?;
        None
    };
    let captured_compliance_policy_directory = if source.nexus.compliance.enabled {
        if let Some(policy_directory) = source.nexus.compliance.policy_dir.as_deref() {
            let (files, validation_directory) = collect_runtime_directory(
                policy_directory,
                projection_root,
                "lane-policies",
                LANE_POLICY_TARGET,
                "lane compliance policy",
            )?;
            runtime_files.extend(files);
            set_toml_string(
                &mut table,
                &["nexus", "compliance"],
                "policy_dir",
                LANE_POLICY_TARGET,
            )?;
            Some(validation_directory)
        } else {
            None
        }
    } else {
        remove_toml_key(&mut table, &["nexus", "compliance"], "policy_dir")?;
        None
    };
    let captured_sorafs_admission_directory =
        if let Some(admission) = source.torii.sorafs_discovery.admission.as_ref() {
            let (files, validation_directory) = collect_runtime_directory(
                &admission.envelopes_dir,
                projection_root,
                "sorafs-admission",
                SORAFS_ADMISSION_TARGET,
                "SoraFS admission envelope",
            )?;
            runtime_files.extend(files);
            set_toml_string(
                &mut table,
                &["sorafs", "discovery", "admission"],
                "envelopes_dir",
                SORAFS_ADMISSION_TARGET,
            )?;
            Some(validation_directory)
        } else {
            None
        };
    if let Some(salt_directory) = source.torii.sorafs_gateway.salt_schedule_dir.as_deref() {
        let (files, validation_directory) = collect_runtime_directory(
            salt_directory,
            projection_root,
            "sorafs-salt-schedule",
            SORAFS_SALT_TARGET,
            "SoraFS salt schedule",
        )?;
        runtime_files.extend(files);
        set_toml_string(
            &mut table,
            &["sorafs", "gateway"],
            "salt_schedule_dir",
            SORAFS_SALT_TARGET,
        )?;
        captured_validation_paths.push(CapturedValidationPath {
            table_path: &["sorafs", "gateway"],
            key: "salt_schedule_dir",
            source: validation_directory,
        });
    }
    if let Some(artifact_directory) = offline.kagemusha_artifact_dir.as_deref() {
        let (files, validation_directory) = collect_runtime_directory(
            artifact_directory,
            projection_root,
            "kagemusha-artifacts",
            KAGEMUSHA_ARTIFACT_TARGET,
            "Kagemusha release artifact",
        )?;
        runtime_files.extend(files);
        set_toml_string(
            &mut table,
            &["settlement", "offline"],
            "kagemusha_artifact_dir",
            KAGEMUSHA_ARTIFACT_TARGET,
        )?;
        captured_validation_paths.push(CapturedValidationPath {
            table_path: &["settlement", "offline"],
            key: "kagemusha_artifact_dir",
            source: validation_directory,
        });
    }

    let mut captured_site_bindings = None;
    if let Some(site_bindings) = source.torii.sorafs_gateway.site_bindings.path.as_deref() {
        let site_content = read_runtime_file_bounded(
            site_bindings,
            "SoraFS site bindings",
            SITE_BINDINGS_MAX_BYTES,
        )?;
        runtime_files.push(PreparedRuntimeFile {
            target: SITE_BINDINGS_TARGET.to_owned(),
            content: site_content.clone(),
        });
        captured_site_bindings = Some(materialize_container_readable_file(
            projection_root,
            "sorafs-site-bindings",
            "sorafs_sites.json",
            &site_content,
        )?);
        set_toml_string(
            &mut table,
            &["sorafs", "gateway", "site_bindings"],
            "path",
            SITE_BINDINGS_TARGET,
        )?;
    }

    let mut validation_table = table.clone();
    for captured in &captured_validation_paths {
        set_toml_string(
            &mut validation_table,
            captured.table_path,
            captured.key,
            captured.source.to_string_lossy(),
        )?;
    }
    set_toml_string(
        &mut validation_table,
        &["streaming", "codec"],
        "rans_tables_path",
        captured_rans_source.to_string_lossy(),
    )?;
    if let Some(manifest_directory) = captured_manifest_directory.as_deref() {
        set_toml_string(
            &mut validation_table,
            &["nexus", "registry"],
            "manifest_directory",
            manifest_directory.to_string_lossy(),
        )?;
    }
    if let Some(cache_directory) = captured_manifest_cache_directory.as_deref() {
        set_toml_string(
            &mut validation_table,
            &["nexus", "registry"],
            "cache_directory",
            cache_directory.to_string_lossy(),
        )?;
    }
    if let Some(policy_directory) = captured_compliance_policy_directory.as_deref() {
        set_toml_string(
            &mut validation_table,
            &["nexus", "compliance"],
            "policy_dir",
            policy_directory.to_string_lossy(),
        )?;
    }
    if let Some(admission_directory) = captured_sorafs_admission_directory.as_deref() {
        set_toml_string(
            &mut validation_table,
            &["sorafs", "discovery", "admission"],
            "envelopes_dir",
            admission_directory.to_string_lossy(),
        )?;
    }
    if let Some(site_source) = captured_site_bindings.as_deref() {
        set_toml_string(
            &mut validation_table,
            &["sorafs", "gateway", "site_bindings"],
            "path",
            site_source.to_string_lossy(),
        )?;
    }
    if let Some(secret) = captured_onboarding_private_key.as_deref() {
        set_toml_string(
            &mut validation_table,
            &["torii", "account_onboarding"],
            "private_key_file",
            secret.to_string_lossy(),
        )?;
    }
    if let Some(secret) = captured_faucet_private_key.as_deref() {
        set_toml_string(
            &mut validation_table,
            &["torii", "faucet"],
            "private_key_file",
            secret.to_string_lossy(),
        )?;
    }
    let _chain_discriminant =
        ChainDiscriminantGuard::enter(*source.common.chain_discriminant.value());
    let projected = actual::Root::from_toml_source(TomlSource::new(
        config_dir.join(format!("peer{index}.container-validation.toml")),
        validation_table.clone(),
    ))
    .map_err(|error| {
        eyre!("container runtime projection for prepared validator {index} is invalid: {error:?}")
    })?;
    let (projected_effective, requires_sora_profile) =
        effective_runtime_config(projected, &validation_table);
    ensure!(
        source_requires_sora == requires_sora_profile,
        "container runtime projection changed Sora profile requirements"
    );
    validate_runtime_projection_policy(source, &projected_effective, metadata)?;

    let mut content = toml::to_string_pretty(&table)
        .wrap_err("serialize container-safe prepared runtime projection")?;
    if !content.ends_with('\n') {
        content.push('\n');
    }
    let content_blake3 = *blake3::hash(content.as_bytes()).as_bytes();
    let path = materialize_runtime_projection(projection_root, index, &content)?;
    Ok((
        path,
        content_blake3,
        runtime_files,
        secret_files,
        requires_sora_profile,
        projected_effective,
    ))
}

fn load_prepared_bundle(
    config_dir: &Path,
    projection_root: &Path,
    count: std::num::NonZeroU16,
    manifest: &RawGenesisTransaction,
    build_line: PreparedBuildLine,
) -> color_eyre::Result<PreparedBundle> {
    let signed_block = config_dir.join("genesis.signed.nrt");
    let public_key_path = config_dir.join(crate::localnet::GENESIS_PUBLIC_KEY_FILE);
    let expected_hash_path = config_dir.join(crate::localnet::GENESIS_EXPECTED_HASH_FILE);
    let canonical_signed_block = fs::canonicalize(&signed_block).wrap_err_with(|| {
        format!(
            "canonicalize prepared signed genesis {}",
            signed_block.display()
        )
    })?;
    let validated =
        validate_prepared_genesis(&signed_block, &public_key_path, &expected_hash_path)?;
    validate_prepared_manifest_binding(manifest, &validated.block, &validated.public_key)?;
    let signed_metadata = signed_genesis_consensus_metadata(&validated.block)?;
    let config_paths = prepared_peer_config_paths(config_dir, count)?;

    let mut chain = None;
    let mut admitted = Vec::with_capacity(config_paths.len());
    let mut validator_keys = BTreeSet::new();
    let mut exposed_ports = BTreeSet::new();
    for (index, path) in config_paths.iter().enumerate() {
        let ParsedPreparedPeerConfig {
            actual: config,
            table,
        } = parse_prepared_peer_config(path)?;
        if let Some(expected_chain) = chain.as_ref() {
            ensure!(
                &config.common.chain == expected_chain,
                "prepared validator config {} uses chain {}, expected {}",
                path.display(),
                config.common.chain,
                expected_chain
            );
        } else {
            chain = Some(config.common.chain.clone());
        }
        ensure!(
            *config.common.chain_discriminant.value() == manifest.chain_discriminant(),
            "prepared validator config {} uses chain discriminant {}, expected {}",
            path.display(),
            config.common.chain_discriminant.value(),
            manifest.chain_discriminant()
        );
        ensure!(
            matches!(config.sumeragi.role, actual::NodeRole::Validator),
            "prepared validator config {} must set sumeragi.role = \"validator\"",
            path.display()
        );
        validate_prepared_network_projection(&config, path)?;
        ensure!(
            config.genesis.public_key == validated.public_key,
            "prepared validator config {} has a different genesis verifier key",
            path.display()
        );
        ensure!(
            config.genesis.expected_hash == validated.expected_hash,
            "prepared validator config {} has genesis hash {}, expected {}",
            path.display(),
            config.genesis.expected_hash,
            validated.expected_hash
        );
        let configured_genesis = config.genesis.file.as_ref().ok_or_else(|| {
            eyre!(
                "prepared validator config {} does not select a signed genesis body",
                path.display()
            )
        })?;
        let configured_genesis = fs::canonicalize(configured_genesis.resolve_relative_path())
            .wrap_err_with(|| {
                format!(
                    "resolve signed genesis body selected by prepared validator config {}",
                    path.display()
                )
            })?;
        ensure!(
            configured_genesis == canonical_signed_block,
            "prepared validator config {} selects signed genesis body {}, expected {}",
            path.display(),
            configured_genesis.display(),
            canonical_signed_block.display()
        );
        ensure!(
            config.genesis.manifest_json.is_none(),
            "prepared validator config {} must not select a runtime genesis manifest; the source manifest is admission-only",
            path.display()
        );

        let trusted = config.common.trusted_peers.value();
        ensure!(
            trusted.pops == validated.validator_pops,
            "prepared validator config {} PoP roster differs from signed genesis",
            path.display()
        );
        let trusted_keys = std::iter::once(&trusted.myself)
            .chain(trusted.others.iter())
            .map(|peer| peer.id().public_key().clone())
            .collect::<BTreeSet<_>>();
        ensure!(
            trusted_keys
                == validated
                    .validator_pops
                    .keys()
                    .cloned()
                    .collect::<BTreeSet<_>>(),
            "prepared validator config {} trusted roster differs from signed genesis",
            path.display()
        );

        let key_pair = config.common.key_pair.clone();
        let validator_key = key_pair.public_key().clone();
        ensure!(
            trusted.myself.id().public_key() == &validator_key,
            "prepared validator config {} local identity differs from its trusted-roster self",
            path.display()
        );
        ensure!(
            validator_keys.insert(validator_key.clone()),
            "prepared validator identity {validator_key} is duplicated"
        );
        let pop = validated
            .validator_pops
            .get(&validator_key)
            .cloned()
            .ok_or_else(|| {
                eyre!(
                    "prepared validator config {} identity {validator_key} is absent from signed genesis",
                    path.display()
                )
            })?;
        iroha_crypto::bls_normal_pop_verify(&validator_key, &pop)
            .wrap_err_with(|| format!("verify prepared validator {index} PoP"))?;
        let p2p_port = config.network.address.value().port();
        let api_port = config.torii.address.value().port();
        ensure!(
            p2p_port != 0 && api_port != 0 && p2p_port != api_port,
            "prepared validator config {} must use distinct non-zero P2P and Torii ports",
            path.display()
        );
        for port in [p2p_port, api_port] {
            ensure!(
                exposed_ports.insert(port),
                "prepared validator config {} reuses exposed host port {port}",
                path.display()
            );
        }
        admitted.push(AdmittedPreparedValidator {
            config,
            table,
            key_pair,
            pop,
        });
    }
    ensure!(
        validator_keys
            == validated
                .validator_pops
                .keys()
                .cloned()
                .collect::<BTreeSet<_>>(),
        "prepared validator config identities differ from the signed genesis roster"
    );
    let chain = chain.expect("non-empty prepared bundle has a chain id");
    iroha_core::validate_genesis_block(
        &validated.block,
        &AccountId::new(validated.public_key.clone()),
        &chain,
    )
    .map_err(|error| eyre!("prepared signed genesis failed full validation: {error}"))?;

    let runtime_peers = admitted
        .iter()
        .enumerate()
        .map(|(index, admitted)| PreparedRuntimePeer {
            service_name: format!("irohad{index}"),
            p2p_port: admitted.config.network.address.value().port(),
            api_port: admitted.config.torii.address.value().port(),
            public_key: admitted.key_pair.public_key().clone(),
        })
        .collect::<Vec<_>>();
    let mode = match signed_metadata.mode {
        SumeragiConsensusMode::Permissioned => WireConsensusMode::Permissioned,
        SumeragiConsensusMode::Npos => WireConsensusMode::Npos,
    };
    let cadence = Duration::from_millis(signed_metadata.block_cadence_ms.get());
    let mut shared_v2_fingerprint = None;
    let mut shared_execution_projection = None;
    let mut shared_nexus_projection = None;
    let mut shared_runtime_inputs = None;
    let mut staged_context = None;
    let mut validators = Vec::with_capacity(admitted.len());
    for (index, admitted) in admitted.into_iter().enumerate() {
        let (
            runtime_config_path,
            runtime_config_blake3,
            runtime_files,
            secret_files,
            requires_sora_profile,
            effective_config,
        ) = project_prepared_runtime_config(
            config_dir,
            projection_root,
            index,
            admitted.table,
            &admitted.config,
            &signed_metadata,
            &runtime_peers,
        )?;
        let v2_fingerprint = effective_config
            .sumeragi
            .v2_config(cadence, mode)
            .wrap_err_with(|| format!("derive prepared validator {index} Sumeragi v2 fingerprint"))?
            .fingerprint();
        if let Some(expected) = shared_v2_fingerprint.as_ref() {
            ensure!(
                &v2_fingerprint == expected,
                "prepared validator {index} has a different shared Sumeragi v2 fingerprint"
            );
        } else {
            shared_v2_fingerprint = Some(v2_fingerprint);
        }
        let execution_projection = execution_policy_projection(&effective_config);
        if let Some(expected) = shared_execution_projection {
            ensure!(
                execution_projection == expected,
                "prepared validator {index} has a different deterministic execution policy"
            );
        } else {
            shared_execution_projection = Some(execution_projection);
        }
        let nexus_projection = actual::sumeragi_v2_nexus_amx_context_hash(
            &effective_config.nexus,
            &effective_config.pipeline,
            &[],
            &[],
        );
        if let Some(expected) = shared_nexus_projection {
            ensure!(
                nexus_projection == expected,
                "prepared validator {index} has a different Nexus/AMX policy"
            );
        } else {
            shared_nexus_projection = Some(nexus_projection);
        }
        let runtime_input_fingerprint = runtime_files
            .iter()
            .map(|file| (file.target.clone(), Hash::new(&file.content)))
            .collect::<Vec<_>>();
        if let Some(expected) = shared_runtime_inputs.as_ref() {
            ensure!(
                &runtime_input_fingerprint == expected,
                "prepared validator {index} has different captured runtime policy inputs"
            );
        } else {
            shared_runtime_inputs = Some(runtime_input_fingerprint);
        }
        let (nexus_amx, execution_policy) = if let Some(context) = staged_context {
            context
        } else {
            let context = crate::genesis::staged_signed_sumeragi_v2_context_hashes(
                manifest,
                &validated.block,
                &effective_config,
            )
            .wrap_err_with(|| {
                format!("stage prepared validator {index} effective genesis policy")
            })?;
            staged_context = Some(context);
            context
        };
        ensure!(
            nexus_amx
                == iroha_crypto::Hash::prehashed(
                    signed_metadata.sumeragi_v2.nexus_amx_context_hash
                ),
            "prepared validator {index} effective Nexus/AMX context differs from signed genesis"
        );
        ensure!(
            execution_policy
                == iroha_crypto::Hash::prehashed(signed_metadata.sumeragi_v2.execution_policy_hash),
            "prepared validator {index} effective execution policy differs from signed genesis"
        );
        let peer = &runtime_peers[index];
        validators.push(PreparedValidator {
            name: peer.service_name.clone(),
            p2p_port: peer.p2p_port,
            api_port: peer.api_port,
            key_pair: admitted.key_pair,
            pop: admitted.pop,
            requires_sora_profile,
            build_line,
            runtime_config_path,
            runtime_config_blake3,
            runtime_files,
            secret_files,
        });
    }

    let runtime_signed_block = materialize_container_readable_file(
        projection_root,
        "genesis",
        "genesis.signed.nrt",
        &validated.canonical_wire,
    )?;
    let public_key_record = format!("{}\n", validated.public_key);
    let runtime_public_key = materialize_container_readable_file(
        projection_root,
        "genesis-key",
        crate::localnet::GENESIS_PUBLIC_KEY_FILE,
        public_key_record.as_bytes(),
    )?;
    let expected_hash_record = format!("{}\n", validated.expected_hash);
    let runtime_expected_hash = materialize_container_readable_file(
        projection_root,
        "genesis-hash",
        crate::localnet::GENESIS_EXPECTED_HASH_FILE,
        expected_hash_record.as_bytes(),
    )?;

    Ok(PreparedBundle {
        chain,
        validators,
        signed_block: runtime_signed_block,
        public_key: runtime_public_key,
        expected_hash: runtime_expected_hash,
    })
}

impl<T: Write> RunArgs<T> for Args {
    #[allow(clippy::too_many_lines)]
    fn run(self, writer: &mut BufWriter<T>) -> Outcome {
        // let args: Args = <Args as clap::Parser>::parse();
        let args = self;

        ensure!(
            is_valid_committee_size(usize::from(args.peers.get())),
            "`--peers` ({}) must form an exact Sumeragi v2 `3f + 1` validator committee \
             in the supported range 4..={MAX_VALIDATORS_PER_HEIGHT}",
            args.peers
        );

        if !args.print && !args.user_allows_overwrite()? {
            return Ok(());
        }

        let build_line = build_line_from_env();
        let genesis_path = args.config_dir.join("genesis.json");
        const MAX_GENESIS_MANIFEST_BYTES: u64 = 64 * 1024 * 1024;
        let manifest_raw = read_runtime_file_bounded(
            &genesis_path,
            "genesis manifest",
            MAX_GENESIS_MANIFEST_BYTES,
        )?;
        let manifest: RawGenesisTransaction = norito::json::from_slice(&manifest_raw)
            .wrap_err_with(|| {
                eyre!(
                    "failed to parse genesis manifest at {}",
                    genesis_path.display()
                )
            })?;
        let manifest_mode = manifest.consensus_mode();
        validate_consensus_mode_for_line(build_line, manifest_mode, ConsensusPolicy::Any)?;
        if matches!(manifest_mode, SumeragiConsensusMode::Npos) {
            ensure_npos_parameters(&manifest)?;
        }
        let prepared_build_line = match build_line {
            BuildLine::Iroha2 => PreparedBuildLine::Iroha2,
            BuildLine::Iroha3 => PreparedBuildLine::Iroha3,
        };

        let peer_overrides = match &args.peer_config {
            Some(path) => Some(load_peer_overrides(path)?),
            None => None,
        };

        tui::status("Composing Docker deployment manifest");
        let prepared_artifacts;
        let swarm = if let Some(seed) = args.seed.as_deref() {
            prepared_artifacts = None;
            iroha_swarm::Swarm::deterministic_dev(
                args.peers,
                seed.as_bytes(),
                args.healthcheck,
                &args.image,
                args.build.as_deref(),
                args.no_cache,
                &args.out_file,
                peer_overrides,
            )?
        } else {
            let deployment_dir = args
                .out_file
                .parent()
                .filter(|path| !path.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new("."));
            let projection_root = deployment_dir.join(".kagami-compose");
            let PreparedBundle {
                chain,
                validators,
                signed_block,
                public_key,
                expected_hash,
            } = load_prepared_bundle(
                &args.config_dir,
                &projection_root,
                args.peers,
                &manifest,
                prepared_build_line,
            )?;
            let artifacts = PreparedGenesisArtifacts {
                signed_block: &signed_block,
                public_key: &public_key,
                expected_hash: &expected_hash,
            };
            let swarm = iroha_swarm::Swarm::from_prepared(
                chain,
                validators,
                artifacts,
                args.healthcheck,
                &args.image,
                args.build.as_deref(),
                args.no_cache,
                &args.out_file,
            )?;
            prepared_artifacts = Some((signed_block, public_key, expected_hash));
            swarm
        };
        let schema = swarm.build();

        let mut file;

        let manifest_writer: &mut dyn Write = if args.print {
            writer
        } else {
            use color_eyre::eyre::Context;
            file = std::fs::File::create(&args.out_file)
                .wrap_err("Could not open the target file.")?;
            &mut file
        };

        let banner = if args.no_banner {
            None
        } else {
            let mut lines = vec![
                "Generated by `kagami docker`.".to_owned(),
                "You should not edit this manually.".to_owned(),
            ];
            if let Some(seed) = args.seed.as_ref() {
                lines.push(format!("Seed: {seed}"));
            }
            Some(lines)
        };
        let banner_refs = banner
            .as_ref()
            .map(|lines| lines.iter().map(String::as_str).collect::<Vec<_>>());

        schema.write(
            &mut std::io::BufWriter::new(manifest_writer),
            banner_refs.as_deref(),
        )?;

        if !args.print {
            writeln!(
                writer,
                "compose_path: {}",
                swarm.absolute_target_path().display()
            )?;
            writeln!(writer, "config_dir: {}", args.config_dir.display())?;
            writeln!(writer, "image: {}", args.image)?;
            writeln!(writer, "peers: {}", args.peers)?;
            writeln!(
                writer,
                "consensus_mode: {}",
                crate::localnet::consensus_mode_label(manifest_mode)
            )?;
            if let Some((signed_block, public_key, expected_hash)) = prepared_artifacts.as_ref() {
                writeln!(writer, "genesis_signed: {}", signed_block.display())?;
                writeln!(writer, "genesis_public_key: {}", public_key.display())?;
                writeln!(writer, "genesis_expected_hash: {}", expected_hash.display())?;
            } else {
                writeln!(
                    writer,
                    "genesis_public_key_file_env: IROHA_GENESIS_PUBLIC_KEY_FILE"
                )?;
                writeln!(writer, "genesis_signed_file_env: IROHA_GENESIS_SIGNED_FILE")?;
                writeln!(
                    writer,
                    "genesis_expected_hash_file_env: IROHA_GENESIS_EXPECTED_HASH_FILE"
                )?;
            }
            writeln!(
                writer,
                "next: docker compose -f {} up",
                args.out_file.display()
            )?;
        }
        tui::success("Compose manifest ready");

        Ok(())
    }
}

fn load_peer_overrides(path: &Path) -> color_eyre::Result<Vec<PeerOverride>> {
    ensure!(
        path.exists(),
        "peer configuration {} does not exist",
        path.display()
    );
    ensure!(
        path.is_file(),
        "peer configuration {} is not a file",
        path.display()
    );
    let contents = fs::read_to_string(path)
        .wrap_err_with(|| eyre!("failed to read peer configuration at {}", path.display()))?;
    parse_peer_override_toml(&contents)
        .wrap_err_with(|| eyre!("failed to parse peer configuration at {}", path.display()))
}

fn parse_peer_override_toml(input: &str) -> color_eyre::Result<Vec<PeerOverride>> {
    let value: toml::Value =
        toml::from_str(input).wrap_err("peer configuration is not valid TOML")?;
    let peers = value
        .get("peers")
        .ok_or_else(|| eyre!("peer configuration must define [[peers]] entries"))?
        .as_array()
        .ok_or_else(|| eyre!("`peers` must be an array of tables"))?;

    ensure!(
        !peers.is_empty(),
        "peer configuration must list at least one peer"
    );

    peers
        .iter()
        .map(|entry| -> color_eyre::Result<PeerOverride> {
            let table = entry
                .as_table()
                .ok_or_else(|| eyre!("each [[peers]] entry must be a table"))?;
            let name = table
                .get("name")
                .and_then(toml::Value::as_str)
                .ok_or_else(|| eyre!("peer entry is missing `name`"))?;
            let p2p_port = parse_port(table, "p2p_port")?;
            let api_port = parse_port(table, "api_port")?;
            Ok(PeerOverride {
                name: name.to_owned(),
                p2p_port,
                api_port,
            })
        })
        .collect()
}

fn parse_port(table: &toml::Table, field: &str) -> color_eyre::Result<u16> {
    let raw = table
        .get(field)
        .ok_or_else(|| eyre!("peer entry is missing `{field}`"))?;
    let value = raw
        .as_integer()
        .ok_or_else(|| eyre!("`{field}` must be an integer"))?;
    let port = u16::try_from(value).map_err(|_| eyre!("`{field}` must fit into a u16"))?;
    Ok(port)
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        io::{BufWriter, Write},
        num::{NonZeroU16, NonZeroUsize},
        path::{Path, PathBuf},
    };

    use iroha_crypto::{Algorithm, Hash, KeyPair};
    use iroha_data_model::{
        ChainId,
        parameter::{
            Parameter,
            system::{SumeragiConsensusMode, SumeragiNposParameters},
        },
    };
    use iroha_genesis::GenesisBuilder;
    use iroha_swarm::PreparedBuildLine;
    use iroha_version::BuildLine;

    use super::{
        Args, load_peer_overrides, load_prepared_bundle, parse_peer_override_toml,
        parse_prepared_peer_config, signed_genesis_consensus_metadata, validate_prepared_genesis,
        validate_runtime_projection_policy,
    };
    use crate::{RunArgs, localnet::LocalnetOptions};

    fn generate_prepared_bundle(root: &Path) -> PathBuf {
        let bundle = root.join("prepared-bundle");
        let options = LocalnetOptions {
            build_line: BuildLine::Iroha3,
            sora_profile: None,
            perf_profile: None,
            peers: NonZeroU16::new(4).expect("non-zero"),
            seed: Some("kagami-compose-prepared-bundle".to_owned()),
            bind_host: "127.0.0.1".to_owned(),
            public_host: "127.0.0.1".to_owned(),
            base_api_port: 19_080,
            base_p2p_port: 23_337,
            out_dir: bundle.clone(),
            extra_accounts: 0,
            assets: Vec::new(),
            block_cadence_ms: None,
            consensus_mode: SumeragiConsensusMode::Npos,
        };
        crate::localnet::generate_localnet(&options, &mut BufWriter::new(Vec::new()))
            .expect("generate authoritative prepared localnet bundle");
        bundle
    }

    fn load_test_prepared_bundle(
        config_dir: &Path,
        projection_root: &Path,
        count: NonZeroU16,
    ) -> color_eyre::Result<super::PreparedBundle> {
        let manifest =
            iroha_genesis::RawGenesisTransaction::from_path(config_dir.join("genesis.json"))?;
        load_prepared_bundle(
            config_dir,
            projection_root,
            count,
            &manifest,
            PreparedBuildLine::Iroha3,
        )
    }

    #[test]
    fn run_succeeds_without_banner() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let config_dir = temp_dir.path().join("cfg");
        fs::create_dir_all(&config_dir).expect("create config dir");
        write_minimal_genesis(&config_dir.join("genesis.json"));
        let args = Args {
            peers: NonZeroU16::new(4).expect("non-zero"),
            seed: Some("swarm-no-banner-dev".to_owned()),
            healthcheck: false,
            config_dir,
            peer_config: None,
            image: "hyperledger/iroha:dev".to_owned(),
            build: None,
            no_cache: false,
            out_file: temp_dir.path().join("docker-compose.yml"),
            print: true,
            force: false,
            no_banner: true,
        };

        let mut buffer = Vec::new();
        let mut writer = BufWriter::new(&mut buffer);
        args.run(&mut writer)
            .expect("`Args::run` should succeed without banner");
        writer.flush().expect("flush buffer");
        drop(writer);

        let output = String::from_utf8(buffer).expect("output should be UTF-8");
        assert!(!output.contains("Generated by `kagami docker`."));
        assert!(!output.contains("Seed:"));
    }

    #[test]
    fn file_output_reports_required_runtime_genesis_artifacts() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let config_dir = temp_dir.path().join("cfg");
        fs::create_dir_all(&config_dir).expect("create config dir");
        write_minimal_genesis(&config_dir.join("genesis.json"));
        let compose_path = temp_dir.path().join("docker-compose.yml");
        let args = Args {
            peers: NonZeroU16::new(4).expect("non-zero"),
            seed: Some("swarm-artifact-summary-dev".to_owned()),
            healthcheck: false,
            config_dir,
            peer_config: None,
            image: "hyperledger/iroha:dev".to_owned(),
            build: None,
            no_cache: false,
            out_file: compose_path.clone(),
            print: false,
            force: false,
            no_banner: true,
        };

        let mut writer = BufWriter::new(Vec::new());
        args.run(&mut writer).expect("write Compose manifest");
        let output = String::from_utf8(writer.into_inner().expect("flush summary"))
            .expect("summary is UTF-8");

        assert!(compose_path.is_file());
        assert!(output.contains("genesis_public_key_file_env: IROHA_GENESIS_PUBLIC_KEY_FILE"));
        assert!(output.contains("genesis_signed_file_env: IROHA_GENESIS_SIGNED_FILE"));
        assert!(
            output.contains("genesis_expected_hash_file_env: IROHA_GENESIS_EXPECTED_HASH_FILE")
        );
        assert!(!output.contains("IROHA_GENESIS_PRIVATE_KEY_FILE"));
        assert!(output.contains("next: docker compose"));
    }

    #[test]
    fn prepared_bundle_renders_exact_read_only_runtime_inputs() {
        let temp_dir = tempfile::tempdir().expect("prepared bundle temp dir");
        let config_dir = generate_prepared_bundle(temp_dir.path());
        let deployment_dir = temp_dir.path().join("deployment");
        fs::create_dir_all(&deployment_dir).expect("create deployment directory");
        let args = Args {
            peers: NonZeroU16::new(4).expect("non-zero"),
            seed: None,
            healthcheck: false,
            config_dir: config_dir.clone(),
            peer_config: None,
            image: "hyperledger/iroha:dev".to_owned(),
            build: None,
            no_cache: false,
            out_file: deployment_dir.join("docker-compose.yml"),
            print: true,
            force: false,
            no_banner: true,
        };

        let mut output = Vec::new();
        let mut writer = BufWriter::new(&mut output);
        args.run(&mut writer)
            .expect("render prepared validator bundle");
        writer.flush().expect("flush prepared Compose output");
        drop(writer);
        let output = String::from_utf8(output).expect("Compose output is UTF-8");
        assert_eq!(output.matches("--config /config/peer.toml").count(), 4);
        assert_eq!(output.matches("exec env -i").count(), 4);
        assert_eq!(output.matches("IROHA_BUILD_LINE=iroha3").count(), 4);
        assert_eq!(output.matches("--config-blake3 ").count(), 4);
        assert_eq!(output.matches("read_only: true").count(), 4);
        assert_eq!(output.matches("target: /config/peer.toml").count(), 4);
        assert_eq!(
            output
                .matches("target: /run/secrets/iroha_runtime_")
                .count(),
            4
        );
        assert_eq!(output.matches("target: /storage").count(), 4);
        assert!(output.contains(".kagami-compose"));
        for artifact in [
            "genesis.signed.nrt",
            "genesis.public_key",
            "genesis.expected_hash",
            "peer0.toml",
            "peer1.toml",
            "peer2.toml",
            "peer3.toml",
        ] {
            assert!(output.contains(artifact), "missing {artifact}: {output}");
        }
        assert!(!output.contains("prepared-bundle/peer0.toml"));
        assert!(!output.contains("streaming_private_key"));
        assert!(!output.contains("environment:"));
        assert!(!output.contains("PRIVATE_KEY:"));

        let projection_root = deployment_dir.join(".kagami-compose");
        let peer0_directory = fs::read_dir(&projection_root)
            .expect("read projection root")
            .map(|entry| entry.expect("read projection entry"))
            .find(|entry| entry.path().join("peer0.toml").is_file())
            .expect("peer0 projection directory")
            .path();
        let peer0_projection = peer0_directory.join("peer0.toml");
        let projected_raw =
            fs::read_to_string(&peer0_projection).expect("read peer0 config projection");
        let projected = projected_raw
            .parse::<toml::Table>()
            .expect("parse peer0 config projection");
        let table_at = |name: &str| {
            projected
                .get(name)
                .and_then(toml::Value::as_table)
                .unwrap_or_else(|| panic!("projection has [{name}]"))
        };
        assert_eq!(
            table_at("genesis")
                .get("file")
                .and_then(toml::Value::as_str),
            Some("/genesis/genesis.signed.nrt")
        );
        assert!(!table_at("genesis").contains_key("manifest_json"));
        assert_eq!(
            table_at("kura")
                .get("store_dir")
                .and_then(toml::Value::as_str),
            Some("/storage/kura")
        );
        assert_eq!(
            table_at("snapshot")
                .get("store_dir")
                .and_then(toml::Value::as_str),
            Some("/storage/snapshot")
        );
        let network = table_at("network");
        assert!(
            network
                .get("address")
                .and_then(toml::Value::as_str)
                .is_some_and(|address| address.contains("0.0.0.0:23337"))
        );
        assert!(
            network
                .get("public_address")
                .and_then(toml::Value::as_str)
                .is_some_and(|address| address.contains("irohad0:23337"))
        );
        assert!(
            table_at("torii")
                .get("address")
                .and_then(toml::Value::as_str)
                .is_some_and(|address| address.contains("0.0.0.0:19080"))
        );
        assert_eq!(
            table_at("torii")
                .get("da_ingest")
                .and_then(toml::Value::as_table)
                .and_then(|da| da.get("manifest_store_dir"))
                .and_then(toml::Value::as_str),
            Some("/storage/torii/da-manifests")
        );
        assert_eq!(
            table_at("sorafs")
                .get("storage")
                .and_then(toml::Value::as_table)
                .and_then(|storage| storage.get("data_dir"))
                .and_then(toml::Value::as_str),
            Some("/storage/sorafs")
        );
        let trusted_peers = projected
            .get("trusted_peers")
            .and_then(toml::Value::as_array)
            .expect("projected trusted peers");
        assert_eq!(trusted_peers.len(), 4);
        for index in 0..4 {
            assert!(
                trusted_peers.iter().any(|peer| {
                    peer.as_str()
                        .is_some_and(|peer| peer.contains(&format!("@addr:irohad{index}:")))
                }),
                "projected trusted roster lacks irohad{index}"
            );
        }
        let streaming = table_at("streaming");
        assert_eq!(
            streaming
                .get("session_store_dir")
                .and_then(toml::Value::as_str),
            Some("/storage/streaming")
        );
        assert_eq!(
            streaming
                .get("codec")
                .and_then(toml::Value::as_table)
                .and_then(|codec| codec.get("rans_tables_path"))
                .and_then(toml::Value::as_str),
            Some("/config/runtime/rans_seed0.toml")
        );
        assert_eq!(
            table_at("torii")
                .get("account_onboarding")
                .and_then(toml::Value::as_table)
                .and_then(|onboarding| onboarding.get("private_key_file"))
                .and_then(toml::Value::as_str),
            Some("/run/secrets/iroha_peer0_onboarding_private_key")
        );
        assert!(
            output.contains("target: /run/secrets/iroha_peer0_onboarding_private_key"),
            "prepared onboarding signer must be mounted as a Compose secret"
        );
        assert_eq!(
            table_at("torii")
                .get("faucet")
                .and_then(toml::Value::as_table)
                .and_then(|faucet| faucet.get("private_key_file"))
                .and_then(toml::Value::as_str),
            Some("/run/secrets/iroha_peer0_faucet_private_key")
        );
        assert!(
            output.contains("target: /run/secrets/iroha_peer0_faucet_private_key"),
            "prepared faucet signer must be mounted as a Compose secret"
        );
        let original_peer0 =
            fs::read_to_string(config_dir.join("peer0.toml")).expect("read source peer0 config");
        let original_peer0 = original_peer0
            .parse::<toml::Table>()
            .expect("parse source peer0 config");
        for private_key in [
            original_peer0
                .get("private_key")
                .and_then(toml::Value::as_str)
                .expect("source validator private key"),
            original_peer0
                .get("streaming")
                .and_then(toml::Value::as_table)
                .and_then(|streaming| streaming.get("identity_private_key"))
                .and_then(toml::Value::as_str)
                .expect("source streaming private key"),
        ] {
            assert!(
                !output.contains(private_key),
                "private key leaked into Compose YAML"
            );
            assert!(
                projected_raw.contains(private_key),
                "file-backed config secret lost an admitted private key"
            );
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;

            assert_eq!(
                fs::metadata(&projection_root)
                    .expect("projection-root metadata")
                    .permissions()
                    .mode()
                    & 0o777,
                0o700
            );
            assert_eq!(
                fs::metadata(&peer0_directory)
                    .expect("peer projection metadata")
                    .permissions()
                    .mode()
                    & 0o777,
                0o700
            );
            assert_eq!(
                fs::metadata(&peer0_projection)
                    .expect("projected config metadata")
                    .permissions()
                    .mode()
                    & 0o777,
                0o400
            );
        }
        for forbidden in [
            "${IROHA_GENESIS_",
            "genesis.private_key",
            "IROHA_GENESIS_PRIVATE_KEY_FILE",
            "kagami genesis sign",
            "depends_on:",
        ] {
            assert!(
                !output.contains(forbidden),
                "unexpected {forbidden}: {output}"
            );
        }
    }

    #[test]
    fn prepared_bundle_allows_loopback_but_rejects_bridge_trust_cidrs() {
        let temp_dir = tempfile::tempdir().expect("prepared CIDR temp dir");
        let config_dir = generate_prepared_bundle(temp_dir.path());
        let count = NonZeroU16::new(4).expect("non-zero");
        load_test_prepared_bundle(
            &config_dir,
            &temp_dir.path().join("loopback-projection"),
            count,
        )
        .expect("generated loopback-only Torii CIDRs remain local inside each container");

        let peer0_path = config_dir.join("peer0.toml");
        let mut peer0 = fs::read_to_string(&peer0_path)
            .expect("read peer0 fixture")
            .parse::<toml::Table>()
            .expect("parse peer0 fixture");
        peer0
            .get_mut("torii")
            .and_then(toml::Value::as_table_mut)
            .expect("peer0 Torii table")
            .insert(
                "internal_api_trusted_cidrs".to_owned(),
                toml::Value::Array(vec![toml::Value::String("172.16.0.0/12".to_owned())]),
            );
        fs::write(
            &peer0_path,
            toml::to_string_pretty(&peer0).expect("serialize mutated peer0 fixture"),
        )
        .expect("write mutated peer0 fixture");

        let error = load_test_prepared_bundle(
            &config_dir,
            &temp_dir.path().join("bridge-projection"),
            count,
        )
        .expect_err("bridge CIDR trust must not silently change behind Compose NAT");
        assert!(
            error.to_string().contains("non-loopback source CIDRs"),
            "unexpected bridge-CIDR rejection: {error:#}"
        );
    }

    #[test]
    fn prepared_bundle_rejects_existing_default_projected_state() {
        let temp_dir = tempfile::tempdir().expect("prepared state temp dir");
        let config_dir = generate_prepared_bundle(temp_dir.path());
        let revocations = config_dir
            .join("storage")
            .join("soranet")
            .join("ticket_revocations.norito");
        fs::create_dir_all(revocations.parent().expect("revocation-store parent"))
            .expect("create default revocation-store parent");
        fs::write(&revocations, b"existing state").expect("write existing revocation state");

        let error = load_test_prepared_bundle(
            &config_dir,
            &temp_dir.path().join("state-projection"),
            NonZeroU16::new(4).expect("non-zero"),
        )
        .expect_err("prepared projection must reject source state it would replace");
        assert!(
            error
                .to_string()
                .contains("SoraNet ticket-revocation store"),
            "unexpected state rejection: {error:#}"
        );
    }

    #[test]
    fn prepared_bundle_rejects_signer_hash_roster_and_pop_mismatches() {
        let temp_dir = tempfile::tempdir().expect("prepared mismatch temp dir");
        let config_dir = generate_prepared_bundle(temp_dir.path());
        let projection_root = temp_dir.path().join("mismatch-projection");
        let count = NonZeroU16::new(4).expect("non-zero");
        load_test_prepared_bundle(&config_dir, &projection_root, count)
            .expect("baseline prepared bundle validates");

        let signed_path = config_dir.join("genesis.signed.nrt");
        let public_path = config_dir.join(crate::localnet::GENESIS_PUBLIC_KEY_FILE);
        let hash_path = config_dir.join(crate::localnet::GENESIS_EXPECTED_HASH_FILE);
        let validated = validate_prepared_genesis(&signed_path, &public_path, &hash_path)
            .expect("validate prepared genesis fixture");
        let metadata = signed_genesis_consensus_metadata(&validated.block)
            .expect("prepared fixture has consensus metadata");
        let parsed = parse_prepared_peer_config(&config_dir.join("peer0.toml"))
            .expect("parse prepared peer0 fixture");
        let mut drifted = parsed.actual.clone();
        drifted.sumeragi.block.max_transactions = NonZeroUsize::new(
            drifted
                .sumeragi
                .block
                .max_transactions
                .get()
                .checked_add(1)
                .expect("fixture block limit can increase"),
        )
        .expect("incremented block limit is non-zero");
        let projection_error =
            validate_runtime_projection_policy(&parsed.actual, &drifted, &metadata)
                .expect_err("Sumeragi policy drift must reject a runtime projection");
        assert!(
            projection_error
                .to_string()
                .contains("safety/liveness fingerprint"),
            "unexpected projection-policy mismatch: {projection_error:#}"
        );

        let original_signed = fs::read(&signed_path).expect("read signed genesis fixture");
        let mut noncanonical_signed = original_signed.clone();
        noncanonical_signed.push(0);
        fs::write(&signed_path, noncanonical_signed).expect("write non-canonical signed fixture");
        let canonical_error = load_test_prepared_bundle(&config_dir, &projection_root, count)
            .expect_err("non-canonical prepared signed body must fail");
        let canonical_error = format!("{canonical_error:#}").to_ascii_lowercase();
        assert!(
            canonical_error.contains("canonical") || canonical_error.contains("decode"),
            "unexpected canonical-wire mismatch: {canonical_error}"
        );
        fs::write(&signed_path, original_signed).expect("restore signed genesis fixture");

        let original_public = fs::read(&public_path).expect("read public-key fixture");
        let other_signer = KeyPair::try_from_seed(
            b"different-prepared-genesis-signer".to_vec(),
            Algorithm::Ed25519,
        )
        .expect("derive alternate signer");
        fs::write(&public_path, format!("{}\n", other_signer.public_key()))
            .expect("write mismatched signer");
        let signer_error = load_test_prepared_bundle(&config_dir, &projection_root, count)
            .expect_err("mismatched prepared signer must fail");
        assert!(
            signer_error.to_string().contains("signer"),
            "unexpected signer mismatch: {signer_error:#}"
        );
        fs::write(&public_path, original_public).expect("restore public-key fixture");

        let original_hash = fs::read(&hash_path).expect("read hash fixture");
        fs::write(
            &hash_path,
            format!("{}\n", Hash::new(b"different prepared genesis body")),
        )
        .expect("write mismatched hash");
        let hash_error = load_test_prepared_bundle(&config_dir, &projection_root, count)
            .expect_err("mismatched prepared hash must fail");
        assert!(
            hash_error.to_string().contains("body hashes"),
            "unexpected hash mismatch: {hash_error:#}"
        );
        fs::write(&hash_path, original_hash).expect("restore hash fixture");

        let peer3_path = config_dir.join("peer3.toml");
        let peer3 = fs::read(&peer3_path).expect("read peer3 fixture");
        fs::remove_file(&peer3_path).expect("remove peer3 fixture");
        let roster_error = load_test_prepared_bundle(&config_dir, &projection_root, count)
            .expect_err("incomplete prepared roster must fail");
        assert!(
            roster_error.to_string().contains("roster files"),
            "unexpected roster mismatch: {roster_error:#}"
        );
        fs::write(&peer3_path, peer3).expect("restore peer3 fixture");

        let peer0_path = config_dir.join("peer0.toml");
        let original_peer0 = fs::read_to_string(&peer0_path).expect("read peer0 fixture");
        let marker = "pop_hex = \"";
        let pop_start = original_peer0.find(marker).expect("peer0 has PoP") + marker.len();
        let pop_end = original_peer0[pop_start..]
            .find('"')
            .map(|offset| pop_start + offset)
            .expect("peer0 PoP is quoted");
        let mut invalid_peer0 = original_peer0.clone();
        let last = pop_end.checked_sub(1).expect("PoP is non-empty");
        let replacement = if invalid_peer0.as_bytes()[last] == b'0' {
            "1"
        } else {
            "0"
        };
        invalid_peer0.replace_range(last..pop_end, replacement);
        fs::write(&peer0_path, invalid_peer0).expect("write mismatched PoP");
        let pop_error = load_test_prepared_bundle(&config_dir, &projection_root, count)
            .expect_err("mismatched prepared PoP must fail");
        assert!(
            format!("{pop_error:#}")
                .to_ascii_lowercase()
                .contains("pop"),
            "unexpected PoP mismatch: {pop_error:#}"
        );
        fs::write(&peer0_path, &original_peer0).expect("restore peer0 fixture");

        let alternate_signed_path = config_dir.join("alternate-genesis.signed.nrt");
        fs::copy(&signed_path, &alternate_signed_path)
            .expect("write alternate signed genesis fixture");
        let canonical_signed_path =
            fs::canonicalize(&signed_path).expect("canonicalize signed genesis fixture");
        let canonical_alternate_signed_path = fs::canonicalize(&alternate_signed_path)
            .expect("canonicalize alternate signed genesis fixture");
        let alternate_peer0 = original_peer0.replacen(
            &canonical_signed_path.display().to_string(),
            &canonical_alternate_signed_path.display().to_string(),
            1,
        );
        assert_ne!(
            alternate_peer0, original_peer0,
            "peer config must contain the prepared signed-genesis path"
        );
        fs::write(&peer0_path, alternate_peer0).expect("select alternate signed body");
        let selected_body_error = load_test_prepared_bundle(&config_dir, &projection_root, count)
            .expect_err("prepared config selecting another signed body must fail");
        assert!(
            selected_body_error
                .to_string()
                .contains("selects signed genesis body"),
            "unexpected configured-body mismatch: {selected_body_error:#}"
        );
        fs::write(&peer0_path, &original_peer0).expect("restore peer0 fixture");

        let runtime_manifest_peer0 = original_peer0.replacen(
            "[genesis]\n",
            "[genesis]\nmanifest_json = \"genesis.json\"\n",
            1,
        );
        assert_ne!(
            runtime_manifest_peer0, original_peer0,
            "peer config must contain a genesis table"
        );
        fs::write(&peer0_path, runtime_manifest_peer0).expect("select a runtime source manifest");
        let runtime_manifest_error =
            load_test_prepared_bundle(&config_dir, &projection_root, count)
                .expect_err("prepared config selecting a runtime source manifest must fail");
        assert!(
            runtime_manifest_error
                .to_string()
                .contains("source manifest is admission-only"),
            "unexpected runtime-manifest mismatch: {runtime_manifest_error:#}"
        );
        fs::write(&peer0_path, &original_peer0).expect("restore peer0 fixture");

        let manifest_path = config_dir.join("genesis.json");
        let original_manifest = fs::read(&manifest_path).expect("read genesis manifest fixture");
        write_minimal_genesis(&manifest_path);
        let manifest_error = load_test_prepared_bundle(&config_dir, &projection_root, count)
            .expect_err("a manifest unrelated to the signed body must fail");
        let manifest_error = format!("{manifest_error:#}").to_ascii_lowercase();
        assert!(
            manifest_error.contains("manifest")
                || manifest_error.contains("transaction count")
                || manifest_error.contains("consensus mode"),
            "unexpected manifest-binding mismatch: {manifest_error}"
        );
        fs::write(&manifest_path, original_manifest).expect("restore genesis manifest fixture");
    }

    #[test]
    fn load_peer_overrides_reads_valid_file() -> color_eyre::Result<()> {
        let file = tempfile::NamedTempFile::new()?;
        fs::write(
            file.path(),
            r#"
[[peers]]
name = "alpha"
p2p_port = 2000
api_port = 9000

[[peers]]
name = "beta"
p2p_port = 2001
api_port = 9001
"#,
        )?;

        let overrides = load_peer_overrides(file.path())?;
        assert_eq!(overrides.len(), 2);
        assert_eq!(overrides[0].name, "alpha");
        assert_eq!(overrides[0].p2p_port, 2000);
        assert_eq!(overrides[0].api_port, 9000);
        assert_eq!(overrides[1].name, "beta");
        assert_eq!(overrides[1].p2p_port, 2001);
        assert_eq!(overrides[1].api_port, 9001);
        Ok(())
    }

    #[test]
    fn parse_peer_override_toml_rejects_empty_peer_list() {
        let err = parse_peer_override_toml("peers = []").expect_err("should fail on empty peers");
        assert!(
            err.to_string().contains("must list at least one peer"),
            "unexpected error message: {err}"
        );
    }

    #[test]
    fn parse_peer_override_toml_rejects_out_of_range_ports() {
        let err = parse_peer_override_toml(
            r#"
[[peers]]
name = "alpha"
p2p_port = 70000
api_port = 9000
"#,
        )
        .expect_err("port 70000 should be rejected");
        assert!(
            err.to_string().contains("must fit into a u16"),
            "unexpected error message: {err}"
        );
    }

    #[test]
    fn swarm_uses_manifest_consensus_without_environment_overrides() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let config_dir = temp_dir.path().join("cfg");
        fs::create_dir_all(&config_dir).expect("create config dir");
        write_npos_genesis(&config_dir.join("genesis.json"));
        let args = Args {
            peers: NonZeroU16::new(4).expect("non-zero"),
            seed: Some("swarm-npos-overrides".to_owned()),
            healthcheck: false,
            config_dir: config_dir.clone(),
            peer_config: None,
            image: "hyperledger/iroha:dev".to_owned(),
            build: None,
            no_cache: false,
            out_file: temp_dir.path().join("docker-compose.yml"),
            print: true,
            force: false,
            no_banner: true,
        };

        let mut buffer = Vec::new();
        let mut writer = BufWriter::new(&mut buffer);
        args.run(&mut writer)
            .expect("`Args::run` should render compose yaml");
        writer.flush().expect("flush buffer");
        drop(writer);

        let output = String::from_utf8(buffer).expect("output should be UTF-8");
        for retired_override in [
            "GENESIS_CONSENSUS_MODE:",
            "GENESIS_NEXT_CONSENSUS_MODE:",
            "GENESIS_MODE_ACTIVATION_HEIGHT:",
        ] {
            assert!(
                !output.contains(retired_override),
                "compose output must derive consensus from the manifest, not {retired_override}: {output}"
            );
        }
    }

    #[test]
    fn npos_swarm_requires_genesis_with_npos_parameters() {
        let temp_dir = tempfile::tempdir().expect("tmp dir");
        let config_dir = temp_dir.path().join("cfg");
        fs::create_dir_all(&config_dir).expect("create config dir");
        write_npos_genesis_without_parameters(&config_dir.join("genesis.json"));

        let args = Args {
            peers: NonZeroU16::new(4).expect("non-zero"),
            seed: Some("swarm-invalid-npos-dev".to_owned()),
            healthcheck: false,
            config_dir: config_dir.clone(),
            peer_config: None,
            image: "hyperledger/iroha:dev".to_owned(),
            build: None,
            no_cache: false,
            out_file: temp_dir.path().join("docker-compose.yml"),
            print: true,
            force: true,
            no_banner: true,
        };

        let mut writer = BufWriter::new(Vec::new());
        let err = args
            .run(&mut writer)
            .expect_err("missing NPoS parameters should fail compose generation");
        assert!(
            err.to_string().contains("sumeragi_npos_parameters"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn npos_swarm_succeeds_with_npos_genesis() {
        let temp_dir = tempfile::tempdir().expect("tmp dir");
        let config_dir = temp_dir.path().join("cfg");
        fs::create_dir_all(&config_dir).expect("create config dir");
        write_npos_genesis(&config_dir.join("genesis.json"));

        let args = Args {
            peers: NonZeroU16::new(7).expect("non-zero"),
            seed: Some("npos-ok".to_owned()),
            healthcheck: false,
            config_dir: config_dir.clone(),
            peer_config: None,
            image: "hyperledger/iroha:dev".to_owned(),
            build: None,
            no_cache: false,
            out_file: temp_dir.path().join("docker-compose.yml"),
            print: true,
            force: true,
            no_banner: true,
        };

        let mut writer = BufWriter::new(Vec::new());
        args.run(&mut writer)
            .expect("npos genesis with parameters should pass");
    }

    #[test]
    fn run_rejects_non_committee_peer_counts() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let config_dir = temp_dir.path().join("cfg");
        fs::create_dir_all(&config_dir).expect("create config dir");
        write_minimal_genesis(&config_dir.join("genesis.json"));

        for count in [1_u16, 2, 3, 5, 32] {
            let args = Args {
                peers: NonZeroU16::new(count).expect("fixture count is non-zero"),
                seed: Some("swarm-invalid-committee-dev".to_owned()),
                healthcheck: false,
                config_dir: config_dir.clone(),
                peer_config: None,
                image: "hyperledger/iroha:dev".to_owned(),
                build: None,
                no_cache: false,
                out_file: temp_dir.path().join(format!("docker-compose-{count}.yml")),
                print: true,
                force: false,
                no_banner: true,
            };

            let mut writer = BufWriter::new(Vec::new());
            let error = args
                .run(&mut writer)
                .expect_err("non-committee peer count must fail");
            assert!(
                error.to_string().contains("exact Sumeragi v2 `3f + 1`"),
                "unexpected error for {count} peers: {error}"
            );
        }
    }

    fn write_minimal_genesis(path: &Path) {
        let manifest =
            GenesisBuilder::new_without_executor(ChainId::from("test-chain"), PathBuf::from("."))
                .build_raw()
                .with_consensus_mode(
                    iroha_data_model::parameter::system::SumeragiConsensusMode::Permissioned,
                );
        let genesis_json = norito::json::to_json_pretty(&manifest).expect("serialize genesis");
        fs::write(path, genesis_json).expect("write minimal genesis");
    }

    fn write_npos_genesis_without_parameters(path: &Path) {
        let manifest = GenesisBuilder::new_without_executor(
            ChainId::from("npos-without-parameters"),
            PathBuf::from("."),
        )
        .build_raw()
        .with_consensus_mode(iroha_data_model::parameter::system::SumeragiConsensusMode::Npos);
        let json = norito::json::to_json_pretty(&manifest).expect("serialize genesis");
        fs::write(path, json).expect("write NPoS genesis without parameters");
    }

    fn write_npos_genesis(path: &Path) {
        let chain = ChainId::from("npos-swarm");
        let manifest = GenesisBuilder::new_without_executor(chain, PathBuf::from("."))
            .append_parameter(Parameter::Custom(
                SumeragiNposParameters::default().into_custom_parameter(),
            ))
            .build_raw()
            .with_consensus_mode(iroha_data_model::parameter::system::SumeragiConsensusMode::Npos);
        let json = norito::json::to_json_pretty(&manifest).expect("serialize genesis");
        fs::write(path, json).expect("write npos genesis");
    }
}
