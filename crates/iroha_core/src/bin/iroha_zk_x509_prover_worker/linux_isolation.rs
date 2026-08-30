//! Qualified Linux/aarch64 launcher for the zk-X.509 worker.
//!
//! This module is compiled only for the reviewed release target.  The public
//! process never handles certificate witness bytes: it authenticates the
//! request paths, stages the exact static worker image in a sealed executable
//! memfd, creates a bounded cgroup-v2 leaf, and starts one internal worker with
//! Landlock and a TSYNC seccomp filter installed before `execve`.

use super::*;
use core::ffi::{c_char, c_int, c_long, c_uint, c_void};
use std::{
    fs,
    io::{Cursor, ErrorKind, Read as _, Seek as _, SeekFrom, Write as _},
    os::{
        fd::{AsRawFd as _, RawFd},
        unix::{fs::MetadataExt as _, process::CommandExt as _},
    },
    process::{Command, ExitStatus, Stdio},
    sync::OnceLock,
    thread,
    time::{Duration, Instant},
};

pub(super) const INTERNAL_LAUNCH_ARGUMENT_V1: &str = "__iroha-zk-x509-isolated-v1";

const ISOLATION_PACKAGE_DOMAIN_V1: &[u8] =
    b"iroha.privacy.zk-x509.qualified-linux-launcher-package.v1";
const ISOLATION_POLICY_V1: &[u8] = b"target=aarch64-unknown-linux-gnu;kernel-min=6.3;static-elf=true;openat2=resolve-beneath+no-symlinks+no-magiclinks;executable-memfd=mfd-exec+seal-exec+seal-write+seal-grow+seal-shrink+seal-seal;attestation-memfd=mfd-noexec-seal+seal-exec+seal-write+seal-grow+seal-shrink+seal-seal;uid=nonzero-equal-real+effective+saved+fs;capabilities=effective+permitted+inheritable+ambient-zero;landlock-abi-min=3;seccomp-tsync=true;seccomp-future-syscalls=deny;seccomp-pidfd+privileged=deny;no-new-privs=true;dumpable=false;cgroup-v2=true;memory-max=12884901888;memory-swap-max=0;memory-oom-group=1;pids-max=6;cpu-max=max+period-100000;rlimit-as=34359738368;rlimit-core=0;fd-closure=stdio+64-72-bootstrap+stdio+one-data-runtime;wall-ms=300000";
const ATTESTATION_MAGIC_V1: &[u8; 4] = b"X5LI";
const ATTESTATION_VERSION_V1: u8 = 1;
// Keep the launcher contract away from both stdio and `std::process`'s
// internal exec-error pipe.  All source descriptors are first duplicated at
// or above 128, so populating this closed interval cannot clobber them.
const ATTESTATION_FD_V1: RawFd = 64;
const STATUS_FD_V1: RawFd = 65;
const SELF_CGROUP_FD_V1: RawFd = 66;
const MEMORY_MAX_FD_V1: RawFd = 67;
const MEMORY_SWAP_MAX_FD_V1: RawFd = 68;
const PIDS_MAX_FD_V1: RawFd = 69;
const EXECUTABLE_MEMFD_V1: RawFd = 70;
const CPU_MAX_FD_V1: RawFd = 71;
const MEMORY_OOM_GROUP_FD_V1: RawFd = 72;
const FIRST_SOURCE_FD_V1: RawFd = 128;
const MAX_LAUNCH_INPUT_BYTES_V1: usize = MAX_FRAME_BYTES + 32 + 4;
const MAX_WORKER_IMAGE_BYTES_V1: usize = 512 * 1024 * 1024;
const MEMORY_MAX_BYTES_V1: u64 = 12 * 1024 * 1024 * 1024;
const ADDRESS_SPACE_MAX_BYTES_V1: u64 = 32 * 1024 * 1024 * 1024;
const PIDS_MAX_V1: u64 = 6;
const WALL_TIME_V1: Duration = Duration::from_secs(300);

const LANDLOCK_CREATE_RULESET_VERSION: c_uint = 1;
const LANDLOCK_RULE_PATH_BENEATH: c_int = 1;
const LANDLOCK_ACCESS_FS_EXECUTE: u64 = 1 << 0;
const LANDLOCK_ACCESS_FS_WRITE_FILE: u64 = 1 << 1;
const LANDLOCK_ACCESS_FS_READ_FILE: u64 = 1 << 2;
const LANDLOCK_ACCESS_FS_READ_DIR: u64 = 1 << 3;
const LANDLOCK_ACCESS_FS_REMOVE_DIR: u64 = 1 << 4;
const LANDLOCK_ACCESS_FS_REMOVE_FILE: u64 = 1 << 5;
const LANDLOCK_ACCESS_FS_MAKE_CHAR: u64 = 1 << 6;
const LANDLOCK_ACCESS_FS_MAKE_DIR: u64 = 1 << 7;
const LANDLOCK_ACCESS_FS_MAKE_REG: u64 = 1 << 8;
const LANDLOCK_ACCESS_FS_MAKE_SOCK: u64 = 1 << 9;
const LANDLOCK_ACCESS_FS_MAKE_FIFO: u64 = 1 << 10;
const LANDLOCK_ACCESS_FS_MAKE_BLOCK: u64 = 1 << 11;
const LANDLOCK_ACCESS_FS_MAKE_SYM: u64 = 1 << 12;
const LANDLOCK_ACCESS_FS_REFER: u64 = 1 << 13;
const LANDLOCK_ACCESS_FS_TRUNCATE: u64 = 1 << 14;
const LANDLOCK_HANDLED_ACCESS_FS_V1: u64 = LANDLOCK_ACCESS_FS_EXECUTE
    | LANDLOCK_ACCESS_FS_WRITE_FILE
    | LANDLOCK_ACCESS_FS_READ_FILE
    | LANDLOCK_ACCESS_FS_READ_DIR
    | LANDLOCK_ACCESS_FS_REMOVE_DIR
    | LANDLOCK_ACCESS_FS_REMOVE_FILE
    | LANDLOCK_ACCESS_FS_MAKE_CHAR
    | LANDLOCK_ACCESS_FS_MAKE_DIR
    | LANDLOCK_ACCESS_FS_MAKE_REG
    | LANDLOCK_ACCESS_FS_MAKE_SOCK
    | LANDLOCK_ACCESS_FS_MAKE_FIFO
    | LANDLOCK_ACCESS_FS_MAKE_BLOCK
    | LANDLOCK_ACCESS_FS_MAKE_SYM
    | LANDLOCK_ACCESS_FS_REFER
    | LANDLOCK_ACCESS_FS_TRUNCATE;
const LANDLOCK_READ_FILE_ACCESS_V1: u64 = LANDLOCK_ACCESS_FS_READ_FILE;
const LANDLOCK_BUNDLE_DIRECTORY_ACCESS_V1: u64 = LANDLOCK_ACCESS_FS_READ_DIR
    | LANDLOCK_ACCESS_FS_WRITE_FILE
    | LANDLOCK_ACCESS_FS_REMOVE_FILE
    | LANDLOCK_ACCESS_FS_MAKE_REG
    | LANDLOCK_ACCESS_FS_REFER;

const SYS_SECCOMP: c_long = 277;
const SYS_LANDLOCK_CREATE_RULESET: c_long = 444;
const SYS_LANDLOCK_ADD_RULE: c_long = 445;
const SYS_LANDLOCK_RESTRICT_SELF: c_long = 446;
const SECCOMP_SET_MODE_FILTER: c_uint = 1;
const SECCOMP_FILTER_FLAG_TSYNC: c_uint = 1;
const PR_SET_NO_NEW_PRIVS: c_int = 38;
const PR_SET_DUMPABLE: c_int = 4;
const PR_GET_DUMPABLE: c_int = 3;
const PR_SET_PDEATHSIG: c_int = 1;
const PR_CAP_AMBIENT: c_int = 47;
const PR_CAP_AMBIENT_CLEAR_ALL: c_int = 4;
const SIGKILL: c_int = 9;
const F_GETFD: c_int = 1;
const F_SETFD: c_int = 2;
const FD_CLOEXEC: c_int = 1;
const O_RDONLY: c_int = 0;
const O_CLOEXEC: c_int = 0o2000000;
const RLIMIT_CORE: c_int = 4;
const RLIMIT_AS: c_int = 9;
const SYS_CLOSE_RANGE: c_long = 436;
const SYS_CAPSET: c_long = 91;
const LINUX_CAPABILITY_VERSION_3: u32 = 0x2008_0522;

