//! Native retention of original mint-finality seeds; never derive or print a seed.
#![cfg(unix)]

use super::*;
use std::os::unix::fs::{DirBuilderExt, FileExt, MetadataExt};
use zeroize::Zeroizing;

const SEED_ROOT: &str = "/var/lib/taira-epoch-supervisor/seeds";

fn require(ok: bool, message: &'static str) -> Result<()> {
    if ok { Ok(()) } else { Err(eyre!(message)) }
}

fn direct_parent(path: &Path, owner: u32) -> Result<()> {
    require(
        path.is_absolute(),
        "original seed requires an absolute path",
    )?;
    let parent = path
        .parent()
        .ok_or_else(|| eyre!("original seed parent missing"))?;
    require(
        parent.canonicalize()? == parent,
        "original seed parent is not direct",
    )?;
    for ancestor in parent.ancestors() {
        let metadata = fs::symlink_metadata(ancestor)?;
        require(
            metadata.is_dir()
                && !metadata.file_type().is_symlink()
                && (metadata.uid() == 0 || metadata.uid() == owner)
                && metadata.mode() & 0o022 == 0,
            "original seed ancestor has unsafe ownership or permissions",
        )?;
    }
    Ok(())
}

fn same_metadata(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.uid() == right.uid()
        && left.gid() == right.gid()
        && left.mode() == right.mode()
        && left.nlink() == right.nlink()
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
}

fn revalidate_seed_metadata(
    path: &Path,
    file: &File,
    snapshot: &fs::Metadata,
    owner: u32,
) -> Result<()> {
    direct_parent(path, owner)?;
    let named = fs::symlink_metadata(path)?;
    require(
        !named.file_type().is_symlink()
            && same_metadata(snapshot, &named)
            && same_metadata(snapshot, &file.metadata()?),
        "original seed changed while held",
    )
}

fn read_seed_content(file: &File) -> Result<Zeroizing<[u8; 32]>> {
    let mut bytes = Zeroizing::new([0_u8; 32]);
    file.read_exact_at(bytes.as_mut(), 0)
        .map_err(|_| eyre!("original seed exact native read failed"))?;
    let mut extra = Zeroizing::new([0_u8; 1]);
    require(
        file.read_at(extra.as_mut(), 32)
            .map_err(|_| eyre!("original seed exact native read failed"))?
            == 0,
        "original seed length changed",
    )?;
    Ok(bytes)
}

/// Retain the actual original descriptor while the native coordinator frames it.
/// Public observations expose only its path and inode, never a seed digest.
pub(super) struct OriginalSeed {
    path: PathBuf,
    file: File,
    snapshot: fs::Metadata,
    owner: u32,
    original: Zeroizing<[u8; 32]>,
}

impl OriginalSeed {
    pub(super) fn open(path: &Path) -> Result<Self> {
        Self::open_for_owner(path, rustix::process::geteuid().as_raw())
    }

    fn open_for_owner(path: &Path, owner: u32) -> Result<Self> {
        direct_parent(path, owner)?;
        let file = File::from(rustix::fs::open(
            path,
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::CLOEXEC
                | rustix::fs::OFlags::NONBLOCK,
            rustix::fs::Mode::empty(),
        )?);
        let snapshot = file.metadata()?;
        require(
            snapshot.is_file()
                && snapshot.uid() == owner
                && snapshot.mode() & 0o7777 == 0o600
                && snapshot.nlink() == 1
                && snapshot.len() == 32,
            "original seed requires one owner0600 regular32-byte file",
        )?;
        revalidate_seed_metadata(path, &file, &snapshot, owner)?;
        let original = read_seed_content(&file)?;
        let selected = Self {
            path: path.to_path_buf(),
            file,
            snapshot,
            owner,
            original,
        };
        selected.revalidate()?;
        Ok(selected)
    }

    pub(super) fn identity(&self) -> (u64, u64) {
        (self.snapshot.dev(), self.snapshot.ino())
    }

    pub(super) fn revalidate(&self) -> Result<()> {
        self.read().map(drop)
    }

