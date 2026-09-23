//! Fresh native localnet inputs for the production-custody beacon contract.

use color_eyre::eyre::{Result, WrapErr as _, ensure, eyre};
use iroha_core::{
    beacon::global_threshold_beacon_roster_hash_v1, sumeragi::signed_genesis_voting_peers,
};
use iroha_crypto::{ExposedPrivateKey, KeyPair, PublicKey};
use iroha_data_model::{
    NetworkId,
    account::{Account, AccountId},
    asset::{AssetDefinitionId, AssetId},
    consensus::{GLOBAL_THRESHOLD_BEACON_VERSION_V1, GlobalThresholdBeaconDkgSessionV1},
    isi::{Mint, Register},
    parameter::system::{Parameters, SumeragiNposParameters},
    role::Role,
};
use iroha_executor_data_model::permission::account::{
    AccountAliasPermissionScope, CanDelegateAccountAliasResolution,
};
use iroha_genesis::{GenesisBlock, RawGenesisTransaction, validate_prepared_genesis_bundle};
use iroha_model_base::peer::PeerId;
use norito::{derive::JsonSerialize, json};
use rand::{TryRngCore as _, rngs::OsRng};
use std::{
    fs,
    io::Write as _,
    num::NonZeroU64,
    os::{
        fd::AsRawFd as _,
        unix::fs::{OpenOptionsExt as _, PermissionsExt as _},
    },
    path::{Path, PathBuf},
    process::Stdio,
    str::FromStr as _,
};
use tokio::{
    process::Command,
    time::{Instant, timeout_at},
};

pub(super) struct Prepared {
    pub directory: PathBuf,
    pub genesis_directory: PathBuf,
    pub request: PathBuf,
    pub roster: Vec<PeerId>,
    pub network_id: NetworkId,
    pub genesis_public_key: PublicKey,
    pub routed_client: PathBuf,
}

