//! Native custody and public observation for the one fixed epoch supervisor.
//! No service start/stop, seed derivation, transaction dispatch, or reset authority lives here.

#[cfg(target_os = "linux")]
use super::epoch_supervisor::JOURNAL_DIR;
use super::epoch_supervisor::{EpochSupervisorPlanV1, STATE_ROOT, UNIT_NAME};
use super::*;

/// Root-only public generation operations; private material is accepted only by descriptor.
#[derive(clap::Args, Debug)]
pub(in super::super) struct EpochSupervisorHost {
    #[arg(value_enum)]
    action: GenerationAction,
    #[arg(long, value_name = "PATH")]
    wrapper: PathBuf,
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..))]
    timeout_ms: u64,
    #[arg(long, value_parser = clap::value_parser!(u32).range(3..=65535))]
    administrator_config_fd: Option<u32>,
    #[arg(long, value_parser = clap::value_parser!(u32).range(3..=65535))]
    http_operator_key_fd: Option<u32>,
    #[arg(long, value_parser = clap::value_parser!(u32).range(3..=65535))]
    deployment_lock_fd: Option<u32>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
enum GenerationAction {
    Preflight,
    Materialize,
    Observe,
    Quiescence,
    Status,
}

impl EpochSupervisorHost {
    pub(in super::super) fn run<W: Write>(&self, output: &mut W) -> Result<()> {
        // This command runs before client-config admission. Retain the fixed
        // profile through nested public parsing, config admission and receipts.
        let _chain_guard = ChainDiscriminantGuard::enter(super::super::CHAIN_DISCRIMINANT);
        #[cfg(target_os = "linux")]
        {
            linux::run(self, output)
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = output;
            Err(eyre!("epoch supervisor generation custody requires Linux"))
        }
    }
}

/// Actual manager-selected Linux worker; native status authenticates its completions separately.
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(super) struct ObservedEpochWorkerV1 {
    pub(super) boot_id: String,
    pub(super) pid: u32,
    pub(super) start_time_ticks: u64,
    pub(super) invocation_id: String,
    pub(super) n_restarts: u64,
}

pub(super) fn acquire_deployment_lock(deadline: Instant) -> Result<File> {
    #[cfg(target_os = "linux")]
    {
        linux::deployment_lock(None, deadline)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = deadline;
        Err(eyre!("epoch deployment lock requires Linux"))
    }
}
pub(super) fn observe_generation(
    plan: &EpochSupervisorPlanV1,
    deadline: Instant,
) -> Result<ObservedEpochWorkerV1> {
    #[cfg(target_os = "linux")]
    {
        linux::observe(plan, deadline)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (plan, deadline);
        Err(eyre!("epoch worker observation requires Linux"))
    }
}
pub(super) fn quiescent_generation(
    plan: &EpochSupervisorPlanV1,
    deadline: Instant,
) -> Result<Option<crate::taira_dataspace_deploy::epoch_maintenance::SupervisorJournalGuard>> {
    #[cfg(target_os = "linux")]
    {
        linux::quiescent(plan, deadline)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (plan, deadline);
        Err(eyre!("epoch journal quiescence requires Linux"))
    }
}

/// Read-only preparation permits absent successor artifacts before reset Stage.
pub(super) fn preflight_reset_plan(plan: &EpochSupervisorPlanV1, deadline: Instant) -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        linux::preflight_reset(plan, deadline)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (plan, deadline);
        Err(eyre!("epoch generation custody requires Linux"))
    }
}
/// Exact installed native custody; does not acquire the worker journal flock.
pub(super) fn preflight_generation(plan: &EpochSupervisorPlanV1, deadline: Instant) -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        linux::preflight(plan, deadline).map(|_| ())
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (plan, deadline);
        Err(eyre!("epoch generation custody requires Linux"))
    }
}
/// Called only inside an admitted native reset dispatcher holding deployment custody.
/// Buffers belong to the native framed reader and must remain zeroizing at their owner.
pub(super) fn materialize_reset_generation(
    plan: &EpochSupervisorPlanV1,
    admin: &[u8],
    http: &[u8],
    operation: &str,
    deadline: Instant,
) -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        linux::materialize_reset(plan, admin, http, operation, deadline)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (plan, admin, http, operation, deadline);
        Err(eyre!("epoch generation custody requires Linux"))
    }
}

// Public data and pure validation are shared by both input producers and the Linux consumer.
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(super) struct UnitSpecV1 {
    pub(super) schema_version: u32,
    pub(super) cli: String,
    pub(super) admin_config: String,
    pub(super) operator_key: String,
    pub(super) policy: String,
    pub(super) trust: String,
    pub(super) journal_dir: String,
    pub(super) timeout_ms: u64,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(super) struct BindingV1 {
    pub(super) schema_version: u32,
    pub(super) release_source_commit: String,
    pub(super) iroha_sha256: String,
    pub(super) network_id: String,
    pub(super) unit_spec: UnitSpecV1,
    pub(super) unit_bytes: String,
    pub(super) unit_sha256: String,
    pub(super) policy_bytes: String,
    pub(super) policy_sha256: String,
    pub(super) observation_trust_bytes: String,
    pub(super) observation_trust_sha256: String,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(super) struct PreparationV1 {
    pub(super) schema: String,
    pub(super) operation: String,
    pub(super) original_service_state: String,
    pub(super) successor_service_state: String,
    #[norito(required)]
    pub(super) before: Option<BindingV1>,
    #[norito(required)]
    pub(super) installed: Option<BindingV1>,
    pub(super) after: BindingV1,
}

#[cfg(any(target_os = "linux", test))]
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct ReceiptReferenceV1 {
    path: String,
    sha256: String,
}

#[cfg(any(target_os = "linux", test))]
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct WrapperV1 {
    schema: String,
    operation: String,
    original_service_state: String,
    successor_service_state: String,
    #[norito(required)]
    before: Option<BindingV1>,
    #[norito(required)]
    installed: Option<BindingV1>,
    after: BindingV1,
    native_provisioning_receipt: ReceiptReferenceV1,
}

/// Close the updater input only after native custody has published its receipt.
/// This public transformation does not create or verify that receipt itself.
#[cfg(any(target_os = "linux", test))]
fn completed_update_wrapper(
    input: PreparationV1,
    receipt: ReceiptReferenceV1,
) -> Result<WrapperV1> {
    require(
        input.schema == "taira.epoch-supervisor-generation-preparation.v1",
        "epoch preparation schema differs",
    )?;
    operation(&input.operation)?;
    intent(
        &input.original_service_state,
        &input.successor_service_state,
        input.before.is_some(),
    )?;
    as_public_plan(&input.after, &input.original_service_state)?;
    require(
        input
            .before
            .as_ref()
            .is_none_or(|v| v.network_id == input.after.network_id)
            && input
                .installed
                .as_ref()
                .is_none_or(|v| v.network_id == input.after.network_id),
        "updater cannot change the epoch journal network",
    )?;
    require(
        receipt.path
            == format!(
                "{STATE_ROOT}/generations/{}/provisioning-receipt.json",
                input.after.policy_sha256
            )
            && receipt.sha256.len() == 64
            && receipt
                .sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            && receipt.sha256.bytes().any(|b| b != b'0'),
        "completed native provisioning receipt reference differs",
    )?;
    Ok(WrapperV1 {
        schema: "taira.epoch-supervisor-update.v1".into(),
        operation: input.operation,
        original_service_state: input.original_service_state,
        successor_service_state: input.successor_service_state,
        before: input.before,
        installed: input.installed,
        after: input.after,
        native_provisioning_receipt: receipt,
    })
}

fn require(ok: bool, message: &'static str) -> Result<()> {
    if ok { Ok(()) } else { Err(eyre!(message)) }
}

pub(super) fn operation(value: &str) -> Result<()> {
    require(
        !value.is_empty()
            && value.len() <= 128
            && value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b)),
        "invalid epoch deployment operation",
    )
}

