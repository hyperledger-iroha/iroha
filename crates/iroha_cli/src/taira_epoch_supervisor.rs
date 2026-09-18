//! Finite operator worker that renews native public schedules without replacing dispatches.
//!
//! The deployment owner selects release pins; this worker checks those exact bytes.
//! Installation, release signature admission and OS restart policy remain deployment duties.
use super::*;
use iroha_data_model::isi::GrantBox;
use iroha_model_base::peer::PeerId;
use std::io::{Read, Seek, SeekFrom, Write};
#[cfg(target_os = "linux")]
use std::process::{Child, Command as ProcessCommand, Stdio};

/// Explicit operator inputs for one finite supervised invocation.
#[derive(Debug, clap::Args)]
pub(crate) struct Args {
    /// Independently admitted public release, owner, fee and rotation policy.
    #[arg(long)]
    policy: PathBuf,
    /// Current independently selected public observation trust.
    #[arg(long)]
    trust: PathBuf,
    /// Private mapping from the exact four validator identities to original seed files.
    #[arg(long)]
    custody: PathBuf,
    /// Existing owner-private directory shared with the epoch transaction journals.
    #[arg(long)]
    journal_dir: PathBuf,
    /// Finite worker lifetime; the OS service may restart this same retained worker.
    #[arg(long, default_value_t = 86_400_000, value_parser = clap::value_parser!(u64).range(1..))]
    timeout_ms: u64,
}

