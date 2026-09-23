//! Read-only native worker for automatic epoch retention under one durable restart identity.
//!
//! Release pins, exact public trust and the operating-system process incarnation are checked
//! independently. No validator seeds, native provisioner, transaction signer or ledger write
//! belongs to this worker; real authorized workload supplies the finalized carrier blocks.

use super::*;
use std::io::{Read, Seek, SeekFrom, Write};

#[derive(Debug, clap::Args)]
pub(crate) struct Args {
    #[arg(long)]
    policy: PathBuf,
    #[arg(long)]
    trust: PathBuf,
    #[arg(long)]
    journal_dir: PathBuf,
    #[arg(long, default_value_t = 86_400_000, value_parser = clap::value_parser!(u64).range(1..))]
    timeout_ms: u64,
}

#[derive(Debug, clap::Args)]
pub(crate) struct StatusArgs {
    #[arg(long)]
    policy: PathBuf,
    #[arg(long)]
    trust: PathBuf,
    #[arg(long)]
    journal_dir: PathBuf,
    #[arg(long)]
    boot_id: String,
    #[arg(long, value_parser = clap::value_parser!(u32).range(1..))]
    pid: u32,
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..))]
    start_time_ticks: u64,
    #[arg(long, default_value_t = 180_000, value_parser = clap::value_parser!(u64).range(1..))]
    timeout_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct IntentV1 {
    authorization: String,
    network_id: NetworkId,
    administrator: AccountId,
    first_epoch: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct PolicyV1 {
    schema_version: u8,
    intent: IntentV1,
    release_source_commit: String,
    iroha_sha256: String,
    observation_trust_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct WorkerPlanV1 {
    intent: IntentV1,
    original_trust: DeploymentTrustV1,
}

#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct WorkerIdentityV1 {
    boot_id: String,
    pid: u32,
    start_time_ticks: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct ReadyV1 {
    schema_version: u8,
    policy_sha256: String,
    worker: WorkerIdentityV1,
    completion: RetentionCompletionV1,
}

#[derive(JsonSerialize)]
struct StatusReportV1 {
    schema_version: u8,
    policy_sha256: String,
    worker: WorkerIdentityV1,
    initial_completion: RetentionCompletionV1,
    current_completion: RetentionCompletionV1,
}

fn readiness_name(policy_sha256: &str, identity: &WorkerIdentityV1) -> String {
    format!(
        "ready-{policy_sha256}-{}-{}-{}.json",
        identity.boot_id, identity.pid, identity.start_time_ticks
    )
}
fn exact_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
fn validate_policy(policy: &PolicyV1) -> Result<()> {
    require(
        policy.schema_version == 1
            && policy.intent.authorization == "until_stopped"
            && policy.intent.first_epoch > 0
            && exact_hex(&policy.release_source_commit, 40)
            && exact_hex(&policy.iroha_sha256, 64)
            && exact_hex(&policy.observation_trust_sha256, 64),
        "invalid explicit read-only epoch retention policy",
    )
}
fn admitted_generation(
    policy_bytes: &[u8],
    trust_bytes: &[u8],
    config: &iroha::config::Config,
    http_operator: &iroha_crypto::KeyPair,
) -> Result<(PolicyV1, DeploymentTrustV1)> {
    require(
        [policy_bytes, trust_bytes]
            .iter()
            .all(|bytes| !bytes.is_empty() && bytes.len() <= MAX_BYTES),
        "retention generation input exceeds its bounded public document size",
    )?;
    require(
        config.chain.to_string() == "fc56984b-2be7-431d-840e-21514d1883f0"
            && config.account_chain_discriminant == 369,
        "retention generation requires the canonical Taira chain and account profile369",
    )?;
    let _profile = iroha_data_model::account::address::ChainDiscriminantGuard::enter(369);
    let policy: PolicyV1 = json::from_slice(policy_bytes)?;
    let trust: DeploymentTrustV1 = json::from_slice(trust_bytes)?;
    validate_policy(&policy)?;
    require(
        config.account == policy.intent.administrator
            && AccountId::new(config.key_pair.public_key().clone()) == policy.intent.administrator
            && config.network_id == policy.intent.network_id,
        "retention observer identity differs from its admitted network or administrator",
    )?;
    require(
        http_operator.public_key() != config.key_pair.public_key(),
        "retention HTTP operator and configured account identities must remain distinct",
    )?;
    require(
        digest(trust_bytes) == policy.observation_trust_sha256,
        "retention observation trust differs from admitted policy",
    )?;
    trust.validate(policy.intent.network_id)?;
    require(
        trust
            .peers
            .iter()
            .any(|peer| config.torii_api_url.as_str() == peer.torii_origin),
        "retention Torii URL must equal one exact admitted validator origin",
    )?;
    Ok((policy, trust))
}

pub(super) fn generation_admission(
    policy_bytes: &[u8],
    trust_bytes: &[u8],
    config: &iroha::config::Config,
    http_operator: &iroha_crypto::KeyPair,
) -> Result<()> {
    admitted_generation(policy_bytes, trust_bytes, config, http_operator).map(|_| ())
}

#[cfg(unix)]
pub(super) struct PinnedFile {
    path: PathBuf,
    file: File,
    before: fs::Metadata,
}

#[cfg(unix)]
impl PinnedFile {
    fn open(path: &Path, private: bool) -> Result<Self> {
        use rustix::fs::{Mode, OFlags};
        use std::os::unix::fs::MetadataExt as _;
        require(
            path.is_absolute() && path.canonicalize()? == path,
            "worker custody paths must be absolute, direct and canonical",
        )?;
        for ancestor in path.ancestors().skip(1) {
            let metadata = fs::symlink_metadata(ancestor)?;
            require(
                metadata.is_dir()
                    && !metadata.file_type().is_symlink()
                    && (metadata.uid() == 0
                        || metadata.uid() == rustix::process::geteuid().as_raw())
                    && metadata.mode() & 0o022 == 0,
                "worker input has writable or foreign ancestor custody",
            )?;
        }
        let file = File::from(rustix::fs::open(
            path,
            OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
            Mode::empty(),
        )?);
        let before = file.metadata()?;
        if private {
            private_metadata(&before, false)?;
        } else {
            require(
                before.is_file()
                    && before.nlink() == 1
                    && (before.uid() == 0 || before.uid() == rustix::process::geteuid().as_raw())
                    && before.mode() & 0o022 == 0,
                "worker public input must have direct non-shared regular custody",
            )?;
        }
        let pinned = Self {
            path: path.into(),
            file,
            before,
        };
        pinned.revalidate()?;
        Ok(pinned)
    }

    fn revalidate(&self) -> Result<()> {
        require(
            same_file_snapshot(&self.before, &self.file.metadata()?)
                && same_file_snapshot(&self.before, &fs::symlink_metadata(&self.path)?),
            "worker pinned input changed during owned custody",
        )
    }

    fn public_bytes(&mut self) -> Result<Vec<u8>> {
        require(
            self.before.len() <= MAX_BYTES as u64,
            "worker input exceeds bound",
        )?;
        self.file.seek(SeekFrom::Start(0))?;
        let mut bytes = Vec::new();
        std::io::Read::by_ref(&mut self.file)
            .take((MAX_BYTES + 1) as u64)
            .read_to_end(&mut bytes)?;
        require(bytes.len() <= MAX_BYTES, "worker input exceeds bound")?;
        self.revalidate()?;
        Ok(bytes)
    }

    fn binary(&mut self, expected: &str) -> Result<()> {
        use std::os::unix::fs::MetadataExt as _;
        require(
            self.before.mode() & 0o111 != 0 && self.before.len() <= 4 * 1024 * 1024 * 1024,
            "worker binary lacks executable custody or exceeds bound",
        )?;
        self.file.seek(SeekFrom::Start(0))?;
        let mut digest = Sha256::new();
        let mut scratch = [0_u8; 64 * 1024];
        loop {
            let read = self.file.read(&mut scratch)?;
            if read == 0 {
                break;
            }
            digest.update(&scratch[..read]);
        }
        require(
            hex::encode(digest.finalize()) == expected,
            "worker executable differs from selected release SHA256",
        )?;
        self.revalidate()
    }
}

#[cfg(unix)]
pub(super) fn replace_retention_cursor(worker: &Journal, cursor: &RetentionCursorV1) -> Result<()> {
    use rustix::fs::{AtFlags, Mode, OFlags};
    worker.revalidate()?;
    let name = format!(".staging-{}", hex::encode(rand::random::<[u8; 16]>()));
    let mut file = File::from(rustix::fs::openat(
        &worker.directory,
        name.as_str(),
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::from_raw_mode(0o600),
    )?);
    let result = (|| -> Result<()> {
        file.write_all(&json::to_vec(cursor)?)?;
        file.sync_all()?;
        worker.revalidate()?;
        rustix::fs::renameat(
            &worker.directory,
            name.as_str(),
            &worker.directory,
            "cursor.json",
        )?;
        worker.directory.sync_all()?;
        require(
            worker.read_json::<RetentionCursorV1>("cursor.json")? == *cursor,
            "worker cursor changed during retention",
        )
    })();
    match rustix::fs::unlinkat(&worker.directory, name.as_str(), AtFlags::empty()) {
        Ok(()) => worker.directory.sync_all()?,
        Err(rustix::io::Errno::NOENT) => {}
        Err(error) if result.is_ok() => return Err(error.into()),
        Err(_) => {}
    }
    result
}

#[cfg(unix)]
fn require_live_worker(identity: &WorkerIdentityV1) -> Result<()> {
    let actual = crate::taira_public_reset::epoch_worker_process_identity_for(identity.pid)?;
    require(
        actual
            == (
                identity.boot_id.clone(),
                identity.pid,
                identity.start_time_ticks,
            ),
        "supervisor readiness belongs to a different or stopped worker incarnation",
    )
}

#[cfg(unix)]
pub(super) fn read_pinned_json<T: JsonDeserialize>(path: &Path) -> Result<T> {
    let mut input = PinnedFile::open(path, true)?;
    let value = json::from_slice(&input.public_bytes()?)?;
    input.revalidate()?;
    Ok(value)
}

#[cfg(unix)]
pub(super) fn run<C: RunContext>(context: &mut C, args: Args) -> Result<()> {
    let deadline = operation_deadline(args.timeout_ms)?;
    let mut policy_input = PinnedFile::open(&args.policy, false)?;
    let policy_bytes = policy_input.public_bytes()?;
    let mut trust_input = PinnedFile::open(&args.trust, false)?;
    let trust_bytes = trust_input.public_bytes()?;
    let (policy, trust) = admitted_generation(
        &policy_bytes,
        &trust_bytes,
        context.config(),
        context
            .operator_key_pair()
            .ok_or_else(|| eyre!("retention worker requires its admitted HTTP operator"))?,
    )?;
    require(
        crate::compiled_build_identity()?.release_source_commit()? == policy.release_source_commit,
        "retention worker compiled source differs from selected release",
    )?;
    let mut executable = PinnedFile::open(&std::env::current_exe()?.canonicalize()?, false)?;
    executable.binary(&policy.iroha_sha256)?;
    #[cfg(target_os = "linux")]
    require(
        same_file_snapshot(&executable.before, &fs::metadata("/proc/self/exe")?),
        "running retention executable differs from selected release custody",
    )?;
    let path = args
        .journal_dir
        .join(format!("epoch-worker-{}", policy.intent.network_id));
    let create = match fs::symlink_metadata(&path) {
        Ok(_) => false,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => true,
        Err(error) => return Err(error.into()),
    };
    let worker = open_initializing_journal(&path, create)?;
    let plan = match worker.optional_json::<WorkerPlanV1>("plan.json")? {
        Some(plan) => plan,
        None => {
            require_uninitialized_journal(&worker, &[])?;
            let plan = WorkerPlanV1 {
                intent: policy.intent.clone(),
                original_trust: trust.clone(),
            };
            worker.install_json("plan.json", &plan)?;
            plan
        }
    };
    require(
        plan.intent == policy.intent,
        "retained read-only worker intent changed",
    )?;
    validate_observation_trust(&plan.original_trust, &trust, policy.intent.network_id)?;
    let policy_sha256 = digest(&policy_bytes);
    let policy_name = format!("release-{policy_sha256}.json");
    match worker.optional_json::<PolicyV1>(&policy_name)? {
        Some(retained) => require(retained == policy, "retained worker release changed")?,
        None => worker.install_json(&policy_name, &policy)?,
    }
    let (boot_id, pid, start_time_ticks) =
        crate::taira_public_reset::epoch_worker_process_identity()?;
    let identity = WorkerIdentityV1 {
        boot_id,
        pid,
        start_time_ticks,
    };
    let ready_name = readiness_name(&policy_sha256, &identity);
    require(
        worker.read_optional(&ready_name)?.is_none(),
        "worker incarnation already published readiness",
    )?;
    let root = retention_path(&args.journal_dir, policy.intent.network_id);
    let create = match fs::symlink_metadata(&root) {
        Ok(_) => false,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => true,
        Err(error) => return Err(error.into()),
    };
    let retention = open_initializing_journal(&root, create)?;
    let mut runtime = RetentionRuntime::new(context, trust)?;
    let mut ready = false;
    loop {
        require_epoch_budget(deadline, "observing automatic epoch retention")?;
        worker.revalidate()?;
        for input in [&policy_input, &trust_input, &executable] {
            input.revalidate()?;
        }
        let height = runtime.checkpoint(deadline)?;
        let receipts = retention_receipts(&height, u64::MAX)?;
        retain_receipts(&retention, &receipts, &runtime.trust, runtime.network_id)?;
        if let Some(completion) = receipts
            .last()
            .filter(|receipt| receipt.completed_epoch >= policy.intent.first_epoch)
            && !ready
        {
            require_live_worker(&identity)?;
            worker.install_json(
                &ready_name,
                &ReadyV1 {
                    schema_version: 1,
                    policy_sha256: policy_sha256.clone(),
                    worker: identity.clone(),
                    completion: completion.clone(),
                },
            )?;
            ready = true;
        }
        std::thread::sleep(operation_poll_delay(deadline, Instant::now()));
    }
}

#[cfg(unix)]
pub(super) fn status<C: RunContext>(context: &mut C, args: StatusArgs) -> Result<()> {
    let deadline = operation_deadline(args.timeout_ms)?;
    let mut policy_input = PinnedFile::open(&args.policy, false)?;
    let policy_bytes = policy_input.public_bytes()?;
    let mut trust_input = PinnedFile::open(&args.trust, false)?;
    let trust_bytes = trust_input.public_bytes()?;
    let (policy, trust) = admitted_generation(
        &policy_bytes,
        &trust_bytes,
        context.config(),
        context
            .operator_key_pair()
            .ok_or_else(|| eyre!("retention status requires its admitted HTTP operator"))?,
    )?;
    let policy_sha256 = digest(&policy_bytes);
    let identity = WorkerIdentityV1 {
        boot_id: args.boot_id,
        pid: args.pid,
        start_time_ticks: args.start_time_ticks,
    };
    require_live_worker(&identity)?;
    let root = args
        .journal_dir
        .join(format!("epoch-worker-{}", policy.intent.network_id));
    let plan: WorkerPlanV1 = read_pinned_json(&root.join("plan.json"))?;
    require(
        plan.intent == policy.intent,
        "readiness changed the retained worker intent",
    )?;
    validate_observation_trust(&plan.original_trust, &trust, policy.intent.network_id)?;
    let ready: ReadyV1 = read_pinned_json(&root.join(readiness_name(&policy_sha256, &identity)))?;
    require(
        ready.schema_version == 1
            && ready.policy_sha256 == policy_sha256
            && ready.worker == identity,
        "readiness receipt differs from admitted policy and process",
    )?;
    let mut runtime = RetentionRuntime::new(context, trust)?;
    loop {
        require_live_worker(&identity)?;
        let height = runtime.checkpoint(deadline)?;
        let receipts = retention_receipts(&height, u64::MAX)?;
        require(
            receipts.iter().any(|receipt| receipt == &ready.completion),
            "worker initial readiness differs from freshly authenticated retention",
        )?;
        let Some(current) = receipts.last() else {
            continue;
        };
        let cursor: RetentionCursorV1 = read_pinned_json(
            &retention_path(&args.journal_dir, policy.intent.network_id).join("cursor.json"),
        )?;
        if cursor.completed_epoch < current.completed_epoch {
            std::thread::sleep(operation_poll_delay(deadline, Instant::now()));
            continue;
        }
        require(
            cursor.schema_version == 1
                && cursor.network_id == policy.intent.network_id
                && cursor.completed_epoch == current.completed_epoch
                && cursor.cursor_id == current.cursor_id,
            "worker cursor differs from the current authenticated retained epoch",
        )?;
        read_retained_completion(
            &args.journal_dir,
            policy.intent.network_id,
            current,
            &runtime.trust,
        )?;
        for input in [&policy_input, &trust_input] {
            input.revalidate()?;
        }
        require_live_worker(&identity)?;
        return context.print_data(&StatusReportV1 {
            schema_version: 1,
            policy_sha256,
            worker: identity,
            initial_completion: ready.completion,
            current_completion: current.clone(),
        });
    }
}

#[cfg(not(unix))]
pub(super) fn read_pinned_json<T: JsonDeserialize>(_: &Path) -> Result<T> {
    eyre::bail!("retention evidence custody requires Unix")
}
#[cfg(not(unix))]
pub(super) fn replace_retention_cursor(_: &Journal, _: &RetentionCursorV1) -> Result<()> {
    eyre::bail!("retention cursor custody requires Unix")
}
#[cfg(not(unix))]
pub(super) fn run<C: RunContext>(_: &mut C, _: Args) -> Result<()> {
    eyre::bail!("retention worker requires Unix process custody")
}
#[cfg(not(unix))]
pub(super) fn status<C: RunContext>(_: &mut C, _: StatusArgs) -> Result<()> {
    eyre::bail!("retention readiness requires Unix process custody")
}

#[cfg(all(test, unix))]
#[path = "taira_epoch_supervisor_retention_tests.rs"]
mod tests;