pub(super) fn intent(original: &str, successor: &str, before: bool) -> Result<()> {
    require(
        matches!(original, "absent" | "running" | "stopped")
            && matches!(successor, "running" | "stopped")
            && before == (original != "absent")
            && (original == "absent" || original == successor),
        "epoch original and desired service intent differ",
    )
}

pub(super) fn binding_digest(binding: &BindingV1) -> Result<String> {
    Ok(sha256_hex(&json::to_vec(binding)?))
}

pub(super) fn as_plan(
    binding: &BindingV1,
    admin_sha: String,
    http_sha: String,
    original: &str,
) -> Result<EpochSupervisorPlanV1> {
    let mut plan = as_public_plan(binding, original)?;
    plan.admin_config_sha256 = admin_sha;
    plan.http_operator_key_sha256 = http_sha;
    epoch_supervisor::validate_generation(&plan)?;
    Ok(plan)
}

/// Internal public projection for producer validation, never a serializable plan output.
/// Empty credential fields are deliberate absence: full native admission rejects this value.
pub(super) fn as_public_plan(binding: &BindingV1, original: &str) -> Result<EpochSupervisorPlanV1> {
    require(
        binding.schema_version == 1 && binding.unit_spec.schema_version == 1,
        "epoch binding version differs",
    )?;
    let spec = &binding.unit_spec;
    let plan = EpochSupervisorPlanV1 {
        schema: "iroha.taira.public-reset.epoch-supervisor-plan.v1".into(),
        host_slug: "generation-only".into(),
        unit_name: UNIT_NAME.into(),
        state_root: STATE_ROOT.into(),
        journal_dir: spec.journal_dir.clone(),
        release_source_commit: binding.release_source_commit.clone(),
        iroha_sha256: binding.iroha_sha256.clone(),
        cli_path: spec.cli.clone(),
        policy_sha256: binding.policy_sha256.clone(),
        policy_bytes: binding.policy_bytes.as_bytes().to_vec(),
        observation_trust_sha256: binding.observation_trust_sha256.clone(),
        observation_trust_bytes: binding.observation_trust_bytes.as_bytes().to_vec(),
        unit_sha256: binding.unit_sha256.clone(),
        unit_bytes: binding.unit_bytes.as_bytes().to_vec(),
        admin_config_path: spec.admin_config.clone(),
        admin_config_sha256: String::new(),
        http_operator_key_path: spec.operator_key.clone(),
        http_operator_key_sha256: String::new(),
        policy_path: spec.policy.clone(),
        trust_path: spec.trust.clone(),
        timeout_ms: spec.timeout_ms,
        prior_state: original.into(),
        prior: None,
    };
    let policy = epoch_supervisor::validate_public_generation(&plan)?;
    require(
        policy.intent.network_id.to_string() == binding.network_id,
        "epoch CLI/network binding differs",
    )?;
    super::super::validate_lower_hex("epoch release commit", &binding.release_source_commit, 40)?;
    Ok(plan)
}

pub(super) fn binding_from_plan(plan: &EpochSupervisorPlanV1) -> Result<BindingV1> {
    epoch_supervisor::validate_generation(plan)?;
    binding_from_public_plan(plan)
}

/// Convert a validated complete public closure without making any credential hash claim.
pub(super) fn binding_from_public_plan(plan: &EpochSupervisorPlanV1) -> Result<BindingV1> {
    let policy = epoch_supervisor::validate_public_generation(plan)?;
    Ok(BindingV1 {
        schema_version: 1,
        release_source_commit: plan.release_source_commit.clone(),
        iroha_sha256: plan.iroha_sha256.clone(),
        network_id: policy.intent.network_id.to_string(),
        unit_spec: UnitSpecV1 {
            schema_version: 1,
            cli: plan.cli_path.clone(),
            admin_config: plan.admin_config_path.clone(),
            operator_key: plan.http_operator_key_path.clone(),
            policy: plan.policy_path.clone(),
            trust: plan.trust_path.clone(),
            journal_dir: plan.journal_dir.clone(),
            timeout_ms: plan.timeout_ms,
        },
        unit_bytes: String::from_utf8(plan.unit_bytes.clone())?,
        unit_sha256: plan.unit_sha256.clone(),
        policy_bytes: String::from_utf8(plan.policy_bytes.clone())?,
        policy_sha256: plan.policy_sha256.clone(),
        observation_trust_bytes: String::from_utf8(plan.observation_trust_bytes.clone())?,
        observation_trust_sha256: plan.observation_trust_sha256.clone(),
    })
}

