impl Kura {
    fn reject_retired_pipeline_roster_sidecars(blocks_root: &Path) -> Result<()> {
        let pipeline_dir = blocks_root.join(PIPELINE_DIR_NAME);
        let entries = match std::fs::read_dir(&pipeline_dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(Error::IO(error, pipeline_dir)),
        };
        for entry in entries {
            let entry = entry.map_err(|error| Error::IO(error, pipeline_dir.clone()))?;
            if entry
                .file_name()
                .to_string_lossy()
                .starts_with("roster_sidecars")
            {
                return Err(Error::RetiredKuraArtifact { path: entry.path() });
            }
        }
        Ok(())
    }
}
