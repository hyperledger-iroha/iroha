// A local snapshot marker never authorizes missing native execution history.

impl Kura {
    /// Reject retired marker files, directories and links without opening or removing them.
    fn reject_retired_snapshot_tail(blocks_root: &Path) -> Result<()> {
        let entries = match std::fs::read_dir(blocks_root) {
            Ok(entries) => entries,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(Error::IO(error, blocks_root.to_path_buf())),
        };
        for entry in entries {
            let entry = entry.map_err(|error| Error::IO(error, blocks_root.to_path_buf()))?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name == VERIFIED_SNAPSHOT_TAIL_FILE_NAME
                || name == "verified_snapshot_tail.norito.tmp"
                || name.starts_with(".verified-snapshot-tail-")
            {
                return Err(Error::RetiredKuraArtifact { path: entry.path() });
            }
        }
        Ok(())
    }
}