const BPF_LD_W_ABS: u16 = 0x20;
const BPF_JMP_JEQ_K: u16 = 0x15;
const BPF_JMP_JGE_K: u16 = 0x35;
const BPF_JMP_JSET_K: u16 = 0x45;
const BPF_RET_K: u16 = 0x06;
const SECCOMP_DATA_NR_OFFSET: u32 = 0;
const SECCOMP_DATA_ARCH_OFFSET: u32 = 4;
const SECCOMP_DATA_ARG0_LOW_OFFSET: u32 = 16;
const AUDIT_ARCH_AARCH64: u32 = 0xc000_00b7;
const SECCOMP_RET_KILL_PROCESS: u32 = 0x8000_0000;
const SECCOMP_RET_ERRNO_EPERM: u32 = 0x0005_0001;
const SECCOMP_RET_ERRNO_ENOSYS: u32 = 0x0005_0026;
const SECCOMP_RET_ALLOW: u32 = 0x7fff_0000;
const SYS_CLONE: u32 = 220;
const SYS_CLONE3: u32 = 435;
const FIRST_UNREVIEWED_SYSCALL_V1: u32 = 451;
const CLONE_VM: u32 = 0x0000_0100;
const CLONE_THREAD: u32 = 0x0001_0000;
const CLONE_NEWTIME: u32 = 0x0000_0080;
const CLONE_NEWNS: u32 = 0x0002_0000;
const CLONE_NEWCGROUP: u32 = 0x0200_0000;
const CLONE_NEWUTS: u32 = 0x0400_0000;
const CLONE_NEWIPC: u32 = 0x0800_0000;
const CLONE_NEWUSER: u32 = 0x1000_0000;
const CLONE_NEWPID: u32 = 0x2000_0000;
const CLONE_NEWNET: u32 = 0x4000_0000;
const FORBIDDEN_CLONE_FLAGS_V1: u32 = CLONE_NEWTIME
    | CLONE_NEWNS
    | CLONE_NEWCGROUP
    | CLONE_NEWUTS
    | CLONE_NEWIPC
    | CLONE_NEWUSER
    | CLONE_NEWPID
    | CLONE_NEWNET;

// Exact aarch64 syscall numbers rejected by the v1 filter. `clone` is handled
// separately so Rayon may create bounded threads but not processes/namespaces.
const DENIED_SYSCALLS_V1: &[u32] = &[
    18, 30, 33, 39, 40, 41, 42, 51, 54, 55, 58, 60, 89, 91, 92, 97, 104, 105, 106, 112, 116, 117,
    118, 119, 122, 129, 130, 131, 140, 142, 143, 144, 145, 146, 147, 149, 151, 152, 159, 161, 162,
    170, 171, 198, 199, 200, 201, 202, 203, 204, 205, 206, 207, 208, 209, 210, 211, 212, 217, 218,
    219, 224, 225, 238, 239, 240, 241, 242, 261, 262, 263, 264, 265, 266, 268, 270, 271, 272, 273,
    274, 280, 282, 294, 424, 425, 426, 427, 428, 429, 430, 431, 432, 433, 434, 438, 440, 442, 443,
    447, 448,
];

#[repr(C)]
struct LandlockRulesetAttrV1 {
    handled_access_fs: u64,
}

#[repr(C)]
struct LandlockPathBeneathAttrV1 {
    allowed_access: u64,
    parent_fd: c_int,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct SockFilterV1 {
    code: u16,
    jt: u8,
    jf: u8,
    k: u32,
}

#[repr(C)]
struct SockFprogV1 {
    len: u16,
    filter: *const SockFilterV1,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct RlimitV1 {
    current: u64,
    maximum: u64,
}

#[repr(C)]
struct CapabilityHeaderV1 {
    version: u32,
    pid: c_int,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CapabilityDataV1 {
    effective: u32,
    permitted: u32,
    inheritable: u32,
}

#[allow(
    unsafe_code,
    reason = "qualified Linux isolation requires stable kernel/libc syscalls"
)]
unsafe extern "C" {
    fn syscall(number: c_long, ...) -> c_long;
    fn prctl(option: c_int, ...) -> c_int;
    fn setrlimit(resource: c_int, limit: *const RlimitV1) -> c_int;
    fn getrlimit(resource: c_int, limit: *mut RlimitV1) -> c_int;
    fn dup2(old_fd: c_int, new_fd: c_int) -> c_int;
    fn close(fd: c_int) -> c_int;
    fn open(path: *const c_char, flags: c_int, ...) -> c_int;
    fn fcntl(fd: c_int, command: c_int, ...) -> c_int;
    fn write(fd: c_int, buffer: *const c_void, count: usize) -> isize;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct IsolationIdentityV1 {
    pub(super) package_sha256: [u8; 32],
}

#[derive(Clone, Copy)]
struct LandlockRuleV1 {
    descriptor: RawFd,
    allowed_access: u64,
}

#[derive(Clone, Copy)]
struct PreExecDescriptorsV1 {
    executable: RawFd,
    attestation: RawFd,
    cgroup_procs: RawFd,
    memory_max: RawFd,
    memory_swap_max: RawFd,
    pids_max: RawFd,
    cpu_max: RawFd,
    memory_oom_group: RawFd,
}

struct CgroupLeaseV1 {
    parent: PathBuf,
    supervisor: PathBuf,
    worker: PathBuf,
    disable_controllers: Vec<String>,
    restored: bool,
}

impl CgroupLeaseV1 {
    fn create() -> Result<Self, WorkerFailure> {
        require_single_thread_v1()?;
        let relative = current_cgroup_relative_path_v1()?;
        let root = Path::new("/sys/fs/cgroup");
        let parent = root.join(relative.strip_prefix('/').unwrap_or(&relative));
        let parent = fs::canonicalize(&parent).map_err(|_| WorkerFailure::IsolationUnavailable)?;
        if !parent.starts_with(root)
            || !fs::symlink_metadata(&parent)
                .map_err(|_| WorkerFailure::IsolationUnavailable)?
                .is_dir()
        {
            return Err(WorkerFailure::IsolationUnavailable);
        }
        let own_pid = std::process::id();
        let processes = read_canonical_text_v1(&parent.join("cgroup.procs"), 4096)?;
        if processes.trim() != own_pid.to_string() {
            return Err(WorkerFailure::IsolationUnavailable);
        }
        let nonce = launcher_nonce_v1()?;
        let suffix = format!("{own_pid}.{}", hex::encode(&nonce[..8]));
        let supervisor = parent.join(format!("iroha-zk-x509-supervisor-v1.{suffix}"));
        let worker = parent.join(format!("iroha-zk-x509-worker-v1.{suffix}"));
        fs::create_dir(&supervisor).map_err(|_| WorkerFailure::IsolationUnavailable)?;
        let mut lease = Self {
            parent,
            supervisor,
            worker,
            disable_controllers: Vec::new(),
            restored: false,
        };
        if let Err(error) = lease.initialize() {
            let _ = lease.restore();
            return Err(error);
        }
        Ok(lease)
    }

    fn initialize(&mut self) -> Result<(), WorkerFailure> {
        write_exact_text_v1(
            &self.supervisor.join("cgroup.procs"),
            &std::process::id().to_string(),
        )?;
        let controllers = read_word_set_v1(&self.parent.join("cgroup.controllers"))?;
        if !["cpu", "memory", "pids"]
            .into_iter()
            .all(|name| controllers.contains(name))
        {
            return Err(WorkerFailure::IsolationUnavailable);
        }
        let enabled = read_word_set_allow_empty_v1(&self.parent.join("cgroup.subtree_control"))?;
        if ["cpu", "memory", "pids"]
            .into_iter()
            .any(|name| enabled.contains(name))
        {
            return Err(WorkerFailure::IsolationUnavailable);
        }
        self.disable_controllers = ["cpu", "memory", "pids"]
            .into_iter()
            .map(str::to_owned)
            .collect();
        let request = self
            .disable_controllers
            .iter()
            .map(|name| format!("+{name}"))
            .collect::<Vec<_>>()
            .join(" ");
        write_exact_text_v1(&self.parent.join("cgroup.subtree_control"), &request)?;
        fs::create_dir(&self.worker).map_err(|_| WorkerFailure::IsolationUnavailable)?;
        write_exact_text_v1(
            &self.worker.join("memory.max"),
            &MEMORY_MAX_BYTES_V1.to_string(),
        )?;
        write_exact_text_v1(&self.worker.join("memory.swap.max"), "0")?;
        write_exact_text_v1(&self.worker.join("memory.oom.group"), "1")?;
        write_exact_text_v1(&self.worker.join("pids.max"), &PIDS_MAX_V1.to_string())?;
        write_exact_text_v1(&self.worker.join("cpu.max"), "max 100000")?;
        if read_canonical_text_v1(&self.worker.join("memory.max"), 128)?.trim()
            != MEMORY_MAX_BYTES_V1.to_string()
            || read_canonical_text_v1(&self.worker.join("memory.swap.max"), 128)?.trim() != "0"
            || read_canonical_text_v1(&self.worker.join("memory.oom.group"), 128)?.trim() != "1"
            || read_canonical_text_v1(&self.worker.join("pids.max"), 128)?.trim()
                != PIDS_MAX_V1.to_string()
            || read_canonical_text_v1(&self.worker.join("cpu.max"), 128)?.trim() != "max 100000"
        {
            return Err(WorkerFailure::IsolationUnavailable);
        }
        Ok(())
    }

    fn relative_worker_path(&self) -> Result<String, WorkerFailure> {
        let relative = self
            .worker
            .strip_prefix("/sys/fs/cgroup")
            .map_err(|_| WorkerFailure::IsolationUnavailable)?;
        Ok(format!("/{}", relative.display()))
    }

    fn kill_worker(&self) {
        let _ = fs::write(self.worker.join("cgroup.kill"), b"1");
    }

    fn restore(&mut self) -> Result<(), WorkerFailure> {
        if self.restored {
            return Ok(());
        }
        self.kill_worker();
        if self
            .worker
            .try_exists()
            .map_err(|_| WorkerFailure::IsolationUnavailable)?
        {
            fs::remove_dir(&self.worker).map_err(|_| WorkerFailure::IsolationUnavailable)?;
        }
        if !self.disable_controllers.is_empty() {
            let request = self
                .disable_controllers
                .iter()
                .map(|name| format!("-{name}"))
                .collect::<Vec<_>>()
                .join(" ");
            write_exact_text_v1(&self.parent.join("cgroup.subtree_control"), &request)?;
            let enabled =
                read_word_set_allow_empty_v1(&self.parent.join("cgroup.subtree_control"))?;
            if self
                .disable_controllers
                .iter()
                .any(|name| enabled.contains(name))
            {
                return Err(WorkerFailure::IsolationUnavailable);
            }
        }
        write_exact_text_v1(
            &self.parent.join("cgroup.procs"),
            &std::process::id().to_string(),
        )?;
        let supervisor_processes = fs::read(self.supervisor.join("cgroup.procs"))
            .map_err(|_| WorkerFailure::IsolationUnavailable)?;
        if !supervisor_processes
            .iter()
            .all(|byte| byte.is_ascii_whitespace())
        {
            return Err(WorkerFailure::IsolationUnavailable);
        }
        fs::remove_dir(&self.supervisor).map_err(|_| WorkerFailure::IsolationUnavailable)?;
        let parent_processes = read_canonical_text_v1(&self.parent.join("cgroup.procs"), 4096)?;
        if parent_processes.trim() != std::process::id().to_string() {
            return Err(WorkerFailure::IsolationUnavailable);
        }
        self.restored = true;
        Ok(())
    }
}

impl Drop for CgroupLeaseV1 {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}

fn launcher_nonce_v1() -> Result<[u8; 32], WorkerFailure> {
    let mut nonce = [0_u8; 32];
    OsRng.fill_bytes(&mut nonce);
    if nonce == [0; 32] {
        return Err(WorkerFailure::IsolationUnavailable);
    }
    Ok(nonce)
}

fn require_single_thread_v1() -> Result<(), WorkerFailure> {
    let entries =
        fs::read_dir("/proc/self/task").map_err(|_| WorkerFailure::IsolationUnavailable)?;
    let mut count = 0_u8;
    for entry in entries {
        entry.map_err(|_| WorkerFailure::IsolationUnavailable)?;
        count = count
            .checked_add(1)
            .ok_or(WorkerFailure::IsolationUnavailable)?;
    }
    if count != 1 {
        return Err(WorkerFailure::IsolationUnavailable);
    }
    Ok(())
}

fn current_cgroup_relative_path_v1() -> Result<PathBuf, WorkerFailure> {
    let text = read_canonical_text_v1(Path::new("/proc/self/cgroup"), 4096)?;
    let mut lines = text.lines();
    let line = lines.next().ok_or(WorkerFailure::IsolationUnavailable)?;
    if lines.next().is_some() || !line.starts_with("0::/") {
        return Err(WorkerFailure::IsolationUnavailable);
    }
    let path = PathBuf::from(&line[3..]);
    if !path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
    {
        return Err(WorkerFailure::IsolationUnavailable);
    }
    Ok(path)
}

fn require_kernel_release_v1() -> Result<(), WorkerFailure> {
    let release = read_canonical_text_v1(Path::new("/proc/sys/kernel/osrelease"), 256)?;
    let mut components = release.trim().split('.');
    let major = components
        .next()
        .filter(|value| !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()))
        .and_then(|value| value.parse::<u32>().ok())
        .ok_or(WorkerFailure::IsolationUnavailable)?;
    let minor = components
        .next()
        .filter(|value| !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()))
        .and_then(|value| value.parse::<u32>().ok())
        .ok_or(WorkerFailure::IsolationUnavailable)?;
    if (major, minor) < (6, 3) {
        return Err(WorkerFailure::IsolationUnavailable);
    }
    Ok(())
}

