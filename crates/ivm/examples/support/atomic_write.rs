use std::{
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT_TEMPORARY: AtomicU64 = AtomicU64::new(0);
struct TemporaryFile {
    path: PathBuf,
}
impl TemporaryFile {
    fn new(path: PathBuf) -> Self {
        Self { path }
    }
    fn disarm(mut self) {
        self.path = PathBuf::new();
    }
}
impl Drop for TemporaryFile {
    fn drop(&mut self) {
        if !self.path.as_os_str().is_empty() {
            let _ = fs::remove_file(&self.path);
        }
    }
}
pub(super) fn atomic_write(path: &Path, contents: &[u8]) -> io::Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    validate_real_directory(parent)?;
    validate_destination(path)?;
    let file_name = path.file_name().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("fixture target has no file name: {}", path.display()),
        )
    })?;
    let (mut temporary_file, temporary_path) = create_temporary(parent, file_name)?;
    let temporary = TemporaryFile::new(temporary_path.clone());
    temporary_file.write_all(contents)?;
    temporary_file.flush()?;
    temporary_file.sync_all()?;
    drop(temporary_file);
    validate_real_directory(parent)?;
    validate_destination(path)?;
    fs::rename(&temporary_path, path)?;
    temporary.disarm();
    sync_directory(parent)?;
    Ok(())
}
fn create_temporary(parent: &Path, file_name: &std::ffi::OsStr) -> io::Result<(File, PathBuf)> {
    for _ in 0..128 {
        let serial = NEXT_TEMPORARY.fetch_add(1, Ordering::Relaxed);
        let mut temporary_name = OsString::from(".");
        temporary_name.push(file_name);
        temporary_name.push(format!(".{}.{}.tmp", std::process::id(), serial));
        let temporary_path = parent.join(temporary_name);
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary_path)
        {
            Ok(file) => return Ok((file, temporary_path)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        format!(
            "could not reserve a unique fixture temporary file under {}",
            parent.display()
        ),
    ))
}
fn validate_real_directory(path: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "fixture output parent must be a real directory: {}",
                path.display()
            ),
        ));
    }
    Ok(())
}
fn validate_destination(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "fixture destination must be a regular non-symlink file: {}",
                    path.display()
                ),
            ))
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}
#[cfg(unix)]
fn sync_directory(path: &Path) -> io::Result<()> {
    File::open(path)?.sync_all()
}
#[cfg(not(unix))]
fn sync_directory(_path: &Path) -> io::Result<()> {
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::atomic_write;
    use std::{
        fs,
        sync::atomic::{AtomicU64, Ordering},
    };
    static NEXT_TEST_DIRECTORY: AtomicU64 = AtomicU64::new(0);
    fn test_directory(label: &str) -> std::path::PathBuf {
        let serial = NEXT_TEST_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "ivm-fixture-atomic-write-{}-{serial}-{label}",
            std::process::id()
        ))
    }
    #[test]
    fn publication_replaces_complete_file_and_leaves_no_temporary() {
        let directory = test_directory("replace");
        fs::create_dir_all(&directory).expect("create test directory");
        let path = directory.join("fixture.json");
        fs::write(&path, b"old").expect("write old fixture");
        atomic_write(&path, b"complete-new-fixture").expect("publish fixture");
        assert_eq!(
            fs::read(&path).expect("read fixture"),
            b"complete-new-fixture"
        );
        assert_eq!(
            fs::read_dir(&directory)
                .expect("read test directory")
                .count(),
            1,
            "owned temporary file must be removed"
        );
        fs::remove_dir_all(directory).expect("remove test directory");
    }
    #[test]
    fn publication_rejects_non_file_destination_without_mutation() {
        let directory = test_directory("directory-target");
        let path = directory.join("fixture.json");
        fs::create_dir_all(&path).expect("create directory destination");
        assert!(atomic_write(&path, b"fixture").is_err());
        assert!(path.is_dir());
        assert_eq!(
            fs::read_dir(&directory)
                .expect("read test directory")
                .count(),
            1
        );
        fs::remove_dir_all(directory).expect("remove test directory");
    }
    #[cfg(unix)]
    #[test]
    fn publication_rejects_symlink_destination_without_following_it() {
        use std::os::unix::fs::symlink;
        let directory = test_directory("symlink-target");
        fs::create_dir_all(&directory).expect("create test directory");
        let target = directory.join("target.json");
        let path = directory.join("fixture.json");
        fs::write(&target, b"untouched").expect("write symlink target");
        symlink(&target, &path).expect("create symlink destination");
        assert!(atomic_write(&path, b"replacement").is_err());
        assert_eq!(fs::read(&target).expect("read target"), b"untouched");
        assert!(
            fs::symlink_metadata(&path)
                .expect("inspect symlink")
                .file_type()
                .is_symlink()
        );
        fs::remove_dir_all(directory).expect("remove test directory");
    }
    #[cfg(unix)]
    #[test]
    fn publication_rejects_symlink_parent_without_following_it() {
        use std::os::unix::fs::symlink;
        let directory = test_directory("symlink-parent");
        let real_parent = directory.join("real");
        let linked_parent = directory.join("linked");
        fs::create_dir_all(&real_parent).expect("create real parent");
        symlink(&real_parent, &linked_parent).expect("create parent symlink");
        assert!(atomic_write(&linked_parent.join("fixture.json"), b"fixture").is_err());
        assert!(!real_parent.join("fixture.json").exists());
        fs::remove_dir_all(directory).expect("remove test directory");
    }
}
