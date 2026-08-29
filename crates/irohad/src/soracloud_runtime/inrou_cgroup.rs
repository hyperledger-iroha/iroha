//! Fail-closed cgroup-v2 confinement for Linux Inrou PortableVM workers.
//!
//! Every PortableVM worker requires a root-custodied cgroup-v2 hierarchy,
//! projects the guest CPU and memory contract plus bounded QEMU overhead into
//! finite controller values, applies fixed host-safety process and I/O ceilings,
//! and keeps the namespace launcher behind an anonymous acknowledged pipe barrier until the
//! supervisor has placed and attested it. Guest task, descriptor, and ephemeral
//! storage limits are enforced by the generated guest systemd service.

use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::{OsStr, OsString},
    fs,
    io::{self, Read as _, Write as _},
    mem::MaybeUninit,
    os::fd::{AsRawFd as _, FromRawFd as _, OwnedFd, RawFd},
    os::unix::ffi::OsStrExt as _,
    os::unix::fs::{MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _},
    os::unix::process::CommandExt as _,
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};

use eyre::WrapErr as _;
use iroha_crypto::Hash;
use iroha_data_model::soracloud::SoraResourceLimitsV1;

const INROU_CGROUP2_MOUNT: &str = "/sys/fs/cgroup";
const INROU_CGROUP_SUBTREE_NAME: &str = "iroha-inrou-v1";
const INROU_CGROUP_WORKER_PREFIX: &str = "worker-";
const INROU_CGROUP_REQUIRED_CONTROLLERS: [&str; 4] = ["cpu", "io", "memory", "pids"];
const INROU_CGROUP_PROC_MAX_BYTES: u64 = 64 * 1024;
const INROU_CGROUP_CONTROL_MAX_BYTES: u64 = 1024 * 1024;
const INROU_CGROUP_ROOT_MAX_ENTRIES: usize = 1_024;
const INROU_CGROUP_STOP_TIMEOUT: Duration = Duration::from_secs(5);
const INROU_CGROUP_BARRIER_RELEASE_TIMEOUT: Duration = Duration::from_secs(5);
const INROU_CGROUP_CPU_PERIOD_MICROS: u64 = 100_000;
// This bounds only host-side bubblewrap/QEMU processes and threads. The guest
// workload's task contract is independently enforced by `TasksMax=`.
const INROU_CGROUP_QEMU_PIDS_MAX: u64 = 64;
// Guest ephemeral storage is a capacity contract, not a throughput signal.
// Use explicit host-safety ceilings for QEMU block-device traffic instead.
const INROU_CGROUP_QEMU_IO_BYTES_PER_SEC_MAX: u64 = 64 * 1024 * 1024;
const INROU_CGROUP_QEMU_IOPS_MAX: u64 = 1_024;
const INROU_CGROUP_BARRIER_TOKEN: &[u8] = b"inrou-cgroup-go-v1\n";
const INROU_CGROUP_BARRIER_ACK: &[u8] = b"inrou-cgroup-ready-v1\n";
/// Exact hidden argument selecting the child-only Inrou namespace launcher.
pub(super) const INROU_INTERNAL_LAUNCHER_ARG_V1: &str = "--iroha-internal-inrou-launcher-v1";
pub(super) const INROU_INTERNAL_LAUNCHER_MAX_ARGUMENTS: usize = 512;
const INROU_INTERNAL_LAUNCHER_MAX_ARGUMENT_BYTES: usize = 16 * 1024;
const INROU_INTERNAL_LAUNCHER_MAX_BINDINGS: usize = 64;
const INROU_INTERNAL_BWRAP_PATH: &str = "/usr/bin/bwrap";

