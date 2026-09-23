//! One fixed cohort epoch supervisor: public admission and reset lifecycle barriers.
//!
//! Ongoing authority is independent of the finite reset lease. No readiness
//! marker, active service, or old completion authorizes a new transaction. The
//! native status command authenticates both initial and current completion;
//! policy, durable lifecycle owner and process incarnation remain separate gates.

use super::*;

pub(in super::super) const UNIT_NAME: &str = "iroha-taira-epoch-supervisor.service";
pub(in super::super) const STATE_ROOT: &str = "/var/lib/taira-epoch-supervisor";
pub(in super::super) const JOURNAL_DIR: &str = "/var/lib/taira-epoch-supervisor/journals";
const PLAN_SCHEMA: &str = "iroha.taira.public-reset.epoch-supervisor-plan.v1";
const PUBLIC_LIMIT: usize = 1024 * 1024;

/// Required owner-selected public service closure, never inferred from canary inputs.
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(in super::super) struct EpochSupervisorPlanV1 {
    pub(in super::super) schema: String,
    pub(in super::super) host_slug: String,
    pub(in super::super) unit_name: String,
    pub(in super::super) state_root: String,
    pub(in super::super) journal_dir: String,
    pub(in super::super) release_source_commit: String,
    pub(in super::super) iroha_sha256: String,
    pub(in super::super) cli_path: String,
    pub(in super::super) policy_sha256: String,
    pub(in super::super) policy_bytes: Vec<u8>,
    pub(in super::super) observation_trust_sha256: String,
    pub(in super::super) observation_trust_bytes: Vec<u8>,
    pub(in super::super) unit_sha256: String,
    pub(in super::super) unit_bytes: Vec<u8>,
    pub(in super::super) admin_config_path: String,
    pub(in super::super) admin_config_sha256: String,
    pub(in super::super) http_operator_key_path: String,
    pub(in super::super) http_operator_key_sha256: String,
    pub(in super::super) policy_path: String,
    pub(in super::super) trust_path: String,
    pub(in super::super) timeout_ms: u64,
    pub(in super::super) prior_state: String,
    #[norito(required)]
    pub(in super::super) prior: Option<PriorEpochSupervisorV1>,
}

