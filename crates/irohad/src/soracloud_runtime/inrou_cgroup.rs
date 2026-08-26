//! Fail-closed cgroup-v2 confinement for Linux Inrou PortableVM workers.
//!
//! Every PortableVM worker requires a root-custodied cgroup-v2 hierarchy,
//! projects every service resource limit into finite controller values, and
//! keeps the namespace launcher behind a named-pipe barrier until the
//! supervisor has placed and attested it.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{self, Read as _, Write as _},
    os::unix::fs::{FileTypeExt as _, MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _},
    path::{Path, PathBuf},
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
const INROU_CGROUP_QEMU_CPU_OVERHEAD_MILLIS: u64 = 250;
const INROU_CGROUP_QEMU_MEMORY_OVERHEAD_BYTES: u64 = 256 * 1024 * 1024;
const INROU_CGROUP_QEMU_PID_OVERHEAD: u64 = 32;
const INROU_CGROUP_IO_PROJECTION_WINDOW_SECS: u64 = 60;
const INROU_CGROUP_QEMU_IO_OVERHEAD_BYTES_PER_SEC: u64 = 16 * 1024 * 1024;
const INROU_CGROUP_QEMU_IOPS_OVERHEAD: u64 = 128;
const INROU_CGROUP_BARRIER_FILE: &str = ".inrou-cgroup-launch-v1";
const INROU_CGROUP_BARRIER_TOKEN: &[u8] = b"inrou-cgroup-go-v1\n";

/// Root shell gate placed before the fixed bubblewrap namespace launcher.
///
/// The supervisor is the only writer of the root-custodied FIFO. It releases
/// this gate only after writing the launcher's pid to `cgroup.procs` and
/// validating `/proc/<pid>/cgroup`. The child independently checks the unified
/// hierarchy path and closes every descriptor above stderr before it can exec
/// bubblewrap. QMP occupies stdin/stdout and the bounded runtime log occupies
/// stderr, so no other supervisor descriptor crosses the namespace boundary.
pub(super) const INROU_CGROUP_BARRIER_SCRIPT: &str = r#"inrou_barrier_path=$1
inrou_expected_cgroup=$2
shift 2
IFS= read -r inrou_barrier_token < "${inrou_barrier_path}" || exit 126
[ "${inrou_barrier_token}" = "inrou-cgroup-go-v1" ] || exit 126
inrou_seen_cgroup=0
while IFS=: read -r inrou_hierarchy inrou_controllers inrou_cgroup_path; do
    [ "${inrou_seen_cgroup}" -eq 0 ] || exit 126
    [ "${inrou_hierarchy}" = 0 ] || exit 126
    [ -z "${inrou_controllers}" ] || exit 126
    [ "${inrou_cgroup_path}" = "${inrou_expected_cgroup}" ] || exit 126
    inrou_seen_cgroup=1
done < /proc/self/cgroup
[ "${inrou_seen_cgroup}" -eq 1 ] || exit 126
for inrou_fd_path in /proc/self/fd/*; do
    inrou_fd=${inrou_fd_path##*/}
    case "${inrou_fd}" in
        0|1|2) continue ;;
        ''|*[!0-9]*) exit 125 ;;
    esac
    eval "exec ${inrou_fd}>&-" || exit 125
