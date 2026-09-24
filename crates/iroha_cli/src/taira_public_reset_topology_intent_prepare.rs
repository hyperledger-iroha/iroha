//! Read-only producer for one fresh topology intent after a durable dispatcher apply.
use super::super::{admission, storage};
use super::*;
use crate::taira_public_reset as reset;
use rand::{rand_core::TryRngCore as _, rngs::OsRng};
use reset::{
    BUILD_PROFILE, BUILD_TARGET, CHAIN_ID, EdgeInitialStateV1, FaucetPolicyV1, FeeIntentV1,
    RevisionV1, SourceManifestV1, ValidatorClientV1, ValidatorInitialStateV1,
};

/// All live predecessor identity comes from the sealed plan and stopped capture.
#[derive(clap::Args, Debug)]
pub(in super::super::super::super) struct PrepareTopologyIntent {
    #[arg(long)]
    retained_inventory: PathBuf,
    #[arg(long)]
    expected_retained_inventory_sha256: String,
    #[arg(long)]
    current_runtime: PathBuf,
    #[arg(long)]
    expected_current_runtime_sha256: String,
    #[arg(long)]
    transition_plan: PathBuf,
    #[arg(long)]
    expected_plan_sha256: String,
    #[arg(long)]
    import_root: PathBuf,
    #[arg(long)]
    public_inputs: PathBuf,
    #[arg(long)]
    source_manifest: PathBuf,
    #[arg(long)]
    known_hosts: PathBuf,
    #[arg(long, num_args = 4)]
    validator_client_config: Vec<PathBuf>,
    #[arg(long, num_args = 4)]
    validator_config: Vec<PathBuf>,
    #[arg(long, num_args = 4)]
    initial_unit: Vec<PathBuf>,
    #[arg(long)]
    edge_config: PathBuf,
    #[arg(long)]
    output: PathBuf,
}

fn absolute(path: &Path, label: &str) -> Result<String> {
    validate_absolute_normal_path(path, label)?;
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| eyre!("{label} must be UTF-8"))
}

fn artifact(
    role: &str,
    local: &Path,
    remote: String,
) -> Result<reset::inputs::ResetArtifactIntentV1> {
    validate_absolute_normal_path(Path::new(&remote), "candidate remote artifact")?;
    Ok(reset::inputs::ResetArtifactIntentV1 {
        role: role.into(),
        local_path: absolute(local, "candidate local artifact")?,
        remote_path: remote,
    })
}

fn bind_predecessor(old: &InventoryV1, runtime: &CurrentRuntime, plan: &Plan) -> Result<()> {
    need(
        old.validators.len() == 4
            && old.validator_clients.len() == 4
            && runtime.validators.len() == 4
            && plan.predecessor.occupied.len() == 5
            && plan.host_identity_sha256 == runtime.host_identity_sha256
            && old.revision.commit != plan.candidate.commit,
        "retained inventory, stopped runtime and candidate revision differ",
    )?;
    for index in 0..4 {
        let old_validator = &old.validators[index];
        let current = &runtime.validators[index];
        let occupied = &plan.predecessor.occupied[index];
        need(
            old_validator.slug == SLUGS[index]
                && old.validator_clients[index].slug == SLUGS[index]
                && current.commit == old.revision.commit
                && old_validator.endpoint.host_identity_sha256 == runtime.host_identity_sha256
                && occupied.slug == SLUGS[index]
                && occupied.selector.target == current.release_root
                && occupied.files.len() == current.artifacts.len(),
            "stopped validator differs from selected predecessor",
        )?;
        current
            .service_state
            .validate_state_identity(occupied.state.device, occupied.state.inode)?;
        for (pin, file) in occupied.files.iter().zip(&current.artifacts) {
            need(
                pin.path == file.path
                    && pin.sha256 == file.sha256
                    && pin.size == file.size
                    && pin.mode == u32::from(file.mode),
                "stopped validator artifact differs from transition plan",
            )?;
        }
        // Executables may have been refreshed independently; the selected
        // configuration and genesis must still be those of the sealed inventory.
        for role in ["config", "genesis", "genesis_hash"] {
            let signed = old_validator
                .artifacts
                .iter()
                .find(|artifact| artifact.role == role)
                .ok_or_else(|| eyre!("retained validator omits {role}"))?;
            let selected = current.artifact(role)?;
            need(
                signed.sha256 == selected.sha256
                    && signed.size == selected.size
                    && signed.mode == selected.mode,
                "sealed validator configuration or genesis differs from selected runtime",
            )?;
        }
    }
    let edge = &plan.predecessor.occupied[4];
    need(
        old.edge.slug == "taira-edge"
            && old.edge.endpoint.host_identity_sha256 == runtime.host_identity_sha256
            && runtime.edge.commit == old.revision.commit
            && edge.slug == "taira-edge"
            && edge.selector.target == runtime.edge.release_root
            && edge.files.len() == 4
            && edge.files[0].sha256 == runtime.edge.cli_sha256
            && edge.files[1].sha256 == runtime.edge.config_sha256
            && old.edge.artifacts.iter().any(|artifact| {
                artifact.role == "edge_config" && artifact.sha256 == runtime.edge.config_sha256
            }),
        "stopped edge differs from selected predecessor",
    )
}

