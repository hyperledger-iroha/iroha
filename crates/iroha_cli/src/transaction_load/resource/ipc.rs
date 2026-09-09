//! Nonblocking, bounded JSONL transport and capture authentication for one child.

use super::*;
use std::{
    io::{Read, Write},
    os::{fd::AsFd, unix::fs::MetadataExt},
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering},
    time::Instant as WallInstant,
};

#[cfg(test)]
use std::os::unix::fs::DirBuilderExt;

const TURN: Duration = Duration::from_millis(10);

struct Control {
    origin: WallInstant,
    lifetime_ns: u64,
    request_deadline_ns: AtomicU64,
    stop: AtomicBool,
    // 0 running; 1 clean exit; 2 unsuccessful exit; 3 deadline/cancellation.
    exit: AtomicU8,
}
impl Control {
    fn now(&self) -> u64 {
        u64::try_from(self.origin.elapsed().as_nanos()).unwrap_or(u64::MAX)
    }
    fn check(&self, deadline: u64) -> Result<()> {
        if self.stop.load(Ordering::Acquire) || self.now() >= deadline.min(self.lifetime_ns) {
            bail!("resource child deadline or cancellation reached");
        }
        Ok(())
    }
}

/// Owns only the Child returned by this command, never a process group.
struct OwnedChild(Child);
impl Drop for OwnedChild {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            let _ = self.0.kill();
            // This fallback covers partial construction and failed thread
            // creation before LiveClock starts. Never use an unbounded wait.
            let deadline = WallInstant::now() + Duration::from_secs(1);
            while WallInstant::now() < deadline {
                if self.0.try_wait().ok().flatten().is_some() {
                    break;
                }
                std::thread::sleep(TURN);
            }
        }
    }
}
fn watch_child(mut child: OwnedChild, control: Arc<Control>) {
    loop {
        match child.0.try_wait() {
            Ok(Some(status)) => {
                control
                    .exit
                    .store(if status.success() { 1 } else { 2 }, Ordering::Release);
                return;
            }
            Ok(None) => {}
            Err(_) => {
                control.stop.store(true, Ordering::Release);
                break;
            }
        }
        if control.stop.load(Ordering::Acquire)
            || control.now()
                >= control
                    .lifetime_ns
                    .min(control.request_deadline_ns.load(Ordering::Acquire))
        {
            break;
        }
        std::thread::sleep(TURN);
    }
    control.stop.store(true, Ordering::Release);
    control.exit.store(3, Ordering::Release);
    let _ = child.0.kill();
    // SIGKILL targets the one retained child. Reaping is bounded; never call a
    // blocking wait and never signal peer processes or the caller's group.
    let reap_deadline = WallInstant::now() + Duration::from_secs(1);
    while WallInstant::now() < reap_deadline {
        if child.0.try_wait().ok().flatten().is_some() {
            return;
        }
        std::thread::sleep(TURN);
    }
}