/// Read-only verification of one manager-selected worker incarnation and its current target.
#[derive(Debug, clap::Args)]
pub(crate) struct StatusArgs {
    #[arg(long)]
    policy: PathBuf,
    #[arg(long)]
    trust: PathBuf,
    #[arg(long)]
    journal_dir: PathBuf,
    /// Expected kernel boot UUID selected independently by the service manager.
    #[arg(long)]
    boot_id: String,
    /// Expected live manager-owned worker process.
    #[arg(long, value_parser = clap::value_parser!(u32).range(1..))]
    pid: u32,
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..))]
    start_time_ticks: u64,
    #[arg(long, default_value_t = 180_000, value_parser = clap::value_parser!(u64).range(1..))]
    timeout_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct BinaryPin {
    path: PathBuf,
    sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct IntentV1 {
    /// Explicit ongoing owner authority; the sole supported value is `until_stopped`.
    authorization: String,
    network_id: NetworkId,
    administrator: AccountId,
    payment_asset: AssetDefinitionId,
    transaction_fee_maximum: Quantity,
    first_epoch: u64,
    batch_epochs: u16,
    operation_timeout_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct PolicyV1 {
    schema_version: u8,
    intent: IntentV1,
    release_source_commit: String,
    iroha_sha256: String,
    kagami: BinaryPin,
    observation_trust_sha256: String,
    provision_timeout_ms: u64,
}

#[derive(Debug, Clone, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct SeedSourceV1 {
    validator: PeerId,
    path: PathBuf,
}

#[derive(Debug, Clone, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct CustodyV1 {
    schema_version: u8,
    seeds: Vec<SeedSourceV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct WorkerPlanV1 {
    intent: IntentV1,
    original_trust: DeploymentTrustV1,
}

#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct CursorV1 {
    schema_version: u8,
    first_epoch: u64,
    #[norito(required)]
    preceding_completion: Option<CompletionV1>,
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
    schedule_first_epoch: u64,
    schedule_sha256: String,
    completion: CompletionV1,
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

fn batch_end(first: u64, count: u16) -> Result<u64> {
    require(
        first > 0 && (2..=256).contains(&count),
        "epoch worker requires positive epochs and batches of two to256 epochs",
    )?;
    first
        .checked_add(u64::from(count) - 1)
        .ok_or_else(|| eyre!("epoch worker schedule range overflow"))
}

fn validate_policy(policy: &PolicyV1) -> Result<()> {
    require(
        policy.schema_version == 1
            && policy.intent.authorization == "until_stopped"
            && exact_hex(&policy.release_source_commit, 40)
            && exact_hex(&policy.iroha_sha256, 64)
            && exact_hex(&policy.kagami.sha256, 64)
            && exact_hex(&policy.observation_trust_sha256, 64)
            && policy.intent.transaction_fee_maximum > Quantity::zero()
            && policy.intent.operation_timeout_ms > 0
            && policy.provision_timeout_ms > 0,
        "invalid explicit epoch worker release or fee policy",
    )?;
    batch_end(policy.intent.first_epoch, policy.intent.batch_epochs)?;
    Ok(())
}

fn validate_seed_mapping(custody: &CustodyV1, trust: &DeploymentTrustV1) -> Result<()> {
    let mut expected = trust
        .peers
        .iter()
        .map(|peer| peer.peer_id.clone())
        .collect::<Vec<_>>();
    expected.sort();
    require(
        custody.schema_version == 1
            && custody.seeds.len() == 4
            && custody
                .seeds
                .iter()
                .map(|entry| entry.validator.clone())
                .collect::<Vec<_>>()
                == expected
            && custody
                .seeds
                .iter()
                .map(|entry| &entry.path)
                .collect::<BTreeSet<_>>()
                .len()
                == 4,
        "seed custody must name the exact sorted four validators and four distinct original files",
    )
}

fn admitted_generation(
    policy_bytes: &[u8],
    trust_bytes: &[u8],
    custody_bytes: &[u8],
    config: &iroha::config::Config,
    http_operator: &iroha_crypto::KeyPair,
) -> Result<(PolicyV1, DeploymentTrustV1, CustodyV1)> {
    require(
        [policy_bytes, trust_bytes, custody_bytes]
            .iter()
            .all(|bytes| !bytes.is_empty() && bytes.len() <= MAX_BYTES),
        "epoch generation input exceeds its bounded JSON document size",
    )?;
    require(
        config.chain.to_string() == "fc56984b-2be7-431d-840e-21514d1883f0"
            && config.account_chain_discriminant == 369,
        "epoch generation requires the canonical Taira chain and account profile369",
    )?;
    let _profile = iroha_data_model::account::address::ChainDiscriminantGuard::enter(369);
    let policy: PolicyV1 = json::from_slice(policy_bytes)?;
    let trust: DeploymentTrustV1 = json::from_slice(trust_bytes)?;
    let custody: CustodyV1 = json::from_slice(custody_bytes)?;
    validate_policy(&policy)?;
    require(
        config.account == policy.intent.administrator
            && AccountId::new(config.key_pair.public_key().clone()) == policy.intent.administrator
            && config.network_id == policy.intent.network_id,
        "epoch generation configured signer differs from admitted ledger administrator",
    )?;
    require(
        http_operator.public_key() != config.key_pair.public_key(),
        "epoch generation ledger administrator and HTTP operator key must be distinct",
    )?;
    require(
        digest(trust_bytes) == policy.observation_trust_sha256,
        "current observation trust differs from admitted policy",
    )?;
    trust.validate(policy.intent.network_id)?;
    genesis_authorizes(&trust, &policy.intent.administrator)?;
    require(
        trust
            .peers
            .iter()
            .any(|peer| config.torii_api_url.as_str() == peer.torii_origin),
        "epoch generation Torii URL must equal one exact admitted validator origin",
    )?;
    validate_seed_mapping(&custody, &trust)?;
    Ok((policy, trust, custody))
}

/// Pure generation admission shared by deployment and the native supervisor.
pub(super) fn generation_admission(
    policy_bytes: &[u8],
    trust_bytes: &[u8],
    custody_bytes: &[u8],
    config: &iroha::config::Config,
    http_operator: &iroha_crypto::KeyPair,
) -> Result<()> {
    admitted_generation(
        policy_bytes,
        trust_bytes,
        custody_bytes,
        config,
        http_operator,
    )
    .map(|_| ())
}

fn validate_native_schedule(
    schedule: &ScheduleV1,
    policy: &PolicyV1,
    trust: &DeploymentTrustV1,
    first: u64,
) -> Result<()> {
    schedule.validate(trust)?;
    let last = batch_end(first, policy.intent.batch_epochs)?;
    require(
        schedule.network_id == policy.intent.network_id
            && schedule.payment_asset == policy.intent.payment_asset
            && schedule.transaction_fee_maximum == policy.intent.transaction_fee_maximum
            && schedule.parameters.len() == usize::from(policy.intent.batch_epochs)
            && typed_parameter(
                schedule
                    .parameters
                    .first()
                    .ok_or_else(|| eyre!("empty native schedule"))?,
            )?
            .roster
            .epoch
                == first
            && typed_parameter(
                schedule
                    .parameters
                    .last()
                    .ok_or_else(|| eyre!("empty native schedule"))?,
            )?
            .roster
            .epoch
                == last,
        "native provisioner changed the exact public worker schedule",
    )
}

fn genesis_authorizes(trust: &DeploymentTrustV1, owner: &AccountId) -> Result<()> {
    // The caller already authenticated this exact signed wire through TrustV1::validate.
    let block =
        iroha_genesis::decode_signed_genesis(&hex::decode(&trust.genesis_signed_wire_hex)?)?;
    let authorized = block.external_transactions().any(|transaction| {
        let Executable::Instructions(instructions) = transaction.instructions() else { return false; };
        instructions.iter().any(|instruction| {
            matches!(instruction.as_any().downcast_ref::<GrantBox>(), Some(GrantBox::Permission(grant))
                if grant.destination() == owner && grant.object().name() == "CanSetParameters"
                    && grant.object().payload().get() == "null")
        })
    });
    require(
        authorized,
        "epoch worker requires an independently selected genesis-authorized CanSetParameters administrator",
    )
}

#[cfg(unix)]
struct PinnedFile {
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

    #[cfg(any(target_os = "linux", test))]
    fn copy_seed(&mut self, output: &mut impl Write) -> Result<()> {
        use std::os::unix::fs::MetadataExt as _;
        require(
            self.before.len() == 32 && self.before.mode() & 0o777 == 0o600,
            "original mint seed requires exact32-byte owner-private custody",
        )?;
        self.revalidate()?;
        self.file.seek(SeekFrom::Start(0))?;
        let mut scratch = zeroize::Zeroizing::new([0_u8; 33]);
        self.file
            .read_exact(&mut scratch[..32])
            .map_err(|_| eyre!("original mint seed exact read failed"))?;
        require(
            self.file.read(&mut scratch[32..])? == 0,
            "original mint seed length changed",
        )?;
        self.revalidate()?;
        output
            .write_all(&scratch[..32])
            .map_err(|_| eyre!("owned seed pipe transfer failed"))
    }
}

#[cfg(target_os = "linux")]
struct OwnedChild {
    child: Child,
    reaped: bool,
}
#[cfg(target_os = "linux")]
impl Drop for OwnedChild {
    fn drop(&mut self) {
        if !self.reaped {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

#[cfg(target_os = "linux")]
#[allow(
    unsafe_code,
    reason = "only the pinned native executable and owned seed FIFO are inherited by this child"
)]
fn derive_schedule(
    program: &PinnedFile,
    seeds: &mut [PinnedFile],
    policy: &PolicyV1,
    custody: &CustodyV1,
    first: u64,
    deadline: Instant,
) -> Result<ScheduleV1> {
    use std::os::{
        fd::{AsFd as _, AsRawFd as _, BorrowedFd, OwnedFd},
        unix::process::CommandExt as _,
    };
    require_epoch_budget(deadline, "native schedule provisioning")?;
    program.revalidate()?;
    let (reader, writer) = std::io::pipe()?;
    let reader = File::from(OwnedFd::from(reader));
    let mut writer = File::from(OwnedFd::from(writer));
    // Four tiny writes fit in the empty owned pipe before any child exists.
    for seed in seeds {
        seed.copy_seed(&mut writer)?;
    }
    drop(writer);
    let seed_fd = reader.as_raw_fd();
    let program_fd = program.file.as_raw_fd();
    require(
        seed_fd >= 3 && program_fd >= 3 && seed_fd != program_fd,
        "invalid owned child descriptors",
    )?;
    let mut command = ProcessCommand::new(format!("/proc/self/fd/{program_fd}"));
    command
        .args([
            "kagemusha",
            "derive-mint-finality-epoch-schedule-v1",
            "--network-id",
        ])
        .arg(policy.intent.network_id.to_string())
        .arg("--epoch")
        .arg(first.to_string())
        .arg("--epoch-count")
        .arg(policy.intent.batch_epochs.to_string())
        .arg("--payment-asset")
        .arg(policy.intent.payment_asset.to_string())
        .arg("--transaction-fee-maximum")
        .arg(policy.intent.transaction_fee_maximum.to_string())
        .arg("--seed-fd")
        .arg(seed_fd.to_string());
    for source in &custody.seeds {
        command.arg("--validator").arg(source.validator.to_string());
    }
    command
        .env_clear()
        .env("LC_ALL", "C")
        .current_dir("/")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let parent = rustix::process::getpid();
    unsafe {
        command.pre_exec(move || {
            rustix::process::set_parent_process_death_signal(Some(rustix::process::Signal::KILL))
                .map_err(std::io::Error::from)?;
            if rustix::process::getppid() != Some(parent) {
                return Err(std::io::Error::other(
                    "epoch worker exited before provisioner custody",
                ));
            }
            for fd in [seed_fd, program_fd] {
                rustix::io::fcntl_setfd(BorrowedFd::borrow_raw(fd), rustix::io::FdFlags::empty())
                    .map_err(std::io::Error::from)?;
            }
            Ok(())
        });
    }
    let mut owned = OwnedChild {
        child: command
            .spawn()
            .wrap_err("could not spawn pinned native provisioner")?,
        reaped: false,
    };
    drop(reader);
    program.revalidate()?;
    let mut stdout = owned
        .child
        .stdout
        .take()
        .ok_or_else(|| eyre!("native provisioner public output missing"))?;
    let flags = rustix::fs::fcntl_getfl(stdout.as_fd())?;
    rustix::fs::fcntl_setfl(stdout.as_fd(), flags | rustix::fs::OFlags::NONBLOCK)?;
    let mut bytes = Vec::new();
    let mut eof = false;
    loop {
        require_epoch_budget(deadline, "native schedule provisioning")?;
        let mut scratch = [0_u8; 8192];
        loop {
            match stdout.read(&mut scratch) {
                Ok(0) => {
                    eof = true;
                    break;
                }
                Ok(count) => {
                    require(
                        bytes.len().saturating_add(count) <= MAX_BYTES,
                        "native public schedule exceeds bound",
                    )?;
                    bytes.extend_from_slice(&scratch[..count]);
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(_) => return Err(eyre!("native provisioner public output failed")),
            }
        }
        if let Some(status) = owned.child.try_wait()? {
            owned.reaped = true;
            require(status.success(), "native provisioner failed")?;
            if eof {
                program.revalidate()?;
                return json::from_slice(&bytes)
                    .wrap_err("native provisioner returned invalid public schedule");
            }
        }
        std::thread::sleep(
            Duration::from_millis(10).min(deadline.saturating_duration_since(Instant::now())),
        );
    }
}

#[cfg(all(unix, not(target_os = "linux")))]
fn derive_schedule(
    _: &PinnedFile,
    _: &mut [PinnedFile],
    _: &PolicyV1,
    _: &CustodyV1,
    _: u64,
    _: Instant,
) -> Result<ScheduleV1> {
    eyre::bail!("production epoch supervision requires Linux held-descriptor executable custody")
}

#[cfg(unix)]
fn replace_cursor(worker: &Journal, cursor: &CursorV1) -> Result<()> {
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
            worker.read_json::<CursorV1>("cursor.json")? == *cursor,
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

fn remaining_ms(deadline: Instant) -> Result<u64> {
    require_epoch_budget(deadline, "epoch worker invocation")?;
    u64::try_from(
        deadline
            .saturating_duration_since(Instant::now())
            .as_millis(),
    )
    .map(|value| value.max(1))
    .map_err(Into::into)
}

#[cfg(unix)]
pub(super) fn run<C: RunContext>(context: &mut C, args: Args) -> Result<()> {
    require(
        cfg!(target_os = "linux"),
        "production epoch supervision requires Linux held-descriptor executable custody",
    )?;
    require_authority_fee_selection(&context.transaction_fee_payment()?)?;
    require(
        !context.input_instructions()
            && !context.output_instructions()
            && context.transaction_metadata().is_none(),
        "epoch worker cannot combine arbitrary instructions or metadata",
    )?;
    let deadline = operation_deadline(args.timeout_ms)?;
    let mut policy_input = PinnedFile::open(&args.policy, false)?;
    let policy_bytes = policy_input.public_bytes()?;
    let mut trust_input = PinnedFile::open(&args.trust, false)?;
    let trust_bytes = trust_input.public_bytes()?;
    let mut custody_input = PinnedFile::open(&args.custody, true)?;
    let custody_bytes = custody_input.public_bytes()?;
    let (policy, trust, custody) = admitted_generation(
        &policy_bytes,
        &trust_bytes,
        &custody_bytes,
        context.config(),
        context
            .operator_key_pair()
            .ok_or_else(|| eyre!("epoch worker requires a distinct HTTP operator key"))?,
    )?;
    require(
        crate::compiled_build_identity()?.release_source_commit()? == policy.release_source_commit,
        "epoch worker compiled source differs from selected release",
    )?;
    let mut seeds = custody
        .seeds
        .iter()
        .map(|source| PinnedFile::open(&source.path, true))
        .collect::<Result<Vec<_>>>()?;
    let mut kagami = PinnedFile::open(&policy.kagami.path, false)?;
    kagami.binary(&policy.kagami.sha256)?;
    let mut executable = PinnedFile::open(&std::env::current_exe()?.canonicalize()?, false)?;
    executable.binary(&policy.iroha_sha256)?;
    #[cfg(target_os = "linux")]
    require(
        same_file_snapshot(&executable.before, &fs::metadata("/proc/self/exe")?),
        "running worker executable differs from its selected on-disk release",
    )?;
    let worker_path = args
        .journal_dir
        .join(format!("epoch-worker-{}", policy.intent.network_id));
    let create = match fs::symlink_metadata(&worker_path) {
        Ok(_) => false,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => true,
        Err(error) => return Err(error.into()),
    };
    let worker = open_initializing_journal(&worker_path, create)?;
    let retained_plan = match worker.optional_json::<WorkerPlanV1>("plan.json")? {
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
        retained_plan.intent == policy.intent,
        "retained epoch worker intent changed",
    )?;
    validate_observation_trust(
        &retained_plan.original_trust,
        &trust,
        policy.intent.network_id,
    )?;
    if worker.optional_json::<CursorV1>("cursor.json")?.is_none() {
        require_uninitialized_journal(&worker, &["plan.json"])?;
        replace_cursor(
            &worker,
            &CursorV1 {
                schema_version: 1,
                first_epoch: policy.intent.first_epoch,
                preceding_completion: None,
            },
        )?;
    }
    let policy_sha256 = digest(&policy_bytes);
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
        "this worker identity already has readiness evidence",
    )?;
    let mut ready = false;
    let admission_name = format!("release-{policy_sha256}.json");
    if let Some(retained) = worker.optional_json::<PolicyV1>(&admission_name)? {
        require(retained == policy, "retained release policy changed")?;
    } else {
        worker.install_json(&admission_name, &policy)?;
    }
    let current_trust_name = format!("observation-trust-{}.json", policy.observation_trust_sha256);
    if let Some(retained) = worker.read_optional(&current_trust_name)? {
        require(
            retained == trust_bytes,
            "retained current observation trust changed",
        )?;
    } else {
        worker.install(&current_trust_name, &trust_bytes)?;
    }
    loop {
        remaining_ms(deadline)?;
        worker.revalidate()?;
        for input in [
            &policy_input,
            &trust_input,
            &custody_input,
            &executable,
            &kagami,
        ] {
            input.revalidate()?;
        }
        let cursor: CursorV1 = worker.read_json("cursor.json")?;
        require(
            cursor.schema_version == 1
                && cursor.first_epoch >= policy.intent.first_epoch
                && match &cursor.preceding_completion {
                    None => cursor.first_epoch == policy.intent.first_epoch,
                    Some(receipt) => {
                        receipt.schema_version == 1
                            && receipt.network_id == policy.intent.network_id
                            && receipt.target_epoch == cursor.first_epoch
                    }
                },
            "invalid retained worker cursor",
        )?;
        let last = batch_end(cursor.first_epoch, policy.intent.batch_epochs)?;
        let schedule_name = format!("schedule-{}.json", cursor.first_epoch);
        // Re-derive on every invocation, even when this public schedule is retained:
        // pathname/owner checks alone cannot prove original seed content.
        let provision_deadline = operation_deadline(policy.provision_timeout_ms)?.min(deadline);
        let schedule = derive_schedule(
            &kagami,
            &mut seeds,
            &policy,
            &custody,
            cursor.first_epoch,
            provision_deadline,
        )?;
        validate_native_schedule(&schedule, &policy, &trust, cursor.first_epoch)?;
        if let Some(retained) = worker.optional_json::<ScheduleV1>(&schedule_name)? {
            require(
                retained == schedule,
                "native schedule derivation changed retained public custody",
            )?;
        } else {
            worker.install_json(&schedule_name, &schedule)?;
        }
        let common = CommonArgs {
            trust: worker.path.join(&current_trust_name),
            schedule: worker.path.join(&schedule_name),
            journal_dir: args.journal_dir.clone(),
            timeout_ms: remaining_ms(deadline)?,
            operation_timeout_ms: policy.intent.operation_timeout_ms,
        };
        if let Some(expected) = &cursor.preceding_completion {
            // The cursor is a hint. It grants no transaction or epoch authority.
            let mut runtime = Runtime::new(context, &common)?;
            runtime.deadline = runtime.deadline.min(deadline);
            loop {
                let height = runtime.checkpoint_until(runtime.deadline)?;
                match execute(
                    &mut runtime,
                    context,
                    &common,
                    cursor.first_epoch,
                    Action::Status,
                    &height,
                )? {
                    Some(actual) => {
                        require(
                            &actual == expected,
                            "worker cursor differs from freshly authenticated completion",
                        )?;
                        break;
                    }
                    None => runtime.pause(),
                }
            }
        }
        let completion = maintain_until(
            context,
            MaintainArgs {
                common,
                stop_after_epoch: last,
            },
            Some(deadline),
            |completion| {
                if !ready {
                    let current = crate::taira_public_reset::epoch_worker_process_identity()?;
                    require(
                        current
                            == (
                                identity.boot_id.clone(),
                                identity.pid,
                                identity.start_time_ticks,
                            ),
                        "epoch worker process identity changed before readiness",
                    )?;
                    worker.install_json(
                        &ready_name,
                        &ReadyV1 {
                            schema_version: 1,
                            policy_sha256: policy_sha256.clone(),
                            worker: identity.clone(),
                            schedule_first_epoch: cursor.first_epoch,
                            schedule_sha256: digest(&json::to_vec(&schedule)?),
                            completion: completion.clone(),
                        },
                    )?;
                    ready = true;
                }
                Ok(())
            },
        )?;
        require(
            completion.target_epoch == last && completion.network_id == policy.intent.network_id,
            "native maintenance returned a different epoch completion",
        )?;
        // Include E again: the next invocation must reconcile E before observing the actual transition.
        replace_cursor(
            &worker,
            &CursorV1 {
                schema_version: 1,
                first_epoch: last,
                preceding_completion: Some(completion),
            },
        )?;
    }
}

struct ReadOnlyContext<'a, C>(&'a C);
impl<C: RunContext> RunContext for ReadOnlyContext<'_, C> {
    fn config(&self) -> &iroha::config::Config {
        self.0.config()
    }
    fn transaction_metadata(&self) -> Option<&Metadata> {
        self.0.transaction_metadata()
    }
    fn transaction_fee_payment(&self) -> Result<FeePaymentIntent> {
        self.0.transaction_fee_payment()
    }
    fn input_instructions(&self) -> bool {
        self.0.input_instructions()
    }
    fn output_instructions(&self) -> bool {
        self.0.output_instructions()
    }
    fn i18n(&self) -> &iroha_i18n::Localizer {
        self.0.i18n()
    }
    fn operator_key_pair(&self) -> Option<&iroha_crypto::KeyPair> {
        self.0.operator_key_pair()
    }
    fn print_data<T: JsonSerialize + ?Sized>(&mut self, _: &T) -> Result<()> {
        Ok(())
    }
    fn println(&mut self, _: impl std::fmt::Display) -> Result<()> {
        Ok(())
    }
}

#[derive(JsonSerialize)]
struct StatusReportV1 {
    schema_version: u8,
    policy_sha256: String,
    worker: WorkerIdentityV1,
    initial_completion: CompletionV1,
    current_completion: CompletionV1,
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
pub(super) fn status<C: RunContext>(context: &mut C, args: StatusArgs) -> Result<()> {
    let deadline = operation_deadline(args.timeout_ms)?;
    let mut policy_input = PinnedFile::open(&args.policy, false)?;
    let policy_bytes = policy_input.public_bytes()?;
    let policy_sha256 = digest(&policy_bytes);
    let policy: PolicyV1 = json::from_slice(&policy_bytes)?;
    validate_policy(&policy)?;
    require(
        context.config().account == policy.intent.administrator
            && context.config().network_id == policy.intent.network_id,
        "readiness client differs from admitted administrator/network",
    )?;
    let mut trust_input = PinnedFile::open(&args.trust, false)?;
    require(
        digest(&trust_input.public_bytes()?) == policy.observation_trust_sha256,
        "readiness observation trust differs from admitted policy",
    )?;
    let identity = WorkerIdentityV1 {
        boot_id: args.boot_id,
        pid: args.pid,
        start_time_ticks: args.start_time_ticks,
    };
    require_live_worker(&identity)?;
    let worker_path = args
        .journal_dir
        .join(format!("epoch-worker-{}", policy.intent.network_id));
    let mut ready_input = PinnedFile::open(
        &worker_path.join(readiness_name(&policy_sha256, &identity)),
        true,
    )?;
    let ready: ReadyV1 = json::from_slice(&ready_input.public_bytes()?)?;
    require(
        ready.schema_version == 1
            && ready.policy_sha256 == policy_sha256
            && ready.worker == identity,
        "readiness receipt differs from admitted policy/process",
    )?;
    let schedule_path = worker_path.join(format!("schedule-{}.json", ready.schedule_first_epoch));
    let mut schedule_input = PinnedFile::open(&schedule_path, true)?;
    require(
        digest(&schedule_input.public_bytes()?) == ready.schedule_sha256,
        "readiness schedule differs from retained public digest",
    )?;
    let mut common = CommonArgs {
        trust: args.trust,
        schedule: schedule_path,
        journal_dir: args.journal_dir,
        timeout_ms: remaining_ms(deadline)?,
        operation_timeout_ms: policy.intent.operation_timeout_ms,
    };
    let mut quiet = ReadOnlyContext(&*context);
    let mut runtime = Runtime::new(&quiet, &common)?;
    runtime.deadline = runtime.deadline.min(deadline);
    validate_native_schedule(
        &runtime.schedule,
        &policy,
        &runtime.trust,
        ready.schedule_first_epoch,
    )?;
    let mut initial = None;
    loop {
        require_live_worker(&identity)?;
        let height = runtime.checkpoint_until(deadline)?;
        if initial.is_none() {
            if let Some(receipt) = execute(
                &mut runtime,
                &mut quiet,
                &common,
                ready.completion.target_epoch,
                Action::Status,
                &height,
            )? {
                require(
                    receipt == ready.completion,
                    "readiness completion differs from authenticated retained transaction",
                )?;
                initial = Some(receipt);
            } else {
                runtime.pause();
                continue;
            }
        }
        // The mutable cursor is only a hint to an immutable public schedule.
        let mut cursor_input = PinnedFile::open(&worker_path.join("cursor.json"), true)?;
        let cursor: CursorV1 = json::from_slice(&cursor_input.public_bytes()?)?;
        require(
            cursor.schema_version == 1 && cursor.first_epoch >= policy.intent.first_epoch,
            "invalid current worker cursor",
        )?;
        let current_schedule_path =
            worker_path.join(format!("schedule-{}.json", cursor.first_epoch));
        if !current_schedule_path.try_exists()? {
            runtime.pause();
            continue;
        }
        let mut current_schedule_input = PinnedFile::open(&current_schedule_path, true)?;
        let schedule: ScheduleV1 = json::from_slice(&current_schedule_input.public_bytes()?)?;
        validate_native_schedule(&schedule, &policy, &runtime.trust, cursor.first_epoch)?;
        let target = context_at(&height)?
            .epoch
            .checked_add(1)
            .ok_or_else(|| eyre!("epoch overflow"))?;
        if schedule.parameter(target).is_err() {
            runtime.pause();
            continue;
        }
        runtime.schedule = schedule;
        common.schedule = current_schedule_path;
        if !common
            .journal_dir
            .join(operation_name(policy.intent.network_id, target))
            .join("plan.json")
            .try_exists()?
        {
            runtime.pause();
            continue;
        }
        match execute(
            &mut runtime,
            &mut quiet,
            &common,
            target,
            Action::Status,
            &height,
        )? {
            Some(current_completion) => {
                let fresh = runtime.checkpoint_until(deadline)?;
                if !completion_is_current(
                    current_completion.target_epoch,
                    context_at(&fresh)?.epoch,
                ) {
                    runtime.pause();
                    continue;
                }
                for input in [
                    &policy_input,
                    &trust_input,
                    &ready_input,
                    &schedule_input,
                    &current_schedule_input,
                ] {
                    input.revalidate()?;
                }
                require_live_worker(&identity)?;
                return context.print_data(&StatusReportV1 {
                    schema_version: 1,
                    policy_sha256,
                    worker: identity,
                    initial_completion: initial.unwrap(),
                    current_completion,
                });
            }
            None => runtime.pause(),
        }
    }
}

#[cfg(not(unix))]
pub(super) fn status<C: RunContext>(_: &mut C, _: StatusArgs) -> Result<()> {
    eyre::bail!("production epoch supervision status requires Linux")
}

#[cfg(not(unix))]
pub(super) fn run<C: RunContext>(_: &mut C, _: Args) -> Result<()> {
    eyre::bail!("production epoch supervision requires Linux")
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt as _;

    fn admission_fixture() -> (
        PolicyV1,
        DeploymentTrustV1,
        CustodyV1,
        iroha::config::Config,
        iroha_crypto::KeyPair,
    ) {
        let (block, administrator) =
            crate::taira_public_reset::deployment_genesis_administrator_fixture();
        let network = NetworkId::from_genesis_hash(block.hash());
        let mut trust = finality::test_trust();
        trust.genesis_public_key = administrator.public_key().clone();
        trust.genesis_signed_wire_hex = hex::encode(block.encode_wire().unwrap());
        let (mut config, _) = iroha::config::Config::load_bytes_with_musubi_publication(
            "/epoch-admission-fixture.toml",
            include_bytes!("../../../defaults/client.toml"),
        )
        .unwrap();
        config.chain = "fc56984b-2be7-431d-840e-21514d1883f0".into();
        config.network_id = network;
        config.account = AccountId::new(administrator.public_key().clone());
        config.account_chain_discriminant = 369;
        config.key_pair = administrator;
        config.basic_auth = None;
        config.torii_api_url = trust.peers[0].torii_origin.parse().unwrap();
        let policy = PolicyV1 {
            schema_version: 1,
            intent: IntentV1 {
                authorization: "until_stopped".into(),
                network_id: network,
                administrator: config.account.clone(),
                payment_asset: "6TEAJqbb8oEPmLncoNiMRbLEK6tw".parse().unwrap(),
                transaction_fee_maximum: Quantity::from(2_u32),
                first_epoch: 1,
                batch_epochs: 2,
                operation_timeout_ms: 180_000,
            },
            release_source_commit: "a".repeat(40),
            iroha_sha256: "b".repeat(64),
            kagami: BinaryPin {
                path: "/unopened-release/kagami".into(),
                sha256: "c".repeat(64),
            },
            observation_trust_sha256: digest(&json::to_vec(&trust).unwrap()),
            provision_timeout_ms: 30_000,
        };
        let mut peers = trust
            .peers
            .iter()
            .map(|peer| peer.peer_id.clone())
            .collect::<Vec<_>>();
        peers.sort();
        let custody = CustodyV1 {
            schema_version: 1,
            seeds: peers
                .into_iter()
                .enumerate()
                .map(|(index, validator)| SeedSourceV1 {
                    validator,
                    path: format!("/unopened-custody/seed{index}").into(),
                })
                .collect(),
        };
        let operator =
            iroha_crypto::KeyPair::try_from_seed(vec![37; 32], iroha_crypto::Algorithm::Ed25519)
                .unwrap();
        (policy, trust, custody, config, operator)
    }

    fn admit_fixture(
        policy: &PolicyV1,
        trust: &DeploymentTrustV1,
        custody: &CustodyV1,
        config: &iroha::config::Config,
        operator: &iroha_crypto::KeyPair,
    ) -> Result<()> {
        super::super::supervisor_generation_admission(
            &json::to_vec(policy)?,
            &json::to_vec(trust)?,
            &json::to_vec(custody)?,
            config,
            operator,
        )
    }

    #[test]
    fn epoch_supervisor_generation_admission_accepts_exact_public_inputs_without_files() {
        let _profile = iroha_data_model::account::address::ChainDiscriminantGuard::enter(369);
        let (policy, trust, custody, mut config, operator) = admission_fixture();
        for peer in &trust.peers {
            config.torii_api_url = peer.torii_origin.parse().unwrap();
            admit_fixture(&policy, &trust, &custody, &config, &operator).unwrap();
            assert_eq!(config.torii_api_url.as_str(), peer.torii_origin);
        }
    }

    #[test]
    fn epoch_supervisor_generation_admission_rejects_foreign_origin_and_taira_profile() {
        let _profile = iroha_data_model::account::address::ChainDiscriminantGuard::enter(369);
        let (policy, trust, custody, config, operator) = admission_fixture();
        for origin in [
            "http://127.0.0.1:8099/",
            "http://127.0.0.1:8080/unadmitted/",
            "http://127.0.0.1:8080/?redirect=1",
        ] {
            let mut wrong = config.clone();
            wrong.torii_api_url = origin.parse().unwrap();
            assert!(admit_fixture(&policy, &trust, &custody, &wrong, &operator).is_err());
            assert_eq!(wrong.torii_api_url.as_str(), origin);
        }
        let mut wrong = config.clone();
        wrong.chain = "foreign-chain".into();
        assert!(admit_fixture(&policy, &trust, &custody, &wrong, &operator).is_err());
        wrong = config;
        wrong.account_chain_discriminant = 753;
        assert!(admit_fixture(&policy, &trust, &custody, &wrong, &operator).is_err());
    }

    #[test]
    fn epoch_supervisor_generation_admission_rejects_administrator_and_missing_genesis_grant() {
        let _profile = iroha_data_model::account::address::ChainDiscriminantGuard::enter(369);
        let (mut policy, mut trust, custody, config, operator) = admission_fixture();
        let mut wrong = config.clone();
        wrong.account = AccountId::new(operator.public_key().clone());
        assert!(admit_fixture(&policy, &trust, &custody, &wrong, &operator).is_err());
        wrong = config.clone();
        wrong.key_pair = operator.clone();
        assert!(admit_fixture(&policy, &trust, &custody, &wrong, &config.key_pair).is_err());
        // The default shared signed fixture is unchanged and has no explicit grant.
        let (ungranted, key) = crate::taira_public_reset::deployment_genesis_fixture();
        let network = NetworkId::from_genesis_hash(ungranted.hash());
        trust.genesis_signed_wire_hex = hex::encode(ungranted.encode_wire().unwrap());
        trust.genesis_public_key = key.public_key().clone();
        policy.intent.network_id = network;
        policy.intent.administrator = AccountId::new(key.public_key().clone());
        policy.observation_trust_sha256 = digest(&json::to_vec(&trust).unwrap());
        wrong = config;
        wrong.network_id = network;
        wrong.account = policy.intent.administrator.clone();
        wrong.key_pair = key;
        trust.validate(network).unwrap();
        let error = admit_fixture(&policy, &trust, &custody, &wrong, &operator).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("genesis-authorized CanSetParameters")
        );
    }

    #[test]
    fn epoch_supervisor_generation_admission_rejects_shared_operator_key() {
        let _profile = iroha_data_model::account::address::ChainDiscriminantGuard::enter(369);
        let (policy, trust, custody, config, _) = admission_fixture();
        assert!(admit_fixture(&policy, &trust, &custody, &config, &config.key_pair).is_err());
    }

    #[test]
    fn epoch_supervisor_generation_admission_rejects_changed_trust_and_network() {
        let _profile = iroha_data_model::account::address::ChainDiscriminantGuard::enter(369);
        let (policy, trust, custody, config, operator) = admission_fixture();
        let mut wrong = trust.clone();
        wrong.peers[0].config_fingerprint = iroha_crypto::Hash::new(b"unadmitted-current-config");
        assert!(admit_fixture(&policy, &wrong, &custody, &config, &operator).is_err());
        wrong = trust;
        wrong.genesis_public_key = operator.public_key().clone();
        let mut selected = policy.clone();
        selected.observation_trust_sha256 = digest(&json::to_vec(&wrong).unwrap());
        assert!(admit_fixture(&selected, &wrong, &custody, &config, &operator).is_err());
        selected = policy;
        selected.intent.network_id = finality::test_network_id();
        assert!(admit_fixture(&selected, &wrong, &custody, &config, &operator).is_err());
    }

    #[test]
    fn epoch_supervisor_generation_admission_rejects_changed_custody() {
        let _profile = iroha_data_model::account::address::ChainDiscriminantGuard::enter(369);
        let (policy, trust, custody, config, operator) = admission_fixture();
        let mut wrong = custody.clone();
        wrong.seeds.swap(0, 1);
        assert!(admit_fixture(&policy, &trust, &wrong, &config, &operator).is_err());
        wrong = custody.clone();
        wrong.seeds[0].path = wrong.seeds[1].path.clone();
        assert!(admit_fixture(&policy, &trust, &wrong, &config, &operator).is_err());
        wrong = custody;
        wrong.seeds.pop();
        assert!(admit_fixture(&policy, &trust, &wrong, &config, &operator).is_err());
    }

    #[test]
    fn epoch_supervisor_generation_admission_requires_bounded_closed_schemas() {
        let _profile = iroha_data_model::account::address::ChainDiscriminantGuard::enter(369);
        let (policy, trust, custody, config, operator) = admission_fixture();
        let original = [
            json::to_vec(&policy).unwrap(),
            json::to_vec(&trust).unwrap(),
            json::to_vec(&custody).unwrap(),
        ];
        for index in 0..3 {
            let mut values = original.clone();
            let mut value: json::Value = json::from_slice(&values[index]).unwrap();
            value
                .as_object_mut()
                .unwrap()
                .insert("unexpected".into(), json::Value::Bool(true));
            values[index] = json::to_vec(&value).unwrap();
            assert!(
                generation_admission(&values[0], &values[1], &values[2], &config, &operator)
                    .is_err()
            );
            values[index] = vec![b' '; MAX_BYTES + 1];
            assert!(
                generation_admission(&values[0], &values[1], &values[2], &config, &operator)
                    .is_err()
            );
        }
    }

    #[test]
    fn epoch_supervisor_status_parser_has_no_seed_or_mutation_inputs() {
        use clap::Parser as _;
        #[derive(clap::Parser)]
        struct Harness {
            #[command(subcommand)]
            command: super::super::Command,
        }
        let arguments = [
            "test",
            "supervisor-status",
            "--policy",
            "/policy",
            "--trust",
            "/trust",
            "--journal-dir",
            "/journals",
            "--boot-id",
            "01234567-89ab-cdef-0123-456789abcdef",
            "--pid",
            "42",
            "--start-time-ticks",
            "7",
        ];
        assert!(matches!(
            Harness::try_parse_from(arguments).unwrap().command,
            super::super::Command::SupervisorStatus(_)
        ));
        let mut extra = arguments.to_vec();
        extra.extend(["--custody", "/private"]);
        assert!(Harness::try_parse_from(extra).is_err());
        let mut zero = arguments.to_vec();
        zero.extend(["--timeout-ms", "0"]);
        assert!(Harness::try_parse_from(zero).is_err());
    }

    #[test]
    fn epoch_supervisor_policy_schedule_and_custody_reject_wrong_public_authority() {
        let (schedule, trust, key) = super::super::tests::schedule();
        let policy = PolicyV1 {
            schema_version: 1,
            intent: IntentV1 {
                authorization: "until_stopped".into(),
                network_id: schedule.network_id,
                administrator: AccountId::new(key.public_key().clone()),
                payment_asset: schedule.payment_asset.clone(),
                transaction_fee_maximum: schedule.transaction_fee_maximum.clone(),
                first_epoch: 1,
                batch_epochs: 2,
                operation_timeout_ms: 180_000,
            },
            release_source_commit: "a".repeat(40),
            iroha_sha256: "b".repeat(64),
            kagami: BinaryPin {
                path: "/release/kagami".into(),
                sha256: "c".repeat(64),
            },
            observation_trust_sha256: "d".repeat(64),
            provision_timeout_ms: 30_000,
        };
        validate_policy(&policy).unwrap();
        let mut revoked = policy.clone();
        revoked.intent.authorization = "reset_lease".into();
        assert!(validate_policy(&revoked).is_err());
        let mut absent = json::to_value(&policy).unwrap();
        absent
            .get_mut("intent")
            .unwrap()
            .as_object_mut()
            .unwrap()
            .remove("authorization");
        assert!(json::from_value::<PolicyV1>(absent).is_err());
        validate_native_schedule(&schedule, &policy, &trust, 1).unwrap();
        assert!(validate_native_schedule(&schedule, &policy, &trust, 2).is_err());
        let mut wrong = policy.clone();
        wrong.intent.transaction_fee_maximum = Quantity::zero();
        assert!(validate_policy(&wrong).is_err());
        wrong = policy.clone();
        wrong.release_source_commit = "A".repeat(40);
        assert!(validate_policy(&wrong).is_err());
        let mut unknown = json::to_value(&policy).unwrap();
        unknown
            .as_object_mut()
            .unwrap()
            .insert("seed".into(), json::Value::String("forbidden".into()));
        assert!(json::from_value::<PolicyV1>(unknown).is_err());
        let mut peers = trust
            .peers
            .iter()
            .map(|peer| peer.peer_id.clone())
            .collect::<Vec<_>>();
        peers.sort();
        let custody = CustodyV1 {
            schema_version: 1,
            seeds: peers
                .into_iter()
                .enumerate()
                .map(|(index, validator)| SeedSourceV1 {
                    validator,
                    path: PathBuf::from(format!("/custody/seed{index}")),
                })
                .collect(),
        };
        validate_seed_mapping(&custody, &trust).unwrap();
        let mut changed = custody.clone();
        changed.seeds.swap(0, 1);
        assert!(validate_seed_mapping(&changed, &trust).is_err());
        changed = custody;
        changed.seeds[0].path = changed.seeds[1].path.clone();
        assert!(validate_seed_mapping(&changed, &trust).is_err());
        let mut foreign_seed_schedule = schedule;
        foreign_seed_schedule.genesis_roster.validators[0].eq_proof_public_key[0] ^= 1;
        assert!(validate_native_schedule(&foreign_seed_schedule, &policy, &trust, 1).is_err());
        assert!(
            genesis_authorizes(&trust, &policy.intent.administrator).is_err(),
            "an ungranted key cannot become genesis administrator"
        );
    }

    #[test]
    fn epoch_supervisor_readiness_names_bind_policy_and_process_incarnation() {
        let identity = WorkerIdentityV1 {
            boot_id: "01234567-89ab-cdef-0123-456789abcdef".into(),
            pid: 42,
            start_time_ticks: 9,
        };
        let name = readiness_name(&"a".repeat(64), &identity);
        let mut changed = identity.clone();
        changed.start_time_ticks += 1;
        assert_ne!(name, readiness_name(&"a".repeat(64), &changed));
        changed = identity.clone();
        changed.pid += 1;
        assert_ne!(name, readiness_name(&"a".repeat(64), &changed));
        changed = identity.clone();
        changed.boot_id = "11234567-89ab-cdef-0123-456789abcdef".into();
        assert_ne!(name, readiness_name(&"a".repeat(64), &changed));
        assert_ne!(name, readiness_name(&"b".repeat(64), &identity));
    }

    #[test]
    fn epoch_supervisor_rolling_batches_retain_one_epoch_overlap_and_checked_bounds() {
        assert_eq!(batch_end(1, 8).unwrap(), 8);
        assert_eq!(batch_end(8, 8).unwrap(), 15);
        assert_eq!(batch_end(15, 8).unwrap(), 22);
        for (first, count) in [(0, 8), (1, 1), (1, 257), (u64::MAX, 2)] {
            assert!(batch_end(first, count).is_err());
        }
    }

    #[test]
    fn epoch_supervisor_custody_rejects_changed_shared_and_wrong_length_seed_files() {
        let directory = crate::taira_public_reset::private_custody_test_dir("epoch-seed-");
        let path = directory.path().join("seed");
        fs::write(&path, [7_u8; 32]).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        let mut input = PinnedFile::open(&path, true).unwrap();
        let mut output = Vec::new();
        input.copy_seed(&mut output).unwrap();
        assert_eq!(output, vec![7_u8; 32]);
        for mode in [0o720, 0o702, 0o1777] {
            fs::set_permissions(directory.path(), fs::Permissions::from_mode(mode)).unwrap();
            assert!(
                PinnedFile::open(&path, true).is_err(),
                "a private seed cannot admit a writable ancestor, including a sticky directory"
            );
        }
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();
        fs::write(&path, [9_u8; 32]).unwrap();
        assert!(input.copy_seed(&mut Vec::new()).is_err());
        fs::write(&path, [1_u8; 31]).unwrap();
        assert!(
            PinnedFile::open(&path, true)
                .unwrap()
                .copy_seed(&mut Vec::new())
                .is_err()
        );
        fs::hard_link(&path, directory.path().join("shared")).unwrap();
        assert!(PinnedFile::open(&path, true).is_err());
    }

    #[test]
    fn epoch_supervisor_worker_lock_and_cursor_preserve_exclusive_restart_state() {
        let directory = crate::taira_public_reset::private_custody_test_dir("epoch-worker-");
        let path = directory.path().join("worker");
        let worker = Journal::open(&path, true).unwrap();
        assert!(Journal::open(&path, false).is_err());
        let first = CursorV1 {
            schema_version: 1,
            first_epoch: 1,
            preceding_completion: None,
        };
        replace_cursor(&worker, &first).unwrap();
        assert_eq!(worker.read_json::<CursorV1>("cursor.json").unwrap(), first);
        let direct = worker.path.join("cursor.json").canonicalize().unwrap();
        let mut reader = PinnedFile::open(&direct, true).unwrap();
        assert_eq!(
            json::from_slice::<CursorV1>(&reader.public_bytes().unwrap()).unwrap(),
            first,
            "read-only status can inspect pinned public state while the worker holds its exclusive lock"
        );
        let mut later = first;
        later.first_epoch = 8;
        replace_cursor(&worker, &later).unwrap();
        drop(worker);
        let resumed = Journal::open(&path, false).unwrap();
        assert_eq!(resumed.read_json::<CursorV1>("cursor.json").unwrap(), later);
    }
}