// One real funded account gives the retained explicit-route contract an
// independently configured public dataspace, without a second network ceremony.
fn routed_account_and_snapshot_config(directory: &Path, manifest: &Path) -> Result<PathBuf> {
    let _profile = iroha_data_model::account::address::ChainDiscriminantGuard::enter(369);
    let key = KeyPair::try_random()?;
    let account = AccountId::new(key.public_key().clone());
    let authority = account.to_i105_for_discriminant(369)?;
    let private = directory.join("runtime/routed-client.private_key");
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&private)?;
    writeln!(file, "{}", ExposedPrivateKey(key.private_key().clone()))?;
    file.sync_all()?;
    let client_path = directory.join("routed-client.toml");
    let mut client: toml::Table = fs::read_to_string(directory.join("client.toml"))?.parse()?;
    let client_account = client
        .get_mut("account")
        .and_then(toml::Value::as_table_mut)
        .ok_or_else(|| eyre!("generated client has no account"))?;
    client_account.remove("private_key");
    client_account.insert(
        "public_key".into(),
        toml::Value::String(key.public_key().to_string()),
    );
    client_account.insert(
        "private_key_file".into(),
        toml::Value::String(private.to_string_lossy().into_owned()),
    );
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&client_path)?;
    file.write_all(toml::to_string(&client)?.as_bytes())?;
    file.sync_all()?;
    let mut fee_asset = None;
    for index in 0..4 {
        let path = directory.join(format!("peer{index}.toml"));
        let mut config: toml::Table = fs::read_to_string(&path)?.parse()?;
        let nexus = config
            .get_mut("nexus")
            .and_then(toml::Value::as_table_mut)
            .ok_or_else(|| eyre!("missing Nexus fixture config"))?;
        let asset = nexus
            .get("fees")
            .and_then(|v| v.get("fee_asset_id"))
            .and_then(toml::Value::as_str)
            .ok_or_else(|| eyre!("missing explicit fixture fee asset"))?;
        let asset = AssetDefinitionId::parse_address_literal(asset)?;
        if let Some(expected) = &fee_asset {
            ensure!(expected == &asset, "peer fee assets differ");
        } else {
            fee_asset = Some(asset);
        }
        let rules = nexus
            .get_mut("routing_policy")
            .and_then(|v| v.get_mut("rules"))
            .and_then(toml::Value::as_array_mut)
            .ok_or_else(|| eyre!("missing native routing policy"))?;
        let matcher =
            toml::Table::from_iter([("account".into(), toml::Value::String(authority.clone()))]);
        rules.insert(
            0,
            toml::Value::Table(toml::Table::from_iter([
                ("lane".into(), toml::Value::Integer(3)),
                ("dataspace".into(), toml::Value::String("paynet".into())),
                ("matcher".into(), toml::Value::Table(matcher)),
            ])),
        );
        config.insert(
            "snapshot".into(),
            toml::Value::Table(toml::Table::from_iter([
                ("mode".into(), toml::Value::String("disabled".into())),
                (
                    "store_dir".into(),
                    toml::Value::String(
                        directory
                            .join(format!("state/peer{index}/snapshot"))
                            .to_string_lossy()
                            .into_owned(),
                    ),
                ),
                ("create_every_ms".into(), toml::Value::Integer(1_000)),
            ])),
        );
        let logger = config
            .entry("logger")
            .or_insert_with(|| toml::Value::Table(toml::Table::new()))
            .as_table_mut()
            .ok_or_else(|| eyre!("invalid fixture logger"))?;
        logger.insert("format".into(), toml::Value::String("json".into()));
        logger.insert("level".into(), toml::Value::String("INFO".into()));
        fs::write(path, toml::to_string(&config)?)?;
    }
    let writer = iroha::config::Config::load_file(directory.join("client.toml"))
        .map_err(|error| eyre!("native genesis writer config failed: {error:?}"))?
        .account;
    let delegation: iroha_data_model::permission::Permission = CanDelegateAccountAliasResolution {
        scope: AccountAliasPermissionScope::Dataspace(
            iroha_model_base::topology::DataSpaceId::new(56_005),
        ),
    }
    .into();
    // Resolve native resource paths at their original source before deriving
    // a separate manifest; preserve the localnet raw/signed/identity bundle.
    let raw = RawGenesisTransaction::from_path(directory.join("genesis.json"))?
        .into_builder()
        .next_transaction()
        .append_instruction(Register::account(Account::new(account.clone())))
        .append_instruction(Mint::asset_quantity(
            iroha_primitives::numeric::Quantity::from(25_000_u32),
            AssetId::new(fee_asset.ok_or_else(|| eyre!("no fee asset"))?, account),
        ))
        .append_instruction(Register::role(
            Role::new("runtime_catalog_resolution_delegate".parse()?, writer)
                .add_permission(delegation),
        ))
        .build_raw()?
        .with_consensus_meta();
    fs::write(manifest, json::to_vec(&raw)?)?;
    Ok(client_path)
}

#[derive(JsonSerialize)]
struct Request {
    schema: String,
    dkg_session: GlobalThresholdBeaconDkgSessionV1,
    target_roster: Vec<PeerId>,
    authorization_roster: Vec<PeerId>,
    provider_handles: Vec<String>,
    provider_revision: u64,
}

async fn run(command: &mut Command, evidence: &Path, deadline: Instant, stage: &str) -> Result<()> {
    ensure!(
        Instant::now() < deadline,
        "beacon fixture deadline before {stage}"
    );
    // Native configuration may contain keys. Retain child diagnostics only in
    // the fixture's owner-private external runtime, never in public test output.
    fs::create_dir(evidence)?;
    fs::set_permissions(evidence, fs::Permissions::from_mode(0o700))?;
    let log = |name| {
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(evidence.join(name))
    };
    let stdout = log("stdout")?;
    let stderr = log("stderr")?;
    let mut child = command
        .env_clear()
        .kill_on_drop(true)
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout.try_clone()?))
        .stderr(Stdio::from(stderr.try_clone()?))
        .spawn()?;
    let status = timeout_at(deadline, child.wait()).await;
    stdout.sync_all()?;
    stderr.sync_all()?;
    let status = status.wrap_err_with(|| format!("beacon fixture deadline during {stage}"))??;
    ensure!(
        status.success(),
        "beacon fixture {stage} failed: {status}; private diagnostics: {}",
        evidence.display()
    );
    ensure!(
        Instant::now() < deadline,
        "beacon fixture deadline after {stage}"
    );
    Ok(())
}