fn status_fields_v1(status: &str) -> Option<std::collections::BTreeMap<&str, &str>> {
    let mut fields = std::collections::BTreeMap::new();
    for line in status.lines() {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        if fields.insert(name, value.trim()).is_some() {
            return None;
        }
    }
    Some(fields)
}

fn status_has_unprivileged_identity_v1(status: &str) -> bool {
    let Some(fields) = status_fields_v1(status) else {
        return false;
    };
    let Some(uid) = fields.get("Uid") else {
        return false;
    };
    let values = uid
        .split_ascii_whitespace()
        .map(str::parse::<u32>)
        .collect::<Result<Vec<_>, _>>();
    let Ok(values) = values else {
        return false;
    };
    values.len() == 4
        && values[0] != 0
        && values.iter().all(|value| *value == values[0])
        && ["CapEff", "CapPrm", "CapInh", "CapAmb"]
            .into_iter()
            .all(|name| fields.get(name) == Some(&"0000000000000000"))
}

fn require_unprivileged_launcher_v1() -> Result<(), WorkerFailure> {
    let status = read_canonical_text_v1(Path::new("/proc/self/status"), 64 * 1024)?;
    if !status_has_unprivileged_identity_v1(&status) {
        return Err(WorkerFailure::IsolationUnavailable);
    }
    Ok(())
}

fn read_canonical_text_v1(path: &Path, maximum: usize) -> Result<String, WorkerFailure> {
    let bytes = fs::read(path).map_err(|_| WorkerFailure::IsolationUnavailable)?;
    if bytes.is_empty() || bytes.len() > maximum || bytes.contains(&0) {
        return Err(WorkerFailure::IsolationUnavailable);
    }
    String::from_utf8(bytes).map_err(|_| WorkerFailure::IsolationUnavailable)
}

fn read_word_set_v1(path: &Path) -> Result<std::collections::BTreeSet<String>, WorkerFailure> {
    let text = read_canonical_text_v1(path, 4096)?;
    let words = text
        .split_ascii_whitespace()
        .map(ToOwned::to_owned)
        .collect::<std::collections::BTreeSet<_>>();
    if words.is_empty() {
        return Err(WorkerFailure::IsolationUnavailable);
    }
    Ok(words)
}

fn read_word_set_allow_empty_v1(
    path: &Path,
) -> Result<std::collections::BTreeSet<String>, WorkerFailure> {
    let bytes = fs::read(path).map_err(|_| WorkerFailure::IsolationUnavailable)?;
    if bytes.len() > 4096 || bytes.contains(&0) {
        return Err(WorkerFailure::IsolationUnavailable);
    }
    let text = String::from_utf8(bytes).map_err(|_| WorkerFailure::IsolationUnavailable)?;
    Ok(text
        .split_ascii_whitespace()
        .map(ToOwned::to_owned)
        .collect())
}

fn write_exact_text_v1(path: &Path, value: &str) -> Result<(), WorkerFailure> {
    fs::write(path, value.as_bytes()).map_err(|_| WorkerFailure::IsolationUnavailable)
}

fn sha256_reader_v1(
    reader: &mut impl Read,
    maximum: usize,
) -> Result<([u8; 32], Vec<u8>), WorkerFailure> {
    let mut bytes = Vec::new();
    reader
        .take(u64::try_from(maximum).map_err(|_| WorkerFailure::IsolationUnavailable)? + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| WorkerFailure::IsolationUnavailable)?;
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(WorkerFailure::IsolationUnavailable);
    }
    Ok((sha256(&bytes), bytes))
}

fn validate_static_aarch64_elf_v1(bytes: &[u8]) -> Result<(), WorkerFailure> {
    if bytes.len() < 64
        || bytes.get(..4) != Some(b"\x7fELF")
        || bytes[4] != 2
        || bytes[5] != 1
        || u16::from_le_bytes([bytes[18], bytes[19]]) != 183
    {
        return Err(WorkerFailure::IsolationUnavailable);
    }
    let program_offset = u64::from_le_bytes(
        bytes[32..40]
            .try_into()
            .map_err(|_| WorkerFailure::IsolationUnavailable)?,
    );
    let entry_bytes = u64::from(u16::from_le_bytes([bytes[54], bytes[55]]));
    let entry_count = u64::from(u16::from_le_bytes([bytes[56], bytes[57]]));
    if entry_bytes < 56 || entry_count == 0 || entry_count > 1024 {
        return Err(WorkerFailure::IsolationUnavailable);
    }
    let table_end = program_offset
        .checked_add(
            entry_bytes
                .checked_mul(entry_count)
                .ok_or(WorkerFailure::IsolationUnavailable)?,
        )
        .ok_or(WorkerFailure::IsolationUnavailable)?;
    if program_offset < 64
        || table_end
            > u64::try_from(bytes.len()).map_err(|_| WorkerFailure::IsolationUnavailable)?
    {
        return Err(WorkerFailure::IsolationUnavailable);
    }
    for index in 0..entry_count {
        let offset = program_offset
            .checked_add(
                index
                    .checked_mul(entry_bytes)
                    .ok_or(WorkerFailure::IsolationUnavailable)?,
            )
            .ok_or(WorkerFailure::IsolationUnavailable)?;
        let offset = usize::try_from(offset).map_err(|_| WorkerFailure::IsolationUnavailable)?;
        let end = offset
            .checked_add(4)
            .ok_or(WorkerFailure::IsolationUnavailable)?;
        let kind = u32::from_le_bytes(
            bytes
                .get(offset..end)
                .ok_or(WorkerFailure::IsolationUnavailable)?
                .try_into()
                .map_err(|_| WorkerFailure::IsolationUnavailable)?,
        );
        if kind == 3 {
            return Err(WorkerFailure::IsolationUnavailable);
        }
    }
    Ok(())
}