#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use iroha_crypto::KeyPair;
    use std::os::unix::fs::MetadataExt as _;
    use zeroize::Zeroizing;
    const LIMIT: u64 = 8 * 1024 * 1024;
    const LOCK: &str = "/var/lib/taira-epoch-supervisor/.deployment.lock";
    const RESET_OWNER: &str = "/var/lib/taira-epoch-supervisor/.reset-owner.json";
    const RECEIPT: &str = "provisioning-receipt.json";

    #[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
    #[norito(deny_unknown_fields)]
    struct MetadataV1 {
        path: String,
        device: u64,
        inode: u64,
        uid: u32,
        gid: u32,
        mode: u32,
    }
    #[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
    #[norito(deny_unknown_fields)]
    struct GenerationReceiptV1 {
        schema: String,
        operation: String,
        original_service_state: String,
        successor_service_state: String,
        binding_sha256: String,
        policy_sha256: String,
        release_source_commit: String,
        iroha_sha256: String,
        observation_trust_sha256: String,
        unit_sha256: String,
        administrator: AdministratorV1,
        admin_config_sha256: String,
        http_operator_key_sha256: String,
        http_operator_public_key: String,
        generation: MetadataV1,
        journal: MetadataV1,
        files: Vec<MetadataV1>,
    }

    #[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
    #[norito(deny_unknown_fields)]
    pub(super) struct AdministratorV1 {
        account_id: String,
        public_key: String,
        network_id: String,
        chain_discriminant: u16,
        torii_origin: String,
    }
    #[derive(Clone, Debug, JsonSerialize)]
    struct ActionReceiptV1 {
        schema: String,
        action: String,
        service_state: String,
        operation: String,
        policy_sha256: String,
        unit_sha256: String,
        provisioning_receipt: ReceiptReferenceV1,
        #[norito(required)]
        installed_policy_sha256: Option<String>,
        journal: MetadataV1,
        #[norito(required)]
        worker: Option<ObservedEpochWorkerV1>,
        #[norito(required)]
        status: Option<json::Value>,
    }

    fn root() -> Result<()> {
        require(
            rustix::process::geteuid().as_raw() == 0,
            "epoch generation operations require root",
        )
    }
    fn deadline(ms: u64) -> Result<Instant> {
        Instant::now()
            .checked_add(Duration::from_millis(ms))
            .ok_or_else(|| eyre!("generation deadline overflow"))
    }
    fn reject_reset_owner() -> Result<()> {
        match fs::symlink_metadata(RESET_OWNER) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            _ => Err(eyre!(
                "retained reset owner blocks updater generation actions; no expiry or implicit reclaim"
            )),
        }
    }
    fn identity(path: &Path, directory: bool) -> Result<MetadataV1> {
        require_root_no_symlink_ancestors(path, "epoch generation object")?;
        let m = fs::symlink_metadata(path)?;
        require(
            !m.file_type().is_symlink()
                && m.uid() == 0
                && m.gid() == 0
                && if directory {
                    m.is_dir() && m.mode() & 0o7777 == 0o700
                } else {
                    m.is_file() && m.nlink() == 1 && m.mode() & 0o7777 == 0o600
                },
            "epoch generation root custody differs",
        )?;
        Ok(MetadataV1 {
            path: path
                .to_str()
                .ok_or_else(|| eyre!("non-UTF8 epoch path"))?
                .into(),
            device: m.dev(),
            inode: m.ino(),
            uid: m.uid(),
            gid: m.gid(),
            mode: m.mode() & 0o7777,
        })
    }
    fn public_json<T: JsonDeserialize>(path: &Path) -> Result<T> {
        require_root_no_symlink_ancestors(path, "epoch public input")?;
        let (file, snapshot) = open_pinned_regular(path, "epoch public input")?;
        require(
            snapshot.uid == 0 && snapshot.mode & 0o022 == 0 && file.metadata()?.nlink() == 1,
            "epoch public input root custody differs",
        )?;
        require(
            snapshot.len > 0 && snapshot.len <= LIMIT,
            "epoch public input size differs",
        )?;
        let bytes = read_pinned_bytes(path, "epoch public input", file, &snapshot, LIMIT)?;
        json::from_slice(&bytes).map_err(|_| eyre!("epoch public input is not its closed schema"))
    }

    fn generation(binding: &BindingV1) -> PathBuf {
        PathBuf::from(STATE_ROOT)
            .join("generations")
            .join(&binding.policy_sha256)
    }

    fn executable(path: &Path, expected: &str) -> Result<()> {
        require_root_no_symlink_ancestors(path, "epoch executable")?;
        let (mut file, snapshot) = open_pinned_regular(path, "epoch executable")?;
        require(
            snapshot.uid == 0
                && snapshot.mode & 0o7777 == 0o755
                && snapshot.len > 0
                && snapshot.len <= 512 * 1024 * 1024,
            "epoch executable root mode/size differs",
        )?;
        require(
            file.metadata()?.nlink() == 1,
            "epoch executable cannot be hard-linked",
        )?;
        require(
            hash_reader(&mut file)? == expected,
            "epoch executable hash differs",
        )?;
        ensure_pinned_unchanged(path, "epoch executable", &file, &snapshot)
    }
    fn validate_executables(plan: &EpochSupervisorPlanV1) -> Result<()> {
        epoch_supervisor::validate_generation(plan)?;
        executable(Path::new(&plan.cli_path), &plan.iroha_sha256)?;
        require(
            crate::compiled_build_identity()?.release_source_commit()?
                == plan.release_source_commit,
            "generation command is not the selected native source release",
        )?;
        executable(&std::env::current_exe()?, &plan.iroha_sha256)
    }

    fn check_deadline(until: Instant) -> Result<()> {
        require(Instant::now() < until, "epoch generation deadline elapsed")
    }
    fn generation_path(plan: &EpochSupervisorPlanV1) -> PathBuf {
        PathBuf::from(STATE_ROOT)
            .join("generations")
            .join(&plan.policy_sha256)
    }
    fn public_files(plan: &EpochSupervisorPlanV1) -> Vec<(&str, &[u8])> {
        vec![
            ("policy.json", &plan.policy_bytes),
            ("trust.json", &plan.observation_trust_bytes),
            ("unit.service", &plan.unit_bytes),
        ]
    }
    fn private_bytes(path: &Path, maximum: u64) -> Result<Zeroizing<Vec<u8>>> {
        identity(path, false)?;
        let (file, snapshot) = open_pinned_regular(path, "epoch native private input")?;
        require(
            snapshot.len > 0 && snapshot.len <= maximum,
            "epoch private input size differs",
        )?;
        Ok(Zeroizing::new(read_pinned_bytes(
            path,
            "epoch native private input",
            file,
            &snapshot,
            maximum,
        )?))
    }
    fn admit_private(
        plan: &EpochSupervisorPlanV1,
        admin: &[u8],
        http: &[u8],
    ) -> Result<(AdministratorV1, String)> {
        require(
            !admin.is_empty()
                && u64::try_from(admin.len())? <= iroha_config_base::toml::MAX_TOML_SOURCE_BYTES
                && !http.is_empty()
                && http.len() <= 4096,
            "epoch native private input size differs",
        )?;
        require(
            sha256_hex(admin) == plan.admin_config_sha256
                && sha256_hex(http) == plan.http_operator_key_sha256,
            "epoch native private input digest differs from admitted custody",
        )?;
        let (config, _) = ClientConfig::load_bytes_with_musubi_publication(
            Path::new(&plan.admin_config_path),
            admin,
        )
        .map_err(|_| eyre!("epoch administrator failed native strict config admission"))?;
        let operator: KeyPair = crate::operator_key::parse_operator_private_key(http)?;
        // The native observer validates the policy, signed trust, network/account identity,
        // separate HTTP operator and explicitly admitted candidate origin.
        crate::taira_dataspace_deploy::epoch_maintenance::supervisor_generation_admission(
            &plan.policy_bytes,
            &plan.observation_trust_bytes,
            &config,
            &operator,
        )?;
        let _chain_guard = ChainDiscriminantGuard::enter(config.account_chain_discriminant);
        let identity = AdministratorV1 {
            account_id: config.account.to_string(),
            public_key: config.key_pair.public_key().to_string(),
            network_id: config.network_id.to_string(),
            chain_discriminant: config.account_chain_discriminant,
            torii_origin: config.torii_api_url.as_str().into(),
        };
        Ok((identity, operator.public_key().to_string()))
    }
    pub(super) fn preflight_reset(plan: &EpochSupervisorPlanV1, until: Instant) -> Result<()> {
        root()?;
        check_deadline(until)?;
        epoch_supervisor::validate_generation(plan)?;
        match fs::symlink_metadata(generation_path(plan)) {
            Ok(_) => {
                preflight(plan, until)?;
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
        check_deadline(until)
    }
    pub(super) fn preflight(
        plan: &EpochSupervisorPlanV1,
        until: Instant,
    ) -> Result<(AdministratorV1, String)> {
        root()?;
        check_deadline(until)?;
        epoch_supervisor::validate_generation(plan)?;
        identity(Path::new(STATE_ROOT), true)?;
        identity(Path::new(JOURNAL_DIR), true)?;
        let directory = generation_path(plan);
        identity(&directory, true)?;
        for (name, expected) in public_files(plan) {
            let actual = private_bytes(&directory.join(name), LIMIT)?;
            require(
                actual.as_slice() == expected,
                "immutable epoch public generation input differs",
            )?;
        }
        let admin = private_bytes(
            Path::new(&plan.admin_config_path),
            iroha_config_base::toml::MAX_TOML_SOURCE_BYTES,
        )?;
        let http = private_bytes(Path::new(&plan.http_operator_key_path), 4096)?;
        let admitted = admit_private(plan, &admin, &http)?;
        epoch_supervisor::validate_generation(plan)?;
        executable(Path::new(&plan.cli_path), &plan.iroha_sha256)?;
        check_deadline(until)?;
        Ok(admitted)
    }
    pub(super) fn materialize(
        plan: &EpochSupervisorPlanV1,
        admin: &[u8],
        http: &[u8],
        until: Instant,
    ) -> Result<(AdministratorV1, String)> {
        root()?;
        check_deadline(until)?;
        epoch_supervisor::validate_generation(plan)?;
        let admitted = admit_private(plan, admin, http)?;
        validate_executables(plan)?;
        check_deadline(until)?;
        ensure_root_private_directory(Path::new(STATE_ROOT))?;
        ensure_root_private_directory(&Path::new(STATE_ROOT).join("generations"))?;
        ensure_root_private_directory(Path::new(JOURNAL_DIR))?;
        let directory = generation_path(plan);
        ensure_root_private_directory(&directory)?;
        for (name, bytes) in public_files(plan) {
            publish_root_private_noreplace(&directory, name, bytes)?;
        }
        publish_root_private_noreplace(&directory, "administrator.toml", admin)?;
        publish_root_private_noreplace(&directory, "http-operator.key", http)?;
        preflight(plan, until)?;
        Ok(admitted)
    }
    #[allow(
        unsafe_code,
        reason = "fcntl validates an untrusted inherited descriptor before File ownership is constructed"
    )]
    fn duplicate_descriptor(fd: u32) -> Result<File> {
        use std::os::fd::FromRawFd as _;
        unsafe extern "C" {
            fn fcntl(fd: std::ffi::c_int, command: std::ffi::c_int, ...) -> std::ffi::c_int;
        }
        let original = i32::try_from(fd)?;
        // SAFETY: Linux F_DUPFD_CLOEXEC validates the integer; only the returned unique fd is owned.
        let copied = unsafe { fcntl(original, 1030, 3 as std::ffi::c_int) };
        require(copied >= 0, "failed to retain deployment lock descriptor")?;
        Ok(unsafe { File::from_raw_fd(copied) })
    }
    pub(super) fn deployment_lock(inherited: Option<u32>, until: Instant) -> Result<File> {
        use rustix::fs::{FlockOperation as F, Mode, OFlags};
        root()?;
        check_deadline(until)?;
        ensure_root_private_directory(Path::new(STATE_ROOT))?;
        let file = if let Some(fd) = inherited {
            duplicate_descriptor(fd)?
        } else {
            File::from(rustix::fs::open(
                LOCK,
                OFlags::RDWR | OFlags::CREATE | OFlags::CLOEXEC | OFlags::NOFOLLOW,
                Mode::RUSR | Mode::WUSR,
            )?)
        };
        let named = identity(Path::new(LOCK), false)?;
        let held = file.metadata()?;
        require(
            held.dev() == named.device
                && held.ino() == named.inode
                && held.uid() == 0
                && held.gid() == 0
                && held.mode() & 0o7777 == 0o600
                && held.nlink() == 1,
            "deployment lock descriptor differs from fixed root custody",
        )?;
        if inherited.is_some() {
            let probe = File::from(rustix::fs::open(
                LOCK,
                OFlags::RDWR | OFlags::CLOEXEC | OFlags::NOFOLLOW,
                Mode::empty(),
            )?);
            match rustix::fs::flock(&probe, F::NonBlockingLockExclusive) {
                Err(rustix::io::Errno::WOULDBLOCK) => {}
                Ok(()) => return Err(eyre!("inherited deployment lock is not held by its caller")),
                Err(e) => return Err(e.into()),
            }
            // This succeeds only for the same held open-file description; never unlock it.
            rustix::fs::flock(&file, F::NonBlockingLockExclusive)?;
        } else {
            loop {
                match rustix::fs::flock(&file, F::NonBlockingLockExclusive) {
                    Ok(()) => break,
                    Err(rustix::io::Errno::WOULDBLOCK) => {
                        check_deadline(until)?;
                        std::thread::sleep(Duration::from_millis(25));
                    }
                    Err(e) => return Err(e.into()),
                }
            }
        }
        let final_named = identity(Path::new(LOCK), false)?;
        require(
            final_named == named,
            "deployment lock changed during acquisition",
        )?;
        Ok(file)
    }
    const UNIT_PATH: &str = "/etc/systemd/system/iroha-taira-epoch-supervisor.service";
    fn installed_unit(plan: Option<&EpochSupervisorPlanV1>) -> Result<()> {
        match plan {
            Some(plan) => {
                require_root_no_symlink_ancestors(Path::new(UNIT_PATH), "fixed epoch unit")?;
                let (file, snapshot) =
                    open_pinned_regular(Path::new(UNIT_PATH), "fixed epoch unit")?;
                let m = file.metadata()?;
                require(
                    m.uid() == 0 && m.gid() == 0 && m.nlink() == 1 && m.mode() & 0o7777 == 0o644,
                    "fixed epoch unit root custody differs",
                )?;
                let bytes = read_pinned_bytes(
                    Path::new(UNIT_PATH),
                    "fixed epoch unit",
                    file,
                    &snapshot,
                    LIMIT,
                )?;
                require(
                    bytes == plan.unit_bytes,
                    "installed epoch unit differs from explicit installed binding",
                )
            }
            None => match fs::symlink_metadata(UNIT_PATH) {
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                _ => Err(eyre!(
                    "epoch unit exists although explicit installed binding is absent"
                )),
            },
        }
    }
    fn manager(until: Instant) -> Result<BTreeMap<String, String>> {
        check_deadline(until)?;
        let mut runner = RealProcessRunner;
        let result=runner.run(&ProcessSpec {program:PathBuf::from("/usr/bin/systemctl"),
            args:["show",UNIT_NAME,"--no-pager","--property=LoadState,ActiveState,SubState,MainPID,ControlPID,ControlGroup,FragmentPath,DropInPaths,InvocationID,NRestarts,Job"].iter().map(OsString::from).collect(),
            stdin_prefix:Vec::new(),stdin_file:None,stdin_files:Vec::new(),inherited_files:Vec::new(),deadline:until})?;
        let status = result.status;
        let bytes = result.stdout;
        let text = std::str::from_utf8(&bytes)?;
        let mut fields = BTreeMap::new();
        for line in text.lines() {
            let (key, value) = line
                .split_once('=')
                .ok_or_else(|| eyre!("invalid manager observation"))?;
            require(
                fields.insert(key.to_owned(), value.to_owned()).is_none(),
                "duplicate manager observation",
            )?;
        }
        let expected = [
            "LoadState",
            "ActiveState",
            "SubState",
            "MainPID",
            "ControlPID",
            "ControlGroup",
            "FragmentPath",
            "DropInPaths",
            "InvocationID",
            "NRestarts",
            "Job",
        ];
        require(
            fields.len() == expected.len() && expected.iter().all(|key| fields.contains_key(*key)),
            "manager observation has missing or unexpected fields",
        )?;
        require(
            status.success() || (status.code() == Some(1) && fields["LoadState"] == "not-found"),
            "epoch manager show failed",
        )?;
        require(
            fields["DropInPaths"].is_empty()
                && fields["ControlPID"] == "0"
                && (fields["Job"].is_empty() || fields["Job"] == "0"),
            "epoch manager has overrides or an in-flight control job",
        )?;
        Ok(fields)
    }
    fn stopped(plan: Option<&EpochSupervisorPlanV1>, until: Instant) -> Result<()> {
        installed_unit(plan)?;
        let state = manager(until)?;
        require(
            state["MainPID"] == "0"
                && matches!(state["ActiveState"].as_str(), "inactive" | "failed"),
            "epoch supervisor is not stopped",
        )?;
        if plan.is_some() {
            require(
                state["LoadState"] == "loaded" && state["FragmentPath"] == UNIT_PATH,
                "stopped supervisor manager binding differs",
            )?;
        } else {
            require(
                state["LoadState"] == "not-found" && state["FragmentPath"].is_empty(),
                "absent supervisor manager binding differs",
            )?;
        }
        if !state["ControlGroup"].is_empty() {
            require(
                state["ControlGroup"] == format!("/system.slice/{UNIT_NAME}"),
                "epoch manager control group differs",
            )?;
            let path = PathBuf::from("/sys/fs/cgroup")
                .join(state["ControlGroup"].trim_start_matches('/'))
                .join("cgroup.procs");
            match fs::read_to_string(path) {
                Ok(contents) => require(
                    contents.trim().is_empty(),
                    "stopped epoch unit retains live cgroup processes",
                )?,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e.into()),
            }
        }
        Ok(())
    }
    fn expected_argv(plan: &EpochSupervisorPlanV1) -> Result<Vec<String>> {
        epoch_supervisor::validate_generation(plan)?;
        Ok(vec![
            plan.cli_path.clone(),
            "--config".into(),
            plan.admin_config_path.clone(),
            "--operator-private-key-file".into(),
            plan.http_operator_key_path.clone(),
            "taira".into(),
            "epoch-maintenance".into(),
            "supervise".into(),
            "--policy".into(),
            plan.policy_path.clone(),
            "--trust".into(),
            plan.trust_path.clone(),
            "--journal-dir".into(),
            plan.journal_dir.clone(),
            "--timeout-ms".into(),
            plan.timeout_ms.to_string(),
        ])
    }
    pub(super) fn observe(
        plan: &EpochSupervisorPlanV1,
        until: Instant,
    ) -> Result<ObservedEpochWorkerV1> {
        preflight(plan, until)?;
        installed_unit(Some(plan))?;
        let before = manager(until)?;
        require(
            before["LoadState"] == "loaded"
                && before["ActiveState"] == "active"
                && before["SubState"] == "running"
                && before["FragmentPath"] == UNIT_PATH
                && before["ControlGroup"] == format!("/system.slice/{UNIT_NAME}"),
            "epoch manager is not the admitted running unit",
        )?;
        let pid = before["MainPID"].parse::<u32>()?;
        require(pid > 1, "epoch manager has no live main PID")?;
        let (boot_id, actual_pid, start_time_ticks) = epoch_worker_process_identity_for(pid)?;
        require(actual_pid == pid, "epoch worker PID differs")?;
        let argv = expected_argv(plan)?;
        let expected = argv
            .iter()
            .flat_map(|arg| arg.as_bytes().iter().copied().chain(std::iter::once(0)))
            .collect::<Vec<_>>();
        let mut actual = Vec::new();
        File::open(format!("/proc/{pid}/cmdline"))?
            .take(16 * 1024 + 1)
            .read_to_end(&mut actual)?;
        require(
            actual == expected,
            "epoch worker argv differs from fixed native command",
        )?;
        let executable_path = PathBuf::from(&argv[0]);
        require(
            fs::read_link(format!("/proc/{pid}/exe"))? == executable_path,
            "epoch worker executable path differs",
        )?;
        let running = fs::metadata(format!("/proc/{pid}/exe"))?;
        let selected = fs::metadata(&executable_path)?;
        require(
            running.dev() == selected.dev() && running.ino() == selected.ino(),
            "epoch worker executable inode differs",
        )?;
        let after = manager(until)?;
        require(
            before == after
                && epoch_worker_process_identity_for(pid)?
                    == (boot_id.clone(), pid, start_time_ticks),
            "epoch worker changed during observation",
        )?;
        let invocation_id = before["InvocationID"].clone();
        require(
            invocation_id.len() == 32 && invocation_id.bytes().all(|b| b.is_ascii_hexdigit()),
            "epoch manager invocation identity is invalid",
        )?;
        Ok(ObservedEpochWorkerV1 {
            boot_id,
            pid,
            start_time_ticks,
            invocation_id,
            n_restarts: before["NRestarts"].parse()?,
        })
    }
    pub(super) fn quiescent(
        plan: &EpochSupervisorPlanV1,
        until: Instant,
    ) -> Result<Option<crate::taira_dataspace_deploy::epoch_maintenance::SupervisorJournalGuard>>
    {
        epoch_supervisor::validate_generation(plan)?;
        let absent = plan.prior_state == "absent"
            && matches!(fs::symlink_metadata(UNIT_PATH),Err(ref e) if e.kind()==std::io::ErrorKind::NotFound);
        let selected = if absent { None } else { Some(plan) };
        stopped(selected, until)?;
        let policy = epoch_supervisor::validate_generation(plan)?;
        identity(Path::new(JOURNAL_DIR), true)?;
        let guard = crate::taira_dataspace_deploy::epoch_maintenance::supervisor_journal_guard(
            Path::new(JOURNAL_DIR),
            policy.intent.network_id,
        )?;
        guard.revalidate()?;
        stopped(selected, until)?;
        Ok(Some(guard))
    }
    fn select_installed<'a>(
        installed: Option<&'a EpochSupervisorPlanV1>,
        after: &'a EpochSupervisorPlanV1,
    ) -> Result<Option<&'a EpochSupervisorPlanV1>> {
        if installed_unit(Some(after)).is_ok() {
            return Ok(Some(after));
        }
        installed_unit(installed)?;
        Ok(installed)
    }
    fn service_observation(
        plan: Option<&EpochSupervisorPlanV1>,
        until: Instant,
    ) -> Result<(String, Option<ObservedEpochWorkerV1>)> {
        if let Some(plan) = plan {
            let state = manager(until)?;
            if state["ActiveState"] == "active" {
                return Ok(("running".into(), Some(observe(plan, until)?)));
            }
            stopped(Some(plan), until)?;
            Ok(("stopped".into(), None))
        } else {
            stopped(None, until)?;
            Ok(("absent".into(), None))
        }
    }

    pub(super) fn materialize_reset(
        plan: &EpochSupervisorPlanV1,
        admin: &[u8],
        http: &[u8],
        reset_operation: &str,
        until: Instant,
    ) -> Result<()> {
        operation(reset_operation)?;
        let (administrator, http_operator_public_key) = materialize(plan, admin, http, until)?;
        let binding = binding_from_plan(plan)?;
        let receipt = GenerationReceiptV1 {
            schema: "iroha.taira.epoch-supervisor-generation.v1".into(),
            operation: reset_operation.into(),
            original_service_state: plan.prior_state.clone(),
            successor_service_state: "running".into(),
            binding_sha256: binding_digest(&binding)?,
            policy_sha256: plan.policy_sha256.clone(),
            release_source_commit: plan.release_source_commit.clone(),
            iroha_sha256: plan.iroha_sha256.clone(),
            observation_trust_sha256: plan.observation_trust_sha256.clone(),
            unit_sha256: plan.unit_sha256.clone(),
            administrator,
            admin_config_sha256: plan.admin_config_sha256.clone(),
            http_operator_key_sha256: plan.http_operator_key_sha256.clone(),
            http_operator_public_key,
            generation: identity(&generation_path(plan), true)?,
            journal: identity(Path::new(JOURNAL_DIR), true)?,
            files: receipt_metadata(plan)?,
        };
        publish_root_private_noreplace(&generation_path(plan), RECEIPT, &json::to_vec(&receipt)?)
    }
    fn receipt_metadata(plan: &EpochSupervisorPlanV1) -> Result<Vec<MetadataV1>> {
        let directory = generation_path(plan);
        [
            "administrator.toml",
            "http-operator.key",
            "policy.json",
            "trust.json",
            "unit.service",
        ]
        .into_iter()
        .map(|name| identity(&directory.join(name), false))
        .collect()
    }
    fn read_receipt(binding: &BindingV1) -> Result<GenerationReceiptV1> {
        let path = generation(binding).join(RECEIPT);
        identity(&path, false)?;
        public_json(&path)
    }
    fn plan_from_receipt(binding: &BindingV1) -> Result<EpochSupervisorPlanV1> {
        let receipt = read_receipt(binding)?;
        require(
            receipt.schema == "iroha.taira.epoch-supervisor-generation.v1"
                && receipt.binding_sha256 == binding_digest(binding)?
                && receipt.policy_sha256 == binding.policy_sha256
                && receipt.unit_sha256 == binding.unit_sha256
                && receipt.release_source_commit == binding.release_source_commit
                && receipt.iroha_sha256 == binding.iroha_sha256
                && receipt.observation_trust_sha256 == binding.observation_trust_sha256,
            "epoch provisioning receipt does not bind the selected generation",
        )?;
        let plan = as_plan(
            binding,
            receipt.admin_config_sha256.clone(),
            receipt.http_operator_key_sha256.clone(),
            &receipt.original_service_state,
        )?;
        require(
            receipt.generation == identity(&generation(binding), true)?
                && receipt.journal == identity(Path::new(JOURNAL_DIR), true)?
                && receipt.files == receipt_metadata(&plan)?,
            "epoch provisioning metadata changed",
        )?;
        Ok(plan)
    }
    fn validate_receipt(wrapper: &WrapperV1) -> Result<EpochSupervisorPlanV1> {
        let expected = generation(&wrapper.after).join(RECEIPT);
        require(
            Path::new(&wrapper.native_provisioning_receipt.path) == expected,
            "provisioning receipt path escapes the selected generation",
        )?;
        let bytes = private_bytes(&expected, LIMIT)?;
        require(
            sha256_hex(&bytes) == wrapper.native_provisioning_receipt.sha256,
            "provisioning receipt raw hash differs",
        )?;
        let receipt: GenerationReceiptV1 = json::from_slice(&bytes)?;
        require(
            receipt.operation == wrapper.operation
                && receipt.original_service_state == wrapper.original_service_state
                && receipt.successor_service_state == wrapper.successor_service_state,
            "provisioning receipt deployment intent differs",
        )?;
        plan_from_receipt(&wrapper.after)
    }
    fn write_line<W: Write, T: JsonSerialize>(output: &mut W, value: &T) -> Result<()> {
        output.write_all(&json::to_vec(value)?)?;
        output.write_all(b"\n")?;
        output.flush()?;
        Ok(())
    }
    fn hold_until_eof(
        guard: &crate::taira_dataspace_deploy::epoch_maintenance::SupervisorJournalGuard,
        until: Instant,
    ) -> Result<()> {
        let stdin = std::io::stdin();
        let mode = rustix::fs::fstat(&stdin)?;
        require(
            rustix::fs::FileType::from_raw_mode(mode.st_mode) == rustix::fs::FileType::Fifo,
            "quiescence requires a retained controller pipe, not a terminal or regular file",
        )?;
        loop {
            check_deadline(until)?;
            guard.revalidate()?;
            let timeout = rustix::event::Timespec::try_from(
                Duration::from_millis(250).min(until.saturating_duration_since(Instant::now())),
            )?;
            let mut descriptors = [rustix::event::PollFd::new(
                &stdin,
                rustix::event::PollFlags::IN,
            )];
            match rustix::event::poll(&mut descriptors, Some(&timeout)) {
                Ok(0) => continue,
                Err(rustix::io::Errno::INTR) => continue,
                Err(e) => return Err(e.into()),
                Ok(_) => {}
            }
            let mut byte = [0u8; 1];
            match rustix::io::read(&stdin, &mut byte) {
                Ok(0) => {
                    guard.revalidate()?;
                    return Ok(());
                }
                Ok(_) => return Err(eyre!("quiescence control pipe accepts EOF only")),
                Err(rustix::io::Errno::INTR) => {}
                Err(e) => return Err(e.into()),
            }
        }
    }
    pub(super) fn run<W: Write>(args: &EpochSupervisorHost, output: &mut W) -> Result<()> {
        root()?;
        let until = deadline(args.timeout_ms)?;
        let _deployment = deployment_lock(args.deployment_lock_fd, until)?;
        reject_reset_owner()?;
        if args.action == GenerationAction::Materialize {
            let input: PreparationV1 = public_json(&args.wrapper)?;
            require(
                input.schema == "taira.epoch-supervisor-generation-preparation.v1",
                "epoch preparation schema differs",
            )?;
            operation(&input.operation)?;
            intent(
                &input.original_service_state,
                &input.successor_service_state,
                input.before.is_some(),
            )?;
            require(
                input
                    .before
                    .as_ref()
                    .is_none_or(|v| v.network_id == input.after.network_id)
                    && input
                        .installed
                        .as_ref()
                        .is_none_or(|v| v.network_id == input.after.network_id),
                "updater cannot change the epoch journal network",
            )?;
            if let Some(before) = &input.before {
                plan_from_receipt(before)?;
            }
            let installed = input
                .installed
                .as_ref()
                .map(plan_from_receipt)
                .transpose()?;
            installed_unit(installed.as_ref())?;
            let admin = crate::client_config::read_inherited_private_file(
                args.administrator_config_fd
                    .ok_or_else(|| eyre!("administrator descriptor required"))?,
                iroha_config_base::toml::MAX_TOML_SOURCE_BYTES,
                "epoch administrator",
            )?;
            let http = crate::client_config::read_inherited_private_file(
                args.http_operator_key_fd
                    .ok_or_else(|| eyre!("HTTP operator descriptor required"))?,
                4096,
                "epoch HTTP operator",
            )?;
            let plan = as_plan(
                &input.after,
                sha256_hex(&admin),
                sha256_hex(&http),
                &input.original_service_state,
            )?;
            // Admit the complete public closure and separate credentials before publication.
            admit_private(&plan, &admin, &http)?;
            validate_executables(&plan)?;
            check_deadline(until)?;
            let (administrator, http_operator_public_key) =
                materialize(&plan, &admin, &http, until)?;
            let receipt = GenerationReceiptV1 {
                schema: "iroha.taira.epoch-supervisor-generation.v1".into(),
                operation: input.operation.clone(),
                original_service_state: input.original_service_state.clone(),
                successor_service_state: input.successor_service_state.clone(),
                binding_sha256: binding_digest(&input.after)?,
                policy_sha256: plan.policy_sha256.clone(),
                release_source_commit: plan.release_source_commit.clone(),
                iroha_sha256: plan.iroha_sha256.clone(),
                observation_trust_sha256: plan.observation_trust_sha256.clone(),
                unit_sha256: plan.unit_sha256.clone(),
                administrator,
                admin_config_sha256: plan.admin_config_sha256.clone(),
                http_operator_key_sha256: plan.http_operator_key_sha256.clone(),
                http_operator_public_key,
                generation: identity(&generation_path(&plan), true)?,
                journal: identity(Path::new(JOURNAL_DIR), true)?,
                files: receipt_metadata(&plan)?,
            };
            let bytes = json::to_vec(&receipt)?;
            publish_root_private_noreplace(&generation_path(&plan), RECEIPT, &bytes)?;
            let wrapper = completed_update_wrapper(
                input,
                ReceiptReferenceV1 {
                    path: generation_path(&plan)
                        .join(RECEIPT)
                        .to_string_lossy()
                        .into_owned(),
                    sha256: sha256_hex(&bytes),
                },
            )?;
            // Stdout is the complete closed --supervisor-plan input. Its native
            // receipt is already durable; callers never merge public JSON by hand.
            return write_line(output, &wrapper);
        }
        require(
            args.administrator_config_fd.is_none() && args.http_operator_key_fd.is_none(),
            "private descriptors are accepted only for materialize",
        )?;
        let wrapper: WrapperV1 = public_json(&args.wrapper)?;
        require(
            wrapper.schema == "taira.epoch-supervisor-update.v1",
            "epoch update wrapper schema differs",
        )?;
        operation(&wrapper.operation)?;
        intent(
            &wrapper.original_service_state,
            &wrapper.successor_service_state,
            wrapper.before.is_some(),
        )?;
        require(
            wrapper
                .before
                .as_ref()
                .is_none_or(|v| v.network_id == wrapper.after.network_id)
                && wrapper
                    .installed
                    .as_ref()
                    .is_none_or(|v| v.network_id == wrapper.after.network_id),
            "updater cannot change the epoch journal network",
        )?;
        if let Some(before) = &wrapper.before {
            plan_from_receipt(before)?;
        }
        let after = validate_receipt(&wrapper)?;
        preflight(&after, until)?;
        validate_executables(&after)?;
        let installed = wrapper
            .installed
            .as_ref()
            .map(plan_from_receipt)
            .transpose()?;
        let mut receipt = ActionReceiptV1 {
            schema: "iroha.taira.epoch-supervisor-host.v1".into(),
            action: format!("{:?}", args.action).to_ascii_lowercase(),
            service_state: String::new(),
            operation: wrapper.operation.clone(),
            policy_sha256: after.policy_sha256.clone(),
            unit_sha256: after.unit_sha256.clone(),
            provisioning_receipt: wrapper.native_provisioning_receipt.clone(),
            installed_policy_sha256: installed.as_ref().map(|p| p.policy_sha256.clone()),
            journal: identity(Path::new(JOURNAL_DIR), true)?,
            worker: None,
            status: None,
        };
        match args.action {
            GenerationAction::Preflight => {
                let (state, _) = service_observation(installed.as_ref(), until)?;
                receipt.service_state = state;
            }
            GenerationAction::Observe => {
                let selected = select_installed(installed.as_ref(), &after)?;
                receipt.installed_policy_sha256 = selected.map(|p| p.policy_sha256.clone());
                let (state, worker) = service_observation(selected, until)?;
                receipt.service_state = state;
                receipt.worker = worker;
            }
            GenerationAction::Status => {
                // Status qualifies the explicitly selected successor after its publication.
                installed_unit(Some(&after))?;
                receipt.installed_policy_sha256 = Some(after.policy_sha256.clone());
                receipt.service_state = "running".into();
                let status = epoch_supervisor::status_generation(&after, until)?;
                let worker = observe(&after, until)?;
                let identity = status
                    .get("worker")
                    .ok_or_else(|| eyre!("native status worker missing"))?;
                require(
                    identity.get("boot_id").and_then(json::Value::as_str)
                        == Some(worker.boot_id.as_str())
                        && identity.get("pid").and_then(json::Value::as_u64)
                            == Some(u64::from(worker.pid))
                        && identity
                            .get("start_time_ticks")
                            .and_then(json::Value::as_u64)
                            == Some(worker.start_time_ticks),
                    "epoch worker changed after native authenticated status",
                )?;
                receipt.status = Some(status);
                receipt.worker = Some(worker);
            }
            GenerationAction::Quiescence => {
                let actual = select_installed(installed.as_ref(), &after)?;
                stopped(actual, until)?;
                receipt.installed_policy_sha256 = actual.map(|p| p.policy_sha256.clone());
                receipt.service_state = if actual.is_some() {
                    "stopped"
                } else {
                    "absent"
                }
                .into();
                let policy = epoch_supervisor::validate_generation(actual.unwrap_or(&after))?;
                let guard =
                    crate::taira_dataspace_deploy::epoch_maintenance::supervisor_journal_guard(
                        Path::new(JOURNAL_DIR),
                        policy.intent.network_id,
                    )?;
                guard.revalidate()?;
                stopped(actual, until)?;
                write_line(output, &receipt)?;
                hold_until_eof(&guard, until)?;
                // The controller may have published exactly the successor while the guard was held.
                // Re-select only those two admitted unit closures; the journal network is unchanged.
                let final_installed = select_installed(installed.as_ref(), &after)?;
                stopped(final_installed, until)?;
                guard.revalidate()?;
                return Ok(());
            }
            GenerationAction::Materialize => unreachable!(),
        }
        write_line(output, &receipt)
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        fn fixture_binding() -> BindingV1 {
            let inventory = super::super::super::super::sample_inventory_fixture();
            binding_from_plan(&inventory.epoch_supervisor).unwrap()
        }
        #[test]
        fn preparation_requires_explicit_installed_and_successor_intent() {
            let after = fixture_binding();
            let preparation = PreparationV1 {
                schema: "taira.epoch-supervisor-generation-preparation.v1".into(),
                operation: "synthetic-first-install".into(),
                original_service_state: "absent".into(),
                successor_service_state: "stopped".into(),
                before: None,
                installed: None,
                after,
            };
            let bytes = json::to_vec(&preparation).unwrap();
            let mut value: json::Value = json::from_slice(&bytes).unwrap();
            assert!(json::from_slice::<PreparationV1>(&bytes).is_ok());
            let object = value.as_object_mut().unwrap();
            object.remove("installed");
            assert!(json::from_slice::<PreparationV1>(&json::to_vec(&value).unwrap()).is_err());
            let mut with_retired_sources: json::Value = json::from_slice(&bytes).unwrap();
            with_retired_sources.as_object_mut().unwrap().insert(
                "original_seed_sources".into(),
                json::Value::Array(Vec::new()),
            );
            assert!(
                json::from_slice::<PreparationV1>(&json::to_vec(&with_retired_sources).unwrap())
                    .is_err()
            );
            let mut value: json::Value = json::from_slice(&bytes).unwrap();
            value
                .as_object_mut()
                .unwrap()
                .remove("successor_service_state");
            assert!(json::from_slice::<PreparationV1>(&json::to_vec(&value).unwrap()).is_err());
            let mut value: json::Value = json::from_slice(&bytes).unwrap();
            value
                .as_object_mut()
                .unwrap()
                .insert("native_provisioning_receipt".into(), json::Value::Null);
            assert!(json::from_slice::<PreparationV1>(&json::to_vec(&value).unwrap()).is_err());
        }
        #[test]
        fn generation_binding_rejects_alternate_cli_and_private_path() {
            let inventory = super::super::super::super::sample_inventory_fixture();
            let plan = &inventory.epoch_supervisor;
            let binding = binding_from_plan(plan).unwrap();
            let admitted = as_plan(
                &binding,
                plan.admin_config_sha256.clone(),
                plan.http_operator_key_sha256.clone(),
                "absent",
            )
            .unwrap();
            assert_eq!(admitted.unit_bytes, plan.unit_bytes);
            let mut alternate = binding.clone();
            alternate.unit_spec.cli = "/tmp/iroha".into();
            assert!(
                as_plan(
                    &alternate,
                    plan.admin_config_sha256.clone(),
                    plan.http_operator_key_sha256.clone(),
                    "absent"
                )
                .is_err()
            );
            let mut alternate = binding;
            alternate.unit_spec.admin_config = "/tmp/admin.toml".into();
            assert!(
                as_plan(
                    &alternate,
                    plan.admin_config_sha256.clone(),
                    plan.http_operator_key_sha256.clone(),
                    "absent"
                )
                .is_err()
            );
        }
        #[test]
        fn service_intent_never_infers_activation_from_original_absence() {
            assert!(intent("absent", "running", false).is_ok());
            assert!(intent("absent", "stopped", false).is_ok());
            assert!(intent("running", "running", true).is_ok());
            assert!(intent("stopped", "stopped", true).is_ok());
            for (original, successor, before) in [
                ("absent", "", false),
                ("absent", "running", true),
                ("stopped", "running", true),
                ("running", "stopped", true),
                ("running", "running", false),
            ] {
                assert!(intent(original, successor, before).is_err());
            }
        }
    }
}