/// Change only the existing native NPoS parameter, then recompute native metadata.
fn short_epoch_manifest(path: &Path) -> Result<()> {
    let _profile = iroha_data_model::account::address::ChainDiscriminantGuard::enter(369);
    let raw = RawGenesisTransaction::from_path(path)?;
    raw.effective_parameters()?;
    let mut value = json::value::to_value(&raw)?;
    let transactions = value
        .get_mut("transactions")
        .and_then(json::Value::as_array_mut)
        .ok_or_else(|| eyre!("native manifest has no transactions"))?;
    let mut count = 0;
    for transaction in transactions {
        let Some(parameters_value) = transaction.get_mut("parameters") else {
            continue;
        };
        if parameters_value.is_null() {
            continue;
        }
        count += 1;
        let mut parameters: Parameters = json::value::from_value(parameters_value.clone())?;
        let id = SumeragiNposParameters::parameter_id();
        let mut npos = parameters
            .custom
            .get(&id)
            .and_then(SumeragiNposParameters::from_custom_parameter)
            .ok_or_else(|| eyre!("native localnet omitted valid signed NPoS parameters"))?;
        ensure!(
            npos.max_validators == 4,
            "native fixture roster ceiling is not four"
        );
        // The first real epoch-maintenance operation admits at 8, anchors at 9 and
        // executes at 10. Epoch 11 makes that execution merge itself carry the
        // mandatory pulse; no padding transaction or pulse-only block exists.
        npos.epoch_length_blocks = NonZeroU64::new(11).expect("positive fixture epoch");
        // Retain evidence within the signed three-epoch window, rather than
        // truncating only the epoch while leaving incompatible production bounds.
        npos.evidence_horizon_blocks = 11;
        npos.slashing_delay_blocks = 11;
        npos.validate().map_err(|error| eyre!(error))?;
        parameters.custom.insert(id, npos.into_custom_parameter());
        *parameters_value = json::value::to_value(&parameters)?;
    }
    ensure!(
        count == 1,
        "native fixture must have one structured parameter block"
    );
    let raw = RawGenesisTransaction::from_json_slice_at_path(&json::to_vec(&value)?, path)?
        .with_consensus_meta();
    fs::write(path, json::to_vec(&raw)?)?;
    Ok(())
}

// Exercise the real generator-to-reset contract before fixture overlays change the source.
// These outputs remain unused private evidence; the four running peers keep the local layout.
async fn materialize_generated_validator_configs(
    root: &Path,
    directory: &Path,
    cli: &Path,
    api_base: u16,
    deadline: Instant,
) -> Result<()> {
    let network: NetworkId = fs::read_to_string(directory.join("genesis.expected_hash"))?
        .trim()
        .parse()?;
    let deployment = KeyPair::try_random_with_algorithm(iroha_crypto::Algorithm::Ed25519)?;
    let output_dir = root.join("materialized-validator-configs");
    fs::create_dir(&output_dir)?;
    fs::set_permissions(&output_dir, fs::Permissions::from_mode(0o700))?;
    for peer in 0..4 {
        let role = format!("taira-validator-{}", peer + 1);
        let source = fs::OpenOptions::new()
            .read(true)
            .custom_flags(nix::libc::O_NOFOLLOW)
            .open(directory.join(format!("peer{peer}.toml")))?;
        let output = output_dir.join(format!("{role}.toml"));
        let mut command = Command::new(cli);
        command
            .args([
                "taira",
                "public-reset",
                "materialize-validator-config",
                "--config-fd",
                "198",
            ])
            .arg("--localnet-dir")
            .arg(directory)
            .arg("--validator")
            .arg(&role)
            .arg("--network-id")
            .arg(network.to_string())
            .arg("--genesis-file")
            .arg(format!("/srv/taira/{role}/genesis.json"))
            .arg("--operator-public-key")
            .arg(deployment.public_key().to_string())
            .arg("--torii-bind-address")
            .arg(format!("127.0.0.1:{}", api_base + peer))
            .arg("--output")
            .arg(&output);
        super::inherit(&mut command, &[(source.as_raw_fd(), 198)])?;
        run(
            &mut command,
            &root.join(format!("prepare-materialize-{role}")),
            deadline,
            "generated validator reset materialization",
        )
        .await?;
        let metadata = fs::symlink_metadata(&output)?;
        ensure!(
            metadata.is_file()
                && !metadata.file_type().is_symlink()
                && metadata.permissions().mode() & 0o7777 == 0o600,
            "materialized validator must remain a direct private file"
        );
        // Native test code alone reads the fixture-owned private output. Never emit its body.
        let projected: toml::Table = toml::from_str(&fs::read_to_string(&output)?)
            .map_err(|_| eyre!("materialized validator is not TOML"))?;
        ensure!(
            projected["torii"]["address"].as_str()
                == Some(
                    iroha_primitives::addr::SocketAddr::from(([127, 0, 0, 1], api_base + peer))
                        .to_literal()
                        .as_str()
                ),
            "materialized validator omitted the explicit loopback Torii listener"
        );
        ensure!(
            projected["torii"]["operator_signatures"]["allowed_public_keys"]
                == toml::Value::Array(vec![deployment.public_key().to_string().into()]),
            "materialized validator omitted the explicit deployment key"
        );
    }
    Ok(())
}