/// Exact immediate predecessor plan, retained independently from current service state.
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(in super::super) struct PriorEpochSupervisorV1 {
    pub(in super::super) plan_bytes: Vec<u8>,
    pub(in super::super) plan_sha256: String,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(super) struct NativePolicyV1 {
    pub(super) schema_version: u8,
    pub(super) intent: OngoingIntentV1,
    pub(super) release_source_commit: String,
    pub(super) iroha_sha256: String,
    pub(super) observation_trust_sha256: String,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(super) struct OngoingIntentV1 {
    pub(super) authorization: String,
    pub(super) network_id: NetworkId,
    pub(super) administrator: AccountId,
    pub(super) first_epoch: u64,
}
/// Public worker identity emitted by the native supervisor, not caller authority.
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct WorkerV1 {
    boot_id: String,
    pid: u32,
    start_time_ticks: u64,
}

fn digest(value: &str) -> Result<()> {
    if value.len() != 64
        || value
            .bytes()
            .any(|v| !v.is_ascii_digit() && !(b'a'..=b'f').contains(&v))
        || value.bytes().all(|v| v == b'0')
    {
        return Err(eyre!(
            "epoch supervisor requires an exact nonzero lowercase digest"
        ));
    }
    Ok(())
}
fn literal_path(value: &str) -> Result<()> {
    if !value.starts_with('/')
        || value.ends_with('/')
        || value.contains("//")
        || value
            .split('/')
            .skip(1)
            .any(|p| p.is_empty() || p == "." || p == "..")
        || value
            .bytes()
            .any(|v| !v.is_ascii_alphanumeric() && !b"/_ .:@+-".contains(&v))
        || value.contains(' ')
    {
        return Err(eyre!(
            "epoch supervisor path is not a direct absolute literal"
        ));
    }
    Ok(())
}
fn public_bytes(bytes: &[u8], expected: &str) -> Result<()> {
    digest(expected)?;
    if bytes.is_empty() || bytes.len() > PUBLIC_LIMIT || sha256_hex(bytes) != expected {
        return Err(eyre!("epoch supervisor public original differs"));
    }
    Ok(())
}
pub(super) fn validate_generation(plan: &EpochSupervisorPlanV1) -> Result<NativePolicyV1> {
    // Only the native credential consumer or fully pinned reset plan can pass this gate.
    digest(&plan.admin_config_sha256)?;
    digest(&plan.http_operator_key_sha256)?;
    validate_public_generation(plan)
}

/// Validate only the complete public closure. This confers no credential or runtime authority.
pub(super) fn validate_public_generation(plan: &EpochSupervisorPlanV1) -> Result<NativePolicyV1> {
    let _chain_guard = ChainDiscriminantGuard::enter(super::super::CHAIN_DISCRIMINANT);
    if plan.schema != PLAN_SCHEMA
        || plan.unit_name != UNIT_NAME
        || plan.state_root != STATE_ROOT
        || plan.journal_dir != JOURNAL_DIR
        || plan.timeout_ms == 0
    {
        return Err(eyre!(
            "epoch supervisor fixed service or finite invocation differs"
        ));
    }
    digest(&plan.iroha_sha256)?;
    for (bytes, hash) in [
        (&plan.policy_bytes, &plan.policy_sha256),
        (
            &plan.observation_trust_bytes,
            &plan.observation_trust_sha256,
        ),
        (&plan.unit_bytes, &plan.unit_sha256),
    ] {
        public_bytes(bytes, hash)?;
    }
    let generation = format!("{STATE_ROOT}/generations/{}", plan.policy_sha256);
    for (actual, name) in [
        (&plan.admin_config_path, "administrator.toml"),
        (&plan.http_operator_key_path, "http-operator.key"),
        (&plan.policy_path, "policy.json"),
        (&plan.trust_path, "trust.json"),
    ] {
        literal_path(actual)?;
        if actual != &format!("{generation}/{name}") {
            return Err(eyre!(
                "epoch supervisor input escapes its immutable policy generation"
            ));
        }
    }
    let policy: NativePolicyV1 = json::from_slice(&plan.policy_bytes)?;
    if policy.schema_version != 1
        || policy.intent.authorization != "until_stopped"
        || policy.release_source_commit != plan.release_source_commit
        || policy.iroha_sha256 != plan.iroha_sha256
        || policy.observation_trust_sha256 != plan.observation_trust_sha256
        || policy.release_source_commit.len() != 40
        || policy
            .release_source_commit
            .bytes()
            .any(|v| !v.is_ascii_digit() && !(b'a'..=b'f').contains(&v))
        || policy.intent.first_epoch == 0
    {
        return Err(eyre!("epoch supervisor explicit ongoing policy differs"));
    }
    if plan.unit_bytes != render_unit(plan, &plan.cli_path)? {
        return Err(eyre!(
            "epoch supervisor unit differs from fixed native argv and lifecycle policy"
        ));
    }
    Ok(policy)
}

/// Validate public policy/source bindings without opening any credential or seed.
pub(in super::super) fn validate_plan(inventory: &InventoryV1) -> Result<()> {
    let _chain_guard = super::super::enter_inventory_chain_discriminant(inventory)?;
    validate_plan_context(
        &inventory.epoch_supervisor,
        &inventory.revision,
        &inventory.validators,
        &inventory.validator_clients,
        &inventory.maintenance_admin_identity,
        &inventory.maintenance_admin_config_sha256,
        &inventory.beacon_bootstrap.genesis_public_key,
    )
}

/// Same public admission for a generated plan before beacon/inventory assembly.
pub(super) fn validate_plan_context(
    plan: &EpochSupervisorPlanV1,
    revision: &super::super::RevisionV1,
    validators: &[ValidatorV1],
    clients: &[super::super::ValidatorClientV1],
    admin: &super::super::MaintenanceAdminIdentityV1,
    admin_config_sha256: &str,
    genesis_public_key: &iroha_crypto::PublicKey,
) -> Result<()> {
    let _chain_guard = ChainDiscriminantGuard::enter(super::super::CHAIN_DISCRIMINANT);
    let policy = validate_generation(plan)?;
    let nominated = validators
        .iter()
        .find(|v| v.slug == plan.host_slug)
        .ok_or_else(|| eyre!("epoch supervisor host is not in the current cohort"))?;
    if validators.len() != 4
        || validators
            .iter()
            .any(|v| v.endpoint.host_identity_sha256 != nominated.endpoint.host_identity_sha256)
        || plan.release_source_commit != revision.commit
        || plan.admin_config_sha256 != admin_config_sha256
        || policy.intent.administrator.to_string() != admin.account_id
        || policy.intent.network_id.to_string() != admin.network_id
    {
        return Err(eyre!(
            "epoch supervisor requires the admitted four-member host and separate administrator"
        ));
    }
    let trust: crate::taira_dataspace_deploy::DeploymentTrustV1 =
        json::from_slice(&plan.observation_trust_bytes)?;
    if trust.peers.len() != 4
        || !trust
            .peers
            .iter()
            .any(|p| p.torii_origin == admin.torii_origin)
        || trust.genesis_public_key != *genesis_public_key
    {
        return Err(eyre!(
            "maintenance administrator submission origin is not in the selected signed observation trust"
        ));
    }
    let mut selected = BTreeSet::new();
    for client in clients {
        let peer = client.peer_id.parse::<PeerId>()?;
        let validator = validators
            .iter()
            .find(|v| v.slug == client.slug)
            .ok_or_else(|| eyre!("trust validator role missing"))?;
        let observed = trust
            .peers
            .iter()
            .find(|p| p.peer_id == peer)
            .ok_or_else(|| eyre!("observation trust omits current peer"))?;
        if !selected.insert(peer)
            || observed.node_fingerprint.to_string() != validator.node_fingerprint
            || observed.build_fingerprint.to_string() != validator.build_fingerprint
            || observed.config_fingerprint.to_string() != validator.config_fingerprint
            || ![client.torii_origin.as_str(), client.probe_origin.as_str()]
                .contains(&observed.torii_origin.as_str())
        {
            return Err(eyre!(
                "observation trust peer identity, build, config or exact origin differs from signed inventory"
            ));
        }
    }
    let release = Path::new(&nominated.service_root)
        .join("releases")
        .join(&revision.commit);
    let cli = artifact(&nominated.artifacts, "iroha_cli")?;
    if Path::new(&plan.cli_path) != release.join("bin/iroha")
        || plan.cli_path != cli.remote_path
        || cli.sha256 != plan.iroha_sha256
    {
        return Err(eyre!(
            "epoch supervisor CLI differs from the exact signed release"
        ));
    }
    for validator in validators {
        for protected in [&validator.state_root, &validator.reset_guard] {
            let path = Path::new(protected);
            if Path::new(STATE_ROOT).starts_with(path) || path.starts_with(STATE_ROOT) {
                return Err(eyre!(
                    "epoch supervisor persistent state overlaps finite validator/reset state"
                ));
            }
        }
    }
    predecessor(plan)?;
    Ok(())
}

/// Exact fixed systemd unit grammar shared with the public renderer; never reads inputs.
pub(super) fn render_unit(plan: &EpochSupervisorPlanV1, cli: &str) -> Result<Vec<u8>> {
    literal_path(cli)?;
    if Path::new(cli).file_name() != Some(OsStr::new("iroha"))
        || Path::new(cli).parent().and_then(Path::file_name) != Some(OsStr::new("bin"))
    {
        return Err(eyre!(
            "epoch supervisor CLI must be exact release bin/iroha"
        ));
    }
    let timeout = plan.timeout_ms.to_string();
    let args = [
        cli,
        "--config",
        &plan.admin_config_path,
        "--operator-private-key-file",
        &plan.http_operator_key_path,
        "taira",
        "epoch-maintenance",
        "supervise",
        "--policy",
        &plan.policy_path,
        "--trust",
        &plan.trust_path,
        "--journal-dir",
        JOURNAL_DIR,
        "--timeout-ms",
        &timeout,
    ];
    let argv = args
        .iter()
        .map(|value| format!("\"{value}\""))
        .collect::<Vec<_>>()
        .join(" ");
    Ok(format!("[Unit]\nDescription=Taira epoch maintenance supervisor\nAfter=network-online.target\nStartLimitIntervalSec=300s\nStartLimitBurst=3\n\n[Service]\nType=exec\nUser=root\nGroup=root\nUMask=0077\nWorkingDirectory={STATE_ROOT}\nNoNewPrivileges=true\nPrivateTmp=true\nProtectSystem=strict\nProtectHome=true\nReadWritePaths={STATE_ROOT}\nRestart=on-failure\nRestartPreventExitStatus=3 4 7\nRestartSec=5s\nKillMode=control-group\nTimeoutStopSec=30s\nExecStart={argv}\n\n[Install]\nWantedBy=multi-user.target\n").into_bytes())
}

pub(super) fn pin_runtime_administrator(
    path: &Path,
    inventory: &InventoryV1,
) -> Result<super::super::PinnedInput> {
    let input = pin_owner_private_file(path, "maintenance administrator config")?;
    validate_runtime_administrator(&input, inventory)?;
    Ok(input)
}
pub(super) fn validate_runtime_administrator(
    input: &super::super::PinnedInput,
    inventory: &InventoryV1,
) -> Result<()> {
    if hash_pinned_input(input, "maintenance administrator config", None)?
        != inventory.maintenance_admin_config_sha256
    {
        return Err(eyre!(
            "maintenance administrator bytes differ from signed admission"
        ));
    }
    let config =
        load_client_config_for_inventory(input, "maintenance administrator config", inventory)?;
    let identity = &inventory.maintenance_admin_identity;
    if config.account.to_string() != identity.account_id
        || config.key_pair.public_key().to_string() != identity.public_key
        || config.network_id.to_string() != identity.network_id
        || config.account_chain_discriminant != identity.chain_discriminant
        || config.torii_api_url.as_str() != identity.torii_origin
    {
        return Err(eyre!(
            "maintenance administrator identity/origin differs from signed native genesis admission"
        ));
    }
    revalidate_pinned(input, "maintenance administrator config")
}

type JournalGuard = crate::taira_dataspace_deploy::epoch_maintenance::SupervisorJournalGuard;
const OWNER_FILE: &str = ".reset-owner.json";
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct ResetOwnerV1 {
    schema: String,
    authorization_nonce: String,
    inventory_sha256: String,
    policy_sha256: String,
}
fn owner(admitted: &HostAdmission) -> ResetOwnerV1 {
    ResetOwnerV1 {
        schema: "iroha.taira.epoch-supervisor.reset-owner.v1".into(),
        authorization_nonce: admitted.inventory.authorization_nonce.clone(),
        inventory_sha256: admitted.inventory_sha256.clone(),
        policy_sha256: admitted.inventory.epoch_supervisor.policy_sha256.clone(),
    }
}
fn selected_on_this_host(admitted: &HostAdmission) -> bool {
    admitted
        .inventory
        .validators
        .iter()
        .find(|v| v.slug == admitted.inventory.epoch_supervisor.host_slug)
        .is_some_and(|v| {
            v.endpoint.host_identity_sha256 == admitted.target.endpoint().host_identity_sha256
        })
}
fn require_owner(admitted: &HostAdmission) -> Result<()> {
    let (actual, _) = read_private_json::<ResetOwnerV1>(
        &Path::new(STATE_ROOT).join(OWNER_FILE),
        "epoch reset lifecycle owner",
    )?;
    if actual != owner(admitted) {
        return Err(eyre!("epoch lifecycle is owned by another operation"));
    }
    Ok(())
}
fn persistent_name(admitted: &HostAdmission, suffix: &str) -> String {
    format!(
        "reset-{}-{suffix}.json",
        admitted.inventory.authorization_nonce
    )
}
fn exact_retained_owner(admitted: &HostAdmission, suffix: &str) -> Result<bool> {
    let path = Path::new(STATE_ROOT).join(persistent_name(admitted, suffix));
    match fs::symlink_metadata(&path) {
        Ok(_) => {
            let (value, _) =
                read_private_json::<ResetOwnerV1>(&path, "retained epoch lifecycle evidence")?;
            if value != owner(admitted) {
                return Err(eyre!("retained epoch lifecycle evidence conflicts"));
            }
            Ok(true)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}
/// Common lock covers each reset dispatcher; durable owner excludes ordinary updater/reset interleaving between calls.
/// This does not claim a single kernel flock is held across separate SSH processes.
pub(super) fn lifecycle_lock(admitted: &HostAdmission, action: HostAction) -> Result<Option<File>> {
    if !selected_on_this_host(admitted) {
        return Ok(None);
    }
    let lock = epoch_generation::acquire_deployment_lock(admitted.action_deadline)?;
    let path = Path::new(STATE_ROOT).join(OWNER_FILE);
    match fs::symlink_metadata(&path) {
        Ok(_) => require_owner(admitted)?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let sealed = exact_retained_owner(admitted, "sealed")?;
            let restored = exact_retained_owner(admitted, "restored")?;
            if sealed && matches!(action, HostAction::Seal | HostAction::Cleanup)
                || restored
                    && matches!(
                        action,
                        HostAction::Rollback
                            | HostAction::EpochSupervisorRollbackPause
                            | HostAction::EpochSupervisorRollbackRestore
                    )
            {
                sync_directory(Path::new(STATE_ROOT))?;
                return Ok(Some(lock));
            }
            if exact_retained_owner(admitted, "intent")?
                || action.is_supervisor_rollback()
                || action == HostAction::Rollback
                || admitted.execution_expired
                || admitted.request.recovery_only
            {
                return Err(eyre!(
                    "epoch lifecycle recovery cannot recreate missing or lost ownership"
                ));
            }
            preflight(admitted)?;
            publish_root_private_noreplace(
                Path::new(STATE_ROOT),
                OWNER_FILE,
                json::to_json(&owner(admitted))?.as_bytes(),
            )?;
            require_owner(admitted)?;
        }
        Err(error) => return Err(error.into()),
    }
    publish_root_private_noreplace(
        Path::new(STATE_ROOT),
        &persistent_name(admitted, "intent"),
        json::to_json(&owner(admitted))?.as_bytes(),
    )?;
    Ok(Some(lock))
}
fn release_owner(admitted: &HostAdmission, terminal: &str) -> Result<()> {
    require_owner(admitted)?;
    publish_root_private_noreplace(
        Path::new(STATE_ROOT),
        &persistent_name(admitted, terminal),
        json::to_json(&owner(admitted))?.as_bytes(),
    )?;
    require_owner(admitted)?;
    let parent = File::from(rustix::fs::open(
        Path::new(STATE_ROOT),
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::DIRECTORY
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )?);
    rustix::fs::unlinkat(&parent, OWNER_FILE, rustix::fs::AtFlags::empty())?;
    parent.sync_all()?;
    Ok(())
}
/// Called after durable host frontier advancement, never by generic generated-waste cleanup.
pub(super) fn finish_seal(admitted: &HostAdmission, progress: &HostProgressV1) -> Result<()> {
    if selected_on_this_host(admitted) && progress.sealed {
        if exact_retained_owner(admitted, "sealed")?
            && !Path::new(STATE_ROOT).join(OWNER_FILE).try_exists()?
        {
            return Ok(());
        }
        require_ready(admitted)?;
        release_owner(admitted, "sealed")?;
    }
    Ok(())
}
fn predecessor(plan: &EpochSupervisorPlanV1) -> Result<Option<EpochSupervisorPlanV1>> {
    match (plan.prior_state.as_str(), &plan.prior) {
        ("absent", None) => Ok(None),
        ("running" | "stopped", Some(pin)) => {
            public_bytes(&pin.plan_bytes, &pin.plan_sha256)?;
            let prior: EpochSupervisorPlanV1 = json::from_slice(&pin.plan_bytes)?;
            validate_generation(&prior)?;
            if prior.host_slug != plan.host_slug {
                return Err(eyre!("epoch supervisor predecessor host differs"));
            }
            Ok(Some(prior))
        }
        _ => Err(eyre!(
            "epoch supervisor requires explicit absent/running/stopped predecessor closure"
        )),
    }
}

/// Protect the independently authenticated supervisor tool release on every host.
/// Its signed literal path need not share a validator's configuration or daemon root.
pub(super) fn protects_prior_release(plan: &EpochSupervisorPlanV1, path: &Path) -> Result<bool> {
    let Some(prior) = predecessor(plan)? else {
        return Ok(false);
    };
    validate_generation(&prior)?;
    let release = Path::new(&prior.cli_path)
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| eyre!("epoch supervisor tool release root missing"))?;
    // The signed plan binds the CLI pathname, policy and unit. Keep its complete release.
    Ok(release.starts_with(path) || path.starts_with(release))
}

fn nominated_admission(admitted: &HostAdmission) -> Result<HostAdmission> {
    let validator = admitted
        .inventory
        .validators
        .iter()
        .find(|v| v.slug == admitted.inventory.epoch_supervisor.host_slug)
        .ok_or_else(|| eyre!("epoch supervisor host missing"))?;
    admission_for_validator(admitted, validator)
}
fn admission_for_validator(
    admitted: &HostAdmission,
    validator: &ValidatorV1,
) -> Result<HostAdmission> {
    let (guard, bytes) = read_private_json::<HostGuardV1>(
        &Path::new(&validator.reset_guard).join("guard.json"),
        "cohort epoch host guard",
    )?;
    if sha256_hex(&bytes) != validator.endpoint.upload_guard_sha256
        || guard.host_slug != validator.slug
        || guard.service_root != validator.service_root
        || guard.state_root != validator.state_root
        || guard.trusted_key_sha256 != admitted.request.trusted_key_sha256
    {
        return Err(eyre!("cohort epoch target guard differs from admission"));
    }
    let mut selected = admitted.clone();
    selected.target = HostTarget::Validator(validator.clone());
    selected.guard = guard;
    selected.request.host_slug = validator.slug.clone();
    Ok(selected)
}

/// Static public closure and native original generation/custody admission, before any validator mutation.
pub(super) fn preflight(admitted: &HostAdmission) -> Result<()> {
    validate_plan(&admitted.inventory)?;
    if selected_on_this_host(admitted) {
        let plan = &admitted.inventory.epoch_supervisor;
        epoch_generation::preflight_reset_plan(plan, admitted.action_deadline)?;
        match predecessor(plan)? {
            None => {
                exact_unit_absent(admitted.action_deadline)?;
                match fs::symlink_metadata(JOURNAL_DIR) {
                    Ok(_) => {
                        let guard =
                            epoch_generation::quiescent_generation(plan, admitted.action_deadline)?;
                        if let Some(guard) = &guard {
                            guard.revalidate()?;
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                        require_root_directory(
                            Path::new("/var/lib"),
                            false,
                            "epoch persistent state ancestor",
                        )?;
                    }
                    Err(error) => return Err(error.into()),
                }
            }
            Some(prior) => {
                epoch_generation::preflight_generation(&prior, admitted.action_deadline)?;
                verify_regular_hash(Path::new(UNIT_PATH), &prior.unit_sha256)?;
                if plan.prior_state == "running" {
                    epoch_generation::observe_generation(&prior, admitted.action_deadline)?;
                } else {
                    let guard =
                        epoch_generation::quiescent_generation(&prior, admitted.action_deadline)?;
                    if let Some(guard) = &guard {
                        guard.revalidate()?;
                    }
                }
            }
        }
    }
    Ok(())
}

fn nominated(admitted: &HostAdmission) -> Result<()> {
    validate_plan(&admitted.inventory)?;
    if admitted.target.slug() != admitted.inventory.epoch_supervisor.host_slug {
        return Err(eyre!(
            "epoch supervisor action must use its single nominated host target"
        ));
    }
    Ok(())
}

/// No second submission on a lost reply, absent operation, timeout or recovery.
fn manager_once(
    admitted: &HostAdmission,
    label: &str,
    verb: &str,
    recovery_only: bool,
) -> Result<()> {
    manager_once_target(admitted, label, verb, UNIT_NAME, recovery_only)
}
fn manager_once_target(
    admitted: &HostAdmission,
    label: &str,
    verb: &str,
    target: &str,
    recovery_only: bool,
) -> Result<()> {
    let path = ensure_host_receipt_dir(admitted)?.join(manager_intent_name(label)?);
    let intent = if recovery_only {
        let (intent, _) =
            read_private_json::<ManagerIntentV1>(&path, "epoch supervisor manager intent")?;
        validate_manager_intent_for_session(admitted, label, verb, target, &intent)?;
        intent
    } else {
        let (intent, created) = ensure_manager_intent(admitted, label, verb, target)?;
        if created {
            submit_durable_manager_operation(admitted, &intent)?;
        }
        intent
    };
    loop {
        ensure_action_deadline(admitted)?;
        match inspect_manager_operation(&intent, admitted.action_deadline)? {
            ManagerOperationEvidence::Applied => return Ok(()),
            ManagerOperationEvidence::Rejected => {
                return Err(eyre!("epoch supervisor manager operation rejected"));
            }
            ManagerOperationEvidence::Absent | ManagerOperationEvidence::Pending => {
                if recovery_only {
                    return Err(LocalMutationRecoveryPending {
                        action: "epoch_supervisor_manager",
                    }
                    .into());
                }
                let remaining = admitted
                    .action_deadline
                    .saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    return Err(LocalMutationRecoveryPending {
                        action: "epoch_supervisor_manager",
                    }
                    .into());
                }
                std::thread::sleep(PROCESS_POLL_INTERVAL.min(remaining));
            }
        }
    }
}

/// Exact native-only opaque credential body; no shell/Python parsing or output of private bytes.
pub(super) fn materialize_stream(admitted: &HostAdmission, body: &mut impl Read) -> Result<()> {
    nominated(admitted)?;
    require_owner(admitted)?;
    let guard = require_forward_pause(admitted)?;
    let mut header = [0_u8; 16];
    body.read_exact(&mut header)?;
    let administrator_len = u64::from_be_bytes(header[..8].try_into()?);
    let operator_len = u64::from_be_bytes(header[8..16].try_into()?);
    if administrator_len == 0
        || administrator_len > iroha_config_base::toml::MAX_TOML_SOURCE_BYTES
        || operator_len == 0
        || operator_len > 4096
    {
        return Err(eyre!(
            "epoch generation opaque stream exceeds native bounds"
        ));
    }
    let mut administrator = zeroize::Zeroizing::new(vec![0; usize::try_from(administrator_len)?]);
    let mut operator = zeroize::Zeroizing::new(vec![0; usize::try_from(operator_len)?]);
    body.read_exact(&mut administrator)?;
    body.read_exact(&mut operator)?;
    require_stream_eof(body)?;
    let plan = &admitted.inventory.epoch_supervisor;
    if sha256_hex(&administrator) != plan.admin_config_sha256
        || sha256_hex(&operator) != plan.http_operator_key_sha256
    {
        return Err(eyre!(
            "epoch generation native opaque stream differs from signed custody"
        ));
    }
    ensure_action_deadline(admitted)?;
    validate_generation(plan)?;
    epoch_generation::materialize_reset_generation(
        plan,
        &administrator,
        &operator,
        &admitted.inventory.authorization_nonce,
        admitted.action_deadline,
    )?;
    if let Some(guard) = &guard {
        guard.revalidate()?;
    }
    Ok(())
}

const UNIT_PATH: &str = "/etc/systemd/system/iroha-taira-epoch-supervisor.service";
fn install_unit_bytes(
    admitted: &HostAdmission,
    selected: &EpochSupervisorPlanV1,
    suffix: &str,
) -> Result<()> {
    let name = format!(
        "reset-{}-{suffix}.service",
        admitted.inventory.authorization_nonce
    );
    publish_root_private_noreplace(Path::new(STATE_ROOT), &name, &selected.unit_bytes)?;
    let source = Path::new(STATE_ROOT).join(name);
    let artifact = ArtifactV1 {
        role: "epoch_supervisor_unit".into(),
        local_path: source.display().to_string(),
        remote_path: UNIT_PATH.into(),
        sha256: selected.unit_sha256.clone(),
        size: u64::try_from(selected.unit_bytes.len())?,
        mode: 0o644,
        source_commit: selected.release_source_commit.clone(),
        target: super::super::BUILD_TARGET.into(),
    };
    atomic_replace_verified_file(admitted, &source, Path::new(UNIT_PATH), &artifact)?;
    verify_regular_hash(Path::new(UNIT_PATH), &selected.unit_sha256)
}
fn publish_reset_unit(admitted: &HostAdmission) -> Result<()> {
    let plan = &admitted.inventory.epoch_supervisor;
    epoch_generation::preflight_generation(plan, admitted.action_deadline)?;
    let prior = predecessor(plan)?;
    match fs::symlink_metadata(UNIT_PATH) {
        Ok(_) => {
            if verify_regular_hash(Path::new(UNIT_PATH), &plan.unit_sha256).is_err() {
                let prior = prior.as_ref().ok_or_else(|| {
                    eyre!("unexpected existing epoch unit before first installation")
                })?;
                verify_regular_hash(Path::new(UNIT_PATH), &prior.unit_sha256)?;
            }
        }
        Err(error)
            if error.kind() == std::io::ErrorKind::NotFound && plan.prior_state == "absent" => {}
        Err(error) => return Err(error.into()),
    }
    install_unit_bytes(admitted, plan, "candidate")?;
    manager_once_target(
        admitted,
        "epoch-supervisor-unit-reload",
        "daemon-reload",
        "",
        false,
    )
}
fn restore_reset_unit(admitted: &HostAdmission) -> Result<()> {
    let plan = &admitted.inventory.epoch_supervisor;
    if plan.prior_state == "absent" && exact_retained_owner(admitted, "rollback-pause-absent")? {
        return exact_unit_absent(admitted.action_deadline);
    }
    if let Some(prior) = predecessor(plan)? {
        if verify_regular_hash(Path::new(UNIT_PATH), &prior.unit_sha256).is_err() {
            verify_regular_hash(Path::new(UNIT_PATH), &plan.unit_sha256)?;
        }
        install_unit_bytes(admitted, &prior, "predecessor")?;
    } else {
        match fs::symlink_metadata(UNIT_PATH) {
            Ok(_) => {
                verify_regular_hash(Path::new(UNIT_PATH), &plan.unit_sha256)?;
                let parent = File::from(rustix::fs::open(
                    Path::new("/etc/systemd/system"),
                    rustix::fs::OFlags::RDONLY
                        | rustix::fs::OFlags::DIRECTORY
                        | rustix::fs::OFlags::NOFOLLOW
                        | rustix::fs::OFlags::CLOEXEC,
                    rustix::fs::Mode::empty(),
                )?);
                rustix::fs::unlinkat(&parent, UNIT_NAME, rustix::fs::AtFlags::empty())?;
                parent.sync_all()?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                sync_directory(Path::new("/etc/systemd/system"))?;
            }
            Err(error) => return Err(error.into()),
        }
    }
    manager_once_target(
        admitted,
        "rollback-epoch-supervisor-unit-reload",
        "daemon-reload",
        "",
        false,
    )
}

fn exact_unit_absent(deadline: Instant) -> Result<()> {
    match fs::symlink_metadata(UNIT_PATH) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        _ => return Err(eyre!("epoch unit is not genuinely absent")),
    }
    let bytes = run_host_command(
        SYSTEMCTL,
        &[
            "show",
            "--property=LoadState",
            "--property=FragmentPath",
            "--property=DropInPaths",
            "--property=ActiveState",
            "--property=SubState",
            "--property=MainPID",
            "--property=ControlPID",
            "--property=Job",
            UNIT_NAME,
        ],
        deadline,
    )?;
    validate_absent_unit_evidence(&bytes)
}
fn validate_absent_unit_evidence(bytes: &[u8]) -> Result<()> {
    let mut fields = BTreeMap::new();
    for line in std::str::from_utf8(&bytes)?.lines() {
        let (name, value) = line
            .split_once('=')
            .ok_or_else(|| eyre!("epoch unit absence properties malformed"))?;
        if fields.insert(name, value).is_some() {
            return Err(eyre!("duplicate epoch unit absence property"));
        }
    }
    let expected = BTreeMap::from([
        ("LoadState", "not-found"),
        ("FragmentPath", ""),
        ("DropInPaths", ""),
        ("ActiveState", "inactive"),
        ("SubState", "dead"),
        ("MainPID", "0"),
        ("ControlPID", "0"),
        ("Job", ""),
    ]);
    if fields != expected {
        return Err(eyre!(
            "epoch unit has a loaded fragment, process, job or nonterminal state"
        ));
    }
    Ok(())
}
fn record_absent_pause(admitted: &HostAdmission, suffix: &str, recovery_only: bool) -> Result<()> {
    exact_unit_absent(admitted.action_deadline)?;
    let guard = epoch_generation::quiescent_generation(
        &admitted.inventory.epoch_supervisor,
        admitted.action_deadline,
    )?;
    if recovery_only && !exact_retained_owner(admitted, suffix)? {
        return Err(eyre!(
            "read-only recovery cannot invent an absent epoch pause"
        ));
    }
    if let Some(guard) = &guard {
        guard.revalidate()?;
    }
    if !recovery_only {
        publish_root_private_noreplace(
            Path::new(STATE_ROOT),
            &persistent_name(admitted, suffix),
            json::to_json(&owner(admitted))?.as_bytes(),
        )?;
    }
    Ok(())
}
fn reconcile_supervisor_operations(admitted: &HostAdmission) -> Result<()> {
    let directory = ensure_host_receipt_dir(admitted)?;
    for (label, verb, target) in [
        ("epoch-supervisor-pause", "stop", UNIT_NAME),
        ("epoch-supervisor-unit-reload", "daemon-reload", ""),
        ("epoch-supervisor-start", "start", UNIT_NAME),
        ("rollback-epoch-supervisor-pause", "stop", UNIT_NAME),
        ("rollback-epoch-supervisor-unit-reload", "daemon-reload", ""),
        ("rollback-epoch-supervisor-start", "start", UNIT_NAME),
    ] {
        let path = directory.join(manager_intent_name(label)?);
        match fs::symlink_metadata(&path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error.into()),
            Ok(_) => {}
        }
        let (intent, _) =
            read_private_json::<ManagerIntentV1>(&path, "prior epoch manager operation")?;
        validate_manager_intent_for_session(admitted, label, verb, target, &intent)?;
        match inspect_manager_operation(&intent, admitted.action_deadline)? {
            ManagerOperationEvidence::Applied | ManagerOperationEvidence::Rejected => {}
            ManagerOperationEvidence::Absent | ManagerOperationEvidence::Pending => {
                return Err(LocalMutationRecoveryPending {
                    action: "prior_epoch_supervisor_manager",
                }
                .into());
            }
        }
    }
    Ok(())
}

