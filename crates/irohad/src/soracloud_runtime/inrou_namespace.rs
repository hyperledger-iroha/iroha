//! Single-path Linux namespace and minimal-root confinement for PortableVM.
//!
//! The root supervisor retains its self-exec watchdog, the pinned bubblewrap
//! launcher, and bubblewrap's private PID1 reaper as one exact process chain. QEMU is
//! released into private mount, network, IPC, UTS, PID, and cgroup namespaces
//! with a fixed authenticated runtime root populated only by QEMU's closure, `/dev/kvm`,
//! immutable inputs, and the exact writable disks. Live procfs attestation is
//! retained so the supervisor can reject a changed namespace or mount graph
//! before bridging any request.

use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsString,
    fs,
    io::{self, Read as _},
    net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream},
    os::fd::AsRawFd as _,
    os::unix::fs::{FileTypeExt as _, MetadataExt as _, OpenOptionsExt as _},
    path::{Component, Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use eyre::WrapErr as _;
use iroha_crypto::sha256_reader_bounded;
use iroha_data_model::soracloud::SORA_INROU_DATA_VOLUME_MAX_COUNT_V1;

use super::PortableVmChildIdentity;

use super::inrou_cgroup::InrouCgroupAttestation;

const INROU_BWRAP_PATH: &str = "/usr/bin/bwrap";
const INROU_RUNTIME_ROOT: &str = "/opt/iroha/inrou-runtime-v1/root";
const INROU_RUNTIME_MANIFEST: &str = "/opt/iroha/inrou-runtime-v1/manifest.sha256";
const INROU_RUNTIME_MANIFEST_HEADER: &str = "iroha-inrou-runtime-v1 sha256";
const INROU_NAMESPACE_HOSTNAME: &str = "inrou-v1";
const INROU_NAMESPACE_PROC_MAX_BYTES: u64 = 1024 * 1024;
const INROU_RUNTIME_MANIFEST_MAX_BYTES: u64 = 1024 * 1024;
const INROU_RUNTIME_FILE_MAX_BYTES: u64 = 1024 * 1024 * 1024;
const INROU_RUNTIME_TOTAL_MAX_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const INROU_RUNTIME_MAX_FILES: usize = 512;
pub(super) const INROU_NAMESPACE_MAX_LEASE_DISKS: usize = SORA_INROU_DATA_VOLUME_MAX_COUNT_V1;
const INROU_NAMESPACE_TOOL_PROBE_MAX_BYTES: usize = 1024 * 1024;
const INROU_NAMESPACE_TOOL_PROBE_TIMEOUT: Duration = Duration::from_secs(5);
const INROU_NAMESPACE_MAX_CGROUP_PIDS: usize = 4;
const INROU_BWRAP_REQUIRED_OPTIONS: [&str; 14] = [
    "--bind-fd",
    "--clearenv",
    "--dev-bind",
    "--die-with-parent",
    "--new-session",
    "--proc",
    "--ro-bind",
    "--ro-bind-fd",
    "--tmpfs",
    "--unshare-cgroup",
    "--unshare-ipc",
    "--unshare-net",
    "--unshare-pid",
    "--unshare-uts",
];
const INROU_NAMESPACE_CONNECT_TIMEOUT: Duration = Duration::from_secs(1);
const INROU_NAMESPACE_CONNECT_THREAD_STACK_BYTES: usize = 256 * 1024;

pub(super) const INROU_NAMESPACE_QEMU_PATH: &str = "/inrou/bin/qemu";
pub(super) const INROU_NAMESPACE_SETPRIV_PATH: &str = "/inrou/bin/setpriv";
pub(super) const INROU_NAMESPACE_KERNEL_PATH: &str = "/inrou/input/kernel";
pub(super) const INROU_NAMESPACE_INITRD_PATH: &str = "/inrou/input/initrd";
pub(super) const INROU_NAMESPACE_BUNDLE_PATH: &str = "/inrou/input/bundle";
pub(super) const INROU_NAMESPACE_CLOUD_INIT_ROOT: &str = "/inrou/input/cloud-init";
pub(super) const INROU_NAMESPACE_ROOT_DISK_PATH: &str = "/inrou/disk/root";
const INROU_NAMESPACE_MANDATORY_PRODUCTION_BINDINGS: [(&str, bool); 6] = [
    (INROU_NAMESPACE_KERNEL_PATH, false),
    (INROU_NAMESPACE_BUNDLE_PATH, false),
    ("/inrou/input/cloud-init/meta-data", false),
    ("/inrou/input/cloud-init/network-config", false),
    ("/inrou/input/cloud-init/user-data", false),
    (INROU_NAMESPACE_ROOT_DISK_PATH, true),
];
const INROU_NAMESPACE_MAX_INITRD_BINDINGS: usize = 1;
pub(super) const INROU_NAMESPACE_MAX_PRODUCTION_BINDINGS: usize =
    INROU_NAMESPACE_MANDATORY_PRODUCTION_BINDINGS.len()
        + INROU_NAMESPACE_MAX_INITRD_BINDINGS
        + INROU_NAMESPACE_MAX_LEASE_DISKS;

#[derive(Clone, Debug)]
pub(super) struct InrouNamespaceTools {
    bubblewrap: PathBuf,
}

#[derive(Debug)]
pub(super) struct InrouNamespaceBindingRequest {
    pub host_path: PathBuf,
    pub host_file: fs::File,
    pub sandbox_path: PathBuf,
    pub writable: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct InrouNamespaceLauncherBindingV1 {
    pub descriptor: std::os::fd::RawFd,
    pub sandbox_path: PathBuf,
    pub writable: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct InrouFileIdentity {
    device: u64,
    inode: u64,
    mode: u32,
    uid: u32,
    gid: u32,
    links: u64,
    size: u64,
    kind: InrouFileKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum InrouFileKind {
    Regular,
    Character,
    Directory,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum InrouNamespacePlanMode {
    StaticPreflight,
    StartupProbe,
    Production,
}

#[derive(Debug)]
struct InrouNamespaceBinding {
    host_file: fs::File,
    sandbox_path: PathBuf,
    identity: InrouFileIdentity,
    writable: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct InrouRuntimeManifestEntry {
    kind: InrouRuntimeEntryKind,
    sandbox_path: PathBuf,
    sha256: Option<[u8; 32]>,
    exact_bytes: u64,
    mode: u32,
    identity: Option<InrouFileIdentity>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum InrouRuntimeEntryKind {
    Directory,
    Regular,
}

#[derive(Debug)]
pub(super) struct InrouNamespacePlan {
    tools: InrouNamespaceTools,
    runtime_root: PathBuf,
    runtime_root_identity: InrouFileIdentity,
    runtime_entries: Vec<InrouRuntimeManifestEntry>,
    qemu_identity: InrouFileIdentity,
    watchdog_identity: InrouFileIdentity,
    bubblewrap_identity: InrouFileIdentity,
    kvm_identity: InrouFileIdentity,
    bindings: Vec<InrouNamespaceBinding>,
    expected_root_entries: BTreeMap<PathBuf, BTreeSet<OsString>>,
}

#[derive(Clone, Debug)]
pub(super) struct InrouNamespaceAttestation {
    plan: Arc<InrouNamespacePlan>,
    launcher_pid: u32,
    bubblewrap_pid: u32,
    reaper_pid: u32,
    qemu_pid: u32,
    identity: PortableVmChildIdentity,
    namespaces: BTreeMap<&'static str, u64>,
}

#[derive(Debug)]
struct InrouNamespaceProcessStatus {
    pid: u32,
    parent_pid: u32,
    namespace_pids: Vec<u32>,
    uids: [u32; 4],
    gids: [u32; 4],
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct InrouMountInfoRecord {
    mount_point: PathBuf,
    filesystem_type: String,
    mount_options: BTreeSet<String>,
}

impl InrouNamespaceTools {
    pub(super) fn resolve_exact() -> eyre::Result<Self> {
        let tools = Self {
            bubblewrap: validate_pinned_root_executable(Path::new(INROU_BWRAP_PATH), "bubblewrap")?,
        };
        tools.probe_exact_launcher_surface()?;
        Ok(tools)
    }

    pub(super) fn bubblewrap(&self) -> &Path {
        &self.bubblewrap
    }

    fn probe_exact_launcher_surface(&self) -> eyre::Result<()> {
        let output = super::run_host_command_capture_stdout_bounded(
            &self.bubblewrap,
            &["--help"],
            INROU_NAMESPACE_TOOL_PROBE_TIMEOUT,
            INROU_NAMESPACE_TOOL_PROBE_MAX_BYTES,
        )
        .wrap_err("probe the pinned bubblewrap launcher surface")?;
        let help = String::from_utf8(output).wrap_err("decode pinned bubblewrap help")?;
        require_inrou_bubblewrap_help(&help)
    }
}

fn require_inrou_bubblewrap_help(help: &str) -> eyre::Result<()> {
    for required in INROU_BWRAP_REQUIRED_OPTIONS {
        if !help.split_ascii_whitespace().any(|word| word == required) {
            eyre::bail!("pinned bubblewrap launcher omitted mandatory option `{required}`");
        }
    }
    Ok(())
}

impl InrouNamespacePlan {
    pub(super) fn preflight(tools: InrouNamespaceTools) -> eyre::Result<()> {
        Self::prepare_inner(
            tools,
            0,
            Vec::new(),
            InrouNamespacePlanMode::StaticPreflight,
        )
        .map(|_| ())
    }

    /// Prepare the exact minimal-root plan used by the startup KVM probe.
    ///
    /// The probe binds the authenticated empty kernel placeholder over its own
    /// destination through one read-only descriptor, exercising the production
    /// descriptor-mount path without accepting a workload file. It then launches
    /// the production QEMU machine profile without guest artifacts.
    pub(super) fn prepare_startup_probe(tools: InrouNamespaceTools) -> eyre::Result<Arc<Self>> {
        Self::prepare_inner(tools, 0, Vec::new(), InrouNamespacePlanMode::StartupProbe)
    }

    pub(super) fn prepare(
        tools: InrouNamespaceTools,
        child_gid: u32,
        requests: Vec<InrouNamespaceBindingRequest>,
    ) -> eyre::Result<Arc<Self>> {
        Self::prepare_inner(
            tools,
            child_gid,
            requests,
            InrouNamespacePlanMode::Production,
        )
    }

    fn prepare_inner(
        tools: InrouNamespaceTools,
        child_gid: u32,
        requests: Vec<InrouNamespaceBindingRequest>,
        mode: InrouNamespacePlanMode,
    ) -> eyre::Result<Arc<Self>> {
        let runtime_root = PathBuf::from(INROU_RUNTIME_ROOT);
        let runtime_root_identity = inspect_root_runtime_directory(&runtime_root)?;
        let runtime_entries =
            load_and_attest_runtime_manifest(&runtime_root, Path::new(INROU_RUNTIME_MANIFEST))?;
        require_runtime_manifest_layout(&runtime_entries)?;
        let qemu_host_path =
            runtime_host_path(&runtime_root, Path::new(INROU_NAMESPACE_QEMU_PATH))?;
        let setpriv_host_path =
            runtime_host_path(&runtime_root, Path::new(INROU_NAMESPACE_SETPRIV_PATH))?;
        let qemu_identity = inspect_root_runtime_file(&qemu_host_path, "manifest QEMU executable")?;
        inspect_root_runtime_file(&setpriv_host_path, "manifest setpriv executable")?;
        for required in [INROU_NAMESPACE_QEMU_PATH, INROU_NAMESPACE_SETPRIV_PATH] {
            let Some(entry) = runtime_entries
                .iter()
                .find(|entry| entry.sandbox_path == Path::new(required))
            else {
                eyre::bail!("Inrou runtime manifest omitted required `{required}`");
            };
            if entry.kind != InrouRuntimeEntryKind::Regular || entry.mode != 0o555 {
                eyre::bail!("Inrou runtime manifest entry `{required}` is not executable");
            }
        }
        let bubblewrap_identity = inspect_root_runtime_file(&tools.bubblewrap, "bubblewrap")?;
        let watchdog_identity = proc_executable_identity(std::process::id())?;
        let kvm_identity = inspect_kvm_device(Path::new("/dev/kvm"))?;
        let mut bindings = Vec::new();
        for request in requests {
            validate_sandbox_binding_path(&request.sandbox_path)?;
            let placeholder = runtime_entries
                .iter()
                .find(|entry| entry.sandbox_path == request.sandbox_path);
            if !placeholder.is_some_and(|entry| {
                entry.kind == InrouRuntimeEntryKind::Regular
                    && entry.exact_bytes == 0
                    && entry.mode == 0o444
            }) {
                eyre::bail!(
                    "Inrou minimal root lacks an authenticated empty placeholder at {}",
                    request.sandbox_path.display()
                );
            }
            if bindings
                .iter()
                .any(|binding: &InrouNamespaceBinding| binding.sandbox_path == request.sandbox_path)
            {
                eyre::bail!(
                    "Inrou minimal root repeats sandbox target {}",
                    request.sandbox_path.display()
                );
            }
            let identity = inspect_delegated_input_file(
                &request.host_path,
                &request.host_file,
                child_gid,
                request.writable,
            )?;
            bindings.push(InrouNamespaceBinding {
                host_file: request.host_file,
                sandbox_path: request.sandbox_path,
                identity,
                writable: request.writable,
            });
        }

        if mode == InrouNamespacePlanMode::StartupProbe {
            bindings.push(prepare_startup_probe_binding(
                &runtime_root,
                &runtime_entries,
            )?);
        }

        bindings.sort_by(|left, right| left.sandbox_path.cmp(&right.sandbox_path));
        match mode {
            InrouNamespacePlanMode::Production => require_exact_binding_layout(&bindings)?,
            InrouNamespacePlanMode::StaticPreflight => {
                if !bindings.is_empty() {
                    eyre::bail!(
                        "static Inrou namespace preflight must not accept dynamic bindings"
                    );
                }
            }
            InrouNamespacePlanMode::StartupProbe => {
                let [binding] = bindings.as_slice() else {
                    eyre::bail!(
                        "Inrou startup probe requires one exact read-only descriptor binding"
                    );
                };
                if binding.sandbox_path != Path::new(INROU_NAMESPACE_KERNEL_PATH)
                    || binding.writable
                {
                    eyre::bail!(
                        "Inrou startup probe requires one exact read-only descriptor binding"
                    );
                }
            }
        }
        let expected_root_entries = expected_minimal_root_entries(&runtime_entries, &bindings)?;
        Ok(Arc::new(Self {
            tools,
            runtime_root,
            runtime_root_identity,
            runtime_entries,
            qemu_identity,
            watchdog_identity,
            bubblewrap_identity,
            kvm_identity,
            bindings,
            expected_root_entries,
        }))
    }

    pub(super) fn launcher(&self) -> &Path {
        self.tools.bubblewrap()
    }

    pub(super) fn io_backing_paths(&self) -> eyre::Result<Vec<PathBuf>> {
        let mut paths = self
            .runtime_entries
            .iter()
            .filter(|entry| entry.kind == InrouRuntimeEntryKind::Regular)
            .map(|entry| runtime_host_path(&self.runtime_root, &entry.sandbox_path))
            .collect::<eyre::Result<Vec<_>>>()?;
        paths.extend(self.bindings.iter().map(|binding| {
            PathBuf::from(format!("/proc/self/fd/{}", binding.host_file.as_raw_fd()))
        }));
        paths.sort();
        paths.dedup();
        Ok(paths)
    }

    pub(super) fn binding_files(&self) -> impl ExactSizeIterator<Item = &fs::File> {
        self.bindings.iter().map(|binding| &binding.host_file)
    }

    pub(super) fn launcher_binding_map(
        &self,
        inherited_binding_fds: &[std::os::fd::RawFd],
    ) -> eyre::Result<Vec<InrouNamespaceLauncherBindingV1>> {
        build_inrou_launcher_binding_map(&self.bindings, inherited_binding_fds)
    }

    pub(super) fn command_arguments(
        &self,
        identity: &PortableVmChildIdentity,
        inherited_binding_fds: &[std::os::fd::RawFd],
    ) -> eyre::Result<Vec<OsString>> {
        validate_exact_file_identity(
            &self.tools.bubblewrap,
            self.bubblewrap_identity,
            "bubblewrap launcher",
        )?;
        validate_exact_file_identity(
            &self.runtime_root,
            self.runtime_root_identity,
            "Inrou runtime root",
        )?;
        attest_runtime_manifest_entries(&self.runtime_root, &self.runtime_entries)?;
        for binding in &self.bindings {
            validate_opened_file_identity(
                &binding.host_file,
                binding.identity,
                "Inrou namespace binding",
            )?;
        }

        let mut arguments = inrou_bubblewrap_namespace_arguments();

        arguments.extend([
            "--ro-bind".into(),
            self.runtime_root.as_os_str().to_owned(),
            "/".into(),
        ]);
        arguments.extend(["--proc".into(), "/proc".into()]);
        arguments.extend(["--dev".into(), "/dev".into()]);
        arguments.extend(["--dev-bind".into(), "/dev/kvm".into(), "/dev/kvm".into()]);
        arguments.extend(["--tmpfs".into(), "/tmp".into()]);
        append_inrou_binding_arguments(&mut arguments, &self.bindings, inherited_binding_fds)?;
        arguments.extend(["--chdir".into(), "/".into(), "--".into()]);
        arguments.push(INROU_NAMESPACE_SETPRIV_PATH.into());
        arguments.extend(inrou_namespaced_setpriv_arguments(identity));
        arguments.push(INROU_NAMESPACE_QEMU_PATH.into());
        arguments.extend(
            [
                "-no-user-config",
                "-sandbox",
                super::SORACLOUD_INROU_QEMU_SANDBOX_POLICY,
                "-run-with",
                "exit-with-parent=on",
            ]
            .into_iter()
            .map(OsString::from),
        );
        Ok(arguments)
    }

    pub(super) fn discover_and_attest_qemu(
        self: &Arc<Self>,
        child: &mut std::process::Child,
        cgroup: &InrouCgroupAttestation,
        identity: &PortableVmChildIdentity,
        deadline: std::time::Instant,
    ) -> eyre::Result<InrouNamespaceAttestation> {
        loop {
            super::require_inrou_launcher_running(child)?;
            let last_mismatch = match self.try_discover_qemu(child.id(), cgroup, identity) {
                Ok(attestation) => return Ok(attestation),
                Err(error) => error.to_string(),
            };
            if std::time::Instant::now() >= deadline {
                eyre::bail!(
                    "nested Inrou QEMU did not reach its exact namespace and mount posture before the fixed deadline: {last_mismatch}"
                );
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn try_discover_qemu(
        self: &Arc<Self>,
        launcher_pid: u32,
        cgroup: &InrouCgroupAttestation,
        identity: &PortableVmChildIdentity,
    ) -> eyre::Result<InrouNamespaceAttestation> {
        cgroup.attest_pid(launcher_pid)?;
        validate_proc_executable(
            launcher_pid,
            self.watchdog_identity,
            "Inrou supervisor watchdog",
        )?;
        let members = cgroup.member_pids()?;
        if members.len() != INROU_NAMESPACE_MAX_CGROUP_PIDS || !members.contains(&launcher_pid) {
            eyre::bail!(
                "Inrou namespace cgroup must contain exactly the watchdog, bubblewrap launcher, PID1 reaper, and QEMU; members are {:?}",
                members
            );
        }
        let qemu_candidates = members
            .iter()
            .copied()
            .filter(|pid| *pid != launcher_pid)
            .filter(|pid| {
                proc_executable_identity(*pid).is_ok_and(|actual| actual == self.qemu_identity)
            })
            .collect::<Vec<_>>();
        let [qemu_pid] = qemu_candidates.as_slice() else {
            eyre::bail!("Inrou cgroup does not contain exactly one pinned nested QEMU");
        };
        let qemu_pid = *qemu_pid;
        let status = super::read_inrou_proc_status(qemu_pid)?;
        let reaper_pid = parse_namespace_process_status(&status, qemu_pid)?.parent_pid;
        let bubblewrap_candidates = members
            .iter()
            .copied()
            .filter(|pid| *pid != launcher_pid && *pid != qemu_pid)
            .collect::<Vec<_>>();
        if !bubblewrap_candidates.contains(&reaper_pid) {
            eyre::bail!("nested QEMU parent is not the retained bubblewrap PID1 reaper");
        }
        let bubblewrap_pid = *bubblewrap_candidates
            .iter()
            .find(|pid| **pid != reaper_pid)
            .ok_or_else(|| eyre::eyre!("Inrou namespace cgroup omitted outer bubblewrap"))?;
        let mut attestation = InrouNamespaceAttestation {
            plan: Arc::clone(self),
            launcher_pid,
            bubblewrap_pid,
            reaper_pid,
            qemu_pid,
            identity: identity.clone(),
            namespaces: BTreeMap::new(),
        };
        attestation.namespaces = attestation.attest_process_chain(cgroup)?;
        self.attest_mounts_and_root(qemu_pid, true)?;
        attest_private_loopback_only(qemu_pid)?;
        Ok(attestation)
    }

    fn attest_mounts_and_root(
        &self,
        qemu_pid: u32,
        authenticate_contents: bool,
    ) -> eyre::Result<()> {
        let mountinfo = read_bounded_proc_file(
            &PathBuf::from(format!("/proc/{qemu_pid}/mountinfo")),
            "nested QEMU mountinfo",
        )?;
        let mounts = parse_mountinfo(&mountinfo)?;
        require_bind_mount_mode(&mounts, Path::new("/"), false)?;
        require_mount(&mounts, Path::new("/proc"), "proc", false)?;
        require_mount(&mounts, Path::new("/tmp"), "tmpfs", false)?;
        require_mount(&mounts, Path::new("/dev"), "tmpfs", false)?;
        if mounts.iter().any(|mount| {
            mount.mount_point == Path::new("/run")
                || mount.mount_point.starts_with("/run/")
                || mount.mount_point == Path::new("/sys")
                || mount.mount_point.starts_with("/sys/")
        }) {
            eyre::bail!("nested QEMU mount graph exposes forbidden host `/run` or `/sys` state");
        }
        attest_kvm_binding(qemu_pid, &mounts, self.kvm_identity)?;
        validate_followed_file_identity(
            &PathBuf::from(format!("/proc/{qemu_pid}/root")),
            self.runtime_root_identity,
            "nested QEMU runtime root",
        )?;
        for entry in &self.runtime_entries {
            if entry.kind != InrouRuntimeEntryKind::Regular
                || self
                    .bindings
                    .iter()
                    .any(|binding| binding.sandbox_path == entry.sandbox_path)
            {
                continue;
            }
            let actual_path = proc_root_path(qemu_pid, &entry.sandbox_path)?;
            attest_runtime_manifest_file(&actual_path, entry, authenticate_contents)?;
        }
        for binding in &self.bindings {
            let actual_path = proc_root_path(qemu_pid, &binding.sandbox_path)?;
            validate_live_binding_identity(&actual_path, binding)?;
            require_bind_mount_mode(&mounts, &binding.sandbox_path, binding.writable)?;
        }
        validate_minimal_root_tree(qemu_pid, &self.expected_root_entries)?;
        Ok(())
    }
}

fn prepare_startup_probe_binding(
    runtime_root: &Path,
    runtime_entries: &[InrouRuntimeManifestEntry],
) -> eyre::Result<InrouNamespaceBinding> {
    let sandbox_path = PathBuf::from(INROU_NAMESPACE_KERNEL_PATH);
    let entry = runtime_entries
        .iter()
        .find(|entry| entry.sandbox_path == sandbox_path)
        .ok_or_else(|| eyre::eyre!("Inrou startup probe runtime placeholder is absent"))?;
    if entry.kind != InrouRuntimeEntryKind::Regular || entry.mode != 0o444 || entry.exact_bytes != 0
    {
        eyre::bail!("Inrou startup probe runtime placeholder is not an authenticated empty file");
    }
    let identity = entry
        .identity
        .ok_or_else(|| eyre::eyre!("Inrou startup probe runtime placeholder lacks an identity"))?;
    let host_path = runtime_host_path(runtime_root, &sandbox_path)?;
    let mut options = fs::OpenOptions::new();
    options
        .read(true)
        .custom_flags((rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::CLOEXEC).bits() as i32);
    let host_file = options
        .open(&host_path)
        .wrap_err("open the fixed Inrou startup-probe descriptor binding")?;
    require_opened_file_matches_name(&host_path, &host_file, "Inrou startup-probe binding")?;
    validate_opened_file_identity(&host_file, identity, "Inrou startup-probe binding")?;
    Ok(InrouNamespaceBinding {
        host_file,
        sandbox_path,
        identity,
        writable: false,
    })
}

fn append_inrou_binding_arguments(
    arguments: &mut Vec<OsString>,
    bindings: &[InrouNamespaceBinding],
    inherited_binding_fds: &[std::os::fd::RawFd],
) -> eyre::Result<()> {
    if inherited_binding_fds.len() != bindings.len() {
        eyre::bail!(
            "Inrou namespace launcher received {} inherited binding fds for {} exact bindings",
            inherited_binding_fds.len(),
            bindings.len()
        );
    }
    for (binding, inherited_fd) in bindings.iter().zip(inherited_binding_fds) {
        arguments.push(if binding.writable {
            "--bind-fd".into()
        } else {
            "--ro-bind-fd".into()
        });
        arguments.push(inherited_fd.to_string().into());
        arguments.push(binding.sandbox_path.as_os_str().to_owned());
    }
    Ok(())
}

fn build_inrou_launcher_binding_map(
    bindings: &[InrouNamespaceBinding],
    inherited_binding_fds: &[std::os::fd::RawFd],
) -> eyre::Result<Vec<InrouNamespaceLauncherBindingV1>> {
    if inherited_binding_fds.len() != bindings.len() {
        eyre::bail!(
            "Inrou namespace launcher received {} inherited binding fds for {} exact bindings",
            inherited_binding_fds.len(),
            bindings.len()
        );
    }
    Ok(bindings
        .iter()
        .zip(inherited_binding_fds)
        .map(|(binding, descriptor)| InrouNamespaceLauncherBindingV1 {
            descriptor: *descriptor,
            sandbox_path: binding.sandbox_path.clone(),
            writable: binding.writable,
        })
        .collect())
}

fn inrou_bubblewrap_namespace_arguments() -> Vec<OsString> {
    [
        "--die-with-parent",
        "--new-session",
        "--unshare-pid",
        "--unshare-net",
        "--unshare-ipc",
        "--unshare-uts",
        "--unshare-cgroup",
        "--hostname",
        INROU_NAMESPACE_HOSTNAME,
        "--clearenv",
        "--setenv",
        "HOME",
        "/tmp",
        "--setenv",
        "TMPDIR",
        "/tmp",
        "--setenv",
        "PATH",
        "/inrou/bin",
    ]
    .into_iter()
    .map(OsString::from)
    .collect()
}

impl InrouNamespaceAttestation {
    fn attest_process_chain(
        &self,
        cgroup: &InrouCgroupAttestation,
    ) -> eyre::Result<BTreeMap<&'static str, u64>> {
        let processes = [
            (
                self.launcher_pid,
                self.plan.watchdog_identity,
                "Inrou supervisor watchdog",
            ),
            (
                self.bubblewrap_pid,
                self.plan.bubblewrap_identity,
                "outer bubblewrap",
            ),
            (
                self.reaper_pid,
                self.plan.bubblewrap_identity,
                "bubblewrap PID1 reaper",
            ),
            (self.qemu_pid, self.plan.qemu_identity, "nested QEMU"),
        ];
        let expected = processes.map(|(pid, _, _)| pid);
        validate_namespace_cgroup_members(&cgroup.member_pids()?, expected)?;
        let mut statuses = Vec::with_capacity(processes.len());
        for (pid, executable, label) in processes {
            cgroup.attest_pid(pid)?;
            validate_proc_executable(pid, executable, label)?;
            let status = super::read_inrou_proc_status(pid)?;
            if pid == self.qemu_pid {
                super::validate_inrou_qemu_proc_status(&status, &self.identity)?;
            }
            statuses.push(parse_namespace_process_status(&status, pid)?);
        }
        validate_namespace_process_chain_status(std::process::id(), &statuses)?;
        let namespaces = attest_private_namespace_set(self.qemu_pid)?;
        if attest_private_namespace_set(self.reaper_pid)? != namespaces {
            eyre::bail!("bubblewrap PID1 reaper does not share QEMU's private namespace set");
        }
        for namespace in namespaces.keys() {
            let supervisor =
                read_namespace_id(&PathBuf::from(format!("/proc/self/ns/{namespace}")))?;
            let watchdog = read_namespace_id(&PathBuf::from(format!(
                "/proc/{}/ns/{namespace}",
                self.launcher_pid
            )))?;
            if watchdog != supervisor {
                eyre::bail!("Inrou watchdog changed its supervisor's {namespace} namespace");
            }
            if *namespace == "pid"
                && read_namespace_id(&PathBuf::from(format!(
                    "/proc/{}/ns/pid",
                    self.bubblewrap_pid
                )))? != supervisor
            {
                eyre::bail!("outer bubblewrap changed its supervisor's PID namespace");
            }
        }
        validate_followed_file_identity(
            &PathBuf::from(format!("/proc/{}/root", self.reaper_pid)),
            self.plan.runtime_root_identity,
            "bubblewrap PID1 reaper runtime root",
        )?;
        // Recheck after procfs reads so a departing setup process cannot make a
        // partial process-chain snapshot appear complete.
        validate_namespace_cgroup_members(&cgroup.member_pids()?, expected)?;
        Ok(namespaces)
    }

    pub(super) fn attest_live(&self, cgroup: &InrouCgroupAttestation) -> eyre::Result<()> {
        if self.attest_process_chain(cgroup)? != self.namespaces {
            eyre::bail!("nested QEMU namespace identities changed after launch");
        }
        self.plan.attest_mounts_and_root(self.qemu_pid, false)?;
        attest_private_loopback_only(self.qemu_pid)
    }

    pub(super) fn connect_private_loopback(
        &self,
        cgroup: &InrouCgroupAttestation,
        expected_backend: SocketAddr,
    ) -> eyre::Result<TcpStream> {
        if expected_backend.ip() != IpAddr::V4(Ipv4Addr::LOCALHOST) || expected_backend.port() == 0
        {
            eyre::bail!(
                "Inrou private-network connector requires one concrete IPv4 loopback endpoint"
            );
        }
        self.attest_live(cgroup)?;
        let namespace_path = PathBuf::from(format!("/proc/{}/ns/net", self.qemu_pid));
        let namespace = fs::File::open(&namespace_path).wrap_err_with(|| {
            format!(
                "open attested Inrou network namespace {}",
                namespace_path.display()
            )
        })?;
        let expected_namespace = self
            .namespaces
            .get("net")
            .copied()
            .ok_or_else(|| eyre::eyre!("Inrou namespace attestation omitted `net`"))?;
        if namespace.metadata()?.ino() != expected_namespace
            || read_namespace_id(&namespace_path)? != expected_namespace
        {
            eyre::bail!("Inrou network namespace changed while its connector was opened");
        }
        let connector = std::thread::Builder::new()
            .name("inrou-private-net-connect".to_owned())
            .stack_size(INROU_NAMESPACE_CONNECT_THREAD_STACK_BYTES)
            .spawn(move || -> io::Result<TcpStream> {
                enter_inrou_network_namespace(&namespace)?;
                TcpStream::connect_timeout(&expected_backend, INROU_NAMESPACE_CONNECT_TIMEOUT)
            })
            .wrap_err("spawn bounded Inrou private-network connector")?;
        let stream = connector
            .join()
            .map_err(|panic| eyre::eyre!("Inrou private-network connector panicked: {panic:?}"))?
            .wrap_err("connect to the QEMU-owned loopback forward inside its private namespace")?;
        if stream.peer_addr()? != expected_backend
            || stream.local_addr()?.ip() != IpAddr::V4(Ipv4Addr::LOCALHOST)
        {
            eyre::bail!("Inrou private-network connector reached an unexpected endpoint");
        }
        self.attest_live(cgroup)?;
        Ok(stream)
    }
}

#[allow(unsafe_code)]
fn enter_inrou_network_namespace(namespace: &fs::File) -> io::Result<()> {
    unsafe extern "C" {
        fn setns(fd: i32, namespace_type: i32) -> i32;
    }
    const CLONE_NEWNET: i32 = 0x4000_0000;
    // SAFETY: the descriptor is a live, already-attested nsfs network
    // namespace handle. `setns` changes only this newly-created connector
    // thread, which exits immediately after creating one fixed loopback TCP
    // socket and never returns to the supervisor's thread pool.
    if unsafe { setns(namespace.as_raw_fd(), CLONE_NEWNET) } == -1 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

pub(super) fn inrou_namespaced_setpriv_arguments(
    identity: &PortableVmChildIdentity,
) -> Vec<OsString> {
    let mut arguments = vec![
        "--reuid".into(),
        identity.uid.to_string().into(),
        "--regid".into(),
        identity.gid.to_string().into(),
    ];
    if identity.supplementary_gids.is_empty() {
        arguments.push("--clear-groups".into());
    } else {
        arguments.push("--groups".into());
        arguments.push(
            identity
                .supplementary_gids
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(",")
                .into(),
        );
    }
    arguments.extend(
        [
            "--securebits=+noroot,+noroot_locked,+no_setuid_fixup,+no_setuid_fixup_locked",
            "--bounding-set=-all",
            "--inh-caps=-all",
            "--ambient-caps=-all",
            "--no-new-privs",
            "--",
        ]
        .into_iter()
        .map(OsString::from),
    );
    arguments
}

fn validate_pinned_root_executable(path: &Path, label: &str) -> eyre::Result<PathBuf> {
    if !path.is_absolute() {
        eyre::bail!("pinned Inrou {label} path must be absolute");
    }
    let named = fs::symlink_metadata(path)
        .wrap_err_with(|| format!("inspect pinned Inrou {label} {}", path.display()))?;
    if named.file_type().is_symlink() || !named.is_file() || named.uid() != 0 {
        eyre::bail!("pinned Inrou {label} must be one direct root-owned regular file");
    }
    validate_root_custodied_ancestors(path, label)?;
    if named.mode() & 0o111 == 0 || named.mode() & 0o022 != 0 {
        eyre::bail!("pinned Inrou {label} must be executable and not group/other writable");
    }
    Ok(path.to_path_buf())
}

fn validate_root_custodied_ancestors(path: &Path, label: &str) -> eyre::Result<()> {
    for ancestor in path.ancestors().skip(1) {
        let metadata = fs::symlink_metadata(ancestor).wrap_err_with(|| {
            format!(
                "inspect pinned Inrou {label} ancestor {}",
                ancestor.display()
            )
        })?;
        if metadata.file_type().is_symlink()
            || !metadata.is_dir()
            || metadata.uid() != 0
            || metadata.mode() & 0o022 != 0
        {
            eyre::bail!(
                "pinned Inrou {label} ancestor {} is not a direct non-writable root directory",
                ancestor.display()
            );
        }
    }
    Ok(())
}

fn inspect_root_runtime_file(path: &Path, label: &str) -> eyre::Result<InrouFileIdentity> {
    let metadata = fs::symlink_metadata(path)
        .wrap_err_with(|| format!("inspect {label} {}", path.display()))?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.uid() != 0
        || metadata.mode() & 0o022 != 0
    {
        eyre::bail!(
            "{label} {} must be a direct root-owned regular file not writable by group or other",
            path.display()
        );
    }
    Ok(file_identity(&metadata, InrouFileKind::Regular))
}

fn inspect_delegated_input_file(
    path: &Path,
    file: &fs::File,
    child_gid: u32,
    writable: bool,
) -> eyre::Result<InrouFileIdentity> {
    let metadata = require_opened_file_matches_name(path, file, "Inrou namespace input")?;
    let expected_mode = if writable { 0o660 } else { 0o640 };
    if !metadata.is_file()
        || metadata.uid() != 0
        || metadata.gid() != child_gid
        || metadata.nlink() != 1
        || metadata.mode() & 0o7777 != expected_mode
    {
        eyre::bail!(
            "Inrou namespace input {} must retain exact root:{child_gid} {expected_mode:o} singly-linked custody",
            path.display()
        );
    }
    Ok(file_identity(&metadata, InrouFileKind::Regular))
}

fn require_opened_file_matches_name(
    path: &Path,
    file: &fs::File,
    label: &str,
) -> eyre::Result<fs::Metadata> {
    let named = fs::symlink_metadata(path)
        .wrap_err_with(|| format!("inspect {label} {}", path.display()))?;
    let opened = file
        .metadata()
        .wrap_err_with(|| format!("inspect opened {label} {}", path.display()))?;
    if named.file_type().is_symlink() || named.dev() != opened.dev() || named.ino() != opened.ino()
    {
        eyre::bail!(
            "opened {label} {} no longer matches its direct name",
            path.display()
        );
    }
    Ok(opened)
}

fn validate_opened_file_identity(
    file: &fs::File,
    expected: InrouFileIdentity,
    label: &str,
) -> eyre::Result<()> {
    let metadata = file
        .metadata()
        .wrap_err_with(|| format!("inspect opened {label}"))?;
    validate_metadata_identity(Path::new("<inherited-fd>"), &metadata, expected, label)
}

fn file_identity(metadata: &fs::Metadata, kind: InrouFileKind) -> InrouFileIdentity {
    InrouFileIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
        mode: metadata.mode() & 0o7777,
        uid: metadata.uid(),
        gid: metadata.gid(),
        links: metadata.nlink(),
        size: metadata.size(),
        kind,
    }
}

fn validate_exact_file_identity(
    path: &Path,
    expected: InrouFileIdentity,
    label: &str,
) -> eyre::Result<()> {
    let metadata = fs::symlink_metadata(path)
        .wrap_err_with(|| format!("inspect {label} {}", path.display()))?;
    if metadata.file_type().is_symlink() {
        eyre::bail!("{label} {} changed into a symbolic link", path.display());
    }
    validate_metadata_identity(path, &metadata, expected, label)
}

fn validate_followed_file_identity(
    path: &Path,
    expected: InrouFileIdentity,
    label: &str,
) -> eyre::Result<()> {
    let metadata =
        fs::metadata(path).wrap_err_with(|| format!("inspect {label} {}", path.display()))?;
    validate_metadata_identity(path, &metadata, expected, label)
}

fn validate_metadata_identity(
    path: &Path,
    metadata: &fs::Metadata,
    expected: InrouFileIdentity,
    label: &str,
) -> eyre::Result<()> {
    let kind = if metadata.is_file() {
        InrouFileKind::Regular
    } else if metadata.file_type().is_char_device() {
        InrouFileKind::Character
    } else if metadata.is_dir() {
        InrouFileKind::Directory
    } else {
        eyre::bail!("{label} {} changed file type", path.display());
    };
    let actual = file_identity(metadata, kind);
    if actual != expected {
        eyre::bail!(
            "{label} {} changed identity from {:?} to {:?}",
            path.display(),
            expected,
            actual
        );
    }
    Ok(())
}

fn validate_live_binding_identity(
    path: &Path,
    binding: &InrouNamespaceBinding,
) -> eyre::Result<()> {
    let metadata = fs::symlink_metadata(path)
        .wrap_err_with(|| format!("inspect nested QEMU root binding {}", path.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        eyre::bail!(
            "nested QEMU root binding {} changed file type",
            path.display()
        );
    }
    let mut actual = file_identity(&metadata, InrouFileKind::Regular);
    if binding.writable {
        // A qcow2 file allocates clusters while QEMU runs, so its physical
        // length is expected to change. Every custody property, including the
        // exact device/inode and single-link identity, remains immutable.
        actual.size = binding.identity.size;
    }
    if actual != binding.identity {
        eyre::bail!(
            "nested QEMU root binding {} changed custody from {:?} to {:?}",
            path.display(),
            binding.identity,
            actual,
        );
    }
    Ok(())
}

fn validate_sandbox_binding_path(path: &Path) -> eyre::Result<()> {
    if !path.is_absolute() || path == Path::new("/") {
        eyre::bail!("Inrou sandbox binding target must be a non-root absolute path");
    }
    for component in path.components() {
        match component {
            Component::RootDir | Component::Normal(_) => {}
            Component::CurDir | Component::ParentDir | Component::Prefix(_) => {
                eyre::bail!("Inrou sandbox binding target is not canonical")
            }
        }
    }
    if path.starts_with("/proc")
        || path.starts_with("/dev")
        || path.starts_with("/tmp")
        || path.starts_with("/run")
        || path.starts_with("/sys")
    {
        eyre::bail!("Inrou sandbox file binding overlaps a reserved private filesystem");
    }
    Ok(())
}

fn inspect_root_runtime_directory(path: &Path) -> eyre::Result<InrouFileIdentity> {
    let metadata = fs::symlink_metadata(path)
        .wrap_err_with(|| format!("inspect fixed Inrou runtime root {}", path.display()))?;
    if metadata.file_type().is_symlink()
        || !metadata.is_dir()
        || metadata.uid() != 0
        || metadata.gid() != 0
        || metadata.mode() & 0o7777 != 0o555
    {
        eyre::bail!("fixed Inrou runtime root must be a direct root:root 0555 directory");
    }
    validate_root_custodied_ancestors(path, "runtime root")?;
    Ok(file_identity(&metadata, InrouFileKind::Directory))
}

fn load_and_attest_runtime_manifest(
    runtime_root: &Path,
    manifest_path: &Path,
) -> eyre::Result<Vec<InrouRuntimeManifestEntry>> {
    validate_root_custodied_ancestors(manifest_path, "runtime manifest")?;
    let manifest_metadata = fs::symlink_metadata(manifest_path)
        .wrap_err_with(|| format!("inspect Inrou runtime manifest {}", manifest_path.display()))?;
    if manifest_metadata.file_type().is_symlink()
        || !manifest_metadata.is_file()
        || manifest_metadata.uid() != 0
        || manifest_metadata.gid() != 0
        || manifest_metadata.nlink() != 1
        || manifest_metadata.mode() & 0o7777 != 0o444
        || manifest_metadata.size() > INROU_RUNTIME_MANIFEST_MAX_BYTES
    {
        eyre::bail!("Inrou runtime manifest must be a bounded root:root 0444 direct file");
    }
    let contents = read_bounded_file(
        manifest_path,
        INROU_RUNTIME_MANIFEST_MAX_BYTES,
        "Inrou runtime manifest",
    )?;
    let mut entries = parse_runtime_manifest(&contents)?;
    attest_runtime_tree_exhaustive(runtime_root, &mut entries)?;
    Ok(entries)
}

fn parse_runtime_manifest(contents: &str) -> eyre::Result<Vec<InrouRuntimeManifestEntry>> {
    if !contents.ends_with('\n') || contents.contains('\r') {
        eyre::bail!("Inrou runtime manifest must use LF records with a final newline");
    }
    let mut lines = contents.lines();
    if lines.next() != Some(INROU_RUNTIME_MANIFEST_HEADER) {
        eyre::bail!("Inrou runtime manifest has the wrong fixed header");
    }
    let mut entries = Vec::new();
    let mut previous_path: Option<PathBuf> = None;
    for line in lines {
        if line.is_empty() || line.trim() != line || line.contains("  ") {
            eyre::bail!("Inrou runtime manifest contains a non-canonical record");
        }
        let fields = line.split(' ').collect::<Vec<_>>();
        let [kind, digest, exact_bytes, mode, path] = fields.as_slice() else {
            eyre::bail!("Inrou runtime manifest record must contain exactly five fields");
        };
        if !path
            .bytes()
            .all(|byte| (0x21..=0x7e).contains(&byte) && byte != b'\\')
        {
            eyre::bail!("Inrou runtime manifest path must use visible ASCII without backslashes");
        }
        let sandbox_path = PathBuf::from(path);
        validate_runtime_manifest_path(&sandbox_path)?;
        if previous_path
            .as_ref()
            .is_some_and(|previous| previous >= &sandbox_path)
        {
            eyre::bail!("Inrou runtime manifest paths must be strictly sorted and unique");
        }
        previous_path = Some(sandbox_path.clone());
        let exact_bytes = parse_canonical_decimal(exact_bytes, "runtime byte length")?;
        let mode = parse_canonical_mode(mode)?;
        let (kind, sha256) = match *kind {
            "d" => {
                if *digest != "-" || exact_bytes != 0 || mode != 0o555 {
                    eyre::bail!("Inrou runtime directory records must be `d - 0 0555 PATH`");
                }
                (InrouRuntimeEntryKind::Directory, None)
            }
            "f" => {
                if mode != 0o444 && mode != 0o555 {
                    eyre::bail!("Inrou runtime file mode must be exactly 0444 or 0555");
                }
                (
                    InrouRuntimeEntryKind::Regular,
                    Some(parse_sha256_hex(digest)?),
                )
            }
            _ => eyre::bail!("Inrou runtime manifest kind must be `d` or `f`"),
        };
        entries.push(InrouRuntimeManifestEntry {
            kind,
            sandbox_path,
            sha256,
            exact_bytes,
            mode,
            identity: None,
        });
        if entries.len() > INROU_RUNTIME_MAX_FILES {
            eyre::bail!("Inrou runtime manifest exceeds its fixed entry limit");
        }
    }
    if entries.is_empty()
        || entries
            .first()
            .is_none_or(|entry| entry.sandbox_path != Path::new("/"))
    {
        eyre::bail!("Inrou runtime manifest must begin with the root directory record");
    }
    Ok(entries)
}

fn require_runtime_manifest_layout(entries: &[InrouRuntimeManifestEntry]) -> eyre::Result<()> {
    for required in [
        "/",
        "/dev",
        "/proc",
        "/tmp",
        "/inrou",
        "/inrou/bin",
        "/inrou/input",
        INROU_NAMESPACE_CLOUD_INIT_ROOT,
        "/inrou/disk",
    ] {
        let matches = entries
            .iter()
            .filter(|entry| entry.sandbox_path == Path::new(required));
        let Some(entry) = matches.into_iter().next() else {
            eyre::bail!("Inrou runtime manifest omitted required directory `{required}`");
        };
        if entry.kind != InrouRuntimeEntryKind::Directory || entry.mode != 0o555 {
            eyre::bail!("Inrou runtime manifest `{required}` must be a 0555 directory");
        }
    }
    let mut placeholders = vec![
        INROU_NAMESPACE_KERNEL_PATH.to_owned(),
        INROU_NAMESPACE_INITRD_PATH.to_owned(),
        INROU_NAMESPACE_BUNDLE_PATH.to_owned(),
        format!("{INROU_NAMESPACE_CLOUD_INIT_ROOT}/meta-data"),
        format!("{INROU_NAMESPACE_CLOUD_INIT_ROOT}/network-config"),
        format!("{INROU_NAMESPACE_CLOUD_INIT_ROOT}/user-data"),
        INROU_NAMESPACE_ROOT_DISK_PATH.to_owned(),
    ];
    placeholders.extend(
        (0..INROU_NAMESPACE_MAX_LEASE_DISKS).map(|index| format!("/inrou/disk/lease{index}")),
    );
    for placeholder in placeholders {
        let Some(entry) = entries
            .iter()
            .find(|entry| entry.sandbox_path == Path::new(&placeholder))
        else {
            eyre::bail!("Inrou runtime manifest omitted placeholder `{placeholder}`");
        };
        if entry.kind != InrouRuntimeEntryKind::Regular
            || entry.mode != 0o444
            || entry.exact_bytes != 0
        {
            eyre::bail!("Inrou runtime placeholder `{placeholder}` must be an empty 0444 file");
        }
    }
    Ok(())
}

fn require_exact_binding_layout(bindings: &[InrouNamespaceBinding]) -> eyre::Result<()> {
    if bindings.len() > INROU_NAMESPACE_MAX_PRODUCTION_BINDINGS {
        eyre::bail!("Inrou namespace binding plan exceeds the exact V1 surface");
    }
    for (path, writable) in INROU_NAMESPACE_MANDATORY_PRODUCTION_BINDINGS {
        let Some(binding) = bindings
            .iter()
            .find(|binding| binding.sandbox_path == Path::new(path))
        else {
            eyre::bail!("Inrou namespace binding plan omitted mandatory `{path}`");
        };
        if binding.writable != writable {
            eyre::bail!("Inrou namespace binding `{path}` has the wrong write posture");
        }
    }

    let mut leases = BTreeSet::new();
    for binding in bindings {
        let path = binding.sandbox_path.as_path();
        if INROU_NAMESPACE_MANDATORY_PRODUCTION_BINDINGS
            .iter()
            .any(|(required, _)| path == Path::new(required))
        {
            continue;
        }
        if path == Path::new(INROU_NAMESPACE_INITRD_PATH) {
            if binding.writable {
                eyre::bail!("Inrou initrd binding must be read-only");
            }
            continue;
        }
        let Some(index) = path
            .to_str()
            .and_then(|path| path.strip_prefix("/inrou/disk/lease"))
            .and_then(|index| parse_canonical_decimal(index, "lease binding index").ok())
            .and_then(|index| usize::try_from(index).ok())
        else {
            eyre::bail!(
                "Inrou namespace binding {} is outside the exact V1 input/disk surface",
                path.display()
            );
        };
        if !binding.writable || index >= INROU_NAMESPACE_MAX_LEASE_DISKS || !leases.insert(index) {
            eyre::bail!("Inrou namespace lease binding is repeated, read-only, or out of range");
        }
    }
    for expected in 0..leases.len() {
        if !leases.contains(&expected) {
            eyre::bail!("Inrou namespace lease bindings must be contiguous from lease0");
        }
    }
    Ok(())
}

fn parse_canonical_decimal(value: &str, label: &str) -> eyre::Result<u64> {
    if value.is_empty()
        || value.bytes().any(|byte| !byte.is_ascii_digit())
        || (value.len() > 1 && value.starts_with('0'))
    {
        eyre::bail!("{label} is not canonical decimal");
    }
    value.parse().wrap_err_with(|| format!("parse {label}"))
}

fn parse_canonical_mode(value: &str) -> eyre::Result<u32> {
    if value.len() != 4 || !value.bytes().all(|byte| (b'0'..=b'7').contains(&byte)) {
        eyre::bail!("Inrou runtime mode must contain four octal digits");
    }
    u32::from_str_radix(value, 8).wrap_err("parse Inrou runtime mode")
}

fn parse_sha256_hex(value: &str) -> eyre::Result<[u8; 32]> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        eyre::bail!("Inrou runtime SHA-256 must be 64 lowercase hexadecimal digits");
    }
    let decoded = hex::decode(value).wrap_err("decode Inrou runtime SHA-256")?;
    decoded
        .try_into()
        .map_err(|_bytes: Vec<u8>| eyre::eyre!("Inrou runtime SHA-256 has the wrong length"))
}

fn validate_runtime_manifest_path(path: &Path) -> eyre::Result<()> {
    if path == Path::new("/") {
        return Ok(());
    }
    validate_sandbox_binding_path_for_tree(path)?;
    if path.starts_with("/run") || path.starts_with("/sys") {
        eyre::bail!("Inrou runtime closure must not contain `/run` or `/sys`");
    }
    Ok(())
}

fn runtime_host_path(runtime_root: &Path, sandbox_path: &Path) -> eyre::Result<PathBuf> {
    validate_runtime_manifest_path(sandbox_path)?;
    Ok(runtime_root.join(
        sandbox_path
            .strip_prefix("/")
            .wrap_err("strip runtime sandbox root")?,
    ))
}

fn expected_minimal_root_entries(
    runtime_entries: &[InrouRuntimeManifestEntry],
    bindings: &[InrouNamespaceBinding],
) -> eyre::Result<BTreeMap<PathBuf, BTreeSet<OsString>>> {
    let mut entries = BTreeMap::<PathBuf, BTreeSet<OsString>>::new();
    for runtime_entry in runtime_entries {
        if runtime_entry.sandbox_path != Path::new("/") {
            insert_expected_path(
                &mut entries,
                &runtime_entry.sandbox_path,
                runtime_entry.kind,
            )?;
        }
    }
    for binding in bindings {
        insert_expected_path(
            &mut entries,
            &binding.sandbox_path,
            InrouRuntimeEntryKind::Regular,
        )?;
    }
    Ok(entries)
}

fn insert_expected_path(
    entries: &mut BTreeMap<PathBuf, BTreeSet<OsString>>,
    path: &Path,
    kind: InrouRuntimeEntryKind,
) -> eyre::Result<()> {
    validate_sandbox_binding_path_for_tree(path)?;
    let mut current = PathBuf::from("/");
    entries.entry(current.clone()).or_default();
    let mut components = path.components().skip(1).peekable();
    while let Some(component) = components.next() {
        let Component::Normal(name) = component else {
            eyre::bail!("Inrou minimal-root path is not canonical");
        };
        if !entries
            .entry(current.clone())
            .or_default()
            .insert(name.to_owned())
        {
            // Reusing an ancestor is expected; repeated final paths were
            // rejected before this trie is built.
        }
        current.push(name);
        // Every path is a child of its parent, but regular-file leaves are not
        // directories to enumerate. Their identity/content checks run separately.
        if components.peek().is_some() || kind == InrouRuntimeEntryKind::Directory {
            entries.entry(current.clone()).or_default();
        }
    }
    Ok(())
}

fn validate_sandbox_binding_path_for_tree(path: &Path) -> eyre::Result<()> {
    if !path.is_absolute() || path == Path::new("/") {
        eyre::bail!("Inrou minimal-root path must be absolute and non-root");
    }
    if path
        .components()
        .any(|component| !matches!(component, Component::RootDir | Component::Normal(_)))
    {
        eyre::bail!("Inrou minimal-root path is not canonical");
    }
    let text = path
        .to_str()
        .ok_or_else(|| eyre::eyre!("Inrou minimal-root path must be UTF-8"))?;
    if text.contains("//") || text.ends_with('/') {
        eyre::bail!("Inrou minimal-root path contains redundant separators");
    }
    Ok(())
}

fn attest_runtime_tree_exhaustive(
    runtime_root: &Path,
    entries: &mut [InrouRuntimeManifestEntry],
) -> eyre::Result<()> {
    let mut actual_paths = Vec::new();
    collect_runtime_tree_paths(runtime_root, runtime_root, &mut actual_paths)?;
    actual_paths.sort();
    let expected_paths = entries
        .iter()
        .filter(|entry| entry.sandbox_path != Path::new("/"))
        .map(|entry| entry.sandbox_path.clone())
        .collect::<Vec<_>>();
    if actual_paths != expected_paths {
        eyre::bail!(
            "Inrou runtime manifest is not exhaustive; tree paths {:?}, manifest paths {:?}",
            actual_paths,
            expected_paths
        );
    }
    let mut total_bytes = 0_u64;
    for entry in entries {
        let host_path = if entry.sandbox_path == Path::new("/") {
            runtime_root.to_path_buf()
        } else {
            runtime_host_path(runtime_root, &entry.sandbox_path)?
        };
        attest_runtime_manifest_entry_full(&host_path, entry)?;
        if entry.kind == InrouRuntimeEntryKind::Regular {
            total_bytes = total_bytes
                .checked_add(entry.exact_bytes)
                .ok_or_else(|| eyre::eyre!("Inrou runtime aggregate byte length overflow"))?;
            if total_bytes > INROU_RUNTIME_TOTAL_MAX_BYTES {
                eyre::bail!("Inrou runtime tree exceeds its aggregate byte limit");
            }
        }
    }
    Ok(())
}

fn collect_runtime_tree_paths(
    runtime_root: &Path,
    directory: &Path,
    paths: &mut Vec<PathBuf>,
) -> eyre::Result<()> {
    let mut children = fs::read_dir(directory)
        .wrap_err_with(|| format!("enumerate Inrou runtime directory {}", directory.display()))?
        .collect::<Result<Vec<_>, _>>()?;
    children.sort_by_key(fs::DirEntry::file_name);
    for child in children {
        if paths.len() >= INROU_RUNTIME_MAX_FILES {
            eyre::bail!("Inrou runtime tree exceeds its fixed entry limit");
        }
        let host_path = child.path();
        let metadata = fs::symlink_metadata(&host_path)?;
        if metadata.file_type().is_symlink()
            || (!metadata.is_dir() && !metadata.is_file())
            || metadata.uid() != 0
            || metadata.gid() != 0
        {
            eyre::bail!(
                "Inrou runtime tree entry {} must be a direct root:root directory or regular file",
                host_path.display()
            );
        }
        let relative = host_path.strip_prefix(runtime_root)?;
        let sandbox_path = Path::new("/").join(relative);
        paths.push(sandbox_path);
        if metadata.is_dir() {
            collect_runtime_tree_paths(runtime_root, &host_path, paths)?;
        }
    }
    Ok(())
}

fn attest_runtime_manifest_entry_full(
    path: &Path,
    entry: &mut InrouRuntimeManifestEntry,
) -> eyre::Result<()> {
    let metadata = fs::symlink_metadata(path)
        .wrap_err_with(|| format!("inspect runtime manifest entry {}", path.display()))?;
    let identity = match entry.kind {
        InrouRuntimeEntryKind::Directory => {
            if metadata.file_type().is_symlink()
                || !metadata.is_dir()
                || metadata.uid() != 0
                || metadata.gid() != 0
                || metadata.mode() & 0o7777 != entry.mode
            {
                eyre::bail!(
                    "Inrou runtime directory {} violates its manifest",
                    path.display()
                );
            }
            file_identity(&metadata, InrouFileKind::Directory)
        }
        InrouRuntimeEntryKind::Regular => {
            if metadata.file_type().is_symlink()
                || !metadata.is_file()
                || metadata.uid() != 0
                || metadata.gid() != 0
                || metadata.nlink() != 1
                || metadata.mode() & 0o7777 != entry.mode
                || metadata.size() != entry.exact_bytes
                || metadata.size() > INROU_RUNTIME_FILE_MAX_BYTES
            {
                eyre::bail!(
                    "Inrou runtime file {} violates its manifest",
                    path.display()
                );
            }
            let mut options = fs::OpenOptions::new();
            options.read(true).custom_flags(
                (rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::CLOEXEC).bits() as i32,
            );
            let file = options.open(path)?;
            let opened = file.metadata()?;
            if opened.dev() != metadata.dev() || opened.ino() != metadata.ino() {
                eyre::bail!("Inrou runtime file changed while it was opened");
            }
            let (actual_hash, actual_bytes) =
                sha256_reader_bounded(file, INROU_RUNTIME_FILE_MAX_BYTES)?;
            if entry.sha256 != Some(actual_hash) || actual_bytes != entry.exact_bytes {
                eyre::bail!(
                    "Inrou runtime file {} failed SHA-256 attestation",
                    path.display()
                );
            }
            file_identity(&metadata, InrouFileKind::Regular)
        }
    };
    entry.identity = Some(identity);
    Ok(())
}

fn attest_runtime_manifest_entries(
    runtime_root: &Path,
    entries: &[InrouRuntimeManifestEntry],
) -> eyre::Result<()> {
    for entry in entries {
        let path = if entry.sandbox_path == Path::new("/") {
            runtime_root.to_path_buf()
        } else {
            runtime_host_path(runtime_root, &entry.sandbox_path)?
        };
        let expected = entry
            .identity
            .ok_or_else(|| eyre::eyre!("runtime manifest entry lacks an attested identity"))?;
        validate_exact_file_identity(&path, expected, "Inrou runtime manifest entry")?;
        if entry.kind == InrouRuntimeEntryKind::Regular {
            authenticate_runtime_manifest_file(&path, entry)?;
        }
    }
    Ok(())
}

fn attest_runtime_manifest_file(
    path: &Path,
    entry: &InrouRuntimeManifestEntry,
    authenticate_contents: bool,
) -> eyre::Result<()> {
    let expected = entry
        .identity
        .ok_or_else(|| eyre::eyre!("runtime manifest file lacks an attested identity"))?;
    validate_exact_file_identity(path, expected, "nested QEMU runtime file")?;
    if authenticate_contents {
        authenticate_runtime_manifest_file(path, entry)?;
    }
    Ok(())
}

fn authenticate_runtime_manifest_file(
    path: &Path,
    entry: &InrouRuntimeManifestEntry,
) -> eyre::Result<()> {
    let mut options = fs::OpenOptions::new();
    options
        .read(true)
        .custom_flags((rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::CLOEXEC).bits() as i32);
    let file = options
        .open(path)
        .wrap_err_with(|| format!("open authenticated Inrou runtime file {}", path.display()))?;
    let expected = entry
        .identity
        .ok_or_else(|| eyre::eyre!("authenticated runtime file lacks an attested identity"))?;
    let opened = file.metadata()?;
    if file_identity(&opened, InrouFileKind::Regular) != expected {
        eyre::bail!("authenticated Inrou runtime file changed while it was opened");
    }
    let (actual_hash, actual_bytes) = sha256_reader_bounded(file, INROU_RUNTIME_FILE_MAX_BYTES)?;
    if entry.sha256 != Some(actual_hash) || entry.exact_bytes != actual_bytes {
        eyre::bail!(
            "authenticated Inrou runtime file {} changed content",
            path.display()
        );
    }
    Ok(())
}

fn proc_executable_identity(pid: u32) -> eyre::Result<InrouFileIdentity> {
    let metadata = fs::metadata(format!("/proc/{pid}/exe"))?;
    if !metadata.is_file() {
        eyre::bail!("procfs executable is not a regular file");
    }
    Ok(file_identity(&metadata, InrouFileKind::Regular))
}

fn validate_proc_executable(
    pid: u32,
    expected: InrouFileIdentity,
    label: &str,
) -> eyre::Result<()> {
    let actual = proc_executable_identity(pid)?;
    if actual != expected {
        eyre::bail!("{label} pid {pid} changed executable identity");
    }
    Ok(())
}

fn parse_namespace_process_status(
    status: &str,
    pid: u32,
) -> eyre::Result<InrouNamespaceProcessStatus> {
    let recorded_pid = super::inrou_proc_status_field(status, "Pid")?
        .trim()
        .parse::<u32>()
        .wrap_err("parse Inrou namespace process pid")?;
    if pid == 0 || recorded_pid != pid {
        eyre::bail!("Inrou namespace process status changed its observed pid");
    }
    let parent_pid = super::inrou_proc_status_field(status, "PPid")?
        .trim()
        .parse::<u32>()
        .wrap_err("parse Inrou namespace process parent pid")?;
    let namespace_pids = super::inrou_proc_status_ids(status, "NSpid")?;
    if namespace_pids.first() != Some(&pid) || namespace_pids.contains(&0) {
        eyre::bail!("Inrou namespace process status has an invalid PID namespace chain");
    }
    Ok(InrouNamespaceProcessStatus {
        pid,
        parent_pid,
        namespace_pids,
        uids: super::inrou_proc_status_quad_ids(status, "Uid")?,
        gids: super::inrou_proc_status_quad_ids(status, "Gid")?,
    })
}

fn validate_namespace_cgroup_members(
    members: &[u32],
    mut expected: [u32; INROU_NAMESPACE_MAX_CGROUP_PIDS],
) -> eyre::Result<()> {
    expected.sort_unstable();
    if expected[0] == 0 || expected.windows(2).any(|pair| pair[0] == pair[1]) || members != expected
    {
        eyre::bail!("Inrou namespace cgroup membership changed to {:?}", members);
    }
    Ok(())
}

fn validate_namespace_process_chain_status(
    supervisor_pid: u32,
    statuses: &[InrouNamespaceProcessStatus],
) -> eyre::Result<()> {
    let [watchdog, bubblewrap, reaper, qemu] = statuses else {
        eyre::bail!("Inrou namespace process chain must contain exactly four processes");
    };
    let mut parent_pid = supervisor_pid;
    for process in statuses {
        if process.pid == supervisor_pid || process.parent_pid != parent_pid {
            eyre::bail!("Inrou namespace process chain changed its exact parent relationship");
        }
        parent_pid = process.pid;
    }
    for process in [watchdog, bubblewrap, reaper] {
        if process.uids != [0; 4] || process.gids != [0; 4] {
            eyre::bail!("Inrou namespace watchdog and bubblewrap must retain root uid and gid");
        }
    }
    if bubblewrap.namespace_pids.len() != watchdog.namespace_pids.len()
        || reaper.namespace_pids.len() != watchdog.namespace_pids.len() + 1
        || qemu.namespace_pids.len() != reaper.namespace_pids.len()
        || reaper.namespace_pids.last() != Some(&1)
        || !qemu.namespace_pids.last().is_some_and(|pid| *pid > 1)
    {
        eyre::bail!(
            "Inrou namespace process chain requires bubblewrap PID1 and a distinct QEMU child PID"
        );
    }
    Ok(())
}

fn attest_private_namespace_set(pid: u32) -> eyre::Result<BTreeMap<&'static str, u64>> {
    let mut namespaces = BTreeMap::new();
    for namespace in ["cgroup", "mnt", "net", "ipc", "uts", "pid"] {
        let process = read_namespace_id(&PathBuf::from(format!("/proc/{pid}/ns/{namespace}")))?;
        let supervisor = read_namespace_id(&PathBuf::from(format!("/proc/self/ns/{namespace}")))?;
        if process == supervisor {
            eyre::bail!("nested QEMU retained the supervisor's {namespace} namespace");
        }
        namespaces.insert(namespace, process);
    }
    Ok(namespaces)
}

fn attest_private_loopback_only(pid: u32) -> eyre::Result<()> {
    let contents = read_bounded_proc_file(
        &PathBuf::from(format!("/proc/{pid}/net/dev")),
        "nested QEMU network-device table",
    )?;
    validate_loopback_only_network_devices(&contents)
}

fn validate_loopback_only_network_devices(contents: &str) -> eyre::Result<()> {
    let mut lines = contents.lines();
    let Some(first_header) = lines.next() else {
        eyre::bail!("nested QEMU network-device table omitted its first header");
    };
    let Some(second_header) = lines.next() else {
        eyre::bail!("nested QEMU network-device table omitted its second header");
    };
    if !first_header.contains("Inter-")
        || !first_header.contains("Receive")
        || !second_header.contains("bytes")
    {
        eyre::bail!("nested QEMU network-device table has an unexpected header");
    }
    let mut interfaces = BTreeSet::new();
    for line in lines {
        let (name, counters) = line
            .split_once(':')
            .ok_or_else(|| eyre::eyre!("nested QEMU network-device record omitted `:`"))?;
        let name = name.trim();
        if name.is_empty()
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
            || !interfaces.insert(name.to_owned())
            || counters.split_ascii_whitespace().count() != 16
        {
            eyre::bail!("nested QEMU network-device table contains a malformed record");
        }
    }
    if interfaces != BTreeSet::from(["lo".to_owned()]) {
        eyre::bail!(
            "nested QEMU network namespace exposes interfaces {:?} instead of only loopback",
            interfaces
        );
    }
    Ok(())
}

fn read_namespace_id(path: &Path) -> eyre::Result<u64> {
    let link = fs::read_link(path)
        .wrap_err_with(|| format!("read namespace identity {}", path.display()))?;
    parse_namespace_link(&link.to_string_lossy())
}

fn parse_namespace_link(link: &str) -> eyre::Result<u64> {
    let (kind, inode) = link
        .split_once('[')
        .ok_or_else(|| eyre::eyre!("namespace link omitted `[`"))?;
    if kind.is_empty()
        || !kind.ends_with(':')
        || !kind[..kind.len() - 1]
            .bytes()
            .all(|byte| byte.is_ascii_lowercase())
    {
        eyre::bail!("namespace link has a non-canonical kind");
    }
    let inode = inode
        .strip_suffix(']')
        .ok_or_else(|| eyre::eyre!("namespace link omitted closing `]`"))?;
    if inode.is_empty() || inode.bytes().any(|byte| !byte.is_ascii_digit()) {
        eyre::bail!("namespace link inode is not canonical decimal");
    }
    inode.parse().wrap_err("parse namespace inode")
}

fn inspect_kvm_device(path: &Path) -> eyre::Result<InrouFileIdentity> {
    let metadata = fs::symlink_metadata(path)
        .wrap_err_with(|| format!("inspect mandatory KVM device {}", path.display()))?;
    if metadata.file_type().is_symlink()
        || !metadata.file_type().is_char_device()
        || metadata.uid() != 0
        || metadata.nlink() != 1
        || metadata.rdev() == 0
        || metadata.mode() & 0o002 != 0
    {
        eyre::bail!("mandatory `/dev/kvm` must be one direct root-owned character device");
    }
    Ok(file_identity(&metadata, InrouFileKind::Character))
}

fn read_bounded_file(path: &Path, maximum_bytes: u64, label: &str) -> eyre::Result<String> {
    let mut bytes = Vec::new();
    fs::File::open(path)
        .wrap_err_with(|| format!("open {label} {}", path.display()))?
        .take(maximum_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .wrap_err_with(|| format!("read {label} {}", path.display()))?;
    if u64::try_from(bytes.len()).map_or(true, |length| length > maximum_bytes) {
        eyre::bail!("{label} {} exceeds {maximum_bytes} bytes", path.display());
    }
    String::from_utf8(bytes).wrap_err_with(|| format!("decode {label} {}", path.display()))
}

fn read_bounded_proc_file(path: &Path, label: &str) -> eyre::Result<String> {
    read_bounded_file(path, INROU_NAMESPACE_PROC_MAX_BYTES, label)
}

fn parse_mountinfo(contents: &str) -> eyre::Result<Vec<InrouMountInfoRecord>> {
    let mut records = Vec::new();
    let mut mount_points = BTreeSet::new();
    for line in contents.lines() {
        let (before_separator, after_separator) = line
            .split_once(" - ")
            .ok_or_else(|| eyre::eyre!("mountinfo record omitted its separator"))?;
        let before = before_separator
            .split_ascii_whitespace()
            .collect::<Vec<_>>();
        if before.len() < 6 {
            eyre::bail!("mountinfo record is truncated before its separator");
        }
        let after = after_separator.split_ascii_whitespace().collect::<Vec<_>>();
        if after.len() != 3 {
            eyre::bail!("mountinfo record must contain exactly three post-separator fields");
        }
        let mount_point = PathBuf::from(decode_mountinfo_field(before[4])?);
        if !mount_point.is_absolute() || !mount_points.insert(mount_point.clone()) {
            eyre::bail!("mountinfo contains a non-absolute or repeated mount point");
        }
        let filesystem_type = after[0].to_owned();
        if filesystem_type.is_empty()
            || !filesystem_type
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        {
            eyre::bail!("mountinfo contains a non-canonical filesystem type");
        }
        let _super_options = parse_comma_options(after[2])?;
        records.push(InrouMountInfoRecord {
            mount_point,
            filesystem_type,
            mount_options: parse_comma_options(before[5])?,
        });
    }
    if records.is_empty() {
        eyre::bail!("mountinfo must contain at least the private root mount");
    }
    Ok(records)
}

fn decode_mountinfo_field(value: &str) -> eyre::Result<String> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'\\' {
            decoded.push(bytes[index]);
            index += 1;
            continue;
        }
        let escape = bytes
            .get(index + 1..index + 4)
            .ok_or_else(|| eyre::eyre!("mountinfo field ends in a truncated escape"))?;
        let replacement = match escape {
            b"040" => b' ',
            b"011" => b'\t',
            b"012" => b'\n',
            b"134" => b'\\',
            _ => eyre::bail!("mountinfo field contains an unsupported escape"),
        };
        decoded.push(replacement);
        index += 4;
    }
    String::from_utf8(decoded).wrap_err("decode mountinfo field as UTF-8")
}

fn parse_comma_options(value: &str) -> eyre::Result<BTreeSet<String>> {
    if value.is_empty() {
        eyre::bail!("mountinfo options must not be empty");
    }
    let mut options = BTreeSet::new();
    for option in value.split(',') {
        if option.is_empty() || !options.insert(option.to_owned()) {
            eyre::bail!("mountinfo contains an empty or repeated option");
        }
    }
    if options.contains("ro") == options.contains("rw") {
        eyre::bail!("mountinfo options must contain exactly one of `ro` and `rw`");
    }
    Ok(options)
}

fn exact_mount<'a>(
    mounts: &'a [InrouMountInfoRecord],
    target: &Path,
) -> eyre::Result<&'a InrouMountInfoRecord> {
    mounts
        .iter()
        .find(|mount| mount.mount_point == target)
        .ok_or_else(|| eyre::eyre!("nested QEMU mount graph omitted `{}`", target.display()))
}

fn require_mount(
    mounts: &[InrouMountInfoRecord],
    target: &Path,
    filesystem_type: &str,
    read_only: bool,
) -> eyre::Result<()> {
    let mount = exact_mount(mounts, target)?;
    if mount.filesystem_type != filesystem_type {
        eyre::bail!(
            "nested QEMU mount {} uses {} instead of {}",
            target.display(),
            mount.filesystem_type,
            filesystem_type
        );
    }
    require_mount_mode(mount, read_only)
}

fn require_mount_mode(mount: &InrouMountInfoRecord, read_only: bool) -> eyre::Result<()> {
    let expected = if read_only { "ro" } else { "rw" };
    let rejected = if read_only { "rw" } else { "ro" };
    if !mount.mount_options.contains(expected) || mount.mount_options.contains(rejected) {
        eyre::bail!(
            "nested QEMU mount {} does not retain exact {expected} posture",
            mount.mount_point.display()
        );
    }
    Ok(())
}

fn require_bind_mount_mode(
    mounts: &[InrouMountInfoRecord],
    target: &Path,
    writable: bool,
) -> eyre::Result<()> {
    require_mount_mode(exact_mount(mounts, target)?, !writable)
}

fn proc_root_path(pid: u32, sandbox_path: &Path) -> eyre::Result<PathBuf> {
    validate_runtime_manifest_path(sandbox_path)?;
    Ok(PathBuf::from(format!("/proc/{pid}/root")).join(
        sandbox_path
            .strip_prefix("/")
            .wrap_err("strip nested QEMU root path")?,
    ))
}

fn attest_kvm_binding(
    qemu_pid: u32,
    mounts: &[InrouMountInfoRecord],
    expected: InrouFileIdentity,
) -> eyre::Result<()> {
    let path = proc_root_path(qemu_pid, Path::new("/dev/kvm"))?;
    validate_exact_file_identity(&path, expected, "nested QEMU KVM device")?;
    require_bind_mount_mode(mounts, Path::new("/dev/kvm"), true)
}

fn validate_minimal_root_tree(
    qemu_pid: u32,
    expected: &BTreeMap<PathBuf, BTreeSet<OsString>>,
) -> eyre::Result<()> {
    validate_minimal_root_tree_at(&PathBuf::from(format!("/proc/{qemu_pid}/root")), expected)
}

fn validate_minimal_root_tree_at(
    root: &Path,
    expected: &BTreeMap<PathBuf, BTreeSet<OsString>>,
) -> eyre::Result<()> {
    for (directory, expected_children) in expected {
        if matches!(directory.to_str(), Some("/dev" | "/proc" | "/tmp")) {
            continue;
        }
        let path = runtime_host_path(root, directory)?;
        let metadata = if directory == Path::new("/") {
            // The procfs root handle itself is a kernel-provided symlink.
            fs::metadata(&path)?
        } else {
            fs::symlink_metadata(&path)?
        };
        if !metadata.is_dir() {
            eyre::bail!(
                "nested QEMU minimal-root path {} changed from a directory",
                directory.display()
            );
        }
        let actual_children = fs::read_dir(&path)?
            .map(|entry| entry.map(|entry| entry.file_name()))
            .collect::<Result<BTreeSet<_>, _>>()?;
        if actual_children != *expected_children {
            eyre::bail!(
                "nested QEMU minimal-root directory {} contains {:?} instead of {:?}",
                directory.display(),
                actual_children,
                expected_children
            );
        }
    }
    let dev_root = root.join("dev");
    for entry in fs::read_dir(&dev_root)? {
        let entry = entry?;
        let metadata = fs::symlink_metadata(entry.path())?;
        if metadata.file_type().is_socket() || entry.file_name() == "log" {
            eyre::bail!("nested QEMU private `/dev` exposes a host-style socket");
        }
    }
    for forbidden in ["run", "sys"] {
        match fs::symlink_metadata(root.join(forbidden)) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Ok(_) => eyre::bail!("nested QEMU minimal root exposes forbidden `/{forbidden}`"),
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_error_contains(error: &eyre::Report, expected: &str) {
        let rendered = format!("{error:#}");
        assert!(
            rendered.contains(expected),
            "expected error to contain {expected:?}, got {rendered:?}"
        );
    }

    struct MinimalRootFixture {
        root: tempfile::TempDir,
        expected: BTreeMap<PathBuf, BTreeSet<OsString>>,
    }

    fn minimal_root_fixture() -> eyre::Result<MinimalRootFixture> {
        let digest = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        let manifest = format!(
            "{INROU_RUNTIME_MANIFEST_HEADER}\n\
             d - 0 0555 /\n\
             d - 0 0555 /dev\n\
             d - 0 0555 /inrou\n\
             d - 0 0555 /inrou/bin\n\
             f {digest} 0 0555 /inrou/bin/qemu\n\
             f {digest} 0 0555 /inrou/bin/setpriv\n\
             d - 0 0555 /inrou/disk\n\
             f {digest} 0 0444 /inrou/disk/root\n\
             d - 0 0555 /inrou/empty\n\
             d - 0 0555 /inrou/input\n\
             f {digest} 0 0444 /inrou/input/kernel\n\
             d - 0 0555 /proc\n\
             d - 0 0555 /tmp\n"
        );
        let runtime_entries = parse_runtime_manifest(&manifest)?;
        let root = tempfile::tempdir()?;
        for entry in &runtime_entries {
            let path = runtime_host_path(root.path(), &entry.sandbox_path)?;
            match entry.kind {
                InrouRuntimeEntryKind::Directory => fs::create_dir_all(path)?,
                InrouRuntimeEntryKind::Regular => fs::write(path, [])?,
            }
        }
        let mut bindings = Vec::new();
        for (sandbox_path, writable) in [
            (INROU_NAMESPACE_KERNEL_PATH, false),
            (INROU_NAMESPACE_ROOT_DISK_PATH, true),
        ] {
            let host_file =
                fs::File::open(runtime_host_path(root.path(), Path::new(sandbox_path))?)?;
            bindings.push(InrouNamespaceBinding {
                identity: file_identity(&host_file.metadata()?, InrouFileKind::Regular),
                host_file,
                sandbox_path: PathBuf::from(sandbox_path),
                writable,
            });
        }
        let expected = expected_minimal_root_entries(&runtime_entries, &bindings)?;
        Ok(MinimalRootFixture { root, expected })
    }

    #[test]
    fn minimal_root_tree_accepts_files_empty_directories_and_file_bindings() -> eyre::Result<()> {
        let fixture = minimal_root_fixture()?;
        for file in [
            INROU_NAMESPACE_QEMU_PATH,
            INROU_NAMESPACE_SETPRIV_PATH,
            INROU_NAMESPACE_KERNEL_PATH,
            INROU_NAMESPACE_ROOT_DISK_PATH,
        ] {
            assert!(!fixture.expected.contains_key(Path::new(file)));
            let path = Path::new(file);
            assert!(fixture.expected[path.parent().unwrap()].contains(path.file_name().unwrap()));
        }
        assert!(fixture.expected[Path::new("/inrou/empty")].is_empty());
        validate_minimal_root_tree_at(fixture.root.path(), &fixture.expected)
    }

    #[test]
    fn minimal_root_tree_rejects_unexpected_and_missing_children() -> eyre::Result<()> {
        let fixture = minimal_root_fixture()?;
        let extra = fixture.root.path().join("inrou/unexpected");
        fs::write(&extra, [])?;
        let error = validate_minimal_root_tree_at(fixture.root.path(), &fixture.expected)
            .expect_err("undeclared children must remain forbidden");
        assert_error_contains(&error, "minimal-root directory /inrou contains");
        fs::remove_file(extra)?;
        fs::remove_file(fixture.root.path().join("inrou/bin/qemu"))?;
        let error = validate_minimal_root_tree_at(fixture.root.path(), &fixture.expected)
            .expect_err("regular files must remain required directory children");
        assert_error_contains(&error, "minimal-root directory /inrou/bin contains");
        Ok(())
    }

    #[test]
    fn minimal_root_tree_rejects_directory_file_and_symlink_substitutions() -> eyre::Result<()> {
        let fixture = minimal_root_fixture()?;
        let directory = fixture.root.path().join("inrou/empty");
        fs::remove_dir(&directory)?;
        fs::write(&directory, [])?;
        let error = validate_minimal_root_tree_at(fixture.root.path(), &fixture.expected)
            .expect_err("a declared directory must not become a file");
        assert_error_contains(
            &error,
            "minimal-root path /inrou/empty changed from a directory",
        );
        fs::remove_file(&directory)?;
        let replacement = tempfile::tempdir()?;
        std::os::unix::fs::symlink(replacement.path(), &directory)?;
        let error = validate_minimal_root_tree_at(fixture.root.path(), &fixture.expected)
            .expect_err("a declared directory must not redirect through a symlink");
        assert_error_contains(
            &error,
            "minimal-root path /inrou/empty changed from a directory",
        );
        Ok(())
    }

    #[test]
    fn manifest_parser_accepts_only_sorted_typed_hashed_records() -> eyre::Result<()> {
        let digest = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        let manifest = format!(
            "{INROU_RUNTIME_MANIFEST_HEADER}\nd - 0 0555 /\nd - 0 0555 /inrou\nf {digest} 0 0444 /inrou/placeholder\n"
        );
        let parsed = parse_runtime_manifest(&manifest)?;
        assert_eq!(parsed.len(), 3);
        assert_eq!(parsed[2].kind, InrouRuntimeEntryKind::Regular);
        assert_eq!(parsed[2].exact_bytes, 0);
        for (rejected, expected) in [
            (
                manifest.replace("0444", "0666"),
                "runtime file mode must be exactly 0444 or 0555",
            ),
            (
                manifest.replace(digest, "ABCDEF"),
                "runtime SHA-256 must be 64 lowercase hexadecimal digits",
            ),
            (
                manifest.replace("/inrou\nf", "/z\nf"),
                "runtime manifest paths must be strictly sorted and unique",
            ),
            (
                manifest.replace("d - 0 0555 /inrou", "d - 0 0755 /inrou"),
                "runtime directory records must be `d - 0 0555 PATH`",
            ),
            (
                manifest.replace("f ", "x "),
                "runtime manifest kind must be `d` or `f`",
            ),
        ] {
            let error = parse_runtime_manifest(&rejected)
                .expect_err("non-canonical manifest must fail closed");
            assert_error_contains(&error, expected);
        }
        Ok(())
    }

    #[test]
    fn namespace_link_parser_is_exact() -> eyre::Result<()> {
        assert_eq!(parse_namespace_link("net:[4026532000]")?, 4_026_532_000);
        for (rejected, expected) in [
            ("4026532000", "namespace link omitted `[`"),
            ("net:[]", "namespace link inode is not canonical decimal"),
            ("net:[1]junk", "namespace link omitted closing `]`"),
            ("net:[01x]", "namespace link inode is not canonical decimal"),
            ("NET:[1]", "namespace link has a non-canonical kind"),
        ] {
            let error =
                parse_namespace_link(rejected).expect_err("malformed namespace identity must fail");
            assert_error_contains(&error, expected);
        }
        Ok(())
    }

    #[test]
    fn mountinfo_parser_rejects_duplicate_and_ambiguous_mounts() -> eyre::Result<()> {
        let root = "29 23 0:26 / / ro,nosuid - ext4 /dev/root ro,relatime\n";
        let parsed = parse_mountinfo(root)?;
        require_bind_mount_mode(&parsed, Path::new("/"), false)?;
        let error = require_bind_mount_mode(&parsed, Path::new("/"), true)
            .expect_err("read-only root must not attest as writable");
        assert_error_contains(&error, "does not retain exact rw posture");
        let error = parse_mountinfo(&format!("{root}{root}"))
            .expect_err("duplicate mount targets must fail closed");
        assert_error_contains(&error, "non-absolute or repeated mount point");
        let error = parse_mountinfo("29 23 0:26 / / ro,rw - ext4 /dev/root ro\n")
            .expect_err("ambiguous mount mode must fail closed");
        assert_error_contains(&error, "options must contain exactly one of `ro` and `rw`");
        assert_eq!(decode_mountinfo_field("/with\\040space")?, "/with space");
        Ok(())
    }

    #[test]
    fn sandbox_paths_reject_host_state_and_traversal() {
        for (rejected, expected) in [
            (
                "relative",
                "sandbox binding target must be a non-root absolute path",
            ),
            ("/../escape", "sandbox binding target is not canonical"),
            (
                "/run/socket",
                "sandbox file binding overlaps a reserved private filesystem",
            ),
            (
                "/sys/kernel",
                "sandbox file binding overlaps a reserved private filesystem",
            ),
            (
                "/proc/1",
                "sandbox file binding overlaps a reserved private filesystem",
            ),
        ] {
            let error = validate_sandbox_binding_path(Path::new(rejected))
                .expect_err("unsafe sandbox target must fail closed");
            assert_error_contains(&error, expected);
        }
        validate_sandbox_binding_path(Path::new("/inrou/input/kernel"))
            .expect("canonical private target");
    }

    #[test]
    fn namespaced_setpriv_surface_drops_every_capability() {
        let arguments = inrou_namespaced_setpriv_arguments(&PortableVmChildIdentity {
            uid: 70_000,
            gid: 70_001,
            supplementary_gids: vec![108],
        });
        let arguments = arguments
            .iter()
            .map(|argument| argument.to_string_lossy())
            .collect::<Vec<_>>();
        assert_eq!(
            arguments[0..6],
            ["--reuid", "70000", "--regid", "70001", "--groups", "108"]
        );
        for required in [
            "--bounding-set=-all",
            "--inh-caps=-all",
            "--ambient-caps=-all",
            "--no-new-privs",
            "--",
        ] {
            assert!(arguments.iter().any(|argument| argument == required));
        }
    }

    #[test]
    fn bubblewrap_surface_unshares_every_mandatory_namespace() {
        let arguments = inrou_bubblewrap_namespace_arguments()
            .into_iter()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        for required in [
            "--clearenv",
            "--die-with-parent",
            "--new-session",
            "--unshare-cgroup",
            "--unshare-ipc",
            "--unshare-net",
            "--unshare-pid",
            "--unshare-uts",
        ] {
            assert!(arguments.iter().any(|argument| argument == required));
        }
        assert!(!arguments.iter().any(|argument| {
            matches!(
                argument.as_str(),
                "--as-pid-1" | "--share-net" | "/run" | "/sys"
            )
        }));
    }

    fn namespace_process_chain_fixture() -> eyre::Result<Vec<InrouNamespaceProcessStatus>> {
        [(100, 90, "100", 0), (101, 100, "101", 0), (102, 101, "102 1", 0), (103, 102, "103 7", 70_000)]
            .into_iter()
            .map(|(pid, parent, namespace_pids, uid)| {
                parse_namespace_process_status(
                    &format!("Pid:\t{pid}\nPPid:\t{parent}\nNSpid:\t{namespace_pids}\nUid:\t{uid} {uid} {uid} {uid}\nGid:\t{uid} {uid} {uid} {uid}\n"),
                    pid,
                )
            })
            .collect()
    }

    #[test]
    fn namespace_process_chain_retains_watchdog_and_pid1_reaper() -> eyre::Result<()> {
        let mut statuses = namespace_process_chain_fixture()?;
        validate_namespace_process_chain_status(90, &statuses)?;
        // A prior setup child may consume PID2; QEMU must be a child of PID1,
        // but its final namespace PID is not a launch-order constant.
        assert_eq!(statuses[3].namespace_pids.last(), Some(&7));
        for (process, namespace_pid) in statuses.iter_mut().zip([40, 41, 42, 43]) {
            process.namespace_pids.insert(1, namespace_pid);
        }
        validate_namespace_process_chain_status(90, &statuses)?;
        Ok(())
    }

    #[test]
    fn namespace_process_chain_rejects_reparenting_and_missing_reaper() -> eyre::Result<()> {
        for index in 0..4 {
            let mut statuses = namespace_process_chain_fixture()?;
            statuses[index].parent_pid = 1;
            let error = validate_namespace_process_chain_status(90, &statuses)
                .expect_err("a reparented process must fail attestation");
            assert_error_contains(&error, "exact parent relationship");
        }
        let mut statuses = namespace_process_chain_fixture()?;
        statuses.remove(2);
        let error = validate_namespace_process_chain_status(90, &statuses)
            .expect_err("QEMU cannot replace the PID1 reaper");
        assert_error_contains(&error, "exactly four processes");
        Ok(())
    }

    #[test]
    fn namespace_process_chain_rejects_changed_root_custody() -> eyre::Result<()> {
        for index in 0..3 {
            for credentials in ["uid", "gid"] {
                let mut statuses = namespace_process_chain_fixture()?;
                if credentials == "uid" {
                    statuses[index].uids[2] = 70_000;
                } else {
                    statuses[index].gids[2] = 70_000;
                }
                let error = validate_namespace_process_chain_status(90, &statuses)
                    .expect_err("every supervisor credential must retain root custody");
                assert_error_contains(&error, "retain root uid and gid");
            }
        }
        Ok(())
    }

    #[test]
    fn namespace_process_chain_rejects_invalid_pid_namespace_roles() -> eyre::Result<()> {
        for (index, namespace_pids) in [
            (1, vec![101, 1]),
            (2, vec![102]),
            (2, vec![102, 2]),
            (3, vec![103, 1]),
            (3, vec![103, 2, 3]),
        ] {
            let mut statuses = namespace_process_chain_fixture()?;
            statuses[index].namespace_pids = namespace_pids;
            let error = validate_namespace_process_chain_status(90, &statuses)
                .expect_err("reaper and QEMU must occupy their exact PID namespace roles");
            assert_error_contains(
                &error,
                "requires bubblewrap PID1 and a distinct QEMU child PID",
            );
        }
        Ok(())
    }

    #[test]
    fn namespace_process_status_rejects_ambiguous_proc_identity() -> eyre::Result<()> {
        let valid = "Pid: 100\nPPid: 90\nNSpid: 100\nUid: 0 0 0 0\nGid: 0 0 0 0\n";
        parse_namespace_process_status(valid, 100)?;
        for rejected in [
            valid.replace("Pid: 100\n", "Pid: 101\n"),
            valid.replace("NSpid: 100", "NSpid: 100 0"),
            valid.replace("NSpid: 100", "NSpid:"),
            valid.replace("Uid: 0 0 0 0", "Uid: 0"),
            format!("{valid}PPid: 90\n"),
        ] {
            assert!(parse_namespace_process_status(&rejected, 100).is_err());
        }
        Ok(())
    }

    #[test]
    fn namespace_cgroup_requires_exact_distinct_process_chain() -> eyre::Result<()> {
        let expected = [100, 101, 102, 103];
        validate_namespace_cgroup_members(&expected, expected)?;
        for members in [
            vec![100, 101, 103],
            vec![100, 101, 102, 103, 104],
            vec![100, 101, 102, 104],
        ] {
            assert!(validate_namespace_cgroup_members(&members, expected).is_err());
        }
        assert!(
            validate_namespace_cgroup_members(&[100, 101, 102, 102], [100, 101, 102, 102]).is_err()
        );
        Ok(())
    }

    #[test]
    fn bubblewrap_probe_requires_both_descriptor_binding_options() -> eyre::Result<()> {
        let complete = INROU_BWRAP_REQUIRED_OPTIONS.join(" ");
        require_inrou_bubblewrap_help(&complete)?;
        for omitted in ["--bind-fd", "--ro-bind-fd"] {
            let incomplete = INROU_BWRAP_REQUIRED_OPTIONS
                .into_iter()
                .filter(|option| *option != omitted)
                .collect::<Vec<_>>()
                .join(" ");
            let error = require_inrou_bubblewrap_help(&incomplete)
                .expect_err("descriptor-less bubblewrap must fail closed");
            assert!(error.to_string().contains(omitted));
        }
        let _legacy_binding_flags_error = require_inrou_bubblewrap_help("--bind --ro-bind")
            .expect_err("legacy path-binding flags must not satisfy the fd-binding contract");
        Ok(())
    }

    #[test]
    fn binding_layout_is_exact_and_lease_slots_are_contiguous() -> eyre::Result<()> {
        let identity = InrouFileIdentity {
            device: 1,
            inode: 1,
            mode: 0o640,
            uid: 0,
            gid: 70_001,
            links: 1,
            size: 1,
            kind: InrouFileKind::Regular,
        };
        let binding = |path: &str, writable: bool| InrouNamespaceBinding {
            host_file: tempfile::tempfile().expect("create synthetic binding file"),
            sandbox_path: PathBuf::from(path),
            identity,
            writable,
        };
        let build_bindings = || {
            let mut bindings = [
                (INROU_NAMESPACE_KERNEL_PATH, false),
                (INROU_NAMESPACE_BUNDLE_PATH, false),
                ("/inrou/input/cloud-init/meta-data", false),
                ("/inrou/input/cloud-init/network-config", false),
                ("/inrou/input/cloud-init/user-data", false),
                (INROU_NAMESPACE_ROOT_DISK_PATH, true),
            ]
            .into_iter()
            .map(|(path, writable)| binding(path, writable))
            .collect::<Vec<_>>();
            bindings.push(binding("/inrou/disk/lease0", true));
            bindings.push(binding("/inrou/disk/lease1", true));
            bindings
        };
        let bindings = build_bindings();
        require_exact_binding_layout(&bindings)?;

        let mut gap = build_bindings();
        gap.last_mut().expect("lease1").sandbox_path = "/inrou/disk/lease2".into();
        let error = require_exact_binding_layout(&gap).expect_err("lease gaps must fail closed");
        assert_error_contains(
            &error,
            "namespace lease bindings must be contiguous from lease0",
        );
        let mut extra = build_bindings();
        extra.push(binding("/run/escape", false));
        let error =
            require_exact_binding_layout(&extra).expect_err("extra bindings must fail closed");
        assert_error_contains(&error, "is outside the exact V1 input/disk surface");
        Ok(())
    }

    #[test]
    fn binding_arguments_use_only_exact_inherited_descriptors() -> eyre::Result<()> {
        let identity = InrouFileIdentity {
            device: 1,
            inode: 1,
            mode: 0o640,
            uid: 0,
            gid: 70_001,
            links: 1,
            size: 1,
            kind: InrouFileKind::Regular,
        };
        let bindings = vec![
            InrouNamespaceBinding {
                host_file: tempfile::tempfile()?,
                sandbox_path: PathBuf::from(INROU_NAMESPACE_KERNEL_PATH),
                identity,
                writable: false,
            },
            InrouNamespaceBinding {
                host_file: tempfile::tempfile()?,
                sandbox_path: PathBuf::from(INROU_NAMESPACE_ROOT_DISK_PATH),
                identity,
                writable: true,
            },
        ];
        let mut arguments = Vec::new();
        append_inrou_binding_arguments(&mut arguments, &bindings, &[66, 67])?;
        assert_eq!(
            arguments,
            [
                "--ro-bind-fd",
                "66",
                INROU_NAMESPACE_KERNEL_PATH,
                "--bind-fd",
                "67",
                INROU_NAMESPACE_ROOT_DISK_PATH,
            ]
            .into_iter()
            .map(OsString::from)
            .collect::<Vec<_>>()
        );
        let _incomplete_binding_arguments_error =
            append_inrou_binding_arguments(&mut Vec::new(), &bindings, &[66])
                .expect_err("an incomplete inherited descriptor map must fail closed");
        assert_eq!(
            build_inrou_launcher_binding_map(&bindings, &[66, 67])?,
            [
                InrouNamespaceLauncherBindingV1 {
                    descriptor: 66,
                    sandbox_path: PathBuf::from(INROU_NAMESPACE_KERNEL_PATH),
                    writable: false,
                },
                InrouNamespaceLauncherBindingV1 {
                    descriptor: 67,
                    sandbox_path: PathBuf::from(INROU_NAMESPACE_ROOT_DISK_PATH),
                    writable: true,
                },
            ]
        );
        let _incomplete_binding_map_error = build_inrou_launcher_binding_map(&bindings, &[66])
            .expect_err("typed launcher bindings require one exact descriptor per binding");
        Ok(())
    }

    #[test]
    fn opened_binding_identity_survives_replaced_host_name() -> eyre::Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("binding.raw");
        let displaced = directory.path().join("binding.displaced");
        fs::write(&path, b"authenticated bytes")?;
        let opened = fs::File::open(&path)?;
        let identity = file_identity(&opened.metadata()?, InrouFileKind::Regular);
        require_opened_file_matches_name(&path, &opened, "test binding")?;

        fs::rename(&path, &displaced)?;
        fs::write(&path, b"replacement")?;

        let _replaced_name_error = require_opened_file_matches_name(&path, &opened, "test binding")
            .expect_err("a replaced host name must fail pre-launch validation");
        validate_opened_file_identity(&opened, identity, "test binding")?;
        let _replacement_identity_error =
            validate_exact_file_identity(&path, identity, "replaced test binding")
                .expect_err("the replacement name must not impersonate the retained file");
        Ok(())
    }

    #[test]
    fn live_writable_binding_retains_custody_while_qcow2_length_changes() -> eyre::Result<()> {
        let file = tempfile::NamedTempFile::new()?;
        file.as_file().set_len(1)?;
        let metadata = fs::symlink_metadata(file.path())?;
        let mut binding = InrouNamespaceBinding {
            host_file: file.as_file().try_clone()?,
            sandbox_path: PathBuf::from(INROU_NAMESPACE_ROOT_DISK_PATH),
            identity: file_identity(&metadata, InrouFileKind::Regular),
            writable: true,
        };
        file.as_file().set_len(4_096)?;
        validate_live_binding_identity(file.path(), &binding)?;
        binding.writable = false;
        let error = validate_live_binding_identity(file.path(), &binding)
            .expect_err("a read-only binding must retain its exact byte length");
        assert_error_contains(&error, "changed custody");
        Ok(())
    }

    #[test]
    fn private_network_table_accepts_only_loopback() -> eyre::Result<()> {
        let header = concat!(
            "Inter-|   Receive                                                |  Transmit\n",
            " face |bytes    packets errs drop fifo frame compressed multicast|bytes    packets errs drop fifo colls carrier compressed\n",
        );
        let loopback = format!("{header}    lo: 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16\n");
        validate_loopback_only_network_devices(&loopback)?;
        let error = validate_loopback_only_network_devices(&format!(
            "{loopback}  eth0: 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16\n"
        ))
        .expect_err("an external interface must fail closed");
        assert_error_contains(&error, "instead of only loopback");
        let error = validate_loopback_only_network_devices(header)
            .expect_err("a missing loopback device must fail closed");
        assert_error_contains(&error, "instead of only loopback");
        Ok(())
    }
}