/// Materialize fresh fixture keys only through Kagami, then sign and independently
/// validate the exact short-epoch genesis before constructing its public request.
pub(super) async fn prepare(
    root: &Path,
    kagami: &Path,
    cli: &Path,
    api_base: u16,
    p2p_base: u16,
    deadline: Instant,
) -> Result<Prepared> {
    ensure!(api_base > 0 && p2p_base > 0, "zero fixture port");
    let api_end = api_base
        .checked_add(3)
        .ok_or_else(|| eyre!("API port range overflow"))?;
    let p2p_end = p2p_base
        .checked_add(3)
        .ok_or_else(|| eyre!("P2P port range overflow"))?;
    ensure!(
        api_end < p2p_base || p2p_end < api_base,
        "fixture port ranges overlap"
    );
    let directory = root.join("localnet");
    ensure!(
        !directory.try_exists()?,
        "native localnet output already exists"
    );
    let mut generate = Command::new(kagami);
    generate
        .args([
            "localnet",
            "--peers",
            "4",
            "--chain-id",
            "fc56984b-2be7-431d-840e-21514d1883f0",
            "--sora-profile",
            "nexus",
            "--consensus-mode",
            "npos",
            "--block-cadence-ms",
            "2000",
            "--bind-host",
            "127.0.0.1",
            "--public-host",
            "127.0.0.1",
            "--base-api-port",
            &api_base.to_string(),
            "--base-p2p-port",
            &p2p_base.to_string(),
            "--out-dir",
        ])
        .arg(&directory);
    run(
        &mut generate,
        &root.join("prepare-localnet"),
        deadline,
        "fresh native localnet",
    )
    .await?;
    materialize_generated_validator_configs(root, &directory, cli, api_base, deadline).await?;
    iroha_genesis::init_instruction_registry();
    // A changed manifest defines a different network. Kagami correctly refuses
    // to replace a published identity, so all final outputs have fresh paths.
    let genesis_directory = directory.join("final-genesis");
    fs::create_dir(&genesis_directory)?;
    let manifest_path = genesis_directory.join("genesis.json");
    let routed_client = routed_account_and_snapshot_config(&directory, &manifest_path)?;
    short_epoch_manifest(&manifest_path)?;
    let genesis_public_key =
        PublicKey::from_str(fs::read_to_string(directory.join("genesis.public_key"))?.trim())?;
    fs::copy(
        directory.join("genesis.public_key"),
        genesis_directory.join("genesis.public_key"),
    )?;
    let mut sign = Command::new(kagami);
    sign.args(["genesis", "sign"])
        .arg(&manifest_path)
        .arg("--out-file")
        .arg(genesis_directory.join("genesis.signed.nrt"))
        .arg("--bound-manifest-out")
        .arg(&manifest_path)
        .arg("--expected-hash-out")
        .arg(genesis_directory.join("genesis.expected_hash"))
        .arg("--private-key-file")
        .arg(directory.join("genesis.private_key"))
        .arg("--expected-public-key")
        .arg(genesis_public_key.to_string())
        .arg("--config")
        .arg(directory.join("peer0.toml"));
    run(
        &mut sign,
        &root.join("prepare-genesis-sign"),
        deadline,
        "short-epoch genesis signing",
    )
    .await?;
    let _profile = iroha_data_model::account::address::ChainDiscriminantGuard::enter(369);
    let network_id = NetworkId::from_str(
        fs::read_to_string(genesis_directory.join("genesis.expected_hash"))?.trim(),
    )?;
    let manifest = RawGenesisTransaction::from_path(&manifest_path)?;
    let bundle = validate_prepared_genesis_bundle(
        &fs::read(genesis_directory.join("genesis.signed.nrt"))?,
        &manifest,
        &genesis_public_key,
        network_id.into_genesis_hash(),
    )?;
    let roster = signed_genesis_voting_peers(&GenesisBlock(bundle.block().clone()))?;
    ensure!(
        roster.len() == 4,
        "native genesis did not authenticate four voters"
    );
    // FD198 config parsing cannot resolve a relative identity-file path. Bind
    // the verified public identity inline while preserving all other config data.
    for index in 0..4 {
        let path = directory.join(format!("peer{index}.toml"));
        let mut config: toml::Table = fs::read_to_string(&path)?.parse()?;
        let genesis = config
            .get_mut("genesis")
            .and_then(toml::Value::as_table_mut)
            .ok_or_else(|| eyre!("native peer config omitted genesis"))?;
        ensure!(
            genesis.remove("expected_hash_file").is_some(),
            "native peer omitted identity file binding"
        );
        ensure!(
            genesis
                .insert(
                    "expected_hash".into(),
                    toml::Value::String(network_id.to_string())
                )
                .is_none(),
            "native peer has conflicting identity bindings"
        );
        ensure!(
            genesis
                .get("file")
                .and_then(toml::Value::as_str)
                .map(Path::new)
                == Some(directory.join("genesis.signed.nrt").as_path()),
            "native peer omitted original signed genesis binding"
        );
        genesis.insert(
            "file".into(),
            toml::Value::String(
                genesis_directory
                    .join("genesis.signed.nrt")
                    .to_string_lossy()
                    .into_owned(),
            ),
        );
        fs::write(path, toml::to_string(&config)?)?;
    }
    // Existing and routed native clients must select the same authenticated
    // final identity as every validator; the original identity stays untouched.
    for path in [directory.join("client.toml"), routed_client.clone()] {
        let mut client: toml::Table = fs::read_to_string(&path)?.parse()?;
        ensure!(
            client
                .remove("network_id_file")
                .as_ref()
                .and_then(toml::Value::as_str)
                == Some("genesis.expected_hash"),
            "native client omitted original network identity binding"
        );
        ensure!(
            client
                .insert(
                    "network_id".into(),
                    toml::Value::String(network_id.to_string())
                )
                .is_none(),
            "native client has conflicting network identity bindings"
        );
        fs::write(path, toml::to_string(&client)?)?;
    }
    let mut session_id = [0; 32];
    OsRng
        .try_fill_bytes(&mut session_id)
        .map_err(|error| eyre!("fresh session entropy: {error}"))?;
    ensure!(session_id != [0; 32], "fresh beacon session is zero");
    let session_name: String = session_id
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let provider_handles: Vec<_> = (1..=4)
        .map(|seat| format!("software://beacon/{session_name}/seat-{seat}"))
        .collect();
    for handle in &provider_handles {
        iroha_config::parameters::validate_production_runtime_handle(handle)
            .map_err(|error| eyre!("invalid public provider handle: {error:?}"))?;
    }
    let request_value = Request {
        schema: "iroha.global-beacon.bootstrap.request.v1".into(),
        dkg_session: GlobalThresholdBeaconDkgSessionV1 {
            version: GLOBAL_THRESHOLD_BEACON_VERSION_V1,
            network_id,
            session_id,
            roster_hash: global_threshold_beacon_roster_hash_v1(&roster),
            committee_size: 4,
            threshold: 2,
            start_height: 1,
            sharing_end_height: 2,
            complaints_end_height: 3,
            responses_end_height: 4,
        },
        target_roster: roster.clone(),
        authorization_roster: roster.clone(),
        provider_handles,
        provider_revision: 1,
    };
    let request = root.join("beacon-request.json");
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&request)?;
    file.write_all(&json::to_vec(&request_value)?)?;
    file.sync_all()?;
    ensure!(
        Instant::now() < deadline,
        "beacon fixture deadline after genesis preparation"
    );
    Ok(Prepared {
        directory,
        genesis_directory,
        request,
        roster,
        network_id,
        genesis_public_key,
        routed_client,
    })
}