/// Forward cohort pause is an explicit action before every validator mutation.
pub(super) fn pause(admitted: &HostAdmission, recovery_only: bool) -> Result<()> {
    nominated(admitted)?;
    require_owner(admitted)?;
    if admitted.inventory.epoch_supervisor.prior_state == "absent" {
        if !recovery_only {
            ensure_root_private_directory(Path::new(JOURNAL_DIR))?;
        }
        return record_absent_pause(admitted, "pause-absent", recovery_only);
    }
    manager_once(admitted, "epoch-supervisor-pause", "stop", recovery_only)?;
    let guard = require_quiescence(admitted)?;
    if let Some(guard) = &guard {
        guard.revalidate()?;
    }
    Ok(())
}

/// Hold the native original worker journal lock for the entire caller mutation.
fn require_quiescence(admitted: &HostAdmission) -> Result<Option<JournalGuard>> {
    require_owner(admitted)?;
    let previous = predecessor(&admitted.inventory.epoch_supervisor)?;
    epoch_generation::quiescent_generation(
        previous
            .as_ref()
            .unwrap_or(&admitted.inventory.epoch_supervisor),
        admitted.action_deadline,
    )
}

pub(super) fn require_forward_pause(admitted: &HostAdmission) -> Result<Option<JournalGuard>> {
    if !selected_on_this_host(admitted) {
        return Ok(None);
    }
    require_owner(admitted)?;
    let selected = nominated_admission(admitted)?;
    if selected.inventory.epoch_supervisor.prior_state == "absent" {
        if !exact_retained_owner(&selected, "pause-absent")? {
            return Err(eyre!("first-install pause absence has no durable evidence"));
        }
        exact_unit_absent(selected.action_deadline)?;
    } else {
        require_session_manager_operation_applied(
            &selected,
            "epoch-supervisor-pause",
            "stop",
            UNIT_NAME,
        )?;
    }
    require_quiescence(&selected)
}
fn installed_generation(admitted: &HostAdmission) -> Result<EpochSupervisorPlanV1> {
    let plan = &admitted.inventory.epoch_supervisor;
    if verify_regular_hash(Path::new(UNIT_PATH), &plan.unit_sha256).is_ok() {
        return Ok(plan.clone());
    }
    if let Some(prior) = predecessor(plan)? {
        verify_regular_hash(Path::new(UNIT_PATH), &prior.unit_sha256)?;
        return Ok(prior);
    }
    exact_unit_absent(admitted.action_deadline)?;
    Ok(plan.clone())
}
pub(super) fn require_rollback_pause(admitted: &HostAdmission) -> Result<Option<JournalGuard>> {
    if !selected_on_this_host(admitted) {
        return Ok(None);
    }
    require_owner(admitted)?;
    let selected = nominated_admission(admitted)?;
    if exact_retained_owner(&selected, "rollback-pause-absent")? {
        exact_unit_absent(selected.action_deadline)?;
    } else {
        require_session_manager_operation_applied(
            &selected,
            "rollback-epoch-supervisor-pause",
            "stop",
            UNIT_NAME,
        )?;
    }
    epoch_generation::quiescent_generation(
        &installed_generation(admitted)?,
        admitted.action_deadline,
    )
}

