//! OS-owned leases for unbound test ports. Lease files are never unlinked.
#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::{
    fs::{self, File, OpenOptions},
    io,
    net::{Ipv4Addr, TcpListener},
    ops::RangeInclusive,
    path::{Path, PathBuf},
};

/// One open-file lock retained until its port owner is dropped or exits.
#[derive(Debug)]
pub(crate) struct PortLease {
    port: u16,
    _file: File,
}
impl PortLease {
    /// The port protected by this lease.
    pub(crate) const fn port(&self) -> u16 {
        self.port
    }
}

/// One shared per-user namespace, independent of checkout and per-test temp roots.
#[derive(Debug)]
pub(crate) struct PortLeasePool {
    directory: PathBuf,
    owner: u32,
}
impl PortLeasePool {
    /// Create or admit the stable owner-private lease directory.
    pub(crate) fn open(directory: &Path, owner: u32) -> io::Result<Self> {
        let mut builder = fs::DirBuilder::new();
        #[cfg(unix)]
        builder.mode(0o700);
        match builder.create(directory) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
        let metadata = fs::symlink_metadata(directory)?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(io::Error::other(
                "test port lease root must be a direct directory",
            ));
        }
        #[cfg(unix)]
        if metadata.uid() != owner || metadata.permissions().mode() & 0o777 != 0o700 {
            return Err(io::Error::other(
                "test port lease root must be owned by this user with mode 0700",
            ));
        }
        Ok(Self {
            directory: directory.canonicalize()?,
            owner,
        })
    }

    fn open_lease_file(&self, port: u16) -> io::Result<File> {
        let path = self.directory.join(format!("port-{port}.lock"));
        let mut options = OpenOptions::new();
        options.read(true).write(true);
        #[cfg(unix)]
        options.mode(0o600);
        let file = match options.create_new(true).open(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                let before = fs::symlink_metadata(&path)?;
                self.validate_file(&before)?;
                options.create_new(false).open(&path)?
            }
            Err(error) => return Err(error),
        };
        let opened = file.metadata()?;
        let named = fs::symlink_metadata(&path)?;
        self.validate_file(&opened)?;
        self.validate_file(&named)?;
        #[cfg(unix)]
        if opened.dev() != named.dev() || opened.ino() != named.ino() {
            return Err(io::Error::other(
                "test port lease inode changed while opening",
            ));
        }
        Ok(file)
    }

    fn validate_file(&self, metadata: &fs::Metadata) -> io::Result<()> {
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(io::Error::other(
                "test port lease must be a direct regular file",
            ));
        }
        #[cfg(unix)]
        if metadata.uid() != self.owner
            || metadata.nlink() != 1
            || metadata.permissions().mode() & 0o777 != 0o600
        {
            return Err(io::Error::other(
                "test port lease must retain owner-only single-inode custody",
            ));
        }
        Ok(())
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
        let file = self.open_lease_file(port)?;
        match file.try_lock() {
            Ok(()) => {}
            Err(std::fs::TryLockError::WouldBlock) => return Ok(None),
            Err(std::fs::TryLockError::Error(error)) => return Err(error),
        }
        match TcpListener::bind((Ipv4Addr::LOCALHOST, port)) {
            Ok(listener) => drop(listener),
            Err(error) if error.kind() == io::ErrorKind::AddrInUse => return Ok(None),
            Err(error) => return Err(error),
        }
        Ok(Some(PortLease { port, _file: file }))
    }

    /// Reserve a contiguous block in a bounded range, rolling back every partial
    /// candidate on contention. The search visits each possible base at most once.
    pub(crate) fn allocate(
        &self,
        count: u16,
        range: RangeInclusive<u16>,
        preferred_start: u16,
    ) -> io::Result<Option<Vec<PortLease>>> {
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
                return Ok(Some(leases));
            }
        }
        Ok(None)
    }
}