fn stage_executable_memfd_v1() -> Result<(File, [u8; 32]), WorkerFailure> {
    let executable = env::current_exe().map_err(|_| WorkerFailure::IsolationUnavailable)?;
    let mut source = File::open(&executable).map_err(|_| WorkerFailure::IsolationUnavailable)?;
    let metadata = source
        .metadata()
        .map_err(|_| WorkerFailure::IsolationUnavailable)?;
    if !metadata.is_file()
        || metadata.uid() != rustix::process::geteuid().as_raw()
        || metadata.mode() & 0o022 != 0
    {
        return Err(WorkerFailure::IsolationUnavailable);
    }
    let (digest, bytes) = sha256_reader_v1(&mut source, MAX_WORKER_IMAGE_BYTES_V1)?;
    validate_static_aarch64_elf_v1(&bytes)?;
    let descriptor = rustix::fs::memfd_create(
        "iroha-zk-x509-worker-v1",
        rustix::fs::MemfdFlags::CLOEXEC
            | rustix::fs::MemfdFlags::ALLOW_SEALING
            | rustix::fs::MemfdFlags::EXEC,
    )
    .map_err(|_| WorkerFailure::IsolationUnavailable)?;
    let mut staged = File::from(descriptor);
    staged
        .write_all(&bytes)
        .and_then(|()| staged.flush())
        .map_err(|_| WorkerFailure::IsolationUnavailable)?;
    staged
        .seek(SeekFrom::Start(0))
        .map_err(|_| WorkerFailure::IsolationUnavailable)?;
    rustix::fs::fcntl_add_seals(
        &staged,
        rustix::fs::SealFlags::WRITE
            | rustix::fs::SealFlags::GROW
            | rustix::fs::SealFlags::SHRINK
            | rustix::fs::SealFlags::EXEC
            | rustix::fs::SealFlags::SEAL,
    )
    .map_err(|_| WorkerFailure::IsolationUnavailable)?;
    let seals =
        rustix::fs::fcntl_get_seals(&staged).map_err(|_| WorkerFailure::IsolationUnavailable)?;
    let required = rustix::fs::SealFlags::WRITE
        | rustix::fs::SealFlags::GROW
        | rustix::fs::SealFlags::SHRINK
        | rustix::fs::SealFlags::EXEC
        | rustix::fs::SealFlags::SEAL;
    if !seals.contains(required) {
        return Err(WorkerFailure::IsolationUnavailable);
    }
    Ok((staged, digest))
}

fn isolation_package_sha256_v1(artifact_sha256: [u8; 32]) -> [u8; 32] {
    let policy_sha256 = sha256(ISOLATION_POLICY_V1);
    let mut digest = Sha256::new();
    digest.update(ISOLATION_PACKAGE_DOMAIN_V1);
    digest.update(artifact_sha256);
    digest.update(policy_sha256);
    digest.finalize().into()
}

fn attestation_bytes_v1(
    artifact_sha256: [u8; 32],
    cgroup_path: &str,
    nonce: [u8; 32],
) -> Result<Vec<u8>, WorkerFailure> {
    let cgroup = cgroup_path.as_bytes();
    let cgroup_length =
        u16::try_from(cgroup.len()).map_err(|_| WorkerFailure::IsolationUnavailable)?;
    if cgroup.is_empty() || cgroup.contains(&0) {
        return Err(WorkerFailure::IsolationUnavailable);
    }
    let mut bytes = Vec::with_capacity(4 + 1 + 32 * 4 + 2 + cgroup.len());
    bytes.extend_from_slice(ATTESTATION_MAGIC_V1);
    bytes.push(ATTESTATION_VERSION_V1);
    bytes.extend_from_slice(&sha256(ISOLATION_POLICY_V1));
    bytes.extend_from_slice(&artifact_sha256);
    bytes.extend_from_slice(&isolation_package_sha256_v1(artifact_sha256));
    bytes.extend_from_slice(&nonce);
    bytes.extend_from_slice(&cgroup_length.to_be_bytes());
    bytes.extend_from_slice(cgroup);
    Ok(bytes)
}

fn sealed_attestation_memfd_v1(bytes: &[u8]) -> Result<File, WorkerFailure> {
    let descriptor = rustix::fs::memfd_create(
        "iroha-zk-x509-isolation-attestation-v1",
        rustix::fs::MemfdFlags::CLOEXEC
            | rustix::fs::MemfdFlags::ALLOW_SEALING
            | rustix::fs::MemfdFlags::NOEXEC_SEAL,
    )
    .map_err(|_| WorkerFailure::IsolationUnavailable)?;
    let mut file = File::from(descriptor);
    file.write_all(bytes)
        .and_then(|()| file.flush())
        .and_then(|()| file.seek(SeekFrom::Start(0)).map(|_| ()))
        .map_err(|_| WorkerFailure::IsolationUnavailable)?;
    rustix::fs::fcntl_add_seals(
        &file,
        rustix::fs::SealFlags::WRITE
            | rustix::fs::SealFlags::GROW
            | rustix::fs::SealFlags::SHRINK
            | rustix::fs::SealFlags::EXEC
            | rustix::fs::SealFlags::SEAL,
    )
    .map_err(|_| WorkerFailure::IsolationUnavailable)?;
    Ok(file)
}

fn duplicate_high_v1(file: &File) -> Result<File, WorkerFailure> {
    rustix::io::fcntl_dupfd_cloexec(file, FIRST_SOURCE_FD_V1)
        .map(File::from)
        .map_err(|_| WorkerFailure::IsolationUnavailable)
}

fn request_landlock_paths_v1(
    args: &[String],
    input: &[u8],
) -> Result<(Vec<PathBuf>, Option<PathBuf>), WorkerFailure> {
    if args.is_empty() {
        let mut reader = Cursor::new(input);
        let mut key = Zeroizing::new([0_u8; 32]);
        reader
            .read_exact(&mut key[..])
            .map_err(|_| WorkerFailure::Request)?;
        let frame = read_request_frame(&mut reader, &key)?;
        if reader.position()
            != u64::try_from(input.len()).map_err(|_| WorkerFailure::IsolationUnavailable)?
        {
            return Err(WorkerFailure::Request);
        }
        if frame.command == COMMAND_IDENTITY && frame.payload.is_empty() {
            return Ok((Vec::new(), None));
        }
        if frame.command != COMMAND_EXECUTE && frame.command != COMMAND_ADMIT_BUNDLE {
            return Err(WorkerFailure::Request);
        }
        let request = canonical_execute_request(&frame.payload)?;
        return Ok((
            vec![
                validate_absolute_path(&request.public_request_path)?,
                validate_absolute_path(&request.secret_bundle_path)?,
            ],
            None,
        ));
    }
    if args.len() != 5 || args[0] != "bundle" {
        return Err(WorkerFailure::Request);
    }
    let output = validate_absolute_path(&args[4])?;
    let output_parent = output
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .ok_or(WorkerFailure::Request)?
        .to_path_buf();
    Ok((
        vec![
            validate_absolute_path(&args[1])?,
            validate_absolute_path(&args[2])?,
            validate_absolute_path(&args[3])?,
        ],
        Some(output_parent),
    ))
}

fn open_landlock_rule_v1(
    path: &Path,
    allowed_access: u64,
) -> Result<(File, LandlockRuleV1), WorkerFailure> {
    let canonical = fs::canonicalize(path).map_err(|_| WorkerFailure::IsolationUnavailable)?;
    if canonical != path {
        return Err(WorkerFailure::IsolationUnavailable);
    }
    let root = File::open("/").map_err(|_| WorkerFailure::IsolationUnavailable)?;
    let relative = path
        .strip_prefix('/')
        .map_err(|_| WorkerFailure::IsolationUnavailable)?;
    let descriptor = rustix::fs::openat2(
        &root,
        relative,
        rustix::fs::OFlags::PATH | rustix::fs::OFlags::CLOEXEC | rustix::fs::OFlags::NOFOLLOW,
        rustix::fs::Mode::empty(),
        rustix::fs::ResolveFlags::BENEATH
            | rustix::fs::ResolveFlags::NO_SYMLINKS
            | rustix::fs::ResolveFlags::NO_MAGICLINKS,
    )
    .map_err(|_| WorkerFailure::IsolationUnavailable)?;
    let opened = File::from(descriptor);
    let file = duplicate_high_v1(&opened)?;
    let rule = LandlockRuleV1 {
        descriptor: file.as_raw_fd(),
        allowed_access,
    };
    Ok((file, rule))
}

fn open_evidence_file_v1(path: &Path) -> Result<File, WorkerFailure> {
    OpenOptions::new()
        .read(true)
        .open(path)
        .map_err(|_| WorkerFailure::IsolationUnavailable)
}

fn read_launch_input_v1(args: &[String]) -> Result<Zeroizing<Vec<u8>>, WorkerFailure> {
    if !args.is_empty() {
        return Ok(Zeroizing::new(Vec::new()));
    }
    let mut bytes = Zeroizing::new(Vec::new());
    io::stdin()
        .lock()
        .take(
            u64::try_from(MAX_LAUNCH_INPUT_BYTES_V1)
                .map_err(|_| WorkerFailure::IsolationUnavailable)?
                + 1,
        )
        .read_to_end(&mut bytes)
        .map_err(|_| WorkerFailure::Request)?;
    if bytes.len() > MAX_LAUNCH_INPUT_BYTES_V1 {
        return Err(WorkerFailure::Request);
    }
    Ok(bytes)
}

