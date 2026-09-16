//! Fresh native localnet inputs for the production-custody beacon contract.

use color_eyre::eyre::{Result, WrapErr as _, ensure, eyre};
use iroha_core::{
    beacon::global_threshold_beacon_roster_hash_v1, sumeragi::signed_genesis_voting_peers,
};
use iroha_crypto::PublicKey;
use iroha_data_model::{
    NetworkId,
    consensus::{GLOBAL_THRESHOLD_BEACON_VERSION_V1, GlobalThresholdBeaconDkgSessionV1},
    parameter::system::{Parameters, SumeragiNposParameters},
};
use iroha_genesis::{GenesisBlock, RawGenesisTransaction, validate_prepared_genesis_bundle};
use iroha_model_base::peer::PeerId;
use norito::{derive::JsonSerialize, json};
use rand::{TryRngCore as _, rngs::OsRng};
use std::{
    fs,
    io::Write as _,
    num::NonZeroU64,
    os::unix::fs::OpenOptionsExt as _,
    path::{Path, PathBuf},
    str::FromStr as _,
};
use tokio::{
    process::Command,
    time::{Instant, timeout_at},
};

pub(super) struct Prepared {
    pub directory: PathBuf,
    pub request: PathBuf,
    pub roster: Vec<PeerId>,
    pub network_id: NetworkId,
    pub genesis_public_key: PublicKey,
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

async fn run(command: &mut Command, deadline: Instant, stage: &str) -> Result<()> {
    ensure!(
        Instant::now() < deadline,
        "beacon fixture deadline before {stage}"
    );
    let output = timeout_at(deadline, command.env_clear().kill_on_drop(true).output())
        .await
        .wrap_err_with(|| format!("beacon fixture deadline during {stage}"))??;
    // Native configuration may contain keys. Never include child output in errors.
    ensure!(
        output.status.success(),
        "beacon fixture {stage} failed: {}",
        output.status
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
        npos.epoch_length_blocks = NonZeroU64::new(8).expect("positive fixture epoch");
        // Retain evidence within the signed three-epoch window, rather than
        // truncating only the epoch while leaving incompatible production bounds.
        npos.evidence_horizon_blocks = 8;
        npos.slashing_delay_blocks = 8;
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

/// Materialize fresh fixture keys only through Kagami, then sign and independently
/// validate the exact short-epoch genesis before constructing its public request.
pub(super) async fn prepare(
    root: &Path,
    kagami: &Path,
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
    run(&mut generate, deadline, "fresh native localnet").await?;
    iroha_genesis::init_instruction_registry();
    let manifest_path = directory.join("genesis.json");
    short_epoch_manifest(&manifest_path)?;
    let genesis_public_key =
        PublicKey::from_str(fs::read_to_string(directory.join("genesis.public_key"))?.trim())?;
    let mut sign = Command::new(kagami);
    sign.args(["genesis", "sign"])
        .arg(&manifest_path)
        .arg("--out-file")
        .arg(directory.join("genesis.signed.nrt"))
        .arg("--bound-manifest-out")
        .arg(&manifest_path)
        .arg("--expected-hash-out")
        .arg(directory.join("genesis.expected_hash"))
        .arg("--private-key-file")
        .arg(directory.join("genesis.private_key"))
        .arg("--expected-public-key")
        .arg(genesis_public_key.to_string())
        .arg("--config")
        .arg(directory.join("peer0.toml"));
    run(&mut sign, deadline, "short-epoch genesis signing").await?;
    let network_id =
        NetworkId::from_str(fs::read_to_string(directory.join("genesis.expected_hash"))?.trim())?;
    let manifest = RawGenesisTransaction::from_path(&manifest_path)?;
    let bundle = validate_prepared_genesis_bundle(
        &fs::read(directory.join("genesis.signed.nrt"))?,
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
        fs::write(path, toml::to_string(&config)?)?;
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
        request,
        roster,
        network_id,
        genesis_public_key,
    })
}