    /// The caller keeps this owner alive and revalidates before and after framing.
    /// The clone shares its file offset; framing must remain sequential.
    pub(super) fn stream_file(&self) -> Result<File> {
        self.revalidate()?;
        let mut stream = self.file.try_clone()?;
        std::io::Seek::rewind(&mut stream)?;
        Ok(stream)
    }

    pub(super) fn read(&self) -> Result<Zeroizing<[u8; 32]>> {
        revalidate_seed_metadata(&self.path, &self.file, &self.snapshot, self.owner)?;
        let bytes = read_seed_content(&self.file)?;
        require(
            bytes.as_ref() == self.original.as_ref(),
            "original seed content changed while held",
        )?;
        revalidate_seed_metadata(&self.path, &self.file, &self.snapshot, self.owner)?;
        Ok(bytes)
    }
}

pub(super) fn destination(network: NetworkId, index: usize) -> Result<PathBuf> {
    require(
        index < 4,
        "original seed index must be one of the four admitted peers",
    )?;
    Ok(Path::new(SEED_ROOT)
        .join(network.to_string())
        .join(format!("peer{index}.seed")))
}

/// Retain bytes from the held native original or the exact native framed stream.
/// The caller has already bound index to the sorted peer in signed public custody.
pub(super) fn retain_original(network: NetworkId, index: usize, bytes: &[u8]) -> Result<PathBuf> {
    require(
        cfg!(target_os = "linux") && rustix::process::geteuid().as_raw() == 0,
        "production original seed retention requires Linux root",
    )?;
    let target = destination(network, index)?;
    retain_at(Path::new(SEED_ROOT), &target, bytes, 0)?;
    Ok(target)
}

fn private_directory(path: &Path, owner: u32) -> Result<()> {
    direct_parent(path, owner)?;
    match fs::DirBuilder::new().mode(0o700).create(path) {
        Ok(()) => {
            File::open(
                path.parent()
                    .ok_or_else(|| eyre!("seed directory parent missing"))?,
            )?
            .sync_all()?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error.into()),
    }
    let metadata = fs::symlink_metadata(path)?;
    require(
        metadata.is_dir()
            && !metadata.file_type().is_symlink()
            && metadata.uid() == owner
            && metadata.mode() & 0o7777 == 0o700
            && path.canonicalize()? == path,
        "retained seed directory has unsafe custody",
    )?;
    File::open(path)?.sync_all()?;
    Ok(())
}

fn retain_at(root: &Path, target: &Path, bytes: &[u8], owner: u32) -> Result<()> {
    require(
        bytes.len() == 32,
        "native retained seed body must have exactly32 bytes",
    )?;
    let parent = target
        .parent()
        .ok_or_else(|| eyre!("retained seed parent missing"))?;
    require(
        parent.parent() == Some(root),
        "retained original seed escaped its network directory",
    )?;
    private_directory(root, owner)?;
    private_directory(parent, owner)?;
    let original = Zeroizing::new(
        <[u8; 32]>::try_from(bytes).map_err(|_| eyre!("native original seed length differs"))?,
    );
    match fs::symlink_metadata(target) {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            stage_and_publish(parent, target, original.as_ref(), owner)?;
        }
        Err(error) => return Err(error.into()),
    }
    let retained = OriginalSeed::open_for_owner(target, owner)?;
    let actual = retained.read()?;
    require(
        actual.as_ref() == original.as_ref(),
        "retained original seed conflicts; replacement is forbidden",
    )?;
    retained.file.sync_all()?;
    File::open(parent)?.sync_all()?;
    retained.revalidate()
}