#[cfg(test)]
mod public_binding_tests {
    use super::*;

    #[test]
    fn public_projection_cannot_satisfy_native_credential_admission() {
        let inventory = super::super::super::sample_inventory_fixture();
        let _chain = ChainDiscriminantGuard::enter(inventory.chain_discriminant);
        let plan = inventory.epoch_supervisor;
        let binding = binding_from_plan(&plan).expect("fully pinned fixture binding");
        let projection = as_public_plan(&binding, "absent").expect("public closure");
        assert!(projection.admin_config_sha256.is_empty());
        assert!(projection.http_operator_key_sha256.is_empty());
        assert!(epoch_supervisor::validate_generation(&projection).is_err());
        assert!(binding_from_plan(&projection).is_err());
        assert_eq!(
            json::to_vec(&binding).unwrap(),
            json::to_vec(&binding_from_public_plan(&projection).unwrap()).unwrap()
        );
        assert!(
            as_plan(
                &binding,
                String::new(),
                plan.http_operator_key_sha256.clone(),
                "absent"
            )
            .is_err()
        );
        assert!(
            as_plan(
                &binding,
                plan.admin_config_sha256.clone(),
                "0".repeat(64),
                "absent"
            )
            .is_err()
        );
        as_plan(
            &binding,
            plan.admin_config_sha256,
            plan.http_operator_key_sha256,
            "absent",
        )
        .expect("native actual digests complete admission");
    }