pub(super) fn launch_v1(args: Vec<String>) -> Result<ExitStatus, WorkerFailure> {
    require_kernel_release_v1()?;
    require_unprivileged_launcher_v1()?;
    let server_mode = args.is_empty();
    let input = read_launch_input_v1(&args)?;
    let (read_paths, writable_directory) = request_landlock_paths_v1(&args, &input)?;
    let (executable, artifact_sha256) = stage_executable_memfd_v1()?;
    let executable = duplicate_high_v1(&executable)?;
    let mut cgroup = CgroupLeaseV1::create()?;
    let cgroup_path = cgroup.relative_worker_path()?;
    let attestation = sealed_attestation_memfd_v1(&attestation_bytes_v1(
        artifact_sha256,
        &cgroup_path,
        launcher_nonce_v1()?,
    )?)?;
    let attestation = duplicate_high_v1(&attestation)?;

    let mut rule_files = Vec::new();
    let mut rules = Vec::new();
    for path in &read_paths {
        let (file, rule) = open_landlock_rule_v1(path, LANDLOCK_READ_FILE_ACCESS_V1)?;
        rule_files.push(file);
        rules.push(rule);
    }
    if let Some(path) = writable_directory.as_deref() {
        let (file, rule) = open_landlock_rule_v1(path, LANDLOCK_BUNDLE_DIRECTORY_ACCESS_V1)?;
        rule_files.push(file);
        rules.push(rule);
    }
    let cgroup_procs = OpenOptions::new()
        .write(true)
        .open(cgroup.worker.join("cgroup.procs"))
        .map_err(|_| WorkerFailure::IsolationUnavailable)?;
    let cgroup_procs = duplicate_high_v1(&cgroup_procs)?;
    let memory_max = duplicate_high_v1(&open_evidence_file_v1(&cgroup.worker.join("memory.max"))?)?;
    let memory_swap_max = duplicate_high_v1(&open_evidence_file_v1(
        &cgroup.worker.join("memory.swap.max"),
    )?)?;
    let pids_max = duplicate_high_v1(&open_evidence_file_v1(&cgroup.worker.join("pids.max"))?)?;
    let cpu_max = duplicate_high_v1(&open_evidence_file_v1(&cgroup.worker.join("cpu.max"))?)?;
    let memory_oom_group = duplicate_high_v1(&open_evidence_file_v1(
        &cgroup.worker.join("memory.oom.group"),
    )?)?;
    let bootstrap_seccomp =
        seccomp_program_v1(false).map_err(|_| WorkerFailure::IsolationUnavailable)?;

    let descriptors = PreExecDescriptorsV1 {
        executable: executable.as_raw_fd(),
        attestation: attestation.as_raw_fd(),
        cgroup_procs: cgroup_procs.as_raw_fd(),
        memory_max: memory_max.as_raw_fd(),
        memory_swap_max: memory_swap_max.as_raw_fd(),
        pids_max: pids_max.as_raw_fd(),
        cpu_max: cpu_max.as_raw_fd(),
        memory_oom_group: memory_oom_group.as_raw_fd(),
    };
    let mut command = Command::new(format!("/proc/self/fd/{EXECUTABLE_MEMFD_V1}"));
    command
        .arg(INTERNAL_LAUNCH_ARGUMENT_V1)
        .args(&args)
        .current_dir("/")
        .env_clear()
        .stdin(if server_mode {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    #[allow(
        unsafe_code,
        reason = "all child isolation must be installed atomically before exec"
    )]
    unsafe {
        command.pre_exec(move || {
            // Keep the O_PATH descriptors alive until every Landlock rule is
            // installed in this child.
            let _rule_files = &rule_files;
            pre_exec_isolation_v1(descriptors, &rules, &bootstrap_seccomp)
        });
    }
    let child = command
        .spawn()
        .map_err(|_| WorkerFailure::IsolationUnavailable)?;
    drop(command);
    supervise_child_v1(child, input, server_mode, &mut cgroup)
}

fn supervise_child_v1(
    mut child: std::process::Child,
    input: Zeroizing<Vec<u8>>,
    server_mode: bool,
    cgroup: &mut CgroupLeaseV1,
) -> Result<ExitStatus, WorkerFailure> {
    let deadline = Instant::now() + WALL_TIME_V1;
    let writer = child
        .stdin
        .take()
        .map(|mut stdin| {
            thread::Builder::new()
                .name("zk-x509-input-v1".to_owned())
                .spawn(move || stdin.write_all(&input).and_then(|()| stdin.flush()))
        })
        .transpose()
        .map_err(|_| WorkerFailure::IsolationUnavailable)?;
    let mut child_stdout = child
        .stdout
        .take()
        .ok_or(WorkerFailure::IsolationUnavailable)?;
    let maximum_output = if server_mode { MAX_FRAME_BYTES + 4 } else { 32 };
    let reader = thread::Builder::new()
        .name("zk-x509-output-v1".to_owned())
        .spawn(move || {
            let mut output = Vec::new();
            child_stdout
                .take(
                    u64::try_from(maximum_output).map_err(|_| {
                        io::Error::new(ErrorKind::InvalidData, "output cap is invalid")
                    })? + 1,
                )
                .read_to_end(&mut output)?;
            if output.len() > maximum_output {
                return Err(io::Error::new(
                    ErrorKind::InvalidData,
                    "worker output exceeds its protocol cap",
                ));
            }
            Ok(output)
        })
        .map_err(|_| WorkerFailure::IsolationUnavailable)?;
    let status = loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|_| WorkerFailure::IsolationUnavailable)?
        {
            break status;
        }
        if Instant::now() >= deadline {
            cgroup.kill_worker();
            let _ = child.kill();
            let _ = child.wait();
            if let Some(writer) = writer {
                let _ = writer.join();
            }
            let _ = reader.join();
            return Err(WorkerFailure::IsolationUnavailable);
        }
        thread::sleep(Duration::from_millis(5));
    };
    if let Some(writer) = writer {
        writer
            .join()
            .map_err(|_| WorkerFailure::IsolationUnavailable)?
            .map_err(|_| WorkerFailure::IsolationUnavailable)?;
    }
    let output = reader
        .join()
        .map_err(|_| WorkerFailure::IsolationUnavailable)?
        .map_err(|_| WorkerFailure::IsolationUnavailable)?;
    if status.success() {
        if (server_mode && output.len() < 4) || (!server_mode && output.len() != 32) {
            return Err(WorkerFailure::IsolationUnavailable);
        }
    } else if !output.is_empty() {
        return Err(WorkerFailure::IsolationUnavailable);
    }
    cgroup.restore()?;
    if status.success() {
        let mut stdout = io::stdout().lock();
        stdout
            .write_all(&output)
            .and_then(|()| stdout.flush())
            .map_err(|_| WorkerFailure::IsolationUnavailable)?;
    }
    Ok(status)
}

#[allow(
    unsafe_code,
    reason = "pre-exec isolation uses audited Linux syscalls only"
)]
unsafe fn pre_exec_isolation_v1(
    descriptors: PreExecDescriptorsV1,
    rules: &[LandlockRuleV1],
    bootstrap_seccomp: &[SockFilterV1],
) -> io::Result<()> {
    unsafe {
        write_all_raw_v1(descriptors.cgroup_procs, b"0")?;
        dup_inherited_fd_v1(descriptors.attestation, ATTESTATION_FD_V1)?;
        dup_inherited_fd_v1(descriptors.executable, EXECUTABLE_MEMFD_V1)?;
        dup_inherited_fd_v1(descriptors.memory_max, MEMORY_MAX_FD_V1)?;
        dup_inherited_fd_v1(descriptors.memory_swap_max, MEMORY_SWAP_MAX_FD_V1)?;
        dup_inherited_fd_v1(descriptors.pids_max, PIDS_MAX_FD_V1)?;
        dup_inherited_fd_v1(descriptors.cpu_max, CPU_MAX_FD_V1)?;
        dup_inherited_fd_v1(descriptors.memory_oom_group, MEMORY_OOM_GROUP_FD_V1)?;
        let status = open_c_path_v1(b"/proc/self/status\0")?;
        dup_inherited_fd_v1(status, STATUS_FD_V1)?;
        if status != STATUS_FD_V1 {
            close(status);
        }
        let cgroup = open_c_path_v1(b"/proc/self/cgroup\0")?;
        dup_inherited_fd_v1(cgroup, SELF_CGROUP_FD_V1)?;
        if cgroup != SELF_CGROUP_FD_V1 {
            close(cgroup);
        }
        if prctl(PR_SET_PDEATHSIG, SIGKILL, 0, 0, 0) != 0
            || prctl(PR_SET_DUMPABLE, 0, 0, 0, 0) != 0
            || setrlimit(
                RLIMIT_CORE,
                &RlimitV1 {
                    current: 0,
                    maximum: 0,
                },
            ) != 0
            || setrlimit(
                RLIMIT_AS,
                &RlimitV1 {
                    current: ADDRESS_SPACE_MAX_BYTES_V1,
                    maximum: ADDRESS_SPACE_MAX_BYTES_V1,
                },
            ) != 0
        {
            return Err(io::Error::last_os_error());
        }
        drop_process_capabilities_v1()?;
        if prctl(PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) != 0 {
            return Err(io::Error::last_os_error());
        }
        install_landlock_v1(rules)?;
        install_seccomp_program_v1(bootstrap_seccomp)?;
        close_unrelated_fds_v1()?;
    }
    Ok(())
}