/// Publish/start only after all four certified provider activations are durable.
pub(super) fn start(admitted: &HostAdmission, recovery_only: bool) -> Result<()> {
    nominated(admitted)?;
    let progress = load_or_create_host_progress(admitted)?;
    let done = host_forward_plan(admitted);
    let before = done
        .get(..usize::from(progress.next_forward_ordinal))
        .ok_or_else(|| eyre!("epoch supervisor start frontier exceeds host plan"))?;
    for validator in &admitted.inventory.validators {
        if !before.iter().any(|key| {
            key.host_slug == validator.slug && key.action == HostAction::BeaconActivate.label()
        }) {
            return Err(eyre!(
                "epoch supervisor start requires all four prior BeaconActivate actions"
            ));
        }
    }
    require_owner(admitted)?;
    if !recovery_only {
        let guard = require_forward_pause(admitted)?;
        publish_reset_unit(admitted)?;
        if let Some(guard) = &guard {
            guard.revalidate()?;
        }
        drop(guard);
    }
    require_session_manager_operation_applied(
        admitted,
        "epoch-supervisor-unit-reload",
        "daemon-reload",
        "",
    )?;
    manager_once(admitted, "epoch-supervisor-start", "start", recovery_only)?;
    require_ready(admitted)
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct NativeStatusV1 {
    schema_version: u32,
    policy_sha256: String,
    worker: WorkerV1,
    initial_completion: json::Value,
    current_completion: json::Value,
}

fn status_arguments(
    plan: &EpochSupervisorPlanV1,
    worker: &epoch_generation::ObservedEpochWorkerV1,
    timeout_ms: u64,
) -> Vec<String> {
    [
        "--config".to_owned(),
        plan.admin_config_path.clone(),
        "--operator-private-key-file".to_owned(),
        plan.http_operator_key_path.clone(),
        "taira".to_owned(),
        "epoch-maintenance".to_owned(),
        "supervisor-status".to_owned(),
        "--policy".to_owned(),
        plan.policy_path.clone(),
        "--trust".to_owned(),
        plan.trust_path.clone(),
        "--journal-dir".to_owned(),
        JOURNAL_DIR.to_owned(),
        "--boot-id".to_owned(),
        worker.boot_id.clone(),
        "--pid".to_owned(),
        worker.pid.to_string(),
        "--start-time-ticks".to_owned(),
        worker.start_time_ticks.to_string(),
        "--timeout-ms".to_owned(),
        timeout_ms.to_string(),
    ]
    .into()
}

/// Read-only native verifier authenticates retained initial and current next-target completions.
/// Process identity is independently observed before and after; opaque receipt bodies never grant authority here.
pub(super) fn status_generation(
    plan: &EpochSupervisorPlanV1,
    deadline: Instant,
) -> Result<json::Value> {
    validate_generation(plan)?;
    let before = epoch_generation::observe_generation(plan, deadline)?;
    let remaining = u64::try_from(
        deadline
            .saturating_duration_since(Instant::now())
            .as_millis(),
    )?;
    if remaining == 0 {
        return Err(eyre!("epoch supervisor status deadline elapsed"));
    }
    let cli = Path::new(&plan.cli_path);
    verify_regular_hash(&cli, &plan.iroha_sha256)?;
    let args = status_arguments(plan, &before, remaining);
    let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
    let bytes = run_host_command(
        cli.to_str()
            .ok_or_else(|| eyre!("native CLI path is not UTF-8"))?,
        &refs,
        deadline,
    )?;
    let result: NativeStatusV1 = json::from_slice(&bytes)?;
    let after = epoch_generation::observe_generation(plan, deadline)?;
    validate_status_identity(&result, &plan.policy_sha256, &before, &after)?;
    Ok(json::from_slice(&bytes)?)
}

fn validate_status_identity(
    result: &NativeStatusV1,
    policy_sha256: &str,
    before: &epoch_generation::ObservedEpochWorkerV1,
    after: &epoch_generation::ObservedEpochWorkerV1,
) -> Result<()> {
    if before.boot_id != after.boot_id
        || before.pid != after.pid
        || before.start_time_ticks != after.start_time_ticks
        || before.invocation_id != after.invocation_id
        || before.n_restarts != after.n_restarts
        || result.schema_version != 1
        || result.policy_sha256 != policy_sha256
        || result.worker.boot_id != before.boot_id
        || result.worker.pid != before.pid
        || result.worker.start_time_ticks != before.start_time_ticks
        || !result.initial_completion.is_object()
        || !result.current_completion.is_object()
    {
        return Err(eyre!(
            "native supervisor status is not bound to one current worker and its authenticated completions"
        ));
    }
    Ok(())
}

/// Convergence never treats an active service or an old ready file as completion.
pub(super) fn require_ready(admitted: &HostAdmission) -> Result<()> {
    status_generation(
        &admitted.inventory.epoch_supervisor,
        admitted.action_deadline,
    )?;
    Ok(())
}

fn qualify_restored_cohort(admitted: &HostAdmission) -> Result<()> {
    let progress = load_or_create_host_progress(admitted)?;
    if !progress.rolling_back
        || !required_rollback_targets(admitted, &progress)?
            .iter()
            .all(|slug| progress.rolled_back_hosts.contains(slug))
    {
        return Err(eyre!(
            "epoch supervisor restore precedes completed cohort rollback"
        ));
    }
    for validator in &admitted.inventory.validators {
        let selected = admission_for_validator(admitted, validator)?;
        if progress.touched_hosts.contains(&validator.slug) {
            verify_rollback_postcondition(&selected)?;
        } else if validator.is_vacant() {
            require_vacant_host_precondition(&selected)?;
        } else {
            verify_occupied_predecessor(&selected, validator)?;
        }
        if !validator.is_vacant() {
            occupied::verify_prior_genesis_hash(&selected, validator)?;
        }
    }
    let plan = &admitted.inventory.epoch_supervisor;
    let previous = predecessor(plan)?;
    if let Some(previous) = &previous {
        let policy = validate_generation(previous)?;
        let genesis = hex::decode(&admitted.inventory.previous_genesis_hash)?;
        if policy.intent.network_id.as_bytes().as_slice() != genesis.as_slice() {
            return Err(eyre!(
                "epoch supervisor predecessor policy differs from the restored authenticated genesis"
            ));
        }
        epoch_generation::preflight_reset_plan(previous, admitted.action_deadline)?;
    }
    Ok(())
}
fn restored_service_postcondition(admitted: &HostAdmission) -> Result<bool> {
    let plan = &admitted.inventory.epoch_supervisor;
    let label = if plan.prior_state == "running" {
        "rollback-epoch-supervisor-start"
    } else {
        "rollback-epoch-supervisor-unit-reload"
    };
    let path = ensure_host_receipt_dir(admitted)?.join(manager_intent_name(label)?);
    if !path.try_exists()? {
        if plan.prior_state == "absent" && exact_retained_owner(admitted, "rollback-pause-absent")?
        {
            qualify_restored_cohort(admitted)?;
            exact_unit_absent(admitted.action_deadline)?;
            return Ok(true);
        }
        return Ok(false);
    }
    qualify_restored_cohort(admitted)?;
    let (verb, target) = if plan.prior_state == "running" {
        ("start", UNIT_NAME)
    } else {
        ("daemon-reload", "")
    };
    require_session_manager_operation_applied(admitted, label, verb, target)?;
    if let Some(previous) = predecessor(plan)? {
        verify_regular_hash(Path::new(UNIT_PATH), &previous.unit_sha256)?;
        if plan.prior_state == "running" {
            status_generation(&previous, admitted.action_deadline)?;
        } else {
            let guard =
                epoch_generation::quiescent_generation(&previous, admitted.action_deadline)?;
            if let Some(guard) = &guard {
                guard.revalidate()?;
            }
        }
    } else {
        exact_unit_absent(admitted.action_deadline)?;
    }
    Ok(true)
}

/// Rollback containment precedes every edge/validator state change.
pub(super) fn rollback_pause(admitted: &HostAdmission) -> Result<()> {
    nominated(admitted)?;
    require_owner(admitted)?;
    reconcile_supervisor_operations(admitted)?;
    if restored_service_postcondition(admitted)? {
        return Ok(());
    }
    if admitted.inventory.epoch_supervisor.prior_state == "absent"
        && fs::symlink_metadata(UNIT_PATH)
            .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound)
    {
        ensure_root_private_directory(Path::new(JOURNAL_DIR))?;
        return record_absent_pause(admitted, "rollback-pause-absent", false);
    }
    manager_once(admitted, "rollback-epoch-supervisor-pause", "stop", false)?;
    let guard = epoch_generation::quiescent_generation(
        &installed_generation(admitted)?,
        admitted.action_deadline,
    )?;
    if let Some(guard) = &guard {
        guard.revalidate()?;
    }
    Ok(())
}

