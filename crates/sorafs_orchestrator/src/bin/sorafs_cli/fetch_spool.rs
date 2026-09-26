//! Private fetch spools become visible only after complete manifest verification.
use std::{fs, path::Path};
use tempfile::NamedTempFile;

pub(super) fn create(output: Option<&Path>) -> Result<NamedTempFile, String> {
    match output {
        Some(path) => {
            let parent = path
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new("."));
            fs::create_dir_all(parent)
                .map_err(|error| format!("failed to create output directory: {error}"))?;
            NamedTempFile::new_in(parent)
        }
        None => NamedTempFile::new(),
    }
    .map_err(|error| format!("failed to create private fetch spool: {error}"))
}

pub(super) fn publish(spool: NamedTempFile, path: &Path) -> Result<(), String> {
    spool
        .as_file()
        .sync_all()
        .map_err(|error| format!("failed to sync verified payload: {error}"))?;
    // PersistError owns the temporary file. Mapping it to text drops that owner, so failed
    // publication cleans up the spool just like any earlier verification or sink failure.
    spool
        .persist(path)
        .map_err(|error| format!("failed to publish verified payload: {}", error.error))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn unverified_spool_is_private_and_drop_preserves_existing_output() {
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("payload");
        fs::write(&output, b"previous verified object").unwrap();
        let mut spool = create(Some(&output)).unwrap();
        let temporary_path = spool.path().to_owned();
        assert_eq!(temporary_path.parent(), output.parent());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                spool.as_file().metadata().unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        spool.write_all(b"unverified prefix").unwrap();
        drop(spool);
        assert!(!temporary_path.exists());
        assert_eq!(fs::read(&output).unwrap(), b"previous verified object");
    }

    #[test]
    fn failed_publication_removes_spool_and_preserves_destination() {
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("existing_directory");
        fs::create_dir(&output).unwrap();
        let mut spool = create(Some(&output)).unwrap();
        let temporary_path = spool.path().to_owned();
        spool.write_all(b"verified payload").unwrap();
        assert!(publish(spool, &output).is_err());
        assert!(output.is_dir());
        assert!(!temporary_path.exists());
    }

    #[test]
    fn successful_publication_replaces_output_with_complete_spool() {
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("nested/payload");
        let mut spool = create(Some(&output)).unwrap();
        let temporary_path = spool.path().to_owned();
        fs::write(&output, b"previous verified object").unwrap();
        spool.write_all(b"complete verified payload").unwrap();
        publish(spool, &output).unwrap();
        assert!(!temporary_path.exists());
        assert_eq!(fs::read(&output).unwrap(), b"complete verified payload");
    }
}
