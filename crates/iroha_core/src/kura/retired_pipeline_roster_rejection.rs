impl Kura {
    fn reject_retired_pipeline_artifacts(blocks_root: &Path) -> Result<()> {
        let pipeline_dir = blocks_root.join(PIPELINE_DIR_NAME);
        let entries = match std::fs::read_dir(&pipeline_dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(Error::IO(error, pipeline_dir)),
        };
        for entry in entries {
            let entry = entry.map_err(|error| Error::IO(error, pipeline_dir.clone()))?;
            let file_name = entry.file_name();
            let file_name = file_name.to_string_lossy();
            let retired_json = file_name
                .strip_prefix("block_")
                .and_then(|name| name.strip_suffix(".json"))
                .is_some_and(|height| height.parse::<u64>().is_ok());
            if file_name.starts_with("roster_sidecars") || retired_json {
                return Err(Error::RetiredKuraArtifact { path: entry.path() });
            }
        }
        Ok(())
    }
}