fn distinct_new_clients(
    old: &InventoryV1,
    new: &[ValidatorClientV1],
    faucet: &FaucetPolicyV1,
    new_canary: &str,
) -> Result<()> {
    need(new.len() == 4, "four new validator clients required")?;
    let old_accounts: BTreeSet<&str> = old
        .validator_clients
        .iter()
        .map(|v| v.account_id.as_str())
        .collect();
    let old_peers: BTreeSet<&str> = old
        .validator_clients
        .iter()
        .map(|v| v.peer_id.as_str())
        .collect();
    let accounts: BTreeSet<&str> = new.iter().map(|v| v.account_id.as_str()).collect();
    let peers: BTreeSet<&str> = new.iter().map(|v| v.peer_id.as_str()).collect();
    need(
        accounts.len() == 4
            && peers.len() == 4
            && accounts.is_disjoint(&old_accounts)
            && peers.is_disjoint(&old_peers)
            && !accounts.contains(faucet.authority.as_str())
            && !accounts.contains(old.canary_onboarding_request.account_id.as_str())
            && !accounts.contains(new_canary)
            && faucet.authority != old.faucet_policy.authority
            && !old_accounts.contains(faucet.authority.as_str())
            && faucet.authority != old.canary_onboarding_request.account_id
            && faucet.authority != new_canary
            && new_canary != old.canary_onboarding_request.account_id
            && new_canary != old.faucet_policy.authority
            && !old_accounts.contains(new_canary),
        "candidate account, peer or faucet identity was reused",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn topology_candidate_rejects_reused_or_duplicate_account_peer_and_faucet_identities() {
        let old = reset::sample_inventory_fixture();
        let mut clients = old.validator_clients.clone();
        for (index, client) in clients.iter_mut().enumerate() {
            client.account_id = format!("new-account-{index}");
            client.peer_id = format!("new-peer-{index}");
        }
        let mut faucet = old.faucet_policy.clone();
        faucet.authority = "new-faucet".into();
        distinct_new_clients(&old, &clients, &faucet, "new-canary").unwrap();
        clients[0].account_id = old.validator_clients[0].account_id.clone();
        assert!(distinct_new_clients(&old, &clients, &faucet, "new-canary").is_err());
        clients[0].account_id = clients[1].account_id.clone();
        assert!(distinct_new_clients(&old, &clients, &faucet, "new-canary").is_err());
        clients[0].account_id = "new-account-0".into();
        clients[0].peer_id = old.validator_clients[0].peer_id.clone();
        assert!(distinct_new_clients(&old, &clients, &faucet, "new-canary").is_err());
        clients[0].peer_id = "new-peer-0".into();
        faucet.authority = clients[0].account_id.clone();
        assert!(distinct_new_clients(&old, &clients, &faucet, "new-canary").is_err());
        faucet.authority = old.validator_clients[0].account_id.clone();
        assert!(distinct_new_clients(&old, &clients, &faucet, "new-canary").is_err());
        faucet.authority = "new-faucet".into();
        assert!(distinct_new_clients(&old, &clients, &faucet, "new-account-0").is_err());
        assert!(
            distinct_new_clients(
                &old,
                &clients,
                &faucet,
                &old.canary_onboarding_request.account_id
            )
            .is_err()
        );
    }
}

fn daemon_identity(
    path: &Path,
    release: &str,
    public: &reset::public_inputs::PublicInputsV1,
    observed: &mut Observed,
) -> Result<(String, String, FaucetPolicyV1)> {
    use iroha_config::{
        base::toml::{MAX_TOML_SOURCE_BYTES, TomlSource},
        parameters::actual,
    };
    let pin = observed.pin(path, Some(0o600), MAX_TOML_SOURCE_BYTES as u64)?;
    let bytes = zeroize::Zeroizing::new(admission::read(&pin)?);
    reset::validate_validator_genesis_config(
        &bytes,
        Path::new(&format!("{release}/genesis/genesis.json")),
        &public.genesis_hash,
    )?;
    let text = std::str::from_utf8(&bytes).wrap_err("candidate validator config is not UTF-8")?;
    let table: toml::Table =
        toml::from_str(text).wrap_err("candidate validator config is not TOML")?;
    let config = actual::Root::from_toml_source(TomlSource::new_sensitive(
        path.to_path_buf(),
        table,
        crate::soracloud::zeroize_taira_toml_table,
    ))
    .map_err(|_| eyre!("candidate validator config failed current typed admission"))?;
    need(
        config.common.chain.to_string() == CHAIN_ID,
        "candidate validator chain differs",
    )?;
    let faucet = config
        .torii
        .faucet
        .as_ref()
        .ok_or_else(|| eyre!("candidate validator faucet is absent"))?;
    let policy = FaucetPolicyV1 {
        authority: faucet.authority.to_string(),
        asset_definition_id: faucet.asset_definition_id.clone(),
        amount: faucet.amount.clone(),
    };
    reset::validate_faucet_policy(&policy)?;
    reset::validate_validator_faucet_config(&bytes, &policy)?;
    reset::inputs::validate_validator_pin_fee_asset(
        &config.gov.sorafs_pin_fee_asset_id,
        &policy.asset_definition_id,
    )?;
    let origin = format!("http://127.0.0.1:{}/", config.torii.address.value().port());
    reset::validate_candidate_probe_bind(&origin, config.torii.address.value())?;
    observed.revalidate()?;
    Ok((config.common.peer.id.to_string(), origin, policy))
}

fn source_revision(
    source_manifest: &Path,
    import_root: &Path,
    candidate_commit: &str,
    observed: &mut Observed,
) -> Result<RevisionV1> {
    let manifest_pin = observed.pin(source_manifest, None, MAX_PROOF)?;
    let bytes = admission::read(&manifest_pin)?;
    let source: SourceManifestV1 = json::from_slice(&bytes)?;
    let revision = RevisionV1 {
        branch: source.branch,
        commit: source.head_commit_sha1.clone(),
        tree: source.head_tree_sha1,
        cargo_lock_sha256: source.cargo_lock_sha256,
        source_root: absolute(&import_root.join("source/source"), "qualified source root")?,
        source_manifest_path: absolute(source_manifest, "native source manifest")?,
        source_manifest_sha256: sha256_hex(&bytes),
        source_closure_sha256: source.closure_sha256,
        target: BUILD_TARGET.into(),
        profile: BUILD_PROFILE.into(),
        build_id: source.head_commit_sha1,
    };
    need(
        revision.commit == candidate_commit,
        "signed source differs from qualified candidate",
    )?;
    reset::validate_revision(&revision)?;
    reset::validate_source_closure(&revision)?;
    Ok(revision)
}

impl PrepareTopologyIntent {
    /// Create a private reviewable intent without starting, deploying or signing a reset.
    pub(in super::super::super::super) fn run<W: Write>(&self, output: &mut W) -> Result<()> {
        #[cfg(not(target_os = "linux"))]
        {
            let _ = output;
            return Err(eyre!("topology intent preparation requires Linux"));
        }
        #[cfg(target_os = "linux")]
        {
            need(rustix::process::geteuid().as_raw() == 0, "root is required")?;
            validate_absolute_normal_path(&self.output, "topology output")?;
            let parent = self
                .output
                .parent()
                .ok_or_else(|| eyre!("topology output has no parent"))?;
            reset::validate_owner_private_dir(parent, "topology output directory")?;
            need(!self.output.exists(), "topology output already exists")?;
            for hash in [
                &self.expected_retained_inventory_sha256,
                &self.expected_current_runtime_sha256,
                &self.expected_plan_sha256,
            ] {
                require_lower_sha256(hash, "topology producer input digest")?;
            }
            for paths in [
                &self.validator_client_config,
                &self.validator_config,
                &self.initial_unit,
            ] {
                need(paths.len() == 4, "four ordered candidate inputs required")?;
                let unique: BTreeSet<&PathBuf> = paths.iter().collect();
                need(unique.len() == 4, "candidate input paths must be distinct")?;
            }
            let mut observed = Observed(Vec::new());
            let plan_pin = observed.pin(&self.transition_plan, Some(0o600), MAX_PROOF)?;
            need(
                plan_pin.sha256 == self.expected_plan_sha256,
                "transition plan digest differs",
            )?;
            let plan_bytes = admission::read(&plan_pin)?;
            let plan: Plan = json::from_slice(&plan_bytes)?;
            admission::validate_plan(&plan)?;
            need(
                self.import_root.as_path()
                    == Path::new(&plan.candidate.executable.path)
                        .parent()
                        .ok_or_else(|| eyre!("candidate executable has no parent"))?
                        .parent()
                        .ok_or_else(|| eyre!("candidate binary has no import root"))?
                        .parent()
                        .ok_or_else(|| eyre!("candidate artifact has no import root"))?
                    && plan.candidate.preparation.path
                        == self
                            .import_root
                            .join("preparation/result.json")
                            .to_string_lossy()
                            .as_ref(),
                "qualified import root differs from reviewed transition plan",
            )?;
            let locks = admission::locks(&plan)?;
            let inventory_pin = observed.pin(&self.retained_inventory, None, MAX_PROOF)?;
            need(
                inventory_pin.sha256 == self.expected_retained_inventory_sha256
                    && inventory_pin.sha256 == plan.predecessor.inventory_sha256,
                "retained inventory digest differs from transition plan",
            )?;
            let inventory_bytes = admission::read(&inventory_pin)?;
            let (old, _chain_guard) =
                reset::decode_inventory(&inventory_bytes, "retained inventory")?;
            reset::validate_inventory_for_controller(
                &old,
                reset::ControllerAdmission::AbandonOriginalTarget,
            )?;
            let runtime_pin = observed.pin(&self.current_runtime, Some(0o600), MAX_PROOF)?;
            need(
                runtime_pin.sha256 == self.expected_current_runtime_sha256,
                "stopped runtime digest differs",
            )?;
            let runtime: CurrentRuntime = json::from_slice(&admission::read(&runtime_pin)?)?;
            validate_runtime(&runtime)?;
            bind_predecessor(&old, &runtime, &plan)?;
            let held = admission::admit(&plan)?;
            let operation = operation_root(&plan);
            let guards = admission::new_guards(&plan, &operation)?;
            storage::check_applied(&plan, &plan_bytes, &operation, &guards)?;
            let revision = source_revision(
                &self.source_manifest,
                &self.import_root,
                &plan.candidate.commit,
                &mut observed,
            )?;
            let public = reset::public_inputs::load(&self.public_inputs)?;
            need(
                old.next_genesis_hash != public.genesis_hash,
                "fresh public genesis must differ from completed predecessor",
            )?;
            let mut intent = reset::inputs::ResetTopologyIntentV1::from(&old);
            intent.deployment_id = format!("taira-public-{}-", &revision.commit[..8]);
            let mut nonce = [0_u8; 16];
            OsRng
                .try_fill_bytes(&mut nonce)
                .map_err(|error| eyre!("topology nonce OS RNG failed: {error}"))?;
            intent.authorization_nonce = hex::encode(nonce);
            intent
                .deployment_id
                .push_str(&intent.authorization_nonce[..8]);
            intent.previous_genesis_hash = old.next_genesis_hash.clone();
            intent.revision.source_root = revision.source_root.clone();
            intent.revision.source_manifest_path = revision.source_manifest_path.clone();
            intent.canary_onboarding_request = public.canary_onboarding_request.clone();
            intent.fee_intent = FeeIntentV1 {
                payer: "authority".into(),
                sponsor_program: None,
                sponsor_program_revision: None,
            };
            let import_bins = self.import_root.join("artifacts/bin");
            for name in ["iroha3d_taira", "iroha", "kagami", "sorafs-node"] {
                observed.pin(&import_bins.join(name), Some(0o755), MAX_BINARY)?;
            }
            let mut clients = Vec::new();
            let mut policy: Option<FaucetPolicyV1> = None;
            for index in 0..4 {
                let slug = SLUGS[index];
                let release = format!("/srv/taira/{slug}/releases/{}", revision.commit);
                let client_observed =
                    observed.pin(&self.validator_client_config[index], Some(0o600), MAX_PROOF)?;
                let client_pinned = reset::pin_owner_private_file(
                    &self.validator_client_config[index],
                    "candidate validator client",
                )?;
                need(
                    reset::host::hash_pinned_input(
                        &client_pinned,
                        "candidate validator client",
                        None,
                    )? == client_observed.sha256,
                    "candidate validator client changed between custody and typed admission",
                )?;
                let client = reset::host::load_client_config_for_reset_genesis(
                    &client_pinned,
                    "candidate validator client",
                    &public.genesis_hash,
                )?;
                let origin = old.validator_clients[index].torii_origin.clone();
                need(
                    client.torii_api_url.as_str() == origin.as_str(),
                    "candidate client Torii origin differs from retained topology",
                )?;
                let (peer_id, probe_origin, current_policy) = daemon_identity(
                    &self.validator_config[index],
                    &release,
                    &public,
                    &mut observed,
                )?;
                need(
                    probe_origin == old.validator_clients[index].probe_origin,
                    "candidate probe origin differs from retained topology and daemon bind",
                )?;
                if let Some(expected) = &policy {
                    need(
                        *expected == current_policy,
                        "candidate validator faucet policies differ",
                    )?;
                } else {
                    policy = Some(current_policy);
                }
                clients.push(ValidatorClientV1 {
                    slug: slug.into(),
                    torii_origin: origin,
                    probe_origin,
                    account_id: client.account.to_string(),
                    peer_id,
                });
                let unit = &self.initial_unit[index];
                need(
                    unit.file_name()
                        == Some(std::ffi::OsStr::new(&format!("iroha3d-{slug}.service"))),
                    "candidate initial validator unit order differs",
                )?;
                observed.pin(unit, Some(0o644), MAX_PROOF)?;
                let validator = &mut intent.validators[index];
                validator.endpoint.remote_cli = format!("{release}/bin/iroha");
                validator.endpoint.upload_guard_sha256 = sha256_hex(&guards[index]);
                validator.initial_state =
                    ValidatorInitialStateV1::AdmittedRelease(runtime.validators[index].clone());
                validator.artifacts = vec![
                    artifact(
                        "iroha3d",
                        &import_bins.join("iroha3d_taira"),
                        format!("{release}/bin/iroha3d_taira"),
                    )?,
                    artifact(
                        "iroha_cli",
                        &import_bins.join("iroha"),
                        format!("{release}/bin/iroha"),
                    )?,
                    artifact(
                        "kagami",
                        &import_bins.join("kagami"),
                        format!("{release}/bin/kagami"),
                    )?,
                    artifact(
                        "sorafs_node",
                        &import_bins.join("sorafs-node"),
                        format!("{release}/bin/sorafs-node"),
                    )?,
                    artifact(
                        "config",
                        &self.validator_config[index],
                        format!("{release}/config/config.toml"),
                    )?,
                    artifact(
                        "genesis",
                        &self.public_inputs.join("genesis.signed.nrt"),
                        format!("{release}/genesis/genesis.json"),
                    )?,
                    artifact(
                        "genesis_hash",
                        &self.public_inputs.join("genesis.hash"),
                        format!("{release}/genesis/genesis.sha256"),
                    )?,
                    artifact(
                        "validator_unit",
                        unit,
                        format!("{release}/systemd/iroha3d-{slug}.service"),
                    )?,
                ];
            }
            let policy = policy.ok_or_else(|| eyre!("candidate faucet policy is absent"))?;
            distinct_new_clients(
                &old,
                &clients,
                &policy,
                &public.canary_onboarding_request.account_id,
            )?;
            intent.validator_clients = clients;
            intent.faucet_policy = policy;
            observed.pin(&self.edge_config, Some(0o640), MAX_PROOF)?;
            let edge_release = format!("/srv/taira/edge/releases/{}", revision.commit);
            intent.edge.endpoint.remote_cli = format!("{edge_release}/bin/iroha");
            intent.edge.endpoint.upload_guard_sha256 = sha256_hex(&guards[4]);
            intent.edge.initial_state = EdgeInitialStateV1::AdmittedRelease(runtime.edge.clone());
            intent.edge.artifacts = vec![
                artifact(
                    "iroha_cli",
                    &import_bins.join("iroha"),
                    format!("{edge_release}/bin/iroha"),
                )?,
                artifact(
                    "edge_config",
                    &self.edge_config,
                    format!("{edge_release}/taira.conf"),
                )?,
            ];
            let endpoints = intent
                .validators
                .iter()
                .map(|v| &v.endpoint)
                .chain(std::iter::once(&intent.edge.endpoint))
                .collect::<Vec<_>>();
            let known_hosts = reset::validate_known_host_endpoints(&endpoints, &self.known_hosts)?;
            reset::inputs::validate_topology_intent(&intent)?;
            let mut bytes = json::to_vec(&intent)?;
            bytes.push(b'\n');
            let (decoded, _guard) = reset::inputs::decode_reset_topology_intent(&bytes)?;
            need(
                json::to_vec(&decoded)? == json::to_vec(&intent)?,
                "topology intent roundtrip differs",
            )?;
            observed.revalidate()?;
            reset::revalidate_pinned(&known_hosts, "topology known-hosts")?;
            locks.revalidate()?;
            admission::revalidate(&plan, &held)?;
            storage::check_applied(&plan, &plan_bytes, &operation, &guards)?;
            need(
                reset::public_inputs::load(&self.public_inputs)? == public,
                "public input bundle changed",
            )?;
            reset::inputs::write_new_private(&self.output, &bytes)?;
            writeln!(
                output,
                "{}",
                json::to_json(&norito::json!({
                    "schema": "iroha.taira.public-reset.topology-intent-prepared.v1",
                    "path": (absolute(&self.output, "topology output")?),
                    "sha256": (sha256_hex(&bytes)),
                    "deployment_id": (intent.deployment_id),
                    "previous_genesis_hash": (intent.previous_genesis_hash),
                    "next_genesis_hash": (public.genesis_hash),
                    "ledger_mutated": false,
                }))?
            )?;
            Ok(())
        }
    }
}