#[derive(Clone, Copy, Debug)]
pub(super) struct InrouCgroupWorkerKey<'a> {
    pub service_name: &'a str,
    pub service_version: &'a str,
    pub replica_slot: u16,
    pub process_generation: u64,
    pub bundle_hash: &'a str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct InrouCgroupLimits {
    memory_max_bytes: u64,
    pids_max: u64,
    cpu_quota_micros: u64,
    cpu_period_micros: u64,
    io_read_bytes_per_sec: u64,
    io_write_bytes_per_sec: u64,
    io_read_iops: u64,
    io_write_iops: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct InrouCgroupIoDevice {
    major: u32,
    minor: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct InrouCgroupIoLimits {
    read_bytes_per_sec: u64,
    write_bytes_per_sec: u64,
    read_iops: u64,
    write_iops: u64,
}

impl From<InrouCgroupLimits> for InrouCgroupIoLimits {
    fn from(limits: InrouCgroupLimits) -> Self {
        Self {
            read_bytes_per_sec: limits.io_read_bytes_per_sec,
            write_bytes_per_sec: limits.io_write_bytes_per_sec,
            read_iops: limits.io_read_iops,
            write_iops: limits.io_write_iops,
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct InrouCgroupAttestation {
    worker_path: PathBuf,
    expected_proc_path: String,
}

pub(super) struct InrouWorkerCgroup {
    attestation: InrouCgroupAttestation,
    launcher_placement_attempted: bool,
    active: bool,
}

/// Proof that one exact worker cgroup was empty after a bounded kill-and-wait.
///
/// The proof deliberately does not remove the cgroup. The supervisor may
/// consume it only after it has independently reaped the direct child.
pub(super) struct InrouEmptyCgroupAttestation {
    worker_path: PathBuf,
    device: u64,
    inode: u64,
}

pub(super) struct InrouLaunchBarrier {
    child_gate_reader: Option<fs::File>,
    parent_gate_writer: Option<fs::File>,
    parent_ack_reader: Option<fs::File>,
    child_ack_writer: Option<fs::File>,
}

#[derive(Debug, PartialEq, Eq)]
struct InrouInternalLauncherV1 {
    gate_fd: RawFd,
    acknowledgement_fd: RawFd,
    expected_cgroup_path: String,
    bindings: Vec<InrouInternalLauncherBindingV1>,
    bubblewrap_arguments: Vec<OsString>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct InrouInternalLauncherBindingV1 {
    descriptor: RawFd,
    destination: PathBuf,
    writable: bool,
}

/// Run the post-exec Inrou cgroup gate and replace this helper with bubblewrap.
///
/// This process is a fresh `/proc/self/exe` invocation. Ordinary daemon
/// initialization has not run, and the only non-CLOEXEC inputs are the exact
/// gate, acknowledgement, and binding descriptors admitted by the parent.
#[allow(unsafe_code)]
pub(super) fn run_inrou_internal_launcher_v1(arguments: Vec<OsString>) -> eyre::Result<()> {
    let request = parse_inrou_internal_launcher_v1(arguments)?;
    let required_descriptors = std::iter::once(request.gate_fd)
        .chain(std::iter::once(request.acknowledgement_fd))
        .chain(request.bindings.iter().map(|binding| binding.descriptor))
        .collect::<BTreeSet<_>>();
    let open_descriptors = list_open_inrou_launcher_descriptors()?;
    if !required_descriptors.is_subset(&open_descriptors) {
        eyre::bail!("Inrou internal launcher request names a descriptor that is not open");
    }
    let mut gate = unsafe {
        // SAFETY: parsing proved uniqueness and the immediately preceding
        // single-threaded /proc enumeration proved this descriptor open above
        // stdio. It is now owned by this post-exec helper process.
        fs::File::from_raw_fd(request.gate_fd)
    };
    let mut acknowledgement = unsafe {
        // SAFETY: as above; the acknowledgement descriptor is open and
        // distinct from the gate and every retained binding descriptor.
        fs::File::from_raw_fd(request.acknowledgement_fd)
    };
    let mut token = Vec::with_capacity(INROU_CGROUP_BARRIER_TOKEN.len() + 1);
    std::io::Read::by_ref(&mut gate)
        .take(u64::try_from(INROU_CGROUP_BARRIER_TOKEN.len() + 1).expect("small token bound"))
        .read_to_end(&mut token)
        .wrap_err("read the anonymous Inrou cgroup gate")?;
    if token != INROU_CGROUP_BARRIER_TOKEN {
        eyre::bail!("Inrou cgroup gate returned an invalid or non-exact token");
    }
    drop(gate);

    let proc_cgroup = read_bounded_text(
        Path::new("/proc/self/cgroup"),
        INROU_CGROUP_PROC_MAX_BYTES,
        "Inrou child cgroup",
    )?;
    validate_inrou_proc_cgroup(&proc_cgroup, &request.expected_cgroup_path)?;
    acknowledgement
        .write_all(INROU_CGROUP_BARRIER_ACK)
        .wrap_err("acknowledge exact Inrou child cgroup placement")?;
    drop(acknowledgement);

    close_unrelated_inrou_launcher_descriptors(
        &request
            .bindings
            .iter()
            .map(|binding| binding.descriptor)
            .collect::<Vec<_>>(),
    )?;
    validate_internal_bubblewrap_executable()?;
    let mut command = Command::new(INROU_INTERNAL_BWRAP_PATH);
    command
        .args(&request.bubblewrap_arguments)
        .env_clear()
        .current_dir("/");
    let error = command.exec();
    Err(error).wrap_err("exec the fixed Inrou bubblewrap launcher")
}

fn parse_inrou_internal_launcher_v1(
    arguments: Vec<OsString>,
) -> eyre::Result<InrouInternalLauncherV1> {
    if arguments.len() < 5 || arguments.len() > INROU_INTERNAL_LAUNCHER_MAX_ARGUMENTS {
        eyre::bail!("Inrou internal launcher argument count is outside the V1 bound");
    }
    if arguments.iter().any(|argument| {
        argument.as_bytes().is_empty()
            || argument.as_bytes().len() > INROU_INTERNAL_LAUNCHER_MAX_ARGUMENT_BYTES
    }) {
        eyre::bail!("Inrou internal launcher argument length is outside the V1 bound");
    }
    let mut arguments = arguments.into_iter();
    let gate_fd = parse_inrou_launcher_fd(
        arguments.next().expect("minimum argument count"),
        "gate descriptor",
    )?;
    let acknowledgement_fd = parse_inrou_launcher_fd(
        arguments.next().expect("minimum argument count"),
        "acknowledgement descriptor",
    )?;
    let expected_cgroup_path = arguments
        .next()
        .expect("minimum argument count")
        .into_string()
        .map_err(|_| eyre::eyre!("Inrou expected cgroup path is not UTF-8"))?;
    validate_inrou_expected_cgroup_path(&expected_cgroup_path)?;
    let binding_count = parse_inrou_launcher_count(
        arguments.next().expect("minimum argument count"),
        "binding count",
    )?;
    if binding_count > INROU_INTERNAL_LAUNCHER_MAX_BINDINGS {
        eyre::bail!("Inrou internal launcher binding count exceeds the V1 bound");
    }
    let mut bindings = Vec::with_capacity(binding_count);
    for _ in 0..binding_count {
        let writable = match arguments
            .next()
            .ok_or_else(|| eyre::eyre!("Inrou internal launcher binding map is truncated"))?
            .as_os_str()
        {
            value if value == OsStr::new("--bind-fd") => true,
            value if value == OsStr::new("--ro-bind-fd") => false,
            _ => eyre::bail!("Inrou internal launcher binding mode is not canonical"),
        };
        let descriptor = parse_inrou_launcher_fd(
            arguments
                .next()
                .ok_or_else(|| eyre::eyre!("Inrou internal launcher binding list is truncated"))?,
            "binding descriptor",
        )?;
        let destination = PathBuf::from(arguments.next().ok_or_else(|| {
            eyre::eyre!("Inrou internal launcher binding destination is truncated")
        })?);
        validate_inrou_launcher_binding_destination(&destination)?;
        bindings.push(InrouInternalLauncherBindingV1 {
            descriptor,
            destination,
            writable,
        });
    }
    let program = arguments
        .next()
        .ok_or_else(|| eyre::eyre!("Inrou internal launcher omitted bubblewrap"))?;
    if program != OsStr::new(INROU_INTERNAL_BWRAP_PATH) {
        eyre::bail!("Inrou internal launcher program is not the fixed bubblewrap path");
    }
    let bubblewrap_arguments = arguments.collect::<Vec<_>>();
    if bubblewrap_arguments.is_empty() {
        eyre::bail!("Inrou internal launcher omitted bubblewrap arguments");
    }
    let descriptors = std::iter::once(gate_fd)
        .chain(std::iter::once(acknowledgement_fd))
        .chain(bindings.iter().map(|binding| binding.descriptor))
        .collect::<BTreeSet<_>>();
    if descriptors.len() != bindings.len() + 2 {
        eyre::bail!("Inrou internal launcher descriptors are not unique");
    }
    if bindings
        .iter()
        .map(|binding| &binding.destination)
        .collect::<BTreeSet<_>>()
        .len()
        != bindings.len()
    {
        eyre::bail!("Inrou internal launcher binding destinations are not unique");
    }
    validate_bubblewrap_binding_map(&bubblewrap_arguments, &bindings)?;
    Ok(InrouInternalLauncherV1 {
        gate_fd,
        acknowledgement_fd,
        expected_cgroup_path,
        bindings,
        bubblewrap_arguments,
    })
}

fn parse_inrou_launcher_fd(value: OsString, label: &str) -> eyre::Result<RawFd> {
    let value = value
        .into_string()
        .map_err(|_| eyre::eyre!("Inrou {label} is not UTF-8"))?;
    let parsed = value
        .parse::<RawFd>()
        .wrap_err_with(|| format!("parse Inrou {label}"))?;
    if parsed <= 2 || parsed.to_string() != value {
        eyre::bail!("Inrou {label} is not one canonical descriptor above stdio");
    }
    Ok(parsed)
}

fn parse_inrou_launcher_count(value: OsString, label: &str) -> eyre::Result<usize> {
    let value = value
        .into_string()
        .map_err(|_| eyre::eyre!("Inrou {label} is not UTF-8"))?;
    let parsed = value
        .parse::<usize>()
        .wrap_err_with(|| format!("parse Inrou {label}"))?;
    if parsed.to_string() != value {
        eyre::bail!("Inrou {label} is not canonical decimal");
    }
    Ok(parsed)
}

fn validate_inrou_expected_cgroup_path(value: &str) -> eyre::Result<()> {
    let path = Path::new(value);
    let worker_prefix = format!("/{INROU_CGROUP_SUBTREE_NAME}/{INROU_CGROUP_WORKER_PREFIX}");
    let worker_digest = value.strip_prefix(&worker_prefix);
    if value.len() > 4_096
        || !value.starts_with('/')
        || value.ends_with('/')
        || value.contains("//")
        || value.chars().any(char::is_whitespace)
        || path.components().any(|component| {
            matches!(
                component,
                std::path::Component::Prefix(_)
                    | std::path::Component::CurDir
                    | std::path::Component::ParentDir
            )
        })
        || !worker_digest.is_some_and(|digest| {
            digest.len() == Hash::LENGTH * 2
                && digest
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        })
    {
        eyre::bail!("Inrou expected cgroup path is not canonical absolute V1 syntax");
    }
    Ok(())
}

fn validate_inrou_launcher_binding_destination(destination: &Path) -> eyre::Result<()> {
    let Some(value) = destination.to_str() else {
        eyre::bail!("Inrou internal launcher binding destination is not UTF-8");
    };
    if !destination.is_absolute()
        || destination == Path::new("/")
        || !value.starts_with("/inrou/")
        || value.ends_with('/')
        || value.contains("//")
        || value.len() > 4_096
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'.' | b'_' | b'-'))
        || destination.components().any(|component| {
            matches!(
                component,
                std::path::Component::Prefix(_)
                    | std::path::Component::CurDir
                    | std::path::Component::ParentDir
            )
        })
    {
        eyre::bail!("Inrou internal launcher binding destination is not canonical and absolute");
    }
    Ok(())
}

fn validate_bubblewrap_binding_map(
    arguments: &[OsString],
    expected: &[InrouInternalLauncherBindingV1],
) -> eyre::Result<()> {
    let separator = arguments
        .iter()
        .position(|argument| argument == "--")
        .ok_or_else(|| eyre::eyre!("Inrou bubblewrap arguments omit the command separator"))?;
    let namespace_arguments = &arguments[..separator];
    for required in [
        "--die-with-parent",
        "--new-session",
        "--as-pid-1",
        "--unshare-pid",
        "--unshare-net",
        "--unshare-ipc",
        "--unshare-uts",
        "--unshare-cgroup",
        "--clearenv",
    ] {
        if namespace_arguments
            .iter()
            .filter(|argument| argument.as_os_str() == OsStr::new(required))
            .count()
            != 1
        {
            eyre::bail!("Inrou bubblewrap arguments omit or duplicate `{required}`");
        }
    }
    let mut actual = Vec::new();
    let mut index = 0;
    while index < namespace_arguments.len() {
        let argument = &namespace_arguments[index];
        if argument == "--bind-fd" || argument == "--ro-bind-fd" {
            let descriptor = namespace_arguments
                .get(index + 1)
                .ok_or_else(|| eyre::eyre!("Inrou bubblewrap bind descriptor is truncated"))?
                .clone();
            let destination = namespace_arguments
                .get(index + 2)
                .ok_or_else(|| eyre::eyre!("Inrou bubblewrap bind destination is truncated"))?;
            let destination = PathBuf::from(destination.as_os_str());
            validate_inrou_launcher_binding_destination(&destination)?;
            actual.push(InrouInternalLauncherBindingV1 {
                descriptor: parse_inrou_launcher_fd(descriptor, "bubblewrap binding descriptor")?,
                destination,
                writable: argument == "--bind-fd",
            });
            index += 3;
        } else {
            index += 1;
        }
    }
    if actual != expected {
        eyre::bail!("Inrou bubblewrap arguments differ from the typed retained binding map");
    }
    Ok(())
}

#[allow(unsafe_code)]
fn close_unrelated_inrou_launcher_descriptors(retained: &[RawFd]) -> eyre::Result<()> {
    let open = list_open_inrou_launcher_descriptors()?;
    if retained.iter().any(|descriptor| !open.contains(descriptor)) {
        eyre::bail!("Inrou launcher retained binding descriptor is not open");
    }
    let retained = retained.iter().copied().collect::<BTreeSet<_>>();
    for descriptor in open {
        if descriptor <= 2 || retained.contains(&descriptor) {
            continue;
        }
        drop(unsafe {
            // SAFETY: this fresh, single-threaded helper owns every inherited
            // descriptor. The /proc enumeration handle is already closed,
            // stdio and the exact binding set are excluded, and entries are
            // unique.
            OwnedFd::from_raw_fd(descriptor)
        });
    }
    Ok(())
}

fn list_open_inrou_launcher_descriptors() -> eyre::Result<BTreeSet<RawFd>> {
    let directory = rustix::fs::open(
        "/proc/self/fd",
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::DIRECTORY
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )?;
    let directory_fd = directory.as_raw_fd();
    let mut buffer = [MaybeUninit::<u8>::uninit(); 4_096];
    let mut directory_entries = rustix::fs::RawDir::new(&directory, &mut buffer);
    let mut entries = Vec::new();
    while let Some(entry) = directory_entries.next() {
        let entry = entry?;
        let name = entry.file_name().to_bytes();
        if name != b"." && name != b".." {
            entries.push(name.to_vec());
        }
    }
    drop(directory_entries);
    let mut open = BTreeSet::new();
    for name in entries {
        let name = std::str::from_utf8(&name)
            .wrap_err("Inrou launcher observed a non-UTF8 descriptor name")?;
        let descriptor = name
            .parse::<RawFd>()
            .wrap_err("parse Inrou launcher /proc/self/fd entry")?;
        if descriptor.to_string() != name || !open.insert(descriptor) {
            eyre::bail!("Inrou launcher observed a non-canonical descriptor entry");
        }
    }
    if !open.remove(&directory_fd) {
        eyre::bail!("Inrou launcher descriptor enumeration omitted its own directory handle");
    }
    drop(directory);
    Ok(open)
}

fn validate_internal_bubblewrap_executable() -> eyre::Result<()> {
    let path = Path::new(INROU_INTERNAL_BWRAP_PATH);
    let metadata = fs::symlink_metadata(path)
        .wrap_err("inspect the fixed internal Inrou bubblewrap launcher")?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.uid() != 0
        || metadata.mode() & 0o111 == 0
        || metadata.mode() & 0o022 != 0
        || !path.ancestors().skip(1).all(|ancestor| {
            fs::symlink_metadata(ancestor).ok().is_some_and(|metadata| {
                !metadata.file_type().is_symlink()
                    && metadata.is_dir()
                    && metadata.uid() == 0
                    && metadata.mode() & 0o022 == 0
            })
        })
    {
        eyre::bail!("fixed internal Inrou bubblewrap launcher custody drifted");
    }
    Ok(())
}

impl InrouCgroupAttestation {
    pub(super) fn expected_proc_path(&self) -> &str {
        &self.expected_proc_path
    }

    pub(super) fn attest_pid(&self, pid: u32) -> eyre::Result<()> {
        let cgroup = read_bounded_text(
            &PathBuf::from(format!("/proc/{pid}/cgroup")),
            INROU_CGROUP_PROC_MAX_BYTES,
            "Inrou process cgroup",
        )?;
        validate_inrou_proc_cgroup(&cgroup, &self.expected_proc_path)?;
        let members = self.member_pids()?;
        if members.binary_search(&pid).is_err() {
            eyre::bail!(
                "Inrou worker cgroup {} does not contain attested pid {pid}; members are {:?}",
                self.worker_path.display(),
                members,
            );
        }
        Ok(())
    }

    pub(super) fn member_pids(&self) -> eyre::Result<Vec<u32>> {
        read_cgroup_pids(&self.worker_path.join("cgroup.procs"))
    }

    fn attest_isolated_launcher(&self, pid: u32) -> eyre::Result<()> {
        self.attest_pid(pid)?;
        let members = self.member_pids()?;
        if members != [pid] {
            eyre::bail!(
                "Inrou worker cgroup {} contains pids {:?} instead of only the gated launcher {pid}",
                self.worker_path.display(),
                members,
            );
        }
        Ok(())
    }
}

impl InrouWorkerCgroup {
    pub(super) fn prepare(
        key: InrouCgroupWorkerKey<'_>,
        resources: &SoraResourceLimitsV1,
        io_backing_paths: &[&Path],
    ) -> eyre::Result<Self> {
        let subtree = prepare_inrou_cgroup_root()?;
        let worker_name = inrou_cgroup_worker_name(key);
        validate_inrou_cgroup_worker_name(&worker_name)?;
        let worker_path = subtree.join(&worker_name);
        fs::create_dir(&worker_path).wrap_err_with(|| {
            format!(
                "create unique Inrou worker cgroup {}; a pre-existing worker cgroup is a fail-closed stale-runtime condition",
                worker_path.display()
            )
        })?;
        let expected_proc_path = format!("/{INROU_CGROUP_SUBTREE_NAME}/{worker_name}");
        let mut worker = Self {
            attestation: InrouCgroupAttestation {
                worker_path,
                expected_proc_path,
            },
            launcher_placement_attempted: false,
            active: true,
        };
        let initialize = (|| -> eyre::Result<()> {
            fs::set_permissions(
                &worker.attestation.worker_path,
                fs::Permissions::from_mode(0o700),
            )
            .wrap_err_with(|| {
                format!(
                    "set exact permissions on {}",
                    worker.attestation.worker_path.display()
                )
            })?;
            validate_root_custodied_directory(
                &worker.attestation.worker_path,
                "Inrou worker cgroup",
            )
        })();
        if let Err(error) = initialize {
            let cleanup = worker.cleanup_unlaunched_bounded();
            return Err(error).wrap_err_with(|| {
                format!(
                    "initialize root-custodied Inrou worker cgroup{}",
                    cleanup.err().map_or_else(String::new, |cleanup| format!(
                        "; empty-cgroup cleanup also failed: {cleanup}"
                    ))
                )
            });
        }
        if let Err(error) = worker.configure(resources, io_backing_paths) {
            let cleanup = worker.cleanup_unlaunched_bounded();
            return Err(error).wrap_err_with(|| {
                format!(
                    "configure finite Inrou cgroup limits{}",
                    cleanup.err().map_or_else(String::new, |cleanup| format!(
                        "; empty-cgroup cleanup also failed: {cleanup}"
                    ))
                )
            });
        }
        Ok(worker)
    }

    pub(super) fn attestation(&self) -> &InrouCgroupAttestation {
        &self.attestation
    }

    pub(super) fn place_launcher(&mut self, pid: u32) -> eyre::Result<()> {
        // A failed control-file write cannot safely prove that the kernel did
        // not consume the pid, so every placement attempt requires the full
        // direct-child teardown protocol from this point onward.
        self.launcher_placement_attempted = true;
        write_control(
            &self.attestation.worker_path.join("cgroup.procs"),
            &pid.to_string(),
            "place Inrou launcher in its cgroup",
        )?;
        self.attestation
            .attest_isolated_launcher(pid)
            .wrap_err("attest Inrou launcher cgroup before releasing its exec barrier")
    }

    pub(super) fn kill_and_attest_empty_bounded(
        &self,
    ) -> eyre::Result<InrouEmptyCgroupAttestation> {
        if !self.active {
            eyre::bail!("cannot attest an already released Inrou worker cgroup");
        }
        let kill_path = self.attestation.worker_path.join("cgroup.kill");
        write_control(&kill_path, "1", "kill the exact Inrou worker cgroup")?;
        let deadline = std::time::Instant::now() + INROU_CGROUP_STOP_TIMEOUT;
        loop {
            let events = read_bounded_text(
                &self.attestation.worker_path.join("cgroup.events"),
                INROU_CGROUP_CONTROL_MAX_BYTES,
                "Inrou cgroup events",
            )?;
            let populated = parse_inrou_cgroup_populated(&events)?;
            let pids = read_cgroup_pids(&self.attestation.worker_path.join("cgroup.procs"))?;
            if !populated && pids.is_empty() {
                break;
            }
            if std::time::Instant::now() >= deadline {
                eyre::bail!(
                    "Inrou worker cgroup {} remained populated by pids {:?} after {:?}",
                    self.attestation.worker_path.display(),
                    pids,
                    INROU_CGROUP_STOP_TIMEOUT,
                );
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let metadata = fs::symlink_metadata(&self.attestation.worker_path).wrap_err_with(|| {
            format!(
                "reinspect empty Inrou worker cgroup {}",
                self.attestation.worker_path.display()
            )
        })?;
        validate_root_custodied_directory(
            &self.attestation.worker_path,
            "empty Inrou worker cgroup",
        )?;
        Ok(InrouEmptyCgroupAttestation {
            worker_path: self.attestation.worker_path.clone(),
            device: metadata.dev(),
            inode: metadata.ino(),
        })
    }

    pub(super) fn release_attested_empty(
        &mut self,
        empty: InrouEmptyCgroupAttestation,
    ) -> eyre::Result<()> {
        if !self.active {
            eyre::bail!("cannot release an already released Inrou worker cgroup");
        }
        if empty.worker_path != self.attestation.worker_path {
            eyre::bail!("empty-cgroup proof belongs to another Inrou worker");
        }
        let metadata = fs::symlink_metadata(&self.attestation.worker_path).wrap_err_with(|| {
            format!(
                "reinspect attested empty Inrou worker cgroup {}",
                self.attestation.worker_path.display()
            )
        })?;
        validate_root_custodied_directory(
            &self.attestation.worker_path,
            "attested empty Inrou worker cgroup",
        )?;
        if metadata.dev() != empty.device || metadata.ino() != empty.inode {
            eyre::bail!("attested empty Inrou worker cgroup changed identity before release");
        }
        let events = read_bounded_text(
            &self.attestation.worker_path.join("cgroup.events"),
            INROU_CGROUP_CONTROL_MAX_BYTES,
            "attested empty Inrou cgroup events",
        )?;
        let populated = parse_inrou_cgroup_populated(&events)?;
        let pids = read_cgroup_pids(&self.attestation.worker_path.join("cgroup.procs"))?;
        if populated || !pids.is_empty() {
            eyre::bail!(
                "attested empty Inrou worker cgroup {} became populated by pids {:?} before release",
                self.attestation.worker_path.display(),
                pids,
            );
        }
        fs::remove_dir(&self.attestation.worker_path).wrap_err_with(|| {
            format!(
                "remove empty Inrou worker cgroup {}",
                self.attestation.worker_path.display()
            )
        })?;
        self.active = false;
        Ok(())
    }

    fn cleanup_unlaunched_bounded(&mut self) -> eyre::Result<()> {
        if self.launcher_placement_attempted {
            eyre::bail!(
                "Inrou worker cgroup cannot use unlaunched cleanup after launcher placement was attempted"
            );
        }
        let empty = self.kill_and_attest_empty_bounded()?;
        self.release_attested_empty(empty)
    }

    fn configure(
        &self,
        resources: &SoraResourceLimitsV1,
        io_backing_paths: &[&Path],
    ) -> eyre::Result<()> {
        require_inrou_cgroup_kill_control(&self.attestation.worker_path.join("cgroup.kill"))?;
        if !read_cgroup_pids(&self.attestation.worker_path.join("cgroup.procs"))?.is_empty() {
            eyre::bail!("new Inrou worker cgroup was unexpectedly populated before configuration");
        }
        let events = read_bounded_text(
            &self.attestation.worker_path.join("cgroup.events"),
            INROU_CGROUP_CONTROL_MAX_BYTES,
            "new Inrou cgroup events",
        )?;
        if parse_inrou_cgroup_populated(&events)? {
            eyre::bail!("new Inrou worker cgroup reported populated before configuration");
        }

        let limits = project_inrou_cgroup_limits(resources)?;
        write_control_and_require_exact(
            &self.attestation.worker_path.join("memory.max"),
            &limits.memory_max_bytes.to_string(),
            "Inrou memory.max",
        )?;
        write_control_and_require_exact(
            &self.attestation.worker_path.join("memory.swap.max"),
            "0",
            "Inrou memory.swap.max",
        )?;
        write_control_and_require_exact(
            &self.attestation.worker_path.join("memory.oom.group"),
            "1",
            "Inrou memory.oom.group",
        )?;
        write_control_and_require_exact(
            &self.attestation.worker_path.join("pids.max"),
            &limits.pids_max.to_string(),
            "Inrou pids.max",
        )?;
        write_control_and_require_exact(
            &self.attestation.worker_path.join("cpu.max"),
            &format!("{} {}", limits.cpu_quota_micros, limits.cpu_period_micros),
            "Inrou cpu.max",
        )?;

        let devices = resolve_inrou_cgroup_io_devices(io_backing_paths)?;
        let expected_io = devices
            .into_iter()
            .map(|device| (device, InrouCgroupIoLimits::from(limits)))
            .collect::<BTreeMap<_, _>>();
        let io_max_path = self.attestation.worker_path.join("io.max");
        for (device, io_limits) in &expected_io {
            write_control(
                &io_max_path,
                &format_inrou_io_max_line(*device, *io_limits),
                "Inrou io.max",
            )?;
        }
        let actual_io = parse_inrou_io_max(&read_bounded_text(
            &io_max_path,
            INROU_CGROUP_CONTROL_MAX_BYTES,
            "Inrou io.max",
        )?)?;
        if actual_io != expected_io {
            eyre::bail!(
                "kernel retained Inrou io.max {:?} instead of {:?}",
                actual_io,
                expected_io
            );
        }
        Ok(())
    }
}

impl Drop for InrouWorkerCgroup {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        if !self.launcher_placement_attempted {
            if let Err(error) = self.cleanup_unlaunched_bounded() {
                iroha_logger::error!(
                    ?error,
                    cgroup = %self.attestation.worker_path.display(),
                    "failed to clean an unlaunched Inrou cgroup; retaining the confined subtree"
                );
            }
            return;
        }
        // Killing and proving the subgroup empty is safe on drop, but removal
        // still requires the owner's explicit direct-child exit proof. Keep
        // the empty root-custodied directory as a fail-closed restart barrier.
        match self.kill_and_attest_empty_bounded() {
            Ok(_) => iroha_logger::error!(
                cgroup = %self.attestation.worker_path.display(),
                "dropped a launched Inrou cgroup without direct-child exit proof; retaining the empty confined subtree"
            ),
            Err(error) => iroha_logger::error!(
                ?error,
                cgroup = %self.attestation.worker_path.display(),
                "failed to empty a dropped Inrou cgroup; retaining the confined subtree"
            ),
        }
    }
}

impl InrouLaunchBarrier {
    pub(super) fn create() -> eyre::Result<Self> {
        let (gate_reader, gate_writer) = rustix::pipe::pipe_with(rustix::pipe::PipeFlags::CLOEXEC)
            .wrap_err("create anonymous Inrou cgroup gate pipe")?;
        let (ack_reader, ack_writer) = rustix::pipe::pipe_with(rustix::pipe::PipeFlags::CLOEXEC)
            .wrap_err("create anonymous Inrou cgroup acknowledgement pipe")?;
        let ack_reader = fs::File::from(ack_reader);
        let ack_flags = rustix::fs::fcntl_getfl(&ack_reader)?;
        rustix::fs::fcntl_setfl(&ack_reader, ack_flags | rustix::fs::OFlags::NONBLOCK)?;
        Ok(Self {
            child_gate_reader: Some(fs::File::from(gate_reader)),
            parent_gate_writer: Some(fs::File::from(gate_writer)),
            parent_ack_reader: Some(ack_reader),
            child_ack_writer: Some(fs::File::from(ack_writer)),
        })
    }

    pub(super) fn child_gate_reader(&self) -> eyre::Result<&fs::File> {
        self.child_gate_reader
            .as_ref()
            .ok_or_else(|| eyre::eyre!("Inrou launch gate child descriptor was already retired"))
    }

    pub(super) fn child_ack_writer(&self) -> eyre::Result<&fs::File> {
        self.child_ack_writer.as_ref().ok_or_else(|| {
            eyre::eyre!("Inrou launch acknowledgement child descriptor was already retired")
        })
    }

    pub(super) fn child_spawned(&mut self) {
        drop(self.child_gate_reader.take());
        drop(self.child_ack_writer.take());
    }

    pub(super) fn release(&mut self) -> eyre::Result<()> {
        if self.child_gate_reader.is_some() || self.child_ack_writer.is_some() {
            eyre::bail!("Inrou launch barrier cannot release before the child spawn boundary");
        }
        let mut writer = self
            .parent_gate_writer
            .take()
            .ok_or_else(|| eyre::eyre!("Inrou launch barrier gate was already released"))?;
        writer
            .write_all(INROU_CGROUP_BARRIER_TOKEN)
            .wrap_err("release Inrou cgroup launch barrier")?;
        drop(writer);
        let reader = self
            .parent_ack_reader
            .as_mut()
            .ok_or_else(|| eyre::eyre!("Inrou launch acknowledgement reader is unavailable"))?;
        let deadline = std::time::Instant::now() + INROU_CGROUP_BARRIER_RELEASE_TIMEOUT;
        let mut acknowledgement = Vec::with_capacity(INROU_CGROUP_BARRIER_ACK.len() + 1);
        loop {
            if acknowledgement.len() > INROU_CGROUP_BARRIER_ACK.len() {
                eyre::bail!("Inrou launch child returned an overlong acknowledgement token");
            }
            let mut chunk = [0_u8; 64];
            let maximum =
                (INROU_CGROUP_BARRIER_ACK.len() + 1 - acknowledgement.len()).min(chunk.len());
            match reader.read(&mut chunk[..maximum]) {
                Ok(0) => break,
                Ok(read) => acknowledgement.extend_from_slice(&chunk[..read]),
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                    ) && std::time::Instant::now() < deadline =>
                {
                    std::thread::sleep(Duration::from_millis(1));
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => eyre::bail!(
                    "Inrou launch child did not acknowledge cgroup validation within {:?}",
                    INROU_CGROUP_BARRIER_RELEASE_TIMEOUT
                ),
                Err(error) => return Err(error).wrap_err("read Inrou launch acknowledgement"),
            }
        }
        if acknowledgement != INROU_CGROUP_BARRIER_ACK {
            eyre::bail!("Inrou launch child returned an invalid cgroup acknowledgement token");
        }
        drop(self.parent_ack_reader.take());
        Ok(())
    }
}

impl Drop for InrouLaunchBarrier {
    fn drop(&mut self) {
        self.child_spawned();
        drop(self.parent_gate_writer.take());
        drop(self.parent_ack_reader.take());
    }
}

pub(super) fn ensure_inrou_cgroup_v2_available() -> eyre::Result<()> {
    prepare_inrou_cgroup_root().map(|_| ())
}

/// Prove that startup inherited no worker cgroup from an earlier supervisor.
///
/// This must run immediately before the real startup probe. An empty worker
/// subtree is the only durable evidence that no orphaned worker can continue
/// charging a reporter counter after process restart.
pub(super) fn attest_inrou_worker_absence() -> eyre::Result<()> {
    let subtree = prepare_inrou_cgroup_root()?;
    validate_root_custodied_directory(&subtree, "Inrou cgroup root")?;
    let directory = fs::File::open(&subtree)
        .wrap_err_with(|| format!("open Inrou cgroup root {}", subtree.display()))?;
    let opened = directory.metadata()?;
    let named_before = fs::symlink_metadata(&subtree)?;
    if opened.dev() != named_before.dev() || opened.ino() != named_before.ino() {
        eyre::bail!("Inrou cgroup root changed while it was opened");
    }
    let mut entry_count = 0_usize;
    for entry in fs::read_dir(&subtree)? {
        if entry_count == INROU_CGROUP_ROOT_MAX_ENTRIES {
            eyre::bail!(
                "Inrou cgroup root exceeds its {INROU_CGROUP_ROOT_MAX_ENTRIES}-entry startup scan bound"
            );
        }
        entry_count += 1;
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            eyre::bail!(
                "Inrou cgroup root contains unexpected symlink {}",
                entry.path().display()
            );
        }
        if file_type.is_dir() {
            eyre::bail!(
                "Inrou startup found a pre-existing child cgroup {}; worker absence is not attested",
                entry.path().display()
            );
        }
    }
    let named_after = fs::symlink_metadata(&subtree)?;
    validate_root_custodied_directory(&subtree, "Inrou cgroup root")?;
    if opened.dev() != named_after.dev() || opened.ino() != named_after.ino() {
        eyre::bail!("Inrou cgroup root changed during the bounded startup scan");
    }
    Ok(())
}

fn prepare_inrou_cgroup_root() -> eyre::Result<PathBuf> {
    if rustix::process::geteuid() != rustix::process::Uid::ROOT {
        eyre::bail!("Inrou cgroup-v2 setup requires an effective root supervisor");
    }
    let mount = Path::new(INROU_CGROUP2_MOUNT);
    let mountinfo = read_bounded_text(
        Path::new("/proc/self/mountinfo"),
        INROU_CGROUP_CONTROL_MAX_BYTES,
        "Linux mountinfo",
    )?;
    validate_inrou_cgroup2_mount(&mountinfo, mount)?;
    validate_root_custodied_directory(mount, "cgroup-v2 mount")?;
    require_inrou_cgroup_controllers(&read_bounded_text(
        &mount.join("cgroup.controllers"),
        INROU_CGROUP_CONTROL_MAX_BYTES,
        "cgroup-v2 root controllers",
    )?)?;
    enable_inrou_subtree_controllers(mount)?;

    let subtree = mount.join(INROU_CGROUP_SUBTREE_NAME);
    match fs::create_dir(&subtree) {
        Ok(()) => fs::set_permissions(&subtree, fs::Permissions::from_mode(0o700))?,
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) => {
            return Err(error)
                .wrap_err_with(|| format!("create Inrou cgroup root {}", subtree.display()));
        }
    }
    validate_root_custodied_directory(&subtree, "Inrou cgroup root")?;
    if !read_cgroup_pids(&subtree.join("cgroup.procs"))?.is_empty() {
        eyre::bail!(
            "Inrou cgroup root {} contains direct processes instead of worker subgroups",
            subtree.display()
        );
    }
    require_inrou_cgroup_controllers(&read_bounded_text(
        &subtree.join("cgroup.controllers"),
        INROU_CGROUP_CONTROL_MAX_BYTES,
        "Inrou delegated controllers",
    )?)?;
    enable_inrou_subtree_controllers(&subtree)?;
    Ok(subtree)
}

fn enable_inrou_subtree_controllers(parent: &Path) -> eyre::Result<()> {
    let path = parent.join("cgroup.subtree_control");
    let mut enabled = parse_unique_words(&read_bounded_text(
        &path,
        INROU_CGROUP_CONTROL_MAX_BYTES,
        "cgroup.subtree_control",
    )?)?;
    let missing = INROU_CGROUP_REQUIRED_CONTROLLERS
        .iter()
        .copied()
        .filter(|controller| !enabled.contains(*controller))
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        write_control(
            &path,
            &missing
                .iter()
                .map(|controller| format!("+{controller}"))
                .collect::<Vec<_>>()
                .join(" "),
            "enable mandatory Inrou cgroup controllers",
        )?;
        enabled = parse_unique_words(&read_bounded_text(
            &path,
            INROU_CGROUP_CONTROL_MAX_BYTES,
            "updated cgroup.subtree_control",
        )?)?;
    }
    require_inrou_cgroup_controller_set(&enabled)
        .wrap_err("cgroup controller delegation did not retain the mandatory Inrou set")
}

fn project_inrou_cgroup_limits(
    resources: &SoraResourceLimitsV1,
) -> eyre::Result<InrouCgroupLimits> {
    resources
        .validate_for_inrou()
        .map_err(|error| eyre::eyre!("invalid Inrou resource contract: {error}"))?;
    let memory_max_bytes = resources
        .checked_inrou_host_memory_bytes()
        .ok_or_else(|| eyre::eyre!("Inrou memory cgroup projection overflow"))?;
    let cpu_millis = resources
        .checked_inrou_host_cpu_millis()
        .ok_or_else(|| eyre::eyre!("Inrou CPU cgroup projection overflow"))?;
    let cpu_quota_micros = cpu_millis
        .checked_mul(INROU_CGROUP_CPU_PERIOD_MICROS)
        .ok_or_else(|| eyre::eyre!("Inrou CPU quota projection overflow"))?
        .div_ceil(1_000);
    Ok(InrouCgroupLimits {
        memory_max_bytes,
        pids_max: INROU_CGROUP_QEMU_PIDS_MAX,
        cpu_quota_micros,
        cpu_period_micros: INROU_CGROUP_CPU_PERIOD_MICROS,
        io_read_bytes_per_sec: INROU_CGROUP_QEMU_IO_BYTES_PER_SEC_MAX,
        io_write_bytes_per_sec: INROU_CGROUP_QEMU_IO_BYTES_PER_SEC_MAX,
        io_read_iops: INROU_CGROUP_QEMU_IOPS_MAX,
        io_write_iops: INROU_CGROUP_QEMU_IOPS_MAX,
    })
}

fn resolve_inrou_cgroup_io_devices(
    io_backing_paths: &[&Path],
) -> eyre::Result<BTreeSet<InrouCgroupIoDevice>> {
    if io_backing_paths.is_empty() {
        eyre::bail!("Inrou cgroup IO confinement requires at least one VM backing path");
    }
    io_backing_paths
        .iter()
        .map(|path| {
            let metadata = fs::metadata(path)
                .wrap_err_with(|| format!("inspect Inrou IO backing path {}", path.display()))?;
            if !metadata.is_file() {
                eyre::bail!("Inrou IO backing path {} is not a regular file", path.display());
            }
            let major = rustix::fs::major(metadata.dev());
            let minor = rustix::fs::minor(metadata.dev());
            if major == 0 {
                eyre::bail!(
                    "Inrou IO backing path {} resolves to device {major}:{minor}, which cannot be governed by the cgroup-v2 IO controller",
                    path.display()
                );
            }
            Ok(InrouCgroupIoDevice { major, minor })
        })
        .collect()
}

fn inrou_cgroup_worker_name(key: InrouCgroupWorkerKey<'_>) -> String {
    let mut preimage = b"iroha.inrou.cgroup.worker.v1".to_vec();
    for value in [
        key.service_name.as_bytes(),
        key.service_version.as_bytes(),
        key.bundle_hash.as_bytes(),
    ] {
        preimage.extend_from_slice(&(value.len() as u64).to_be_bytes());
        preimage.extend_from_slice(value);
    }
    preimage.extend_from_slice(&key.replica_slot.to_be_bytes());
    preimage.extend_from_slice(&key.process_generation.to_be_bytes());
    format!(
        "{INROU_CGROUP_WORKER_PREFIX}{}",
        hex::encode(Hash::new(&preimage).as_ref())
    )
}

fn validate_inrou_cgroup_worker_name(name: &str) -> eyre::Result<()> {
    let Some(digest) = name.strip_prefix(INROU_CGROUP_WORKER_PREFIX) else {
        eyre::bail!("Inrou cgroup worker name lacks the fixed worker prefix");
    };
    if digest.len() != Hash::LENGTH * 2
        || !digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        eyre::bail!("Inrou cgroup worker name must contain one lowercase hash");
    }
    Ok(())
}

