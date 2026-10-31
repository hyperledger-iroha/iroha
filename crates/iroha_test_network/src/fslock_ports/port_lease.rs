//! OS-owned leases for unbound test ports. Lease files are never unlinked.
use iroha_fs::{FileSnapshot, PrivateDirectory};
use std::{
    fs::File,
    io,
    net::{Ipv4Addr, TcpListener},
    ops::RangeInclusive,
    sync::Arc,
};

#[cfg(any(unix, test))]
use std::path::Path;

fn changed() -> io::Error {
    io::Error::other("test port lease original custody changed")
}

/// One original lock and all private ancestors retained until the lease is dropped or exits.
#[derive(Debug)]
pub(crate) struct PortLease {
    port: u16,
    file: File,
    snapshot: FileSnapshot,
    directory: Arc<PrivateDirectory>,
}
impl PortLease {
    /// The port protected by this lease, after closing original custody.
    pub(crate) fn port(&self) -> u16 {
        self.revalidate().expect("original test port lease custody");
        self.port
    }

    pub(crate) fn revalidate(&self) -> io::Result<()> {
        if FileSnapshot::private_journal(&self.file)? != self.snapshot {
            return Err(changed());
        }
        self.directory.read_scope(|reader| {
            reader.require_same_file(format!("port-{}.lock", self.port), &self.file, changed)
        })?;
        if FileSnapshot::private_journal(&self.file)? != self.snapshot {
            return Err(changed());
        }
        Ok(())
    }
}

/// One shared per-user namespace, independent of checkout and per-test temp roots.
#[derive(Debug)]
pub(crate) struct PortLeasePool {
    directory: Arc<PrivateDirectory>,
}
impl PortLeasePool {
    /// Create or admit one current-user private lease directory without repairing permissions.
    #[cfg(test)]
    pub(crate) fn open(directory: &Path) -> io::Result<Self> {
        Self::from_directory(PrivateDirectory::open_or_create(directory)?)
    }

    pub(crate) fn runtime() -> io::Result<Self> {
        Self::from_directory(runtime_lease_directory()?)
    }

    fn from_directory(directory: PrivateDirectory) -> io::Result<Self> {
        directory.revalidate()?;
        Ok(Self {
            directory: Arc::new(directory),
        })
    }

    /// Lease one currently unoccupied port, without holding the socket itself.
    /// A bindable port with another lease is still reserved and cannot be reused.
    pub(crate) fn try_port(&self, port: u16) -> io::Result<Option<PortLease>> {
        if port == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "port zero is not a lease identity",
            ));
        }
        self.directory.revalidate()?;
        let file = self.directory.open_lock(format!("port-{port}.lock"))?;
        let snapshot = FileSnapshot::private_journal(&file)?;
        let lease = PortLease {
            port,
            file,
            snapshot,
            directory: Arc::clone(&self.directory),
        };
        lease.revalidate()?;
        match lease.file.try_lock() {
            Ok(()) => {}
            Err(std::fs::TryLockError::WouldBlock) => {
                lease.revalidate()?;
                return Ok(None);
            }
            Err(std::fs::TryLockError::Error(error)) => return Err(error),
        }
        lease.revalidate()?;
        match TcpListener::bind((Ipv4Addr::LOCALHOST, port)) {
            Ok(listener) => drop(listener),
            Err(error) if error.kind() == io::ErrorKind::AddrInUse => {
                lease.revalidate()?;
                return Ok(None);
            }
            Err(error) => return Err(error),
        }
        lease.revalidate()?;
        Ok(Some(lease))
    }

    /// Reserve a contiguous block in a bounded range, rolling back every partial
    /// candidate on contention. The search visits each possible base at most once.
    pub(crate) fn allocate(
        &self,
        count: u16,
        range: RangeInclusive<u16>,
        preferred_start: u16,
    ) -> io::Result<Option<Vec<PortLease>>> {
        self.directory.revalidate()?;
        let (start, end) = (*range.start(), *range.end());
        if count == 0 || start == 0 || start > end {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid test port allocation range",
            ));
        }
        let Some(max_base) = end.checked_sub(count - 1).filter(|base| *base >= start) else {
            return Ok(None);
        };
        let first = preferred_start.clamp(start, max_base);
        for base in (first..=max_base).chain(start..first) {
            let mut leases = Vec::with_capacity(usize::from(count));
            for port in base..=base + (count - 1) {
                let Some(lease) = self.try_port(port)? else {
                    break;
                };
                leases.push(lease);
            }
            if leases.len() == usize::from(count) {
                for lease in &leases {
                    lease.revalidate()?;
                }
                return Ok(Some(leases));
            }
        }
        Ok(None)
    }
}