/// Restore exact original absent/running/stopped intent only after every original validator state and genesis qualifies.
pub(super) fn rollback_restore(admitted: &HostAdmission) -> Result<()> {
    nominated(admitted)?;
    require_owner(admitted)?;
    qualify_restored_cohort(admitted)?;
    if restored_service_postcondition(admitted)? {
        return Ok(());
    }
    let guard = require_rollback_pause(admitted)?;
    let plan = &admitted.inventory.epoch_supervisor;
    let previous = predecessor(plan)?;
    restore_reset_unit(admitted)?;
    if let Some(guard) = &guard {
        guard.revalidate()?;
    }
    drop(guard);
    if plan.prior_state == "running" {
        manager_once(admitted, "rollback-epoch-supervisor-start", "start", false)?;
        status_generation(
            previous
                .as_ref()
                .ok_or_else(|| eyre!("prior running generation missing"))?,
            admitted.action_deadline,
        )?;
    } else if plan.prior_state == "absent" {
        exact_unit_absent(admitted.action_deadline)?;
    } else {
        require_unit_stopped(UNIT_NAME, admitted.action_deadline)?;
    }
    publish_root_private_noreplace(
        Path::new(STATE_ROOT),
        &persistent_name(admitted, "restore-qualified"),
        json::to_json(&owner(admitted))?.as_bytes(),
    )?;
    Ok(())
}

