//! Private retained generations, context selection, and foreground control operations.

use super::*;
use iroha_fs::{PrivateDirectory, PublishMode};
use std::{
    fs::{self, File},
    io::{Read as _, Seek as _, SeekFrom},
    net::{Ipv4Addr, TcpListener},
    path::Path,
    process::{Command, Stdio},
    thread,
    time::Instant,
};

/// Held loopback reservations for the four Torii and four P2P listeners.
///
/// Keep this value alive until preparation finishes; the native worker acquires the actual
/// listeners immediately after these reservations are released.
pub struct LocalnetPorts {
    /// First of four consecutive Torii ports.
    pub base_api: u16,
    /// First of four consecutive P2P ports.
    pub base_p2p: u16,
    reservations: Vec<TcpListener>,
}

impl LocalnetPorts {
    /// Reserve two disjoint four-port blocks on IPv4 loopback.
    ///
    /// # Errors
    /// Returns an error when the operating system cannot allocate the listeners.
    pub fn reserve() -> Result<Self> {
        let (base_api, mut reservations) = reserve_block(8080)?;
        let (base_p2p, p2p) = reserve_block(1337)?;
        reservations.extend(p2p);
        Ok(Self {
            base_api,
            base_p2p,
            reservations,
        })
    }

    /// Number of sockets still reserved by this preparation.
    #[must_use]
    pub fn reserved_count(&self) -> usize {
        self.reservations.len()
    }
}

fn reserve_block(preferred: u16) -> Result<(u16, Vec<TcpListener>)> {
    for attempt in 0..128 {
        let base = if attempt == 0 {
            preferred
        } else {
            let probe = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
            probe.local_addr()?.port()
        };
        if base > u16::MAX - 3 {
            continue;
        }
        let mut listeners = Vec::with_capacity(4);
        for offset in 0..4 {
            match TcpListener::bind((Ipv4Addr::LOCALHOST, base + offset)) {
                Ok(listener) => listeners.push(listener),
                Err(_) => break,
            }
        }
        if listeners.len() == 4 {
            return Ok((base, listeners));
        }
    }
    Err(Error::Invalid(
        "cannot reserve four consecutive loopback ports".into(),
    ))
}

/// Owner-private managed contexts and their retained localnet generations.
pub struct ManagedStore {
    root: PrivateDirectory,
    networks: PrivateDirectory,
}

/// Whether successful startup also changes the workspace's default environment.
#[derive(Clone, Copy)]
pub(super) enum StartupSelection {
    Select,
    Preserve,
}

impl StartupSelection {
    pub(super) fn for_requested_context(requested: Option<&str>) -> Self {
        if requested.is_some() {
            Self::Preserve
        } else {
            Self::Select
        }
    }

    pub(super) fn apply(self, store: &ManagedStore, name: &str) -> Result<()> {
        if matches!(self, Self::Select) {
            store.select(name)?;
        }
        Ok(())
    }
}

/// Generation intent is enforced only while holding the named operation lock.
#[derive(Clone, Copy)]
enum GenerationPolicy {
    CreateOrRetain,
    RetainOnly,
    CreateOnly,
}

impl ManagedStore {
    /// Open or create a private store at an explicit application-selected path.
    ///
    /// # Errors
    /// Fails if custody cannot be established or the directory cannot be created.
    pub fn open(root: &Path) -> Result<Self> {
        let root = PrivateDirectory::open_or_create(root)?;
        let networks = root.ensure_child("networks")?;
        Ok(Self { root, networks })
    }

    /// Canonical private root passed to the worker entry point.
    #[must_use]
    pub fn root(&self) -> &Path {
        self.root.path()
    }

    pub(super) fn directory(&self, name: &str) -> Result<PrivateDirectory> {
        validate_name(name)?;
        Ok(self.networks.open_child(name)?)
    }

