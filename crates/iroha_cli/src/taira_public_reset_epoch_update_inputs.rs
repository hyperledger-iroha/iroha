//! Public, typed preparation for the maintained updater's native custody step.
//!
//! This command reads only selected public records. It does not contact hosts,
//! inspect credentials/seeds, provision a generation, or attest runtime readiness.

use super::*;
use crate::taira_dataspace_deploy::{DeploymentTrustV1, validate_deployment_trust};
use epoch_generation::{BindingV1, PreparationV1};
use epoch_supervisor::{
    EpochSupervisorPlanV1, KagamiV1, NativePolicyV1, OngoingIntentV1, SeedCustodyV1, SeedV1,
};

/// Produce the exact public preparation consumed by epoch-supervisor-host materialize.
#[derive(clap::Args, Debug)]
pub(in super::super) struct PrepareEpochUpdate {
    #[arg(long)]
    deployment: PathBuf,
    #[arg(long)]
    prepared_result: PathBuf,
    /// Explicit fresh update-<32 lowercase hex> operation; determines binary paths.
    #[arg(long)]
    operation: String,
    #[arg(long, value_parser = ["absent", "running", "stopped"])]
    original_service_state: String,
    #[arg(long, value_parser = ["running", "stopped"])]
    successor_service_state: String,
    #[arg(long)]
    before_binding: Option<PathBuf>,
    /// Actual installed state is selected independently of original operator intent.
    #[arg(long, value_parser = ["absent", "present"])]
    installed_state: String,
    #[arg(long)]
    installed_binding: Option<PathBuf>,
    /// Explicit current observation trust; never rebased from a client config.
    #[arg(long)]
    trust: PathBuf,
    #[arg(long, value_parser = ["until-stopped"])]
    authorization: String,
    #[arg(long)]
    administrator: String,
    #[arg(long)]
    payment_asset: String,
    #[arg(long)]
    transaction_fee_maximum: String,
    #[arg(long)]
    first_epoch: u64,
    #[arg(long)]
    batch_epochs: u64,
    #[arg(long)]
    operation_timeout_ms: u64,
    #[arg(long)]
    provision_timeout_ms: u64,
    #[arg(long)]
    worker_timeout_ms: u64,
    /// Four original file references in sorted native PeerId order; never opened here.
    #[arg(long, num_args = 4, value_name = "PATH")]
    original_seed_sources: Vec<PathBuf>,
    /// Fresh owner-private directory; publication never replaces an existing bundle.
    #[arg(long)]
    output: PathBuf,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct ReferenceV1 {
    path: String,
    sha256: String,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct SshV1 {
    argv: Vec<String>,
    pins: Vec<ReferenceV1>,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct CurrentV1 {
    commit: String,
    daemon: String,
    attempt_name: String,
    plan_schema: String,
    result_schema: String,
    local_plan: String,
    local_plan_sha256: String,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct DeploymentV1 {
    schema: String,
    guest_ssh: SshV1,
    runtime_root: String,
    state_root: String,
    config_root: String,
    config_release: String,
    genesis_manifest: String,
    network_id: NetworkId,
    public_origin: String,
    roles: Vec<String>,
    ports: Vec<u16>,
    replay_floor: u64,
    renderer_sha256: String,
    current: CurrentV1,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct ArtifactV1 {
    name: String,
    package: String,
    path: String,
    sha256: String,
    size: u64,
}
/// Exact maintained preparation result, including its retained provenance fields.
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct PreparedV1 {
    commit: String,
    signer_fingerprint: String,
    native_check_scope: String,
    native_incremental: bool,
    environment_sha256: String,
    native_environment_sha256: String,
    tree: String,
    target: String,
    profile: String,
    jobs: u64,
    source_unchanged: bool,
    toolchain_unchanged: bool,
    source_snapshot_sha256: String,
    source_root: String,
    source_output_target: String,
    compiler_tools: Vec<json::Value>,
    tools: Vec<json::Value>,
    command: Vec<String>,
    release_qualified: bool,
    deployed: bool,
    artifacts: Vec<ArtifactV1>,
    timings_seconds: json::Value,
    attempt: String,
}
#[derive(JsonSerialize)]
struct OutputV1 {
    schema: String,
    operation: String,
    deployment: ReferenceV1,
    prepared_result: ReferenceV1,
    trust: ReferenceV1,
    before_binding: Option<ReferenceV1>,
    installed_binding: Option<ReferenceV1>,
    preparation: ReferenceV1,
    after_binding: ReferenceV1,
    credential_contents_read: bool,
    seed_contents_read: bool,
    host_contacted: bool,
    native_materialization_required: bool,
}

fn require(ok: bool, message: &'static str) -> Result<()> {
    if ok { Ok(()) } else { Err(eyre!(message)) }
}
fn lower_hex(value: &str, len: usize) -> bool {
    value.len() == len
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        && value.bytes().any(|b| b != b'0')
}
fn public_path(value: &str) -> Result<()> {
    require(
        value.starts_with('/')
            && !value.ends_with('/')
            && !value.contains("//")
            && value
                .split('/')
                .skip(1)
                .all(|p| !p.is_empty() && p != "." && p != "..")
            && !value.bytes().any(|b| b.is_ascii_control()),
        "public path must be absolute and normalized",
    )
}
fn update_operation(operation: &str, current: &str) -> Result<()> {
    require(
        operation.strip_prefix("update-").is_some_and(|suffix| {
            suffix.len() == 32
                && suffix
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        }) && operation != current,
        "fresh explicit update operation required",
    )
}
fn validate_deployment(value: &DeploymentV1) -> Result<()> {
    require(
        value.schema == "taira.runtime-deployment.v1"
            && value.replay_floor > 0
            && lower_hex(&value.config_release, 40)
            && lower_hex(&value.current.commit, 40)
            && lower_hex(&value.renderer_sha256, 64)
            && lower_hex(&value.current.local_plan_sha256, 64),
        "maintained runtime deployment identity differs",
    )?;
    for path in [
        &value.runtime_root,
        &value.state_root,
        &value.config_root,
        &value.genesis_manifest,
        &value.current.daemon,
        &value.current.local_plan,
    ] {
        public_path(path)?;
    }
    require(
        Path::new(&value.current.daemon).starts_with(&value.runtime_root)
            && Path::new(&value.current.daemon).file_name() == Some(OsStr::new("iroha3d_taira"))
            && value.roles
                == (1..=4)
                    .map(|i| format!("taira-validator-{i}"))
                    .collect::<Vec<_>>()
            && value.ports.len() == 4
            && value.ports.iter().all(|p| *p > 0)
            && value.ports.iter().collect::<BTreeSet<_>>().len() == 4,
        "deployment must retain its exact four-validator runtime",
    )?;
    epoch_generation::operation(&value.current.attempt_name)?;
    for schema in [&value.current.plan_schema, &value.current.result_schema] {
        require(
            schema.starts_with("taira.")
                && schema.ends_with(".v1")
                && schema
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b".-".contains(&b)),
            "retained installation receipt schema differs",
        )?;
    }
    let origin = Url::parse(&value.public_origin)?;
    require(
        origin.scheme() == "https"
            && origin.host_str().is_some()
            && origin.username().is_empty()
            && origin.password().is_none()
            && origin.query().is_none()
            && origin.fragment().is_none()
            && origin.path() == "/"
            && !value.public_origin.ends_with('/'),
        "canonical public origin required",
    )?;
    // Transport execution and host-key file admission belong to the existing updater.
    require(
        !value.guest_ssh.argv.is_empty() && !value.guest_ssh.pins.is_empty(),
        "explicit pinned SSH route required",
    )?;
    for pin in &value.guest_ssh.pins {
        public_path(&pin.path)?;
        require(
            lower_hex(&pin.sha256, 64),
            "SSH public evidence digest missing",
        )?;
    }
    Ok(())
}
fn validate_prepared(value: &PreparedV1, current: &str) -> Result<()> {
    require(
        lower_hex(&value.commit, 40)
            && value.commit != current
            && value.target == "aarch64-unknown-linux-gnu"
            && value.profile == "release"
            && value.jobs == 6
            && value.native_check_scope == "basic"
            && value.source_unchanged
            && value.toolchain_unchanged
            && !value.release_qualified
            && !value.deployed
            && value.artifacts.len() == 4,
        "completed maintained four-artifact basic preparation required",
    )?;
    let expected = [
        ("iroha3d_taira", "irohad"),
        ("iroha", "iroha_cli"),
        ("sorafs-node", "sorafs_node"),
        ("kagami", "iroha_kagami"),
    ];
    for (row, (name, package)) in value.artifacts.iter().zip(expected) {
        require(
            row.name == name
                && row.package == package
                && lower_hex(&row.sha256, 64)
                && row.size > 1_000_000
                && row.size < 1024 * 1024 * 1024,
            "prepared artifact identity, order, package or size differs",
        )?;
        public_path(&row.path)?;
    }
    Ok(())
}
fn seed_references(
    trust: &DeploymentTrustV1,
    paths: &[PathBuf],
    network: NetworkId,
) -> Result<(Vec<SeedV1>, SeedCustodyV1)> {
    let mut peers: Vec<_> = trust.peers.iter().map(|p| p.peer_id.clone()).collect();
    peers.sort();
    require(
        peers.len() == 4 && peers.windows(2).all(|p| p[0] < p[1]) && paths.len() == 4,
        "exact four sorted native validators and source references required",
    )?;
    let mut original = Vec::new();
    let mut retained = Vec::new();
    let mut seen = BTreeSet::new();
    for (index, (validator, source)) in peers.into_iter().zip(paths).enumerate() {
        let path = source
            .to_str()
            .ok_or_else(|| eyre!("seed source path must be UTF-8"))?;
        public_path(path)?;
        require(
            seen.insert(path.to_owned()),
            "original seed source references must be distinct",
        )?;
        original.push(SeedV1 {
            validator: validator.clone(),
            path: path.into(),
        });
        retained.push(SeedV1 {
            validator,
            path: format!(
                "{}/seeds/{network}/peer{index}.seed",
                epoch_supervisor::STATE_ROOT
            ),
        });
    }
    Ok((
        original,
        SeedCustodyV1 {
            schema_version: 1,
            seeds: retained,
        },
    ))
}
fn validate_binding(binding: &BindingV1, deployment: &DeploymentV1, baseline: bool) -> Result<()> {
    let plan = epoch_generation::as_public_plan(binding, "stopped")?;
    require(
        binding.network_id == deployment.network_id.to_string(),
        "binding targets another network",
    )?;
    if baseline {
        require(
            binding.release_source_commit == deployment.current.commit
                && Path::new(&binding.unit_spec.cli)
                    == Path::new(&deployment.current.daemon).with_file_name("iroha"),
            "original binding differs from completed runtime",
        )?;
    }
    let trust: DeploymentTrustV1 = json::from_slice(&plan.observation_trust_bytes)?;
    validate_deployment_trust(&trust, deployment.network_id)
}
fn unchanged_authority(before: &BindingV1, after: &BindingV1) -> Result<()> {
    let old: NativePolicyV1 = json::from_slice(before.policy_bytes.as_bytes())?;
    let new: NativePolicyV1 = json::from_slice(after.policy_bytes.as_bytes())?;
    let old_trust: DeploymentTrustV1 = json::from_slice(before.observation_trust_bytes.as_bytes())?;
    let new_trust: DeploymentTrustV1 = json::from_slice(after.observation_trust_bytes.as_bytes())?;
    let origins = |trust: &DeploymentTrustV1| {
        trust
            .peers
            .iter()
            .map(|p| (p.peer_id.clone(), p.torii_origin.clone()))
            .collect::<BTreeMap<_, _>>()
    };
    require(
        old.intent.network_id == new.intent.network_id
            && old.intent.administrator == new.intent.administrator
            && old.intent.payment_asset == new.intent.payment_asset
            && old.intent.transaction_fee_maximum == new.intent.transaction_fee_maximum
            && old.intent.first_epoch == new.intent.first_epoch
            && old.intent.batch_epochs == new.intent.batch_epochs
            && old.intent.operation_timeout_ms == new.intent.operation_timeout_ms
            && old_trust.genesis_public_key == new_trust.genesis_public_key
            && old_trust.genesis_signed_wire_hex == new_trust.genesis_signed_wire_hex
            && origins(&old_trust) == origins(&new_trust)
            && before.custody_bytes == after.custody_bytes,
        "update cannot change original authority, genesis, roster, origins or retained seed custody",
    )
}

impl PrepareEpochUpdate {
    pub(in super::super) fn run<W: Write>(&self, output: &mut W) -> Result<()> {
        #[cfg(unix)]
        {
            self.prepare(output)
        }
        #[cfg(not(unix))]
        {
            let _ = output;
            Err(eyre!(
                "epoch input preparation requires a Unix operator host"
            ))
        }
    }

    fn build(
        &self,
        deployment: &DeploymentV1,
        prepared: &PreparedV1,
        trust: &DeploymentTrustV1,
        trust_bytes: &[u8],
        before: Option<BindingV1>,
        installed: Option<BindingV1>,
    ) -> Result<PreparationV1> {
        validate_deployment(deployment)?;
        validate_prepared(prepared, &deployment.current.commit)?;
        update_operation(&self.operation, &deployment.current.attempt_name)?;
        epoch_generation::intent(
            &self.original_service_state,
            &self.successor_service_state,
            before.is_some(),
        )?;
        require(
            matches!(self.installed_state.as_str(), "absent" | "present")
                && installed.is_some() == (self.installed_state == "present")
                && self.authorization == "until-stopped"
                && self.worker_timeout_ms > 0,
            "explicit installed state, ongoing authority and finite worker invocation required",
        )?;
        validate_deployment_trust(trust, deployment.network_id)?;
        for (binding, baseline) in before
            .iter()
            .map(|b| (b, true))
            .chain(installed.iter().map(|b| (b, false)))
        {
            validate_binding(binding, deployment, baseline)?;
        }
        let (original_seed_sources, custody) =
            seed_references(trust, &self.original_seed_sources, deployment.network_id)?;
        let release = format!(
            "{}/release-{}-{}/bin",
            deployment.runtime_root, prepared.commit, self.operation
        );
        let policy = NativePolicyV1 {
            schema_version: 1,
            intent: OngoingIntentV1 {
                authorization: "until_stopped".into(),
                network_id: deployment.network_id,
                administrator: AccountId::parse_encoded(&self.administrator)?,
                payment_asset: self.payment_asset.parse()?,
                transaction_fee_maximum: self.transaction_fee_maximum.parse()?,
                first_epoch: self.first_epoch,
                batch_epochs: self.batch_epochs,
                operation_timeout_ms: self.operation_timeout_ms,
            },
            release_source_commit: prepared.commit.clone(),
            iroha_sha256: prepared.artifacts[1].sha256.clone(),
            kagami: KagamiV1 {
                path: format!("{release}/kagami"),
                sha256: prepared.artifacts[3].sha256.clone(),
            },
            observation_trust_sha256: sha256_hex(trust_bytes),
            provision_timeout_ms: self.provision_timeout_ms,
        };
        let policy_bytes = json::to_vec(&policy)?;
        let policy_sha256 = sha256_hex(&policy_bytes);
        let generation = format!(
            "{}/generations/{policy_sha256}",
            epoch_supervisor::STATE_ROOT
        );
        let custody_bytes = json::to_vec(&custody)?;
        // Public projection only. Credential digests are absent, never invented or output.
        // The materializer derives and validates actual private custody before its receipt.
        let mut plan = EpochSupervisorPlanV1 {
            schema: "iroha.taira.public-reset.epoch-supervisor-plan.v1".into(),
            host_slug: "generation-only".into(),
            unit_name: epoch_supervisor::UNIT_NAME.into(),
            state_root: epoch_supervisor::STATE_ROOT.into(),
            journal_dir: epoch_supervisor::JOURNAL_DIR.into(),
            release_source_commit: prepared.commit.clone(),
            iroha_sha256: policy.iroha_sha256.clone(),
            kagami_sha256: policy.kagami.sha256.clone(),
            policy_sha256,
            policy_bytes,
            observation_trust_sha256: sha256_hex(trust_bytes),
            observation_trust_bytes: trust_bytes.to_vec(),
            custody_sha256: sha256_hex(&custody_bytes),
            custody_bytes,
            original_seed_sources: Vec::new(),
            unit_sha256: String::new(),
            unit_bytes: Vec::new(),
            admin_config_path: format!("{generation}/administrator.toml"),
            admin_config_sha256: String::new(),
            http_operator_key_path: format!("{generation}/http-operator.key"),
            http_operator_key_sha256: String::new(),
            policy_path: format!("{generation}/policy.json"),
            trust_path: format!("{generation}/trust.json"),
            custody_path: format!("{generation}/custody.json"),
            timeout_ms: self.worker_timeout_ms,
            prior_state: self.original_service_state.clone(),
            prior: None,
        };
        plan.unit_bytes = epoch_supervisor::render_unit(&plan, &format!("{release}/iroha"))?;
        plan.unit_sha256 = sha256_hex(&plan.unit_bytes);
        let after = epoch_generation::binding_from_public_plan(&plan)?;
        for binding in before.iter().chain(installed.iter()) {
            unchanged_authority(binding, &after)?;
        }
        Ok(PreparationV1 {
            schema: "taira.epoch-supervisor-generation-preparation.v1".into(),
            operation: self.operation.clone(),
            original_service_state: self.original_service_state.clone(),
            successor_service_state: self.successor_service_state.clone(),
            before,
            installed,
            after,
            original_seed_sources,
        })
    }

    #[cfg(unix)]
    fn prepare<W: Write>(&self, output: &mut W) -> Result<()> {
        let _guard = ChainDiscriminantGuard::enter(super::super::CHAIN_DISCRIMINANT);
        let (deployment, deployment_ref) = read_public::<DeploymentV1>(&self.deployment)?;
        let (prepared, prepared_ref) = read_public::<PreparedV1>(&self.prepared_result)?;
        let (trust, trust_bytes) =
            super::super::read_json::<DeploymentTrustV1>(&self.trust, "epoch observation trust")?;
        let (before, before_ref) = read_optional(&self.before_binding)?;
        let (installed, installed_ref) = read_optional(&self.installed_binding)?;
        let preparation = self.build(
            &deployment,
            &prepared,
            &trust,
            &trust_bytes,
            before,
            installed,
        )?;
        let preparation_bytes = json::to_vec(&preparation)?;
        let binding_bytes = json::to_vec(&preparation.after)?;
        let record = OutputV1 {
            schema: "iroha.taira.epoch-update-inputs.v1".into(),
            operation: self.operation.clone(),
            deployment: deployment_ref,
            prepared_result: prepared_ref,
            trust: reference(&self.trust, &trust_bytes)?,
            before_binding: before_ref,
            installed_binding: installed_ref,
            preparation: reference(&self.output.join("preparation.json"), &preparation_bytes)?,
            after_binding: reference(&self.output.join("after-binding.json"), &binding_bytes)?,
            credential_contents_read: false,
            seed_contents_read: false,
            host_contacted: false,
            native_materialization_required: true,
        };
        let record_bytes = json::to_vec(&record)?;
        publish_bundle(
            &self.output,
            &[
                ("preparation.json", &preparation_bytes),
                ("after-binding.json", &binding_bytes),
                ("inputs.json", &record_bytes),
            ],
        )?;
        output.write_all(&record_bytes)?;
        output.write_all(b"\n")?;
        Ok(())
    }
}

fn reference(path: &Path, bytes: &[u8]) -> Result<ReferenceV1> {
    let path = path
        .to_str()
        .ok_or_else(|| eyre!("public input path must be UTF-8"))?;
    public_path(path)?;
    Ok(ReferenceV1 {
        path: path.into(),
        sha256: sha256_hex(bytes),
    })
}
fn read_public<T: JsonDeserialize>(path: &Path) -> Result<(T, ReferenceV1)> {
    let (value, bytes) = super::super::read_json(path, "epoch update public input")?;
    Ok((value, reference(path, &bytes)?))
}
fn read_optional(path: &Option<PathBuf>) -> Result<(Option<BindingV1>, Option<ReferenceV1>)> {
    match path {
        Some(path) => {
            let (value, reference) = read_public(path)?;
            Ok((Some(value), Some(reference)))
        }
        None => Ok((None, None)),
    }
}
#[cfg(unix)]
fn publish_bundle(path: &Path, files: &[(&str, &[u8])]) -> Result<()> {
    public_path(
        path.to_str()
            .ok_or_else(|| eyre!("output path must be UTF-8"))?,
    )?;
    let parent = path
        .parent()
        .ok_or_else(|| eyre!("output parent missing"))?;
    validate_owner_private_dir(parent, "epoch update output parent")?;
    require(
        fs::symlink_metadata(path).is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound),
        "epoch update output must be fresh",
    )?;
    let temporary = tempfile::Builder::new()
        .prefix(".epoch-update-inputs-")
        .tempdir_in(parent)?;
    fs::set_permissions(temporary.path(), fs::Permissions::from_mode(0o700))?;
    for (name, bytes) in files {
        super::super::inputs::write_new_private(&temporary.path().join(name), bytes)?;
    }
    validate_owner_private_dir(parent, "epoch update output parent")?;
    rustix::fs::renameat_with(
        rustix::fs::CWD,
        temporary.path(),
        rustix::fs::CWD,
        path,
        rustix::fs::RenameFlags::NOREPLACE,
    )
    .wrap_err("publish fresh epoch update inputs")?;
    File::open(parent)?.sync_all()?;
    Ok(())
}

#[cfg(test)]
#[path = "taira_public_reset_epoch_update_inputs_tests.rs"]
mod tests;