pub(super) fn dispatch_rollback(
    admitted: &HostAdmission,
    action: HostAction,
) -> Result<HostReceiptV1> {
    nominated(admitted)?;
    if exact_retained_owner(admitted, "restored")? {
        if !restored_service_postcondition(admitted)? {
            return Err(eyre!(
                "terminal epoch restoration lacks native original-state evidence"
            ));
        }
        let directory = ensure_host_receipt_dir(admitted)?;
        let receipt = read_existing_host_receipt(
            &directory,
            &host_receipt_name(action, "")?,
            admitted,
            action,
        )?
        .ok_or_else(|| eyre!("restored epoch lifecycle omitted terminal receipt"))?;
        if Path::new(STATE_ROOT).join(OWNER_FILE).try_exists()? {
            release_owner(admitted, "restored")?;
        } else {
            sync_directory(Path::new(STATE_ROOT))?;
        }
        return Ok(receipt);
    }
    require_owner(admitted)?;
    let progress = load_or_create_host_progress(admitted)?;
    if progress.sealed {
        return Err(eyre!(
            "sealed reset cannot alter epoch supervisor rollback intent"
        ));
    }
    match action {
        HostAction::EpochSupervisorRollbackPause => rollback_pause(admitted)?,
        HostAction::EpochSupervisorRollbackRestore => rollback_restore(admitted)?,
        _ => return Err(eyre!("invalid epoch rollback action")),
    }
    let receipt = host_receipt(
        admitted,
        action,
        false,
        0,
        0,
        "exact native epoch containment/restoration postcondition verified",
    );
    publish_host_receipt(
        &ensure_host_receipt_dir(admitted)?,
        &host_receipt_name(action, "")?,
        &receipt,
    )?;
    if action == HostAction::EpochSupervisorRollbackRestore {
        release_owner(admitted, "restored")?;
    }
    Ok(receipt)
}

/// Reconcile only the submitted exact operation; never execute a new manager call.
pub(super) fn recover(
    admitted: &HostAdmission,
    action: HostAction,
    directory: &Path,
    receipt_name: &str,
    progress: &mut HostProgressV1,
    decision: HostProgressDecision,
) -> Result<HostReceiptV1> {
    let result = match action {
        HostAction::EpochSupervisorPause => pause(admitted, true),
        HostAction::EpochSupervisorStart => start(admitted, true),
        _ => return Err(eyre!("unsupported epoch supervisor recovery action")),
    };
    match result {
        Ok(()) => {
            let receipt = host_recovery_receipt(
                admitted,
                action,
                "ok",
                "exact epoch supervisor operation and native postcondition recovered",
            );
            publish_host_receipt(directory, receipt_name, &receipt)?;
            if decision == HostProgressDecision::Advance {
                advance_host_progress(admitted, action, progress)?;
            }
            Ok(receipt)
        }
        Err(error) if is_local_mutation_recovery_pending(&error) => Ok(host_recovery_receipt(
            admitted,
            action,
            "pending",
            "original epoch supervisor operation remains pending",
        )),
        Err(_) => Ok(host_recovery_receipt(
            admitted,
            action,
            "rejected",
            "epoch supervisor identity or native postcondition rejected",
        )),
    }
}