    #[test]
    fn public_binding_still_rejects_changed_hash_argv_and_network() {
        let inventory = super::super::super::sample_inventory_fixture();
        let _chain = ChainDiscriminantGuard::enter(inventory.chain_discriminant);
        let binding = binding_from_plan(&inventory.epoch_supervisor).unwrap();
        let mut changed = binding.clone();
        changed.policy_sha256 = "f".repeat(64);
        assert!(as_public_plan(&changed, "absent").is_err());
        changed = binding.clone();
        changed.unit_spec.cli = "/srv/other/bin/iroha".into();
        assert!(as_public_plan(&changed, "absent").is_err());
        changed = binding;
        changed.network_id = "wrong-network".into();
        assert!(as_public_plan(&changed, "absent").is_err());
    }
}

#[cfg(test)]
mod completed_wrapper_tests {
    use super::*;

    fn fixture() -> (PreparationV1, ReceiptReferenceV1) {
        let inventory = super::super::super::sample_inventory_fixture();
        let after = binding_from_plan(&inventory.epoch_supervisor).unwrap();
        let receipt = ReceiptReferenceV1 {
            path: format!(
                "{STATE_ROOT}/generations/{}/provisioning-receipt.json",
                after.policy_sha256
            ),
            sha256: "a".repeat(64),
        };
        (
            PreparationV1 {
                schema: "taira.epoch-supervisor-generation-preparation.v1".into(),
                operation: format!("update-{}", "1".repeat(32)),
                original_service_state: "absent".into(),
                successor_service_state: "running".into(),
                before: None,
                installed: None,
                after,
            },
            receipt,
        )
    }