#[allow(
    unsafe_code,
    reason = "the qualified child must clear every Linux capability set before exec"
)]
unsafe fn drop_process_capabilities_v1() -> io::Result<()> {
    unsafe {
        if prctl(PR_CAP_AMBIENT, PR_CAP_AMBIENT_CLEAR_ALL, 0, 0, 0) != 0 {
            return Err(io::Error::last_os_error());
        }
        let header = CapabilityHeaderV1 {
            version: LINUX_CAPABILITY_VERSION_3,
            pid: 0,
        };
        let empty = [CapabilityDataV1 {
            effective: 0,
            permitted: 0,
            inheritable: 0,
        }; 2];
        if syscall(SYS_CAPSET, &raw const header, empty.as_ptr()) != 0 {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}

#[allow(
    unsafe_code,
    reason = "fixed inherited descriptors must survive exec without CLOEXEC"
)]
unsafe fn dup_inherited_fd_v1(source: RawFd, destination: RawFd) -> io::Result<()> {
    unsafe {
        if source != destination && dup2(source, destination) < 0 {
            return Err(io::Error::last_os_error());
        }
        let flags = fcntl(destination, F_GETFD);
        if flags < 0 || fcntl(destination, F_SETFD, flags & !FD_CLOEXEC) < 0 {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}

#[allow(
    unsafe_code,
    reason = "pre-exec code cannot allocate through std file APIs"
)]
unsafe fn open_c_path_v1(path: &'static [u8]) -> io::Result<RawFd> {
    let pointer = path.as_ptr().cast::<c_char>();
    let descriptor = unsafe { open(pointer, O_RDONLY | O_CLOEXEC) };
    if descriptor < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(descriptor)
}

#[allow(
    unsafe_code,
    reason = "cgroup membership is written in the pre-exec child"
)]
unsafe fn write_all_raw_v1(descriptor: RawFd, bytes: &[u8]) -> io::Result<()> {
    let mut offset = 0;
    while offset < bytes.len() {
        let written = unsafe {
            write(
                descriptor,
                bytes[offset..].as_ptr().cast::<c_void>(),
                bytes.len() - offset,
            )
        };
        if written <= 0 {
            return Err(io::Error::last_os_error());
        }
        offset += usize::try_from(written)
            .map_err(|_| io::Error::new(ErrorKind::InvalidData, "write length is invalid"))?;
    }
    Ok(())
}

#[allow(unsafe_code, reason = "Landlock has no stable libc wrapper")]
unsafe fn install_landlock_v1(rules: &[LandlockRuleV1]) -> io::Result<()> {
    let abi = unsafe {
        syscall(
            SYS_LANDLOCK_CREATE_RULESET,
            core::ptr::null::<c_void>(),
            0_usize,
            LANDLOCK_CREATE_RULESET_VERSION,
        )
    };
    if abi < 3 {
        return Err(io::Error::last_os_error());
    }
    let attribute = LandlockRulesetAttrV1 {
        handled_access_fs: LANDLOCK_HANDLED_ACCESS_FS_V1,
    };
    let ruleset = unsafe {
        syscall(
            SYS_LANDLOCK_CREATE_RULESET,
            &raw const attribute,
            core::mem::size_of::<LandlockRulesetAttrV1>(),
            0_u32,
        )
    };
    if ruleset < 0 {
        return Err(io::Error::last_os_error());
    }
    let ruleset = RawFd::try_from(ruleset)
        .map_err(|_| io::Error::new(ErrorKind::InvalidData, "Landlock fd is invalid"))?;
    for rule in rules {
        let attribute = LandlockPathBeneathAttrV1 {
            allowed_access: rule.allowed_access,
            parent_fd: rule.descriptor,
        };
        let result = unsafe {
            syscall(
                SYS_LANDLOCK_ADD_RULE,
                ruleset,
                LANDLOCK_RULE_PATH_BENEATH,
                &raw const attribute,
                0_u32,
            )
        };
        if result != 0 {
            unsafe {
                close(ruleset);
            }
            return Err(io::Error::last_os_error());
        }
    }
    let result = unsafe { syscall(SYS_LANDLOCK_RESTRICT_SELF, ruleset, 0_u32) };
    unsafe {
        close(ruleset);
    }
    if result != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

const fn statement_v1(code: u16, value: u32) -> SockFilterV1 {
    SockFilterV1 {
        code,
        jt: 0,
        jf: 0,
        k: value,
    }
}

const fn jump_v1(code: u16, value: u32, yes: u8, no: u8) -> SockFilterV1 {
    SockFilterV1 {
        code,
        jt: yes,
        jf: no,
        k: value,
    }
}

fn seccomp_program_v1(terminal: bool) -> Result<Vec<SockFilterV1>, io::Error> {
    let mut program = Vec::with_capacity(24 + DENIED_SYSCALLS_V1.len() * 2);
    program.push(statement_v1(BPF_LD_W_ABS, SECCOMP_DATA_ARCH_OFFSET));
    program.push(jump_v1(BPF_JMP_JEQ_K, AUDIT_ARCH_AARCH64, 1, 0));
    program.push(statement_v1(BPF_RET_K, SECCOMP_RET_KILL_PROCESS));
    program.push(statement_v1(BPF_LD_W_ABS, SECCOMP_DATA_NR_OFFSET));
    program.push(jump_v1(BPF_JMP_JEQ_K, SYS_CLONE, 0, 6));
    program.push(statement_v1(BPF_LD_W_ABS, SECCOMP_DATA_ARG0_LOW_OFFSET));
    program.push(jump_v1(BPF_JMP_JSET_K, CLONE_VM, 0, 3));
    program.push(jump_v1(BPF_JMP_JSET_K, CLONE_THREAD, 0, 2));
    program.push(jump_v1(BPF_JMP_JSET_K, FORBIDDEN_CLONE_FLAGS_V1, 1, 0));
    program.push(statement_v1(BPF_RET_K, SECCOMP_RET_ALLOW));
    program.push(statement_v1(BPF_RET_K, SECCOMP_RET_ERRNO_EPERM));
    // glibc's static pthread implementation falls back to the inspectable
    // `clone` ABI only when `clone3` reports ENOSYS.  The fallback is then
    // restricted above to same-process, non-namespace worker threads.
    program.push(jump_v1(BPF_JMP_JEQ_K, SYS_CLONE3, 0, 1));
    program.push(statement_v1(BPF_RET_K, SECCOMP_RET_ERRNO_ENOSYS));
    program.push(jump_v1(BPF_JMP_JGE_K, FIRST_UNREVIEWED_SYSCALL_V1, 0, 1));
    program.push(statement_v1(BPF_RET_K, SECCOMP_RET_ERRNO_EPERM));
    for syscall_number in DENIED_SYSCALLS_V1 {
        program.push(jump_v1(BPF_JMP_JEQ_K, *syscall_number, 0, 1));
        program.push(statement_v1(BPF_RET_K, SECCOMP_RET_ERRNO_EPERM));
    }
    if terminal {
        for syscall_number in [221_u32, 281_u32] {
            program.push(jump_v1(BPF_JMP_JEQ_K, syscall_number, 0, 1));
            program.push(statement_v1(BPF_RET_K, SECCOMP_RET_ERRNO_EPERM));
        }
    }
    program.push(statement_v1(BPF_RET_K, SECCOMP_RET_ALLOW));
    if program.len() > usize::from(u16::MAX) {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "seccomp program is too large",
        ));
    }
    Ok(program)
}

#[allow(
    unsafe_code,
    reason = "seccomp TSYNC requires the Linux seccomp syscall"
)]
unsafe fn install_seccomp_v1(terminal: bool) -> io::Result<()> {
    let program = seccomp_program_v1(terminal)?;
    unsafe { install_seccomp_program_v1(&program) }
}