done
exec "$@"
"#;

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
    path: PathBuf,
    device: u64,
    inode: u64,
    child_gid: u32,
    active: bool,
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
    pub(super) fn create(parent: &Path, child_gid: u32) -> eyre::Result<Self> {
        validate_inrou_launch_barrier_parent(parent, child_gid)?;
        let path = parent.join(INROU_CGROUP_BARRIER_FILE);
        rustix::fs::mkfifoat(
            rustix::fs::CWD,
            &path,
            rustix::fs::Mode::from_raw_mode(0o600),
        )
        .wrap_err_with(|| {
            format!(
                "create unique root-custodied Inrou launch barrier {}; a pre-existing barrier is a fail-closed stale-runtime condition",
                path.display()
            )
        })?;
        let mut barrier = Self {
            path,
            device: 0,
            inode: 0,
            child_gid,
            active: true,
        };
        let mut options = fs::OpenOptions::new();
        options.read(true).custom_flags(
            (rustix::fs::OFlags::NONBLOCK
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::CLOEXEC)
                .bits() as i32,
        );
        let reader = options
            .open(&barrier.path)
            .wrap_err_with(|| format!("open {}", barrier.path.display()))?;
        rustix::fs::fchown(
            &reader,
            Some(rustix::fs::Uid::ROOT),
            Some(rustix::fs::Gid::from_raw(child_gid)),
        )?;
        rustix::fs::fchmod(&reader, rustix::fs::Mode::from_raw_mode(0o640))?;
        validate_inrou_launch_barrier(&barrier.path, &reader, child_gid)?;
        let metadata = reader.metadata()?;
        barrier.device = metadata.dev();
        barrier.inode = metadata.ino();
        Ok(barrier)
    }

    pub(super) fn path(&self) -> &Path {
        &self.path
    }

    pub(super) fn release(&mut self) -> eyre::Result<()> {
        validate_named_inrou_launch_barrier(&self.path, self.device, self.inode, self.child_gid)?;
        let mut options = fs::OpenOptions::new();
        options.write(true).custom_flags(
            (rustix::fs::OFlags::NONBLOCK
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::CLOEXEC)
                .bits() as i32,
        );
        let deadline = std::time::Instant::now() + INROU_CGROUP_BARRIER_RELEASE_TIMEOUT;
        let mut writer = loop {
            match options.open(&self.path) {
                Ok(writer) => break writer,
                Err(error)
                    if error.raw_os_error() == Some(rustix::io::Errno::NXIO.raw_os_error())
                        && std::time::Instant::now() < deadline =>
                {
                    // O_NONBLOCK returns ENXIO until the gated child has
                    // opened the FIFO for reading. Waiting for that reader is
                    // the acknowledgement that unlinking cannot race ahead
                    // of the child-side barrier check.
                    std::thread::sleep(Duration::from_millis(1));
                }
                Err(error) => {
                    return Err(error).wrap_err_with(|| {
                        format!(
                            "open acknowledged Inrou launch barrier {} within {:?}",
                            self.path.display(),
                            INROU_CGROUP_BARRIER_RELEASE_TIMEOUT
                        )
                    });
                }
            }
        };
        let opened = writer.metadata()?;
        if opened.dev() != self.device || opened.ino() != self.inode {
            eyre::bail!("Inrou launch barrier changed before release");
        }
        writer
            .write_all(INROU_CGROUP_BARRIER_TOKEN)
            .wrap_err("release Inrou cgroup launch barrier")?;
        drop(writer);
        fs::remove_file(&self.path)
            .wrap_err_with(|| format!("unlink released Inrou barrier {}", self.path.display()))?;
        self.active = false;
        Ok(())
    }
}