    #[test]
    fn materialization_output_is_complete_closed_update_input() {
        let _guard = ChainDiscriminantGuard::enter(super::super::super::CHAIN_DISCRIMINANT);
        let (input, receipt) = fixture();
        let original_binding = json::to_vec(&input.after).unwrap();
        let expected_receipt = json::to_vec(&receipt).unwrap();
        let wrapper = completed_update_wrapper(input, receipt).unwrap();
        let bytes = json::to_vec(&wrapper).unwrap();
        let decoded: WrapperV1 = json::from_slice(&bytes).unwrap();
        assert_eq!(decoded.schema, "taira.epoch-supervisor-update.v1");
        assert_eq!(json::to_vec(&decoded.after).unwrap(), original_binding);
        assert_eq!(
            json::to_vec(&decoded.native_provisioning_receipt).unwrap(),
            expected_receipt
        );
        let value: json::Value = json::from_slice(&bytes).unwrap();
        let object = value.as_object().unwrap();
        let mut fields: Vec<_> = object.keys().map(String::as_str).collect();
        fields.sort_unstable();
        assert_eq!(
            fields,
            vec![
                "after",
                "before",
                "installed",
                "native_provisioning_receipt",
                "operation",
                "original_service_state",
                "schema",
                "successor_service_state"
            ]
        );
        for key in ["installed", "native_provisioning_receipt"] {
            let mut missing = value.clone();
            missing.as_object_mut().unwrap().remove(key);
            assert!(json::from_slice::<WrapperV1>(&json::to_vec(&missing).unwrap()).is_err());
        }
        assert!(json::from_slice::<PreparationV1>(&bytes).is_err());
    }