#[cfg(test)]
pub(in super::super) fn fixture_plan(
    validators: &[ValidatorV1],
    clients: &[super::super::ValidatorClientV1],
    revision: &super::super::RevisionV1,
    administrator: &super::super::MaintenanceAdminIdentityV1,
) -> EpochSupervisorPlanV1 {
    let nominated = &validators[0];
    let release = format!("{}/releases/{}", nominated.service_root, revision.commit);
    let iroha_sha256 = artifact(&nominated.artifacts, "iroha_cli")
        .unwrap()
        .sha256
        .clone();
    let trust = crate::taira_dataspace_deploy::DeploymentTrustV1 {
        genesis_public_key: iroha_test_samples::ALICE_KEYPAIR.public_key().clone(),
        genesis_signed_wire_hex: "00".into(),
        peers: clients
            .iter()
            .map(|client| {
                let validator = validators.iter().find(|v| v.slug == client.slug).unwrap();
                crate::taira_dataspace_deploy::DeploymentPeerV1 {
                    torii_origin: client.probe_origin.clone(),
                    peer_id: client.peer_id.parse().unwrap(),
                    node_fingerprint: validator.node_fingerprint.parse().unwrap(),
                    build_fingerprint: validator.build_fingerprint.parse().unwrap(),
                    config_fingerprint: validator.config_fingerprint.parse().unwrap(),
                }
            })
            .collect(),
    };
    let trust = json::to_json(&trust).unwrap().into_bytes();
    let policy = NativePolicyV1 {
        schema_version: 1,
        intent: OngoingIntentV1 {
            authorization: "until_stopped".into(),
            network_id: administrator.network_id.parse().unwrap(),
            administrator: AccountId::parse_encoded(&administrator.account_id)
                .expect("canonical fixture administrator"),
            first_epoch: 1,
        },
        release_source_commit: revision.commit.clone(),
        iroha_sha256: iroha_sha256.clone(),
        observation_trust_sha256: sha256_hex(&trust),
    };
    let policy_bytes = json::to_json(&policy).unwrap().into_bytes();
    let policy_sha256 = sha256_hex(&policy_bytes);
    let generation = format!("{STATE_ROOT}/generations/{policy_sha256}");
    let mut plan = EpochSupervisorPlanV1 {
        schema: PLAN_SCHEMA.into(),
        host_slug: nominated.slug.clone(),
        unit_name: UNIT_NAME.into(),
        state_root: STATE_ROOT.into(),
        journal_dir: JOURNAL_DIR.into(),
        release_source_commit: revision.commit.clone(),
        iroha_sha256,
        cli_path: format!("{release}/bin/iroha"),
        policy_sha256,
        policy_bytes,
        observation_trust_sha256: sha256_hex(&trust),
        observation_trust_bytes: trust,
        unit_sha256: String::new(),
        unit_bytes: Vec::new(),
        admin_config_path: format!("{generation}/administrator.toml"),
        admin_config_sha256: "5".repeat(64),
        http_operator_key_path: format!("{generation}/http-operator.key"),
        http_operator_key_sha256: "6".repeat(64),
        policy_path: format!("{generation}/policy.json"),
        trust_path: format!("{generation}/trust.json"),
        timeout_ms: 3_600_000,
        prior_state: "absent".into(),
        prior: None,
    };
    plan.unit_bytes = render_unit(&plan, &format!("{release}/bin/iroha")).unwrap();
    plan.unit_sha256 = sha256_hex(&plan.unit_bytes);
    plan
}

/// Independently bound prior supervisor generation for cleanup and custody tests.
#[cfg(test)]
pub(super) fn fixture_prior_generation(
    plan: &EpochSupervisorPlanV1,
    release: &Path,
) -> EpochSupervisorPlanV1 {
    let _chain_guard = ChainDiscriminantGuard::enter(super::super::CHAIN_DISCRIMINANT);
    let mut prior = plan.clone();
    prior.prior_state = "absent".into();
    prior.prior = None;
    prior.release_source_commit = "7".repeat(40);
    let mut policy: NativePolicyV1 = json::from_slice(&prior.policy_bytes).unwrap();
    policy.release_source_commit = prior.release_source_commit.clone();
    prior.cli_path = release.join("bin/iroha").display().to_string();
    prior.policy_bytes = json::to_json(&policy).unwrap().into_bytes();
    prior.policy_sha256 = sha256_hex(&prior.policy_bytes);
    let generation = format!("{STATE_ROOT}/generations/{}", prior.policy_sha256);
    prior.admin_config_path = format!("{generation}/administrator.toml");
    prior.http_operator_key_path = format!("{generation}/http-operator.key");
    prior.policy_path = format!("{generation}/policy.json");
    prior.trust_path = format!("{generation}/trust.json");
    prior.unit_bytes = render_unit(&prior, release.join("bin/iroha").to_str().unwrap()).unwrap();
    prior.unit_sha256 = sha256_hex(&prior.unit_bytes);
    validate_generation(&prior).unwrap();
    prior
}

#[cfg(test)]
mod tests {
    use super::*;
    fn bind_prior(plan: &mut EpochSupervisorPlanV1, prior: &EpochSupervisorPlanV1) {
        let bytes = json::to_json(prior).unwrap().into_bytes();
        plan.prior = Some(PriorEpochSupervisorV1 {
            plan_sha256: sha256_hex(&bytes),
            plan_bytes: bytes,
        });
    }

    #[test]
    fn prior_release_protection_preserves_independent_authenticated_tool_roots() {
        let mut plan = super::super::super::sample_inventory_fixture().epoch_supervisor;
        for root in [
            "/private/runtime/taira-public-reset/release94".to_owned(),
            format!("/srv/taira/taira-validator-2/releases/{}", "8".repeat(40)),
        ] {
            let root = Path::new(&root);
            let prior = fixture_prior_generation(&plan, root);
            for state in ["running", "stopped"] {
                plan.prior_state = state.into();
                bind_prior(&mut plan, &prior);
                for path in [
                    root.to_path_buf(),
                    root.join("bin"),
                    root.join("bin/iroha"),
                    root.join("bin/kagami"),
                ] {
                    assert!(
                        protects_prior_release(&plan, &path).unwrap(),
                        "{}",
                        path.display()
                    );
                }
                assert!(!protects_prior_release(&plan, &root.with_file_name("unrelated")).unwrap());
            }
        }
        plan.prior_state = "absent".into();
        plan.prior = None;
        assert!(!protects_prior_release(&plan, Path::new("/srv/taira")).unwrap());
    }

    #[test]
    fn prior_release_protection_rejects_malformed_state_or_plan() {
        let original = super::super::super::sample_inventory_fixture().epoch_supervisor;
        let root = Path::new("/private/runtime/taira-public-reset/release94");
        for change in 0..10 {
            let mut plan = original.clone();
            let mut prior = fixture_prior_generation(&plan, root);
            plan.prior_state = "running".into();
            match change {
                0 => plan.prior_state = "absent".into(),
                1 | 2 => {}
                3 => plan.prior_state = "unknown".into(),
                4 | 5 => {}
                6 => prior.host_slug = "taira-validator-2".into(),
                7 => prior.iroha_sha256 = "9".repeat(64),
                8 => prior.policy_bytes.push(b' '),
                9 => prior.unit_bytes.push(b' '),
                _ => unreachable!(),
            }
            bind_prior(&mut plan, &prior);
            match change {
                1 | 2 => {
                    plan.prior = None;
                    if change == 2 {
                        plan.prior_state = "stopped".into();
                    }
                }
                4 => plan.prior.as_mut().unwrap().plan_sha256 = "1".repeat(64),
                5 => {
                    let pin = plan.prior.as_mut().unwrap();
                    pin.plan_bytes = b"{}".to_vec();
                    pin.plan_sha256 = sha256_hex(&pin.plan_bytes);
                }
                _ => {}
            }
            assert!(
                protects_prior_release(&plan, root).is_err(),
                "mutation {change}"
            );
            assert!(
                protects_prior_release(&plan, Path::new("/unrelated")).is_err(),
                "mutation {change} must not become no protection"
            );
        }
    }