/// A failed write leaves only its private attempt file. The final name can only
/// become visible after the exact complete original has been synced. Retained
/// failed attempts are never overwritten, implicitly repaired, or read as seeds.
fn stage_and_publish(parent: &Path, target: &Path, bytes: &[u8], owner: u32) -> Result<()> {
    let parent_file = File::from(rustix::fs::open(
        parent,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::DIRECTORY
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )?);
    let directory = parent_file.metadata()?;
    let temporary = tempfile::Builder::new()
        .prefix(".original-seed-attempt-")
        .tempfile_in(parent)?;
    let (mut file, staging) = temporary.into_parts();
    let staging = staging
        .keep()
        .map_err(|_| eyre!("could not retain native seed attempt"))?;
    let metadata = file.metadata()?;
    require(
        metadata.is_file()
            && metadata.uid() == owner
            && metadata.mode() & 0o7777 == 0o600
            && metadata.nlink() == 1
            && metadata.len() == 0,
        "native original seed staging custody differs",
    )?;
    parent_file.sync_all()?;
    file.write_all(bytes)
        .map_err(|_| eyre!("native original seed staging failed"))?;
    file.sync_all()?;
    let named_parent = fs::symlink_metadata(parent)?;
    require(
        !named_parent.file_type().is_symlink()
            && directory.dev() == named_parent.dev()
            && directory.ino() == named_parent.ino(),
        "native seed parent changed before publication",
    )?;
    let staged = OriginalSeed::open_for_owner(&staging, owner)?;
    require(
        staged.identity() == (metadata.dev(), metadata.ino()) && staged.read()?.as_ref() == bytes,
        "native original seed staging changed",
    )?;
    let source_name = staging
        .file_name()
        .ok_or_else(|| eyre!("seed staging name missing"))?;
    let target_name = target
        .file_name()
        .ok_or_else(|| eyre!("seed target name missing"))?;
    match rustix::fs::renameat_with(
        &parent_file,
        source_name,
        &parent_file,
        target_name,
        rustix::fs::RenameFlags::NOREPLACE,
    ) {
        Ok(()) | Err(rustix::io::Errno::EXIST) => {}
        Err(error) => return Err(error.into()),
    }
    parent_file.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

    fn fixture() -> (tempfile::TempDir, PathBuf) {
        let home = PathBuf::from(std::env::var_os("HOME").expect("native custody test home"))
            .canonicalize()
            .unwrap();
        let temporary = tempfile::Builder::new()
            .prefix(".epoch-seed-custody-test-")
            .tempdir_in(home)
            .unwrap();
        let directory = temporary.path().canonicalize().unwrap();
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).unwrap();
        // Shared temporary roots fail the same strict ancestor policy as production.
        (temporary, directory)
    }

    fn source(directory: &Path) -> PathBuf {
        let path = directory.join("original.seed");
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
            .unwrap();
        file.write_all(&[0xA5; 32]).unwrap();
        file.sync_all().unwrap();
        path
    }

    #[test]
    fn original_epoch_seed_rejects_shared_wrong_mode_length_and_symlink() {
        let (_temporary, directory) = fixture();
        let path = source(&directory);
        let link = directory.join("alias.seed");
        std::os::unix::fs::symlink(&path, &link).unwrap();
        assert!(OriginalSeed::open(&link).is_err());
        fs::remove_file(&link).unwrap();
        fs::hard_link(&path, &link).unwrap();
        assert!(OriginalSeed::open(&path).is_err());
        fs::remove_file(&link).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(OriginalSeed::open(&path).is_err());
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        fs::write(&path, [0xA5; 31]).unwrap();
        assert!(OriginalSeed::open(&path).is_err());
    }

    #[test]
    fn original_epoch_seed_held_descriptor_rejects_rebinding_and_changed_content() {
        let (_temporary, directory) = fixture();
        let path = source(&directory);
        let held = OriginalSeed::open(&path).unwrap();
        assert_eq!(held.read().unwrap().as_ref(), &[0xA5; 32]);
        fs::write(&path, [0x5A; 32]).unwrap();
        assert!(held.revalidate().is_err());
        fs::remove_file(&path).unwrap();
        let _replacement = source(&directory);
        assert!(held.read().is_err());
    }

    #[test]
    fn original_epoch_seed_content_binding_preserves_offset_and_rejects_metadata_collisions() {
        use std::io::{Seek as _, SeekFrom};
        let (_temporary, directory) = fixture();
        let path = source(&directory);
        let mut held = OriginalSeed::open(&path).unwrap();
        let mut shared = held.stream_file().unwrap();
        shared.seek(SeekFrom::Start(7)).unwrap();
        held.revalidate().unwrap();
        assert_eq!(held.read().unwrap().as_ref(), &[0xA5; 32]);
        assert_eq!(shared.stream_position().unwrap(), 7);

        fs::write(&path, [0x5A; 32]).unwrap();
        // Preserve the admitted bytes while making all metadata comparisons
        // identical, independently of the actual filesystem timestamp clock.
        held.snapshot = held.file.metadata().unwrap();
        assert!(same_metadata(&held.snapshot, &fs::metadata(&path).unwrap()));
        let error = held.revalidate().unwrap_err();
        assert_eq!(
            error.to_string(),
            "original seed content changed while held"
        );
        assert!(held.read().is_err());
        assert!(held.stream_file().is_err());
        assert_eq!(shared.stream_position().unwrap(), 7);

        for length in [31, 33] {
            fs::write(&path, vec![0xA5; length]).unwrap();
            held.snapshot = held.file.metadata().unwrap();
            assert!(held.revalidate().is_err());
            assert!(held.read().is_err());
            assert_eq!(shared.stream_position().unwrap(), 7);
        }
    }

    #[test]
    fn original_epoch_seed_retention_is_exact_idempotent_and_never_overwrites() {
        let (_temporary, directory) = fixture();
        let root = directory.join("seeds");
        let target = root.join("network").join("peer0.seed");
        let uid = rustix::process::geteuid().as_raw();
        retain_at(&root, &target, &[0xA5; 32], uid).unwrap();
        let inode = fs::metadata(&target).unwrap().ino();
        retain_at(&root, &target, &[0xA5; 32], uid).unwrap();
        assert_eq!(fs::metadata(&target).unwrap().ino(), inode);
        assert!(retain_at(&root, &target, &[0x5A; 32], uid).is_err());
        assert_eq!(fs::read(&target).unwrap(), vec![0xA5; 32]);
        assert_eq!(fs::metadata(&target).unwrap().mode() & 0o7777, 0o600);
    }

    #[test]
    fn original_epoch_seed_invalid_body_does_not_create_retained_paths() {
        let (_temporary, directory) = fixture();
        let root = directory.join("seeds");
        let target = root.join("network").join("peer0.seed");
        assert!(
            retain_at(
                &root,
                &target,
                &[0; 31],
                rustix::process::geteuid().as_raw()
            )
            .is_err()
        );
        assert!(!root.exists());
    }

    #[test]
    fn original_epoch_seed_fifo_is_rejected_without_waiting_for_a_writer() {
        let (_temporary, directory) = fixture();
        let path = directory.join("fifo.seed");
        // POSIX mkfifo is available on both shipping Unix platforms; rustix's
        // mkfifoat binding is unavailable on macOS.
        assert!(
            std::process::Command::new("mkfifo")
                .args(["-m", "600"])
                .arg(&path)
                .status()
                .expect("create synthetic FIFO")
                .success()
        );
        use std::os::unix::fs::FileTypeExt as _;
        assert!(fs::symlink_metadata(&path).unwrap().file_type().is_fifo());
        assert!(OriginalSeed::open(&path).is_err());
    }

    #[test]
    fn original_epoch_seed_partial_staging_never_becomes_or_blocks_final() {
        let (_temporary, directory) = fixture();
        let root = directory.join("seeds");
        let parent = root.join("network");
        let target = parent.join("peer0.seed");
        let uid = rustix::process::geteuid().as_raw();
        private_directory(&root, uid).unwrap();
        private_directory(&parent, uid).unwrap();
        let partial = parent.join(".original-seed-attempt-interrupted");
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&partial)
            .unwrap();
        file.write_all(&[0xA5; 7]).unwrap();
        file.sync_all().unwrap();
        assert!(!target.exists());
        retain_at(&root, &target, &[0xA5; 32], uid).unwrap();
        assert_eq!(fs::read(&target).unwrap(), vec![0xA5; 32]);
        assert_eq!(fs::read(&partial).unwrap(), vec![0xA5; 7]);
        assert_eq!(fs::metadata(&target).unwrap().nlink(), 1);
    }
}