fn validate_inrou_proc_cgroup(contents: &str, expected_path: &str) -> eyre::Result<()> {
    if !expected_path.starts_with(&format!("/{INROU_CGROUP_SUBTREE_NAME}/"))
        || expected_path
            .rsplit_once('/')
            .is_none_or(|(_, name)| validate_inrou_cgroup_worker_name(name).is_err())
    {
        eyre::bail!("expected Inrou procfs cgroup path is not canonical");
    }
    let mut lines = contents.lines();
    let Some(line) = lines.next() else {
        eyre::bail!("Inrou process must expose exactly one unified cgroup-v2 membership record");
    };
    if lines.next().is_some() {
        eyre::bail!("Inrou process must expose exactly one unified cgroup-v2 membership record");
    }
    let mut fields = line.split(':');
    if fields.next() != Some("0")
        || fields.next() != Some("")
        || fields.next() != Some(expected_path)
        || fields.next().is_some()
    {
        eyre::bail!("Inrou process cgroup membership must be exactly `0::{expected_path}`");
    }
    Ok(())
}

fn validate_inrou_cgroup2_mount(contents: &str, expected_mount: &Path) -> eyre::Result<()> {
    let expected_mount = expected_mount
        .to_str()
        .ok_or_else(|| eyre::eyre!("cgroup-v2 mount path is not UTF-8"))?;
    let mut matches = 0_u8;
    for line in contents.lines() {
        let Some((before_separator, after_separator)) = line.split_once(" - ") else {
            eyre::bail!("Linux mountinfo contains a malformed record");
        };
        let mut fields = before_separator.split_ascii_whitespace();
        let mountpoint = fields.nth(4);
        let has_mount_options = fields.next().is_some();
        let mut after = after_separator.split_ascii_whitespace();
        let filesystem_type = after.next();
        let has_mount_source = after.next().is_some();
        let has_super_options = after.next().is_some();
        if mountpoint.is_none()
            || !has_mount_options
            || filesystem_type.is_none()
            || !has_mount_source
            || !has_super_options
        {
            eyre::bail!("Linux mountinfo contains a truncated record");
        }
        if mountpoint == Some(expected_mount) {
            matches = matches
                .checked_add(1)
                .ok_or_else(|| eyre::eyre!("too many cgroup mount records"))?;
            if filesystem_type != Some("cgroup2") {
                eyre::bail!("{expected_mount} is not a cgroup-v2 filesystem");
            }
        }
    }
    if matches != 1 {
        eyre::bail!("Linux mountinfo must contain one exact cgroup-v2 mount at {expected_mount}");
    }
    Ok(())
}