#[allow(
    unsafe_code,
    reason = "seccomp TSYNC requires the Linux seccomp syscall"
)]
unsafe fn install_seccomp_program_v1(program: &[SockFilterV1]) -> io::Result<()> {
    let descriptor = SockFprogV1 {
        len: u16::try_from(program.len())
            .map_err(|_| io::Error::new(ErrorKind::InvalidData, "seccomp program is too large"))?,
        filter: program.as_ptr(),
    };
    let result = unsafe {
        syscall(
            SYS_SECCOMP,
            SECCOMP_SET_MODE_FILTER,
            SECCOMP_FILTER_FLAG_TSYNC,
            &raw const descriptor,
        )
    };
    if result != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[allow(
    unsafe_code,
    reason = "the qualified child must discard every non-contract descriptor"
)]
unsafe fn close_unrelated_fds_v1() -> io::Result<()> {
    for (first, last) in [(3_u32, 63_u32), (73_u32, u32::MAX)] {
        let result = unsafe { syscall(SYS_CLOSE_RANGE, first, last, 0_u32) };
        if result != 0 {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}

static VERIFIED_ISOLATION_V1: OnceLock<Result<IsolationIdentityV1, WorkerFailure>> =
    OnceLock::new();

pub(super) fn verified_isolation_identity_v1() -> Result<IsolationIdentityV1, WorkerFailure> {
    *VERIFIED_ISOLATION_V1.get_or_init(verify_isolation_identity_once_v1)
}

#[allow(
    unsafe_code,
    reason = "fixed inherited descriptors are launcher-authenticated"
)]
fn read_fixed_file_v1(descriptor: RawFd, maximum: usize) -> Result<Vec<u8>, WorkerFailure> {
    let duplicate = rustix::io::fcntl_dupfd_cloexec(
        // SAFETY: the fixed descriptor is inherited only by the internal worker.
        unsafe { std::os::fd::BorrowedFd::borrow_raw(descriptor) },
        10,
    )
    .map_err(|_| WorkerFailure::IsolationUnavailable)?;
    let mut file = File::from(duplicate);
    file.seek(SeekFrom::Start(0))
        .map_err(|_| WorkerFailure::IsolationUnavailable)?;
    let mut bytes = Vec::new();
    file.take(
        u64::try_from(
            maximum
                .checked_add(1)
                .ok_or(WorkerFailure::IsolationUnavailable)?,
        )
        .map_err(|_| WorkerFailure::IsolationUnavailable)?,
    )
    .read_to_end(&mut bytes)
    .map_err(|_| WorkerFailure::IsolationUnavailable)?;
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(WorkerFailure::IsolationUnavailable);
    }
    Ok(bytes)
}

#[allow(
    unsafe_code,
    reason = "fixed inherited descriptors are launcher-authenticated"
)]
fn fixed_fd_seals_v1(descriptor: RawFd) -> Result<rustix::fs::SealFlags, WorkerFailure> {
    // SAFETY: callers validate only launcher-reserved inherited descriptors.
    rustix::fs::fcntl_get_seals(unsafe { std::os::fd::BorrowedFd::borrow_raw(descriptor) })
        .map_err(|_| WorkerFailure::IsolationUnavailable)
}

#[allow(
    unsafe_code,
    reason = "the internal worker verifies the exact inherited process limits"
)]
fn current_rlimit_v1(resource: c_int) -> Result<RlimitV1, WorkerFailure> {
    let mut limit = RlimitV1 {
        current: 0,
        maximum: 0,
    };
    // SAFETY: `limit` is a live writable C-compatible rlimit value.
    if unsafe { getrlimit(resource, &mut limit) } != 0 {
        return Err(WorkerFailure::IsolationUnavailable);
    }
    Ok(limit)
}

#[allow(
    unsafe_code,
    reason = "the internal worker verifies its post-exec dumpability state"
)]
fn process_is_nondumpable_v1() -> bool {
    // SAFETY: PR_GET_DUMPABLE takes no pointer arguments.
    (unsafe { prctl(PR_GET_DUMPABLE, 0, 0, 0, 0) }) == 0
}

fn verify_isolation_identity_once_v1() -> Result<IsolationIdentityV1, WorkerFailure> {
    let attestation_seals = fixed_fd_seals_v1(ATTESTATION_FD_V1)?;
    let required_attestation_seals = rustix::fs::SealFlags::WRITE
        | rustix::fs::SealFlags::GROW
        | rustix::fs::SealFlags::SHRINK
        | rustix::fs::SealFlags::EXEC
        | rustix::fs::SealFlags::SEAL;
    if !attestation_seals.contains(required_attestation_seals) {
        return Err(WorkerFailure::IsolationUnavailable);
    }
    let attestation = read_fixed_file_v1(ATTESTATION_FD_V1, 4096)?;
    let header_bytes = 4 + 1 + 32 * 4 + 2;
    if attestation.len() < header_bytes
        || attestation.get(..4) != Some(ATTESTATION_MAGIC_V1)
        || attestation.get(4) != Some(&ATTESTATION_VERSION_V1)
    {
        return Err(WorkerFailure::IsolationUnavailable);
    }
    let policy_sha256: [u8; 32] = attestation[5..37]
        .try_into()
        .map_err(|_| WorkerFailure::IsolationUnavailable)?;
    let artifact_sha256: [u8; 32] = attestation[37..69]
        .try_into()
        .map_err(|_| WorkerFailure::IsolationUnavailable)?;
    let package_sha256: [u8; 32] = attestation[69..101]
        .try_into()
        .map_err(|_| WorkerFailure::IsolationUnavailable)?;
    let nonce: [u8; 32] = attestation[101..133]
        .try_into()
        .map_err(|_| WorkerFailure::IsolationUnavailable)?;
    let cgroup_length = usize::from(u16::from_be_bytes(
        attestation[133..135]
            .try_into()
            .map_err(|_| WorkerFailure::IsolationUnavailable)?,
    ));
    let cgroup_bytes = attestation
        .get(135..)
        .ok_or(WorkerFailure::IsolationUnavailable)?;
    if cgroup_bytes.len() != cgroup_length
        || nonce == [0; 32]
        || policy_sha256 != sha256(ISOLATION_POLICY_V1)
        || package_sha256 != isolation_package_sha256_v1(artifact_sha256)
    {
        return Err(WorkerFailure::IsolationUnavailable);
    }
    let cgroup_path =
        core::str::from_utf8(cgroup_bytes).map_err(|_| WorkerFailure::IsolationUnavailable)?;
    if !cgroup_path.starts_with('/') || cgroup_path.contains('\0') {
        return Err(WorkerFailure::IsolationUnavailable);
    }

    let executable_seals = fixed_fd_seals_v1(EXECUTABLE_MEMFD_V1)?;
    let required_executable_seals = rustix::fs::SealFlags::WRITE
        | rustix::fs::SealFlags::GROW
        | rustix::fs::SealFlags::SHRINK
        | rustix::fs::SealFlags::EXEC
        | rustix::fs::SealFlags::SEAL;
    if !executable_seals.contains(required_executable_seals) {
        return Err(WorkerFailure::IsolationUnavailable);
    }
    let executable = read_fixed_file_v1(EXECUTABLE_MEMFD_V1, MAX_WORKER_IMAGE_BYTES_V1)?;
    if sha256(&executable) != artifact_sha256 {
        return Err(WorkerFailure::IsolationUnavailable);
    }

    // The bootstrap filter permits exactly one static-image exec. Install a
    // second TSYNC filter after authenticating the sealed launch descriptors
    // and before opening any request path, so the proving process cannot exec.
    // SAFETY: the audited filter is installed before any request file opens.
    #[allow(
        unsafe_code,
        reason = "terminal seccomp closes the one-exec bootstrap window"
    )]
    unsafe {
        install_seccomp_v1(true).map_err(|_| WorkerFailure::IsolationUnavailable)?;
    }

    verify_runtime_contract_v1(cgroup_path)?;
    close_bootstrap_fds_v1()?;
    Ok(IsolationIdentityV1 { package_sha256 })
}

fn verify_runtime_contract_v1(cgroup_path: &str) -> Result<(), WorkerFailure> {
    // This descriptor was opened before Landlock, but procfs materializes the
    // current task status when it is read here, after exec and both filters.
    // Do not copy launcher claims into the sealed attestation as a substitute
    // for this kernel-produced runtime evidence.
    let status = String::from_utf8(read_fixed_file_v1(STATUS_FD_V1, 64 * 1024)?)
        .map_err(|_| WorkerFailure::IsolationUnavailable)?;
    let status_fields = status_fields_v1(&status).ok_or(WorkerFailure::IsolationUnavailable)?;
    if !status_has_unprivileged_identity_v1(&status)
        || status_fields.get("NoNewPrivs") != Some(&"1")
        || status_fields.get("Seccomp") != Some(&"2")
        || status_fields.get("TracerPid") != Some(&"0")
        || status_fields
            .get("Seccomp_filters")
            .and_then(|value| value.parse::<u32>().ok())
            .is_none_or(|count| count < 2)
        || !process_is_nondumpable_v1()
    {
        return Err(WorkerFailure::IsolationUnavailable);
    }
    let core_limit = current_rlimit_v1(RLIMIT_CORE)?;
    let address_space_limit = current_rlimit_v1(RLIMIT_AS)?;
    if core_limit.current != 0
        || core_limit.maximum != 0
        || address_space_limit.current != ADDRESS_SPACE_MAX_BYTES_V1
        || address_space_limit.maximum != ADDRESS_SPACE_MAX_BYTES_V1
    {
        return Err(WorkerFailure::IsolationUnavailable);
    }
    let actual_cgroup = String::from_utf8(read_fixed_file_v1(SELF_CGROUP_FD_V1, 4096)?)
        .map_err(|_| WorkerFailure::IsolationUnavailable)?;
    if actual_cgroup.trim_end() != format!("0::{cgroup_path}") {
        return Err(WorkerFailure::IsolationUnavailable);
    }
    let memory_max = String::from_utf8(read_fixed_file_v1(MEMORY_MAX_FD_V1, 128)?)
        .map_err(|_| WorkerFailure::IsolationUnavailable)?;
    let memory_swap_max = String::from_utf8(read_fixed_file_v1(MEMORY_SWAP_MAX_FD_V1, 128)?)
        .map_err(|_| WorkerFailure::IsolationUnavailable)?;
    let pids_max = String::from_utf8(read_fixed_file_v1(PIDS_MAX_FD_V1, 128)?)
        .map_err(|_| WorkerFailure::IsolationUnavailable)?;
    let cpu_max = String::from_utf8(read_fixed_file_v1(CPU_MAX_FD_V1, 128)?)
        .map_err(|_| WorkerFailure::IsolationUnavailable)?;
    let memory_oom_group = String::from_utf8(read_fixed_file_v1(MEMORY_OOM_GROUP_FD_V1, 128)?)
        .map_err(|_| WorkerFailure::IsolationUnavailable)?;
    if memory_max.trim() != MEMORY_MAX_BYTES_V1.to_string()
        || memory_swap_max.trim() != "0"
        || pids_max.trim() != PIDS_MAX_V1.to_string()
        || cpu_max.trim() != "max 100000"
        || memory_oom_group.trim() != "1"
    {
        return Err(WorkerFailure::IsolationUnavailable);
    }
    match File::open("/etc/passwd") {
        Err(error) if error.kind() == ErrorKind::PermissionDenied => {}
        _ => return Err(WorkerFailure::IsolationUnavailable),
    }
    Ok(())
}