/// Use the OS user's stable namespace, independently of source and temp environment variables.
#[cfg(unix)]
fn runtime_lease_directory() -> io::Result<PrivateDirectory> {
    let owner = nix::unistd::Uid::effective().as_raw();
    let path = Path::new("/tmp")
        .canonicalize()?
        .join(format!("iroha-test-network-ports-{owner}"));
    PrivateDirectory::open_or_create(path)
}

#[cfg(windows)]
fn runtime_lease_directory() -> io::Result<PrivateDirectory> {
    let parent = iroha_fs::windows::current_user_local_data_directory()?;
    let name = "iroha-test-network-ports";
    match parent.open_private_child_optional(name)? {
        Some(directory) => Ok(directory),
        None => match parent.create_private_child(name) {
            Ok(directory) => Ok(directory),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                parent.open_private_child(name)
            }
            Err(error) => Err(error),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt as _;
    use std::{fs, path::PathBuf};
    use std::{
        process::Command,
        sync::{
            Mutex,
            atomic::{AtomicU64, Ordering},
        },
        time::{SystemTime, UNIX_EPOCH},
    };
    static NEXT: AtomicU64 = AtomicU64::new(0);
    static SOCKET_PROBES: Mutex<()> = Mutex::new(());
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let root = std::env::temp_dir().join(format!(
                "iroha-port-lease-test-{}-{stamp}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&root).unwrap();
            #[cfg(unix)]
            fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
            Self(root)
        }
        fn pool(&self) -> PortLeasePool {
            PortLeasePool::open(&self.0.join("leases")).unwrap()
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn free_block(pool: &PortLeasePool, count: u16) -> Vec<PortLease> {
        pool.allocate(count, 30_000..=49_151, 30_000)
            .unwrap()
            .expect("available test port block")
    }

    #[test]
    fn held_unbound_port_cannot_be_reserved_twice_and_drop_releases_it() {
        let _socket_probes = SOCKET_PROBES.lock().unwrap();
        let fixture = Fixture::new();
        let pool = fixture.pool();
        let leases = free_block(&pool, 1);
        let port = leases[0].port();
        let listener =
            TcpListener::bind((Ipv4Addr::LOCALHOST, port)).expect("lease leaves the port unbound");
        drop(listener);
        assert!(pool.try_port(port).unwrap().is_none());
        let identity = iroha_fs::FileMetadata::symlink_metadata(
            pool.directory.path().join(format!("port-{port}.lock")),
        )
        .unwrap()
        .identity();
        drop(leases);
        assert!(pool.try_port(port).unwrap().is_some());
        assert_eq!(
            identity,
            iroha_fs::FileMetadata::symlink_metadata(
                pool.directory.path().join(format!("port-{port}.lock")),
            )
            .unwrap()
            .identity(),
            "lease files must keep their full native identity after release",
        );
    }

    #[test]
    fn saturated_contiguous_block_releases_partial_leases_and_recovers() {
        let _socket_probes = SOCKET_PROBES.lock().unwrap();
        let fixture = Fixture::new();
        let pool = fixture.pool();
        let leases = free_block(&pool, 3);
        let base = leases[0].port();
        drop(leases);
        let middle = pool.try_port(base + 1).unwrap().unwrap();
        assert!(pool.allocate(2, base..=base + 2, base).unwrap().is_none());
        assert!(
            pool.try_port(base).unwrap().is_some(),
            "failed candidate must release its first port"
        );
        assert!(pool.try_port(base + 2).unwrap().is_some());
        drop(middle);
        let block = pool.allocate(3, base..=base + 2, base).unwrap().unwrap();
        assert_eq!(
            block.iter().map(PortLease::port).collect::<Vec<_>>(),
            vec![base, base + 1, base + 2]
        );
    }

    #[test]
    fn externally_bound_port_is_not_allocated() {
        let _socket_probes = SOCKET_PROBES.lock().unwrap();
        let fixture = Fixture::new();
        let pool = fixture.pool();
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        assert!(
            pool.try_port(listener.local_addr().unwrap().port())
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn invalid_or_too_small_ranges_do_not_allocate() {
        let fixture = Fixture::new();
        let pool = fixture.pool();
        assert!(pool.allocate(0, 30_000..=30_001, 30_000).is_err());
        assert!(pool.allocate(3, 30_000..=30_001, 30_000).unwrap().is_none());
        assert!(pool.allocate(3, 65_534..=65_535, 65_534).unwrap().is_none());
        assert!(pool.try_port(0).is_err());
    }

    fn child(pool: &PortLeasePool, port: u16, mode: &str, cwd: &Path) -> std::process::Output {
        let module = module_path!().split_once("::").unwrap().1;
        let exact = format!("{module}::lease_child");
        Command::new(std::env::current_exe().unwrap())
            .args(["--exact", &exact, "--nocapture"])
            .env("IROHA_PORT_LEASE_TEST_ROOT", pool.directory.path())
            .env("IROHA_PORT_LEASE_TEST_PORT", port.to_string())
            .env("IROHA_PORT_LEASE_TEST_MODE", mode)
            .env("CARGO_MANIFEST_DIR", cwd)
            .env("TMPDIR", cwd)
            .env("TMP", cwd)
            .env("TEMP", cwd)
            .env("LOCALAPPDATA", cwd)
            .env("USERPROFILE", cwd)
            .current_dir(cwd)
            .output()
            .unwrap()
    }
    #[test]
    fn lease_child() {
        let Some(root) = std::env::var_os("IROHA_PORT_LEASE_TEST_ROOT") else {
            return;
        };
        let port = std::env::var("IROHA_PORT_LEASE_TEST_PORT")
            .unwrap()
            .parse()
            .unwrap();
        let mode = std::env::var("IROHA_PORT_LEASE_TEST_MODE").unwrap();
        if mode == "namespace" {
            let directory = runtime_lease_directory().unwrap();
            println!("runtime-namespace={}", directory.path().display());
            return;
        }
        let pool = PortLeasePool::open(Path::new(&root)).unwrap();
        if mode == "blocked" {
            assert!(pool.try_port(port).unwrap().is_none());
            println!("lease-conflict-confirmed");
        } else {
            let _lease = pool.try_port(port).unwrap().expect("child lease");
            println!("lease-acquired-before-exit");
            // Deliberately bypass Rust Drop to verify OS release on process exit.
            std::process::exit(0);
        }
    }

    #[test]
    fn cross_process_unbound_lease_and_abrupt_exit_release() {
        let _socket_probes = SOCKET_PROBES.lock().unwrap();
        let fixture = Fixture::new();
        let pool = fixture.pool();
        let leases = free_block(&pool, 1);
        let port = leases[0].port();
        let blocked = child(&pool, port, "blocked", &fixture.0);
        assert!(
            blocked.status.success(),
            "{}",
            String::from_utf8_lossy(&blocked.stderr)
        );
        assert!(String::from_utf8_lossy(&blocked.stdout).contains("lease-conflict-confirmed"));
        drop(leases);
        let exited = child(&pool, port, "exit", &fixture.0);
        assert!(
            exited.status.success(),
            "{}",
            String::from_utf8_lossy(&exited.stderr)
        );
        assert!(String::from_utf8_lossy(&exited.stdout).contains("lease-acquired-before-exit"));
        assert!(pool.try_port(port).unwrap().is_some());
    }

    #[cfg(unix)]
    #[test]
    fn read_only_source_and_temp_environment_receive_no_allocator_writes() {
        let _socket_probes = SOCKET_PROBES.lock().unwrap();
        let fixture = Fixture::new();
        let pool = fixture.pool();
        let source = fixture.0.join("sealed-source");
        fs::create_dir(&source).unwrap();
        fs::set_permissions(&source, fs::Permissions::from_mode(0o555)).unwrap();
        let leases = free_block(&pool, 1);
        let port = leases[0].port();
        drop(leases);
        let output = child(&pool, port, "exit", &source);
        fs::set_permissions(&source, fs::Permissions::from_mode(0o700)).unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("lease-acquired-before-exit"));
        assert!(fs::read_dir(&source).unwrap().next().is_none());
        assert_eq!(
            runtime_lease_directory().unwrap().path().parent().unwrap(),
            Path::new("/tmp").canonicalize().unwrap()
        );
    }

    #[cfg(unix)]
    #[test]
    fn foreign_permissions_and_symlink_leases_are_rejected() {
        let fixture = Fixture::new();
        let pool = fixture.pool();
        let path = pool.directory.path().join("port-30000.lock");
        std::os::unix::fs::symlink(fixture.0.join("untouched"), &path).unwrap();
        assert!(pool.try_port(30_000).is_err());
        assert!(!fixture.0.join("untouched").exists());
        fs::set_permissions(pool.directory.path(), fs::Permissions::from_mode(0o755)).unwrap();
        assert!(PortLeasePool::open(pool.directory.path()).is_err());
    }

    #[test]
    fn shared_file_identity_is_refused_without_unlinking_the_stable_name() {
        let fixture = Fixture::new();
        let pool = fixture.pool();
        drop(pool.directory.open_lock("port-30000.lock").unwrap());
        let path = pool.directory.path().join("port-30000.lock");
        let identity = iroha_fs::FileMetadata::symlink_metadata(&path)
            .unwrap()
            .identity();
        let alias = fixture.0.join("alias.lock");
        fs::hard_link(&path, &alias).unwrap();
        assert!(pool.try_port(30_000).is_err());
        assert_eq!(
            iroha_fs::FileMetadata::symlink_metadata(&path)
                .unwrap()
                .identity(),
            identity
        );
        assert!(alias.is_file());
        fs::remove_file(alias).unwrap();
    }

    #[test]
    fn lease_keeps_original_parent_after_pool_is_dropped() {
        let _socket_probes = SOCKET_PROBES.lock().unwrap();
        let fixture = Fixture::new();
        let pool = fixture.pool();
        let mut leases = free_block(&pool, 1);
        let lease = leases.pop().unwrap();
        let path = pool.directory.path().to_path_buf();
        let moved = fixture.0.join("displaced");
        drop(pool);
        lease.revalidate().unwrap();
        #[cfg(windows)]
        {
            assert!(
                fs::rename(&path, &moved).is_err(),
                "live lease pins its original directory"
            );
            lease.revalidate().unwrap();
            drop(lease);
            fs::rename(&path, &moved).unwrap();
            fs::rename(&moved, &path).unwrap();
        }
        #[cfg(unix)]
        {
            fs::rename(&path, &moved).unwrap();
            assert!(lease.revalidate().is_err());
            drop(lease);
            fs::rename(&moved, &path).unwrap();
        }
    }

    #[cfg(windows)]
    fn icacls(path: &Path, arguments: &[&str]) {
        let output = Command::new("icacls")
            .arg(path)
            .args(arguments)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[cfg(windows)]
    #[test]
    fn native_directory_and_lease_acl_drift_are_errors_not_contention() {
        let _socket_probes = SOCKET_PROBES.lock().unwrap();
        for mutate_directory in [false, true] {
            let fixture = Fixture::new();
            let pool = fixture.pool();
            let mut leases = free_block(&pool, 1);
            let lease = leases.pop().unwrap();
            let path = if mutate_directory {
                pool.directory.path().to_path_buf()
            } else {
                pool.directory
                    .path()
                    .join(format!("port-{}.lock", lease.port()))
            };
            icacls(&path, &["/grant", "*S-1-1-0:(R)"]);
            assert!(lease.revalidate().is_err());
            assert!(
                pool.try_port(lease.port).is_err(),
                "unsafe ACL is not an available or contended port"
            );
            icacls(&path, &["/remove:g", "*S-1-1-0"]);
        }
    }

    #[cfg(windows)]
    #[test]
    fn native_junction_namespace_and_inherited_private_root_are_refused() {
        let fixture = Fixture::new();
        let pool = fixture.pool();
        let junction = fixture.0.join("junction");
        let result = Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&junction)
            .arg(pool.directory.path())
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(PortLeasePool::open(&junction).is_err());
        fs::remove_dir(&junction).unwrap();
        let inherited = fixture.0.join("inherited");
        fs::create_dir(&inherited).unwrap();
        assert!(PortLeasePool::open(&inherited).is_err());
        assert!(fs::read_dir(inherited).unwrap().next().is_none());
    }

    #[cfg(windows)]
    #[test]
    fn native_os_namespace_ignores_forged_environment_and_read_only_source() {
        let fixture = Fixture::new();
        let pool = fixture.pool();
        let source = PrivateDirectory::open_or_create(fixture.0.join("sealed-source")).unwrap();
        let source_path = source.path().to_path_buf();
        drop(source);
        let output = Command::new("whoami")
            .args(["/user", "/fo", "csv", "/nh"])
            .output()
            .unwrap();
        assert!(output.status.success());
        let row = String::from_utf8(output.stdout).unwrap();
        let sid = row.trim().rsplit(',').next().unwrap().trim_matches('"');
        assert!(
            sid.starts_with("S-1-")
                && sid
                    .bytes()
                    .all(|byte| byte == b'S' || byte == b'-' || byte.is_ascii_digit())
        );
        let read_only = format!("*{sid}:(RX)");
        icacls(&source_path, &["/grant:r", &read_only]);
        let expected = runtime_lease_directory().unwrap();
        let output = child(&pool, 30_000, "namespace", &source_path);
        let full = format!("*{sid}:(F)");
        icacls(&source_path, &["/grant:r", &full]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            String::from_utf8_lossy(&output.stdout)
                .contains(&format!("runtime-namespace={}", expected.path().display()))
        );
        assert!(fs::read_dir(&source_path).unwrap().next().is_none());
        assert_ne!(expected.path(), source_path);
    }
}