fn validate_root_custodied_directory(path: &Path, label: &str) -> eyre::Result<()> {
    let named = fs::symlink_metadata(path)
        .wrap_err_with(|| format!("inspect {label} {}", path.display()))?;
    if named.file_type().is_symlink()
        || !named.is_dir()
        || named.uid() != 0
        || named.gid() != 0
        || named.mode() & 0o022 != 0
    {
        eyre::bail!(
            "{label} {} must be a direct root:root directory not writable by group or other",
            path.display()
        );
    }
    Ok(())
}

fn require_inrou_cgroup_kill_control(path: &Path) -> eyre::Result<()> {
    let named = fs::symlink_metadata(path)
        .wrap_err_with(|| format!("inspect mandatory Inrou cgroup.kill {}", path.display()))?;
    if named.file_type().is_symlink() || !named.is_file() || named.uid() != 0 {
        eyre::bail!(
            "mandatory Inrou cgroup.kill {} must be a direct root-owned control file",
            path.display()
        );
    }
    let mut options = fs::OpenOptions::new();
    options
        .write(true)
        .custom_flags((rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::CLOEXEC).bits() as i32);
    let opened = options.open(path).wrap_err_with(|| {
        format!(
            "open mandatory writable Inrou cgroup.kill {}",
            path.display()
        )
    })?;
    let actual = opened.metadata()?;
    if actual.dev() != named.dev() || actual.ino() != named.ino() {
        eyre::bail!("mandatory Inrou cgroup.kill changed while it was opened");
    }
    Ok(())
}