struct Work {
    request: Request,
    deadline: u64,
    reply: tokio::sync::oneshot::Sender<Result<Response>>,
}
pub(super) struct ChildProbe {
    sender: mpsc::SyncSender<Work>,
    control: Arc<Control>,
}
impl Drop for ChildProbe {
    fn drop(&mut self) {
        self.control.stop.store(true, Ordering::Release);
    }
}
impl ChildProbe {
    pub(super) fn start(args: &Args, plan: Plan) -> Result<Self> {
        if [
            &args.resource_program,
            &args.resource_worker,
            &args.resource_config,
            &args.resource_capture_dir,
        ]
        .iter()
        .any(|path| !path.is_absolute())
        {
            bail!("resource inputs must be absolute paths");
        }
        let captures = Captures::create(&args.resource_capture_dir)?;
        let mut child = OwnedChild(
            Command::new(&args.resource_program)
                .arg(&args.resource_worker)
                .arg("--config")
                .arg(&args.resource_config)
                .arg("--capture-dir")
                .arg(&args.resource_capture_dir)
                .env_clear()
                .current_dir("/")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
                .map_err(|_| eyre!("resource probe could not start"))?,
        );
        let stdin = child
            .0
            .stdin
            .take()
            .ok_or_else(|| eyre!("resource child input pipe missing"))?;
        let stdout = child
            .0
            .stdout
            .take()
            .ok_or_else(|| eyre!("resource child output pipe missing"))?;
        for descriptor in [stdin.as_fd(), stdout.as_fd()] {
            let flags = rustix::fs::fcntl_getfl(descriptor)
                .map_err(|_| eyre!("resource pipe flags unavailable"))?;
            rustix::fs::fcntl_setfl(descriptor, flags | rustix::fs::OFlags::NONBLOCK)
                .map_err(|_| eyre!("resource pipes could not become nonblocking"))?;
        }
        let lifetime_ns = u64::try_from(plan.lifetime.as_nanos())
            .map_err(|_| eyre!("resource lifetime overflow"))?;
        let control = Arc::new(Control {
            origin: WallInstant::now(),
            lifetime_ns,
            request_deadline_ns: AtomicU64::new(lifetime_ns),
            stop: AtomicBool::new(false),
            exit: AtomicU8::new(0),
        });
        let watched = Arc::clone(&control);
        std::thread::Builder::new()
            .name("load-resource-watch".to_owned())
            .spawn(move || watch_child(child, watched))
            .map_err(|_| eyre!("resource child watchdog could not start"))?;
        let (sender, receiver) = mpsc::sync_channel::<Work>(1);
        let worker_control = Arc::clone(&control);
        if std::thread::Builder::new()
            .name("load-resource-pipe".to_owned())
            .spawn(move || {
                let mut io = Pipe {
                    stdin: Some(stdin),
                    stdout,
                    control: worker_control,
                    captures,
                };
                let mut sequence = 0;
                loop {
                    if io.control.check(io.control.lifetime_ns).is_err() {
                        break;
                    }
                    let work = match receiver.recv_timeout(TURN) {
                        Ok(work) => work,
                        Err(mpsc::RecvTimeoutError::Timeout) => continue,
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    };
                    let identity_ok = work.request.sequence == sequence
                        && ((sequence == 0 && work.request.kind == Kind::Preflight)
                            || (sequence > 0 && work.request.kind != Kind::Preflight));
                    let result = if identity_ok {
                        io.exchange(work.request, work.deadline)
                    } else {
                        Err(eyre!("resource request sequence is invalid"))
                    };
                    let finished = result.is_err() || work.request.kind == Kind::Finish;
                    if !finished {
                        io.control
                            .request_deadline_ns
                            .store(io.control.lifetime_ns, Ordering::Release);
                    }
                    if work.reply.send(result).is_err() || finished {
                        break;
                    }
                    sequence += 1;
                }
                io.control.stop.store(true, Ordering::Release);
            })
            .is_err()
        {
            control.stop.store(true, Ordering::Release);
            bail!("resource pipe worker could not start");
        }
        Ok(Self { sender, control })
    }
}
impl Probe for ChildProbe {
    fn exchange(&self, request: Request) -> BoxFuture<'_, Result<Response>> {
        async move {
            let deadline = self
                .control
                .now()
                .checked_add(request.timeout_ms * 1_000_000)
                .ok_or_else(|| eyre!("resource wall deadline overflow"))?;
            self.control.check(deadline)?;
            self.control
                .request_deadline_ns
                .store(deadline, Ordering::Release);
            let (reply, receive) = tokio::sync::oneshot::channel();
            self.sender
                .try_send(Work {
                    request,
                    deadline,
                    reply,
                })
                .map_err(|_| eyre!("resource request owner is unavailable or saturated"))?;
            let remaining = Duration::from_nanos(deadline.saturating_sub(self.control.now()));
            match tokio::time::timeout(remaining, receive).await {
                Ok(Ok(result)) => result,
                _ => {
                    self.control.stop.store(true, Ordering::Release);
                    Err(eyre!("resource response owner timed out or closed"))
                }
            }
        }
        .boxed()
    }
    fn abort(&self) {
        self.control.stop.store(true, Ordering::Release);
    }
}