    #[test]
    fn unit_matches_exact_native_retention_argv_and_lifecycle_golden() {
        let inventory = super::super::super::sample_inventory_fixture();
        let mut plan = inventory.epoch_supervisor;
        let generation = format!("{STATE_ROOT}/generations/{}", "a".repeat(64));
        plan.admin_config_path = format!("{generation}/administrator.toml");
        plan.http_operator_key_path = format!("{generation}/http-operator.key");
        plan.policy_path = format!("{generation}/policy.json");
        plan.trust_path = format!("{generation}/trust.json");
        plan.timeout_ms = 3_600_000;
        let cli = format!(
            "/srv/taira/runtime/release-{}-update-{}/bin/iroha",
            "b".repeat(40),
            "c".repeat(32)
        );
        let bytes = render_unit(&plan, &cli).unwrap();
        assert_eq!(
            sha256_hex(&bytes),
            "d4c2016116ee21fc956178040789b2e9ff4196ecabc78d2b01c3ed966425c427"
        );
        let unit = std::str::from_utf8(&bytes).unwrap();
        assert!(unit.contains("RestartPreventExitStatus=3 4 7\n"));
        assert!(!unit.contains("--custody") && !unit.contains("--fee-payer"));
        assert!(unit.contains("\"taira\" \"epoch-maintenance\" \"supervise\""));
    }
    #[test]
    fn retention_plan_rejects_retired_rotation_fields() {
        let plan = super::super::super::sample_inventory_fixture().epoch_supervisor;
        for field in [
            "kagami_sha256",
            "custody_bytes",
            "custody_sha256",
            "original_seed_sources",
            "custody_path",
        ] {
            let mut value: json::Value = json::from_slice(&json::to_vec(&plan).unwrap()).unwrap();
            value
                .as_object_mut()
                .unwrap()
                .insert(field.into(), json::Value::Null);
            assert!(
                json::from_slice::<EpochSupervisorPlanV1>(&json::to_vec(&value).unwrap()).is_err(),
                "{field}"
            );
        }
        for field in ["kagami", "provision_timeout_ms"] {
            let mut value: json::Value = json::from_slice(&plan.policy_bytes).unwrap();
            value
                .as_object_mut()
                .unwrap()
                .insert(field.into(), json::Value::Null);
            assert!(
                json::from_slice::<NativePolicyV1>(&json::to_vec(&value).unwrap()).is_err(),
                "{field}"
            );
        }
        for field in [
            "payment_asset",
            "transaction_fee_maximum",
            "batch_epochs",
            "operation_timeout_ms",
        ] {
            let mut value: json::Value = json::from_slice(&plan.policy_bytes).unwrap();
            value
                .as_object_mut()
                .unwrap()
                .get_mut("intent")
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert(field.into(), json::Value::Null);
            assert!(
                json::from_slice::<NativePolicyV1>(&json::to_vec(&value).unwrap()).is_err(),
                "{field}"
            );
        }
    }
    #[test]
    fn first_install_pause_requires_genuine_manager_absence() {
        let exact = "LoadState=not-found\nFragmentPath=\nDropInPaths=\nActiveState=inactive\nSubState=dead\nMainPID=0\nControlPID=0\nJob=\n";
        assert!(validate_absent_unit_evidence(exact.as_bytes()).is_ok());
        for (before, after) in [
            ("LoadState=not-found", "LoadState=loaded"),
            (
                "FragmentPath=",
                "FragmentPath=/etc/systemd/system/foreign.service",
            ),
            (
                "DropInPaths=",
                "DropInPaths=/etc/systemd/system/foreign.conf",
            ),
            ("MainPID=0", "MainPID=91"),
            ("ControlPID=0", "ControlPID=92"),
            ("Job=", "Job=17"),
            ("ActiveState=inactive", "ActiveState=activating"),
        ] {
            assert!(
                validate_absent_unit_evidence(exact.replace(before, after).as_bytes()).is_err(),
                "{before}"
            );
        }
        assert!(validate_absent_unit_evidence(format!("{exact}Job=\n").as_bytes()).is_err());
    }
    #[test]
    fn epoch_public_admission_scopes_taira_and_restores_foreign_caller_profile() {
        use iroha::data_model::account::address::chain_discriminant;

        let caller_profile = chain_discriminant();
        {
            let _foreign = ChainDiscriminantGuard::enter(753);
            let inventory = super::super::super::sample_inventory_fixture();
            assert_eq!(chain_discriminant(), 753);
            super::super::super::validate_maintenance_admin_identity(&inventory)
                .expect("administrator admission owns the fixed Taira profile");
            assert_eq!(chain_discriminant(), 753);
            let policy = validate_generation(&inventory.epoch_supervisor)
                .expect("direct generation decoding owns the fixed Taira profile");
            assert_eq!(chain_discriminant(), 753);
            validate_plan(&inventory)
                .expect("plan formatting retains Taira after nested generation admission");
            assert_eq!(chain_discriminant(), 753);
            validate_public_generation(&inventory.epoch_supervisor)
                .expect("public generation decoding owns the fixed Taira profile");
            assert_eq!(chain_discriminant(), 753);
            validate_plan_context(
                &inventory.epoch_supervisor,
                &inventory.revision,
                &inventory.validators,
                &inventory.validator_clients,
                &inventory.maintenance_admin_identity,
                &inventory.maintenance_admin_config_sha256,
                &inventory.beacon_bootstrap.genesis_public_key,
            )
            .expect("direct context admission retains Taira through canonical account formatting");
            assert_eq!(chain_discriminant(), 753);

            let mut wrong = inventory.clone();
            wrong.chain_discriminant = 753;
            assert!(super::super::super::validate_maintenance_admin_identity(&wrong).is_err());
            assert_eq!(chain_discriminant(), 753);
            assert!(validate_plan(&wrong).is_err());
            assert_eq!(chain_discriminant(), 753);
            let mut wrong = inventory.clone();
            wrong.chain_id = "foreign-chain".into();
            assert!(super::super::super::validate_maintenance_admin_identity(&wrong).is_err());
            assert!(validate_plan(&wrong).is_err());
            assert_eq!(chain_discriminant(), 753);

            let mut wrong = inventory.clone();
            wrong.maintenance_admin_identity.account_id = policy.intent.administrator.to_string();
            assert_ne!(
                wrong.maintenance_admin_identity.account_id,
                inventory.maintenance_admin_identity.account_id
            );
            assert!(super::super::super::validate_maintenance_admin_identity(&wrong).is_err());
            assert_eq!(chain_discriminant(), 753);

            // Rebind every public digest/path/unit to a policy serialized under
            // the foreign profile. Structural admission must reach and reject
            // its foreign I105 administrator, even when the caller uses that profile.
            let mut wrong = inventory.epoch_supervisor.clone();
            wrong.policy_bytes = json::to_vec(&policy).unwrap();
            wrong.policy_sha256 = sha256_hex(&wrong.policy_bytes);
            let generation = format!("{STATE_ROOT}/generations/{}", wrong.policy_sha256);
            wrong.admin_config_path = format!("{generation}/administrator.toml");
            wrong.http_operator_key_path = format!("{generation}/http-operator.key");
            wrong.policy_path = format!("{generation}/policy.json");
            wrong.trust_path = format!("{generation}/trust.json");
            let cli = Path::new(&wrong.cli_path);
            wrong.unit_bytes = render_unit(&wrong, cli.to_str().unwrap()).unwrap();
            wrong.unit_sha256 = sha256_hex(&wrong.unit_bytes);
            assert!(validate_generation(&wrong).is_err());
            assert_eq!(chain_discriminant(), 753);
        }
        assert_eq!(chain_discriminant(), caller_profile);
    }

    #[test]
    fn plan_rejects_wrong_administrator_origin_and_duplicate_roster_mapping() {
        let inventory = super::super::super::sample_inventory_fixture();
        validate_plan(&inventory).unwrap();
        let mut wrong = inventory.clone();
        wrong.maintenance_admin_identity.torii_origin = "https://unselected.example/".into();
        assert!(validate_plan(&wrong).is_err());
        let mut wrong = inventory;
        wrong.validator_clients[0].peer_id = wrong.validator_clients[1].peer_id.clone();
        assert!(validate_plan(&wrong).is_err());
    }
    #[test]
    fn status_argv_contains_only_readonly_native_operation_and_exact_worker() {
        let plan = super::super::super::sample_inventory_fixture().epoch_supervisor;
        let worker = epoch_generation::ObservedEpochWorkerV1 {
            boot_id: "11111111-2222-3333-4444-555555555555".into(),
            pid: 17,
            start_time_ticks: 91,
            invocation_id: "a".repeat(32),
            n_restarts: 0,
        };
        let args = status_arguments(&plan, &worker, 1234);
        assert_eq!(
            &args[4..7],
            ["taira", "epoch-maintenance", "supervisor-status"]
        );
        assert!(
            !args
                .iter()
                .any(|v| v == "supervise" || v == "--custody" || v == "apply")
        );
        assert_eq!(
            &args[13..],
            [
                "--boot-id",
                worker.boot_id.as_str(),
                "--pid",
                "17",
                "--start-time-ticks",
                "91",
                "--timeout-ms",
                "1234"
            ]
        );
    }
    #[test]
    fn paths_reject_expansion_and_finite_reset_aliases() {
        for value in [
            "relative", "/a/../b", "/a//b", "/a/$HOME", "/a/%n", "/a b", "/a\nb",
        ] {
            assert!(literal_path(value).is_err(), "{value:?}");
        }
        assert!(literal_path(JOURNAL_DIR).is_ok());
    }
    #[test]
    fn native_status_rejects_previous_worker_or_changed_manager_incarnation() {
        let before = epoch_generation::ObservedEpochWorkerV1 {
            boot_id: "11111111-2222-3333-4444-555555555555".into(),
            pid: 17,
            start_time_ticks: 91,
            invocation_id: "a".repeat(32),
            n_restarts: 0,
        };
        let worker = WorkerV1 {
            boot_id: before.boot_id.clone(),
            pid: before.pid,
            start_time_ticks: before.start_time_ticks,
        };
        let result = NativeStatusV1 {
            schema_version: 1,
            policy_sha256: "ab".repeat(32),
            worker,
            initial_completion: json::Value::Object(json::Map::new()),
            current_completion: json::Value::Object(json::Map::new()),
        };
        validate_status_identity(&result, &result.policy_sha256, &before, &before).unwrap();
        for changed in [
            epoch_generation::ObservedEpochWorkerV1 {
                pid: 18,
                ..before.clone()
            },
            epoch_generation::ObservedEpochWorkerV1 {
                start_time_ticks: 92,
                ..before.clone()
            },
            epoch_generation::ObservedEpochWorkerV1 {
                invocation_id: "b".repeat(32),
                ..before.clone()
            },
            epoch_generation::ObservedEpochWorkerV1 {
                n_restarts: 1,
                ..before.clone()
            },
        ] {
            assert!(
                validate_status_identity(&result, &result.policy_sha256, &before, &changed)
                    .is_err()
            );
        }
        assert!(validate_status_identity(&result, &"cd".repeat(32), &before, &before).is_err());
        let mut empty = result;
        empty.current_completion = json::Value::Null;
        assert!(validate_status_identity(&empty, &empty.policy_sha256, &before, &before).is_err());
    }
}