fn require_inrou_cgroup_controllers(contents: &str) -> eyre::Result<()> {
    require_inrou_cgroup_controller_set(&parse_unique_words(contents)?)
}

fn require_inrou_cgroup_controller_set(controllers: &BTreeSet<String>) -> eyre::Result<()> {
    let missing = INROU_CGROUP_REQUIRED_CONTROLLERS
        .iter()
        .copied()
        .filter(|controller| !controllers.contains(*controller))
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        eyre::bail!(
            "Inrou requires delegated cgroup-v2 controllers {:?}; missing {:?}",
            INROU_CGROUP_REQUIRED_CONTROLLERS,
            missing
        );
    }
    Ok(())
}

fn parse_unique_words(contents: &str) -> eyre::Result<BTreeSet<String>> {
    let mut words = BTreeSet::new();
    for word in contents.split_ascii_whitespace() {
        if !words.insert(word.to_owned()) {
            eyre::bail!("cgroup controller list repeats `{word}`");
        }
    }
    Ok(words)
}

fn parse_inrou_cgroup_populated(contents: &str) -> eyre::Result<bool> {
    let mut populated = None;
    for line in contents.lines() {
        let mut fields = line.split_ascii_whitespace();
        let name = fields.next();
        let value = fields.next();
        if name.is_none() || value.is_none() || fields.next().is_some() {
            eyre::bail!("cgroup.events contains a malformed record");
        }
        if name == Some("populated") {
            if populated.is_some() {
                eyre::bail!("cgroup.events repeats `populated`");
            }
            populated = Some(match value {
                Some("0") => false,
                Some("1") => true,
                _ => eyre::bail!("cgroup.events has a non-boolean populated value"),
            });
        }
    }
    populated.ok_or_else(|| eyre::eyre!("cgroup.events omitted `populated`"))
}