#[allow(
    unsafe_code,
    reason = "authenticated bootstrap descriptors must not reach proof execution"
)]
fn close_bootstrap_fds_v1() -> Result<(), WorkerFailure> {
    for descriptor in ATTESTATION_FD_V1..=MEMORY_OOM_GROUP_FD_V1 {
        // SAFETY: these are the exact launcher-reserved descriptors and every
        // value has been consumed before this terminal cleanup step.
        if unsafe { close(descriptor) } != 0 {
            return Err(WorkerFailure::IsolationUnavailable);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn evaluate_seccomp_v1(program: &[SockFilterV1], arch: u32, syscall: u32, arg0: u32) -> u32 {
        let mut accumulator = 0_u32;
        let mut index = 0_usize;
        loop {
            let instruction = program.get(index).expect("in-bounds BPF control flow");
            match instruction.code {
                BPF_LD_W_ABS => {
                    accumulator = match instruction.k {
                        SECCOMP_DATA_ARCH_OFFSET => arch,
                        SECCOMP_DATA_NR_OFFSET => syscall,
                        SECCOMP_DATA_ARG0_LOW_OFFSET => arg0,
                        _ => panic!("unexpected seccomp input offset"),
                    };
                    index += 1;
                }
                BPF_JMP_JEQ_K => {
                    index += 1 + usize::from(if accumulator == instruction.k {
                        instruction.jt
                    } else {
                        instruction.jf
                    });
                }
                BPF_JMP_JSET_K => {
                    index += 1 + usize::from(if accumulator & instruction.k != 0 {
                        instruction.jt
                    } else {
                        instruction.jf
                    });
                }
                BPF_JMP_JGE_K => {
                    index += 1 + usize::from(if accumulator >= instruction.k {
                        instruction.jt
                    } else {
                        instruction.jf
                    });
                }
                BPF_RET_K => return instruction.k,
                _ => panic!("unexpected seccomp instruction"),
            }
        }
    }

    #[test]
    fn isolation_policy_and_package_digest_are_domain_separated_and_nonzero() {
        assert_ne!(sha256(ISOLATION_POLICY_V1), [0; 32]);
        let first = isolation_package_sha256_v1([0x11; 32]);
        let second = isolation_package_sha256_v1([0x12; 32]);
        assert_ne!(first, [0; 32]);
        assert_ne!(first, second);
    }

    #[test]
    fn runtime_identity_rejects_root_capabilities_and_ambiguous_status() {
        let unprivileged = concat!(
            "Uid:\t1000\t1000\t1000\t1000\n",
            "CapInh:\t0000000000000000\n",
            "CapPrm:\t0000000000000000\n",
            "CapEff:\t0000000000000000\n",
            "CapAmb:\t0000000000000000\n",
        );
        assert!(status_has_unprivileged_identity_v1(unprivileged));
        for hostile in [
            unprivileged.replace("1000\t1000\t1000\t1000", "0\t0\t0\t0"),
            unprivileged.replace("CapEff:\t0000000000000000", "CapEff:\t0000000000000001"),
            unprivileged.replace("CapPrm:\t0000000000000000", "CapPrm:\t0000000000000001"),
            unprivileged.replace("CapInh:\t0000000000000000", "CapInh:\t0000000000000001"),
            unprivileged.replace("CapAmb:\t0000000000000000", "CapAmb:\t0000000000000001"),
            format!("{unprivileged}CapEff:\t0000000000000000\n"),
        ] {
            assert!(!status_has_unprivileged_identity_v1(&hostile));
        }
    }

    #[test]
    fn static_elf_parser_rejects_dynamic_and_wrong_architecture_images() {
        let mut image = vec![0_u8; 120];
        image[..4].copy_from_slice(b"\x7fELF");
        image[4] = 2;
        image[5] = 1;
        image[18..20].copy_from_slice(&183_u16.to_le_bytes());
        image[32..40].copy_from_slice(&64_u64.to_le_bytes());
        image[54..56].copy_from_slice(&56_u16.to_le_bytes());
        image[56..58].copy_from_slice(&1_u16.to_le_bytes());
        assert!(validate_static_aarch64_elf_v1(&image).is_ok());
        image[64..68].copy_from_slice(&3_u32.to_le_bytes());
        assert!(validate_static_aarch64_elf_v1(&image).is_err());
        image[64..68].copy_from_slice(&0_u32.to_le_bytes());
        image[18..20].copy_from_slice(&62_u16.to_le_bytes());
        assert!(validate_static_aarch64_elf_v1(&image).is_err());
    }

    #[test]
    fn seccomp_program_binds_architecture_threads_and_every_denial() {
        let bootstrap = seccomp_program_v1(false).expect("bounded bootstrap program");
        let program = seccomp_program_v1(true).expect("bounded program");
        assert_eq!(program[1].k, AUDIT_ARCH_AARCH64);
        assert_eq!(program[4].k, SYS_CLONE);
        assert!(program.iter().any(|instruction| {
            instruction.code == BPF_JMP_JSET_K && instruction.k == FORBIDDEN_CLONE_FLAGS_V1
        }));
        for denied in DENIED_SYSCALLS_V1 {
            assert!(program.iter().any(|instruction| {
                instruction.code == BPF_JMP_JEQ_K && instruction.k == *denied
            }));
        }
        for denied in [221_u32, 281_u32] {
            assert!(!bootstrap.iter().any(|instruction| {
                instruction.code == BPF_JMP_JEQ_K && instruction.k == denied
            }));
            assert!(program.iter().any(|instruction| {
                instruction.code == BPF_JMP_JEQ_K && instruction.k == denied
            }));
        }
        assert_eq!(
            program.last().map(|instruction| instruction.k),
            Some(SECCOMP_RET_ALLOW)
        );
        assert_eq!(
            evaluate_seccomp_v1(&program, AUDIT_ARCH_AARCH64 ^ 1, 64, 0),
            SECCOMP_RET_KILL_PROCESS
        );
        assert_eq!(
            evaluate_seccomp_v1(&program, AUDIT_ARCH_AARCH64, 64, 0),
            SECCOMP_RET_ALLOW
        );
        assert_eq!(
            evaluate_seccomp_v1(&program, AUDIT_ARCH_AARCH64, 198, 0),
            SECCOMP_RET_ERRNO_EPERM
        );
        assert_eq!(
            evaluate_seccomp_v1(
                &program,
                AUDIT_ARCH_AARCH64,
                SYS_CLONE,
                CLONE_VM | CLONE_THREAD,
            ),
            SECCOMP_RET_ALLOW
        );
        assert_eq!(
            evaluate_seccomp_v1(&program, AUDIT_ARCH_AARCH64, SYS_CLONE, CLONE_VM),
            SECCOMP_RET_ERRNO_EPERM
        );
        assert_eq!(
            evaluate_seccomp_v1(&program, AUDIT_ARCH_AARCH64, SYS_CLONE3, 0),
            SECCOMP_RET_ERRNO_ENOSYS
        );
        for denied in [424_u32, 434_u32, 438_u32, 440_u32, 448_u32] {
            assert_eq!(
                evaluate_seccomp_v1(&program, AUDIT_ARCH_AARCH64, denied, 0),
                SECCOMP_RET_ERRNO_EPERM
            );
        }
        assert_eq!(
            evaluate_seccomp_v1(&program, AUDIT_ARCH_AARCH64, FIRST_UNREVIEWED_SYSCALL_V1, 0,),
            SECCOMP_RET_ERRNO_EPERM
        );
        assert_eq!(
            evaluate_seccomp_v1(&bootstrap, AUDIT_ARCH_AARCH64, 221, 0),
            SECCOMP_RET_ALLOW
        );
        assert_eq!(
            evaluate_seccomp_v1(&program, AUDIT_ARCH_AARCH64, 221, 0),
            SECCOMP_RET_ERRNO_EPERM
        );
    }

    #[test]
    fn attestation_is_exact_and_rejects_oversized_cgroup_path() {
        let nonce = [0x33; 32];
        let bytes = attestation_bytes_v1([0x44; 32], "/release/worker", nonce)
            .expect("canonical attestation");
        assert_eq!(&bytes[..4], ATTESTATION_MAGIC_V1);
        assert_eq!(bytes[4], ATTESTATION_VERSION_V1);
        assert_eq!(&bytes[101..133], nonce.as_slice());
        assert!(
            attestation_bytes_v1([0x44; 32], &format!("/{}", "x".repeat(70_000)), nonce).is_err()
        );
    }
}