impl Drop for InrouLaunchBarrier {
    fn drop(&mut self) {
        if self.active
            && let Err(error) = fs::remove_file(&self.path)
            && error.kind() != io::ErrorKind::NotFound
        {
            iroha_logger::error!(
                ?error,
                barrier = %self.path.display(),
                "failed to remove an unreleased Inrou launch barrier"
            );
        }
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
    let guest_memory_bytes = resources
        .memory_bytes
        .get()
        .div_ceil(1024 * 1024)
        .max(128)
        .checked_mul(1024 * 1024)
        .ok_or_else(|| eyre::eyre!("Inrou guest-memory cgroup projection overflow"))?;
    let memory_max_bytes = guest_memory_bytes
        .checked_add(INROU_CGROUP_QEMU_MEMORY_OVERHEAD_BYTES)
        .ok_or_else(|| eyre::eyre!("Inrou memory cgroup projection overflow"))?;
    let pids_max = u64::from(resources.max_tasks.get())
        .checked_add(INROU_CGROUP_QEMU_PID_OVERHEAD)
        .ok_or_else(|| eyre::eyre!("Inrou pids cgroup projection overflow"))?;
    let cpu_millis = u64::from(resources.cpu_millis.get())
        .checked_add(INROU_CGROUP_QEMU_CPU_OVERHEAD_MILLIS)
        .ok_or_else(|| eyre::eyre!("Inrou CPU cgroup projection overflow"))?;
    let cpu_quota_micros = cpu_millis
        .checked_mul(INROU_CGROUP_CPU_PERIOD_MICROS)
        .ok_or_else(|| eyre::eyre!("Inrou CPU quota projection overflow"))?
        .div_ceil(1_000);
    let service_io_bytes_per_sec = resources
        .ephemeral_storage_bytes
        .get()
        .div_ceil(INROU_CGROUP_IO_PROJECTION_WINDOW_SECS);
    let io_bytes_per_sec = service_io_bytes_per_sec
        .checked_add(INROU_CGROUP_QEMU_IO_OVERHEAD_BYTES_PER_SEC)
        .ok_or_else(|| eyre::eyre!("Inrou IO-bandwidth cgroup projection overflow"))?;
    let service_iops = u64::from(resources.max_open_files.get())
        .checked_add(u64::from(resources.max_tasks.get()))
        .ok_or_else(|| eyre::eyre!("Inrou IOPS cgroup projection overflow"))?;
    let io_iops = service_iops
        .checked_add(INROU_CGROUP_QEMU_IOPS_OVERHEAD)
        .ok_or_else(|| eyre::eyre!("Inrou IOPS-overhead cgroup projection overflow"))?;
    Ok(InrouCgroupLimits {
        memory_max_bytes,
        pids_max,
        cpu_quota_micros,
        cpu_period_micros: INROU_CGROUP_CPU_PERIOD_MICROS,
        io_read_bytes_per_sec: io_bytes_per_sec,
        io_write_bytes_per_sec: io_bytes_per_sec,
        io_read_iops: io_iops,
        io_write_iops: io_iops,
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

fn validate_inrou_launch_barrier_parent(parent: &Path, child_gid: u32) -> eyre::Result<()> {
    let named = fs::symlink_metadata(parent)
        .wrap_err_with(|| format!("inspect Inrou launch-barrier parent {}", parent.display()))?;
    if named.file_type().is_symlink()
        || !named.is_dir()
        || named.uid() != 0
        || named.gid() != child_gid
        || named.mode() & 0o7777 != 0o710
    {
        eyre::bail!(
            "Inrou launch-barrier parent {} must retain exact root/dedicated-group 0710 custody",
            parent.display()
        );
    }
    Ok(())
}

fn validate_inrou_launch_barrier(
    path: &Path,
    reader: &fs::File,
    child_gid: u32,
) -> eyre::Result<()> {
    let held = reader.metadata()?;
    let named = fs::symlink_metadata(path)?;
    if named.file_type().is_symlink()
        || !named.file_type().is_fifo()
        || !held.file_type().is_fifo()
        || named.dev() != held.dev()
        || named.ino() != held.ino()
        || held.nlink() != 1
        || held.uid() != 0
        || held.gid() != child_gid
        || held.mode() & 0o7777 != 0o640
    {
        eyre::bail!(
            "Inrou launch barrier {} lost exact root/dedicated-group FIFO custody",
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

fn validate_named_inrou_launch_barrier(
    path: &Path,
    expected_device: u64,
    expected_inode: u64,
    child_gid: u32,
) -> eyre::Result<()> {
    let named = fs::symlink_metadata(path)?;
    if named.file_type().is_symlink()
        || !named.file_type().is_fifo()
        || named.dev() != expected_device
        || named.ino() != expected_inode
        || named.nlink() != 1
        || named.uid() != 0
        || named.gid() != child_gid
        || named.mode() & 0o7777 != 0o640
    {
        eyre::bail!(
            "Inrou launch barrier {} changed before child acknowledgement",
            path.display()
        );
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
    use std::io::Write as _;
    use std::num::{NonZeroU16, NonZeroU32, NonZeroU64};
    use std::os::unix::fs::OpenOptionsExt as _;
    use std::process::{Command, Stdio};

    use super::*;

    fn resources() -> SoraResourceLimitsV1 {
        SoraResourceLimitsV1 {
            cpu_millis: NonZeroU32::new(1_500).expect("nonzero"),
            memory_bytes: NonZeroU64::new(512 * 1024 * 1024).expect("nonzero"),
            ephemeral_storage_bytes: NonZeroU64::new(60 * 1024 * 1024).expect("nonzero"),
            max_open_files: NonZeroU32::new(256).expect("nonzero"),
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
    fn resource_projection_includes_only_explicit_bounded_qemu_overhead() -> eyre::Result<()> {
        let projected = project_inrou_cgroup_limits(&resources())?;
        assert_eq!(projected.memory_max_bytes, 768 * 1024 * 1024);
        assert_eq!(projected.pids_max, 48);
        assert_eq!(projected.cpu_quota_micros, 175_000);
        assert_eq!(projected.cpu_period_micros, 100_000);
        assert_eq!(projected.io_read_bytes_per_sec, 17 * 1024 * 1024);
        assert_eq!(projected.io_write_bytes_per_sec, 17 * 1024 * 1024);
        assert_eq!(projected.io_read_iops, 400);
        assert_eq!(projected.io_write_iops, 400);
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

    #[test]
    fn launch_barrier_script_checks_token_and_procfs_before_exec() {
        assert!(INROU_CGROUP_BARRIER_SCRIPT.contains("inrou-cgroup-go-v1"));
        assert!(INROU_CGROUP_BARRIER_SCRIPT.contains("/proc/self/cgroup"));
        assert!(INROU_CGROUP_BARRIER_SCRIPT.contains("exec \"$@\""));
        assert!(
            INROU_CGROUP_BARRIER_SCRIPT.find("/proc/self/cgroup")
                < INROU_CGROUP_BARRIER_SCRIPT.find("exec \"$@\"")
        );
    }

    #[test]
    fn unreleased_launch_barrier_owner_removes_the_created_fifo() -> eyre::Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let path = temp_dir.path().join(INROU_CGROUP_BARRIER_FILE);
        rustix::fs::mkfifoat(
            rustix::fs::CWD,
            &path,
            rustix::fs::Mode::from_raw_mode(0o600),
        )?;
        let metadata = fs::symlink_metadata(&path)?;
        let barrier = InrouLaunchBarrier {
            path: path.clone(),
            device: metadata.dev(),
            inode: metadata.ino(),
            child_gid: metadata.gid(),
            active: true,
        };

        drop(barrier);

        assert!(
            !path.exists(),
            "an unreleased barrier must not survive its owner"
        );
        Ok(())
    }

    #[test]
    fn launch_barrier_blocks_exec_and_rejects_cgroup_drift() -> eyre::Result<()> {
        use std::os::fd::AsRawFd as _;

        let proc_cgroup = fs::read_to_string("/proc/self/cgroup")?;
        let mut records = proc_cgroup.lines();
        let Some(record) = records.next() else {
            // This test exercises the unified-hierarchy shell gate. Hosts
            // still using cgroup-v1 are rejected by the dedicated parser test.
            return Ok(());
        };
        if records.next().is_some() {
            return Ok(());
        }
        let mut fields = record.split(':');
        if fields.next() != Some("0") || fields.next() != Some("") {
            return Ok(());
        }
        let Some(expected_path) = fields.next() else {
            return Ok(());
        };
        if fields.next().is_some() {
            return Ok(());
        }
        let temp_dir = tempfile::tempdir()?;
        let marker = temp_dir.path().join("executed");
        let inherited = fs::File::open("/dev/null")?;
        let inherited_fd = inherited.as_raw_fd().to_string();

        for (case, supplied_path, succeeds) in [
            ("exact", expected_path.to_owned(), true),
            ("drifted", format!("{expected_path}-wrong"), false),
        ] {
            let fifo = temp_dir.path().join(format!("{case}.fifo"));
            rustix::fs::mkfifoat(
                rustix::fs::CWD,
                &fifo,
                rustix::fs::Mode::from_raw_mode(0o600),
            )?;
            rustix::io::fcntl_setfd(&inherited, rustix::io::FdFlags::empty())?;
            let child = Command::new("/bin/sh")
                .arg("-c")
                .arg(INROU_CGROUP_BARRIER_SCRIPT)
                .arg("inrou-cgroup-barrier-test")
                .arg(&fifo)
                .arg(&supplied_path)
                .arg("/bin/sh")
                .arg("-c")
                .arg("[ ! -e \"/proc/self/fd/$2\" ] || exit 124; printf executed > \"$1\"")
                .arg("inrou-cgroup-payload")
                .arg(&marker)
                .arg(&inherited_fd)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn();
            rustix::io::fcntl_setfd(&inherited, rustix::io::FdFlags::CLOEXEC)?;
            let mut child = child?;
            std::thread::sleep(Duration::from_millis(25));
            assert!(
                !marker.exists(),
                "payload executed before the supervisor released its cgroup barrier"
            );
            let mut writer_options = fs::OpenOptions::new();
            writer_options.write(true).custom_flags(
                (rustix::fs::OFlags::NONBLOCK | rustix::fs::OFlags::NOFOLLOW).bits() as i32,
            );
            let deadline = std::time::Instant::now() + Duration::from_secs(2);
            let mut writer = loop {
                match writer_options.open(&fifo) {
                    Ok(writer) => break writer,
                    Err(error)
                        if error.raw_os_error() == Some(rustix::io::Errno::NXIO.raw_os_error()) =>
                    {
                        if let Some(status) = child.try_wait()? {
                            eyre::bail!(
                                "barrier child exited with {status} before opening its FIFO"
                            );
                        }
                        if std::time::Instant::now() >= deadline {
                            let _ = child.kill();
                            let _ = child.wait();
                            eyre::bail!("barrier child did not open its FIFO before the deadline");
                        }
                        std::thread::sleep(Duration::from_millis(1));
                    }
                    Err(error) => return Err(error.into()),
                }
            };
            writer.write_all(INROU_CGROUP_BARRIER_TOKEN)?;
            drop(writer);
            let status = child.wait()?;
            fs::remove_file(&fifo)?;
            assert_eq!(status.success(), succeeds);
            assert_eq!(marker.exists(), succeeds);
            if marker.exists() {
                fs::remove_file(&marker)?;
            }
        }
        Ok(())
    }
}