fn format_inrou_io_max_line(device: InrouCgroupIoDevice, limits: InrouCgroupIoLimits) -> String {
    format!(
        "{}:{} rbps={} wbps={} riops={} wiops={}",
        device.major,
        device.minor,
        limits.read_bytes_per_sec,
        limits.write_bytes_per_sec,
        limits.read_iops,
        limits.write_iops,
    )
}

fn parse_inrou_io_max(
    contents: &str,
) -> eyre::Result<BTreeMap<InrouCgroupIoDevice, InrouCgroupIoLimits>> {
    let mut records = BTreeMap::new();
    for line in contents.lines() {
        let mut fields = line.split_ascii_whitespace();
        let device = fields
            .next()
            .ok_or_else(|| eyre::eyre!("io.max contains an empty record"))?;
        let (major, minor) = device
            .split_once(':')
            .ok_or_else(|| eyre::eyre!("io.max device omits `:`"))?;
        let device = InrouCgroupIoDevice {
            major: major.parse().wrap_err("parse io.max major device id")?,
            minor: minor.parse().wrap_err("parse io.max minor device id")?,
        };
        let mut values = BTreeMap::new();
        for field in fields {
            let (name, value) = field
                .split_once('=')
                .ok_or_else(|| eyre::eyre!("io.max limit omits `=`"))?;
            let value = value.parse::<u64>().wrap_err_with(|| {
                format!("parse finite io.max `{name}` value; `max` is not accepted")
            })?;
            if values.insert(name, value).is_some() {
                eyre::bail!("io.max repeats `{name}` for {major}:{minor}");
            }
        }
        let required = |name| {
            values
                .get(name)
                .copied()
                .ok_or_else(|| eyre::eyre!("io.max omitted `{name}` for {major}:{minor}"))
        };
        let limits = InrouCgroupIoLimits {
            read_bytes_per_sec: required("rbps")?,
            write_bytes_per_sec: required("wbps")?,
            read_iops: required("riops")?,
            write_iops: required("wiops")?,
        };
        if let Some(unexpected) = values
            .keys()
            .find(|name| !matches!(**name, "rbps" | "wbps" | "riops" | "wiops"))
        {
            eyre::bail!("io.max contains unexpected limit `{unexpected}` for {major}:{minor}");
        }
        if records.insert(device, limits).is_some() {
            eyre::bail!("io.max repeats device {major}:{minor}");
        }
    }
    Ok(records)
}

fn read_cgroup_pids(path: &Path) -> eyre::Result<Vec<u32>> {
    let contents = read_bounded_text(path, INROU_CGROUP_CONTROL_MAX_BYTES, "cgroup.procs")?;
    let mut pids = contents
        .lines()
        .map(|line| {
            if line.is_empty() || line.trim() != line {
                eyre::bail!("cgroup.procs contains a non-canonical pid");
            }
            line.parse::<u32>().wrap_err("parse cgroup.procs pid")
        })
        .collect::<eyre::Result<Vec<_>>>()?;
    pids.sort_unstable();
    if pids.windows(2).any(|pair| pair[0] == pair[1]) {
        eyre::bail!("cgroup.procs repeats a pid");
    }
    Ok(pids)
}