struct Pipe<W = ChildStdin, R = ChildStdout> {
    stdin: Option<W>,
    stdout: R,
    control: Arc<Control>,
    captures: Captures,
}
impl<W: Write + AsFd, R: Read + AsFd> Pipe<W, R> {
    fn poll(&self, writing: bool, deadline: u64) -> Result<()> {
        self.control.check(deadline)?;
        let mut descriptors = Vec::with_capacity(2);
        if writing {
            if let Some(stdin) = &self.stdin {
                descriptors.push(rustix::event::PollFd::new(
                    stdin,
                    rustix::event::PollFlags::OUT,
                ));
            }
        }
        descriptors.push(rustix::event::PollFd::new(
            &self.stdout,
            rustix::event::PollFlags::IN,
        ));
        let remaining = Duration::from_nanos(deadline.saturating_sub(self.control.now()));
        let timeout = rustix::event::Timespec::try_from(TURN.min(remaining))
            .map_err(|_| eyre!("resource poll deadline invalid"))?;
        match rustix::event::poll(&mut descriptors, Some(&timeout)) {
            Ok(_) | Err(rustix::io::Errno::INTR) => Ok(()),
            Err(_) => Err(eyre!("resource pipe readiness failed")),
        }
    }
    fn exchange(&mut self, request: Request, deadline: u64) -> Result<Response> {
        self.control.check(deadline)?;
        // A worker may only answer after this request; stale prefetched frames
        // cannot satisfy a later sequence even when their JSON looks plausible.
        let mut byte = [0_u8; 1];
        match self.stdout.read(&mut byte) {
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
            _ => bail!("resource worker closed or emitted unsolicited output"),
        }
        let line = request.line()?;
        let mut written = 0;
        while written < line.len() {
            self.control.check(deadline)?;
            let stdin = self
                .stdin
                .as_mut()
                .ok_or_else(|| eyre!("resource request pipe already closed"))?;
            match stdin.write(&line[written..]) {
                Ok(0) => bail!("resource request pipe closed"),
                Ok(count) => written += count,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    self.poll(true, deadline)?
                }
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                Err(_) => bail!("resource request write failed"),
            }
        }
        let mut bytes = Vec::new();
        loop {
            self.control.check(deadline)?;
            let mut buffer = [0_u8; 1024];
            match self.stdout.read(&mut buffer) {
                Ok(0) => bail!("resource worker closed before its response"),
                Ok(count) => {
                    accept_frame_chunk(&mut bytes, &buffer[..count])?;
                    if bytes.last() == Some(&b'\n') {
                        break;
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    self.poll(false, deadline)?
                }
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                Err(_) => bail!("resource response read failed"),
            }
        }
        let response = parse_response(request, &bytes)?;
        if let Some(manifest) = &response.manifest {
            self.captures
                .authenticate(request, &response, manifest, &self.control, deadline)?;
        }
        if request.kind == Kind::Finish {
            drop(self.stdin.take());
            let mut eof = false;
            loop {
                self.control.check(deadline)?;
                if !eof {
                    match self.stdout.read(&mut byte) {
                        Ok(0) => eof = true,
                        Ok(_) => bail!("resource worker emitted trailing output after finish"),
                        Err(error)
                            if matches!(
                                error.kind(),
                                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                            ) => {}
                        Err(_) => bail!("resource finish pipe failed"),
                    }
                }
                match self.control.exit.load(Ordering::Acquire) {
                    1 if eof => break,
                    2 | 3 => bail!("resource worker did not exit cleanly"),
                    _ => {}
                }
                if eof {
                    std::thread::sleep(TURN);
                } else {
                    self.poll(false, deadline)?;
                }
            }
        }
        self.control.check(deadline)?;
        Ok(response)
    }
}
fn accept_frame_chunk(bytes: &mut Vec<u8>, chunk: &[u8]) -> Result<()> {
    if bytes
        .len()
        .checked_add(chunk.len())
        .is_none_or(|length| length > MAX_IPC_BYTES)
        || bytes.contains(&b'\n')
        || chunk
            .iter()
            .position(|byte| *byte == b'\n')
            .is_some_and(|position| position + 1 != chunk.len())
    {
        bail!("resource response exceeded one bounded IPC line");
    }
    bytes.extend_from_slice(chunk);
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CaptureSetupStep {
    ParentAdmitted,
    ParentRetained,
    BeforeSync,
    AfterSync,
}

/// Parent identity is retained before creation; sync never reopens a pathname.
struct CaptureParent {
    path: PathBuf,
    directory: File,
    admitted: std::fs::Metadata,
}
impl CaptureParent {
    fn retain(path: &Path, hook: &mut impl FnMut(CaptureSetupStep) -> Result<()>) -> Result<Self> {
        let admitted = std::fs::symlink_metadata(path)
            .map_err(|_| eyre!("resource capture parent identity unavailable"))?;
        if !admitted.is_dir() {
            bail!("resource capture parent must be a direct directory");
        }
        hook(CaptureSetupStep::ParentAdmitted)?;
        // DIRECTORY rejects special files and NONBLOCK prevents a substituted
        // blocking node from becoming an unbounded pathname open. Kernel disk
        // latency itself is not bounded by these admission flags.
        let descriptor = rustix::fs::open(
            path,
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::DIRECTORY
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::NONBLOCK
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
        .map_err(|_| eyre!("resource capture parent could not be retained safely"))?;
        let parent = Self {
            path: path.to_owned(),
            directory: File::from(descriptor),
            admitted,
        };
        parent.check()?;
        hook(CaptureSetupStep::ParentRetained)?;
        parent.check()?;
        Ok(parent)
    }
    fn check(&self) -> Result<()> {
        let held = self
            .directory
            .metadata()
            .map_err(|_| eyre!("resource capture parent descriptor failed"))?;
        let named = std::fs::symlink_metadata(&self.path)
            .map_err(|_| eyre!("resource capture parent identity lost"))?;
        // Creating a child legitimately changes directory length, timestamps
        // and link count. Its inode, type, owner and permissions must be stable.
        for meta in [&held, &named] {
            if !meta.is_dir()
                || meta.dev() != self.admitted.dev()
                || meta.ino() != self.admitted.ino()
                || meta.mode() != self.admitted.mode()
                || meta.uid() != self.admitted.uid()
                || meta.gid() != self.admitted.gid()
            {
                bail!("resource capture parent identity changed");
            }
        }
        Ok(())
    }
}

struct Captures {
    path: PathBuf,
    directory: File,
    device: u64,
    inode: u64,
    parent: CaptureParent,
}
impl Captures {
    fn create(path: &Path) -> Result<Self> {
        Self::create_with_hook(path, |_| Ok(()))
    }
    // One implementation with a per-call hook makes substitution at the actual
    // admission/sync boundaries testable; the production caller supplies no work.
    fn create_with_hook(
        path: &Path,
        mut hook: impl FnMut(CaptureSetupStep) -> Result<()>,
    ) -> Result<Self> {
        let parent_path = path
            .parent()
            .ok_or_else(|| eyre!("resource capture parent missing"))?;
        if !path.is_absolute()
            || parent_path.canonicalize().ok().as_deref() != Some(parent_path)
            || path.file_name().is_none()
        {
            bail!("resource capture directory requires a canonical existing parent");
        }
        let parent = CaptureParent::retain(parent_path, &mut hook)?;
        let name = path
            .file_name()
            .ok_or_else(|| eyre!("resource capture name missing"))?;
        rustix::fs::mkdirat(
            &parent.directory,
            name,
            rustix::fs::Mode::from_raw_mode(0o700),
        )
        .map_err(|_| eyre!("resource capture directory must be new"))?;
        let descriptor = rustix::fs::openat(
            &parent.directory,
            name,
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::DIRECTORY
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::NONBLOCK
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
        .map_err(|_| eyre!("resource capture directory could not be retained"))?;
        let directory = File::from(descriptor);
        let metadata = directory
            .metadata()
            .map_err(|_| eyre!("resource capture identity unavailable"))?;
        let captures = Self {
            path: path.to_owned(),
            directory,
            device: metadata.dev(),
            inode: metadata.ino(),
            parent,
        };
        captures.check()?;
        hook(CaptureSetupStep::BeforeSync)?;
        captures.parent.check()?;
        captures
            .parent
            .directory
            .sync_all()
            .map_err(|_| eyre!("resource capture parent sync failed"))?;
        hook(CaptureSetupStep::AfterSync)?;
        captures.check()?;
        Ok(captures)
    }
    fn check(&self) -> Result<()> {
        self.parent.check()?;
        let named = std::fs::symlink_metadata(&self.path)
            .map_err(|_| eyre!("resource capture directory identity lost"))?;
        let held = self
            .directory
            .metadata()
            .map_err(|_| eyre!("resource capture directory descriptor failed"))?;
        for meta in [&named, &held] {
            if !meta.is_dir()
                || meta.dev() != self.device
                || meta.ino() != self.inode
                || meta.mode() & 0o7777 != 0o700
                || meta.uid() != rustix::process::geteuid().as_raw()
            {
                bail!("resource capture directory identity changed");
            }
        }
        Ok(())
    }
    fn authenticate(
        &self,
        request: Request,
        response: &Response,
        manifest: &Manifest,
        control: &Control,
        deadline: u64,
    ) -> Result<()> {
        control.check(deadline)?;
        if manifest.name != request.kind.manifest_name(request.sequence)
            || request.kind == Kind::Finish
            || !(1..=MAX_MANIFEST_BYTES).contains(&manifest.bytes)
        {
            bail!("resource manifest identity is not this request");
        }
        self.check()?;
        let path = self.path.join(&manifest.name);
        let before =
            std::fs::symlink_metadata(&path).map_err(|_| eyre!("resource manifest is missing"))?;
        validate_manifest_metadata(&before, manifest.bytes)?;
        let descriptor = rustix::fs::openat(
            &self.directory,
            manifest.name.as_str(),
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::NONBLOCK
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
        .map_err(|_| eyre!("resource manifest could not be opened safely"))?;
        let mut file = File::from(descriptor);
        let opened = file
            .metadata()
            .map_err(|_| eyre!("resource manifest identity unavailable"))?;
        if !same_file(&before, &opened) {
            bail!("resource manifest changed during admission");
        }
        let mut bytes = Vec::with_capacity(manifest.bytes as usize);
        let mut buffer = [0_u8; 8192];
        loop {
            control.check(deadline)?;
            let count = file
                .read(&mut buffer)
                .map_err(|_| eyre!("resource manifest read failed"))?;
            if count == 0 {
                break;
            }
            if bytes.len() + count > manifest.bytes as usize {
                bail!("resource manifest grew beyond exact size");
            }
            bytes.extend_from_slice(&buffer[..count]);
        }
        let held = file
            .metadata()
            .map_err(|_| eyre!("resource manifest descriptor identity lost"))?;
        let named = std::fs::symlink_metadata(path)
            .map_err(|_| eyre!("resource manifest publication disappeared"))?;
        if !same_file(&before, &held)
            || !same_file(&before, &named)
            || bytes.len() as u64 != manifest.bytes
            || format!("{:x}", Sha256::digest(&bytes)) != manifest.sha256
        {
            bail!("resource manifest publication changed or digest mismatched");
        }
        let value: Value =
            json::from_slice(&bytes).map_err(|_| eyre!("resource capture manifest is not JSON"))?;
        let object = value
            .as_object()
            .ok_or_else(|| eyre!("resource capture manifest is not an object"))?;
        if object.get("schema").and_then(Value::as_str) != Some(CAPTURE_SCHEMA)
            || object.get("kind").and_then(Value::as_str) != Some(request.kind.text())
            || object.get("sequence").and_then(Value::as_u64) != Some(request.sequence)
            || object.get("available").and_then(Value::as_bool)
                != Some(response.outcome == Outcome::Complete)
        {
            bail!("resource capture manifest does not bind this response");
        }
        self.check()?;
        control.check(deadline)
    }
}
fn validate_manifest_metadata(meta: &std::fs::Metadata, bytes: u64) -> Result<()> {
    if !meta.is_file()
        || meta.nlink() != 1
        || meta.mode() & 0o7777 != 0o600
        || meta.uid() != rustix::process::geteuid().as_raw()
        || meta.len() != bytes
        || !(1..=MAX_MANIFEST_BYTES).contains(&bytes)
    {
        bail!("resource manifest must be an exact bounded private regular file");
    }
    Ok(())
}
fn same_file(a: &std::fs::Metadata, b: &std::fs::Metadata) -> bool {
    a.dev() == b.dev()
        && a.ino() == b.ino()
        && a.len() == b.len()
        && a.mode() == b.mode()
        && a.uid() == b.uid()
        && a.nlink() == b.nlink()
        && a.mtime() == b.mtime()
        && a.mtime_nsec() == b.mtime_nsec()
        && a.ctime() == b.ctime()
        && a.ctime_nsec() == b.ctime_nsec()
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod transport_tests;

#[cfg(test)]
mod parent_tests;