/// Use a common host-local namespace even when test runs override TMPDIR.
/// `/tmp` is canonicalized to its native location before private custody begins.
pub(crate) fn runtime_lease_directory(owner: u32) -> io::Result<PathBuf> {
    Ok(Path::new("/tmp")
        .canonicalize()?
        .join(format!("iroha-test-network-ports-{owner}")))
}

#[cfg(test)]
mod tests {
    use super::*;
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
        fn owner(&self) -> u32 {
            #[cfg(unix)]
            {
                fs::metadata(&self.0).unwrap().uid()
            }
            #[cfg(not(unix))]
            {
                0
            }
        }
        fn pool(&self) -> PortLeasePool {
            PortLeasePool::open(&self.0.join("leases"), self.owner()).unwrap()
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
        let inode = fs::metadata(pool.directory.join(format!("port-{port}.lock"))).unwrap();
        drop(leases);
        assert!(pool.try_port(port).unwrap().is_some());
        #[cfg(unix)]
        assert_eq!(
            inode.ino(),
            fs::metadata(pool.directory.join(format!("port-{port}.lock")))
                .unwrap()
                .ino()
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

    fn child(
        fixture: &Fixture,
        pool: &PortLeasePool,
        port: u16,
        mode: &str,
        cwd: &Path,
    ) -> std::process::Output {
        let module = module_path!().split_once("::").unwrap().1;
        let exact = format!("{module}::lease_child");
        Command::new(std::env::current_exe().unwrap())
            .args(["--exact", &exact, "--nocapture"])
            .env("IROHA_PORT_LEASE_TEST_ROOT", &pool.directory)
            .env("IROHA_PORT_LEASE_TEST_OWNER", fixture.owner().to_string())
            .env("IROHA_PORT_LEASE_TEST_PORT", port.to_string())
            .env("IROHA_PORT_LEASE_TEST_MODE", mode)
            .env("CARGO_MANIFEST_DIR", cwd)
            .env("TMPDIR", cwd)
            .current_dir(cwd)
            .output()
            .unwrap()
    }
    #[test]
    fn lease_child() {
        let Some(root) = std::env::var_os("IROHA_PORT_LEASE_TEST_ROOT") else {
            return;
        };
        let owner = std::env::var("IROHA_PORT_LEASE_TEST_OWNER")
            .unwrap()
            .parse()
            .unwrap();
        let port = std::env::var("IROHA_PORT_LEASE_TEST_PORT")
            .unwrap()
            .parse()
            .unwrap();
        let pool = PortLeasePool::open(Path::new(&root), owner).unwrap();
        if std::env::var("IROHA_PORT_LEASE_TEST_MODE").unwrap() == "blocked" {
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
        let blocked = child(&fixture, &pool, port, "blocked", &fixture.0);
        assert!(
            blocked.status.success(),
            "{}",
            String::from_utf8_lossy(&blocked.stderr)
        );
        assert!(String::from_utf8_lossy(&blocked.stdout).contains("lease-conflict-confirmed"));
        drop(leases);
        let exited = child(&fixture, &pool, port, "exit", &fixture.0);
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
        let output = child(&fixture, &pool, port, "exit", &source);
        fs::set_permissions(&source, fs::Permissions::from_mode(0o700)).unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("lease-acquired-before-exit"));
        assert!(fs::read_dir(&source).unwrap().next().is_none());
        assert_eq!(
            runtime_lease_directory(fixture.owner())
                .unwrap()
                .parent()
                .unwrap(),
            Path::new("/tmp").canonicalize().unwrap()
        );
    }

    #[cfg(unix)]
    #[test]
    fn foreign_permissions_and_symlink_leases_are_rejected() {
        let fixture = Fixture::new();
        let pool = fixture.pool();
        let path = pool.directory.join("port-30000.lock");
        std::os::unix::fs::symlink(fixture.0.join("untouched"), &path).unwrap();
        assert!(pool.try_port(30_000).is_err());
        assert!(!fixture.0.join("untouched").exists());
        fs::set_permissions(&pool.directory, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(PortLeasePool::open(&pool.directory, fixture.owner()).is_err());
    }
}