fn read_bounded_text(path: &Path, maximum_bytes: u64, label: &str) -> eyre::Result<String> {
    let mut bytes = Vec::new();
    fs::File::open(path)
        .wrap_err_with(|| format!("open {label} {}", path.display()))?
        .take(maximum_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .wrap_err_with(|| format!("read {label} {}", path.display()))?;
    if bytes.len() as u64 > maximum_bytes {
        eyre::bail!("{label} {} exceeds {maximum_bytes} bytes", path.display());
    }
    String::from_utf8(bytes).wrap_err_with(|| format!("decode {label} {}", path.display()))
}

fn write_control(path: &Path, value: &str, label: &str) -> eyre::Result<()> {
    fs::write(path, value.as_bytes()).wrap_err_with(|| {
        format!(
            "{label} through {}; controller absence or delegation failure is fatal",
            path.display()
        )
    })
}

fn write_control_and_require_exact(path: &Path, value: &str, label: &str) -> eyre::Result<()> {
    write_control(path, value, label)?;
    let actual = read_bounded_text(path, INROU_CGROUP_CONTROL_MAX_BYTES, label)?;
    if actual.trim() != value {
        eyre::bail!(
            "kernel retained `{}` for {label} instead of `{value}`",
            actual.trim()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::io::{Read as _, Write as _};
    use std::num::{NonZeroU16, NonZeroU32, NonZeroU64};

    use super::*;

    fn resources() -> SoraResourceLimitsV1 {
        SoraResourceLimitsV1 {
            cpu_millis: NonZeroU32::new(1_500).expect("nonzero"),
            memory_bytes: NonZeroU64::new(512 * 1024 * 1024).expect("nonzero"),
            ephemeral_storage_bytes: NonZeroU64::new(60 * 1024 * 1024).expect("nonzero"),
            max_open_files_per_process: NonZeroU32::new(256).expect("nonzero"),
            max_tasks: NonZeroU16::new(16).expect("nonzero"),
        }
    }

    fn worker_key<'a>() -> InrouCgroupWorkerKey<'a> {
        InrouCgroupWorkerKey {
            service_name: "http-canary",
            service_version: "1.0.0",
            replica_slot: 2,
            process_generation: 7,
            bundle_hash: "0123456789abcdef",
        }
    }

    #[test]
    fn resource_projection_uses_guest_cpu_memory_and_fixed_host_safety_limits() -> eyre::Result<()>
    {
        let projected = project_inrou_cgroup_limits(&resources())?;
        assert_eq!(projected.memory_max_bytes, 768 * 1024 * 1024);
        assert_eq!(projected.pids_max, INROU_CGROUP_QEMU_PIDS_MAX);
        assert_eq!(projected.cpu_quota_micros, 175_000);
        assert_eq!(projected.cpu_period_micros, 100_000);
        assert_eq!(
            projected.io_read_bytes_per_sec,
            INROU_CGROUP_QEMU_IO_BYTES_PER_SEC_MAX
        );
        assert_eq!(
            projected.io_write_bytes_per_sec,
            INROU_CGROUP_QEMU_IO_BYTES_PER_SEC_MAX
        );
        assert_eq!(projected.io_read_iops, INROU_CGROUP_QEMU_IOPS_MAX);
        assert_eq!(projected.io_write_iops, INROU_CGROUP_QEMU_IOPS_MAX);
        Ok(())
    }

    #[test]
    fn host_safety_projection_ignores_guest_fd_task_and_storage_limits() -> eyre::Result<()> {
        let baseline = resources();
        let mut unrelated_guest_limits = baseline;
        unrelated_guest_limits.ephemeral_storage_bytes =
            NonZeroU64::new(16 * 1024 * 1024 * 1024).expect("nonzero");
        unrelated_guest_limits.max_open_files_per_process =
            NonZeroU32::new(8_192).expect("nonzero");
        unrelated_guest_limits.max_tasks = NonZeroU16::new(1_024).expect("nonzero");

        assert_eq!(
            project_inrou_cgroup_limits(&baseline)?,
            project_inrou_cgroup_limits(&unrelated_guest_limits)?,
            "guest FD, task, and storage contracts must not masquerade as host QEMU limits"
        );
        Ok(())
    }

    #[test]
    fn resource_projection_accepts_v1_cpu_boundary_and_rejects_above_it() -> eyre::Result<()> {
        let mut boundary = resources();
        boundary.cpu_millis =
            NonZeroU32::new(iroha_data_model::soracloud::SORA_INROU_MAX_CPU_MILLIS_V1)
                .expect("nonzero V1 CPU ceiling");
        let projected = project_inrou_cgroup_limits(&boundary)?;
        assert_eq!(projected.pids_max, INROU_CGROUP_QEMU_PIDS_MAX);

        boundary.cpu_millis = NonZeroU32::new(
            iroha_data_model::soracloud::SORA_INROU_MAX_CPU_MILLIS_V1
                + iroha_data_model::soracloud::SORA_INROU_CPU_MILLIS_ALIGNMENT_V1,
        )
        .expect("nonzero CPU above V1");
        let _projection_error = project_inrou_cgroup_limits(&boundary)
            .expect_err("cgroup projection must reject resources above the qualified V1 ceiling");
        Ok(())
    }

    #[test]
    fn worker_name_is_canonical_and_binds_every_worker_dimension() -> eyre::Result<()> {
        let base = worker_key();
        let name = inrou_cgroup_worker_name(base);
        validate_inrou_cgroup_worker_name(&name)?;
        assert_eq!(
            name.len(),
            INROU_CGROUP_WORKER_PREFIX.len() + Hash::LENGTH * 2
        );
        let variants = [
            InrouCgroupWorkerKey {
                service_name: "http-canary-2",
                ..base
            },
            InrouCgroupWorkerKey {
                service_version: "1.0.1",
                ..base
            },
            InrouCgroupWorkerKey {
                replica_slot: 3,
                ..base
            },
            InrouCgroupWorkerKey {
                process_generation: 8,
                ..base
            },
            InrouCgroupWorkerKey {
                bundle_hash: "fedcba9876543210",
                ..base
            },
        ];
        for variant in variants {
            assert_ne!(inrou_cgroup_worker_name(variant), name);
        }
        for (rejected, expected_error) in [
            ("worker-../escape", "must contain one lowercase hash"),
            ("worker-ABCDEF", "must contain one lowercase hash"),
            ("worker-00", "must contain one lowercase hash"),
            (
                "other-0000000000000000000000000000000000000000000000000000000000000000",
                "lacks the fixed worker prefix",
            ),
        ] {
            let error = validate_inrou_cgroup_worker_name(rejected)
                .expect_err("non-canonical worker names must fail closed");
            assert!(
                error.to_string().contains(expected_error),
                "unexpected rejection for {rejected}: {error:?}"
            );
        }
        Ok(())
    }

    #[test]
    fn procfs_attestation_accepts_only_one_exact_unified_membership() -> eyre::Result<()> {
        let name = inrou_cgroup_worker_name(worker_key());
        let expected = format!("/{INROU_CGROUP_SUBTREE_NAME}/{name}");
        validate_inrou_proc_cgroup(&format!("0::{expected}\n"), &expected)?;
        for (rejected, expected_error) in [
            (format!("1:cpu:{expected}\n"), "membership must be exactly"),
            (format!("0::/other/{name}\n"), "membership must be exactly"),
            (
                format!("0::{expected}\n0::{expected}\n"),
                "exactly one unified cgroup-v2 membership record",
            ),
            (
                "".to_owned(),
                "exactly one unified cgroup-v2 membership record",
            ),
        ] {
            let error = validate_inrou_proc_cgroup(&rejected, &expected)
                .expect_err("non-exact cgroup membership must fail closed");
            assert!(
                error.to_string().contains(expected_error),
                "unexpected rejection for {rejected:?}: {error:?}"
            );
        }
        Ok(())
    }

    #[test]
    fn controller_and_mount_parsers_have_no_legacy_or_partial_fallback() -> eyre::Result<()> {
        require_inrou_cgroup_controllers("cpuset cpu io memory hugetlb pids")?;
        for missing in [
            "io memory pids",
            "cpu memory pids",
            "cpu io pids",
            "cpu io memory",
        ] {
            let error = require_inrou_cgroup_controllers(missing)
                .expect_err("every mandatory controller must be delegated");
            assert!(
                error
                    .to_string()
                    .contains("Inrou requires delegated cgroup-v2 controllers"),
                "unexpected controller rejection for {missing:?}: {error:?}"
            );
        }
        let duplicate_error = require_inrou_cgroup_controllers("cpu cpu io memory pids")
            .expect_err("ambiguous duplicate controller records must fail closed");
        assert!(
            duplicate_error
                .to_string()
                .contains("cgroup controller list repeats `cpu`"),
            "unexpected duplicate-controller rejection: {duplicate_error:?}"
        );
        let mount = Path::new("/sys/fs/cgroup");
        validate_inrou_cgroup2_mount(
            "29 23 0:26 / /sys/fs/cgroup rw,nosuid,nodev,noexec,relatime - cgroup2 cgroup rw\n",
            mount,
        )?;
        let legacy_mount_error = validate_inrou_cgroup2_mount(
            "29 23 0:26 / /sys/fs/cgroup rw - cgroup cgroup rw,cpu\n",
            mount,
        )
        .expect_err("cgroup-v1 must never be accepted as a fallback");
        assert!(
            legacy_mount_error
                .to_string()
                .contains("is not a cgroup-v2 filesystem"),
            "unexpected cgroup-v1 rejection: {legacy_mount_error:?}"
        );
        Ok(())
    }

    #[test]
    fn event_and_io_attestation_rejects_unbounded_or_ambiguous_values() -> eyre::Result<()> {
        assert!(!parse_inrou_cgroup_populated("populated 0\nfrozen 0\n")?);
        assert!(parse_inrou_cgroup_populated("populated 1\nfrozen 0\n")?);
        for (rejected, expected_error) in [
            ("frozen 0\n", "omitted `populated`"),
            ("populated max\n", "non-boolean populated value"),
            ("populated 0\npopulated 1\n", "repeats `populated`"),
        ] {
            let error = parse_inrou_cgroup_populated(rejected)
                .expect_err("malformed populated state must fail closed");
            assert!(
                error.to_string().contains(expected_error),
                "unexpected cgroup.events rejection for {rejected:?}: {error:?}"
            );
        }

        let limits = InrouCgroupIoLimits::from(project_inrou_cgroup_limits(&resources())?);
        let device = InrouCgroupIoDevice { major: 8, minor: 1 };
        let parsed = parse_inrou_io_max(&format_inrou_io_max_line(device, limits))?;
        assert_eq!(parsed.get(&device), Some(&limits));
        let unbounded_error = parse_inrou_io_max("8:1 rbps=max wbps=1 riops=1 wiops=1")
            .expect_err("an unbounded IO controller value must fail closed");
        assert!(
            unbounded_error
                .to_string()
                .contains("parse finite io.max `rbps` value; `max` is not accepted"),
            "unexpected unbounded io.max rejection: {unbounded_error:?}"
        );
        let partial_error = parse_inrou_io_max("8:1 rbps=1 wbps=1 riops=1")
            .expect_err("a partial IO controller record must fail closed");
        assert!(
            partial_error
                .to_string()
                .contains("io.max omitted `wiops` for 8:1"),
            "unexpected partial io.max rejection: {partial_error:?}"
        );
        let extra_error = parse_inrou_io_max("8:1 rbps=1 wbps=1 riops=1 wiops=1 burst=1")
            .expect_err("an unknown IO controller limit must fail closed");
        assert!(
            extra_error
                .to_string()
                .contains("io.max contains unexpected limit `burst` for 8:1"),
            "unexpected extra io.max limit rejection: {extra_error:?}"
        );
        Ok(())
    }

    fn internal_launcher_arguments(binding_fds: &[RawFd]) -> Vec<OsString> {
        let mut arguments = vec![
            "17".into(),
            "29".into(),
            format!("/iroha-inrou-v1/worker-{}", "01".repeat(Hash::LENGTH)).into(),
            binding_fds.len().to_string().into(),
        ];
        for (index, descriptor) in binding_fds.iter().enumerate() {
            arguments.extend([
                OsString::from("--ro-bind-fd"),
                descriptor.to_string().into(),
                format!("/inrou/input/binding-{index}").into(),
            ]);
        }
        arguments.push(INROU_INTERNAL_BWRAP_PATH.into());
        arguments.extend(
            [
                "--die-with-parent",
                "--new-session",
                "--as-pid-1",
                "--unshare-pid",
                "--unshare-net",
                "--unshare-ipc",
                "--unshare-uts",
                "--unshare-cgroup",
                "--clearenv",
            ]
            .into_iter()
            .map(OsString::from),
        );
        for (index, descriptor) in binding_fds.iter().enumerate() {
            arguments.extend([
                OsString::from("--ro-bind-fd"),
                descriptor.to_string().into(),
                format!("/inrou/input/binding-{index}").into(),
            ]);
        }
        arguments.extend([OsString::from("--"), OsString::from("/inrou/bin/qemu")]);
        arguments
    }

    #[test]
    fn internal_launcher_parser_accepts_dynamic_descriptors_and_zero_bindings() -> eyre::Result<()>
    {
        let parsed = parse_inrou_internal_launcher_v1(internal_launcher_arguments(&[41, 907]))?;
        assert_eq!(parsed.gate_fd, 17);
        assert_eq!(parsed.acknowledgement_fd, 29);
        assert_eq!(
            parsed
                .bindings
                .iter()
                .map(|binding| binding.descriptor)
                .collect::<Vec<_>>(),
            [41, 907]
        );
        assert_eq!(
            parsed.expected_cgroup_path,
            format!("/iroha-inrou-v1/worker-{}", "01".repeat(Hash::LENGTH))
        );

        let parsed = parse_inrou_internal_launcher_v1(internal_launcher_arguments(&[]))?;
        assert!(parsed.bindings.is_empty());
        Ok(())
    }

    #[test]
    fn internal_launcher_parser_enforces_exact_binding_and_argv_boundaries() -> eyre::Result<()> {
        let maximum_binding_fds = (100..)
            .take(INROU_INTERNAL_LAUNCHER_MAX_BINDINGS)
            .collect::<Vec<RawFd>>();
        let parsed =
            parse_inrou_internal_launcher_v1(internal_launcher_arguments(&maximum_binding_fds))?;
        assert_eq!(parsed.bindings.len(), INROU_INTERNAL_LAUNCHER_MAX_BINDINGS);

        let overflow_binding_fds = (100..)
            .take(INROU_INTERNAL_LAUNCHER_MAX_BINDINGS + 1)
            .collect::<Vec<RawFd>>();
        let _binding_overflow_error =
            parse_inrou_internal_launcher_v1(internal_launcher_arguments(&overflow_binding_fds))
                .expect_err("one binding above the V1 parser bound must fail closed");

        let mut maximum_arguments = internal_launcher_arguments(&[]);
        maximum_arguments.resize(
            INROU_INTERNAL_LAUNCHER_MAX_ARGUMENTS,
            OsString::from("qemu-padding-argument"),
        );
        parse_inrou_internal_launcher_v1(maximum_arguments.clone())?;
        maximum_arguments.push("qemu-argument-overflow".into());
        let _argument_overflow_error = parse_inrou_internal_launcher_v1(maximum_arguments)
            .expect_err("one argument above the V1 parser bound must fail closed");

        let mut maximum_argument_bytes = internal_launcher_arguments(&[]);
        maximum_argument_bytes.push(
            "x".repeat(INROU_INTERNAL_LAUNCHER_MAX_ARGUMENT_BYTES)
                .into(),
        );
        parse_inrou_internal_launcher_v1(maximum_argument_bytes.clone())?;
        *maximum_argument_bytes
            .last_mut()
            .expect("padded launcher argument") = "x"
            .repeat(INROU_INTERNAL_LAUNCHER_MAX_ARGUMENT_BYTES + 1)
            .into();
        let _argument_bytes_overflow_error =
            parse_inrou_internal_launcher_v1(maximum_argument_bytes)
                .expect_err("one byte above the V1 per-argument bound must fail closed");
        Ok(())
    }

    #[test]
    fn maximum_production_launcher_plan_stays_below_internal_parser_bounds() {
        // The fixed count covers launcher framing, fixed bubblewrap/setpriv/QEMU
        // arguments, the optional initrd, QMP, and the three non-lease drives.
        // Each binding appears once in the typed map and once in bubblewrap;
        // each lease also adds one QEMU drive/device pair.
        const FIXED_ARGUMENTS_WITH_INITRD_AND_FIXED_DRIVES: usize = 99;
        const ARGUMENTS_PER_BINDING: usize = 6;
        const ARGUMENTS_PER_LEASE_DISK: usize = 4;
        let maximum_bindings =
            super::super::inrou_namespace::INROU_NAMESPACE_MAX_PRODUCTION_BINDINGS;
        let maximum_lease_disks = super::super::inrou_namespace::INROU_NAMESPACE_MAX_LEASE_DISKS;
        let maximum_arguments = FIXED_ARGUMENTS_WITH_INITRD_AND_FIXED_DRIVES
            + maximum_bindings * ARGUMENTS_PER_BINDING
            + maximum_lease_disks * ARGUMENTS_PER_LEASE_DISK;

        assert_eq!(maximum_bindings, 39);
        assert_eq!(maximum_arguments, 461);
        assert!(maximum_bindings < INROU_INTERNAL_LAUNCHER_MAX_BINDINGS);
        assert!(maximum_arguments < INROU_INTERNAL_LAUNCHER_MAX_ARGUMENTS);
    }

    #[test]
    #[allow(unsafe_code)]
    fn internal_launcher_closes_unrelated_inherited_descriptors_in_subprocess() -> eyre::Result<()>
    {
        use std::{
            os::{fd::BorrowedFd, unix::process::CommandExt as _},
            process::Stdio,
        };

        const CHILD_MODE: &str = "IROHA_INROU_DESCRIPTOR_CLOSURE_CHILD_V1";
        const CHILD_SUCCESS_EXIT_CODE: i32 = 23;
        if let Some(descriptors) = std::env::var_os(CHILD_MODE) {
            let descriptors = descriptors
                .to_str()
                .ok_or_else(|| eyre::eyre!("descriptor test mode is not UTF-8"))?
                .split(':')
                .map(str::parse::<RawFd>)
                .collect::<Result<Vec<_>, _>>()?;
            let [retained, unrelated] = descriptors.as_slice() else {
                eyre::bail!("descriptor test mode has the wrong arity");
            };
            let inherited = list_open_inrou_launcher_descriptors()?;
            eyre::ensure!(
                inherited.contains(retained) && inherited.contains(unrelated),
                "descriptor test inputs did not survive exec"
            );
            close_unrelated_inrou_launcher_descriptors(&[*retained])?;
            let open = list_open_inrou_launcher_descriptors()?;
            eyre::ensure!(
                open.contains(retained),
                "typed binding descriptor was closed"
            );
            eyre::ensure!(
                !open.contains(unrelated),
                "unrelated inherited descriptor survived closure"
            );
            std::process::exit(CHILD_SUCCESS_EXIT_CODE);
        }

        let retained = fs::File::open("/dev/null")?;
        let unrelated = fs::File::open("/dev/null")?;
        let retained_fd = retained.as_raw_fd();
        let unrelated_fd = unrelated.as_raw_fd();
        eyre::ensure!(retained_fd > 2 && unrelated_fd > 2 && retained_fd != unrelated_fd);
        let mut command = Command::new(std::env::current_exe()?);
        command
            .arg("internal_launcher_closes_unrelated_inherited_descriptors_in_subprocess")
            .arg("--test-threads=1")
            .env(CHILD_MODE, format!("{retained_fd}:{unrelated_fd}"))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        // SAFETY: the closure performs only child-local F_SETFD syscalls over
        // parent-validated live descriptor numbers and constructs no strings.
        unsafe {
            command.pre_exec(move || {
                for descriptor in [retained_fd, unrelated_fd] {
                    rustix::io::fcntl_setfd(
                        BorrowedFd::borrow_raw(descriptor),
                        rustix::io::FdFlags::empty(),
                    )
                    .map_err(io::Error::from)?;
                }
                Ok(())
            });
        }
        let status = command.status()?;
        assert_eq!(
            status.code(),
            Some(CHILD_SUCCESS_EXIT_CODE),
            "descriptor-closure subprocess did not complete its proof"
        );
        Ok(())
    }

    #[test]
    fn internal_launcher_parser_rejects_ambiguous_or_drifted_requests() {
        let mut stdio = internal_launcher_arguments(&[41]);
        stdio[0] = "2".into();
        let _stdio_descriptor_error = parse_inrou_internal_launcher_v1(stdio)
            .expect_err("stdio descriptors must never be launcher inputs");

        let mut duplicate = internal_launcher_arguments(&[17]);
        duplicate[0] = "17".into();
        let _duplicate_descriptor_error = parse_inrou_internal_launcher_v1(duplicate)
            .expect_err("gate, acknowledgement, and binding descriptors must be unique");

        let mut noncanonical = internal_launcher_arguments(&[41]);
        noncanonical[0] = "017".into();
        let _noncanonical_descriptor_error = parse_inrou_internal_launcher_v1(noncanonical)
            .expect_err("descriptor spellings must be canonical decimal");

        let mut cgroup_drift = internal_launcher_arguments(&[41]);
        cgroup_drift[2] = "/iroha-inrou-v1/../outside".into();
        let _cgroup_path_error = parse_inrou_internal_launcher_v1(cgroup_drift)
            .expect_err("the expected cgroup path must be canonical");

        let mut truncated = internal_launcher_arguments(&[41]);
        truncated[3] = "2".into();
        let _truncated_binding_list_error = parse_inrou_internal_launcher_v1(truncated)
            .expect_err("the binding count must exactly frame the binding list");

        let mut alternate_program = internal_launcher_arguments(&[41]);
        alternate_program[7] = "/bin/sh".into();
        let _alternate_program_error = parse_inrou_internal_launcher_v1(alternate_program)
            .expect_err("only the pinned bubblewrap path may be executed");

        let mut mismatched_map = internal_launcher_arguments(&[41]);
        let mapped = mismatched_map
            .iter()
            .position(|argument| argument == "41")
            .expect("binding descriptor argument");
        mismatched_map[mapped + 1..]
            .iter_mut()
            .find(|argument| argument.as_os_str() == OsStr::new("41"))
            .map(|argument| *argument = "42".into())
            .expect("bubblewrap binding descriptor");
        let _mismatched_binding_map_error = parse_inrou_internal_launcher_v1(mismatched_map)
            .expect_err("bubblewrap bindings must match the typed retained map");

        let mut widened_mode = internal_launcher_arguments(&[41]);
        let mut modes = widened_mode
            .iter_mut()
            .filter(|argument| argument.as_os_str() == OsStr::new("--ro-bind-fd"));
        assert!(modes.next().is_some(), "typed binding mode");
        *modes.next().expect("bubblewrap binding mode") = "--bind-fd".into();
        assert!(modes.next().is_none(), "only two binding mode records");
        let _widened_binding_mode_error = parse_inrou_internal_launcher_v1(widened_mode)
            .expect_err("bubblewrap cannot widen a typed read-only binding");

        let mut changed_destination = internal_launcher_arguments(&[41]);
        let mut destinations = changed_destination
            .iter_mut()
            .filter(|argument| argument.as_os_str() == OsStr::new("/inrou/input/binding-0"));
        assert!(destinations.next().is_some(), "typed binding destination");
        *destinations.next().expect("bubblewrap binding destination") = "/inrou/input/other".into();
        assert!(
            destinations.next().is_none(),
            "only two binding destination records"
        );
        let _changed_destination_error = parse_inrou_internal_launcher_v1(changed_destination)
            .expect_err("bubblewrap cannot retarget a typed binding");

        let mut duplicate_namespace_flag = internal_launcher_arguments(&[]);
        let separator = duplicate_namespace_flag
            .iter()
            .position(|argument| argument == "--")
            .expect("bubblewrap command separator");
        duplicate_namespace_flag.insert(separator, "--clearenv".into());
        let _duplicate_namespace_flag_error =
            parse_inrou_internal_launcher_v1(duplicate_namespace_flag)
                .expect_err("mandatory namespace flags must occur exactly once");
    }

    #[test]
    fn launch_barrier_refuses_release_before_child_spawn() -> eyre::Result<()> {
        let mut barrier = InrouLaunchBarrier::create()?;
        let _premature_release_error = barrier
            .release()
            .expect_err("a supervisor must retire its child pipe ends after spawn");
        Ok(())
    }

    #[test]
    fn anonymous_launch_barrier_requires_exact_gate_and_ack_tokens() -> eyre::Result<()> {
        for (acknowledgement, succeeds) in [
            (INROU_CGROUP_BARRIER_ACK.to_vec(), true),
            (b"inrou-cgroup-denied-v1\n".to_vec(), false),
            ([INROU_CGROUP_BARRIER_ACK, b"extra"].concat(), false),
        ] {
            let mut barrier = InrouLaunchBarrier::create()?;
            let mut gate = barrier.child_gate_reader()?.try_clone()?;
            let mut ack = barrier.child_ack_writer()?.try_clone()?;
            barrier.child_spawned();
            let child = std::thread::spawn(move || -> io::Result<()> {
                let mut token = Vec::new();
                gate.read_to_end(&mut token)?;
                if token != INROU_CGROUP_BARRIER_TOKEN {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "non-exact test gate token",
                    ));
                }
                ack.write_all(&acknowledgement)
            });
            assert_eq!(barrier.release().is_ok(), succeeds);
            child.join().expect("barrier child thread must not panic")?;
        }
        Ok(())
    }
}