    /// Start or reconnect to a named network, preparing its generation only once.
    ///
    /// Canonical generation runs only for a new context, with private fixed-path seed custody
    /// and reserved loopback ports. Existing identities, configuration and ledger are retained.
    ///
    /// # Errors
    /// Invalid input, competing ownership, preparation, binary changes, startup or readiness failure.
    pub fn up(&self, request: &LocalnetRequest) -> Result<ManagedStatus> {
        self.up_environment(
            request,
            RootKind::Global,
            GenerationPolicy::CreateOrRetain,
            StartupSelection::Select,
            |_| Ok(()),
        )
    }

    /// Create and select a new global localnet, refusing an existing named generation.
    ///
    /// The name check and generation publication share the operation lock. A context created
    /// concurrently by another frontend cannot turn this request into a restart or private root.
    ///
    /// # Errors
    /// Invalid input, an existing or concurrently owned name, unsafe custody or failed startup.
    pub fn create_localnet(&self, request: &LocalnetRequest) -> Result<ManagedStatus> {
        self.up_environment(
            request,
            RootKind::Global,
            GenerationPolicy::CreateOnly,
            StartupSelection::Select,
            |_| Ok(()),
        )
    }

    /// Start or reconnect to an independently signed private root under the same native owner.
    ///
    /// The caller authenticates parent admission separately. This operation prepares and starts
    /// only the local child validators; it does not publish parent registration or certificates.
    ///
    /// # Errors
    /// Rejects an invalid or changed private identity, unsafe custody, or failed native readiness.
    pub fn up_private_root(
        &self,
        request: &LocalnetRequest,
        spec: &crate::localnet::PrivateRootSpec,
    ) -> Result<ManagedStatus> {
        self.up_private_root_bound(request, spec, |_| Ok(()))
    }

    pub(super) fn up_private_root_bound(
        &self,
        request: &LocalnetRequest,
        spec: &crate::localnet::PrivateRootSpec,
        retain_context: impl FnOnce(&PreparedLocalnet) -> Result<()>,
    ) -> Result<ManagedStatus> {
        spec.validate()
            .map_err(|_| Error::Invalid("invalid private-root SNS identity".into()))?;
        self.up_environment(
            request,
            RootKind::Private { spec: spec.clone() },
            GenerationPolicy::CreateOrRetain,
            StartupSelection::Select,
            retain_context,
        )
    }

    /// Restart an existing generation with its exact retained root identity and signer.
    ///
    /// # Errors
    /// Rejects missing or malformed metadata, changed binaries, or failed native readiness.
    pub fn up_retained(&self, request: &LocalnetRequest) -> Result<ManagedStatus> {
        self.up_retained_with_selection(request, StartupSelection::Select)
    }

    pub(super) fn up_retained_with_selection(
        &self,
        request: &LocalnetRequest,
        selection: StartupSelection,
    ) -> Result<ManagedStatus> {
        let directory = self.directory(&request.name)?;
        let retained = generation::read(&directory)?;
        self.up_environment(
            request,
            retained.root_kind,
            GenerationPolicy::RetainOnly,
            selection,
            |_| Ok(()),
        )
    }