    #[test]
    fn completed_update_keeps_original_intent_and_actual_installed_state_distinct() {
        let _guard = ChainDiscriminantGuard::enter(super::super::super::CHAIN_DISCRIMINANT);
        let (mut input, receipt) = fixture();
        input.original_service_state = "stopped".into();
        input.successor_service_state = "stopped".into();
        input.before = Some(input.after.clone());
        let output = completed_update_wrapper(input.clone(), receipt.clone()).unwrap();
        assert_eq!(output.original_service_state, "stopped");
        assert_eq!(output.successor_service_state, "stopped");
        assert!(output.before.is_some());
        assert!(output.installed.is_none());
        input.successor_service_state = "running".into();
        assert!(completed_update_wrapper(input, receipt).is_err());
    }

    #[test]
    fn completed_update_rejects_foreign_receipt_and_network() {
        let _guard = ChainDiscriminantGuard::enter(super::super::super::CHAIN_DISCRIMINANT);
        let (input, receipt) = fixture();
        let mut wrong = receipt.clone();
        wrong.path = "/tmp/unverified-receipt.json".into();
        assert!(completed_update_wrapper(input.clone(), wrong).is_err());
        wrong = receipt.clone();
        wrong.sha256 = "0".repeat(64);
        assert!(completed_update_wrapper(input.clone(), wrong).is_err());
        let mut changed = input.clone();
        changed.schema = "taira.epoch-supervisor-update.v1".into();
        assert!(completed_update_wrapper(changed, receipt.clone()).is_err());
        changed = input;
        let mut installed = changed.after.clone();
        installed.network_id = "different-network".into();
        changed.installed = Some(installed);
        assert!(completed_update_wrapper(changed, receipt).is_err());
    }
}
