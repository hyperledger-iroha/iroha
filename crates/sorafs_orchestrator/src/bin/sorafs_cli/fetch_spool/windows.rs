//! Native gateway spool ownership through verification, publication and temporary cleanup.

use std::{
    io::{self, Read, Seek, SeekFrom, Write},
    path::Path,
};

/// The original file drops before its optional private temporary-directory container.
pub(in super::super) struct FetchSpool {
    staged: iroha_fs::StagedOutputFile,
    temporary_root: Option<tempfile::TempDir>,
}

pub(in super::super) fn create(output: Option<&Path>) -> Result<FetchSpool, String> {
    let (directory, name, temporary_root) = match output {
        Some(path) => {
            let name = path
                .file_name()
                .ok_or_else(|| "output requires a direct regular file name".to_owned())?;
            let parent = path
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new("."));
            let directory = iroha_fs::OwnerDirectory::open_or_create(parent)
                .map_err(|error| format!("failed to retain fetch output parent: {error}"))?;
            (directory, name.to_owned(), None)
        }
        None => {
            let temporary_root = tempfile::tempdir()
                .map_err(|error| format!("failed to create private fetch spool root: {error}"))?;
            let private = iroha_fs::PrivateDirectory::open_or_create(
                temporary_root.path().join("private-spool"),
            )
            .map_err(|error| format!("failed to retain private fetch spool directory: {error}"))?;
            let directory = iroha_fs::OwnerDirectory::open(private.path())
                .map_err(|error| format!("failed to retain private fetch spool parent: {error}"))?;
            (
                directory,
                "unpublished-payload".into(),
                Some(temporary_root),
            )
        }
    };
    let staged = directory.stage_output(name);
    // Release this extra retained parent before a constructor error drops TempDir.
    // A successful stage carries its own original directory owner into FetchSpool.
    drop(directory);
    let staged =
        staged.map_err(|error| format!("failed to create private fetch spool: {error}"))?;
    Ok(FetchSpool {
        staged,
        temporary_root,
    })
}

pub(in super::super) fn publish(spool: FetchSpool, path: &Path) -> Result<(), String> {
    if spool.temporary_root.is_some() {
        return Err("verification-only fetch spool has no publication destination".to_owned());
    }
    let requested = std::path::absolute(path).map_err(|error| error.to_string())?;
    if requested != spool.staged.destination() {
        return Err("verified payload destination differs from its bound staging owner".to_owned());
    }
    spool
        .staged
        .publish()
        .map_err(|error| format!("failed to publish verified payload: {error}"))
}

impl FetchSpool {
    #[cfg(test)]
    pub(super) fn path(&self) -> &Path {
        self.staged.path()
    }
}

impl Read for FetchSpool {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.staged.read(bytes)
    }
}

impl Write for FetchSpool {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.staged.write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.staged.flush()
    }
}

impl Seek for FetchSpool {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.staged.seek(position)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_gateway_spool_preserves_seek_replay_and_send_contract() {
        fn require_gateway_writer<T: Read + Write + Seek + Send + 'static>() {}
        require_gateway_writer::<FetchSpool>();
        let temporary = tempfile::tempdir().unwrap();
        let output = temporary.path().join("payload");
        let mut spool = create(Some(&output)).unwrap();
        assert_eq!(spool.seek(SeekFrom::End(0)).unwrap(), 0);
        spool.write_all(b"canonical payload").unwrap();
        spool.flush().unwrap();
        spool.rewind().unwrap();
        let mut bytes = Vec::new();
        spool.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, b"canonical payload");
        spool.rewind().unwrap();
        publish(spool, &output).unwrap();
        assert_eq!(std::fs::read(output).unwrap(), bytes);
    }

    #[test]
    fn native_gateway_verification_only_spool_drops_file_before_private_directory() {
        let mut spool = create(None).unwrap();
        let root = spool.temporary_root.as_ref().unwrap().path().to_owned();
        let stage = spool.path().to_owned();
        let parent = stage.parent().unwrap().to_owned();
        iroha_fs::PrivateDirectory::open(&parent).unwrap();
        spool.write_all(b"not published").unwrap();
        let mut bytes = Vec::new();
        spool
            .staged
            .reader()
            .unwrap()
            .read_to_end(&mut bytes)
            .unwrap();
        assert_eq!(bytes, b"not published");
        drop(spool);
        assert!(!stage.exists());
        assert!(!parent.exists());
        assert!(!root.exists());
    }

    #[test]
    fn native_gateway_spool_refuses_changed_or_unbound_publication_path() {
        let temporary = tempfile::tempdir().unwrap();
        let output = temporary.path().join("bound");
        let other = temporary.path().join("other");
        std::fs::write(&other, b"unchanged").unwrap();
        let mut spool = create(Some(&output)).unwrap();
        let stage = spool.path().to_owned();
        spool.write_all(b"verified").unwrap();
        assert!(publish(spool, &other).is_err());
        assert!(!stage.exists());
        assert!(!output.exists());
        assert_eq!(std::fs::read(&other).unwrap(), b"unchanged");
        assert!(publish(create(None).unwrap(), &other).is_err());
        assert_eq!(std::fs::read(other).unwrap(), b"unchanged");
    }
}