    fn up_environment(
        &self,
        request: &LocalnetRequest,
        root_kind: RootKind,
        generation_policy: GenerationPolicy,
        selection: StartupSelection,
        retain_context: impl FnOnce(&PreparedLocalnet) -> Result<()>,
    ) -> Result<ManagedStatus> {
        if matches!(root_kind, RootKind::Private { .. })
            && request.service_profile != crate::localnet::LocalnetServiceProfile::Standard
        {
            return Err(Error::Invalid(
                "service-authority profiles require a global managed root".into(),
            ));
        }
        transport::supported()?;
        validate_name(&request.name)?;
        if request.startup_timeout.is_zero() || request.startup_timeout > Duration::from_secs(600) {
            return Err(Error::Invalid(
                "startup timeout must be greater than zero and at most ten minutes".into(),
            ));
        }
        let started = Instant::now();
        let programs = request.admit_programs()?;
        let (launcher, daemon) = programs.pins()?;
        let directory = self.networks.ensure_child(&request.name)?;
        let _operation = acquire(&directory, "operation.lock", &request.name)?;
        let mut reservations = None;
        let retained = match generation::read(&directory) {
            Ok(retained) => {
                if matches!(generation_policy, GenerationPolicy::CreateOnly) {
                    return Err(Error::Invalid(format!(
                        "managed environment `{}` already exists; select or start it explicitly",
                        request.name
                    )));
                }
                if retained.prepared.service_profile != request.service_profile {
                    return Err(Error::Invalid(
                        "managed generation has a different immutable service profile".into(),
                    ));
                }
                if retained.root_kind != root_kind {
                    return Err(Error::Invalid(
                        "managed generation has a different immutable root identity".into(),
                    ));
                }
                validate_prepared(
                    &request.name,
                    directory.path(),
                    &retained.prepared,
                    &root_kind,
                )?;
                if retained.launcher.blake3 != launcher.blake3
                    || retained.daemon.blake3 != daemon.blake3
                {
                    return Err(Error::Invalid("managed generation uses different binary contents; select its original matching installation".into()));
                }
                retained
            }
            Err(Error::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {
                if matches!(generation_policy, GenerationPolicy::RetainOnly) {
                    return Err(Error::Invalid(
                        "retained managed generation disappeared; refusing to replace its identity"
                            .into(),
                    ));
                }
                let ports = LocalnetPorts::reserve()?;
                let retained =
                    generation::prepare(&directory, request, root_kind, launcher, daemon, &ports)?;
                reservations = Some(ports);
                retained
            }
            Err(error) => return Err(error),
        };
        // Binding is serialized with this exact validated generation, before spawn and before
        // any reset can acquire operation.lock. No callback may activate parent operations.
        programs.validate()?;
        retain_context(&retained.prepared)?;
        runtime::startup_remaining(started, request.startup_timeout)?;
        if let Ok(status) = exchange(&directory, "status") {
            let status = observe_startup_status(
                &directory,
                &retained.prepared.context,
                status,
                started,
                request.startup_timeout,
                true,
            )?;
            if status.phase == ManagedPhase::Ready {
                programs.validate()?;
                selection.apply(self, &request.name)?;
                return Ok(status);
            }
            if status.phase == ManagedPhase::Failed {
                return Ok(status);
            }
        } else {
            // A crashed controller's still-running children keep this lock. Never adopt or kill
            // processes merely because their integer PID appears in a previous record.
            let available = acquire(&directory, "runtime.lock", &request.name)?;
            drop(available);
            let launcher = super::program::NativeProgram::matching(&retained.launcher)?;
            let output = directory.open_append("supervisor.log")?;
            let errors = output.try_clone()?;
            let mut command = Command::new(&retained.launcher.path);
            command
                .arg("_managed-worker")
                .arg("--root")
                .arg(self.root())
                .arg("--name")
                .arg(&request.name)
                .stdin(Stdio::null())
                .stdout(output)
                .stderr(errors);
            transport::detach(&mut command);
            directory.write_atomic(
                STATUS,
                &encode(&ManagedStatus {
                    context: retained.prepared.context.clone(),
                    phase: ManagedPhase::Starting,
                    running_peers: 0,
                    failure: None,
                })?,
                PublishMode::Replace,
            )?;
            // Binary verification and durable status publication consume the caller's same
            // startup budget; the worker must not receive the earlier, larger remainder.
            let remaining = runtime::startup_remaining(started, request.startup_timeout)?;
            let milliseconds = runtime::worker_startup_millis(remaining, request.startup_timeout)?;
            command
                .arg("--startup-timeout-ms")
                .arg(milliseconds.to_string());
            drop(reservations.take());
            programs.validate()?;
            launcher.validate()?;
            // These original native owners remain alive across spawn. This is a source fence,
            // not a claim that pathname execution is atomic with the validation on every OS.
            let mut worker = command.spawn()?;
            // Reap this exact child eventually without blocking the CLI after successful startup.
            thread::spawn(move || {
                let _ = worker.wait();
            });
        }
        loop {
            if started.elapsed() >= request.startup_timeout {
                return expire_startup(
                    &directory,
                    &retained.prepared.context,
                    request.startup_timeout,
                );
            }
            if let Ok(status) = exchange(&directory, "status") {
                let status = observe_startup_status(
                    &directory,
                    &retained.prepared.context,
                    status,
                    started,
                    request.startup_timeout,
                    false,
                )?;
                match status.phase {
                    ManagedPhase::Ready => {
                        programs.validate()?;
                        selection.apply(self, &request.name)?;
                        return Ok(status);
                    }
                    ManagedPhase::Failed | ManagedPhase::Stopped => return Ok(status),
                    ManagedPhase::Starting => {}
                }
            } else if let Ok(bytes) = directory.read(STATUS, MAX_METADATA) {
                let status: ManagedStatus = decode(&bytes)?;
                if status.context != retained.prepared.context {
                    return Err(Error::Invalid(
                        "retained status belongs to another managed identity".into(),
                    ));
                }
                if status.phase == ManagedPhase::Failed && !runtime_owned(&directory)? {
                    return Ok(status);
                }
            }
            thread::sleep(POLL);
        }
    }

    /// Query the authenticated live worker, or report a stopped retained generation.
    ///
    /// # Errors
    /// Missing context, custody failure, or live ownership without a reachable control endpoint.
    pub fn status(&self, name: &str) -> Result<ManagedStatus> {
        let directory = self.directory(name)?;
        let retained = generation::read(&directory)?;
        validate_prepared(
            name,
            directory.path(),
            &retained.prepared,
            &retained.root_kind,
        )?;
        if let Ok(status) = exchange(&directory, "status") {
            if status.context != retained.prepared.context {
                return Err(Error::Invalid(
                    "worker status belongs to another managed identity".into(),
                ));
            }
            return Ok(status);
        }
        if runtime_owned(&directory)? {
            return Err(Error::Busy(name.into()));
        }
        match directory.read(STATUS, MAX_METADATA) {
            Ok(bytes) => {
                let mut last: ManagedStatus = decode(&bytes)?;
                if last.context != retained.prepared.context {
                    return Err(Error::Invalid(
                        "retained status belongs to another context".into(),
                    ));
                }
                if last.phase != ManagedPhase::Stopped {
                    if last.phase == ManagedPhase::Starting
                        && file_owned(&directory, "operation.lock")?
                    {
                        // The foreground operation publishes Starting before native spawn; its
                        // lock distinguishes that small handoff window from a crashed owner.
                        last.running_peers = 0;
                        return Ok(last);
                    }
                    if last.phase != ManagedPhase::Failed {
                        last.failure = Some("the background controller stopped unexpectedly; inspect its retained log".into());
                    }
                    last.phase = ManagedPhase::Failed;
                    last.running_peers = 0;
                    return Ok(last);
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        Ok(ManagedStatus {
            context: retained.prepared.context,
            phase: ManagedPhase::Stopped,
            running_peers: 0,
            failure: None,
        })
    }

    /// Stop exactly this worker's children while retaining all chain and signer material.
    ///
    /// # Errors
    /// Missing context, competing operation, failed custody or unreconciled process ownership.
    pub fn down(&self, name: &str) -> Result<ManagedStatus> {
        let directory = self.directory(name)?;
        let _operation = acquire(&directory, "operation.lock", name)?;
        if !runtime_owned(&directory)? {
            return self.status(name);
        }
        let status = exchange(&directory, "down")?;
        let started = Instant::now();
        while runtime_owned(&directory)? {
            if started.elapsed() > Duration::from_secs(15) {
                return Err(Error::Busy(name.into()));
            }
            thread::sleep(POLL);
        }
        Ok(status)
    }

    /// Delete a stopped managed generation after the caller has obtained explicit reset intent.
    ///
    /// This is destructive: all chain state and signer material of the named context is removed.
    /// Running or orphan-owned generations are refused, and unrelated contexts are untouched.
    ///
    /// # Errors
    /// Competing operation, active ownership, malformed name, custody failure or deletion failure.
    pub fn reset(&self, name: &str) -> Result<()> {
        let directory = self.directory(name)?;
        let _operation = acquire(&directory, "operation.lock", name)?;
        let _runtime = acquire(&directory, "runtime.lock", name)?;
        directory.revalidate()?;
        transport::clear_stopped_endpoint(&directory)?;
        directory.clear_contents_preserving(&["operation.lock", "runtime.lock"])?;
        // The ownership directory and locked files remain pinned on every platform. Keep
        // selection resolution explicit: a reset selection should report the missing
        // generation, never silently switch transaction signing to another context.
        Ok(())
    }

    /// Select an existing context without exposing its signer or copying its secret config.
    ///
    /// # Errors
    /// The named generation is absent, malformed, or fails private custody checks.
    pub fn select(&self, name: &str) -> Result<ManagedContext> {
        let context = self.context(Some(name))?;
        self.root.write_atomic(
            "active.json",
            &encode(&name.to_owned())?,
            PublishMode::Replace,
        )?;
        Ok(context)
    }

    /// Load the named context, or the currently selected managed context.
    ///
    /// # Errors
    /// Missing selection, missing generation or malformed retained public metadata.
    pub fn context(&self, name: Option<&str>) -> Result<ManagedContext> {
        let selected: String;
        let name = if let Some(name) = name {
            name
        } else {
            let bytes = self
                .root
                .read("active.json", MAX_METADATA)
                .map_err(|error| {
                    if error.kind() == std::io::ErrorKind::NotFound {
                        Error::NoSelection
                    } else {
                        error.into()
                    }
                })?;
            selected = decode(&bytes)?;
            &selected
        };
        Ok(self.prepared(name)?.context)
    }

    /// Observe one retained generation's public context, peer endpoints and configuration paths.
    ///
    /// The returned metadata contains no key or token bytes and does not assert process health.
    /// Use [`Self::status`] for authenticated live lifecycle observations.
    ///
    /// # Errors
    /// Missing generation, malformed metadata, escaped paths or unsafe private custody.
    pub fn prepared(&self, name: &str) -> Result<PreparedLocalnet> {
        let directory = self.directory(name)?;
        let retained = generation::read(&directory)?;
        validate_prepared(
            name,
            directory.path(),
            &retained.prepared,
            &retained.root_kind,
        )?;
        Ok(retained.prepared)
    }

    /// List retained contexts in stable name order.
    ///
    /// # Errors
    /// A retained context or its parent cannot be read under private custody.
    pub fn contexts(&self) -> Result<Vec<ManagedContext>> {
        self.networks.revalidate()?;
        let mut names = Vec::new();
        for entry in fs::read_dir(self.networks.path())? {
            let entry = entry?;
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| Error::Invalid("invalid context filename".into()))?;
            validate_name(&name)?;
            if entry.path().join(generation::DIRECTORY).try_exists()? {
                names.push(name);
            }
        }
        names.sort();
        names
            .into_iter()
            .map(|name| self.context(Some(&name)))
            .collect()
    }

    /// Read the bounded tail of one retained peer or supervisor log.
    ///
    /// # Errors
    /// An unknown log, insecure file, or more than one MiB was requested.
    pub fn logs(&self, name: &str, peer: Option<usize>, max_bytes: usize) -> Result<String> {
        if max_bytes == 0 || max_bytes > 1024 * 1024 {
            return Err(Error::Invalid(
                "log tail size must be between 1 byte and one MiB".into(),
            ));
        }
        let directory = self.directory(name)?;
        let retained = generation::read(&directory)?;
        validate_prepared(
            name,
            directory.path(),
            &retained.prepared,
            &retained.root_kind,
        )?;
        let log_name = match peer {
            Some(index) => retained
                .prepared
                .peers
                .get(index)
                .ok_or_else(|| Error::Invalid("localnet peer index must be 0..3".into()))?
                .log_name
                .as_str(),
            None => "supervisor.log",
        };
        let mut file = directory.open_read(log_name)?;
        let length = file.metadata()?.len();
        file.seek(SeekFrom::Start(length.saturating_sub(max_bytes as u64)))?;
        let mut bytes = Vec::new();
        file.take(max_bytes as u64).read_to_end(&mut bytes)?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }
}

pub(super) fn random_token() -> String {
    hex::encode(rand::random::<[u8; 32]>())
}

pub(super) fn acquire(directory: &PrivateDirectory, file: &str, name: &str) -> Result<File> {
    let lock = directory.open_ownership_lock(file).map_err(|error| {
        if ownership_contended(&error) {
            Error::Busy(name.into())
        } else {
            error.into()
        }
    })?;
    lock.try_lock().map_err(|error| match error {
        fs::TryLockError::WouldBlock => Error::Busy(name.into()),
        fs::TryLockError::Error(error) => error.into(),
    })?;
    Ok(lock)
}

fn runtime_owned(directory: &PrivateDirectory) -> Result<bool> {
    file_owned(directory, "runtime.lock")
}

fn file_owned(directory: &PrivateDirectory, name: &str) -> Result<bool> {
    let lock = match directory.open_ownership_lock(name) {
        Ok(lock) => lock,
        Err(error) if ownership_contended(&error) => return Ok(true),
        Err(error) => return Err(error.into()),
    };
    match lock.try_lock() {
        Ok(()) => Ok(false),
        Err(fs::TryLockError::WouldBlock) => Ok(true),
        Err(fs::TryLockError::Error(error)) => Err(error.into()),
    }
}

fn ownership_contended(error: &std::io::Error) -> bool {
    // ERROR_SHARING_VIOLATION is the Windows handle-lifetime ownership fence. Unlike a
    // process-owned LockFileEx lock, inherited child handles retain this writer exclusion.
    cfg!(windows) && error.raw_os_error() == Some(32)
}

pub(super) fn pin_binary(path: &Path) -> Result<BinaryPin> {
    super::program::NativeProgram::capture(path)?.pin()
}

pub(super) fn verify_binary(pin: &BinaryPin) -> Result<()> {
    super::program::NativeProgram::matching(pin)?.validate()?;
    Ok(())
}

pub(super) fn validate_prepared(
    name: &str,
    root: &Path,
    prepared: &PreparedLocalnet,
    root_kind: &RootKind,
) -> Result<()> {
    let generation_root = root.join(generation::DIRECTORY);
    let root = generation_root.as_path();
    if prepared.context.name != name || prepared.peers.len() != 4 {
        return Err(Error::Invalid(
            "prepared localnet must bind its exact name and four validators".into(),
        ));
    }
    match root_kind {
        RootKind::Global
            if prepared.context.dataspace_id != 0
                || prepared.context.dataspace_alias != "universal" =>
        {
            return Err(Error::Invalid(
                "global localnet must bind the universal dataspace".into(),
            ));
        }
        RootKind::Private { spec } => {
            if prepared.service_profile != crate::localnet::LocalnetServiceProfile::Standard {
                return Err(Error::Invalid(
                    "private roots cannot retain a service-authority profile".into(),
                ));
            }
            spec.validate()
                .map_err(|_| Error::Invalid("invalid retained private-root SNS identity".into()))?;
            if prepared.context.dataspace_id != spec.dataspace_id.as_u64()
                || prepared.context.dataspace_alias != spec.dataspace_alias
            {
                return Err(Error::Invalid(
                    "private context differs from its immutable root identity".into(),
                ));
            }
        }
        RootKind::Global => {}
    }
    let mut endpoints = std::collections::BTreeSet::new();
    let mut configs = std::collections::BTreeSet::new();
    let mut logs = std::collections::BTreeSet::new();
    for peer in &prepared.peers {
        let url: url::Url = peer
            .torii_url
            .parse()
            .map_err(|_| Error::Invalid("invalid managed Torii URL".into()))?;
        if url.scheme() != "http"
            || url.host_str() != Some("127.0.0.1")
            || url.port().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.path() != "/"
            || url.query().is_some()
            || url.fragment().is_some()
            || !endpoints.insert(peer.torii_url.clone())
        {
            return Err(Error::Invalid(
                "managed Torii endpoints must be distinct bare IPv4 loopback HTTP origins".into(),
            ));
        }
        if !confined_path(root, &peer.config_path)?
            || !configs.insert(peer.config_path.clone())
            || peer.log_name.contains(['/', '\\'])
            || !peer.log_name.ends_with(".log")
            || peer.log_name.starts_with('.')
            || peer.log_name == "supervisor.log"
            || !logs.insert(peer.log_name.clone())
        {
            return Err(Error::Invalid(
                "managed configs and log names must be distinct and confined to the generation"
                    .into(),
            ));
        }
        iroha_fs::read_private(&peer.config_path, MAX_METADATA)?;
    }
    if prepared.context.torii_url != prepared.peers[0].torii_url
        || !confined_path(root, &prepared.context.client_config)?
    {
        return Err(Error::Invalid(
            "client context must belong to this generation and its first validator".into(),
        ));
    }
    iroha_fs::read_private(&prepared.context.client_config, MAX_METADATA)?;
    if let RootKind::Private { spec } = root_kind {
        let generation =
            prepared.context.client_config.parent().ok_or_else(|| {
                Error::Invalid("private context has no generation directory".into())
            })?;
        crate::localnet::verify_private_root(generation, prepared, spec)?;
    } else {
        crate::localnet::service_authorities::validate_retained(prepared)?;
    }
    Ok(())
}

fn confined_path(root: &Path, path: &Path) -> Result<bool> {
    use std::path::Component;
    // Do not normalize a malicious retained path into an allowed one: exact canonical spelling
    // is part of the context binding, and the custody reader subsequently pins every component.
    if !path.is_absolute()
        || path
            .components()
            .any(|part| matches!(part, Component::ParentDir | Component::CurDir))
        || !path.starts_with(root)
    {
        return Ok(false);
    }
    Ok(path.canonicalize()? == path && root.canonicalize()? == root)
}

pub(super) fn exchange(directory: &PrivateDirectory, action: &str) -> Result<ManagedStatus> {
    let worker: WorkerRecord = decode(&directory.read(WORKER, MAX_METADATA)?)?;
    transport::request(
        directory,
        &ControlRequest {
            token: worker.token,
            action: action.into(),
        },
    )
}

pub(super) fn observe_startup_status(
    directory: &PrivateDirectory,
    context: &ManagedContext,
    status: ManagedStatus,
    started: Instant,
    timeout: Duration,
    initial_observation: bool,
) -> Result<ManagedStatus> {
    if status.context != *context {
        return Err(Error::Invalid(
            "worker status belongs to another managed identity".into(),
        ));
    }
    // IPC consumes the same foreground budget. Do not select a late Ready. An initial
    // observation of a previously ready worker belongs to a prior successful invocation:
    // a slow repeated `up` must not stop that healthy retained network.
    if started.elapsed() >= timeout {
        if initial_observation && status.phase == ManagedPhase::Ready {
            return Err(Error::Timeout(timeout));
        }
        return expire_startup(directory, context, timeout);
    }
    Ok(status)
}

pub(super) fn expire_startup(
    directory: &PrivateDirectory,
    context: &ManagedContext,
    timeout: Duration,
) -> Result<ManagedStatus> {
    if let Ok(status) = exchange(directory, "startup_expired") {
        if status.context != *context {
            return Err(Error::Invalid(
                "startup cancellation belongs to another managed identity".into(),
            ));
        }
        if status.phase == ManagedPhase::Failed && status.running_peers == 0 {
            return Ok(status);
        }
    }
    // A worker already shutting down may not answer. Its own same bounded deadline still
    // publishes the closed failure; never replace that evidence with a fabricated stopped state.
    Err(Error::Timeout(timeout))
}
